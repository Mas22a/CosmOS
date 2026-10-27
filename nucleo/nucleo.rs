/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.6.2026
 *   nucleo/nucleo.rs — Núcleo do CosmOS (Nucos) — o maestro
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. O inicio procura /nucleo/nucos
 *            no ext4 da imagem, carrega em 0x20000, espera 2 segundos e
 *            salta para cá.
 *
 *   O núcleo é o MAESTRO do sistema: ele não desenha nem administra a
 *   memória diretamente. Apenas chama, na ordem certa, os módulos:
 *     - nucleo/drivers/video/video.rs  → tela (modo 13h)
 *     - nucleo/drivers/tempo/pit.rs    → tempo (PIT canal 0)
 *     - nucleo/drivers/tempo/rtc.rs    → relógio (RTC, data e hora na tela)
 *     - nucleo/texto.rs                → escrita (fonte 8x8)
 *     - nucleo/memoria.rs              → heap da RAM
 *
 *   Layout das linhas (fonte 8x8, folga de 12 px):
 *     y =  2 → "Entrou no Nucos"          (1ª linha)
 *     y = 14 → data à esquerda | hora      (2ª linha, rtc.rs)
 *              à direita, atualizando sozinha
 *     y = 26 → "RAM DE <bytes> BYTES"      (3ª linha)
 *
 *   Sem biblioteca padrão (#![no_std]): compilado com
 *       rustc --target i686-unknown-linux-gnu -C panic=abort -C opt-level=s
 *   e linkado junto com video.o, texto.o, memoria.o, pit.o e rtc.o como
 *   binário plano em 0x20000 (ver copilar.sh).
 * ========================================================================== */

#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{read, write_volatile};

/* Módulos: compilados à parte e linkados junto para formar o nucos. */
extern "C" {
    fn video_limpar(cor: u8);
    fn texto_escrever(texto: *const u8, tamanho: usize, x_inicial: usize, y: usize);
    fn texto_escrever_numero(valor: usize, x_inicial: usize, y: usize) -> usize;
    fn memoria_iniciar() -> usize;
    fn pit_iniciar() -> u32;
    fn pit_esperar_ms(ms: u32);
    fn rtc_iniciar();
    fn rtc_atualizar();
    /* Mouse PS/2 (nucleo/drivers/mouse/ps2.rs) — MUDO, não desenha. */
    fn mouse_ps2_iniciar() -> u8;
    fn mouse_ps2_processar_tudo() -> u32;
    fn mouse_ps2_ler_movimento(dx: *mut i16, dy: *mut i16) -> u8;
    /* IDT (nucleo/cpu/idt.rs) */
    fn idt_iniciar();
    fn idt_esperar_ms(ms: u32);
}

const COR_PRETO: u8 = 0;

const MENSAGEM_BOAS_VINDAS: &str = "Entrou no Nucos";
const MENSAGEM_RAM: &str = "RAM DE ";
const MENSAGEM_BYTES: &str = " BYTES";

/* Linhas da tela (12 px de folga entre elas). */
const LINHA_BOAS_VINDAS: usize = 2;
const LINHA_RAM: usize = 26; /* a 2ª (y = 14) é da data/hora (rtc.rs) */

/* Quantas vezes por segundo a hora é relida no RTC. 250 ms detecta a
 * virada do segundo rápido e deixa a contagem estável. */
const INTERVALO_RELOGIO_MS: u32 = 250;

/* Ponto de entrada: o inicio salta para 0x20000, onde o copilar.sh linkou
 * este binário. O #[link_section] joga o _start para .text.startup, que o
 * nucleo/nucleo.ld coloca em PRIMEIRO LUGAR — sem isto o rust_begin_unwind
 * (código frio, .text.unlikely) ocuparia 0x20000 e o sistema cairia no
 * pânico logo no salto. */
#[no_mangle]
#[link_section = ".text.startup"]
pub extern "C" fn _start() -> ! {
    iniciar_video();

    let ram = iniciar_memoria();
    iniciar_tempo();
    escrever_boas_vindas(ram);
    iniciar_relogio();
    iniciar_mouse();
    iniciar_idt();

    laço_do_relogio();
}

/* --- Coordenação dos módulos -------------------------------------------- */

fn iniciar_video() {
    unsafe { video_limpar(COR_PRETO) };
}

fn iniciar_memoria() -> usize {
    unsafe { memoria_iniciar() }
}

/* Liga o PIT no modo periódico (100 Hz). É mudo de propósito: o retorno
 * (a frequência real) fica descartado — nada é escrito na tela aqui. */
fn iniciar_tempo() {
    unsafe {
        let _hz = pit_iniciar();
    }
}

fn escrever_boas_vindas(ram: usize) {
    unsafe {
        texto_escrever(
            MENSAGEM_BOAS_VINDAS.as_ptr(),
            MENSAGEM_BOAS_VINDAS.len(),
            0,
            LINHA_BOAS_VINDAS,
        );

        if ram > 0 {
            texto_escrever(MENSAGEM_RAM.as_ptr(), MENSAGEM_RAM.len(), 0, LINHA_RAM);

            let x_numero = MENSAGEM_RAM.len() * 8;
            let digitos = texto_escrever_numero(ram, x_numero, LINHA_RAM);

            texto_escrever(
                MENSAGEM_BYTES.as_ptr(),
                MENSAGEM_BYTES.len(),
                x_numero + digitos * 8,
                LINHA_RAM,
            );
        }
    }
}

/* A primeira leitura do RTC desenha a data (esquerda) e a hora (direita)
 * na segunda linha — aí a tela já tem as três linhas prontas. */
fn iniciar_relogio() {
    unsafe { rtc_iniciar() };
}

/* Inicializa o mouse PS/2 (sem desenhar nada). */
fn iniciar_mouse() {
    unsafe { mouse_ps2_iniciar() };
}

/* Inicializa IDT + habilita interrupções (STI). */
fn iniciar_idt() {
    unsafe { idt_iniciar() };
}

/* Laço do relógio: espera pelo timer (via IDT/interrupção) e relê o RTC.
 * O rtc_atualizar() só redesenha o que mudou, então a
 * hora vira sozinha sem piscar a tela e sem nunca mais sair daqui.
 * O mouse agora é processado por IRQ12 (isr_mouse_handler), não mais aqui. */
fn laço_do_relogio() -> ! {
    loop {
        unsafe {
            idt_esperar_ms(INTERVALO_RELOGIO_MS);
            rtc_atualizar();
        }
    }
}

#[panic_handler]
fn panico(_: &PanicInfo) -> ! {
    parar()
}

/* Espera infinita: a tela fica como está. */
fn parar() -> ! {
    loop {
        unsafe { asm!("hlt", options(nomem, nostack)) }
    }
}

/* O copilar.sh linka com o ld do sistema (sem as rlibs do Rust), então os
 * símbolos de memória que o código gerado pode precisar estão aqui. */
use core::ffi::c_void;

#[no_mangle]
pub unsafe extern "C" fn memset(destino: *mut c_void, valor: i32, tamanho: usize) -> *mut c_void {
    let mut i = 0;
    while i < tamanho {
        write_volatile((destino as *mut u8).add(i), valor as u8);
        i += 1;
    }
    destino
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(
    destino: *mut c_void,
    origem: *const c_void,
    tamanho: usize,
) -> *mut c_void {
    let mut i = 0;
    while i < tamanho {
        write_volatile(
            (destino as *mut u8).add(i),
            read((origem as *const u8).add(i)),
        );
        i += 1;
    }
    destino
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, tamanho: usize) -> i32 {
    let mut i = 0;
    while i < tamanho {
        let x = read((a as *const u8).add(i)) as i32;
        let y = read((b as *const u8).add(i)) as i32;
        if x != y {
            return x - y;
        }
        i += 1;
    }
    0
}
