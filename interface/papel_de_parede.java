/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   interface/papel_de_parede.java — Interface gráfica "grain"
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. Binário plano em 0x80000.
 *   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
 *   Memória: vídeo em 0xA0000 (64 KiB: 320 * 200 = 64000 bytes)
 *   Saída  : binário plano chamado "grain", instalado em /interface/grain
 *            na imagem ext4. O serviço usr (/servicos/usr) carrega e salta.
 *
 *   Linguagem: Java "bare-metal" (dialeto CosmOS) — subconjunto compilado
 *              por interface/compilador.py DIRETAMENTE para código de
 *              máquina x86-32. Não passa por C, não usa gcc e NÃO usa
 *              máquina virtual.
 *
 *   Intrínsecos do dialeto:
 *     memoria_preencher(destino, tamanho, valor) → preenche memória
 *     cpu_halt()                                 → instrução HLT
 *
 *   Efeito: pinta a tela inteira de azul claro (cor 9 da paleta VGA) e
 *           dorme com HLT — a interface assume o controle após o usr.
 * ========================================================================== */

public final class papel_de_parede {

    /* Constantes do modo 13h (320x200, 256 cores) */
    static final int LARGURA_TELA = 320;
    static final int ALTURA_TELA = 200;
    static final int TAMANHO_TELA = 64000;   /* 320 * 200 */
    static final int VIDEO_BASE = 0xA0000;   /* memória de vídeo do modo 13h */

    /* Paleta padrão do modo 13h — cor 78 = azul-céu claro (127,191,255).
     * (A cor 9 vira um azul arroxeado; a 78 é um "azul clarinho" de verdade.) */
    static final int COR_AZUL_CLARO = 78;

    /* Ponto de entrada: o usr carrega este binário em 0x80000 e salta. */
    public static void main() {
        /* 1) Pinta os 64000 bytes da memória de vídeo com azul claro. */
        memoria_preencher(VIDEO_BASE, TAMANHO_TELA, COR_AZUL_CLARO);

        /* 2) Dorme para sempre: a tela fica azul claro. */
        while (true) {
            cpu_halt();
        }
    }
}
