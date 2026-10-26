# CosmOS Beta v0.1 — Construção 0.1.2026

Sistema operacional construído do zero, começando pelo zero absoluto: o **primeiro
estágio de boot**. Tudo é escrito à mão em Assembly (NASM) e testado no emulador QEMU.

> **Regra do projeto:** nenhum arquivo ou pasta é criado ou alterado sem permissão
> do desenvolvedor. Os únicos artefatos gerados automaticamente são os produtos do
> próprio `copilar.sh` (`ini/inimin` e `CosmOS.img`).

---

## Estrutura do projeto

```
Desenvolvimento/
├── .vscode/
│   └── tasks.json      # Tarefa única: copilar.sh + QEMU em sequência
├── ini/
│   └── inimin.asm      # Primeiro estágio de boot (512 bytes) — FONTE
├── copilar.sh          # Compila o inimin.asm e gera a CosmOS.img
└── README.md           # Este arquivo
```

Gerados pelo `copilar.sh` (não versionados à mão):

```
├── ini/inimin          # Binário montado do inimin.asm (setor de boot, 512 bytes)
└── CosmOS.img          # Imagem de 20 MB, ext4, pronta para bootar no QEMU
```

---

## Requisitos e instalação

Em Debian/Ubuntu (testado no Debian 13 “trixie”):

```bash
sudo apt update && sudo apt install -y nasm qemu-system-x86 e2fsprogs
```

| Ferramenta | Pacote | Para que serve |
|---|---|---|
| `nasm` | `nasm` | Montar o `ini/inimin.asm` |
| `qemu-system-x86_64` | `qemu-system-x86` | Executar a `CosmOS.img` |
| `mkfs.ext4` / `debugfs` | `e2fsprogs` | Criar e popuar a imagem ext4 (sem root) |

No Arch Linux:

```bash
sudo pacman -S nasm qemu-system-x86 e2fsprogs
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
./copilar.sh      # compila e gera a CosmOS.img
qemu-system-x86_64 -drive format=raw,file=CosmOS.img,if=ide -boot order=c -m 128M
```

**Resultado:** o QEMU abre e a tela do CosmOS aparece **totalmente preta** em
320×200 (modo gráfico), sem nenhum texto — exatamente o comportamento da Fase 1.

---

## Fase 1 — `ini/inimin.asm` (primeiro estágio de boot)

| Item | Valor |
|---|---|
| Arquivo | `ini/inimin.asm` |
| Binário | `ini/inimin` |
| Tamanho | **exatamente 512 bytes** (1 setor), com assinatura `0xAA55` |
| Arquitetura alvo | **x86-64** — o BIOS entrega a CPU em modo real de 16 bits, é daí que todo boot x86-64 começa |
| Vídeo | **Modo 13h: 320×200, 256 cores (8 bits por pixel)** |
| Estado da tela | **Totalmente preta** (nada é desenhado nesta fase) |

O que ele faz, passo a passo:

1. Desliga interrupções (`cli`) e zera os segmentos (`DS`, `ES`, `SS`);
2. Monta a pilha em `0x7C00` (logo abaixo do próprio código);
3. Chama `int 0x10` com `AX = 0x0013` → troca para o modo gráfico 320×200×8
   (o bit 7 de `AL` fica em 0, então a tela é limpa e permanece preta);
4. Entra em espera infinita (`hlt` + `jmp`), sem desenhar absolutamente nada;
5. Preenche o resto com zeros até o byte 510 e grava a assinatura `0xAA55`
   (obrigatória para o BIOS aceitar o setor de boot).

---

## Como o `copilar.sh` funciona

1. **Monta** `ini/inimin.asm` → `ini/inimin` com `nasm -f bin` e **verifica**
   que o resultado tem exatamente 512 bytes (erro se tiver diferente);
2. **Cria** a `CosmOS.img` vazia com exatamente **20 MB** (`dd`);
3. **Formata** em **ext4** com o rótulo `CosmOS` (`mkfs.ext4`);
4. **Popula a imagem sem precisar de root**, usando `debugfs`:
   - cria somente a pasta `inicio/`;
   - copia o binário para `inicio/inimin`;
   - apaga o `lost+found` que o ext4 cria sozinho — assim a raiz da imagem
     contém **apenas** `inicio/`;
   - confere se `inicio/inimin` realmente está lá;
5. **Grava o setor de boot** (`ini/inimin`) no início da imagem com `dd
   conv=notrunc`. Os primeiros 1024 bytes de uma imagem ext4 são o *boot
   block* reservado (o superbloco começa no byte 1024), então **o sistema de
   arquivos não é corrompido**.

Resultado final da `CosmOS.img`:

```
Setor 0     : código de boot do inimin (512 bytes, termina em 55 AA)
Byte 1024   : superbloco ext4 (íntegro — verificado com e2fsck)
Raiz (ext4) : /inicio/inimin  ← e nada mais
```

---

## Verificação realizada

A Fase 1 foi testada de ponta a ponta no **QEMU 10.0.13** (sem instalar nada no
sistema, apenas para validação):

- `ini/inimin` montado com **512 bytes exatos** e assinatura `55 AA` no offset 510;
- `CosmOS.img` com **20971520 bytes** (20 MB) e `e2fsck` sem erros;
- raiz da imagem contendo **apenas** `/inicio` com o binário idêntico ao original;
- boot real no QEMU: memória de vídeo em `0xA0000` zerada (**modo gráfico**) e
  buffer de texto em `0xB8000` inativo (**nada de modo texto**) — tela 100% preta.
