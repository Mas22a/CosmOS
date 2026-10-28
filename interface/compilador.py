#!/usr/bin/env python3
# ==========================================================================
#   CosmOS Beta v0.1 — Construção 0.8.2026
#   interface/compilador.py — Compilador Java bare-metal → BINÁRIO x86-32
# --------------------------------------------------------------------------
#   Lê um subconjunto de Java (dialeto CosmOS) e gera DIRETAMENTE o código
#   de máquina x86 de 32 bits. NÃO passa por C, NÃO usa gcc, NÃO usa
#   biblioteca padrão e NÃO usa máquina virtual. A saída é um binário plano
#   (flat binary) carregado em 0x80000.
#
#   Uso:
#       python3 interface/compilador.py entrada.java saida.grain
#
#   Subconjunto suportado:
#     - package / import (ignorados)
#     - uma classe: class Nome { ... }
#     - modificadores: public private protected static final native abstract
#     - tipos: void byte short int long boolean char
#     - campos static final com valor constante
#     - métodos static (com corpo) e main()
#     - if/else, while, for, return, break, continue, blocos
#     - operadores aritméticos, lógicos, bit a bit, comparação, atribuição
#     - chamadas a métodos, ++/--, atribuições compostas
#
#   Intrínsecos (funções embutidas):
#     memoria_preencher(destino, tamanho, valor) → REP STOSB
#     cpu_halt()                                 → HLT
#
#   Convenção de chamada: cdecl (argumentos na pilha, retorno em EAX).
#   Endereço base: 0x80000 (o serviço usr salta para cá).
# ==========================================================================

import re
import struct
import sys


# --------------------------------------------------------------------------
#   Erros
# --------------------------------------------------------------------------
class ErroCompilacao(Exception):
    pass


def erro(msg, tok=None):
    if tok is not None:
        raise ErroCompilacao("linha %d: %s" % (tok.linha, msg))
    raise ErroCompilacao(msg)


# --------------------------------------------------------------------------
#   Lexer
# --------------------------------------------------------------------------
class Token:
    __slots__ = ("tipo", "valor", "linha")

    def __init__(self, tipo, valor, linha):
        self.tipo = tipo
        self.valor = valor
        self.linha = linha

    def __repr__(self):
        return "Token(%s, %r)" % (self.tipo, self.valor)


PALAVRAS = {
    "public", "private", "protected", "static", "final", "native",
    "abstract", "class", "void", "byte", "short", "int", "long",
    "boolean", "char", "if", "else", "while", "for", "return",
    "break", "continue", "true", "false", "new", "package", "import",
}

OPERADORES = [
    "<<=", ">>=", ">>>=",
    "==", "!=", "<=", ">=", "&&", "||", "++", "--",
    "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>", "->", "::",
    "+", "-", "*", "/", "%", "=", "<", ">", "!", "&", "|", "^", "~",
    "?", ":", ";", ",", ".", "(", ")", "{", "}", "[", "]",
]

RE_ESPACO = re.compile(r"[ \t\r\n\f]+")
RE_LINHA = re.compile(r"//[^\n]*")
RE_BLOCO = re.compile(r"/\*.*?\*/", re.DOTALL)
RE_NUMERO = re.compile(r"0[xX][0-9a-fA-F]+|[0-9]+")
RE_STRING = re.compile(r'"(?:\\.|[^"\\])*"')
RE_CHAR = re.compile(r"'(?:\\.|[^'\\])'")
RE_IDENT = re.compile(r"[A-Za-z_$][A-Za-z0-9_$]*")


def lexar(fonte):
    toks = []
    i = 0
    linha = 1
    n = len(fonte)
    while i < n:
        m = RE_ESPACO.match(fonte, i)
        if m:
            linha += m.group(0).count("\n")
            i = m.end()
            continue
        m = RE_LINHA.match(fonte, i)
        if m:
            i = m.end()
            continue
        m = RE_BLOCO.match(fonte, i)
        if m:
            linha += m.group(0).count("\n")
            i = m.end()
            continue
        m = RE_STRING.match(fonte, i)
        if m:
            toks.append(Token("string", m.group(0), linha))
            i = m.end()
            continue
        m = RE_CHAR.match(fonte, i)
        if m:
            toks.append(Token("char", m.group(0), linha))
            i = m.end()
            continue
        m = RE_NUMERO.match(fonte, i)
        if m:
            toks.append(Token("numero", m.group(0), linha))
            i = m.end()
            continue
        m = RE_IDENT.match(fonte, i)
        if m:
            texto = m.group(0)
            toks.append(Token("palavra" if texto in PALAVRAS else "ident", texto, linha))
            i = m.end()
            continue
        achou = False
        for op in OPERADORES:
            if fonte.startswith(op, i):
                toks.append(Token("op", op, linha))
                i += len(op)
                achou = True
                break
        if achou:
            continue
        erro("caractere inesperado %r" % fonte[i])
    toks.append(Token("eof", "", linha))
    return toks


