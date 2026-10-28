/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.8.2026
 *   ini/inicio.c — Segundo estágio de boot do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido (o inimin entra em modo protegido
 *            e salta para 0x10000, onde este binário foi carregado)
 *   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
 *   Estado : fundo totalmente preto; na primeira linha, centralizada,
 *            a mensagem "CosmOS Beta v0.1 Construção 0.8.2026"
 *   Depois : lê o ext4 da imagem (ATA PIO — a BIOS não existe em modo
 *            protegido), procura /nucleo/nucos, carrega em 0x20000,
 *            espera 2 segundos (PIT por polling, sem interrupções) e
 *            passa o controle para o núcleo, que limpa a tela e escreve
 *            "Entrou no Nucos" na primeira linha, à esquerda.
 *
 *   Sem biblioteca padrão: compilado como freestanding e linkado como
 *   binário plano em 0x10000 (ver copilar.sh).
 * ========================================================================== */

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;

#define LARGURA_TELA 320
#define ALTURA_TELA  200
#define COR_TEXTO    15          /* branco no modo 13h */

#define BLOCO_EXT4   1024        /* blocos do ext4 gerado pelo copilar.sh */
#define TICKS_SEGUNDO 1193182u   /* frequência base do PIT */
#define MAX_ESPERA   65536u      /* maior carga de um canal do PIT (0 = 65536) */

#define NUCOS_MEM    0x20000u    /* onde /nucleo/nucos é carregado */
#define NUCOS_MAX    0x40000u    /* tamanho máximo do núcleo (256 KiB) */

static u8 *const video = (u8 *)0xA0000;   /* memória de vídeo do modo 13h */

/* Buffers de trabalho: fora do caminho do bootloader e da pilha (0x90000) */
static u8 bloco[BLOCO_EXT4];    /* 1 bloco ext4 lido do disco */
static u8 inode[256];           /* inode copiado da tabela de inodes */
static u32 itblk;               /* primeiro bloco da tabela de inodes */

/* Fonte 8x8 desenhada à mão, apenas os caracteres usados na mensagem.
 * Cada glifo ocupa 8 bytes (8 linhas); os 5 bits mais à esquerda de cada
 * byte são os 5 pixels de largura do caractere. */
struct glifo {
    u8 codigo;
    u8 linhas[8];
};

