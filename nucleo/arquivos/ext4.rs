/* ==========================================================================
 *   CosmOS Beta v0.1 — Construção 0.7.2026
 *   nucleo/arquivos/ext4.rs — Leitor de arquivos ext4 do CosmOS
 * --------------------------------------------------------------------------
 *   Alvo   : x86 32 bits, modo protegido.
 *   Função : localizar um arquivo "/diretorio/arquivo" na imagem ext4 e
 *            carregá-lo na memória — é assim que o núcleo SAI para o
 *            serviço /servicos/usr.
 *
 *   O núcleo herdou do ini/inicio.c o mesmo caminho de leitura, agora em
 *   Rust: descritor do grupo 0 (bloco 2) → tabela de inodes → inode raiz
 *   (2) → varre o bloco do diretório (árvore de extensões, profundidade
 *   0) → recarrega o inode encontrado. Os dados vêm do disco pelo
 *   nucleo/drivers/disco/ata.rs (ATA PIO), 1 bloco ext4 = 1 KiB = 2
 *   setores.
 *
 *   Premissas (garantidas pelo copilar.sh): blocos de 1024 bytes, inodes
 *   de 256 bytes e árvore de extensões em profundidade 0 (folha).
 *
 *   Exporta ext4_carregar(dir, dir_tam, arq, arq_tam, destino, max),
 *   chamado pelo nucleo/nucleo.rs. Compilado separadamente e linkado
 *   junto com os demais módulos para formar /nucleo/nucos (ver copilar.sh).
 * ========================================================================== */

#![no_std]

use core::ptr::read;

/* ATA PIO — quem fornece os bytes do disco. */
extern "C" {
    fn ata_ler_setores(lba: u32, n: u16, destino: *mut u8) -> u8;
}

const BLOCO_EXT4: usize = 1024; /* blocos do ext4 gerado pelo copilar.sh */
const TAM_INODE: usize = 256; /* inodes de 256 bytes (4 por bloco) */
const MAGIA_EXT: u16 = 0xF30A; /* assinatura da árvore de extensões */

/* Buffers de trabalho: fora do caminho do bootloader e da pilha (0x90000).
 * Estão no .bss do nucos (o nucleo.ld garante que ele é a última seção
 * do binário plano, então nada disso é gravado no arquivo). */
static mut BLOCO: [u8; BLOCO_EXT4] = [0; BLOCO_EXT4];
static mut INODE: [u8; TAM_INODE] = [0; TAM_INODE];
static mut ITBLK: u32 = 0; /* primeiro bloco da tabela de inodes */

/* --- Leituras little-endian (o x86 é little-endian) ---------------------- */

unsafe fn u16_em(p: *const u8) -> u16 {
    (read(p) as u16) | ((read(p.add(1)) as u16) << 8)
}

unsafe fn u32_em(p: *const u8) -> u32 {
    (read(p) as u32)
        | ((read(p.add(1)) as u32) << 8)
        | ((read(p.add(2)) as u32) << 16)
        | ((read(p.add(3)) as u32) << 24)
}

/* Lê 1 bloco ext4 (2 setores ATA) para `destino`. */
unsafe fn ler_bloco(b: u32, destino: *mut u8) -> u8 {
    ata_ler_setores(b * 2, 2, destino)
}

/* --- ext4: do inode ao arquivo ------------------------------------------- */

/* n (inode) → INODE[256]. Devolve 0 = falha. */
unsafe fn carrega_inode(n: u32) -> u8 {
    let b = ITBLK + (n - 1) / 4; /* 4 inodes de 256 bytes por bloco */
    let off = (((n - 1) & 3) as usize) * TAM_INODE;

    if ler_bloco(b, BLOCO.as_mut_ptr()) == 0 {
        return 0;
    }

    let mut i: usize = 0;
    while i < TAM_INODE {
        INODE[i] = read(BLOCO.as_ptr().add(off + i));
        i += 1;
    }
    1
}

/* Procura `nome` no diretório guardado em INODE[]; se achar, recarrega
 * INODE[] com o inode encontrado. Devolve 0 = falha. */
