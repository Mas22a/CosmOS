/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.4.2026
 *   nucleo/memoria/memoria.rs — Gerenciador de memória RAM do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Função : descobrir o tamanho REAL da RAM e administrar um heap linear
 *            a partir de 0x100000.
 *
 *   Como a RAM é descoberta:
 *     O primeiro estágio (ini/inimin.asm) chama a BIOS (int 0x15, E820) e
 *     grava o mapa de memória em 0x5000, com a contagem em 0x4FF0 e a
 *     assinatura 0xE820 em 0x4FF2. Este módulo lê esse mapa e calcula o
 *     tamanho real da RAM (o topo da memória utilizável, arredondado para
 *     MiB). Se o mapa não existir, cai para a CMOS (0x30/0x31) como reserva.
 *
 *   Como o heap funciona:
 *     Alocador "bump": cada alocação avança um ponteiro. Não há liberação
 *     individual ainda — `memoria_resetar()` devolve o heap inteiro. É o
 *     suficiente para o boot e deixa a API pronta para um alocador com
 *     lista de blocos livres depois.
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com video.o, texto.o e nucleo.o para formar o /nucleo/nucos
 *   (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::arch::asm;
use core::ptr::{null_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

/* O heap começa em 1 MiB, logo depois da área baixa reservada ao BIOS,
 * à memória de vídeo e ao próprio núcleo (carregado em 0x20000). */
const BASE_HEAP: usize = 0x100000; /* 1 MiB */

/* Mapa E820 deixado pelo inimin: 0x5000 (24 bytes por entrada),
 * contagem em 0x4FF0 e assinatura em 0x4FF2. */
const MAPA_E820: usize = 0x5000;
const CONT_E820: usize = 0x4FF0;
const MAGIA_E820: usize = 0x4FF2;
const TAMANHO_ENTRADA_E820: usize = 24;

/* Se nem o E820 nem a CMOS informarem nada, assumimos 4 MiB estendidos. */
const ESTENDIDA_MINIMA_KB: usize = 4 * 1024;

/* Reserva no fim da RAM para tabelas do BIOS/ACPI e áreas usadas pelo
 * firmware. Não entregamos essa faixa ao heap. */
const RESERVA_FIRMWARE: usize = 2 * 1024 * 1024; /* 2 MiB */

/* Heap mínimo, mesmo que a detecção informe um valor absurdo. */
const HEAP_MINIMO: usize = 1 * 1024 * 1024; /* 1 MiB */

/* Limites do heap. 0 em FIM significa "ainda não inicializado". */
static INICIO: AtomicUsize = AtomicUsize::new(0);
static ATUAL: AtomicUsize = AtomicUsize::new(0);
static FIM: AtomicUsize = AtomicUsize::new(0);

/* Lê um registrador da CMOS (porta de índice 0x70, porta de dados 0x71). */
fn ler_cmos(indice: u8) -> u8 {
    unsafe {
        asm!("out dx, al", in("dx") 0x70u16, in("al") indice, options(nomem, nostack));
        let valor: u8;
        asm!("in al, dx", out("al") valor, in("dx") 0x71u16, options(nomem, nostack));
        valor
    }
}

/* Memória estendida em KiB, segundo o BIOS (registradores 0x30/0x31).
 * É só a reserva caso o mapa E820 não esteja disponível. */
fn ram_estendida_kb() -> usize {
    let baixo = ler_cmos(0x30) as usize;
    let alto = ler_cmos(0x31) as usize;
    (alto << 8) | baixo
}

/* Lê o mapa E820 e devolve (RAM real em bytes, fim da região utilizável
 * que contém o heap). None se o mapa não estiver válido. */
fn detectar_e820() -> Option<(usize, usize)> {
    unsafe {
        if read_volatile(MAGIA_E820 as *const u16) != 0xE820 {
            return None;
        }

        let entradas = read_volatile(CONT_E820 as *const u16) as usize;
        if entradas == 0 {
            return None;
        }

        let mut maior_fim_util: u64 = 0;
        let mut fim_regiao_heap: u64 = 0;

        let mut i = 0;
        while i < entradas {
            let p = MAPA_E820 + i * TAMANHO_ENTRADA_E820;
            let base = read_volatile(p as *const u64);
            let tamanho = read_volatile((p + 8) as *const u64);
            let tipo = read_volatile((p + 16) as *const u32);
            let fim = base.saturating_add(tamanho);

            /* Tipo 1 = memória utilizável. Ignoramos regiões reservadas
             * (ROM do BIOS, ACPI etc.) para não superestimar a RAM. */
            if tipo == 1 {
                if fim > maior_fim_util {
                    maior_fim_util = fim;
                }
                if base <= BASE_HEAP as u64
                    && fim > BASE_HEAP as u64
                    && fim > fim_regiao_heap
                {
                    fim_regiao_heap = fim;
                }
            }

            i += 1;
        }

        if maior_fim_util == 0 {
            return None;
        }

        /* O topo utilizável costuma parar alguns KiB antes do tamanho
         * redondo (ACPI no fim). Arredondamos para o próximo MiB para
         * mostrar o tamanho real instalado (ex.: 128 MiB). */
        let limite = u32::MAX as u64;
        let mut total = (maior_fim_util + 0xFFFFF) & !0xFFFFFu64;
        if total > limite {
            total = limite;
        }

        let mut fim_heap = fim_regiao_heap;
        if fim_heap > limite {
            fim_heap = limite;
        }

        Some((total as usize, fim_heap as usize))
    }
}

/* Inicializa o heap e devolve o tamanho REAL da RAM em bytes.
 * Pode ser chamada de novo: redefine os limites e começa do zero. */
#[no_mangle]
pub extern "C" fn memoria_iniciar() -> usize {
    let (ram_total, mut fim) = match detectar_e820() {
        Some((total, fim_heap)) => (total, fim_heap),
        None => {
            let mut estendida_kb = ram_estendida_kb();
            if estendida_kb == 0 {
                estendida_kb = ESTENDIDA_MINIMA_KB;
            }
            let total = BASE_HEAP + estendida_kb * 1024;
            (total, total)
        }
    };

    /* Se o mapa não indicar a região do heap, usa o total como limite. */
    if fim <= BASE_HEAP {
        fim = ram_total;
    }

    /* Não entrega ao heap a faixa reservada do firmware. */
    fim = fim.saturating_sub(RESERVA_FIRMWARE);

    /* Nunca menos que BASE_HEAP + HEAP_MINIMO. */
    if fim < BASE_HEAP + HEAP_MINIMO {
        fim = BASE_HEAP + HEAP_MINIMO;
    }

    INICIO.store(BASE_HEAP, Ordering::Relaxed);
    ATUAL.store(BASE_HEAP, Ordering::Relaxed);
    FIM.store(fim, Ordering::Relaxed);

    ram_total
}

/* Aloca `tamanho` bytes com o alinhamento pedido (potência de 2).
 * A memória volta zerada. Retorna nulo se não couber no heap. */
#[no_mangle]
pub extern "C" fn memoria_alocar(tamanho: usize, alinhamento: usize) -> *mut u8 {
    if tamanho == 0 || alinhamento == 0 || !alinhamento.is_power_of_two() {
        return null_mut();
    }

    let fim = FIM.load(Ordering::Relaxed);
    if fim == 0 {
        return null_mut(); /* memoria_iniciar() ainda não foi chamada */
    }

    let atual = ATUAL.load(Ordering::Relaxed);
    let inicio = (atual + alinhamento - 1) & !(alinhamento - 1);

    let novo_fim = match inicio.checked_add(tamanho) {
        Some(v) => v,
        None => return null_mut(),
    };

    if novo_fim > fim {
        return null_mut();
    }

    /* Zera o bloco antes de entregar. */
    let mut i = 0;
    while i < tamanho {
        unsafe { write_volatile((inicio as *mut u8).add(i), 0) };
        i += 1;
    }

    ATUAL.store(novo_fim, Ordering::Relaxed);
    inicio as *mut u8
}

/* Devolve todo o heap ao estado inicial (descarta todas as alocações). */
#[no_mangle]
pub extern "C" fn memoria_resetar() {
    if FIM.load(Ordering::Relaxed) != 0 {
        let inicio = INICIO.load(Ordering::Relaxed);
        ATUAL.store(inicio, Ordering::Relaxed);
    }
}

/* Tamanho total do heap, em bytes. */
#[no_mangle]
pub extern "C" fn memoria_total() -> usize {
    let fim = FIM.load(Ordering::Relaxed);
    if fim == 0 {
        return 0;
    }
    fim - INICIO.load(Ordering::Relaxed)
}

/* Quantos bytes já foram alocados. */
#[no_mangle]
pub extern "C" fn memoria_usada() -> usize {
    let fim = FIM.load(Ordering::Relaxed);
    if fim == 0 {
        return 0;
    }
    ATUAL.load(Ordering::Relaxed) - INICIO.load(Ordering::Relaxed)
}

/* Quantos bytes ainda estão livres. */
#[no_mangle]
pub extern "C" fn memoria_livre() -> usize {
    let fim = FIM.load(Ordering::Relaxed);
    if fim == 0 {
        return 0;
    }
    fim - ATUAL.load(Ordering::Relaxed)
}
