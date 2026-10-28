/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   nucleo/drivers/disco/ata.rs — Driver de disco ATA PIO do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Função : ler setores do disco primário mestre (0x1F0) por PIO.
 *
 *   Em modo protegido não existe mais a int 0x13 do BIOS — este módulo
 *   fala direto com o controlador IDE, com o mesmo protocolo usado pelo
 *   ini/inicio.c (espera BSY=0, envia o endereço LBA, READ SECTORS e
 *   polimenta DRQ). O QEMU monta a CosmOS.img como primário mestre
 *   (-drive ... if=ide).
 *
 *   Exporta ata_ler_setores(lba, n, destino) — o nucleo/arquivos/ext4.rs
 *   é quem o chama para montar os blocos de 1 KiB do sistema de arquivos.
 *
 *   Compilado separadamente (rustc --crate-type=lib --emit=obj) e linkado
 *   junto com os demais módulos para formar o /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::arch::asm;

/* --- I/O de porta -------------------------------------------------------- */

fn outb(porta: u16, valor: u8) {
    unsafe {
        asm!("out dx, al", in("dx") porta, in("al") valor, options(nomem, nostack));
    }
}

fn inb(porta: u16) -> u8 {
    unsafe {
        let valor: u8;
        asm!("in al, dx", out("al") valor, in("dx") porta, options(nomem, nostack));
        valor
    }
}

fn inw(porta: u16) -> u16 {
    unsafe {
        let valor: u16;
        asm!("in ax, dx", out("ax") valor, in("dx") porta, options(nomem, nostack));
        valor
    }
}

/* Espera o controlador ficar pronto (BSY = 0). */
fn espera_ata() {
    while inb(0x1F7) & 0x80 != 0 {}
}

/* Lê `n` setores (1..255) começando no LBA para `destino`.
 * Devolve 1 = ok, 0 = erro (ERR ou DF no status). */
#[no_mangle]
pub extern "C" fn ata_ler_setores(lba: u32, n: u16, mut destino: *mut u8) -> u8 {
    if n == 0 || n > 255 {
        return 0;
    }

    espera_ata();
    outb(0x3F6, 0x02); /* nIEN: só polimento, sem IRQ */
    outb(0x1F6, (0xE0 | ((lba >> 24) & 0x0F)) as u8);
    outb(0x1F1, 0x00);
    outb(0x1F2, n as u8); /* contador de setores */
    outb(0x1F3, (lba & 0xFF) as u8);
    outb(0x1F4, ((lba >> 8) & 0xFF) as u8);
    outb(0x1F5, ((lba >> 16) & 0xFF) as u8);
    outb(0x1F7, 0x20); /* READ SECTORS */

    let mut i: u16 = 0;
    while i < n {
        /* Espera o DRQ (pronto para transferir). */
        loop {
            let st = inb(0x1F7);
            if st & 0x01 != 0 {
                return 0; /* ERR */
            }
            if st & 0x20 != 0 {
                return 0; /* DF  */
            }
            if st & 0x08 != 0 {
                break; /* DRQ */
            }
        }

        /* 256 palavras de 16 bits = 1 setor de 512 bytes. */
        let mut j: u16 = 0;
        while j < 256 {
            unsafe {
                core::ptr::write_volatile(destino as *mut u16, inw(0x1F0));
                destino = destino.add(2);
            }
            j += 1;
        }
        i += 1;
    }
    1
}