# --------------------------------------------------------------------------
#   Parser (descida recursiva)
# --------------------------------------------------------------------------
class Parser:
    def __init__(self, toks):
        self.toks = toks
        self.pos = 0

    def atual(self):
        return self.toks[self.pos]

    def olhar(self, k=1):
        return self.toks[min(self.pos + k, len(self.toks) - 1)]

    def avancar(self):
        t = self.toks[self.pos]
        self.pos += 1
        return t

    def aceitar(self, valor):
        if self.atual().valor == valor:
            self.pos += 1
            return True
        return False

    def esperar(self, valor):
        t = self.atual()
        if t.valor != valor:
            erro("esperado %r, encontrado %r" % (valor, t.valor), t)
        self.pos += 1
        return t

    def eh_tipo(self):
        t = self.atual()
        return t.tipo == "palavra" and t.valor in (
            "void", "byte", "short", "int", "long", "boolean", "char"
        )

    def eh_tipo_ou_ident(self):
        return self.eh_tipo() or self.atual().tipo == "ident"

    def parse_programa(self):
        while self.atual().valor in ("package", "import"):
            while self.atual().valor != ";":
                self.avancar()
            self.esperar(";")
        mods = self.parse_modificadores()
        self.esperar("class")
        nome = self.esperar_ident()
        self.esperar("{")
        campos = []
        metodos = []
        while not self.aceitar("}"):
            m = self.parse_modificadores()
            if self.atual().valor == "class":
                erro("classes aninhadas não suportadas", self.atual())
            tipo = self.parse_tipo()
            nome_m = self.esperar_ident()
            if self.atual().valor == "(":
                metodos.append(self.parse_metodo(tipo, nome_m, m))
            else:
                campos.extend(self.parse_campos(tipo, nome_m, m))
        return {"nome": nome, "mods": mods, "campos": campos, "metodos": metodos}

    def parse_modificadores(self):
        mods = []
        while True:
            t = self.atual()
            if t.tipo == "palavra" and t.valor in (
                "public", "private", "protected", "static", "final",
                "native", "abstract"
            ):
                mods.append(t.valor)
                self.avancar()
            else:
                break
        return mods

    def esperar_ident(self):
        t = self.atual()
        if t.tipo != "ident":
            erro("esperado identificador, encontrado %r" % t.valor, t)
        self.pos += 1
        return t.valor

    def parse_tipo(self):
        base = self.atual().valor
        if not self.eh_tipo_ou_ident():
            erro("esperado tipo, encontrado %r" % base, self.atual())
        self.avancar()
        dims = 0
        while self.atual().valor == "[":
            self.avancar()
            self.esperar("]")
            dims += 1
        return base + ("[]" * dims)

    def parse_campos(self, tipo, primeiro, mods):
        campos = []
        nome = primeiro
        while True:
            init = None
            if self.aceitar("="):
                init = self.parse_expr()
            campos.append({"tipo": tipo, "nome": nome, "init": init, "mods": mods})
            if self.aceitar(","):
                nome = self.esperar_ident()
                continue
            break
        self.esperar(";")
        return campos

    def parse_metodo(self, ret, nome, mods):
        self.esperar("(")
        params = []
        if self.atual().valor != ")":
            while True:
                pt = self.parse_tipo()
                pn = self.esperar_ident()
                params.append((pt, pn))
                if not self.aceitar(","):
                    break
        self.esperar(")")
        if self.aceitar(";"):
            return {"ret": ret, "nome": nome, "params": params, "corpo": None, "mods": mods}
        corpo = self.parse_bloco()
        return {"ret": ret, "nome": nome, "params": params, "corpo": corpo, "mods": mods}

    def parse_bloco(self):
        self.esperar("{")
        stmts = []
        while not self.aceitar("}"):
            stmts.append(self.parse_stmt())
        return stmts

    def parse_stmt(self):
        t = self.atual()
        v = t.valor
        if v == "{":
            return ("block", self.parse_bloco())
        if v == "if":
            self.avancar()
            self.esperar("(")
            cond = self.parse_expr()
            self.esperar(")")
            ent = self.parse_stmt()
            sai = self.parse_stmt() if self.aceitar("else") else None
            return ("if", cond, ent, sai)
        if v == "while":
            self.avancar()
            self.esperar("(")
            cond = self.parse_expr()
            self.esperar(")")
            return ("while", cond, self.parse_stmt())
        if v == "for":
            return self.parse_for()
        if v == "return":
            self.avancar()
            e = None if self.atual().valor == ";" else self.parse_expr()
            self.esperar(";")
            return ("return", e)
        if v == "break":
            self.avancar()
            self.esperar(";")
            return ("break",)
        if v == "continue":
            self.avancar()
            self.esperar(";")
            return ("continue",)
        if v == ";":
            self.avancar()
            return ("vazio",)
        if self.eh_tipo() and self.olhar().tipo == "ident":
            tipo = self.parse_tipo()
            decls = []
            nome = self.esperar_ident()
            while True:
                init = self.parse_expr() if self.aceitar("=") else None
                decls.append((nome, init))
                if self.aceitar(","):
                    nome = self.esperar_ident()
                    continue
                break
            self.esperar(";")
            return ("vardecl", tipo, decls)
        e = self.parse_expr()
        self.esperar(";")
        return ("expr", e)

    def parse_for(self):
        self.esperar("for")
        self.esperar("(")
        init = None
        if self.atual().valor != ";":
            if self.eh_tipo() and self.olhar().tipo == "ident":
                tipo = self.parse_tipo()
                nome = self.esperar_ident()
                val = self.parse_expr() if self.aceitar("=") else None
                init = ("vardecl", tipo, [(nome, val)])
            else:
                init = ("expr", self.parse_expr())
        self.esperar(";")
        cond = None if self.atual().valor == ";" else self.parse_expr()
        self.esperar(";")
        update = None
        if self.atual().valor != ")":
            updates = [self.parse_expr()]
            while self.aceitar(","):
                updates.append(self.parse_expr())
            update = ("expr", ("seq", updates)) if len(updates) > 1 else ("expr", updates[0])
        self.esperar(")")
        return ("for", init, cond, update, self.parse_stmt())

    # ---- expressões ------------------------------------------------------
    def parse_expr(self):
        return self.parse_atribuicao()

    def parse_atribuicao(self):
        esq = self.parse_ternario()
        t = self.atual()
        if t.tipo == "op" and t.valor in (
            "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="
        ):
            op = t.valor
            self.avancar()
            return ("assign", op, esq, self.parse_atribuicao())
        return esq

    def parse_ternario(self):
        cond = self.parse_ou()
        if self.aceitar("?"):
            a = self.parse_expr()
            self.esperar(":")
            b = self.parse_ternario()
            return ("ternario", cond, a, b)
        return cond

    def parse_ou(self):
        e = self.parse_e()
        while self.atual().valor == "||":
            self.avancar()
            e = ("bin", "||", e, self.parse_e())
        return e

    def parse_e(self):
        e = self.parse_ou_bit()
        while self.atual().valor == "&&":
            self.avancar()
            e = ("bin", "&&", e, self.parse_ou_bit())
        return e

    def parse_ou_bit(self):
        e = self.parse_xor()
        while self.atual().valor == "|":
            self.avancar()
            e = ("bin", "|", e, self.parse_xor())
        return e

    def parse_xor(self):
        e = self.parse_e_bit()
        while self.atual().valor == "^":
            self.avancar()
            e = ("bin", "^", e, self.parse_e_bit())
        return e

    def parse_e_bit(self):
        e = self.parse_igual()
        while self.atual().valor == "&":
            self.avancar()
            e = ("bin", "&", e, self.parse_igual())
        return e

    def parse_igual(self):
        e = self.parse_relacional()
        while self.atual().valor in ("==", "!="):
            op = self.avancar().valor
            e = ("bin", op, e, self.parse_relacional())
        return e

    def parse_relacional(self):
        e = self.parse_desloc()
        while self.atual().valor in ("<", ">", "<=", ">="):
            op = self.avancar().valor
            e = ("bin", op, e, self.parse_desloc())
        return e

    def parse_desloc(self):
        e = self.parse_aditivo()
        while self.atual().valor in ("<<", ">>"):
            op = self.avancar().valor
            e = ("bin", op, e, self.parse_aditivo())
        return e

    def parse_aditivo(self):
        e = self.parse_multiplicativo()
        while self.atual().valor in ("+", "-"):
            op = self.avancar().valor
            e = ("bin", op, e, self.parse_multiplicativo())
        return e

    def parse_multiplicativo(self):
        e = self.parse_unario()
        while self.atual().valor in ("*", "/", "%"):
            op = self.avancar().valor
            e = ("bin", op, e, self.parse_unario())
        return e

    def parse_unario(self):
        t = self.atual()
        if t.valor in ("!", "~", "-", "+"):
            self.avancar()
            return ("un", t.valor, self.parse_unario())
        if t.valor in ("++", "--"):
            self.avancar()
            return ("pre", t.valor, self.parse_unario())
        if t.valor == "(" and self.eh_tipo() and self.olhar(2).valor == ")":
            self.avancar()
            tipo = self.parse_tipo()
            self.esperar(")")
            return ("cast", tipo, self.parse_unario())
        return self.parse_posfixo()

    def parse_posfixo(self):
        e = self.parse_primario()
        while True:
            t = self.atual()
            if t.valor == "(":
                self.avancar()
                args = []
                if self.atual().valor != ")":
                    while True:
                        args.append(self.parse_expr())
                        if not self.aceitar(","):
                            break
                self.esperar(")")
                e = ("call", e, args)
            elif t.valor == "[":
                self.avancar()
                idx = self.parse_expr()
                self.esperar("]")
                e = ("index", e, idx)
            elif t.valor == ".":
                self.avancar()
                e = ("field", e, self.esperar_ident())
            elif t.valor in ("++", "--"):
                self.avancar()
                e = ("post", t.valor, e)
            else:
                break
        return e

    def parse_primario(self):
        t = self.atual()
        if t.tipo == "numero":
            self.avancar()
            return ("num", t.valor)
        if t.tipo == "string":
            self.avancar()
            return ("str", t.valor)
        if t.tipo == "char":
            self.avancar()
            return ("char", t.valor)
        if t.valor in ("true", "false"):
            self.avancar()
            return ("bool", t.valor)
        if t.tipo == "ident":
            self.avancar()
            return ("ident", t.valor)
        if t.valor == "(":
            self.avancar()
            e = self.parse_expr()
            self.esperar(")")
            return e
        erro("expressão inesperada %r" % t.valor, t)


