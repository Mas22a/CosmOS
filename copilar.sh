#!/usr/bin/env bash
# ==========================================================================
#   CosmOS Beta v0.1 — Construção 0.2.2026
#   copilar.sh — compila os estágios de boot e gera a imagem CosmOS.img
# --------------------------------------------------------------------------
#   O que este script faz:
#     1) Monta ini/inimin.asm  →  ini/inimin (1024 bytes = 2 setores)
#     2) Compila ini/inicio.c  →  ini/inicio.o (objeto ELF)
#                                →  ini/inicio   (binário plano em 0x10000)
#     3) Cria a imagem vazia CosmOS.img com 20 MB
#     4) Formata a imagem em ext4 (blocos de 1 KiB, inodes de 256 bytes)
#     5) Cria somente a pasta inicio/ dentro da imagem e coloca o inimin
#        e o inicio nela (sem precisar de root, via debugfs) e apaga o
#        lost+found que o ext4 cria sozinho
#     6) Escreve os 2 primeiros setores (bloco de boot ext4) no início
#        da imagem: o setor 0 carrega o setor 1, que localiza
#        /inicio/inicio no ext4 e passa o controle para ele
#
#   Uso: ./copilar.sh
# ==========================================================================
set -euo pipefail

# As ferramentas do ext4 (/usr/sbin) podem estar fora do PATH do usuário
export PATH="/usr/sbin:/sbin:$PATH"

RAIZ="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ASM="$RAIZ/ini/inimin.asm"
BIN="$RAIZ/ini/inimin"
FONTE_C="$RAIZ/ini/inicio.c"
OBJ_C="$RAIZ/ini/inicio.o"
BIN_C="$RAIZ/ini/inicio"
IMG="$RAIZ/CosmOS.img"
TAMANHO_MB=20

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- 1) Compilar o primeiro estágio --------------------------------------
echo "[CosmOS] 1/7 Compilando ini/inimin.asm ..."
nasm -f bin "$ASM" -o "$BIN"

BYTES="$(wc -c < "$BIN")"
if [ "$BYTES" -ne 1024 ]; then
    echo "[CosmOS] ERRO: ini/inimin tem $BYTES bytes; o primeiro estágio precisa ter exatamente 1024 (2 setores)." >&2
    exit 1
fi

# --- 2) Compilar o segundo estágio (C freestanding, binário plano) -------
echo "[CosmOS] 2/7 Compilando ini/inicio.c ..."
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

# --- 3) Imagem vazia de 20 MB --------------------------------------------
echo "[CosmOS] 3/7 Criando CosmOS.img (${TAMANHO_MB} MB) ..."
dd if=/dev/zero of="$IMG" bs=1M count="$TAMANHO_MB" status=none

# --- 4) Formatar em ext4 --------------------------------------------------
echo "[CosmOS] 4/7 Formatando em ext4 (blocos de 1 KiB, inodes de 256 bytes) ..."
mkfs.ext4 -F -q -L CosmOS -b 1024 -I 256 "$IMG"

# --- 5) Só a pasta inicio/ com o inimin e o inicio dentro ----------------
echo "[CosmOS] 5/7 Criando inicio/inimin e inicio/inicio dentro da imagem ..."
printf 'mkdir inicio\ncd inicio\nwrite %s inimin\nwrite %s inicio\ncd /\nrmdir lost+found\nquit\n' \
    "$BIN" "$BIN_C" | debugfs -w "$IMG" > /dev/null

for ARQ in inimin inicio; do
    if ! debugfs -R "stat /inicio/$ARQ" "$IMG" 2> /dev/null | grep -q 'Inode:'; then
        echo "[CosmOS] ERRO: não foi possível gravar inicio/$ARQ dentro da imagem." >&2
        exit 1
    fi
done

# --- 6) Gravar o bloco de boot (2 setores) no início da imagem -----------
echo "[CosmOS] 6/7 Gravando o primeiro estágio no bloco de boot (2 setores) ..."
dd if="$BIN" of="$IMG" bs=512 count=2 conv=notrunc status=none

# --- 7) Conferir o sistema de arquivos -----------------------------------
echo "[CosmOS] 7/7 Verificando a imagem ..."
e2fsck -f -n "$IMG" > /dev/null

echo "[CosmOS] Pronto! Imagem gerada: CosmOS.img (${TAMANHO_MB} MB, ext4)"
echo "[CosmOS] Conteúdo: /inicio/inimin (1º estágio)  |  /inicio/inicio (2º estágio, ${TAM_C} bytes)"
echo "[CosmOS] Setor de boot: 2 setores (1024 bytes) — setor 0 → setor 1 → /inicio/inicio"
