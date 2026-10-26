/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.4.2026
 *   nucleo/nucleo.rs — Núcleo do CosmOS (Nucos) — o maestro
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. O inicio procura /nucleo/nucos
 *            no ext4 da imagem, carrega em 0x20000, espera 2 segundos e
 *            salta para cá.
 *
 *   O núcleo é o MAESTRO do sistema: ele não desenha nem administra a
 *   memória diretamente. Apenas chama, na ordem certa, os módulos:
 *     - nucleo/drivers/video/video.rs  → tela (modo 13h)
 *     - nucleo/texto.rs                → escrita (fonte 8x8)
 *     - nucleo/memoria.rs              → heap da RAM
 *
 *   Sem biblioteca padrão (#![no_std]): compilado com
 *       rustc --target i686-unknown-linux-gnu -C panic=abort -C opt-level=s
 *   e linkado junto com video.o, texto.o e memoria.o como binário plano em
 *   0x20000 (ver copilar.sh).
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
}

const COR_PRETO: u8 = 0;

const MENSAGEM_BOAS_VINDAS: &str = "Entrou no Nucos";
const MENSAGEM_RAM: &str = "RAM DE ";
const MENSAGEM_BYTES: &str = " BYTES";

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
    escrever_boas_vindas(ram);

    parar();
}

/* --- Coordenação dos módulos -------------------------------------------- */

fn iniciar_video() {
    unsafe { video_limpar(COR_PRETO) };
}

fn iniciar_memoria() -> usize {
    unsafe { memoria_iniciar() }
}

fn escrever_boas_vindas(ram: usize) {
    unsafe {
        texto_escrever(
            MENSAGEM_BOAS_VINDAS.as_ptr(),
            MENSAGEM_BOAS_VINDAS.len(),
            0,
            2,
        );

        if ram > 0 {
            texto_escrever(MENSAGEM_RAM.as_ptr(), MENSAGEM_RAM.len(), 0, 14);

            let x_numero = MENSAGEM_RAM.len() * 8;
            let digitos = texto_escrever_numero(ram, x_numero, 14);

            texto_escrever(
                MENSAGEM_BYTES.as_ptr(),
                MENSAGEM_BYTES.len(),
                x_numero + digitos * 8,
                14,
            );
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
