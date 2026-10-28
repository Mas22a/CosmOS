/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.7.2026
 *   nucleo/drivers/tempo/pit.rs — Driver de tempo (PIT) do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Relógio: PIT (Programmable Interval Timer) — canal 0, portas 0x43
 *            (comando) e 0x40 (contagem), base de 1193182 Hz (1,193182 MHz).
 *
 *   Este driver é MUDO: não escreve na tela nem mexe na memória de vídeo.
 *   Ele só programa as portas do PIT e espera contando ticks por polling —
 *   a CPU está com cli desde o inimin (sem IDT não existe IRQ0), então aqui
 *   vale a mesma técnica de polling provada no ini/inicio.c. Quando a
 *   interrupção do timer for implantada, o modo periódico de 100 Hz que
 *   deixamos programado já estará rodando.
 *
 *   O que este módulo exporta (extern "C", chamado pelo nucleo/nucleo.rs):
 *     - pit_iniciar()             → canal 0 em modo periódico a 100 Hz;
 *                                   devolve a frequência real em Hz;
 *     - pit_esperar_ms(ms)        → espera silenciosa em milissegundos;
 *     - pit_esperar_ticks(ticks)  → espera em ticks do PIT (1193182/s);
 *     - pit_ler_contador()        → contador atual do canal 0 (latch).
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com video.o, texto.o, memoria.o e nucleo.o para formar o
 *   /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::arch::asm;

/* Portas do PIT: 0x43 recebe o comando, 0x40 é o contador do canal 0. */
const PORTA_COMANDO: u16 = 0x43;
const PORTA_CANAL_0: u16 = 0x40;

/* O PIT conta a 1193182 Hz (11,93182 MHz) — a base de todo cálculo. */
const FREQUENCIA_BASE: u32 = 1_193_182;

/* Carga máxima de um canal do PIT: 65536 (escrita 0) ≈ 54,9 ms. */
const MAX_CARGA: u32 = 65_536;

/* Relógio do sistema: canal 0 em modo periódico a 100 Hz (10 ms/tick). */
const FREQUENCIA_HZ: u32 = 100;

/* Comandos do canal 0 (byte escrito na porta 0x43):
 *   0x36 — canal 0, lo/hi, modo 3 (onda quadrada = periódico);
 *   0x30 — canal 0, lo/hi, modo 0 (one-shot, usado nas esperas);
 *   0x00 — trava (latch) o contador do canal 0 para leitura. */
const CMD_PERIODO: u8 = 0x36;
const CMD_ONE_SHOT: u8 = 0x30;
const CMD_TRAVAR: u8 = 0x00;

/* Escreve um byte numa porta de E/S. */
fn outb(porta: u16, valor: u8) {
    unsafe {
        asm!("out dx, al", in("dx") porta, in("al") valor, options(nomem, nostack));
    }
}

/* Lê um byte de uma porta de E/S. */
fn inb(porta: u16) -> u8 {
    unsafe {
        let valor: u8;
        asm!("in al, dx", out("al") valor, in("dx") porta, options(nomem, nostack));
        valor
    }
}

/* Programa o contador do canal 0 com a carga pedida e o comando dado. */
fn programar(carga: u16, comando: u8) {
    outb(PORTA_COMANDO, comando);
    outb(PORTA_CANAL_0, (carga & 0xFF) as u8);
    outb(PORTA_CANAL_0, (carga >> 8) as u8);
}

/* Carga inteira do modo periódico de FREQUENCIA_HZ. */
fn carga_periodica() -> u16 {
    (FREQUENCIA_BASE / FREQUENCIA_HZ) as u16
}

/* Espera o contador do canal 0 chegar a 0 (one-shot), por polling no
 * contador travado — mesma técnica do espera_zerar() do ini/inicio.c. */
fn esperar_zerar() {
    while pit_ler_contador() != 0 {}
}

/* Deixa o PIT canal 0 em modo periódico (100 Hz) e devolve a frequência
 * real em Hz — o divisor é inteiro, então o resultado é o arredondado.
 * Em silêncio: só as portas 0x43 e 0x40, nada na tela. */
#[no_mangle]
pub extern "C" fn pit_iniciar() -> u32 {
    let carga = carga_periodica();
    programar(carga, CMD_PERIODO);
    FREQUENCIA_BASE / (carga as u32)
}

/* Espera `total` ticks do PIT (1 tick = 1/1193182 s), em blocos de no
 * máximo 65536 ticks (~54,9 ms), sempre por polling. No fim o canal 0
 * volta ao modo periódico de pit_iniciar(). */
#[no_mangle]
pub extern "C" fn pit_esperar_ticks(total: u32) {
    let mut resto = total;

    while resto >= MAX_CARGA {
        programar(0, CMD_ONE_SHOT); /* carga 0 = 65536 ticks ≈ 54,9 ms */
        esperar_zerar();
        resto -= MAX_CARGA;
    }

    if resto > 0 {
        programar(resto as u16, CMD_ONE_SHOT);
        esperar_zerar();
    }

    /* A espera usou o canal em one-shot; devolve o relógio ao modo
     * periódico. pit_esperar_ticks(0) não espera e também deixa assim. */
    programar(carga_periodica(), CMD_PERIODO);
}

/* Espera `ms` milissegundos, em silêncio. A conversão é fatiada em
 * pedaços de no máximo 3000 ms para ms * 1193182 caber num u32
 * (3000 * 1193182 = 3 579 546 000 < 2³²). */
#[no_mangle]
pub extern "C" fn pit_esperar_ms(ms: u32) {
    let mut resto = ms;

    while resto > 0 {
        let pedaco = if resto > 3000 { 3000 } else { resto };
        resto -= pedaco;
        pit_esperar_ticks(pedaco * FREQUENCIA_BASE / 1000);
    }
}

/* Devolve o contador atual do canal 0 (16 bits), travado (latch) para a
 * leitura não mudar no meio. Em modo periódico ele conta de `carga` até
 * 1 e recarrega; em one-shot, para em 0. */
#[no_mangle]
pub extern "C" fn pit_ler_contador() -> u16 {
    outb(PORTA_COMANDO, CMD_TRAVAR);
    let baixo = inb(PORTA_CANAL_0) as u16;
    let alto = inb(PORTA_CANAL_0) as u16;
    (alto << 8) | baixo
}
