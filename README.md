# CosmOS Beta v0.1 — Construção 0.3.2026

Sistema operacional construído do zero, começando pelo zero absoluto: o **primeiro
estágio de boot** (Assembly/NASM), o **segundo estágio** (C freestanding) e o
**núcleo** (Rust `no_std`), testados no emulador QEMU.

> **Regra do projeto:** nenhum arquivo ou pasta é criado ou alterado sem permissão
> do desenvolvedor. Os únicos artefatos gerados automaticamente são os produtos do
> próprio `copilar.sh` (`ini/inimin`, `ini/inicio.o`, `ini/inicio`, `nucleo/nucos` e
> `CosmOS.img`).

---

## Estrutura do projeto

```
Desenvolvimento/
├── .vscode/
│   └── tasks.json      # Tarefa única: copilar.sh + QEMU em sequência
├── ini/
│   ├── inimin.asm      # Primeiro estágio de boot (2 setores) — FONTE
│   └── inicio.c        # Segundo estágio de boot (C) — FONTE
├── nucleo/
│   ├── nucleo.rs       # Núcleo do CosmOS (Rust no_std) — FONTE
│   └── nucleo.ld       # Linker script: _start é o primeiro byte em 0x20000
├── copilar.sh          # Compila os três estágios e gera a CosmOS.img
└── README.md           # Este arquivo
```

Gerados pelo `copilar.sh` (não versionados à mão):

```
├── ini/inimin          # Binário do inimin.asm (1024 bytes = 2 setores)
├── ini/inicio.o        # Objeto ELF gerado a partir de inicio.c
├── ini/inicio          # Binário plano do inicio.c (carregado em 0x10000)
├── nucleo/nucos        # Binário plano do nucleo.rs (carregado em 0x20000)
└── CosmOS.img          # Imagem de 20 MB, ext4, pronta para bootar no QEMU
```

---

## Requisitos e instalação

Em Debian/Ubuntu (testado no Debian 13 “trixie”):

```bash
sudo apt update && sudo apt install -y nasm gcc binutils qemu-system-x86 e2fsprogs

# Rust (rustup) + alvo de 32 bits usado pelo núcleo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
rustup target add i686-unknown-linux-gnu
```

