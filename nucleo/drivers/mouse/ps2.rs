/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.6.2026
 *   nucleo/drivers/mouse/ps2.rs — Driver do mouse PS/2
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Mouse  : PS/2 padrão (porta 0x60/0x64), modo stream, 3 bytes/pacote.
 *
 *   Este driver é MUDO: não desenha nada na tela. Apenas inicializa o
 *   controlador PS/2, habilita o mouse e fornece funções para ler
 *   pacotes e extrair movimento (dx, dy) e botões.
 *
 *   Protocolo PS/2 (mouse):
 *     Byte 0: Y overflow | X overflow | Y sign | X sign | 1 | Middle | Right | Left
 *     Byte 1: X movement (delta X, complemento de 2)
 *     Byte 2: Y movement (delta Y, complemento de 2)
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com os demais .o para formar o /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::arch::asm;
use core::sync::atomic::{AtomicI16, AtomicU8, Ordering};

/* Portas do controlador PS/2 (8042). */
const PORTA_DADOS: u16 = 0x60;
const PORTA_COMANDO: u16 = 0x64;
const PORTA_STATUS: u16 = 0x64;  /* mesma porta do comando, leitura = status */

/* Comandos para o controlador (porta 0x64). */
const CMD_DESABILITAR_PORTA2: u8 = 0xA7;  /* desabilita segunda porta (mouse) */
const CMD_HABILITAR_PORTA2: u8 = 0xA8;    /* habilita segunda porta */
const CMD_TESTE_PORTA2: u8 = 0xA9;        /* testa porta do mouse */
const CMD_ESCREVER_PORTA2: u8 = 0xD4;     /* próximo byte vai para porta 2 (mouse) */
const CMD_LER_BYTE_PORTA2: u8 = 0x20;     /* lê byte de configuração da porta 2 */
const CMD_ESCREVER_BYTE_PORTA2: u8 = 0x60; /* escreve byte de configuração da porta 2 */

/* Comandos para o mouse (enviados via 0xD4 + 0x60). */
const MOUSE_RESET: u8 = 0xFF;      /* reset - mouse responde 0xFA, depois 0xAA 0x00 */
const MOUSE_HABILITAR: u8 = 0xF4;  /* habilita streaming de dados */
const MOUSE_DESABILITAR: u8 = 0xF5; /* desabilita streaming */
const MOUSE_DEFINIR_PADRAO: u8 = 0xF6; /* restaura padrões */
const MOUSE_LER_DADOS: u8 = 0xEB;  /* lê um pacote (modo remoto) */

/* Bits do registro de status (porta 0x64, leitura). */
const STAT_BUFFER_CHEIO: u8 = 0x01;  /* output buffer cheio (pode ler 0x60) */
const STAT_BUFFER_ENTRADA: u8 = 0x02; /* input buffer cheio (não escreva ainda) */
const STAT_DADOS_MOUSE: u8 = 0x20;   /* dado veio do mouse (porta 2) */

/* Estado do mouse (pacote de 3 bytes). */
static PACOTE: [AtomicU8; 3] = [
    const { AtomicU8::new(0) },
    const { AtomicU8::new(0) },
    const { AtomicU8::new(0) },
];
static INDICE_PACOTE: AtomicU8 = AtomicU8::new(0);

/* Movimento acumulado desde a última leitura (para o kernel consumir). */
static DX_ACUMULADO: AtomicI16 = AtomicI16::new(0);
static DY_ACUMULADO: AtomicI16 = AtomicI16::new(0);

/* Botões atuais (bit 0 = esquerdo, 1 = direito, 2 = meio). */
static BOTOES_ATUAIS: AtomicU8 = AtomicU8::new(0);

/* Indica se o mouse foi inicializado com sucesso. */
static INICIALIZADO: AtomicU8 = AtomicU8::new(0);

/* ===== Funções de baixo nível (portas) ===== */

#[inline]
fn esperar_buffer_entrada_livre() {
    let mut tentativas: u32 = 0;
    loop {
        let status = inb(PORTA_STATUS);
        if status & STAT_BUFFER_ENTRADA == 0 {
            break;
        }
        tentativas += 1;
        if tentativas > 1_000_000 {
            break; /* evita travar para sempre */
        }
    }
}

#[inline]
fn esperar_buffer_saida_cheio() -> bool {
    let mut tentativas: u32 = 0;
    loop {
        let status = inb(PORTA_STATUS);
        if status & STAT_BUFFER_CHEIO != 0 {
            return true;
        }
        tentativas += 1;
        if tentativas > 1_000_000 {
            return false;
        }
    }
}

#[inline]
fn outb(porta: u16, valor: u8) {
    esperar_buffer_entrada_livre();
    unsafe {
        asm!("out dx, al", in("dx") porta, in("al") valor, options(nomem, nostack));
    }
}

