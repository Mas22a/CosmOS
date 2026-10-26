#!/usr/bin/env bash
# ==========================================================================
#   CosmOS Beta v0.1 — Construção 0.1.2026
#   copilar.sh — compila o primeiro estágio e gera a imagem CosmOS.img
# --------------------------------------------------------------------------
#   O que este script faz:
#     1) Monta ini/inimin.asm  →  ini/inimin (setor de boot de 512 bytes)
#     2) Cria a imagem vazia CosmOS.img com 20 MB
#     3) Formata a imagem em ext4
#     4) Cria somente a pasta inicio/ dentro da imagem e coloca o inimin
#        nela (sem precisar de root, via debugfs) e apaga o lost+found
#        que o ext4 cria sozinho — assim só existe inicio/
#     5) Escreve o setor de boot no início da imagem (os primeiros
#        1024 bytes do ext4 são a área reservada do boot block, então
#        o sistema de arquivos NÃO é corrompido)
#
#   Uso: ./copilar.sh
# ==========================================================================
set -euo pipefail

# As ferramentas do ext4 (/usr/sbin) podem estar fora do PATH do usuário
export PATH="/usr/sbin:/sbin:$PATH"

RAIZ="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ASM="$RAIZ/ini/inimin.asm"
BIN="$RAIZ/ini/inimin"
IMG="$RAIZ/CosmOS.img"
TAMANHO_MB=20

# --- 1) Compilar o primeiro estágio --------------------------------------
echo "[CosmOS] 1/5 Compilando ini/inimin.asm ..."
nasm -f bin "$ASM" -o "$BIN"

BYTES="$(wc -c < "$BIN")"
if [ "$BYTES" -ne 512 ]; then
    echo "[CosmOS] ERRO: ini/inimin tem $BYTES bytes; o setor de boot precisa ter exatamente 512." >&2
    exit 1
fi

# --- 2) Imagem vazia de 20 MB --------------------------------------------
echo "[CosmOS] 2/5 Criando CosmOS.img (${TAMANHO_MB} MB) ..."
dd if=/dev/zero of="$IMG" bs=1M count="$TAMANHO_MB" status=none

# --- 3) Formatar em ext4 --------------------------------------------------
echo "[CosmOS] 3/5 Formatando em ext4 ..."
mkfs.ext4 -F -q -L CosmOS "$IMG"

# --- 4) Só a pasta inicio/ com o inimin dentro ----------------------------
echo "[CosmOS] 4/5 Criando inicio/inimin dentro da imagem ..."
printf 'mkdir inicio\ncd inicio\nwrite %s inimin\ncd /\nrmdir lost+found\nquit\n' "$BIN" \
    | debugfs -w "$IMG" > /dev/null

if ! debugfs -R "ls -p /inicio" "$IMG" 2> /dev/null | grep -q 'inimin'; then
    echo "[CosmOS] ERRO: não foi possível gravar inicio/inimin dentro da imagem." >&2
    exit 1
fi

# --- 5) Gravar o setor de boot no início da imagem ------------------------
echo "[CosmOS] 5/5 Gravando o setor de boot no início da imagem ..."
dd if="$BIN" of="$IMG" bs=512 count=1 conv=notrunc status=none

echo "[CosmOS] Pronto! Imagem gerada: CosmOS.img (${TAMANHO_MB} MB, ext4)"
echo "[CosmOS] Conteúdo: /inicio/inimin   |   Setor de boot: inimin (512 bytes)"
