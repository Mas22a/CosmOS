/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   servicos/servicos.cpp — Serviço "usr" (o programa do usuário)
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido. Binário plano em 0x60000.
 *   Origem : o núcleo (nucleo/nucleo.rs) espera 5 segundos depois de
 *            entrar, procura /servicos/usr no ext4 da imagem (via
 *            nucleo/arquivos/ext4.rs + nucleo/drivers/disco/ata.rs),
 *            carrega em 0x60000, desliga as interrupções (CLI) e salta
 *            para cá — É A SAÍDA DO KERNEL.
 *
 *   Papel  : este arquivo é o PRIMEIRO serviço do sistema. Ele:
 *            1) Limpa a tela (modo 13h continua ativo)
 *            2) Prepara o terreno: zera a área 0x80000..0x88000
 *            3) Lê o ext4 via ATA PIO (sem BIOS, modo protegido)
 *            4) Procura /interface/grain, carrega em GRAIN_MEM (0x80000)
 *            5) Ajusta a pilha (0x90000) e salta para grain — a interface
 *               gráfica, um binário x86-32 puro (sem C, sem VM), assume
 *
 *   Sem biblioteca padrão: compilado freestanding com g++ -m32 e
 *   linkado como binário plano em 0x60000 (ver copilar.sh).
 * ========================================================================== */

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;

#define LARGURA_TELA 320
#define ALTURA_TELA  200

/* Memória de vídeo do modo 13h (320x200x8 bits). O modo continua
 * ligado: o kernel deixou a CPU em modo protegido com o modo 13h. */
static volatile u8 *const video = (volatile u8 *)0xA0000;

/* Onde o grain será carregado — logo depois do usr (0x60000 + 128 KiB = 0x80000).
 * A área reservada vai de 0x80000 até 0x88000 (32 KiB). A pilha fica em
 * 0x90000 e cresce para baixo, tendo 32 KiB de folga antes de tocar o grain. */
#define GRAIN_MEM   0x80000u
#define GRAIN_MAX   0x8000u       /* 32 KiB: 0x80000..0x88000 */
#define PILHA_MEM   0x90000u      /* topo da pilha usada pelo grain */

/* Tamanho do bloco ext4 (1 KiB = 2 setores) */
#define BLOCO_EXT4  1024

/* Endereços ATA (primário mestre) */
#define ATA_DATA    0x1F0
#define ATA_ERR     0x1F1
#define ATA_SECCNT  0x1F2
#define ATA_LBA0    0x1F3
#define ATA_LBA1    0x1F4
#define ATA_LBA2    0x1F5
#define ATA_DRIVE   0x1F6
#define ATA_CMD     0x1F7
#define ATA_ALT     0x3F6

/* Buffers de trabalho */
static u8 bloco[BLOCO_EXT4];    /* 1 bloco ext4 lido do disco */
static u8 inode[256];           /* inode copiado da tabela de inodes */
static u32 itblk;               /* primeiro bloco da tabela de inodes */

/* Protótipos */
static inline void outb(u16 porta, u8 valor);
static inline u8 inb(u16 porta);
static inline u16 inw(u16 porta);
static void espera_ata(void);
static u8 ler_setores(u32 lba, u16 n, u8 *dest);
static u8 ler_bloco(u32 b, u8 *dest);
static u16 u16_em(const u8 *p);
static u32 u32_em(const u8 *p);
static u8 compara(const u8 *a, const u8 *b, u8 n);
static u8 carrega_inode(u32 n);
static u8 acha_entrada(const char *nome, u8 len);
static u8 carrega_arquivo(u8 *destino);
static u8 carrega_grain(void);
static void limpar_tela(void);
static void preparar_terreno(void);
static void saltar_grain(void);

/* Ponto de entrada: o nucleo.rs salta para 0x60000, onde o copilar.sh
 * linkou este binário. */
