/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.3.2026
 *   nucleo/nucleo.rs — Núcleo do CosmOS (Nucos)
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. O inicio procura /nucleo/nucos
 *            no ext4 da imagem, carrega em 0x20000, espera 2 segundos e
 *            salta para cá.
 *   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
 *   Estado : limpa a tela e desenha, na primeira linha, à esquerda,
 *            a mensagem "Entrou no Nucos"
 *
 *   Sem biblioteca padrão (#![no_std]): compilado com
 *       rustc --target i686-unknown-none -C panic=abort -C opt-level=s
 *   e linkado como binário plano em 0x20000 (ver copilar.sh). No futuro
 *   outros arquivos .rs entrarão aqui, com um linker de verdade.
 * ========================================================================== */

#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{read, write_volatile};

const LARGURA_TELA: usize = 320;
const ALTURA_TELA: usize = 200;
const COR_TEXTO: u8 = 15; /* branco no modo 13h */

/* Memória de vídeo do modo 13h */
fn video() -> *mut u8 {
    0xA0000 as *mut u8
}

/* Ponto de entrada: o inicio salta para 0x20000, onde o copilar.sh linkou
 * este binário. O #[link_section] joga o _start para .text.startup, que o
 * nucleo/nucleo.ld coloca em PRIMEIRO LUGAR — sem isto o rust_begin_unwind
 * (código frio, .text.unlikely) ocuparia 0x20000 e o sistema cairia no
 * pânico logo no salto. */
#[no_mangle]
#[link_section = ".text.startup"]
pub extern "C" fn _start() -> ! {
    limpar_tela();
    desenhar_texto("Entrou no Nucos", 0, 2);
    parar();
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

fn limpar_tela() {
    let mut i = 0;
    while i < LARGURA_TELA * ALTURA_TELA {
        unsafe { write_volatile(video().add(i), 0) };
        i += 1;
    }
}

/* Fonte 8x8 desenhada à mão, apenas os caracteres da mensagem
 * "Entrou no Nucos" — mesmo estilo do ini/inicio.c.
 * Cada glifo ocupa 8 bytes (8 linhas); os 5 bits mais à esquerda de cada
 * byte são os 5 pixels de largura do caractere. */
struct Glifo {
    codigo: u8,
    linhas: [u8; 8],
}

const FONTE: [Glifo; 10] = [
    Glifo { codigo: b' ', linhas: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00] },
    Glifo { codigo: b'E', linhas: [0xF8, 0x80, 0x80, 0xF0, 0x80, 0x80, 0xF8, 0x00] },
    Glifo { codigo: b'N', linhas: [0x88, 0xC8, 0xA8, 0x98, 0x88, 0x88, 0x88, 0x00] },
    Glifo { codigo: b'c', linhas: [0x00, 0x00, 0x70, 0x88, 0x80, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'n', linhas: [0x00, 0x00, 0xD8, 0xC8, 0x88, 0x88, 0x88, 0x00] },
    Glifo { codigo: b'o', linhas: [0x00, 0x00, 0x70, 0x88, 0x88, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'r', linhas: [0x00, 0x00, 0xD8, 0xC8, 0x80, 0x80, 0x80, 0x00] },
    Glifo { codigo: b's', linhas: [0x00, 0x00, 0x78, 0x80, 0x70, 0x08, 0xF0, 0x00] },
    Glifo { codigo: b't', linhas: [0x20, 0x20, 0xF8, 0x20, 0x20, 0x28, 0x18, 0x00] },
    Glifo { codigo: b'u', linhas: [0x00, 0x00, 0x88, 0x88, 0x88, 0x98, 0x68, 0x00] },
];

fn achar_glifo(codigo: u8) -> Option<&'static [u8; 8]> {
    for glifo in FONTE.iter() {
        if glifo.codigo == codigo {
            return Some(&glifo.linhas);
        }
    }
    None
}

fn desenhar_texto(texto: &str, x_inicial: usize, y: usize) {
    let mut x = x_inicial;

    for &codigo in texto.as_bytes() {
        if let Some(linhas) = achar_glifo(codigo) {
            for (linha, &bits) in linhas.iter().enumerate() {
                for coluna in 0..8 {
                    if bits & (0x80 >> coluna) != 0 {
                        let pos = (y + linha) * LARGURA_TELA + x + coluna;
                        unsafe { write_volatile(video().add(pos), COR_TEXTO) };
                    }
                }
            }
        }
        x += 8;
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
