#!/usr/bin/env bash
# ==========================================================================
#   CosmOS Beta v0.1 — Construção 0.3.2026
#   copilar.sh — compila os estágios de boot e gera a imagem CosmOS.img
# --------------------------------------------------------------------------
#   O que este script faz:
#     1) Monta ini/inimin.asm  →  ini/inimin (1024 bytes = 2 setores)
#     2) Compila ini/inicio.c  →  ini/inicio.o (objeto ELF)
#                                →  ini/inicio   (binário plano em 0x10000)
#     3) Compila nucleo/nucleo.rs → nucleo/nucos (binário plano em 0x20000)
#     4) Cria a imagem vazia CosmOS.img com 20 MB
#     5) Formata a imagem em ext4 (blocos de 1 KiB, inodes de 256 bytes)
#     6) Cria somente as pastas inicio/ e nucleo/ dentro da imagem e coloca
#        o inimin, o inicio e o nucos nelas (sem precisar de root, via
#        debugfs) e apaga o lost+found que o ext4 cria sozinho
#     7) Escreve os 2 primeiros setores (bloco de boot ext4) no início
#        da imagem: o setor 0 carrega o setor 1, que localiza
#        /inicio/inicio no ext4 e passa o controle para ele — ele acha
#        /nucleo/nucos, espera 2 s e salta para o núcleo
#     8) Conferir o sistema de arquivos
#
#   Uso: ./copilar.sh
# ==========================================================================
set -euo pipefail

# As ferramentas do ext4 (/usr/sbin) podem estar fora do PATH do usuário
export PATH="$HOME/.cargo/bin:/usr/sbin:/sbin:$PATH"

RAIZ="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ASM="$RAIZ/ini/inimin.asm"
BIN="$RAIZ/ini/inimin"
FONTE_C="$RAIZ/ini/inicio.c"
OBJ_C="$RAIZ/ini/inicio.o"
BIN_C="$RAIZ/ini/inicio"
FONTE_RS="$RAIZ/nucleo/nucleo.rs"
NUCOS="$RAIZ/nucleo/nucos"
IMG="$RAIZ/CosmOS.img"
TAMANHO_MB=20

# Endereço onde o /nucleo/nucos é carregado e seu tamanho máximo
NUCOS_MEM=0x20000
NUCOS_MAX=262144          # 256 KiB (0x40000)

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- 0) Ferramentas --------------------------------------------------------
if ! command -v rustc > /dev/null 2>&1; then
    echo "[CosmOS] ERRO: rustc não encontrado. Instale o Rust com:" >&2
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal" >&2
    exit 1
fi

ALVO_LIB="$(rustc --print target-libdir --target i686-unknown-linux-gnu 2> /dev/null || true)"
if [ -z "${ALVO_LIB:-}" ] || [ ! -d "$ALVO_LIB" ]; then
    echo "[CosmOS] ERRO: o alvo i686-unknown-linux-gnu não está instalado. Rode:" >&2
    echo "    rustup target add i686-unknown-linux-gnu" >&2
    exit 1
fi

# --- 1) Compilar o primeiro estágio --------------------------------------
echo "[CosmOS] 1/8 Compilando ini/inimin.asm ..."
nasm -f bin "$ASM" -o "$BIN"

BYTES="$(wc -c < "$BIN")"
if [ "$BYTES" -ne 1024 ]; then
    echo "[CosmOS] ERRO: ini/inimin tem $BYTES bytes; o primeiro estágio precisa ter exatamente 1024 (2 setores)." >&2
    exit 1
fi

# --- 2) Compilar o segundo estágio (C freestanding, binário plano) -------
echo "[CosmOS] 2/8 Compilando ini/inicio.c ..."
gcc -m32 -Os -ffreestanding -nostdlib -nostartfiles \
    -fno-pie -fno-builtin -fno-stack-protector \
    -fno-asynchronous-unwind-tables -fno-unwind-tables \
    -fno-toplevel-reorder \
    -c "$FONTE_C" -o "$OBJ_C"

ld -m elf_i386 -Ttext=0x10000 -e _start --nmagic \
    -o "$TMP/inicio.elf" "$OBJ_C"
objcopy -O binary "$TMP/inicio.elf" "$BIN_C"

ENTRADA="$(nm "$TMP/inicio.elf" | awk '$3 == "_start" { print $1 }')"
if [ "$ENTRADA" != "00010000" ]; then
    echo "[CosmOS] ERRO: _start está em 0x$ENTRADA; o inimin salta para 0x10000." >&2
    exit 1