extern "C" void _start(void)
{
    /* 1) Limpa a tela inteira: fundo totalmente preto. */
    limpar_tela();

    /* 2) Prepara o terreno: zera a área onde o grain será carregado. */
    preparar_terreno();

    /* 3) Procura /interface/grain no ext4, carrega em GRAIN_MEM. */
    if (carrega_grain()) {
        /* 4) Entrega a CPU para o grain (binário x86-32, sem C, sem VM). */
        saltar_grain();
    }

    /* Se falhou, fica parado com tela preta. */
    for (;;)
        __asm__ volatile("hlt");
}

/* ==========================================================================
 *   I/O de porta
 * ========================================================================== */
static inline void outb(u16 porta, u8 valor)
{
    __asm__ volatile("outb %0, %1" :: "a"(valor), "Nd"(porta));
}

static inline u8 inb(u16 porta)
{
    u8 valor;
    __asm__ volatile("inb %1, %0" : "=a"(valor) : "Nd"(porta));
    return valor;
}

static inline u16 inw(u16 porta)
{
    u16 valor;
    __asm__ volatile("inw %1, %0" : "=a"(valor) : "Nd"(porta));
    return valor;
}

/* ==========================================================================
 *   ATA PIO — leitura de setores do disco primário mestre (0x1F0)
 * ========================================================================== */
static void espera_ata(void)
{
    while (inb(ATA_CMD) & 0x80)    /* espera BSY = 0 */
        ;
}

static u8 ler_setores(u32 lba, u16 n, u8 *dest)
{
    u16 i, j;

    espera_ata();
    outb(ATA_ALT, 0x02);                       /* nIEN: só polimento, sem IRQ */
    outb(ATA_DRIVE, (u8)(0xE0 | ((lba >> 24) & 0x0F)));
    outb(ATA_ERR, 0x00);
    outb(ATA_SECCNT, (u8)n);                   /* contador de setores (< 256) */
    outb(ATA_LBA0, (u8)(lba & 0xFF));
    outb(ATA_LBA1, (u8)((lba >> 8) & 0xFF));
    outb(ATA_LBA2, (u8)((lba >> 16) & 0xFF));
    outb(ATA_CMD, 0x20);                       /* READ SECTORS */

    for (i = 0; i < n; i++) {
        u8 st;
        for (;;) {
            st = inb(ATA_CMD);
            if (st & 0x01) return 0;           /* ERR  */
            if (st & 0x20) return 0;           /* DF   */
            if (st & 0x08) break;              /* DRQ  */
        }
        for (j = 0; j < 256; j++) {            /* 256 palavras = 1 setor */
            *(u16 *)dest = inw(ATA_DATA);
            dest += 2;
        }
    }
    return 1;
}

static u8 ler_bloco(u32 b, u8 *dest)
{
    return ler_setores(b * 2, 2, dest);        /* bloco de 1 KiB = 2 setores */
}

/* ==========================================================================
 *   ext4 — leitura mínima para achar e carregar /interface/grain
 * ========================================================================== */
static u16 u16_em(const u8 *p)
{
    return (u16)(p[0] | ((u16)p[1] << 8));
}

static u32 u32_em(const u8 *p)
{
    return (u32)p[0] | ((u32)p[1] << 8) | ((u32)p[2] << 16) | ((u32)p[3] << 24);
}

static u8 compara(const u8 *a, const u8 *b, u8 n)
{
    u8 i;
    for (i = 0; i < n; i++) {
        if (a[i] != b[i])
            return 0;
    }
    return 1;
}

/* n (inode) -> inode[256] */
static u8 carrega_inode(u32 n)
{
    u32 b = itblk + (n - 1) / 4;              /* 4 inodes de 256 bytes por bloco */
    u32 off = ((n - 1) & 3) * 256;
    u16 i;

    if (!ler_bloco(b, bloco))
        return 0;
    for (i = 0; i < 256; i++)
        inode[i] = bloco[off + i];
    return 1;
}

/* Procura "nome" no diretório guardado em inode[]; se achar, recarrega
 * inode[] com o inode encontrado. Devolve 0 = falha. */