# --------------------------------------------------------------------------
#   Codificador x86-32 (sem assembler, sem C)
# --------------------------------------------------------------------------
EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI = range(8)


class Binario:
    def __init__(self):
        self.code = bytearray()
        self.labels = {}
        self.fixups = []

    def pos(self):
        return len(self.code)

    def b(self, *vals):
        self.code.extend(vals)

    def imm32(self, v):
        self.code.extend(struct.pack("<I", v & 0xFFFFFFFF))

    def rel32(self, label):
        p = len(self.code)
        self.code.extend(b"\x00\x00\x00\x00")
        self.fixups.append((p, label))

    def rotulo(self, nome):
        self.labels[nome] = len(self.code)

    # --- instruções -------------------------------------------------------
    def push_r(self, r):
        self.b(0x50 + r)

    def pop_r(self, r):
        self.b(0x58 + r)

    def mov_r_imm(self, r, imm):
        self.b(0xB8 + r)
        self.imm32(imm)

    def mov_r_mem(self, r, disp):
        # mov r32, [ebp+disp32]
        self.b(0x8B, 0x85 | (r << 3))
        self.imm32(disp)

    def mov_mem_r(self, disp, r):
        # mov [ebp+disp32], r32
        self.b(0x89, 0x85 | (r << 3))
        self.imm32(disp)

    def mov_mem_imm(self, disp, imm):
        # mov dword [ebp+disp32], imm32
        self.b(0xC7, 0x85)
        self.imm32(disp)
        self.imm32(imm)

    def rr(self, opcode, reg, rm):
        # opcode r/m32, r32  (mod=11)
        self.b(opcode, 0xC0 | (reg << 3) | rm)

    def add_eax_ebx(self):
        self.rr(0x01, EBX, EAX)

    def sub_eax_ebx(self):
        self.rr(0x29, EBX, EAX)

    def and_eax_ebx(self):
        self.rr(0x21, EBX, EAX)

    def or_eax_ebx(self):
        self.rr(0x09, EBX, EAX)

    def xor_eax_ebx(self):
        self.rr(0x31, EBX, EAX)

    def cmp_eax_ebx(self):
        self.rr(0x39, EBX, EAX)

    def mov_ecx_ebx(self):
        self.rr(0x89, EBX, ECX)

    def mov_ebx_eax(self):
        self.rr(0x89, EAX, EBX)

    def mov_eax_edx(self):
        self.rr(0x89, EDX, EAX)

    def imul_eax_ebx(self):
        self.b(0x0F, 0xAF, 0xC3)

    def idiv_ebx(self):
        self.b(0xF7, 0xFB)

    def cdq(self):
        self.b(0x99)

    def not_eax(self):
        self.b(0xF7, 0xD0)

    def neg_eax(self):
        self.b(0xF7, 0xD8)

    def test_eax_eax(self):
        self.b(0x85, 0xC0)

    def setcc_al(self, cc):
        self.b(0x0F, 0x90 + cc, 0xC0)

    def movzx_eax_al(self):
        self.b(0x0F, 0xB6, 0xC0)

    def shl_eax_cl(self):
        self.b(0xD3, 0xE0)

    def sar_eax_cl(self):
        self.b(0xD3, 0xF8)

    def sub_esp_imm(self, imm):
        self.b(0x81, 0xEC)
        self.imm32(imm)

    def add_esp_imm(self, imm):
        self.b(0x81, 0xC4)
        self.imm32(imm)

    def push_ebp(self):
        self.b(0x55)

    def pop_ebp(self):
        self.b(0x5D)

    def mov_ebp_esp(self):
        self.b(0x89, 0xE5)

    def mov_esp_ebp(self):
        self.b(0x89, 0xEC)

    def rep_stosb(self):
        self.b(0xF3, 0xAA)

    def hlt(self):
        self.b(0xF4)

    def ret(self):
        self.b(0xC3)

    def jmp(self, label):
        self.b(0xE9)
        self.rel32(label)

    def je(self, label):
        self.b(0x0F, 0x84)
        self.rel32(label)

    def jne(self, label):
        self.b(0x0F, 0x85)
        self.rel32(label)

    def call(self, label):
        self.b(0xE8)
        self.rel32(label)

    def resolver(self):
        for p, label in self.fixups:
            if label not in self.labels:
                raise ErroCompilacao("rótulo não encontrado: %s" % label)
            rel = self.labels[label] - (p + 4)
            self.code[p:p + 4] = struct.pack("<i", rel)


