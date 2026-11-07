//! Traduções do Glove.
//!
//! O texto em português continua no código e serve de chave: `tr!("Abrir")` devolve
//! "Open" em inglês e "Abrir" em português. As traduções ficam em arquivos `.po`
//! (o formato do gettext): os de `locales/` vêm embutidos no programa, e outros
//! podem ser postos em `/usr/share/glove/locales` ou `~/.local/share/glove/locales`
//! sem recompilar.
//!
//! Idioma: `GLOVE_LANG`, senão `LC_ALL`, `LC_MESSAGES` e `LANG` (o painel
//! Internacional grava o `LANG` da sessão). Português usa o texto do código; um
//! idioma sem tradução cai no inglês.
//!
//! Textos com valores: `tr!("{} itens", n)` troca cada `{}` (ou `{0}`, `{1}`…) pelos
//! valores, na ordem; a tradução pode mudar a posição usando os números.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Os catálogos que vêm no programa: (idioma, conteúdo do .po).
const EMBUTIDOS: &[(&str, &str)] = &[("en", include_str!("../../../locales/en.po"))];

/// O idioma de origem (o do texto no código).
pub const ORIGEM: &str = "pt";

struct Catalogo {
    idioma: String,
    textos: HashMap<String, &'static str>,
}

fn idioma_do_ambiente() -> String {
    for v in ["GLOVE_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(s) = std::env::var(v) {
            if !s.is_empty() {
                return normaliza(&s);
            }
        }
    }
    ORIGEM.to_string()
}

/// "pt_BR.UTF-8" → "pt"; "en_US" → "en"; "C"/"POSIX" → "en".
fn normaliza(s: &str) -> String {
    let base = s.split(['.', '@']).next().unwrap_or("");
    let lang = base.split(['_', '-']).next().unwrap_or("").to_ascii_lowercase();
    match lang.as_str() {
        "" | "c" | "posix" => "en".into(),
        _ => lang,
    }
}

/// Lê um .po: pares msgid/msgstr (com as linhas de continuação). Textos vazios ou
/// marcados como "fuzzy" ficam de fora.
pub fn ler_po(texto: &str) -> Vec<(String, String)> {
    let mut pares = Vec::new();
    // `fuzzy` vale para a entrada em leitura; `proximo_fuzzy` é a marca (#, fuzzy)
    // vista nos comentários, que vale para a próxima entrada.
    let (mut id, mut s, mut campo, mut fuzzy, mut proximo_fuzzy) = (String::new(), String::new(), 0u8, false, false);
    let mut fecha = |id: &mut String, s: &mut String, fuzzy: &mut bool| {
        if !id.is_empty() && !s.is_empty() && !*fuzzy {
            pares.push((std::mem::take(id), std::mem::take(s)));
        }
        id.clear();
        s.clear();
        *fuzzy = false;
    };
    for linha in texto.lines() {
        let l = linha.trim();
        if l.starts_with("#,") && l.contains("fuzzy") {
            proximo_fuzzy = true;
        } else if let Some(r) = l.strip_prefix("msgid ") {
            fecha(&mut id, &mut s, &mut fuzzy);
            fuzzy = std::mem::take(&mut proximo_fuzzy);
            id = desescapa(r);
            campo = 1;
        } else if let Some(r) = l.strip_prefix("msgstr ") {
            s = desescapa(r);
            campo = 2;
        } else if l.starts_with('"') {
            match campo {
                1 => id.push_str(&desescapa(l)),
                2 => s.push_str(&desescapa(l)),
                _ => {}
            }
        } else if l.is_empty() {
            campo = 0;
        }
    }
    fecha(&mut id, &mut s, &mut fuzzy);
    pares
}

fn desescapa(l: &str) -> String {
    let l = l.trim();
    let l = l.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(l);
    let mut out = String::with_capacity(l.len());
    let mut it = l.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn catalogo() -> &'static Catalogo {
    static C: OnceLock<Catalogo> = OnceLock::new();
    C.get_or_init(|| carregar(&idioma_do_ambiente()))
}

fn carregar(pedido: &str) -> Catalogo {
    let mut textos = HashMap::new();
    if pedido == ORIGEM {
        return Catalogo { idioma: ORIGEM.into(), textos };
    }
    // Pastas de fora (sem recompilar) e depois o embutido.
    let mut fontes: Vec<String> = Vec::new();
    let mut pastas = vec!["/usr/share/glove/locales".to_string(), "/usr/local/share/glove/locales".to_string()];
    if let Some(home) = std::env::var_os("HOME") {
        pastas.push(format!("{}/.local/share/glove/locales", home.to_string_lossy()));
    }
    for p in pastas {
        if let Ok(t) = std::fs::read_to_string(format!("{p}/{pedido}.po")) {
            fontes.push(t);
        }
    }
    let mut idioma = pedido.to_string();
    let embutido = EMBUTIDOS.iter().find(|(l, _)| *l == pedido).map(|(_, t)| *t);
    if fontes.is_empty() && embutido.is_none() {
        // Idioma sem tradução: inglês.
        idioma = "en".into();
    }
    let embutido = embutido.or_else(|| EMBUTIDOS.iter().find(|(l, _)| *l == idioma).map(|(_, t)| *t));
    if let Some(e) = embutido {
        for (k, v) in ler_po(e) {
            textos.insert(k, &*Box::leak(v.into_boxed_str()));
        }
    }
    // Os de fora valem por cima do embutido (o último lido ganha).
    for f in fontes {
        for (k, v) in ler_po(&f) {
            textos.insert(k, &*Box::leak(v.into_boxed_str()));
        }
    }
    Catalogo { idioma, textos }
}

