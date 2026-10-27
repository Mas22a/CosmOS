/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.6.2026
 *   nucleo/texto.rs — Driver de texto do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Função : escrever texto na tela usando a fonte 8x8 embutida.
 *
 *   Este arquivo guarda SOMENTE a escrita: a fonte, a busca de glifo e o
 *   desenho dos caracteres. Ele acende cada pixel chamando video_pixel()
 *   do driver de vídeo (nucleo/drivers/video/video.rs). O nucleo/nucleo.rs
 *   não desenha mais nada — só pede "escreva este texto nesta posição".
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com video.o e nucleo.o para formar o /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::ptr::read;

/* Driver de vídeo: compilado à parte e linkado junto no nucos. */
extern "C" {
    fn video_pixel(x: usize, y: usize, cor: u8);
}

/* Cada caractere ocupa 8 pixels de largura (fonte 8x8). */
pub const LARGURA_CARACTERE: usize = 8;

/* Cor padrão do texto no modo 13h (branco). */
pub const COR_TEXTO: u8 = 15;

/* Fonte 8x8 desenhada à mão, apenas os caracteres usados até agora —
 * mesmo estilo do ini/inicio.c.
 * Cada glifo ocupa 8 bytes (8 linhas); os 5 bits mais à esquerda de cada
 * byte são os 5 pixels de largura do caractere. */
struct Glifo {
    codigo: u8,
    linhas: [u8; 8],
}

const FONTE: [Glifo; 32] = [
    Glifo { codigo: b' ', linhas: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00] },
    Glifo { codigo: b'/', linhas: [0x00, 0x00, 0x08, 0x10, 0x20, 0x40, 0x80, 0x00] },
    Glifo { codigo: b'0', linhas: [0x70, 0x88, 0x98, 0xA8, 0xC8, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'1', linhas: [0x20, 0x60, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00] },
    Glifo { codigo: b'2', linhas: [0x70, 0x88, 0x08, 0x10, 0x20, 0x40, 0xF8, 0x00] },
    Glifo { codigo: b'3', linhas: [0xF8, 0x10, 0x20, 0x10, 0x08, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'4', linhas: [0x10, 0x30, 0x50, 0x90, 0xF8, 0x10, 0x10, 0x00] },
    Glifo { codigo: b'5', linhas: [0xF8, 0x80, 0xF0, 0x08, 0x08, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'6', linhas: [0x30, 0x40, 0x80, 0xF0, 0x88, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'7', linhas: [0xF8, 0x08, 0x10, 0x20, 0x40, 0x40, 0x40, 0x00] },
    Glifo { codigo: b'8', linhas: [0x70, 0x88, 0x88, 0x70, 0x88, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'9', linhas: [0x70, 0x88, 0x88, 0x78, 0x08, 0x10, 0x60, 0x00] },
    Glifo { codigo: b':', linhas: [0x00, 0x00, 0x30, 0x30, 0x00, 0x30, 0x30, 0x00] },
    Glifo { codigo: b'A', linhas: [0x70, 0x88, 0x88, 0xF8, 0x88, 0x88, 0x88, 0x00] },
    Glifo { codigo: b'B', linhas: [0xF0, 0x88, 0x88, 0xF0, 0x88, 0x88, 0xF0, 0x00] },
    Glifo { codigo: b'D', linhas: [0xF0, 0x88, 0x88, 0x88, 0x88, 0x88, 0xF0, 0x00] },
    Glifo { codigo: b'E', linhas: [0xF8, 0x80, 0x80, 0xF0, 0x80, 0x80, 0xF8, 0x00] },
    Glifo { codigo: b'K', linhas: [0x88, 0x90, 0xA0, 0xC0, 0xA0, 0x90, 0x88, 0x00] },
    Glifo { codigo: b'M', linhas: [0x88, 0xD8, 0xA8, 0xA8, 0x88, 0x88, 0x88, 0x00] },
    Glifo { codigo: b'N', linhas: [0x88, 0xC8, 0xA8, 0x98, 0x88, 0x88, 0x88, 0x00] },
    Glifo { codigo: b'O', linhas: [0x70, 0x88, 0x88, 0x88, 0x88, 0x88, 0x70, 0x00] },
    Glifo { codigo: b'R', linhas: [0xF0, 0x88, 0x88, 0xF0, 0xA0, 0x90, 0x88, 0x00] },
    Glifo { codigo: b'S', linhas: [0x78, 0x80, 0x80, 0x70, 0x08, 0x08, 0xF0, 0x00] },
    Glifo { codigo: b'T', linhas: [0xF8, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00] },
    Glifo { codigo: b'Y', linhas: [0x88, 0x88, 0x50, 0x20, 0x20, 0x20, 0x20, 0x00] },
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

/* Escreve `tamanho` bytes a partir de `texto` na posição (x, y), em branco.
 * Interface C (ponteiro + tamanho) para poder ser chamada pelo nucleo.rs,
 * que é compilado como outro crate e linkado junto.
 *
 * # Segurança
 * `texto` precisa apontar para pelo menos `tamanho` bytes válidos. */
#[no_mangle]
pub unsafe extern "C" fn texto_escrever(
    texto: *const u8,
    tamanho: usize,
    x_inicial: usize,
    y: usize,
) {
    let mut x = x_inicial;
    let mut i = 0;

    while i < tamanho {
        let codigo = unsafe { read(texto.add(i)) };

        if let Some(linhas) = achar_glifo(codigo) {
            for (linha, &bits) in linhas.iter().enumerate() {
                for coluna in 0..8 {
                    if bits & (0x80 >> coluna) != 0 {
                        unsafe { video_pixel(x + coluna, y + linha, COR_TEXTO) };
                    }
                }
            }
        }

        x += LARGURA_CARACTERE;
        i += 1;
    }
}

/* Escreve um número decimal (sem sinal) na posição (x, y) e devolve
 * quantos caracteres foram desenhados. */
#[no_mangle]
pub unsafe extern "C" fn texto_escrever_numero(
    mut valor: usize,
    x_inicial: usize,
    y: usize,
) -> usize {
    let mut digitos = [0u8; 20];
    let p = digitos.as_mut_ptr();
    let mut n = 0;

    if valor == 0 {
        unsafe { *p = b'0' };
        n = 1;
    } else {
        while valor > 0 {
            unsafe { *p.add(n) = b'0' + (valor % 10) as u8 };
            valor /= 10;
            n += 1;
        }

        /* Os dígitos saíram ao contrário; inverte no próprio buffer. */
        let mut i = 0;
        while i < n / 2 {
            unsafe {
                let temp = *p.add(i);
                *p.add(i) = *p.add(n - 1 - i);
                *p.add(n - 1 - i) = temp;
            }
            i += 1;
        }
    }

    unsafe { texto_escrever(p, n, x_inicial, y) };
    n
}
