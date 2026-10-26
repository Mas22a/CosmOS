# CosmOS Beta v0.1 — Construção 0.2.2026

Sistema operacional construído do zero, começando pelo zero absoluto: o **primeiro
estágio de boot** (Assembly/NASM) e o **segundo estágio** (C freestanding), testados
no emulador QEMU.

> **Regra do projeto:** nenhum arquivo ou pasta é criado ou alterado sem permissão
> do desenvolvedor. Os únicos artefatos gerados automaticamente são os produtos do
> próprio `copilar.sh` (`ini/inimin`, `ini/inicio.o`, `ini/inicio` e `CosmOS.img`).

---

## Estrutura do projeto

```
Desenvolvimento/
├── .vscode/
│   └── tasks.json      # Tarefa única: copilar.sh + QEMU em sequência
├── ini/
│   ├── inimin.asm      # Primeiro estágio de boot (2 setores) — FONTE
│   └── inicio.c        # Segundo estágio de boot (C) — FONTE
├── copilar.sh          # Compila os dois estágios e gera a CosmOS.img
└── README.md           # Este arquivo
```

Gerados pelo `copilar.sh` (não versionados à mão):

```
├── ini/inimin          # Binário do inimin.asm (1024 bytes = 2 setores)
├── ini/inicio.o        # Objeto ELF gerado a partir de inicio.c
├── ini/inicio          # Binário plano do inicio.c (carregado em 0x10000)
└── CosmOS.img          # Imagem de 20 MB, ext4, pronta para bootar no QEMU
```

---

## Requisitos e instalação

Em Debian/Ubuntu (testado no Debian 13 “trixie”):

```bash
sudo apt update && sudo apt install -y nasm gcc binutils qemu-system-x86 e2fsprogs
```

| Ferramenta | Pacote | Para que serve |
|---|---|---|
| `nasm` | `nasm` | Montar o `ini/inimin.asm` |
| `gcc` | `gcc` | Compilar o `ini/inicio.c` em modo freestanding (`-m32`) |
| `ld`, `objcopy`, `nm` | `binutils` | Linkar o segundo estágio em binário plano e conferir `_start` |
| `qemu-system-x86_64` | `qemu-system-x86` | Executar a `CosmOS.img` |
| `mkfs.ext4` / `debugfs` / `e2fsck` | `e2fsprogs` | Criar, popular e verificar a imagem ext4 (sem root) |

No Arch Linux:

```bash
sudo pacman -S nasm gcc binutils qemu-system-x86 e2fsprogs
```

---

## Como compilar e executar

### Pelo VS Code (recomendado)

Abra a paleta de tarefas (`Ctrl+Shift+B` ou *Terminal → Executar tarefa*) e
execute a tarefa única:

- **CosmOS: compilar e executar no QEMU** — roda o `copilar.sh` e, se der certo,
  abre a `CosmOS.img` no emulador (tudo em um só comando)

### Pela linha de comando

```bash
./copilar.sh      # compila os estágios e gera a CosmOS.img
qemu-system-x86_64 -drive format=raw,file=CosmOS.img,if=ide -boot order=c -m 128M
```

**Resultado:** o QEMU abre a tela do CosmOS em **320×200 (modo 13h)** com fundo
**totalmente preto** e, na **primeira linha, centralizada**, a mensagem:

```
CosmOS Beta v0.1 Construção 0.2.2026
```

---

## Fluxo de boot

```
BIOS
 └─> setor 0 (0x7C00)        inimin — stub de 512 bytes
      └─> setor 1 (0x7E00)   inimin — leitor ext4
           └─> /inicio/inicio (ext4)
                └─> 0x10000   início do modo protegido de 32 bits
                     └─> inicio.c desenha a mensagem na tela
```

### Fase 1 — `ini/inimin.asm` (primeiro estágio)

| Item | Valor |
|---|---|
| Arquivo | `ini/inimin.asm` |
| Binário | `ini/inimin` |
| Tamanho | **exatamente 1024 bytes** (2 setores) |
| Arquitetura alvo | **x86-64** — o BIOS entrega a CPU em modo real de 16 bits |
| Vídeo | **Modo 13h: 320×200, 256 cores (8 bits por pixel)** |

O binário ocupa os dois primeiros setores da imagem, que formam o **bloco de boot
reservado do ext4** (os primeiros 1024 bytes). O sistema de arquivos começa apenas
no byte 1024, então nada do ext4 é corrompido.

**Setor 0 (0x7C00):** monta a pilha, lê o setor 1 com `int 0x13` (AH=42h, LBA) para
`0x7E00` e salta para lá.

