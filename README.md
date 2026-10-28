# CosmOS Beta v0.1 — Construção 0.8.2026

Sistema operacional construído do zero, começando pelo zero absoluto: o **primeiro
estágio de boot** (Assembly/NASM), o **segundo estágio** (C freestanding), o
**núcleo** (Rust `no_std`) e a **interface gráfica** escrita em **Java bare-metal**
e compilada para **binário x86-32 puro** por um compilador próprio em Python —
tudo testado no emulador QEMU.

> **Regra do projeto:** nenhum arquivo ou pasta é criado ou alterado sem permissão
> do desenvolvedor. Os únicos artefatos gerados automaticamente são os produtos do
> próprio `copilar.sh` (`ini/inimin`, os `.o` (`ini/inicio.o`, `servicos/servicos.o`
> e os dez do núcleo), `ini/inicio`, `servicos/usr`, `nucleo/nucos`,
> `interface/grain` e `CosmOS.img`).

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
│   ├── nucleo.rs       # Núcleo/maestro — coordena os módulos — FONTE
│   ├── texto.rs        # Driver de texto (fonte 8x8) — FONTE
│   ├── cpu/
│   │   └── idt.rs      # IDT — interrupções de hardware (timer IRQ0) — FONTE
│   ├── arquivos/
│   │   └── ext4.rs     # Sistema de arquivos (ext4) — acha /servicos/usr — FONTE
│   ├── memoria/
│   │   └── memoria.rs  # Gerenciador de memória RAM (heap) — FONTE
│   └── drivers/
│       ├── tempo/
│       │   ├── pit.rs   # Driver de tempo (PIT canal 0) — mudo — FONTE
│       │   └── rtc.rs   # Relógio de tempo real (RTC) — data/hora na tela — FONTE
│       ├── mouse/
│       │   └── ps2.rs   # Driver de mouse PS/2 — MUDO (não desenha) — FONTE
│       ├── disco/
│       │   └── ata.rs   # Driver de disco ATA PIO (leitura de setores) — FONTE
│       └── video/
│           └── video.rs # Driver de vídeo (modo 13h) — FONTE
│   └── nucleo.ld       # Linker script: _start é o primeiro byte em 0x20000
├── servicos/
│   └── servicos.cpp    # Serviço "usr" — carrega e salta para a interface — FONTE
├── interface/
│   ├── papel_de_parede.java  # Interface "grain" (Java bare-metal) — FONTE
│   └── compilador.py         # Compilador Java → binário x86-32 — FONTE
├── copilar.sh          # Compila os estágios e gera a CosmOS.img
└── README.md           # Este arquivo
```

Gerados pelo `copilar.sh` (não versionados à mão):

```
├── ini/inimin                    # Binário do inimin.asm (1024 bytes = 2 setores)
├── ini/inicio.o                  # Objeto ELF gerado a partir de inicio.c
├── ini/inicio                    # Binário plano do inicio.c (carregado em 0x10000)
├── servicos/servicos.o           # Objeto ELF gerado a partir de servicos.cpp
├── servicos/usr                  # Binário plano do servicos.cpp (carregado em 0x60000)
├── interface/grain               # Binário x86-32 puro da interface (carregado em 0x80000)
├── nucleo/nucleo.o               # Objeto ELF do nucleo.rs (maestro)
├── nucleo/texto.o                # Objeto ELF do texto.rs
├── nucleo/memoria/memoria.o      # Objeto ELF do memoria.rs
├── nucleo/drivers/video/video.o  # Objeto ELF do video.rs
├── nucleo/drivers/tempo/pit.o    # Objeto ELF do pit.rs
├── nucleo/drivers/tempo/rtc.o    # Objeto ELF do rtc.rs
├── nucleo/drivers/mouse/ps2.o    # Objeto ELF do ps2.rs
├── nucleo/drivers/disco/ata.o    # Objeto ELF do ata.rs
├── nucleo/arquivos/ext4.o        # Objeto ELF do ext4.rs
├── nucleo/cpu/idt.o              # Objeto ELF do idt.rs
├── nucleo/nucos                  # Binário plano dos dez .o linkados (0x20000)
└── CosmOS.img                    # Imagem de 20 MB, ext4, pronta para bootar no QEMU
```

---

## Requisitos e instalação

Em Debian/Ubuntu (testado no Debian 13 “trixie”):

```bash
sudo apt update && sudo apt install -y nasm gcc binutils qemu-system-x86 e2fsprogs python3

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
| `rustc`, `rustup` | [rustup.rs](https://rustup.rs) | Compilar `nucleo/nucleo.rs` e os drivers (`#![no_std]`) |
| `python3` | `python3` | Rodar o `interface/compilador.py` (Java → binário x86-32) |
| `qemu-system-x86_64` | `qemu-system-x86` | Executar a `CosmOS.img` |
| `mkfs.ext4` / `debugfs` / `e2fsck` | `e2fsprogs` | Criar, popular e verificar a imagem ext4 (sem root) |

> **Por que `i686-unknown-linux-gnu` e não um alvo “none”?** O rustc 1.99 não
> oferece mais nenhum alvo bare-metal x86 de 32 bits. Como o núcleo é
> `#![no_std]` (nunca toca libc) e o `copilar.sh` linka manualmente com
> `ld -m elf_i386` usando `relocation-model=static`, o “linux” do nome não muda
> nada: só o `core` é usado.

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
CosmOS Beta v0.1 Construção 0.8.2026
```

**2 segundos depois**, o segundo estágio passa o controle para o núcleo, que
**limpa a tela** e desenha três linhas à esquerda (`y = 2`, `y = 14` e `y = 26`):

```
Entrou no Nucos                ← 1ª linha (y =  2)
27/10/2026              10:40:45   ← 2ª linha (y = 14): data à esquerda,
                                   hora à direita — e a HORA se atualiza
                                   sozinha, sem ninguém tocar na tela
RAM DE 134217728 BYTES         ← 3ª linha (y = 26)
```

**5 segundos depois de entrar no núcleo**, o kernel **SAI**: ele procura
`/servicos/usr` no ext4 da imagem (ATA PIO + `ext4.rs`), carrega em `0x60000`,
desliga as interrupções e salta para ele. O `servicos/servicos.cpp` limpa a
tela, **prepara a área do grain** (zera `0x80000..0x88000`), procura
`/interface/grain` no ext4, carrega em `0x80000`, ajusta a pilha para `0x90000`
e salta. O `grain` — **65 bytes de código x86-32 puro**, gerado pelo compilador
Java em Python — pinta a tela inteira de **azul-céu claro** (índice 78 da
paleta: `127,191,255`) e dorme em `hlt`.

**Resultado final:** a tela fica inteira no tom **`#7FBFFF`** — a interface
gráfica assumiu o controle. A cadeia completa é
`inimin → inicio → nucos → usr → /interface/grain`.

Enquanto o núcleo está na tela, a **2ª linha é viva**: ele espera os 5 s
**dormindo com `hlt`** até a **interrupção do timer (IRQ0)** acordá-lo a cada
250 ms (`idt_esperar_ms`) e relendo o RTC. Só os caracteres que mudaram são
redesenhados — o resto da tela não pisca. A CPU não fica em *polling*.

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
                                         └─> nucleo.rs limpa a tela (preto)
                                              e pede ao texto.rs para
                                              escrever "Entrou no Nucos"
                                              (que desenha via video.rs)
                                              └─> rtc.rs desenha data (esq.)
                                                  e hora (dir.) na 2ª linha
                                                  └─> idt.rs monta a IDT,
                                                      carrega com LIDT e
                                                      dá STI (liga as
                                                      interrupções)
                                                      └─> espera 5 s (idt_esperar_ms — hlt até o IRQ0 do
                                                          timer) e relê o RTC — a hora corre sozinha
                                                          └─> ata.rs + ext4.rs: acha /servicos/usr no ext4
                                                              └─> 0x60000 usr (servicos): limpa a tela,
                                                                  prepara 0x80000..0x88000, acha
                                                                  /interface/grain no ext4
                                                                  └─> 0x80000 grain: pinta a tela
                                                                      de azul-céu claro (127,191,255)
                                                                      e dorme em hlt
```

### Mapa de memória (modo protegido)

| Endereço | O que vive lá |
|---|---|
| `0x6000` | Buffer de 1 bloco ext4 usado pelo `inicio` |
| `0x10000` | `ini/inicio` (2º estágio, até 61440 bytes) |
| `0x20000` | `nucleo/nucos` (núcleo, até 256 KiB) |
| `0x60000` | `servicos/usr` (serviço usr, até 128 KiB — o núcleo salta para cá) |
| `0x80000` | `interface/grain` (interface gráfica, até 32 KiB — o usr salta para cá) |
| `0x88000` | Fim da área reservada ao grain |
| `0x90000` | Topo da pilha (definida pelo `inimin`; o usr a reajusta antes de saltar para o grain) |
| `0xA0000` | Memória de vídeo do modo 13h (320×200×8) |
| `0x100000` | Início do heap gerenciado pelo `memoria.rs` (1 MiB) |

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

### Fase 3 — `nucleo/nucleo.rs` (maestro) + módulos

| Item | Valor |
|---|---|
| Arquivos | `nucleo/nucleo.rs` (maestro), `nucleo/drivers/video/video.rs` (vídeo), `nucleo/drivers/tempo/pit.rs` (tempo), `nucleo/drivers/tempo/rtc.rs` (relógio), `nucleo/drivers/mouse/ps2.rs` (mouse), `nucleo/drivers/disco/ata.rs` (disco), `nucleo/arquivos/ext4.rs` (ext4), `nucleo/cpu/idt.rs` (interrupções), `nucleo/texto.rs` (texto) e `nucleo/memoria/memoria.rs` (RAM) |
| Binário | `nucleo/nucos` (binário plano, sem ELF) |
| Endereço de carga | **0x20000** (o `_start` fica exatamente no primeiro byte) |
| Arquitetura | **x86 32 bits, modo protegido** |
| Biblioteca | nenhuma — `#![no_std]`, sem libc |
| Compilação | `rustc --target i686-unknown-linux-gnu -C panic=abort -C opt-level=s -C target-feature=-sse,-sse2 --emit=obj` para cada `.rs` |
| Linkagem | `ld -m elf_i386 -T nucleo/nucleo.ld --nmagic` (nucleo.o + video.o + pit.o + rtc.o + ps2.o + ata.o + ext4.o + idt.o + texto.o + memoria.o) + `objcopy -O binary` |
| Vídeo | **Modo 13h: 320×200, 256 cores** |

O `nucleo.rs` é só o **maestro**: não desenha nem administra memória, apenas
chama os módulos na ordem certa.

- `video.rs` exporta `video_limpar(cor)` e `video_pixel(x, y, cor)`;
- `pit.rs` exporta `pit_iniciar()`, `pit_esperar_ms(ms)`, `pit_esperar_ticks(ticks)`
  e `pit_ler_contador()` — programa o PIT canal 0 (portas `0x43`/`0x40`, base de
  1193182 Hz) e espera por polling; é **mudo**, não escreve nada na tela;
- `rtc.rs` exporta `rtc_iniciar()` e `rtc_atualizar()` — lê o RTC do CMOS
  (portas `0x70`/`0x71`, esperando o bit **UIP** do status A e convertendo
  BCD/12 h conforme o status B) e desenha **data** (`DD/MM/AAAA`, encostada
  na esquerda) e **hora** (`HH:MM:SS`, encostada na direita) na segunda linha;
  redesenha **apenas os caracteres que mudaram**, apagando a célula de 8×8
  antes (sem rabiscos e sem piscar o resto da tela);
- `ps2.rs` exporta `mouse_ps2_iniciar()`, `mouse_ps2_processar_tudo()` e
  `mouse_ps2_ler_movimento()` — inicializa o mouse PS/2 (controlador e
  dispositivo) e monta os pacotes de 3 bytes vindos do IRQ12; é **mudo**:
  não escreve nada na tela (o cursor só aparecer quando você pedir);
- `idt.rs` exporta `idt_iniciar()`, `idt_ticks()` e `idt_esperar_ms(ms)` —
  monta a **IDT** (256 entradas) com portas de 8 bytes, preenche os vetores
  de hardware do **PIC não remapeado** (8..15 e 0x70..0x77) com stubs que
  enviam **EOI** e voltam com `iretd`, carrega a tabela com **LIDT** e liga
  as interrupções com **STI**. O **IRQ0 (timer, vetor 8)** incrementa o
  contador `TICKS` (100 Hz) e o **IRQ12 (mouse, vetor 0x74)** só manda EOI
  nos dois PICs. `idt_esperar_ms()` conta os ticks e dorme com `hlt` até
  o próximo IRQ0 — é assim que o laço do relógio espera sem *polling*;
- `ata.rs` exporta `ata_ler_setores(lba, n, destino)` — lê setores do
  **disco primário mestre** (portas `0x1F0`/`0x1F7`) por PIO, com o mesmo
  protocolo do `ini/inicio.c` (espera BSY, manda o LBA e polimenta DRQ);
  é o caminho do núcleo para o ext4 (a int 0x13 do BIOS não existe aqui);
- `ext4.rs` exporta `ext4_carregar(dir, dir_tam, arq, arq_tam, destino, max)`
  — desce do **inode raiz** (2) pelo descritor do grupo 0, varre os
  diretórios (árvore de extensões, profundidade 0) e lê o arquivo pelo
  `ata.rs` em blocos de 1 KiB; é assim que o núcleo acha
  **`/servicos/usr`** para sair;
- `texto.rs` exporta `texto_escrever(texto, tamanho, x, y)` e
  `texto_escrever_numero(valor, x, y)`, guarda a fonte 8×8 (dígitos, letras e
  minúsculas do alfabeto usado) e desenha cada caractere chamando `video_pixel`;
- `memoria.rs` exporta `memoria_iniciar()`, `memoria_alocar(tamanho, alinhamento)`,
  `memoria_resetar()`, `memoria_total()`, `memoria_usada()` e `memoria_livre()`.

O que o maestro faz:

1. **inicia o vídeo**: limpa a tela inteira em preto (`video_limpar(0)`);
2. **inicia a memória**: chama `memoria_iniciar()`, que lê o mapa **E820**
   deixado pelo primeiro estágio e monta um heap linear a partir de **1 MiB**;
3. **inicia o tempo**: chama `pit_iniciar()`, que deixa o canal 0 do PIT em
   modo periódico a **100 Hz** — sem desenhar ou imprimir nada na tela;
4. manda escrever **"Entrou no Nucos"** (`x = 0`, `y = 2`) e, na terceira linha
   (`y = 26`), **`RAM DE <bytes> BYTES`** com o tamanho real da RAM instalada,
   usando `texto_escrever_numero()`;
5. **inicia o relógio**: chama `rtc_iniciar()`, que faz a primeira leitura do
   RTC e desenha **data à esquerda** e **hora à direita** na segunda linha
   (`y = 14`);
6. **inicializa o mouse PS/2** (`mouse_ps2_iniciar()`) — mudo, não desenha
   nada;
7. **inicializa a IDT** (`idt_iniciar()`): monta a tabela, carrega com `LIDT`
   e dá `STI` — a partir daqui o **IRQ0 do timer** passa a contar os ticks;
8. **espera 5 segundos mantendo o relógio vivo**: dorme em **fatias de
   250 ms** (`idt_esperar_ms(INTERVALO_RELOGIO_MS)`) e relê o RTC
   (`rtc_atualizar()`) a cada fatia — a espera é **por interrupção**
   (dorme em `hlt` até o IRQ0 acordar) e os **segundos continuam
   correndo** na 2ª linha durante toda a espera;
9. **SAI do kernel**: chama `ext4_carregar("servicos", "usr", 0x60000, ...)`,
   que usa `ata.rs` para ler o disco e achar **`/servicos/usr`** no ext4;
   com o arquivo em `0x60000`, dá **`CLI`**, zera a pilha em `0x90000` e
   **salta** para o serviço. Se o arquivo não for achado, o núcleo cai no
   **laço do relógio** (as três linhas ficam na tela — é o diagnóstico da
   falha).

O laço do relógio (`laço_do_relogio`) também continua disponível: é o
"modo de segurança" do núcleo, onde `idt_esperar_ms(250)` + `rtc_atualizar()`
mantêm a hora viva sem *polling*.

#### Como o `memoria.rs` gerencia a RAM

- O primeiro estágio (`ini/inimin.asm`) chama a BIOS (`int 0x15`, `E820`) antes
  de carregar o setor 1 e grava o mapa em `0x5000` (24 bytes por entrada), com a
  contagem em `0x4FF0` e a assinatura `0xE820` em `0x4FF2`.
- O módulo percorre as entradas de **tipo 1** (memória utilizável), pega o maior
  fim e arredonda para o próximo MiB — é assim que a tela mostra o tamanho real
  (ex.: `-m 128M` → `RAM DE 134217728 BYTES`). Se o mapa não existir, cai para a
  CMOS (`0x30`/`0x31`).
- O heap começa em `0x100000` (1 MiB) e termina no fim da região utilizável que
  contém o heap, menos uma reserva de 2 MiB para tabelas do firmware.
- O alocador é do tipo **bump**: `memoria_alocar()` avança um ponteiro,
  respeita o alinhamento pedido e devolve memória zerada. Ainda não há
  liberação individual; `memoria_resetar()` devolve o heap inteiro.
- `memoria_total()`, `memoria_usada()` e `memoria_livre()` expõem as contas
  para os próximos módulos.

Detalhe importante do `nucleo/nucleo.ld`: o rustc marca o código de pânico como
frio (`.text.unlikely`) e o script padrão do ld colocaria **essa seção primeiro**,
fazendo o salto para `0x20000` cair no pânico. Por isso o `_start` usa
`#[link_section = ".text.startup"]` e o linker script garante essa seção **antes**
de `.text.unlikely` — o primeiro byte do arquivo é sempre o `_start`
(o `copilar.sh` confere com `nm` que ele está em `0x00020000`).

Cada módulo novo entra como um `.rs` separado (compilado em seu próprio `.o`) e é
somado na linha do `ld`; o `nucleo.ld` continua sendo a base da linkagem.

### Fase 4 — `servicos/servicos.cpp` (o serviço usr)

| Item | Valor |
|---|---|
| Arquivo | `servicos/servicos.cpp` |
| Binário | `servicos/usr` (binário plano, sem ELF) |
| Endereço de carga | **0x60000** (logo depois dos 256 KiB do núcleo; a entrada `_start` fica exatamente aí) |
| Arquitetura | **x86 32 bits, modo protegido** |
| Biblioteca | nenhuma — *freestanding*, compilado com `g++ -m32` sem exceções/RTTI |
| Vídeo | **Modo 13h: 320×200, 256 cores** (o modo continua ligado do boot) |

Este é o **primeiro serviço do sistema** e a **saída do kernel**:

1. o `nucleo.rs` espera **5 segundos**, lê **`/servicos/usr`** do ext4 e salta
   para `0x60000` com as interrupções desligadas (`CLI`) e a pilha zerada em
   `0x90000`;
2. o `_start` do usr **limpa a tela inteira** em preto (`0xA0000`), no mesmo
   modo 13h que o boot deixou ligado;
3. **prepara o terreno**: zera `0x80000..0x88000` (32 KiB reservados ao grain);
4. lê **`/interface/grain`** do ext4 (ATA PIO + `ext4.rs`) e carrega em
   `0x80000`;
5. ajusta a pilha para `0x90000` e **salta para o grain** — a interface
   gráfica assume o controle.

O `copilar.sh` linka o usr em `0x60000` (`ld -Ttext=0x60000 -e _start`) e
confere com `nm` que o `_start` está em `0x00060000`, do mesmo jeito que faz
com o `inicio` (0x10000) e o `nucos` (0x20000).

### Fase 5 — `interface/papel_de_parede.java` (a interface "grain")

| Item | Valor |
|---|---|
| Fonte | `interface/papel_de_parede.java` (Java **bare-metal**, dialeto CosmOS) |
| Compilador | `interface/compilador.py` (Python — gera código de máquina direto) |
| Binário | `interface/grain` (binário plano x86-32, sem ELF, **sem C e sem VM**) |
| Endereço de carga | **0x80000** (o usr salta para cá) |
| Arquitetura | **x86 32 bits, modo protegido** |
| Vídeo | **Modo 13h: 320×200, 256 cores** |

O `compilador.py` é um compilador de um subconjunto de Java que **emite os
bytes x86-32 diretamente** (lexer + parser + codificador de instruções), sem
passar por C, sem `gcc` e sem máquina virtual. Ele suporta variáveis, laços,
condicionais, comparações, chamadas e dois intrínsecos:

| Intrínseco | Vira | Efeito |
|---|---|---|
| `memoria_preencher(destino, tamanho, valor)` | `REP STOSB` | preenche memória |
| `cpu_halt()` | `HLT` | dorme a CPU |

O programa atual preenche os **64000 bytes** da memória de vídeo (`0xA0000`)
com o índice **78** da paleta (`127,191,255`, azul-céu claro) e entra em
`hlt` para sempre. O `grain` resultante tem apenas **65 bytes**.

---

## Como o `copilar.sh` funciona

1. **Monta** `ini/inimin.asm` → `ini/inimin` e verifica que tem exatamente
   **1024 bytes**;
2. **Compila** `ini/inicio.c` com `gcc -m32` freestanding em `ini/inicio.o`,
   linka em `0x10000` (`ld -e _start`), converte para binário plano com
   `objcopy` e confere a entrada `_start` com `nm`;
3. **Compila** `servicos/servicos.cpp` com `g++ -m32` freestanding em
   `servicos/servicos.o`, linka em `0x60000` (`ld -e _start`), converte para
   binário plano (`servicos/usr`) e confere a entrada `_start` com `nm`;
3b. **Compila** `interface/papel_de_parede.java` com o `interface/compilador.py`
    (Python), que gera **diretamente** o binário x86-32 `interface/grain`
    (sem C, sem `gcc`, sem VM) e confere que ele começa com `CALL` (`0xE8`) e
    cabe em 32 KiB;
4. **Compila** `nucleo/nucleo.rs` e os módulos `nucleo/drivers/video/video.rs`,
   `nucleo/drivers/tempo/pit.rs`, `nucleo/drivers/tempo/rtc.rs`,
   `nucleo/drivers/mouse/ps2.rs`, `nucleo/drivers/disco/ata.rs`,
   `nucleo/arquivos/ext4.rs`, `nucleo/cpu/idt.rs`,
   `nucleo/texto.rs` e `nucleo/memoria/memoria.rs` com `rustc` (`#![no_std]`,
   `panic=abort`, `opt-level=s`, `relocation-model=static`) em objetos
   separados — cada `.o` fica salvo ao lado da sua fonte, igual ao
   `ini/inicio.o` —, linka os **dez** com o `nucleo.ld` em `0x20000`,
   converte para binário plano e confere o `_start` com `nm`;
5. **Cria** a `CosmOS.img` vazia com exatamente **20 MB** (`dd`);
6. **Formata** em **ext4** com rótulo `CosmOS`, blocos de **1 KiB** e inodes de
   **256 bytes** (`mkfs.ext4 -b 1024 -I 256`);
7. **Popula a imagem sem precisar de root**, usando `debugfs`:
   - cria as pastas `inicio/`, `nucleo/`, `servicos/` e `interface/`;
   - copia `inimin` e `inicio` para `inicio/`;
   - copia `nucos` para `nucleo/`;
   - copia `usr` para `servicos/`;
   - copia `grain` para `interface/`;
   - apaga o `lost+found` que o ext4 cria sozinho;
   - confere se os **cinco** arquivos estão lá;
8. **Grava os 2 setores de boot** (`ini/inimin`) no início da imagem com
   `dd conv=notrunc` — é o bloco de boot reservado do ext4;
9. **Verifica** a imagem com `e2fsck -f -n`.

Resultado final da `CosmOS.img`:

```
Bytes 0-511    : setor 0 do inimin (stub)
Bytes 512-1023 : setor 1 do inimin (leitor ext4)
Byte 1024      : superbloco ext4 (íntegro — verificado com e2fsck)
Raiz (ext4)    : /inicio/inimin   (1º estágio)
                 /inicio/inicio   (2º estágio)
                 /nucleo/nucos    (núcleo em Rust)
                 /servicos/usr    (serviço usr — a saída do kernel)
                 /interface/grain (interface gráfica — binário x86-32 puro)
```

---

## Verificação realizada

Testado de ponta a ponta no **QEMU 10.0.13** (com `screendump` + análise dos
pixels das capturas):

- `ini/inimin` com **1024 bytes** e assinatura `55 AA` no offset 510;
- `ini/inicio` com entrada `_start` em **0x10000**;
- `servicos/usr` com entrada `_start` em **0x60000**;
- `nucleo/nucos` com entrada `_start` em **0x20000** (primeiro byte do arquivo);
- `CosmOS.img` com **20971520 bytes** (20 MB) e `e2fsck` sem erros;
- raiz da imagem contendo **apenas** `/inicio`, `/nucleo`, `/servicos` e
  `/interface`, com `inimin`, `inicio`, `nucos`, `usr` e `grain` como arquivos
  separados (nenhum `.py`, `.java`, `.c` ou `.o` vai para a imagem);
- `interface/grain` com **65 bytes** de código x86-32, começando em `0x80000`
  com a instrução `CALL` (`0xE8`) para `main` — conferido com `objdump`;
- boot real no QEMU: CPU em **modo protegido** (`CPL=0`), sem nenhuma
  instrução SSE/AVX no núcleo. O `copilar.sh` compila os módulos com
  `-C target-feature=-sse,-sse2` justamente para garantir isso (um `xorps`/
  `movaps` gerado pelo `rustc` derrubava a CPU em *triple fault*);
- **t ≈ 1 s:** mensagem **"CosmOS Beta v0.1 Construção 0.8.2026"** centralizada,
  ocupando `x = 16..300` e `y = 2..9` (primeira linha);
- **t ≈ 4 s:** tela **limpa**, mensagem **"Entrou no Nucos"** na primeira linha
  à esquerda, ocupando `x = 0..116` e `y = 2..8` — o controle chegou ao núcleo;
- **memória:** a terceira linha (`y = 26`) mostra o tamanho **real** da RAM em
  bytes, lido do mapa E820: com `-m 64M` → **`RAM DE 67108864 BYTES`**,
  com `-m 128M` → **`RAM DE 134217728 BYTES`** e com `-m 256M` →
  **`RAM DE 268435456 BYTES`**;
- **relógio:** a segunda linha (`y = 14`) mostra a **data à esquerda**
  (`DD/MM/AAAA`) e a **hora à direita** (`HH:MM:SS`) lidas do RTC — conferido
  no QEMU com dois `screendump`s: os segundos avançam sozinhos e **só os
  caracteres que mudam** são redesenhados (o restante da tela fica intacto,
  byte a byte). O horário bate com o UTC do host.
- **interrupções (IDT):** o boot **não reinicia mais** (fica estável no QEMU
  por tempo indeterminado) e a espera do relógio passou a ser **por IRQ0**:
  dois `screendump`s com **4 s** de diferença mostram **348 pixeis mudando**
  com a tela mantendo seus ~7944 pixeis desenhados — a hora avança sozinha
  enquanto a CPU dorme em `hlt`. Conferido também com `qemu -d int,cpu_reset`:
  nenhum *triple fault*, os vetores de hardware todos respondem com EOI.
- **saída do kernel e interface:**
  - **t ≈ 4 s:** as **três linhas do núcleo** (2692 pixéis acesos em
    `y = 4..65` da captura 640×400 — boas-vindas, data/hora e RAM);
  - **t ≈ 11 s:** a tela inteira fica **uniforme** em **`#7FBFFF`**
    (`127,191,255` — índice **78** da paleta do modo 13h), sem nenhum pixel
    de outra cor. Os 5 s passaram, o `/servicos/usr` foi achado no ext4,
    carregado em `0x60000`, preparou `0x80000..0x88000`, carregou
    `/interface/grain` em `0x80000` e saltou; os **65 bytes** de código
    x86-32 do grain pintaram os **64000 bytes** da memória de vídeo e
    entraram em `hlt`. A cadeia completa é
    `inimin → inicio → nucos → usr → /interface/grain`;
  - trocando só o `interface/papel_de_parede.java` de `COR_AZUL_CLARO = 9`
    para **`78`** e recompilando, a captura passou de `(87,87,255)` (azulado
    arroxeado) para o azul-céu claro `(127,191,255)` — confirmado por
    amostragem de todos os pixels do `screendump`.

Bugs encontrados e corrigidos durante a Construção 0.8.2026:

1. `u8 i; ... i < 256` em `carrega_inode` virava **loop infinito** (o `u8` dá a
   volta antes de chegar a 256) — corrigido para `u16`;
2. o `_start` do núcleo saía em `0x20062` porque `.text.unlikely` (pânico)
   vinha antes — corrigido com `nucleo.ld` + `#[link_section]`;
3. o `rustc` gerava `xorps`/`movaps` ao zerar o buffer de dígitos de
   `texto_escrever_numero`. Com `CR4.OSFXSR` desligado, a instrução SSE gera
   `#UD` e a CPU entrava em *triple fault* (a tela ficava no "Entrou no Nucos"
   e o QEMU reiniciava) — corrigido compilando o núcleo com
   `-C target-feature=-sse,-sse2`;
4. `anterior[i]` no `rtc.rs` fazia o `ld` falhar com *undefined reference to
   `core::panicking::panic_bounds_check`* — o indexamento normal do Rust gera
   checagem de limites, e o runtime de pânico não existe na linkagem plana do
   nucos — corrigido com `get_unchecked()`;
5. o **status B** do RTC foi lido com os bits errados (usei bit 0 como BCD e
   bit 2 como 24 h) e a tela mostrava **`00/00/2038 00:56`**: na MC146818 o
   bit 2 é o **DM** (1 = binário, 0 = BCD) e o bit 1 é o **24 h/12 h** — com
   BCD interpretado como binário, `27` → 39 estourava o limite e virava `00`
   e o ano `0x26` virava `38` ("2038"). Corrigido lendo os bits certos — a
   data passou a mostrar `27/10/2026` com a hora certa do host;
6. o `lidt` estava com `options(nomem, nostack)` no `asm!` — **mentindo ao
   compilador** ("essa instrução não lê memória"), que otimizava **todo o
   preenchimento da IDT como código morto**: a CPU caía sem tabela de
   interrupções e o sistema reiniciava em loop. Corrigido tirando o `nomem`;
7. os stubs usavam `iret`, que esta ferramenta **monta como `iretw` (16
   bits)** — ao voltar de uma interrupção a pilha era corrompida. Trocado
   por **`iretd`** (32 bits);
8. com o **PIC não remapeado**, os IRQs caem nos vetores **8..15** e
   **0x70..0x77**; só o timer e o mouse tinham porta, então o **IRQ1 do
   teclado** (vetor 9, que o BIOS deixa liberado) batia numa porta **vazia**
   → **#GP → #GP em cascata → triple fault** → boot loop. Corrigido
   preenchendo **todos** os vetores de hardware com um stub que só manda EOI
   e retorna (o EOI é obrigatório: sem ele o PIC fica mascarado e nenhuma
   interrupção nova chega).

9. o `preparar_terreno()` do `servicos.cpp` ajustava `esp` para `0x90000` e
   **depois fazia `ret`** — o `ret` lia o endereço de retorno da pilha nova
   (zerada em `0x80000`) e saltava para `0x00000000`, derrubando o sistema
   com **tela preta**. A troca de pilha só pode acontecer na função que
   **não retorna** (`saltar_grain`); corrigido removendo o `esp` do
   `preparar_terreno`;
10. a área reservada ao grain era de **128 KiB** (`0x80000..0xA0000`), mas a
    pilha fica em `0x90000` e cresce para baixo — o grain e a pilha se
    sobrepunham. Reduzida para **32 KiB** (`0x80000..0x88000`), com 32 KiB de
    folga antes da pilha; o `copilar.sh` recusa um `grain` maior que isso;
11. o `grain` aparecia com tom **arroxeado** (`(87,87,255)`) porque o índice
    **9** da paleta do modo 13h tem R = G. Trocado só no `.java` para o índice
    **78** (`(127,191,255)`, azul-céu claro), escolhido depois de varrer a
    paleta real do QEMU pintando uma faixa por índice.
