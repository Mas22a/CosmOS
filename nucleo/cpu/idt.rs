/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   nucleo/cpu/idt.rs — Interrupt Descriptor Table (IDT) do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Função : monta a IDT (256 entradas) e a carrega via LIDTR.
 *
 *   O PIC continua com o mapeamento padrão (não remapeado), então os
 *   vetores de hardware são:
 *     IRQ0..IRQ7  → vetores 8..15    (8 = timer, 9 = teclado, ...)
 *     IRQ8..IRQ15 → vetores 0x70..0x77 (0x74 = mouse)
 *   Todos os vetores de hardware recebem um stub (senão qualquer IRQ
 *   (ex.: o teclado que o BIOS deixa liberado) cairia num vetor vazio
 *   e causaria triple fault).
 *
 *   Compilado como lib (--crate-type=lib --emit=obj) e linkado no nucos.
 * ========================================================================== */

#![no_std]

use core::arch::asm;
use core::arch::global_asm;
use core::sync::atomic::{AtomicU32, Ordering};

/* ===== Estruturas da IDT (32 bits = 8 bytes por entrada) ===== */

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    zero: u8,
    type_attr: u8,
    offset_high: u16,
}

impl IdtEntry {
    const fn new(offset: u32, selector: u16, type_attr: u8) -> Self {
        Self {
            offset_low: (offset & 0xFFFF) as u16,
            selector,
            zero: 0,
            type_attr,
            offset_high: ((offset >> 16) & 0xFFFF) as u16,
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtPtr {
    limit: u16,
    base: u32,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::new(0, 0, 0); 256];
static mut IDT_PTR: IdtPtr = IdtPtr { limit: 0, base: 0 };

const KERNEL_CS: u16 = 0x08;
const INT_GATE_32: u8 = 0x8E;

/* PIC sem remapeamento: IRQ0 = vetor 8, IRQ12 = vetor 0x74. */
const VETOR_TIMER: u8 = 8;
const VETOR_MOUSE: u8 = 0x74;

static TICKS: AtomicU32 = AtomicU32::new(0);

/* Stubs de ISR em assembly (definidos no fim do arquivo). */
extern "C" {
    fn isr_stub_timer();
    fn isr_stub_mouse();
    fn isr_stub_mestre();
    fn isr_stub_escravo();
}

#[no_mangle]
pub extern "C" fn isr_timer_handler() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn idt_iniciar() {
    unsafe {
        for entrada in IDT.iter_mut() {
            *entrada = IdtEntry::new(0, 0, 0);
        }

        /* Vetores de hardware do PIC não remapeado (8..15 e 0x70..0x77):
         * todos precisam de um presente=1, senão o IRQ derruba a CPU. */
        let mut v: u16 = 8;
        while v <= 15 {
            let stub = if v as u8 == VETOR_TIMER {
                isr_stub_timer as *const () as usize as u32
            } else {
                isr_stub_mestre as *const () as usize as u32
            };
            idt_set_gate(v as u8, stub, KERNEL_CS, INT_GATE_32);
            v += 1;
        }

        let mut v: u16 = 0x70;
        while v <= 0x77 {
            let stub = if v as u8 == VETOR_MOUSE {
                isr_stub_mouse as *const () as usize as u32
            } else {
                isr_stub_escravo as *const () as usize as u32
            };
            idt_set_gate(v as u8, stub, KERNEL_CS, INT_GATE_32);
            v += 1;
        }

        IDT_PTR.limit = 2047;
        IDT_PTR.base = IDT.as_ptr() as u32;
        let idtr: u64 = ((IDT_PTR.base as u64) << 16) | (IDT_PTR.limit as u64);
        let idtr_ptr: *const u64 = &idtr;
        /* sem "nomem": o LIDT lê 6 bytes desta memória; se mentirmos, o
         * compilador otimiza todo o preenchimento da IDT como morto. */
        asm!("lidt [{}]", in(reg) idtr_ptr);

        asm!("sti", options(nomem, nostack));
    }
}

#[no_mangle]
pub extern "C" fn idt_ticks() -> u32 {
    TICKS.load(Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn idt_esperar_ms(ms: u32) {
    let inicio = idt_ticks();
    let alvo = ms / 10;
    while idt_ticks().wrapping_sub(inicio) < alvo {
        unsafe { asm!("hlt", options(nomem, nostack)); }
    }
}

unsafe fn idt_set_gate(vetor: u8, handler: u32, seletor: u16, flags: u8) {
    let entrada = &mut IDT[vetor as usize];
    entrada.offset_low = (handler & 0xFFFF) as u16;
    entrada.selector = seletor;
    entrada.zero = 0;
    entrada.type_attr = flags;
    entrada.offset_high = ((handler >> 16) & 0xFFFF) as u16;
}

/* ===== Stubs de ISR em Assembly (global_asm!) =====
 * Todos salvam/restauram os registradores e enviam EOI antes do IRETD
 * (32 bits — "iret" aqui montaria um IRET de 16 bits e corromperia a
 * pilha). */

global_asm!(
    r#"
/* Timer (IRQ0 = vetor 8): só conta o tick, EOI no PIC mestre. */
.globl isr_stub_timer
.type isr_stub_timer, @function
isr_stub_timer:
    cli
    pushad
    push ds
    push es
    push fs
    push gs
    mov eax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    call isr_timer_handler
    pop gs
    pop fs
    pop es
    pop ds
    popad
    mov al, 0x20
    out 0x20, al
    iretd

/* Mouse (IRQ12 = vetor 0x74): EOI no PIC escravo e depois no mestre. */
.globl isr_stub_mouse
.type isr_stub_mouse, @function
isr_stub_mouse:
    cli
    pushad
    push ds
    push es
    push fs
    push gs
    mov eax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    pop gs
    pop fs
    pop es
    pop ds
    popad
    mov al, 0x20
    out 0xA0, al
    out 0x20, al
    iretd

/* Qualquer outro IRQ do PIC mestre (ex.: teclado = vetor 9). */
.globl isr_stub_mestre
.type isr_stub_mestre, @function
isr_stub_mestre:
    cli
    pushad
    push ds
    push es
    push fs
    push gs
    mov eax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    pop gs
    pop fs
    pop es
    pop ds
    popad
    mov al, 0x20
    out 0x20, al
    iretd

/* Qualquer outro IRQ do PIC escravo (0x70..0x77). */
.globl isr_stub_escravo
.type isr_stub_escravo, @function
isr_stub_escravo:
    cli
    pushad
    push ds
    push es
    push fs
    push gs
    mov eax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    pop gs
    pop fs
    pop es
    pop ds
    popad
    mov al, 0x20
    out 0xA0, al
    out 0x20, al
    iretd
"#
);