static const struct glifo fonte[] = {
    { ' ', { 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 } },
    { 'C', { 0x70, 0x88, 0x80, 0x80, 0x80, 0x88, 0x70, 0x00 } },
    { 'o', { 0x00, 0x00, 0x70, 0x88, 0x88, 0x88, 0x70, 0x00 } },
    { 'n', { 0x00, 0x00, 0xD8, 0xC8, 0x88, 0x88, 0x88, 0x00 } },
    { 's', { 0x00, 0x00, 0x78, 0x80, 0x70, 0x08, 0xF0, 0x00 } },
    { 't', { 0x20, 0x20, 0xF8, 0x20, 0x20, 0x28, 0x18, 0x00 } },
    { 'r', { 0x00, 0x00, 0xD8, 0xC8, 0x80, 0x80, 0x80, 0x00 } },
    { 'u', { 0x00, 0x00, 0x88, 0x88, 0x88, 0x98, 0x68, 0x00 } },
    { 'm', { 0x00, 0x00, 0xD8, 0xA8, 0xA8, 0xA8, 0xA8, 0x00 } },
    { 'O', { 0x70, 0x88, 0x88, 0x88, 0x88, 0x88, 0x70, 0x00 } },
    { 'S', { 0x78, 0x80, 0x70, 0x08, 0x08, 0x88, 0x70, 0x00 } },
    { 'B', { 0xF0, 0x88, 0x88, 0xF0, 0x88, 0x88, 0xF0, 0x00 } },
    { 'e', { 0x00, 0x00, 0x70, 0x88, 0xF8, 0x80, 0x70, 0x00 } },
    { 'a', { 0x00, 0x00, 0x70, 0x08, 0x78, 0x88, 0x78, 0x00 } },
    { 'v', { 0x00, 0x00, 0x88, 0x88, 0x88, 0x50, 0x20, 0x00 } },
    { '0', { 0x70, 0x88, 0x98, 0xA8, 0xC8, 0x88, 0x70, 0x00 } },
    { '.', { 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00 } },
    { '1', { 0x20, 0x60, 0x20, 0x20, 0x20, 0x20, 0x70, 0x00 } },
    { '2', { 0x70, 0x88, 0x08, 0x10, 0x20, 0x40, 0xF8, 0x00 } },
    { '3', { 0x70, 0x88, 0x08, 0x38, 0x08, 0x88, 0x70, 0x00 } },
    { '4', { 0x10, 0x30, 0x50, 0x90, 0xF8, 0x10, 0x10, 0x00 } },
    { '5', { 0xF8, 0x80, 0xF0, 0x08, 0x08, 0x88, 0x70, 0x00 } },
    { '6', { 0x30, 0x40, 0x80, 0xF0, 0x88, 0x88, 0x70, 0x00 } },
    { '7', { 0xF8, 0x08, 0x10, 0x20, 0x40, 0x40, 0x40, 0x00 } },
    { '8', { 0x70, 0x88, 0x88, 0x70, 0x88, 0x88, 0x70, 0x00 } },
    { '9', { 0x70, 0x88, 0x88, 0xF0, 0x80, 0x40, 0x30, 0x00 } },
    { 0xE7, { 0x00, 0x00, 0x70, 0x80, 0x80, 0x80, 0x70, 0x10 } }, /* ç */
    { 0xE3, { 0x58, 0x00, 0x70, 0x08, 0x78, 0x88, 0x78, 0x00 } }  /* ã */
};

#define N_GLIFOS (sizeof(fonte) / sizeof(fonte[0]))

static void desenhar_texto(void);
static u8  ler_setores(u32 lba, u16 n, u8 *dest);
static u8  ler_bloco(u32 bloco, u8 *dest);
static u8  carrega_inode(u32 n);
static u8  acha_entrada(const char *nome, u8 len);
static u8  carrega_nucos(void);
static void espera_ticks(u32 total);
static void saltar_nucos(void);

/* Ponto de entrada: o inimin salta para 0x10000 em modo protegido.
 * Esta função é a primeira do arquivo para ficar exatamente em 0x10000
 * (o build usa -fno-toplevel-reorder). */
void _start(void)
{
    desenhar_texto();

    /* Procura /nucleo/nucos no ext4, carrega em 0x20000, espera 2 s
     * e entrega a CPU para o núcleo. Se falhar, fica parado com a
     * mensagem de construçao na tela. */
    if (carrega_nucos()) {
        espera_ticks(2 * TICKS_SEGUNDO);
        saltar_nucos();
    }

    for (;;)
        ;
}

/* ==========================================================================
 *   I/O de porta e espera
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
 *
 *   Em modo protegido não existe mais a int 0x13 do BIOS, então o
 *   segundo estágio fala direto com o controlador IDE. O QEMU monta a
 *   CosmOS.img como primário mestre (-drive ... if=ide).
 * ========================================================================== */

static void espera_ata(void)
{
    while (inb(0x1F7) & 0x80)    /* espera BSY = 0 */
        ;
}