static u8 acha_entrada(const char *nome, u8 len)
{
    u32 b, off;

    if (u16_em(inode + 0x28) != 0xF30A)       /* magic da árvore de extensões */
        return 0;
    if (u16_em(inode + 0x2E) != 0)            /* profundidade 0 (folha) */
        return 0;

    b = u32_em(inode + 0x34 + 8)              /* ee_start_lo */
        | ((u32)u16_em(inode + 0x34 + 6) << 16);  /* ee_start_hi */
    if (b == 0 || !ler_bloco(b, bloco))
        return 0;

    for (off = 0; off < BLOCO_EXT4; ) {
        const u8 *e = bloco + off;
        u32 ino = u32_em(e);
        u16 rec = u16_em(e + 4);

        if (ino != 0 && e[6] == len &&
            compara(e + 8, (const u8 *)nome, len))
            return carrega_inode(ino);
        if (rec == 0)
            return 0;
        off += rec;
    }
    return 0;
}

/* Lê todas as extensões do inode[] para destino (tamanho máximo GRAIN_MAX) */
static u8 carrega_arquivo(u8 *destino)
{
    u16 n, i, j;
    const u8 *ext;
    u32 tam = u32_em(inode + 4);              /* i_size_lo */

    if (u16_em(inode + 0x28) != 0xF30A)
        return 0;
    if (u16_em(inode + 0x2E) != 0)
        return 0;
    if (tam == 0 || tam > GRAIN_MAX)
        return 0;

    n = u16_em(inode + 0x2A);                 /* número de extensões */
    if (n > 4)
        return 0;                             /* só folhas com 4 extensões */

    ext = inode + 0x34;                       /* primeira extensão */
    for (i = 0; i < n; i++, ext += 12) {
        u32 log = u32_em(ext);                        /* ee_block (lógico) */
        u32 fis = u32_em(ext + 8)                     /* ee_start_lo */
                  | ((u32)u16_em(ext + 6) << 16);     /* ee_start_hi */
        u16 len = u16_em(ext + 4) & 0x7FFF;           /* ee_len */

        for (j = 0; j < len; j++) {
            u32 desloc = (log + j) * (u32)BLOCO_EXT4;
            if (desloc >= GRAIN_MAX)
                return 0;
            if (!ler_bloco(fis + j, destino + desloc))
                return 0;
        }
    }
    return 1;
}

/* /interface/grain -> GRAIN_MEM (0x80000) */
static u8 carrega_grain(void)
{
    /* descritor do grupo 0 (bloco 2) -> tabela de inodes */
    if (!ler_bloco(2, bloco))
        return 0;
    itblk = u32_em(bloco + 8);                /* bg_inode_table_lo */

    if (!carrega_inode(2))                    /* inode raiz */
        return 0;
    if (!acha_entrada("interface", 9))        /* /interface */
        return 0;
    if (!acha_entrada("grain", 5))            /* /interface/grain */
        return 0;

    return carrega_arquivo((u8 *)GRAIN_MEM);
}

/* ==========================================================================
 *   Tela
 * ========================================================================== */
static void limpar_tela(void)
{
    for (int i = 0; i < LARGURA_TELA * ALTURA_TELA; i++)
        video[i] = 0;                         /* preto */
}

/* ==========================================================================
 *   Preparação do terreno para o grain
 *
 *   Zera toda a área reservada ao grain (0x80000..0x88000) antes de copiar
 *   o binário, e deixa o segmento de pilha pronto em 0x90000. Assim o grain
 *   recebe uma região limpa e determinística — sem lixo do que estava antes.
 * ========================================================================== */
static void preparar_terreno(void)
{
    volatile u8 *area = (volatile u8 *)GRAIN_MEM;
    u32 i;

    for (i = 0; i < GRAIN_MAX; i++)
        area[i] = 0;
}

/* ==========================================================================
 *   Passagem de controle para o grain
 * ========================================================================== */
static void saltar_grain(void)
{
    /* pilha limpa em PILHA_MEM (0x90000) e salto para GRAIN_MEM (0x80000),
     * onde /interface/grain (binário x86-32) foi carregado */
    __asm__ volatile(
        "mov %1, %%esp\n\t"
        "mov %0, %%eax\n\t"
        "jmp *%%eax\n\t"
        :: "r"(GRAIN_MEM), "r"(PILHA_MEM) : "memory");
    __builtin_unreachable();
}