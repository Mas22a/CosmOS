/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   nucleo/nucleo.ld — Linker script do núcleo (Nucos)
 * --------------------------------------------------------------------------
 *   O ini/inicio carrega /nucleo/nucos em 0x20000 como binário plano e salta
 *   para o _start, então O PRIMEIRO BYTE DO ARQUIVO precisa ser o _start.
 *
 *   A ordem das seções aqui é o coração do script:
 *     - .text.startup primeiro → é onde o nucleo.rs coloca o _start
 *       (via #[link_section]); sem isto, o rust_begin_unwind — que o rustc
 *       marca como código frio (.text.unlikely) — ocuparia 0x20000 e o
 *       sistema cairia no pânico logo no salto;
 *     - depois o resto do código, a rodata, os dados e o .bss por último
 *       (o .bss não vai para o arquivo, então ele tem de ser a última
 *       seção para não furar o binário plano).
 *
 *   Linkado com: ld -m elf_i386 -T nucleo/nucleo.ld --nmagic (ver copilar.sh)
 * ========================================================================== */

ENTRY(_start)

SECTIONS
{
    . = 0x20000;

    .text : {
        *(.text.startup)              /* _start — precisa ser o primeiro byte */
        *(.text .text.*)              /* resto do código (inclusive o pânico) */
    }

    .rodata : { *(.rodata .rodata.*) }
    .data   : { *(.data .data.*) *(.got .got.*) }
    .bss    : { *(.bss .bss.*) *(COMMON) }

    /DISCARD/ : { *(.eh_frame*) *(.rel*) *(.comment) *(.note*) }
}
