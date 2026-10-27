/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.6.2026
 *   nucleo/drivers/tempo/rtc.rs — Driver do relógio de tempo real (RTC)
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Relógio: RTC do CMOS (porta 0x70 = índice, porta 0x71 = dados):
 *            segundo 0x00, minuto 0x02, hora 0x04, dia 0x07, mês 0x08,
 *            ano 0x09, status A 0x0A (UIP) e status B 0x0B (BCD e 12/24h).
 *
 *   Ao contrário do PIT (que é mudo), este driver ESCREVE NA TELA: na
 *   segunda linha (y = 14, a mesma onde a RAM mostrava — ela migrou para
 *   a terceira, y = 26), a DATA fica encostada no lado ESQUERDO (x = 0,
 *   "DD/MM/AAAA") e a HORA no lado DIREITO (x = 256, "HH:MM:SS", 8
 *   caracteres de 8 px encostados na borda dos 320 px do modo 13h).
 *
 *   A hora se atualiza sozinha: o nucleo/nucleo.rs chama rtc_atualizar()
 *   num laço a cada pit_esperar_ms(250). Este módulo só redesenha os
 *   caracteres que MUDARAM — a célula de 8x8 é apagada (preta) antes, para
 *   o glifo antigo não deixar rabisco (ex.: trocar "8" por "1"), sem
 *   piscar o resto da tela.
 *
 *   A leitura espera o bit de atualização em andamento (UIP, bit 7 do
 *   status A) liberar, para não capturar valores pela metade na virada do
 *   segundo. O status B diz o formato: bit 2 (DM) = 1 os valores vêm em
 *   binário (0 = BCD) e bit 1 = 1 a hora é de 24 h (0 = 12 h); em 12 h,
 *   o bit 7 da hora é o indicador PM.
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com video.o, texto.o, memoria.o, pit.o e nucleo.o para formar o
 *   /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::arch::asm;
use core::sync::atomic::{AtomicU8, Ordering};

/* Módulos linkados junto no nucos: desenho de texto e pixels. */
extern "C" {
    fn texto_escrever(texto: *const u8, tamanho: usize, x_inicial: usize, y: usize);
    fn video_pixel(x: usize, y: usize, cor: u8);
}

/* Portas do CMOS/RTC. O bit 7 da porta 0x70 é o NMI — deixamos 0 (ligado),
 * igual ao que o memoria.rs já faz ao ler o CMOS. */
const PORTA_INDICE: u16 = 0x70;
const PORTA_DADOS: u16 = 0x71;

/* Registradores do RTC. */
const REG_SEGUNDOS: u8 = 0x00;
const REG_MINUTOS: u8 = 0x02;
const REG_HORAS: u8 = 0x04;
const REG_DIA: u8 = 0x07;
const REG_MES: u8 = 0x08;
const REG_ANO: u8 = 0x09;
const REG_STATUS_A: u8 = 0x0A;
const REG_STATUS_B: u8 = 0x0B;

/* Layout da segunda linha (modo 13h: 320x200). */
const LARGURA_TELA: usize = 320;
const LINHA: usize = 14; /* 2ª linha (a RAM foi para y = 26) */
const COLUNA_DATA: usize = 0; /* data encostada na esquerda */
const TAMANHO_DATA: usize = 10; /* "DD/MM/AAAA" */
const TAMANHO_HORA: usize = 8; /* "HH:MM:SS" */
const COLUNA_HORA: usize = LARGURA_TELA - TAMANHO_HORA * 8; /* direita: 256 */
const CELULA: usize = 8; /* fonte 8x8 */
const COR_PRETO: u8 = 0;

/* O que já está desenhado na tela: só redesenhamos o que mudou. */
static DATA_DESENHADA: [AtomicU8; TAMANHO_DATA] =
    [const { AtomicU8::new(0) }; TAMANHO_DATA];
static HORA_DESENHADA: [AtomicU8; TAMANHO_HORA] =
    [const { AtomicU8::new(0) }; TAMANHO_HORA];

/* Lê um registrador do RTC. */
fn ler_registro(indice: u8) -> u8 {
    unsafe {
        asm!("out dx, al", in("dx") PORTA_INDICE, in("al") indice, options(nomem, nostack));
        let valor: u8;
        asm!("in al, dx", out("al") valor, in("dx") PORTA_DADOS, options(nomem, nostack));
        valor
    }
}

/* Espera o RTC terminar a atualização (bit 7 = UIP do status A) para não
 * ler a hora pela metade na virada do segundo. Limite de tentativas para
 * travar nunca. */
fn esperar_atualizacao() {
    let mut tentativas: u32 = 0;
    while ler_registro(REG_STATUS_A) & 0x80 != 0 && tentativas < 1_000_000 {
        tentativas += 1;
    }
}

/* Converte o valor do registrador: BCD (4 bits por dígito) ou binário. */
fn converter(valor: u8, bcd: bool) -> u8 {
    if bcd {
        (valor >> 4) * 10 + (valor & 0x0F)
    } else {
        valor
    }
}

/* Garante valores plausíveis (o CMOS pode estar zoeira fora do ar). */
fn limitar(valor: u8, maximo: u8) -> u8 {
    if valor > maximo {
        0
    } else {
        valor
    }
}