| Ferramenta | Pacote | Para que serve |
|---|---|---|
| `nasm` | `nasm` | Montar o `ini/inimin.asm` |
| `gcc` | `gcc` | Compilar o `ini/inicio.c` em modo freestanding (`-m32`) |
| `ld`, `objcopy`, `nm` | `binutils` | Linkar os binários planos e conferir os endereços |
| `rustc`, `rustup` | [rustup.rs](https://rustup.rs) | Compilar o `nucleo/nucleo.rs` (`#![no_std]`) |
| `qemu-system-x86_64` | `qemu-system-x86` | Executar a `CosmOS.img` |
| `mkfs.ext4` / `debugfs` / `e2fsck` | `e2fsprogs` | Criar, popular e verificar a imagem ext4 (sem root) |

> **Por que `i686-unknown-linux-gnu` e não um alvo “none”?** O rustc 1.99 não
> oferece mais nenhum alvo bare-metal x86 de 32 bits. Como o núcleo é
> `#![no_std]` (nunca toca libc) e o `copilar.sh` linka manualmente com
> `ld -m elf_i386` usando `relocation-model=static`, o “linux” do nome não muda
> nada: só o `core` é usado.

No Arch Linux:

```bash
sudo pacman -S nasm gcc binutils qemu-system-x86 e2fsprogs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
rustup target add i686-unknown-linux-gnu
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
CosmOS Beta v0.1 Construção 0.3.2026
```

**2 segundos depois**, o segundo estágio passa o controle para o núcleo, que
**limpa a tela** e desenha na **primeira linha, à esquerda**:

```
Entrou no Nucos
```

---

## Fluxo de boot

```
BIOS
 └─> setor 0 (0x7C00)        inimin — stub de 512 bytes
      └─> setor 1 (0x7E00)   inimin — leitor ext4
           └─> /inicio/inicio (ext4)
                └─> 0x10000   início do modo protegido de 32 bits
                     └─> inicio.c desenha a mensagem
                          └─> ATA PIO: acha /nucleo/nucos no ext4
                               └─> espera 2 s (PIT por polling)
                                    └─> 0x20000   núcleo (nucos)
                                         └─> nucleo.rs limpa a tela e
                                              escreve "Entrou no Nucos"
```

### Mapa de memória (modo protegido)

| Endereço | O que vive lá |
|---|---|
| `0x6000` | Buffer de 1 bloco ext4 usado pelo `inicio` |
| `0x10000` | `ini/inicio` (2º estágio, até 61440 bytes) |
| `0x20000` | `nucleo/nucos` (núcleo, até 256 KiB) |
| `0x90000` | Topo da pilha (definida pelo `inimin`; o `inicio` a zera antes do salto) |
| `0xA0000` | Memória de vídeo do modo 13h (320×200×8) |

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
5. lê o disco com **ATA PIO** (a int 0x13 do BIOS não existe em modo protegido);
6. percorre o ext4 procurando **`/nucleo/nucos`** e o carrega em **`0x20000`**;
7. espera **2 segundos** medindo o **PIT canal 0 por polling** (sem interrupções:
   a CPU está com `cli` desde o inimin, não existe IDT);
8. zera a pilha em `0x90000` e salta para o núcleo.

A mensagem é tratada como UTF-8 para desenhar corretamente **`ç`** e **`ã`**.
O build usa `-fno-toplevel-reorder` para que `_start` seja o primeiro código do
binário; o `copilar.sh` confere com `nm` que ele está em `0x00010000`.

### Fase 3 — `nucleo/nucleo.rs` (núcleo)

| Item | Valor |
|---|---|
| Arquivo | `nucleo/nucleo.rs` |
| Binário | `nucleo/nucos` (binário plano, sem ELF) |
| Endereço de carga | **0x20000** (o `_start` fica exatamente no primeiro byte) |
| Arquitetura | **x86 32 bits, modo protegido** |
| Biblioteca | nenhuma — `#![no_std]`, sem libc |
| Compilação | `rustc --target i686-unknown-linux-gnu -C panic=abort --emit=obj` |
| Linkagem | `ld -m elf_i386 -T nucleo/nucleo.ld --nmagic` + `objcopy -O binary` |
| Vídeo | **Modo 13h: 320×200, 256 cores** |

O que ele faz:

1. **limpa a tela** inteira (64000 bytes de `0xA0000`);
2. desenha **"Entrou no Nucos"** na **primeira linha, à esquerda** (`x = 0`, `y = 2`),
   com a mesma fonte 8×8 do `inicio` (glifos `E`, `N` e `c` desenhados à mão);
3. entra em espera infinita (`hlt`).

Detalhe importante do `nucleo/nucleo.ld`: o rustc marca o código de pânico como
frio (`.text.unlikely`) e o script padrão do ld colocaria **essa seção primeiro**,
fazendo o salto para `0x20000` cair no pânico. Por isso o `_start` usa
`#[link_section = ".text.startup"]` e o linker script garante essa seção **antes**
de `.text.unlikely` — o primeiro byte do arquivo é sempre o `_start`
(o `copilar.sh` confere com `nm` que ele está em `0x00020000`).

Hoje o núcleo é um arquivo só; no futuro outros `.rs` entram junto, com o
`nucleo.ld` como base para a linkagem.

---

## Como o `copilar.sh` funciona

1. **Monta** `ini/inimin.asm` → `ini/inimin` e verifica que tem exatamente
   **1024 bytes**;
2. **Compila** `ini/inicio.c` com `gcc -m32` freestanding em `ini/inicio.o`,
   linka em `0x10000` (`ld -e _start`), converte para binário plano com
   `objcopy` e confere a entrada `_start` com `nm`;
3. **Compila** `nucleo/nucleo.rs` com `rustc` (`#![no_std]`, `panic=abort`,
   `relocation-model=static`) em um objeto, linka com o `nucleo.ld` em
   `0x20000`, converte para binário plano e confere o `_start` com `nm`;
4. **Cria** a `CosmOS.img` vazia com exatamente **20 MB** (`dd`);
5. **Formata** em **ext4** com rótulo `CosmOS`, blocos de **1 KiB** e inodes de
   **256 bytes** (`mkfs.ext4 -b 1024 -I 256`);
6. **Popula a imagem sem precisar de root**, usando `debugfs`:
   - cria somente as pastas `inicio/` e `nucleo/`;
   - copia `inimin` e `inicio` para `inicio/`;
   - copia `nucos` para `nucleo/`;
   - apaga o `lost+found` que o ext4 cria sozinho;
   - confere se os três arquivos estão lá;
7. **Grava os 2 setores de boot** (`ini/inimin`) no início da imagem com
   `dd conv=notrunc` — é o bloco de boot reservado do ext4;
8. **Verifica** a imagem com `e2fsck -f -n`.

Resultado final da `CosmOS.img`:

```
Bytes 0-511    : setor 0 do inimin (stub)
Bytes 512-1023 : setor 1 do inimin (leitor ext4)
Byte 1024      : superbloco ext4 (íntegro — verificado com e2fsck)
Raiz (ext4)    : /inicio/inimin   (1º estágio)
                 /inicio/inicio   (2º estágio)
                 /nucleo/nucos    (núcleo em Rust)
```

---

## Verificação realizada

Testado de ponta a ponta no **QEMU 10.0.13** (com `screendump` + análise dos
pixels das capturas):

- `ini/inimin` com **1024 bytes** e assinatura `55 AA` no offset 510;
- `ini/inicio` com entrada `_start` em **0x10000**;
- `nucleo/nucos` com entrada `_start` em **0x20000** (primeiro byte do arquivo);
- `CosmOS.img` com **20971520 bytes** (20 MB) e `e2fsck` sem erros;
- raiz da imagem contendo **apenas** `/inicio` e `/nucleo`, com `inimin`,
  `inicio` e `nucos` como arquivos separados;
- boot real no QEMU: CPU em **modo protegido** (`CPL=0`), sem nenhuma
  instrução SSE/AVX no núcleo (não precisa mexer no CR4);
- **t ≈ 1 s:** mensagem **"CosmOS Beta v0.1 Construção 0.3.2026"** centralizada,
  ocupando `x = 16..300` e `y = 2..9` (primeira linha);
- **t ≈ 4 s:** tela **limpa**, mensagem **"Entrou no Nucos"** na primeira linha
  à esquerda, ocupando `x = 0..116` e `y = 2..8` — o controle chegou ao núcleo.

Bugs encontrados e corrigidos durante a Construção 0.3.2026:

1. `u8 i; ... i < 256` em `carrega_inode` virava **loop infinito** (o `u8` dá a
   volta antes de chegar a 256) — corrigido para `u16`;
2. o `_start` do núcleo saía em `0x20062` porque `.text.unlikely` (pânico)
   vinha antes — corrigido com `nucleo.ld` + `#[link_section]`.
