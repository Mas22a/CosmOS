/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.7.2026
 *   servicos/servicos.cpp — Serviço "usr" (o programa do usuário)
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. Binário plano em 0x60000.
 *   Origem : o núcleo (nucleo/nucleo.rs) espera 5 segundos depois de
 *            entrar, procura /servicos/usr no ext4 da imagem (via
 *            nucleo/arquivos/ext4.rs + nucleo/drivers/disco/ata.rs),
 *            carrega em 0x60000, desliga as interrupções (CLI) e salta
 *            para cá — É A SAÍDA DO KERNEL.
 *
 *   Papel atual : este arquivo é o PRIMEIRO serviço do sistema. Por
 *                 enquanto ele só limpa a tela (modo 13h continua
 *                 ativo) e dorme em HLT — o usuário vê uma tela
 *                 preta, sinal de que o kernel saiu e o usr assumiu.
 *
 *   Papel futuro : o núcleo será a PONTE permanente entre este serviço
 *                  e a interface gráfica: o usr voltará a chamar o
 *                  kernel (por exemplo, pedindo desenho de janelas) e
 *                  o kernel repassará para a GUI.
 *
 *   Sem biblioteca padrão: compilado freestanding com g++ -m32 e
 *   linkado como binário plano em 0x60000 (ver copilar.sh).
 * ========================================================================== */

typedef unsigned char u8;

#define LARGURA_TELA 320
#define ALTURA_TELA  200

/* Memória de vídeo do modo 13h (320x200x8 bits). O modo continua
 * ligado: o kernel deixou a CPU em modo protegido com o modo 13h. */
static volatile u8 *const video = (volatile u8 *)0xA0000;

/* Ponto de entrada: o nucleo.rs salta para 0x60000, onde o copilar.sh
 * linkou este binário. */
extern "C" void _start(void)
{
    /* Limpa a tela inteira: fundo totalmente preto. */
    for (int i = 0; i < LARGURA_TELA * ALTURA_TELA; i++)
        video[i] = 0;

    /* Tela preta e nada mais: dorme para sempre com HLT. As
     * interrupções estão desligadas (o kernel deu CLI antes do salto),
     * então a CPU fica em repouso até o próximo evento do QEMU. */
    for (;;)
        __asm__ volatile("hlt");
}
