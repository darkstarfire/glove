#!/usr/bin/env python3
"""Ajuda a internacionalizar o Glove (só para quem desenvolve).

    tools/i18n.py marcar ARQUIVO.rs ARQUIVO.json
        O JSON é uma lista de [linha, "texto em português", "tradução em inglês"].
        Na linha indicada, troca "texto" por tr!("texto"), põe `use glove_config::tr;`
        no arquivo (se faltar) e registra a tradução em locales/en.po.
        Linha 0 = só registra a tradução (para textos que o código já marca).

    tools/i18n.py faltam
        Lista os tr!("…") do projeto que ainda não têm tradução em inglês.
"""
import json
import os
import re
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PO = os.path.join(RAIZ, "locales", "en.po")


def escapa(s):
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t")


def le_po():
    """O catálogo do Glove somado ao da glove-ui (ui/locales/en.po)."""
    pares = {}
    for po in (os.path.join(RAIZ, "ui", "locales", "en.po"), PO):
        if os.path.exists(po):
            pares.update(le_po_arquivo(po))
    return pares


def le_po_arquivo(PO):
    pares = {}
    if not os.path.exists(PO):
        return pares
    msgid = msgstr = None
    campo = None
    for linha in open(PO, encoding="utf-8"):
        l = linha.strip()
        if l.startswith("msgid "):
            if msgid:
                pares[msgid] = msgstr
            msgid, msgstr, campo = json.loads(l[6:]), "", "id"
        elif l.startswith("msgstr "):
            msgstr, campo = json.loads(l[7:]), "str"
        elif l.startswith('"') and campo:
            if campo == "id":
                msgid += json.loads(l)
            else:
                msgstr += json.loads(l)
    if msgid:
        pares[msgid] = msgstr
    return pares


def registra(novos):
    existentes = le_po()
    with open(PO, "a", encoding="utf-8") as f:
        for pt, en in novos:
            if pt in existentes:
                if existentes[pt] != en:
                    print(f"  aviso: '{pt}' já traduzido como '{existentes[pt]}' (mantido)")
                continue
            existentes[pt] = en
            f.write(f'msgid "{escapa(pt)}"\nmsgstr "{escapa(en)}"\n\n')


