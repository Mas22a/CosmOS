; ==========================================================================
;   CosmOS Beta v0.1 — Construção 0.8.2026
;   ini/inimin.asm — Primeiro estágio de boot do CosmOS
; --------------------------------------------------------------------------
;   Alvo   : x86-64 (o BIOS entrega a CPU em modo real de 16 bits)
;   Tamanho: 1024 bytes = 2 setores
;              setor 0 (0x7C00): carrega o setor 1 e salta para ele
;              setor 1 (0x7E00): lê o ext4, acha /inicio/inicio,
;                                carrega em 0x10000 e entra em modo
;                                protegido de 32 bits
;   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
;
;   Os dois setores ficam no bloco de boot reservado do ext4 (os primeiros
;   1024 bytes), portanto o sistema de arquivos não é corrompido.
;
;   Premissas do ext4 gerado pelo copilar.sh:
;     - blocos de 1024 bytes;
;     - inodes de 256 bytes;
;     - tabela de inodes localizada pelo descritor do grupo 0.
; ==========================================================================

[BITS 16]
[ORG 0x7C00]

%define BUF        0x6000       ; bloco ext4 de 1 KiB lido pelo BIOS
%define CARREGADOR 0x7E00       ; onde o setor 1 é carregado
%define DEST_SEG   0x1000       ; 0x1000:0x0000 = 0x10000 (2º estágio)
%define MAPA_E820  0x5000       ; mapa de memória do BIOS (24 bytes/entrada)
%define CONT_E820  0x4FF0       ; contagem de entradas do mapa
%define MAGIA_E820 0x4FF2       ; assinatura "mapa válido"

; ==========================================================================
; SETOR 0 — carrega o setor 1 para 0x7E00 e salta para ele
; ==========================================================================
setor0:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    cld
    sti

    ; --- mapa de RAM do BIOS (E820) ------------------------------------
    mov [drive_boot], dl
    call detectar_ram
    mov dl, [drive_boot]

    mov si, dap_boot
    mov ah, 0x42
    int 0x13
    jc erro_boot
    jmp 0x0000:CARREGADOR

erro_boot:
    cli
.parado:
    hlt
    jmp .parado

align 8
dap_boot:
    db 16, 0
    dw 1                        ; 1 setor
    dw CARREGADOR
    dw 0
    dq 1                        ; LBA 1

; --------------------------------------------------------------------------
; detectar_ram — int 0x15/E820 -> mapa em 0x5000 e contagem em 0x4FF0
; --------------------------------------------------------------------------
detectar_ram:
    xor ebx, ebx
    xor bp, bp
    mov di, MAPA_E820
.loop:
    mov eax, 0xE820
    mov edx, 0x534D4150         ; "SMAP"
    mov ecx, 24
    mov dword [di + 20], 1
    int 0x15
    jc .fim
    cmp eax, 0x534D4150
    jne .fim
    add di, 24
    inc bp
    cmp bp, 128                 ; não passa de 0x5C00 (BUF fica em 0x6000)
    jae .fim
    test ebx, ebx
    jnz .loop
.fim:
    mov [CONT_E820], bp
    test bp, bp
    jz .saida
    mov word [MAGIA_E820], 0xE820
.saida:
    ret

drive_boot: db 0

    times 510 - ($ - $$) db 0
    dw 0xAA55

; ==========================================================================
; SETOR 1 — segundo pedaço do primeiro estágio, em 0x7E00
; ==========================================================================
carregador:
    mov [drive], dl             ; drive de boot
    mov ax, 0x0013              ; modo 13h: 320x200x8, tela preta
    int 0x10

    ; --- descritor do grupo 0 (bloco 2) --------------------------------
    mov eax, 2
    call ler_buf
    mov eax, [BUF + 8]          ; bg_inode_table_lo
    mov [itblk], eax

    ; --- /inicio --------------------------------------------------------
    mov eax, 2                  ; inode raiz
    call carrega_inode
    call primeiro_ext
    mov eax, ebx
    call ler_buf
    call acha_inicio
    test eax, eax
    jz erro

    ; --- /inicio/inicio (diretório) ------------------------------------
    call carrega_inode
    call primeiro_ext
    mov eax, ebx
    call ler_buf
    call acha_inicio
    test eax, eax
    jz erro

    ; --- /inicio/inicio (arquivo) --------------------------------------
    call carrega_inode
    call carrega_arquivo
    jmp entra_pm

; --------------------------------------------------------------------------
; ler_buf — lê 1 bloco (EAX) para BUF
; ler_dest — lê 1 bloco (EAX) para 0x10000 + (DX - 0x1000) * 16
; --------------------------------------------------------------------------
ler_buf:
    shl eax, 1                  ; bloco de 1 KiB -> LBA de 512 bytes
    mov [dap + 8], eax
    mov word [dap + 4], BUF
    mov word [dap + 6], 0
    jmp le_dap