/// O idioma em uso ("pt", "en"…).
pub fn idioma() -> &'static str {
    &catalogo().idioma
}

/// Separador decimal do idioma em uso: vírgula (pt, es, fr, de, it…) ou ponto (en, ja…).
pub fn virgula() -> &'static str {
    match idioma() {
        "en" | "ja" | "zh" | "ko" | "he" | "th" => ".",
        _ => ",",
    }
}

/// Tradução de um texto fixo (o que `tr!` usa). Uma chave pode levar contexto depois de
/// `|` quando a mesma palavra tem dois sentidos ("Monitor" o app e "Monitor|tela"): sem
/// tradução, mostra só o que vem antes do `|`.
pub fn tr(texto: &'static str) -> &'static str {
    catalogo().textos.get(texto).copied().unwrap_or_else(|| sem_contexto(texto))
}

/// Tradução de um texto que não é fixo (vindo da configuração, por exemplo).
pub fn tr_dyn(texto: &str) -> String {
    catalogo().textos.get(texto).map(|s| s.to_string()).unwrap_or_else(|| sem_contexto(texto).to_string())
}

fn sem_contexto(texto: &str) -> &str {
    texto.split_once('|').map_or(texto, |(t, _)| t)
}

/// Troca os `{}` (na ordem) e os `{0}`, `{1}`… pelos valores.
pub fn formata(modelo: &str, valores: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(modelo.len() + 16);
    let mut seq = 0;
    let mut it = modelo.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if c == '{' {
            if let Some(fim) = modelo[i..].find('}') {
                let dentro = &modelo[i + 1..i + fim];
                let n = if dentro.is_empty() {
                    seq += 1;
                    Some(seq - 1)
                } else {
                    dentro.parse::<usize>().ok()
                };
                if let Some(v) = n.and_then(|n| valores.get(n)) {
                    out.push_str(&v.to_string());
                    for _ in 0..fim {
                        it.next();
                    }
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// `tr!("Abrir")` → &'static str traduzido; `tr!("{} itens", n)` → String.
#[macro_export]
macro_rules! tr {
    ($s:literal) => {
        $crate::tr($s)
    };
    ($s:literal, $($v:expr),+ $(,)?) => {
        $crate::formata($crate::tr($s), &[$(&$v as &dyn ::std::fmt::Display),+])
    };
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normaliza_idiomas() {
        assert_eq!(normaliza("pt_BR.UTF-8"), "pt");
        assert_eq!(normaliza("en_US.utf8"), "en");
        assert_eq!(normaliza("C"), "en");
        assert_eq!(normaliza("de_DE@euro"), "de");
    }

    #[test]
    fn le_po() {
        let po = "msgid \"\"\nmsgstr \"cabeçalho\"\n\nmsgid \"Abrir\"\nmsgstr \"Open\"\n\n#, fuzzy\nmsgid \"X\"\nmsgstr \"Y\"\n\nmsgid \"Linha \"\n\"dupla\"\nmsgstr \"Two \"\n\"lines\"\n";
        let p = ler_po(po);
        assert!(p.contains(&("Abrir".into(), "Open".into())));
        assert!(p.contains(&("Linha dupla".into(), "Two lines".into())));
        assert!(!p.iter().any(|(k, _)| k == "X"));
    }

    #[test]
    fn contexto() {
        assert_eq!(sem_contexto("Monitor|tela"), "Monitor");
        assert_eq!(sem_contexto("Abrir"), "Abrir");
    }

    #[test]
    fn formata_valores() {
        assert_eq!(formata("{} itens, {} livres", &[&3, &"9 GB"]), "3 itens, 9 GB livres");
        assert_eq!(formata("{1} de {0}", &[&"a", &"b"]), "b de a");
        assert_eq!(formata("sem {chave}", &[&1]), "sem {chave}");
    }

    /// Todo texto marcado com tr!(…) no projeto precisa ter tradução em inglês.
    #[test]
    fn todos_traduzidos_para_ingles() {
        let raiz = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let en: HashMap<String, String> = ler_po(include_str!("../../../locales/en.po")).into_iter().collect();
        let mut faltam = Vec::new();
        for chave in chaves_do_projeto(std::path::Path::new(raiz)) {
            if !en.contains_key(&chave) {
                faltam.push(chave);
            }
        }
        faltam.sort();
        faltam.dedup();
        assert!(faltam.is_empty(), "sem tradução em locales/en.po ({}):\n{}", faltam.len(), faltam.join("\n"));
    }

    /// Os textos de tr!("…") em todos os .rs de crates/.
    pub fn chaves_do_projeto(raiz: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut pilha = vec![raiz.join("crates")];
        while let Some(d) = pilha.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    // O próprio glove-i18n só tem exemplos na documentação.
                    if p.file_name().is_some_and(|n| n != "target" && n != "i18n") {
                        pilha.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        extrai(&t, &mut out);
                    }
                }
            }
        }
        out
    }

    fn extrai(t: &str, out: &mut Vec<String>) {
        let mut resto = t;
        while let Some(i) = resto.find("tr!(\"") {
            let depois = &resto[i + 5..];
            let mut s = String::new();
            let mut esc = false;
            let mut fim = 0;
            for (j, c) in depois.char_indices() {
                if esc {
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        o => o,
                    });
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    fim = j;
                    break;
                } else {
                    s.push(c);
                }
            }
            if !s.is_empty() && !s.contains("{chave}") {
                out.push(s);
            }
            resto = &depois[fim..];
        }
    }
}
