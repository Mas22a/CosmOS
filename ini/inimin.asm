; ==========================================================================
;   CosmOS Beta v0.1 — Construção 0.1.2026
;   ini/inimin.asm — Primeiro estágio de boot do CosmOS
; --------------------------------------------------------------------------
;   Alvo   : x86-64 (o BIOS entrega a CPU em modo real de 16 bits)
;   Tamanho: 512 bytes (setor de boot com assinatura 0xAA55)
;   Vídeo  : modo 13h — 320x200, 256 cores (8 bits por pixel)
;   Estado : tela totalmente preta (nada é desenhado nesta fase)
; ==========================================================================

[BITS 16]                       ; o BIOS inicia o processador em modo real (16 bits)
[ORG 0x7C00]                    ; endereço físico onde o BIOS carrega o setor de boot

inicio:
    cli                         ; sem interrupções durante a configuração
    xor ax, ax
    mov ds, ax                  ; DS = 0 (segmento de dados)
    mov es, ax                  ; ES = 0 (segmento extra)
    mov ss, ax                  ; SS = 0 (segmento de pilha)
    mov sp, 0x7C00              ; a pilha cresce para baixo a partir do código de boot

    ; ---------------------------------------------------------------
    ; Modo de vídeo 13h: 320 x 200 com 256 cores (8 bits por pixel)
    ; bit 7 de AL = 0 → a tela é limpa na troca do modo (fica preta)
    ; ---------------------------------------------------------------
    mov ax, 0x0013
    int 0x10

    ; Nada é desenhado: a tela permanece totalmente preta.
parado:
    hlt                         ; suspende a CPU
    jmp parado                  ; volta a suspender (espera para sempre)

; --------------------------------------------------------------------------
; Preenchimento dos bytes restantes + assinatura válida de setor de boot
; --------------------------------------------------------------------------
    times 510 - ($ - $$) db 0
    dw 0xAA55