static u8 ler_setores(u32 lba, u16 n, u8 *dest)
{
    u16 i, j;

    espera_ata();
    outb(0x3F6, 0x02);                       /* nIEN: só polimento, sem IRQ */
    outb(0x1F6, (u8)(0xE0 | ((lba >> 24) & 0x0F)));
    outb(0x1F1, 0x00);
    outb(0x1F2, (u8)n);                      /* contador de setores (< 256) */
    outb(0x1F3, (u8)(lba & 0xFF));
    outb(0x1F4, (u8)((lba >> 8) & 0xFF));
    outb(0x1F5, (u8)((lba >> 16) & 0xFF));
    outb(0x1F7, 0x20);                       /* READ SECTORS */

    for (i = 0; i < n; i++) {
        u8 st;
        for (;;) {
            st = inb(0x1F7);
            if (st & 0x01) return 0;         /* ERR  */
            if (st & 0x20) return 0;         /* DF   */
            if (st & 0x08) break;            /* DRQ  */
        }
        for (j = 0; j < 256; j++) {          /* 256 palavras = 1 setor */
            *(u16 *)dest = inw(0x1F0);
            dest += 2;
        }
    }
    return 1;
}

static u8 ler_bloco(u32 b, u8 *dest)
{
    return ler_setores(b * 2, 2, dest);      /* bloco de 1 KiB = 2 setores */
}

/* ==========================================================================
 *   ext4 — os mesmos passos do inimin.asm, agora em C
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
    u32 b = itblk + (n - 1) / 4;             /* 4 inodes de 256 bytes por bloco */
    u32 off = ((n - 1) & 3) * 256;
    u16 i;                                   /* u16: u8 < 256 seria sempre verdade! */

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

    if (u16_em(inode + 0x28) != 0xF30A)      /* magic da árvore de extensões */
        return 0;
    if (u16_em(inode + 0x2E) != 0)           /* profundidade 0 (folha) */
        return 0;

    b = u32_em(inode + 0x34 + 8)             /* ee_start_lo */
        | ((u32)u16_em(inode + 0x34 + 6) << 16);   /* ee_start_hi */
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

/* Lê todas as extensões do inode[] para destino (tamanho máximo NUCOS_MAX) */
static u8 carrega_arquivo(u8 *destino)
{
    u16 n, i, j;
    const u8 *ext;
    u32 tam = u32_em(inode + 4);             /* i_size_lo */

    if (u16_em(inode + 0x28) != 0xF30A)
        return 0;
    if (u16_em(inode + 0x2E) != 0)
        return 0;
    if (tam == 0 || tam > NUCOS_MAX)
        return 0;

    n = u16_em(inode + 0x2A);                /* número de extensões */
    if (n > 4)
        return 0;                            /* só folhas com 4 extensões */

    ext = inode + 0x34;                      /* primeira extensão */
    for (i = 0; i < n; i++, ext += 12) {
        u32 log = u32_em(ext);                       /* ee_block (lógico) */
        u32 fis = u32_em(ext + 8)                    /* ee_start_lo */
                  | ((u32)u16_em(ext + 6) << 16);    /* ee_start_hi */
        u16 len = u16_em(ext + 4) & 0x7FFF;          /* ee_len */

        for (j = 0; j < len; j++) {
            u32 desloc = (log + j) * (u32)BLOCO_EXT4;
            if (desloc >= NUCOS_MAX)
                return 0;
            if (!ler_bloco(fis + j, destino + desloc))
                return 0;
        }
    }
    return 1;
}

/* /nucleo/nucos -> NUCOS_MEM (0x20000) */
static u8 carrega_nucos(void)
{
    /* descritor do grupo 0 (bloco 2) -> tabela de inodes */
    if (!ler_bloco(2, bloco))
        return 0;
    itblk = u32_em(bloco + 8);               /* bg_inode_table_lo */

    if (!carrega_inode(2))                   /* inode raiz */
        return 0;
    if (!acha_entrada("nucleo", 6))           /* /nucleo */
        return 0;
    if (!acha_entrada("nucos", 5))            /* /nucleo/nucos */
        return 0;

    return carrega_arquivo((u8 *)NUCOS_MEM);
}