**Setor 1 (0x7E00):** coloca o vídeo em modo 13h e localiza o segundo estágio
percorrendo o ext4:

1. lê o superbloco (LBA 2) — blocos de 1 KiB e inodes de 256 bytes;
2. lê o descritor do grupo 0 (bloco 2) e obtém a tabela de inodes;
3. lê o inode raiz (`inode 2`) e a sua árvore de extensões;
4. varre o bloco do diretório raiz procurando a entrada `inicio`;
5. repete o processo para `/inicio/inicio`;
6. lê as extensões do arquivo diretamente para `0x10000`;
7. entra em modo protegido (GDT flat, 32 bits) e salta para `0x10000`.

Premissas do ext4 (garantidas pelo `copilar.sh`): blocos de **1024 bytes**, inodes
de **256 bytes** e árvore de extensões em profundidade 0 (folha).

### Fase 2 — `ini/inicio.c` (segundo estágio)

| Item | Valor |
|---|---|
| Arquivo | `ini/inicio.c` |
| Binário | `ini/inicio` (binário plano, sem ELF) |
| Endereço de carga | **0x10000** (a entrada `_start` fica exatamente aí) |
| Arquitetura | **x86 32 bits, modo protegido** |
| Biblioteca | nenhuma — *freestanding*, sem libc |
| Vídeo | **Modo 13h: 320×200, 256 cores** |

O que ele faz:

1. não troca mais o modo de vídeo: o fundo continua preto;
2. usa uma fonte **8×8 desenhada à mão** (somente os caracteres da mensagem);
3. calcula a centralização: `x = (320 − caracteres × 8) / 2`;
4. desenha a mensagem na **primeira linha** (`y = 2`), em branco (cor 15);
5. entra em espera infinita.

A mensagem é tratada como UTF-8 para desenhar corretamente **`ç`** e **`ã`**.
O build usa `-fno-toplevel-reorder` para que `_start` seja o primeiro código do
binário; o `copilar.sh` confere com `nm` que ele está em `0x00010000`.

---

## Como o `copilar.sh` funciona

1. **Monta** `ini/inimin.asm` → `ini/inimin` e verifica que tem exatamente
   **1024 bytes**;
2. **Compila** `ini/inicio.c` com `gcc -m32` freestanding em `ini/inicio.o`,
   linka em `0x10000` (`ld -e _start`), converte para binário plano com
   `objcopy` e confere a entrada `_start` com `nm`;
3. **Cria** a `CosmOS.img` vazia com exatamente **20 MB** (`dd`);
4. **Formata** em **ext4** com rótulo `CosmOS`, blocos de **1 KiB** e inodes de
   **256 bytes** (`mkfs.ext4 -b 1024 -I 256`);
5. **Popula a imagem sem precisar de root**, usando `debugfs`:
   - cria somente a pasta `inicio/`;
   - copia `inimin` para `inicio/inimin`;
   - copia o segundo estágio para `inicio/inicio`;
   - apaga o `lost+found` que o ext4 cria sozinho;
   - confere se os dois arquivos estão lá;
6. **Grava os 2 setores de boot** (`ini/inimin`) no início da imagem com
   `dd conv=notrunc` — é o bloco de boot reservado do ext4;
7. **Verifica** a imagem com `e2fsck -f -n`.

Resultado final da `CosmOS.img`:

```
Bytes 0-511    : setor 0 do inimin (stub)
Bytes 512-1023 : setor 1 do inimin (leitor ext4)
Byte 1024      : superbloco ext4 (íntegro — verificado com e2fsck)
Raiz (ext4)    : /inicio/inimin   (1º estágio)
                 /inicio/inicio   (2º estágio)
```

---

## Verificação realizada

Testado de ponta a ponta no **QEMU 10.0.13**:

- `ini/inimin` com **1024 bytes** e assinatura `55 AA` no offset 510;
- `ini/inicio` com entrada `_start` em **0x10000**;
- `CosmOS.img` com **20971520 bytes** (20 MB) e `e2fsck` sem erros;
- raiz da imagem contendo **apenas** `/inicio`, com `inimin` e `inicio` como
  **dois arquivos separados**;
- boot real no QEMU: CPU em **modo protegido** (`CPL=0`, `EIP=0x1000b`),
  memória de vídeo em `0xA0000` com **somente 407 pixels brancos** — exatamente
  os glifos da mensagem — e todo o resto preto;
- mensagem ocupando `x = 16..300` e `y = 2..9` (primeira linha, centralizada).
