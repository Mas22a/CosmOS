/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.4.2026
 *   nucleo/drivers/video/video.rs — Driver de vídeo do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
 *
 *   Este arquivo guarda SOMENTE as configurações e as operações de vídeo:
 *   limpar a tela e acender um pixel. O nucleo/nucleo.rs não mexe mais
 *   direto na memória de vídeo — ele só pede "limpe preto" e "escreva
 *   este caractere", usando as funções exportadas aqui.
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com o nucleo.o para formar o /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::ptr::write_volatile;

/* Modo 13h: 320 pixels de largura por 200 de altura, 1 byte por pixel. */
pub const LARGURA_TELA: usize = 320;
pub const ALTURA_TELA: usize = 200;

/* Endereço da memória de vídeo do modo 13h. */
const BASE_VIDEO: usize = 0xA0000;

fn video() -> *mut u8 {
    BASE_VIDEO as *mut u8
}

/* Pinta a tela inteira com uma cor (0 = preto). */
#[no_mangle]
pub extern "C" fn video_limpar(cor: u8) {
    let mut i = 0;
    while i < LARGURA_TELA * ALTURA_TELA {
        unsafe { write_volatile(video().add(i), cor) };
        i += 1;
    }
}

/* Acende um pixel. Ignora coordenadas fora da tela. */
#[no_mangle]
pub extern "C" fn video_pixel(x: usize, y: usize, cor: u8) {
    if x < LARGURA_TELA && y < ALTURA_TELA {
        let pos = y * LARGURA_TELA + x;
        unsafe { write_volatile(video().add(pos), cor) };
    }
}