fi

TAM_C="$(wc -c < "$BIN_C")"
if [ "$TAM_C" -le 0 ] || [ "$TAM_C" -gt 61440 ]; then
    echo "[CosmOS] ERRO: ini/inicio tem $TAM_C bytes (esperado entre 1 e 61440)." >&2
    exit 1
fi

# --- 3) Compilar o núcleo (Rust no_std, binário plano) --------------------
echo "[CosmOS] 3/8 Compilando nucleo/nucleo.rs ..."
rustc --edition 2021 --target i686-unknown-linux-gnu \
    -C panic=abort -C opt-level=s -C relocation-model=static \
    --emit=obj -o "$TMP/nucleo.o" "$FONTE_RS"

# O nucleo.ld garante que o _start seja o primeiro byte em 0x20000
ld -m elf_i386 -T "$RAIZ/nucleo/nucleo.ld" --nmagic \
    -o "$TMP/nucos.elf" "$TMP/nucleo.o"
objcopy -O binary "$TMP/nucos.elf" "$NUCOS"

ENTRADA_N="$(nm "$TMP/nucos.elf" | awk '$3 == "_start" { print $1 }')"
if [ "$ENTRADA_N" != "00020000" ]; then
    echo "[CosmOS] ERRO: _start do núcleo está em 0x$ENTRADA_N; o inicio salta para 0x20000." >&2
    exit 1
fi

TAM_N="$(wc -c < "$NUCOS")"
if [ "$TAM_N" -le 0 ] || [ "$TAM_N" -gt "$NUCOS_MAX" ]; then
    echo "[CosmOS] ERRO: nucleo/nucos tem $TAM_N bytes (esperado entre 1 e $NUCOS_MAX)." >&2
    exit 1
fi

# --- 4) Imagem vazia de 20 MB --------------------------------------------
echo "[CosmOS] 4/8 Criando CosmOS.img (${TAMANHO_MB} MB) ..."
dd if=/dev/zero of="$IMG" bs=1M count="$TAMANHO_MB" status=none

# --- 5) Formatar em ext4 --------------------------------------------------
echo "[CosmOS] 5/8 Formatando em ext4 (blocos de 1 KiB, inodes de 256 bytes) ..."
mkfs.ext4 -F -q -L CosmOS -b 1024 -I 256 "$IMG"

# --- 6) Só as pastas inicio/ e nucleo/ com os binários dentro -------------
echo "[CosmOS] 6/8 Criando inicio/ e nucleo/ com os binários dentro da imagem ..."
printf 'mkdir inicio\nmkdir nucleo\ncd inicio\nwrite %s inimin\nwrite %s inicio\ncd ../nucleo\nwrite %s nucos\ncd /\nrmdir lost+found\nquit\n' \
    "$BIN" "$BIN_C" "$NUCOS" | debugfs -w "$IMG" > /dev/null

for ARQ in inicio/inimin inicio/inicio nucleo/nucos; do
    if ! debugfs -R "stat /$ARQ" "$IMG" 2> /dev/null | grep -q 'Inode:'; then
        echo "[CosmOS] ERRO: não foi possível gravar /$ARQ dentro da imagem." >&2
        exit 1
    fi
done

# --- 7) Gravar o bloco de boot (2 setores) no início da imagem -----------
echo "[CosmOS] 7/8 Gravando o primeiro estágio no bloco de boot (2 setores) ..."
dd if="$BIN" of="$IMG" bs=512 count=2 conv=notrunc status=none

# --- 8) Conferir o sistema de arquivos -----------------------------------
echo "[CosmOS] 8/8 Verificando a imagem ..."
e2fsck -f -n "$IMG" > /dev/null

echo "[CosmOS] Pronto! Imagem gerada: CosmOS.img (${TAMANHO_MB} MB, ext4)"
echo "[CosmOS] Conteúdo: /inicio/inimin (1º estágio)  |  /inicio/inicio (2º estágio, ${TAM_C} bytes)"
echo "[CosmOS]           /nucleo/nucos (núcleo em Rust, ${TAM_N} bytes, carregado em 0x20000)"
echo "[CosmOS] Setor de boot: 2 setores (1024 bytes) — setor 0 → setor 1 → /inicio/inicio → /nucleo/nucos"