def literal_rust(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t") + '"'


def marcar(arquivo, lista):
    linhas = open(arquivo, encoding="utf-8").read().split("\n")
    novos = []
    for linha, pt, en in lista:
        novos.append((pt, en))
        if linha == 0:
            continue
        i = linha - 1
        lit = literal_rust(pt)
        if lit not in linhas[i]:
            sys.exit(f"{arquivo}:{linha}: não achei {lit} em: {linhas[i].strip()}")
        # Todas as ocorrências ainda sem tr!( na linha.
        linhas[i] = re.sub(r"(?<!tr!\()" + re.escape(lit), lambda m: f"tr!({lit})", linhas[i])
    texto = "\n".join(linhas)
    if "tr!(" in texto and not re.search(r"^use (glove_config|glove_i18n|crate)::(tr|\{[^}]*\btr\b)", texto, re.M) and "macro_rules! tr" not in texto:
        # Depois dos comentários de abertura (//!) e dos atributos do arquivo.
        ls = texto.split("\n")
        j = 0
        while j < len(ls) and (ls[j].startswith("//!") or ls[j].startswith("#![") or ls[j].strip() == ""):
            j += 1
        ls.insert(j, "use glove_config::tr;")
        texto = "\n".join(ls)
    open(arquivo, "w", encoding="utf-8").write(texto)
    registra(novos)
    print(f"{arquivo}: {len(lista)} textos")


def faltam():
    en = le_po()
    padrao = re.compile(r'tr!\("((?:[^"\\]|\\.)*)"')
    falta = set()
    for base, dirs, arqs in os.walk(os.path.join(RAIZ, "crates")):
        dirs[:] = [d for d in dirs if d not in ("target", "i18n")]
        for a in arqs:
            if a.endswith(".rs"):
                for m in padrao.finditer(open(os.path.join(base, a), encoding="utf-8").read()):
                    s = bytes(m.group(1), "utf-8").decode("unicode_escape").encode("latin-1").decode("utf-8")
                    if s not in en:
                        falta.add(s)
    for s in sorted(falta):
        print(s)
    print(f"{len(falta)} sem tradução", file=sys.stderr)


if __name__ == "__main__" and not (len(sys.argv) >= 3 and sys.argv[1] in ("candidatos", "aplicar", "consertar")):
    if len(sys.argv) >= 4 and sys.argv[1] == "marcar":
        marcar(sys.argv[2], json.load(open(sys.argv[3], encoding="utf-8")))
    elif len(sys.argv) >= 2 and sys.argv[1] == "faltam":
        faltam()
    else:
        print(__doc__)


# ---------------------------------------------------------------------------
# Modo automático por pasta: candidatos / aplicar
# ---------------------------------------------------------------------------

LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')
# Linhas que não são texto de interface (comandos, caminhos, registro, testes…).
PULAR = re.compile(r'tracing::|\binfo!\(|\bwarn!\(|\berror!\(|\bdebug!\(|eprintln!|println!|Command::new|\.arg\(|\.args\(|env::|set_var|'
                   r'read_to_string|Path::new|PathBuf::from|\.join\(|assert|panic!|expect\(|unreachable!|#\[|include_str|concat!|r#"')
# Linhas em que o texto é chave da lógica.
LOGICA = re.compile(r'==|!=|\bstarts_with\(|\bends_with\(|\bcontains\(|\bstrip_prefix\(|\bstrip_suffix\(|\bfind\(|\beq_ignore_ascii_case\(|matches!|\.get\("')


def parece_texto(s):
    if not re.search(r"[A-Za-zÀ-ú]", s):
        return False
    if re.fullmatch(r"[a-z0-9_.+:/@-]+", s):          # identificador, comando, caminho
        return False
    if any(x in s for x in ("://", "/usr", "/etc", "/tmp", "~/", "$HOME", "%U", "%f", "glove-", "glove.", "org.", "text/", "image/", "application/")):
        return False
    if re.fullmatch(r"(Ctrl|Alt|Shift|Super)[+A-Za-z0-9_+ -]*", s):
        return False
    return bool(re.search(r"[ À-ú]", s) or re.match(r"[A-ZÀ-Ú]", s))


def converte_format(linha):
    """format!("a {x} b {}", y) → tr!("a {} b {}", x, y) quando só há {} e {nome}.
    Devolve (linha nova, chave) ou None."""
    i = linha.find('format!("')
    if i < 0:
        return None
    m = LIT.match(linha, i + 8)
    if not m:
        return None
    lit = m.group(1)
    # Fim do format!(…) na mesma linha.
    j, nivel, dentro = m.end(), 1, False
    while j < len(linha):
        c = linha[j]
        if dentro:
            if c == "\\":
                j += 1
            elif c == '"':
                dentro = False
        elif c == '"':
            dentro = True
        elif c == "(":
            nivel += 1
        elif c == ")":
            nivel -= 1
            if nivel == 0:
                break
        j += 1
    if nivel != 0:
        return None
    resto = linha[m.end():j].strip()
    args_pos = [a.strip() for a in dividir_args(resto[1:])] if resto.startswith(",") else []
    if resto and not resto.startswith(","):
        return None
    novos, chave, k = [], "", 0
    pos = 0
    for ph in re.finditer(r"\{([^{}]*)\}", lit):
        dentro_ph = ph.group(1)
        chave += lit[pos:ph.start()]
        pos = ph.end()
        if dentro_ph == "":
            if k >= len(args_pos):
                return None
            novos.append(args_pos[k]); k += 1
        elif re.fullmatch(r"[a-z_][a-z0-9_]*", dentro_ph):
            novos.append(dentro_ph)
        else:
            return None           # {:02}, {x:?}… : à mão
        chave += "{}"
    chave += lit[pos:]
    if "{{" in lit or k != len(args_pos):
        return None
    if not parece_texto(re.sub(r"\{\}", "", chave)):
        return None
    novo = linha[:i] + "tr!(" + '"' + chave + '"' + "".join(", " + a for a in novos) + ")" + linha[j + 1:]
    return novo, chave


def dividir_args(s):
    partes, nivel, atual, dentro = [], 0, "", False
    for c in s:
        if dentro:
            atual += c
            if c == '"':
                dentro = False
            continue
        if c == '"':
            dentro = True
        elif c in "([{":
            nivel += 1
        elif c in ")]}":
            nivel -= 1
        elif c == "," and nivel == 0:
            partes.append(atual); atual = ""; continue
        atual += c
    if atual.strip():
        partes.append(atual)
    return partes


def arquivos_rs(pasta):
    for base, dirs, arqs in os.walk(pasta):
        dirs[:] = [d for d in dirs if d != "target"]
        for a in sorted(arqs):
            if a.endswith(".rs"):
                yield os.path.join(base, a)


def varrer(pasta):
    """(arquivo, nº da linha, texto, linha) dos candidatos; e os textos da lógica."""
    cands, logica = [], set()
    for arq in arquivos_rs(pasta):
        teste = False
        for n, linha in enumerate(open(arq, encoding="utf-8").read().split("\n"), 1):
            t = linha.strip()
            if t.startswith("#[cfg(test)]"):
                teste = True
            if teste or t.startswith("//") or t.startswith("///"):
                continue
            # Textos que aparecem em comparação ou como padrão de match são chave.
            for m in LIT.finditer(linha):
                depois = linha[m.end():m.end() + 6]
                if LOGICA.search(linha) or re.match(r"\s*(=>|\|)", depois) or re.search(r'\|\s*$', linha[:m.start()]):
                    logica.add(m.group(1))
            if PULAR.search(linha) or "tr!(" in linha and 'tr!("' not in linha:
                continue
            fmt = converte_format(linha)
            if fmt:
                cands.append((arq, n, fmt[1], linha, True))
                continue
            for m in LIT.finditer(linha):
                s = m.group(1)
                antes = linha[:m.start()]
                if antes.endswith("tr!(") or antes.rstrip().endswith("format!(") or antes.rstrip().endswith("write!("):
                    continue
                if parece_texto(s):
                    cands.append((arq, n, s, linha, False))
    return cands, logica


def candidatos(pasta):
    en = le_po()
    cands, logica = varrer(pasta)
    novos, suspeitos = [], []
    for arq, n, s, linha, fmt in cands:
        if s in logica:
            suspeitos.append(f"{os.path.relpath(arq, RAIZ)}:{n}: {s!r}  |  {linha.strip()[:110]}")
        elif s not in en and decodifica(s) not in en and s not in novos:
            novos.append(s)
    print(json.dumps({s: "" for s in novos}, ensure_ascii=False, indent=0))
    if suspeitos:
        print(f"\n# {len(suspeitos)} suspeitos (o texto também é chave da lógica):", file=sys.stderr)
        for x in suspeitos:
            print("#", x, file=sys.stderr)


def decodifica(raw):
    """Texto como o Rust o entende a partir do literal do código (\\n, \\", \\\\)."""
    out, i = "", 0
    while i < len(raw):
        c = raw[i]
        if c == "\\" and i + 1 < len(raw):
            n = raw[i + 1]
            out += {"n": "\n", "t": "\t", "r": "\r", '"': '"', "\\": "\\", "'": "'"}.get(n, n)
            i += 2
            continue
        out += c
        i += 1
    return out


def aplicar(pasta, dic):
    # As chaves vêm como estão no código (cru); o .po guarda o texto decodificado.
    novos = [(decodifica(k), decodifica(v) if "\\" in v else v) for k, v in dic.items() if v]
    registra(novos)
    en = {k: v for k, v in le_po().items()}
    crus = {k for k in dic if dic[k]} | {k for k in en}
    cands, logica = varrer(pasta)
    porarq = {}
    for arq, n, s, linha, fmt in cands:
        if s in logica or (s not in en and decodifica(s) not in en):
            continue
        porarq.setdefault(arq, []).append((n, s, fmt))
    total = 0
    for arq, itens in porarq.items():
        linhas = open(arq, encoding="utf-8").read().split("\n")
        for n, s, fmt in itens:
            i = n - 1
            if fmt:
                r = converte_format(linhas[i])
                if r and r[1] == s:
                    linhas[i] = r[0]; total += 1
            else:
                lit = '"' + s + '"'
                nova = re.sub(r"(?<!tr!\()" + re.escape(lit), lambda m: f"tr!({lit})", linhas[i])
                if nova != linhas[i]:
                    linhas[i] = nova; total += 1
        texto = "\n".join(linhas)
        if "tr!(" in texto and not re.search(r"^use (glove_config|glove_i18n|crate)::(tr|\{[^}]*\btr\b)", texto, re.M):
            ls = texto.split("\n")
            j = 0
            while j < len(ls) and (ls[j].startswith("//!") or ls[j].startswith("#![") or ls[j].strip() == ""):
                j += 1
            ls.insert(j, "use glove_config::tr;")
            texto = "\n".join(ls)
        open(arq, "w", encoding="utf-8").write(texto)
    print(f"{total} linhas marcadas em {len(porarq)} arquivos")


if __name__ == "__main__" and len(sys.argv) >= 3 and sys.argv[1] in ("candidatos", "aplicar"):
    pasta = sys.argv[2] if os.path.isabs(sys.argv[2]) else os.path.join(RAIZ, sys.argv[2])
    if sys.argv[1] == "candidatos":
        candidatos(pasta)
    else:
        aplicar(pasta, json.load(open(sys.argv[3], encoding="utf-8")))


def consertar(pacote):
    """Compila o pacote e desfaz tr!(…) nas linhas que o compilador recusa
    (constantes e padrões de match). Lista o que foi desfeito."""
    import subprocess
    desfeitas = []
    for _ in range(5):
        r = subprocess.run(["nice", "-n", "10", "cargo", "build", "--release", "--offline", "-j", "4", "-p", pacote],
                           cwd=RAIZ, capture_output=True, text=True)
        alvos = set()
        bloco = ""
        for linha in r.stderr.split("\n"):
            if linha.startswith("error"):
                bloco = linha
            m = re.match(r"\s*--> (\S+):(\d+):\d+", linha)
            if m and ("E0015" in bloco or "E0532" in bloco or "E0658" in bloco or "pattern" in bloco):
                alvos.add((m.group(1), int(m.group(2))))
                bloco = ""
        if not alvos:
            break
        for arq, n in alvos:
            p = os.path.join(RAIZ, arq)
            ls = open(p, encoding="utf-8").read().split("\n")
            novo = re.sub(r'tr!\(("(?:[^"\\]|\\.)*")\)', r"\1", ls[n - 1])
            if novo != ls[n - 1]:
                desfeitas.append(f"{arq}:{n}: {novo.strip()[:140]}")
                ls[n - 1] = novo
                open(p, "w", encoding="utf-8").write("\n".join(ls))
    erros = [l for l in r.stderr.split("\n") if l.startswith("error")]
    for d in desfeitas:
        print("desfeito:", d)
    print("compilou" if r.returncode == 0 else f"ainda com {len(erros)} erros:\n" + "\n".join(erros[:10]))


if __name__ == "__main__" and len(sys.argv) >= 3 and sys.argv[1] == "consertar":
    consertar(sys.argv[2])