/* "DD/MM/AAAA" — o registrador do ano guarda 0..99; usamos 2000 + ano. */
fn montar_data(dia: u8, mes: u8, ano: u8) -> [u8; TAMANHO_DATA] {
    let ano_completo = 2000 + ano as u16;
    [
        b'0' + dia / 10,
        b'0' + dia % 10,
        b'/',
        b'0' + mes / 10,
        b'0' + mes % 10,
        b'/',
        b'0' + (ano_completo / 1000) as u8,
        b'0' + (ano_completo / 100 % 10) as u8,
        b'0' + (ano_completo / 10 % 10) as u8,
        b'0' + (ano_completo % 10) as u8,
    ]
}

/* "HH:MM:SS" em 24 h. */
fn montar_hora(hora: u8, minuto: u8, segundo: u8) -> [u8; TAMANHO_HORA] {
    [
        b'0' + hora / 10,
        b'0' + hora % 10,
        b':',
        b'0' + minuto / 10,
        b'0' + minuto % 10,
        b':',
        b'0' + segundo / 10,
        b'0' + segundo % 10,
    ]
}

/* Lê o RTC inteiro e devolve a data e a hora já formatadas. */
fn ler_data_hora() -> ([u8; TAMANHO_DATA], [u8; TAMANHO_HORA]) {
    esperar_atualizacao();

    /* Bits do status B (MC146818): bit 2 = DM (1 = binário, 0 = BCD) e
     * bit 1 = 24 h (1 = 24 h, 0 = 12 h). Confundir estes dois faz a data
     * virar "00/00/2038" (BCD lido como binário) e a hora, 12 h errada. */
    let status_b = ler_registro(REG_STATUS_B);
    let bcd = status_b & 0x04 == 0;             /* DM = 0 → valores em BCD */
    let vinte_e_quatro = status_b & 0x02 != 0;   /* bit 1 = 1 → hora 24 h  */

    let segundo = limitar(converter(ler_registro(REG_SEGUNDOS) & 0x7F, bcd), 59);
    let minuto = limitar(converter(ler_registro(REG_MINUTOS) & 0x7F, bcd), 59);
    let bruto_hora = ler_registro(REG_HORAS);
    let dia = limitar(converter(ler_registro(REG_DIA) & 0x7F, bcd), 31);
    let mes = limitar(converter(ler_registro(REG_MES) & 0x7F, bcd), 12);
    let ano = converter(ler_registro(REG_ANO), bcd).min(99);

    let hora = if vinte_e_quatro {
        limitar(converter(bruto_hora & 0x7F, bcd), 23)
    } else {
        /* Formato 12 h: bit 7 = PM; 12 AM vira 00, 12 PM continua 12. */
        let pm = bruto_hora & 0x80 != 0;
        let h12 = converter(bruto_hora & 0x7F, bcd).min(12);
        let base = if h12 == 12 { 0 } else { h12 };
        base + if pm { 12 } else { 0 }
    };

    (montar_data(dia, mes, ano), montar_hora(hora, minuto, segundo))
}

/* Apaga uma célula de 8x8 (fundo preto): o glifo antigo não deixa
 * rabisco quando o novo tem menos pixels (ex.: "8" → "1"). */
fn limpar_celula(x: usize, y: usize) {
    let mut dy = 0;
    while dy < CELULA {
        let mut dx = 0;
        while dx < CELULA {
            unsafe { video_pixel(x + dx, y + dy, COR_PRETO) };
            dx += 1;
        }
        dy += 1;
    }
}

/* Desenha só os caracteres que mudaram em relação ao que está na tela.
 * get_unchecked em vez de [i]: o indexamento normal chamaria
 * core::panicking::panic_bounds_check, que não existe na linkagem plana
 * do nucos (o mesmo motivo de texto.rs ler a fonte com ponteiro). */
fn redesenhar(novo: &[u8], x_inicial: usize, anterior: &[AtomicU8]) {
    let mut i = 0;

    while i < novo.len() {
        let codigo = unsafe { *novo.get_unchecked(i) };
        let guardado = unsafe { anterior.get_unchecked(i) };

        if guardado.load(Ordering::Relaxed) != codigo {
            let x = x_inicial + i * CELULA;
            limpar_celula(x, LINHA);
            unsafe { texto_escrever(&codigo as *const u8, 1, x, LINHA) };
            guardado.store(codigo, Ordering::Relaxed);
        }

        i += 1;
    }
}

/* Primeira leitura: desenha a data na esquerda e a hora na direita da
 * segunda linha. As células começam zeradas, então tudo é desenhado. */
#[no_mangle]
pub extern "C" fn rtc_iniciar() {
    rtc_atualizar();
}

/* Relê o RTC e redesenha APENAS o que mudou — o núcleo chama esta função
 * em laço (a cada pit_esperar_ms(250)), e é assim que a hora se atualiza
 * sozinha na tela. */
#[no_mangle]
pub extern "C" fn rtc_atualizar() {
    let (data, hora) = ler_data_hora();
    redesenhar(&data, COLUNA_DATA, &DATA_DESENHADA);
    redesenhar(&hora, COLUNA_HORA, &HORA_DESENHADA);
}