unsafe fn acha_entrada(nome: *const u8, len: u8) -> u8 {
    if u16_em(INODE.as_ptr().add(0x28)) != MAGIA_EXT {
        return 0; /* não é árvore de extensões */
    }
    if u16_em(INODE.as_ptr().add(0x2E)) != 0 {
        return 0; /* profundidade != 0 (não é folha) */
    }

    /* Primeiro bloco da entrada da raiz da árvore (ee_start). */
    let base = INODE.as_ptr().add(0x34);
    let b = u32_em(base.add(8)) | ((u16_em(base.add(6)) as u32) << 16);
    if b == 0 || ler_bloco(b, BLOCO.as_mut_ptr()) == 0 {
        return 0;
    }

    let mut off: usize = 0;
    while off < BLOCO_EXT4 {
        let e = BLOCO.as_ptr().add(off);
        let ino = u32_em(e);
        let rec = u16_em(e.add(4)) as usize;

        /* inode, tamanho do nome, bytes do nome — tudo little-endian. */
        if ino != 0 && read(e.add(6)) == len {
            let mut igual = true;
            let mut k: usize = 0;
            while k < len as usize {
                if read(e.add(8 + k)) != read(nome.add(k)) {
                    igual = false;
                    break;
                }
                k += 1;
            }
            if igual {
                return carrega_inode(ino);
            }
        }
        if rec == 0 {
            return 0;
        }
        off += rec;
    }
    0
}

/* Lê todas as extensões do INODE[] para `destino` (tamanho máximo `max`).
 * Devolve o tamanho carregado ou 0 = falha. */
unsafe fn carrega_arquivo(destino: *mut u8, max: usize) -> usize {
    let tam = u32_em(INODE.as_ptr().add(4)) as usize; /* i_size_lo */

    if u16_em(INODE.as_ptr().add(0x28)) != MAGIA_EXT {
        return 0;
    }
    if u16_em(INODE.as_ptr().add(0x2E)) != 0 {
        return 0;
    }
    if tam == 0 || tam > max {
        return 0;
    }

    let n = u16_em(INODE.as_ptr().add(0x2A)); /* número de extensões */
    if n > 4 {
        return 0; /* só folhas com até 4 extensões */
    }

    let mut ext = INODE.as_ptr().add(0x34); /* primeira extensão */
    let mut i: u16 = 0;
    while i < n {
        let log = u32_em(ext); /* ee_block (bloco lógico) */
        let fis = u32_em(ext.add(8)) | ((u16_em(ext.add(6)) as u32) << 16);
        let len = u16_em(ext.add(4)) & 0x7FFF; /* ee_len */

        let mut j: u16 = 0;
        while j < len {
            let desloc = ((log + j as u32) as usize) * BLOCO_EXT4;
            if desloc >= max {
                return 0;
            }
            if ler_bloco(fis + j as u32, destino.add(desloc)) == 0 {
                return 0;
            }
            j += 1;
        }

        ext = ext.add(12); /* cada extensão ocupa 12 bytes */
        i += 1;
    }
    tam
}

/* --- API pública ---------------------------------------------------------- */

/* Carrega "/diretorio/arquivo" do ext4 para `destino` (tamanho máximo
 * `max` bytes). Devolve o tamanho em bytes ou 0 = falha. */
#[no_mangle]
pub extern "C" fn ext4_carregar(
    diretorio: *const u8,
    dir_tam: usize,
    arquivo: *const u8,
    arq_tam: usize,
    destino: *mut u8,
    max: usize,
) -> usize {
    if diretorio.is_null() || arquivo.is_null() || dir_tam == 0 || arq_tam == 0 {
        return 0;
    }
    if dir_tam > 255 || arq_tam > 255 {
        return 0;
    }

    unsafe {
        /* descritor do grupo 0 (bloco 2) → tabela de inodes */
        if ler_bloco(2, BLOCO.as_mut_ptr()) == 0 {
            return 0;
        }
        ITBLK = u32_em(BLOCO.as_ptr().add(8)); /* bg_inode_table_lo */

        if carrega_inode(2) == 0 {
            return 0; /* inode raiz */
        }
        if acha_entrada(diretorio, dir_tam as u8) == 0 {
            return 0; /* /diretorio */
        }
        if acha_entrada(arquivo, arq_tam as u8) == 0 {
            return 0; /* /diretorio/arquivo */
        }

        carrega_arquivo(destino, max)
    }
}