CC = {"==": 0x04, "!=": 0x05, "<": 0x0C, ">": 0x0F, "<=": 0x0E, ">=": 0x0D}


# --------------------------------------------------------------------------
#   Gerador de código x86-32
# --------------------------------------------------------------------------
class GeradorX86:
    def __init__(self, prog):
        self.prog = prog
        self.classe = prog["nome"]
        self.bin = Binario()
        self.syms = {}
        self.consts = {}
        self.metodos = {m["nome"]: m for m in prog["metodos"]}
        self.contador = 0
        self.loops = []

    # -- utilidades --------------------------------------------------------
    def novo_rotulo(self, prefixo="L"):
        self.contador += 1
        return "%s%d" % (prefixo, self.contador)

    def valor_constante(self, e):
        t = e[0]
        if t == "num":
            return int(e[1], 0)
        if t == "bool":
            return 1 if e[1] == "true" else 0
        if t == "un" and e[1] == "-":
            return -self.valor_constante(e[2])
        if t == "un" and e[1] == "~":
            return ~self.valor_constante(e[2])
        if t == "bin":
            a = self.valor_constante(e[2])
            b = self.valor_constante(e[3])
            op = e[1]
            if op == "+": return a + b
            if op == "-": return a - b
            if op == "*": return a * b
            if op == "/": return int(a / b)
            if op == "%": return a % b
            if op == "&": return a & b
            if op == "|": return a | b
            if op == "^": return a ^ b
            if op == "<<": return a << b
            if op == ">>": return a >> b
        raise ErroCompilacao("valor não constante: %r" % (t,))

    def carregar_constantes(self):
        for c in self.prog["campos"]:
            if c["init"] is not None:
                self.consts[c["nome"]] = self.valor_constante(c["init"])

    # -- locals ------------------------------------------------------------
    def coletar_locais(self, stmts, nomes):
        for s in stmts:
            t = s[0]
            if t == "vardecl":
                for n, _ in s[2]:
                    if n not in nomes:
                        nomes.append(n)
            elif t == "block":
                self.coletar_locais(s[1], nomes)
            elif t == "if":
                self.coletar_locais([s[2]], nomes)
                if s[3] is not None:
                    self.coletar_locais([s[3]], nomes)
            elif t == "while":
                self.coletar_locais([s[2]], nomes)
            elif t == "for":
                if s[1] is not None:
                    self.coletar_locais([s[1]], nomes)
                self.coletar_locais([s[4]], nomes)

    # -- expressões --------------------------------------------------------
    def expr(self, e):
        t = e[0]
        if t == "num":
            self.bin.mov_r_imm(EAX, int(e[1], 0))
        elif t == "char":
            v = e[1][1:-1]
            v = v.replace("\\n", "\n").replace("\\t", "\t")
            self.bin.mov_r_imm(EAX, ord(v) if len(v) == 1 else 0)
        elif t == "bool":
            self.bin.mov_r_imm(EAX, 1 if e[1] == "true" else 0)
        elif t == "ident":
            self.ler_ident(e[1])
        elif t == "bin":
            self.bin_expr(e)
        elif t == "un":
            self.expr(e[2])
            if e[1] == "!":
                self.bin.test_eax_eax()
                self.bin.setcc_al(CC["=="])
                self.bin.movzx_eax_al()
            elif e[1] == "~":
                self.bin.not_eax()
            elif e[1] == "-":
                self.bin.neg_eax()
            # '+' não faz nada
        elif t in ("pre", "post"):
            self.expr_incdec(e)
        elif t == "assign":
            self.expr_assign(e)
        elif t == "ternario":
            falso = self.novo_rotulo("tern_f")
            fim = self.novo_rotulo("tern_end")
            self.expr(e[1])
            self.bin.test_eax_eax()
            self.bin.je(falso)
            self.expr(e[2])
            self.bin.jmp(fim)
            self.bin.rotulo(falso)
            self.expr(e[3])
            self.bin.rotulo(fim)
        elif t == "cast":
            self.expr(e[2])
            if e[1] == "byte":
                # movsx eax, al
                self.bin.b(0x0F, 0xBE, 0xC0)
            elif e[1] == "short":
                self.bin.b(0x0F, 0xBF, 0xC0)
            elif e[1] == "char":
                self.bin.b(0x0F, 0xB7, 0xC0)
        elif t == "seq":
            for x in e[1]:
                self.expr(x)
        elif t == "call":
            self.expr_call(e)
        else:
            raise ErroCompilacao("expressão não suportada: %r" % (t,))

    def ler_ident(self, nome):
        if nome in self.syms:
            _, off = self.syms[nome]
            self.bin.mov_r_mem(EAX, off)
        elif nome in self.consts:
            self.bin.mov_r_imm(EAX, self.consts[nome])
        else:
            raise ErroCompilacao("identificador desconhecido: %s" % nome)

    def guardar_ident(self, nome):
        if nome in self.syms:
            _, off = self.syms[nome]
            self.bin.mov_mem_r(off, EAX)
        else:
            raise ErroCompilacao("não é possível atribuir a: %s" % nome)

    def bin_expr(self, e):
        op = e[1]
        if op == "&&":
            falso = self.novo_rotulo("and_f")
            fim = self.novo_rotulo("and_end")
            self.expr(e[2])
            self.bin.test_eax_eax()
            self.bin.je(falso)
            self.expr(e[3])
            self.bin.test_eax_eax()
            self.bin.je(falso)
            self.bin.mov_r_imm(EAX, 1)
            self.bin.jmp(fim)
            self.bin.rotulo(falso)
            self.bin.mov_r_imm(EAX, 0)
            self.bin.rotulo(fim)
            return
        if op == "||":
            verdade = self.novo_rotulo("or_t")
            fim = self.novo_rotulo("or_end")
            self.expr(e[2])
            self.bin.test_eax_eax()
            self.bin.jne(verdade)
            self.expr(e[3])
            self.bin.test_eax_eax()
            self.bin.jne(verdade)
            self.bin.mov_r_imm(EAX, 0)
            self.bin.jmp(fim)
            self.bin.rotulo(verdade)
            self.bin.mov_r_imm(EAX, 1)
            self.bin.rotulo(fim)
            return
        # avalia esquerda -> pilha, direita -> EAX, esquerda -> EAX
        self.expr(e[2])
        self.bin.push_r(EAX)
        self.expr(e[3])
        self.bin.mov_ebx_eax()
        self.bin.pop_r(EAX)
        if op == "+":
            self.bin.add_eax_ebx()
        elif op == "-":
            self.bin.sub_eax_ebx()
        elif op == "*":
            self.bin.imul_eax_ebx()
        elif op == "/":
            self.bin.cdq()
            self.bin.idiv_ebx()
        elif op == "%":
            self.bin.cdq()
            self.bin.idiv_ebx()
            self.bin.mov_eax_edx()
        elif op == "&":
            self.bin.and_eax_ebx()
        elif op == "|":
            self.bin.or_eax_ebx()
        elif op == "^":
            self.bin.xor_eax_ebx()
        elif op == "<<":
            self.bin.mov_ecx_ebx()
            self.bin.shl_eax_cl()
        elif op == ">>":
            self.bin.mov_ecx_ebx()
            self.bin.sar_eax_cl()
        elif op in CC:
            self.bin.cmp_eax_ebx()
            self.bin.setcc_al(CC[op])
            self.bin.movzx_eax_al()
        else:
            raise ErroCompilacao("operador não suportado: %s" % op)

    def expr_incdec(self, e):
        op = e[1]
        alvo = e[2]
        if alvo[0] != "ident":
            raise ErroCompilacao("++/-- só em variáveis simples")
        self.ler_ident(alvo[1])
        if e[0] == "post":
            self.bin.mov_r_imm(EBX, 1)
            self.bin.rr(0x01 if op == "++" else 0x29, EBX, EAX)  # add/sub eax,1
            self.guardar_ident(alvo[1])
            # resultado do post é o valor antigo: recarrega e subtrai
            self.ler_ident(alvo[1])
            self.bin.mov_r_imm(EBX, 1)
            self.bin.rr(0x29 if op == "++" else 0x01, EBX, EAX)
        else:
            self.bin.mov_r_imm(EBX, 1)
            self.bin.rr(0x01 if op == "++" else 0x29, EBX, EAX)
            self.guardar_ident(alvo[1])

    def expr_assign(self, e):
        op, alvo, valor = e[1], e[2], e[3]
        if alvo[0] != "ident":
            raise ErroCompilacao("atribuição só em variáveis simples")
        nome = alvo[1]
        if op == "=":
            self.expr(valor)
            self.guardar_ident(nome)
        else:
            self.ler_ident(nome)
            self.bin.push_r(EAX)
            self.expr(valor)
            self.bin.mov_ebx_eax()
            self.bin.pop_r(EAX)
            base = op[:-1]
            if base == "+": self.bin.add_eax_ebx()
            elif base == "-": self.bin.sub_eax_ebx()
            elif base == "*": self.bin.imul_eax_ebx()
            elif base == "&": self.bin.and_eax_ebx()
            elif base == "|": self.bin.or_eax_ebx()
            elif base == "^": self.bin.xor_eax_ebx()
            elif base == "<<":
                self.bin.mov_ecx_ebx(); self.bin.shl_eax_cl()
            elif base == ">>":
                self.bin.mov_ecx_ebx(); self.bin.sar_eax_cl()
            elif base == "/":
                self.bin.cdq(); self.bin.idiv_ebx()
            elif base == "%":
                self.bin.cdq(); self.bin.idiv_ebx(); self.bin.mov_eax_edx()
            else:
                raise ErroCompilacao("atribuição não suportada: %s" % op)
            self.guardar_ident(nome)

    def expr_call(self, e):
        alvo, args = e[1], e[2]
        nome = alvo[1] if alvo[0] == "ident" else (alvo[2] if alvo[0] == "field" else None)
        if nome == "memoria_preencher":
            if len(args) != 3:
                raise ErroCompilacao("memoria_preencher espera 3 argumentos")
            self.expr(args[0]); self.bin.push_r(EAX)   # destino
            self.expr(args[1]); self.bin.push_r(EAX)   # tamanho
            self.expr(args[2]); self.bin.push_r(EAX)   # valor
            self.bin.pop_r(EAX)                        # AL = valor
            self.bin.pop_r(ECX)                        # ECX = tamanho
            self.bin.pop_r(EDI)                        # EDI = destino
            self.bin.rep_stosb()
            return
        if nome == "cpu_halt":
            self.bin.hlt()
            return
        if nome not in self.metodos:
            raise ErroCompilacao("chamada desconhecida: %s" % nome)
        for a in reversed(args):
            self.expr(a)
            self.bin.push_r(EAX)
        self.bin.call("fn:" + nome)
        if args:
            self.bin.add_esp_imm(4 * len(args))

    # -- comandos ----------------------------------------------------------
    def stmt(self, s):
        t = s[0]
        if t == "block":
            for x in s[1]:
                self.stmt(x)
        elif t == "vardecl":
            for nome, init in s[2]:
                if init is not None:
                    self.expr(init)
                else:
                    self.bin.mov_r_imm(EAX, 0)
                self.guardar_ident(nome)
        elif t == "expr":
            self.expr(s[1])
        elif t == "if":
            fim = self.novo_rotulo("if_end")
            self.expr(s[1])
            self.bin.test_eax_eax()
            if s[3] is not None:
                senao = self.novo_rotulo("if_else")
                self.bin.je(senao)
                self.stmt(s[2])
                self.bin.jmp(fim)
                self.bin.rotulo(senao)
                self.stmt(s[3])
            else:
                self.bin.je(fim)
                self.stmt(s[2])
            self.bin.rotulo(fim)
        elif t == "while":
            inicio = self.novo_rotulo("while_start")
            fim = self.novo_rotulo("while_end")
            self.loops.append((inicio, fim))
            self.bin.rotulo(inicio)
            self.expr(s[1])
            self.bin.test_eax_eax()
            self.bin.je(fim)
            self.stmt(s[2])
            self.bin.jmp(inicio)
            self.bin.rotulo(fim)
            self.loops.pop()
        elif t == "for":
            init, cond, update, corpo = s[1], s[2], s[3], s[4]
            inicio = self.novo_rotulo("for_start")
            fim = self.novo_rotulo("for_end")
            self.loops.append((inicio, fim))
            if init is not None:
                self.stmt(init)
            self.bin.rotulo(inicio)
            if cond is not None:
                self.expr(cond)
                self.bin.test_eax_eax()
                self.bin.je(fim)
            self.stmt(corpo)
            if update is not None:
                self.stmt(update)
            self.bin.jmp(inicio)
            self.bin.rotulo(fim)
            self.loops.pop()
        elif t == "return":
            if s[1] is not None:
                self.expr(s[1])
            self.bin.jmp(self.rotulo_epilogo)
        elif t == "break":
            if not self.loops:
                raise ErroCompilacao("break fora de laço")
            self.bin.jmp(self.loops[-1][1])
        elif t == "continue":
            if not self.loops:
                raise ErroCompilacao("continue fora de laço")
            self.bin.jmp(self.loops[-1][0])
        elif t == "vazio":
            pass
        else:
            raise ErroCompilacao("comando não suportado: %r" % (t,))

    # -- funções -----------------------------------------------------------
    def gerar_metodo(self, m):
        nome = m["nome"]
        self.syms = {}
        for i, (_, pn) in enumerate(m["params"]):
            self.syms[pn] = ("param", 8 + 4 * i)
        locais = []
        self.coletar_locais(m["corpo"] or [], locais)
        for i, ln in enumerate(locais):
            self.syms[ln] = ("local", -4 * (i + 1))
        tamanho_locais = 4 * len(locais)

        self.rotulo_epilogo = "epi:" + nome
        self.loops = []
        self.bin.rotulo("fn:" + nome)

        self.bin.push_ebp()
        self.bin.mov_ebp_esp()
        if tamanho_locais:
            self.bin.sub_esp_imm(tamanho_locais)

        for s in (m["corpo"] or []):
            self.stmt(s)

        self.bin.rotulo(self.rotulo_epilogo)
        self.bin.mov_esp_ebp()
        self.bin.pop_ebp()
        self.bin.ret()

    def gerar(self):
        self.carregar_constantes()
        # _start precisa ser o primeiro byte (0x80000)
        self.bin.call("fn:main")
        self.bin.hlt()
        self.bin.jmp("__fim__")   # após HLT, laço infinito
        self.bin.rotulo("__fim__")
        self.bin.jmp("__fim__")

        if "main" not in self.metodos:
            raise ErroCompilacao("a classe precisa de um método main()")

        for m in self.prog["metodos"]:
            if m["corpo"] is not None:
                self.gerar_metodo(m)

        self.bin.resolver()
        return bytes(self.bin.code)


# --------------------------------------------------------------------------
#   main
# --------------------------------------------------------------------------
def main(argv):
    if len(argv) != 3:
        print("uso: %s entrada.java saida.grain" % argv[0], file=sys.stderr)
        return 2
    with open(argv[1], "r", encoding="utf-8") as f:
        fonte = f.read()
    prog = Parser(lexar(fonte)).parse_programa()
    corpo = GeradorX86(prog).gerar()
    with open(argv[2], "wb") as f:
        f.write(corpo)
    print("[compilador] %s → %s (%d bytes de código x86-32)"
          % (argv[1], argv[2], len(corpo)))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except ErroCompilacao as e:
        print("erro de compilação: %s" % e, file=sys.stderr)
        sys.exit(1)