#[inline]
fn inb(porta: u16) -> u8 {
    unsafe {
        let valor: u8;
        asm!("in al, dx", out("al") valor, in("dx") porta, options(nomem, nostack));
        valor
    }
}

/* Envia comando para o mouse (via porta 0xD4 do controlador). */
fn mouse_enviar_comando(comando: u8) -> bool {
    outb(PORTA_COMANDO, CMD_ESCREVER_PORTA2);
    outb(PORTA_DADOS, comando);
    esperar_buffer_saida_cheio();
    let resposta = inb(PORTA_DADOS);
    resposta == 0xFA  /* ACK */
}

/* Lê um byte de resposta do mouse. */
fn mouse_ler_resposta() -> Option<u8> {
    if esperar_buffer_saida_cheio() {
        Some(inb(PORTA_DADOS))
    } else {
        None
    }
}

/* ===== Manipulação de pacotes ===== */

/* Processa um byte recebido do mouse (chamado pela ISR ou polling).
 * Usa get_unchecked para evitar panic_bounds_check (sem std, panic não existe). */
fn processar_byte_mouse(byte: u8) {
    let idx = INDICE_PACOTE.load(Ordering::Relaxed) as usize;

    unsafe { (*PACOTE.as_ptr().add(idx)).store(byte, Ordering::Relaxed) };

    let proximo = (idx + 1) % 3;
    INDICE_PACOTE.store(proximo as u8, Ordering::Relaxed);

    /* Pacote completo? Byte 0 sempre tem bit 3 = 1 (bit de sincronismo). */
    if proximo == 0 {
        let b0 = unsafe { (*PACOTE.as_ptr().add(0)).load(Ordering::Relaxed) };
        let b1 = unsafe { (*PACOTE.as_ptr().add(1)).load(Ordering::Relaxed) };
        let b2 = unsafe { (*PACOTE.as_ptr().add(2)).load(Ordering::Relaxed) };

        /* Validação básica: bit 3 do byte 0 deve ser 1. */
        if b0 & 0x08 != 0 {
            /* Botões (bits 0,1,2). */
            BOTOES_ATUAIS.store(b0 & 0x07, Ordering::Relaxed);

            /* Delta X (byte 1, signed 8-bit). */
            let dx = b1 as i8 as i16;

            /* Delta Y (byte 2, signed 8-bit). No PS/2, Y positivo = cima,
             * mas a tela costuma ter Y positivo = baixo. O consumidor decide. */
            let dy = b2 as i8 as i16;

            /* Acumula para leitura atômica posterior. */
            DX_ACUMULADO.fetch_add(dx, Ordering::Relaxed);
            DY_ACUMULADO.fetch_add(dy, Ordering::Relaxed);
        }
    }
}

/* ===== API pública (extern "C") ===== */

/* Inicializa o mouse PS/2.
 * Retorna 1 se sucesso, 0 se falha.
 * Sequência: desabilita portas → habilita porta 2 → testa → habilita mouse → streaming. */
#[no_mangle]
pub extern "C" fn mouse_ps2_iniciar() -> u8 {
    /* 1. Desabilita ambas as portas (teclado e mouse). */
    outb(PORTA_COMANDO, 0xAD);  /* desabilita porta 1 (teclado) */
    outb(PORTA_COMANDO, CMD_DESABILITAR_PORTA2);

    /* 2. Limpa buffer de saída (pode ter lixo). */
    while inb(PORTA_STATUS) & STAT_BUFFER_CHEIO != 0 {
        let _ = inb(PORTA_DADOS);
    }

    /* 3. Configura byte de configuração do controlador:
     *    - habilita IRQ12 (mouse) no bit 1
     *    - habilita porta 2 no bit 5 */
    outb(PORTA_COMANDO, CMD_LER_BYTE_PORTA2);
    if esperar_buffer_saida_cheio() {
        let mut config = inb(PORTA_DADOS);
        config |= 0x02;  /* bit 1 = IRQ12 habilitado */
        config |= 0x20;  /* bit 5 = porta 2 habilitada (alguns controladores) */
        config &= !0x10; /* bit 4 = 0 (clock da porta 2 habilitado) */
        outb(PORTA_COMANDO, CMD_ESCREVER_BYTE_PORTA2);
        outb(PORTA_DADOS, config);
    }

    /* 4. Testa a porta do mouse. */
    outb(PORTA_COMANDO, CMD_TESTE_PORTA2);
    if esperar_buffer_saida_cheio() {
        let teste = inb(PORTA_DADOS);
        if teste != 0x00 {
            return 0; /* falha no teste da porta */
        }
    } else {
        return 0;
    }

    /* 5. Habilita a porta do mouse. */
    outb(PORTA_COMANDO, CMD_HABILITAR_PORTA2);

    /* 6. Reset do mouse. */
    if !mouse_enviar_comando(MOUSE_RESET) {
        return 0;
    }
    /* Espera dois bytes: 0xAA (self-test passed) e 0x00 (device ID). */
    mouse_ler_resposta(); /* 0xAA */
    mouse_ler_resposta(); /* 0x00 */

    /* 7. Define padrões (opcional, mas seguro). */
    mouse_enviar_comando(MOUSE_DEFINIR_PADRAO);

    /* 8. Habilita streaming de dados (mouse envia pacotes ao mover). */
    if !mouse_enviar_comando(MOUSE_HABILITAR) {
        return 0;
    }

    /* Zera estado. */
    INDICE_PACOTE.store(0, Ordering::Relaxed);
    DX_ACUMULADO.store(0, Ordering::Relaxed);
    DY_ACUMULADO.store(0, Ordering::Relaxed);
    BOTOES_ATUAIS.store(0, Ordering::Relaxed);
    for i in 0..3 {
        PACOTE[i].store(0, Ordering::Relaxed);
    }

    INICIALIZADO.store(1, Ordering::Relaxed);
    1
}

