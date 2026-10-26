/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.2.2026
 *   ini/inicio.c — Segundo estágio de boot do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido (o inimin entra em modo protegido
 *            e salta para 0x10000, onde este binário foi carregado)
 *   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
 *   Estado : fundo totalmente preto; na primeira linha, centralizada,
 *            a mensagem "CosmOS Beta v0.1 Construção 0.2.2026"
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

static u8 *const video = (u8 *)0xA0000;   /* memória de vídeo do modo 13h */

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
    { '6', { 0x30, 0x40, 0x80, 0xF0, 0x88, 0x88, 0x70, 0x00 } },
    { 0xE7, { 0x00, 0x00, 0x70, 0x80, 0x80, 0x80, 0x70, 0x10 } }, /* ç */
    { 0xE3, { 0x58, 0x00, 0x70, 0x08, 0x78, 0x88, 0x78, 0x00 } }  /* ã */
};

#define N_GLIFOS (sizeof(fonte) / sizeof(fonte[0]))

static void desenhar_texto(void);

/* Ponto de entrada: o inimin salta para 0x10000 em modo protegido.
 * Esta função é a primeira do arquivo para ficar exatamente em 0x10000
 * (o build usa -fno-toplevel-reorder). */
void _start(void)
{
    desenhar_texto();
    for (;;)
        ;
}

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
        "CosmOS Beta v0.1 Construção 0.2.2026";

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
