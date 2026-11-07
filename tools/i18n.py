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
    if "tr!(" in texto and not re.search(r"^use glove_config::(tr|\{[^}]*\btr\b)", texto, re.M) and "macro_rules! tr" not in texto:
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


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "marcar":
        marcar(sys.argv[2], json.load(open(sys.argv[3], encoding="utf-8")))
    elif len(sys.argv) >= 2 and sys.argv[1] == "faltam":
        faltam()
    else:
        print(__doc__)