/* ==========================================================================
 *   Espera de 2 segundos — PIT canal 0 por polling (sem interrupções)
 *
 *   O PIT conta a 1193182 Hz. Uma carga de 65536 ticks dura ~54,9 ms;
 *   contamos 2 s em pedaços de no máximo 65536 ticks, lendo o contador
 *   travado (latch) até ele chegar a 0. A CPU está com interrupções
 *   desligadas desde o inimin — nada de IDT aqui.
 * ========================================================================== */

static void programa_pit(u16 carga)
{
    outb(0x43, 0x30);                       /* canal 0, lo/hi, modo one-shot */
    outb(0x40, (u8)(carga & 0xFF));
    outb(0x40, (u8)(carga >> 8));
}

static void espera_zerar(void)
{
    for (;;) {
        u16 atual;
        outb(0x43, 0x00);                   /* trava o contador do canal 0 */
        atual  = inb(0x40);
        atual |= (u16)inb(0x40) << 8;
        if (atual == 0)
            return;
    }
}

static void espera_ticks(u32 total)
{
    while (total >= MAX_ESPERA) {
        programa_pit(0);                    /* 0 = 65536 ticks ≈ 54,9 ms */
        espera_zerar();
        total -= MAX_ESPERA;
    }
    if (total > 0) {
        programa_pit((u16)total);
        espera_zerar();
    }
}

/* ==========================================================================
 *   Passagem de controle para o núcleo
 * ========================================================================== */

static void saltar_nucos(void)
{
    /* pilha limpa em 0x90000 (onde o inimin a deixou) e salto para
     * 0x20000, onde /nucleo/nucos foi carregado */
    __asm__ volatile(
        "mov $0x90000, %%esp\n\t"
        "mov $0x20000, %%eax\n\t"
        "jmp *%%eax\n\t"
        :: : "memory");
    __builtin_unreachable();
}

/* ==========================================================================
 *   Tela — fundo preto e a mensagem centralizada na primeira linha
 * ========================================================================== */

static const u8 *achar_glifo(u8 codigo)
{
    u8 i;
    for (i = 0; i < N_GLIFOS; i++) {
        if (fonte[i].codigo == codigo)
            return fonte[i].linhas;
    }
    return (const u8 *)0;
}

/* Decodifica um caractere UTF-8. A mensagem só usa ASCII, ç e ã;
 * devolve o código do glifo e avança o índice. */
static u8 proximo_caractere(const u8 *texto, u16 *indice)
{
    u8 c = texto[*indice];

    if (c < 0x80) {
        (*indice)++;
        return c;
    }

    if (c == 0xC3) {
        u8 c2 = texto[*indice + 1];
        *indice += 2;
        if (c2 == 0xA7)
            return 0xE7;        /* ç */
        if (c2 == 0xA3)
            return 0xE3;        /* ã */
        return '?';
    }

    (*indice)++;
    return '?';
}

static u16 comprimento(const u8 *texto)
{
    u16 i = 0, n = 0;
    while (texto[i] != 0) {
        proximo_caractere(texto, &i);
        n++;
    }
    return n;
}

static void desenhar_texto(void)
{
    static const u8 mensagem[] =
        "CosmOS Beta v0.1 Construção 0.8.2026";

    u16 total = comprimento(mensagem);
    u16 i = 0;
    u16 x = (u16)((LARGURA_TELA - total * 8) / 2);
    const u16 y = 2;               /* primeira linha, com 2 px de margem */

    while (mensagem[i] != 0) {
        u8 codigo = proximo_caractere(mensagem, &i);
        const u8 *linhas = achar_glifo(codigo);

        if (linhas != 0) {
            u8 linha, coluna;
            for (linha = 0; linha < 8; linha++) {
                u8 bits = linhas[linha];
                for (coluna = 0; coluna < 8; coluna++) {
                    if (bits & (0x80 >> coluna)) {
                        video[(u32)((y + linha) * LARGURA_TELA + x + coluna)]
                            = COR_TEXTO;
                    }
                }
            }
        }
        x = (u16)(x + 8);
    }
}