/* Desabilita o mouse (para streaming). */
#[no_mangle]
pub extern "C" fn mouse_ps2_desabilitar() {
    if INICIALIZADO.load(Ordering::Relaxed) != 0 {
        mouse_enviar_comando(MOUSE_DESABILITAR);
        outb(PORTA_COMANDO, CMD_DESABILITAR_PORTA2);
        INICIALIZADO.store(0, Ordering::Relaxed);
    }
}

/* Verifica se há dados do mouse no buffer do controlador e processa.
 * Deve ser chamado periodicamente (polling) ou dentro da ISR do IRQ12.
 * Retorna número de bytes processados (0 ou 1 por chamada). */
#[no_mangle]
pub extern "C" fn mouse_ps2_poll() -> u8 {
    if INICIALIZADO.load(Ordering::Relaxed) == 0 {
        return 0;
    }

    let status = inb(PORTA_STATUS);
    if status & STAT_BUFFER_CHEIO != 0 && status & STAT_DADOS_MOUSE != 0 {
        let byte = inb(PORTA_DADOS);
        processar_byte_mouse(byte);
        1
    } else {
        0
    }
}

/* Processa todos os bytes disponíveis do mouse de uma vez.
 * Útil no laço principal se não houver IRQ. */
#[no_mangle]
pub extern "C" fn mouse_ps2_processar_tudo() -> u32 {
    let mut count: u32 = 0;
    while mouse_ps2_poll() != 0 {
        count += 1;
    }
    count
}

/* Lê o movimento acumulado desde a última chamada e zera os acumuladores.
 * dx, dy são ponteiros para i16 (podem ser nulos se não interessar).
 * Retorna botões atuais (bit 0=esq, 1=dir, 2=meio). */
#[no_mangle]
pub extern "C" fn mouse_ps2_ler_movimento(dx: *mut i16, dy: *mut i16) -> u8 {
    let dx_val = DX_ACUMULADO.swap(0, Ordering::Relaxed);
    let dy_val = DY_ACUMULADO.swap(0, Ordering::Relaxed);
    let botoes = BOTOES_ATUAIS.load(Ordering::Relaxed);

    if !dx.is_null() {
        unsafe { *dx = dx_val };
    }
    if !dy.is_null() {
        unsafe { *dy = dy_val };
    }

    botoes
}

/* Lê apenas os botões atuais (não zera nada). */
#[no_mangle]
pub extern "C" fn mouse_ps2_ler_botoes() -> u8 {
    BOTOES_ATUAIS.load(Ordering::Relaxed)
}

/* Verifica se o mouse está inicializado. */
#[no_mangle]
pub extern "C" fn mouse_ps2_esta_inicializado() -> u8 {
    INICIALIZADO.load(Ordering::Relaxed)
}

/* Estrutura para passar estado completo de uma vez (opcional). */
#[repr(C)]
pub struct MouseEstado {
    pub dx: i16,
    pub dy: i16,
    pub botoes: u8,
    pub inicializado: u8,
}

/* Lê estado completo e zera movimento acumulado. */
#[no_mangle]
pub extern "C" fn mouse_ps2_ler_estado(estado: *mut MouseEstado) {
    if estado.is_null() {
        return;
    }
    unsafe {
        (*estado).dx = DX_ACUMULADO.swap(0, Ordering::Relaxed);
        (*estado).dy = DY_ACUMULADO.swap(0, Ordering::Relaxed);
        (*estado).botoes = BOTOES_ATUAIS.load(Ordering::Relaxed);
        (*estado).inicializado = INICIALIZADO.load(Ordering::Relaxed);
    }
}