ler_dest:
    shl eax, 1
    mov [dap + 8], eax
    mov word [dap + 4], 0
    mov [dap + 6], dx

le_dap:
    mov si, dap
    mov ah, 0x42
    mov dl, [drive]
    int 0x13
    jc erro
    ret

; --------------------------------------------------------------------------
; carrega_inode — EAX (inode) -> SI (ponteiro dentro de BUF)
; --------------------------------------------------------------------------
carrega_inode:
    dec eax
    mov edx, eax
    shr eax, 2                  ; 4 inodes de 256 bytes por bloco
    add eax, [itblk]
    push edx
    call ler_buf
    pop edx
    and edx, 3
    shl dx, 8
    mov si, BUF
    add si, dx
    ret

; --------------------------------------------------------------------------
; primeiro_ext — SI (inode) -> EBX (1º bloco de dados)
; --------------------------------------------------------------------------
primeiro_ext:
    cmp word [si + 0x28], 0xF30A    ; magic da árvore de extensões
    jne erro
    cmp word [si + 0x2E], 0         ; profundidade 0 (folha)
    jne erro
    mov ebx, [si + 0x34 + 8]        ; ee_start_lo
    movzx eax, word [si + 0x34 + 6] ; ee_start_hi
    shl eax, 16
    or ebx, eax
    test ebx, ebx
    jz erro
    ret

; --------------------------------------------------------------------------
; acha_inicio — procura "inicio" no diretório em BUF -> EAX (0 = falha)
; --------------------------------------------------------------------------
acha_inicio:
    mov di, BUF
    mov cx, 1024
.loop:
    mov eax, [di]
    test eax, eax
    jz .prox
    cmp byte [di + 6], 6            ; name_len
    jne .prox
    cmp dword [di + 8], 0x63696E69  ; "inic"
    jne .prox
    cmp word [di + 12], 0x6F69      ; "io"
    jne .prox
    ret
.prox:
    movzx edx, word [di + 4]        ; rec_len
    test dx, dx
    jz .falha
    add di, dx
    sub cx, dx
    ja .loop
.falha:
    xor eax, eax
    ret

; --------------------------------------------------------------------------
; carrega_arquivo — SI (inode) -> lê todas as extensões para 0x10000
; --------------------------------------------------------------------------
carrega_arquivo:
    cmp word [si + 0x28], 0xF30A
    jne erro
    cmp word [si + 0x2E], 0
    jne erro
    mov cx, word [si + 0x2A]      ; número de extensões
    mov bx, si
    add bx, 0x34                    ; primeira extensão

.prox_ext:
    test cx, cx
    jz .fim
    mov eax, [bx]                   ; ee_block (lógico)
    shl eax, 6                      ; * 64 = 1024 bytes / 16
    add eax, DEST_SEG
    mov [dseg], ax
    mov eax, [bx + 8]               ; ee_start_lo
    movzx edx, word [bx + 6]        ; ee_start_hi
    shl edx, 16
    or eax, edx
    mov [curblk], eax
    movzx edx, word [bx + 4]        ; ee_len
    and edx, 0x7FFF
    mov [blkcnt], dx

.bloco:
    mov eax, [curblk]
    mov dx, [dseg]
    call ler_dest
    inc dword [curblk]
    add word [dseg], 0x40            ; próximo bloco de 1 KiB = 64 parágrafos
    dec word [blkcnt]
    jnz .bloco
    add bx, 12
    dec cx
    jmp .prox_ext
.fim:
    ret

; --------------------------------------------------------------------------
; entra_pm — modo protegido de 32 bits e salto para o segundo estágio
; --------------------------------------------------------------------------
entra_pm:
    cli
    lgdt [gdt_desc]
    mov eax, cr0
    or al, 1
    mov cr0, eax
    jmp 0x08:pm32

[BITS 32]
pm32:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000
    jmp 0x10000
[BITS 16]

; --------------------------------------------------------------------------
; Erro: interrompe (tela permanece preta)
; --------------------------------------------------------------------------
erro:
    cli
.parado:
    hlt
    jmp .parado

; --------------------------------------------------------------------------
; Dados
; --------------------------------------------------------------------------
align 8
gdt:
    dq 0x0000000000000000
    dq 0x00CF9A000000FFFF       ; código: base 0, limite 4 GiB, 32 bits
    dq 0x00CF92000000FFFF       ; dados : base 0, limite 4 GiB, 32 bits
gdt_fim:
gdt_desc:
    dw gdt_fim - gdt - 1
    dd gdt

dap:
    db 16, 0
    dw 2                        ; 2 setores = 1 bloco de 1 KiB
    dw BUF
    dw 0
    dq 0

drive:   db 0
itblk:   dd 0
curblk:  dd 0
blkcnt:  dw 0
dseg:      dw 0

; --------------------------------------------------------------------------
; O setor 1 tem exatamente 512 bytes (o bloco de boot ext4 tem 1024)
; --------------------------------------------------------------------------
    times 1024 - ($ - $$) db 0
