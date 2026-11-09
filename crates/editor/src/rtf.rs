//! RTF: gravar e ler documentos ricos no formato do TextEdit (que o Word e o
//! LibreOffice também abrem). O leitor entende o que o Editor usa (fontes,
//! cores, estilos de caractere e de parágrafo, papel e margens) e pula com
//! segurança o resto (imagens, campos, informações…).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::rich::{Align, CharStyle, Doc, HeaderFooter, List, Note, PageSetup, Para, ParaStyle, Role, Run, NOTE_CHAR};

/// Número do estilo no RTF de cada papel (e o nome que o Word usa).
fn role_style(r: Role) -> (u32, &'static str) {
    match r {
        Role::Body => (0, "Normal"),
        Role::Chapter => (1, "heading 1"),
        Role::Section => (2, "heading 2"),
        Role::Subsection => (3, "heading 3"),
        Role::Title => (4, "Title"),
        Role::Quote => (5, "Quote"),
        Role::Toc => (6, "toc 1"),
    }
}

/// Papel pelo nome do estilo (o do Word, do LibreOffice ou o nosso).
fn role_of_name(name: &str) -> Role {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "heading 1" | "título 1" | "titulo 1" => Role::Chapter,
        "heading 2" | "título 2" | "titulo 2" => Role::Section,
        "heading 3" | "título 3" | "titulo 3" => Role::Subsection,
        "title" | "título" | "titulo" => Role::Title,
        "quote" | "quotation" | "quotations" | "citação" | "citacao" => Role::Quote,
        n if n.starts_with("toc ") || n.starts_with("contents ") => Role::Toc,
        _ => Role::Body,
    }
}

/// Tabela de listas do Word: 1 = marcadores, 2 = numerada (9 níveis cada).
fn list_table() -> String {
    let mut s = String::from("{\\*\\listtable");
    for (id, numbered) in [(1, false), (2, true)] {
        let _ = write!(s, "{{\\list\\listtemplateid{id}\\listhybrid");
        for lv in 0..9 {
            let li = tw(crate::rich::LIST_INDENT * (lv as f32 + 1.0));
            if numbered {
                let _ = write!(s, "{{\\listlevel\\levelnfc0\\leveljc0\\levelfollow0\\levelstartat1{{\\leveltext\\'02\\'0{lv}.;}}{{\\levelnumbers\\'01;}}\\fi-{}\\li{li}}}", tw(crate::rich::LIST_INDENT));
            } else {
                let bullet = [8226, 9702, 9642][lv % 3];
                let _ = write!(s, "{{\\listlevel\\levelnfc23\\leveljc0\\levelfollow0\\levelstartat1{{\\leveltext\\'01\\u{bullet} ?;}}{{\\levelnumbers;}}\\fi-{}\\li{li}}}", tw(crate::rich::LIST_INDENT));
            }
        }
        let _ = write!(s, "\\listid{id}}}");
    }
    s.push_str("}\n{\\*\\listoverridetable{\\listoverride\\listid1\\listoverridecount0\\ls1}{\\listoverride\\listid2\\listoverridecount0\\ls2}}\n");
    s
}

/// Pontos → twips (1/20 de ponto).
fn tw(pt: f32) -> i32 {
    (pt * 20.0).round() as i32
}

fn escape(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\t' => out.push_str("\\tab "),
            c if (c as u32) < 0x80 => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{}?", *u as i16);
                }
            }
        }
    }
}

/// Texto do cabeçalho/rodapé com os campos do Word no lugar de {página} etc.
fn write_fields(out: &mut String, s: &str) {
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        escape(out, &rest[..i]);
        let tail = &rest[i..];
        let field = [("{página}", "PAGE", "1"), ("{páginas}", "NUMPAGES", "1"), ("{data}", "DATE \\\\@ \"dd/MM/yyyy\"", ""), ("{título}", "TITLE", "")].into_iter().find(|(k, _, _)| tail.starts_with(k));
        match field {
            Some((k, inst, res)) => {
                let _ = write!(out, "{{\\field{{\\*\\fldinst {inst}}}{{\\fldrslt {res}}}}}");
                rest = &tail[k.len()..];
            }
            None => {
                escape(out, "{");
                rest = &tail[1..];
            }
        }
    }
    escape(out, rest);
}

/// O documento em RTF.
pub fn write(doc: &Doc) -> String {
    // Tabelas de fontes e cores (na ordem em que aparecem).
    let mut fonts: Vec<String> = Vec::new();
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let font_ix = |fonts: &mut Vec<String>, f: &str| match fonts.iter().position(|x| x == f) {
        Some(i) => i,
        None => {
            fonts.push(f.to_string());
            fonts.len() - 1
        }
    };
    let color_ix = |colors: &mut Vec<[u8; 3]>, c: [u8; 4]| {
        let c = [c[0], c[1], c[2]];
        match colors.iter().position(|x| *x == c) {
            Some(i) => i + 1,
            None => {
                colors.push(c);
                colors.len()
            }
        }
    };
    let mut body = String::new();
    let mut counters = [0usize; 9];
    for (pi, p) in doc.paras.iter().enumerate() {
        if pi > 0 {
            body.push_str("\\par\n");
        }
        let ps = &p.style;
        body.push_str("\\pard");
        let (sn, _) = role_style(ps.role);
        if sn != 0 {
            let _ = write!(body, "\\s{sn}");
        }
        if let Some(l) = ps.role.outline() {
            let _ = write!(body, "\\outlinelevel{l}");
        }
        body.push_str(match ps.align {
            Align::Left => "\\ql",
            Align::Center => "\\qc",
            Align::Right => "\\qr",
            Align::Justify => "\\qj",
        });
        if ps.first_indent != 0.0 {
            let _ = write!(body, "\\fi{}", tw(ps.first_indent));
        }
        if ps.left_indent != 0.0 {
            let _ = write!(body, "\\li{}", tw(ps.left_indent));
        }
        if ps.right_indent != 0.0 {
            let _ = write!(body, "\\ri{}", tw(ps.right_indent));
        }
        if ps.space_before != 0.0 {
            let _ = write!(body, "\\sb{}", tw(ps.space_before));
        }
        if ps.space_after != 0.0 {
            let _ = write!(body, "\\sa{}", tw(ps.space_after));
        }
        if (ps.line_spacing - 1.0).abs() > 0.001 {
            let _ = write!(body, "\\sl{}\\slmult1", (240.0 * ps.line_spacing).round() as i32);
        }
        for t in &ps.tabs {
            let _ = write!(body, "\\tx{}", tw(*t));
        }
        if ps.page_break_before {
            body.push_str("\\pagebb");
        }
        // Item de lista: o do Word (\\ls), o marcador para quem não entende listas e o nosso.
        match ps.list {
            Some(l) => {
                let lv = (l.level as usize).min(8);
                for c in counters.iter_mut().skip(lv + 1) {
                    *c = 0;
                }
                let label = if l.numbered {
                    counters[lv] += 1;
                    format!("{}.", counters[lv])
                } else {
                    ["•", "◦", "▪"][lv % 3].to_string()
                };
                let _ = write!(body, "\\ls{}\\ilvl{lv}{{\\listtext\\pard\\plain ", if l.numbered { 2 } else { 1 });
                escape(&mut body, &label);
                let _ = write!(body, "\\tab}}{{\\*\\glovelist\\glovenum{}\\glovelvl{lv}}}", u8::from(l.numbered));
            }
            None => counters = [0; 9],
        }
        for (a, b, st) in p.spans() {
            // Marca de nota: a nota inteira entra aqui (o Word numera sozinho).
            if let Some(id) = st.note {
                let text = doc.note_text(id).unwrap_or("");
                let f = font_ix(&mut fonts, &st.family);
                let _ = write!(body, "{{\\plain\\f{f}\\fs{}\\super\\chftn}}{{\\footnote\\pard\\plain\\f{f}\\fs18 {{\\super\\chftn}} ", (st.size * 2.0).round() as i32);
                for (k, line) in text.split('\n').enumerate() {
                    if k > 0 {
                        body.push_str("\\par ");
                    }
                    escape(&mut body, line);
                }
                body.push('}');
                let _ = (a, b);
                continue;
            }
            let f = font_ix(&mut fonts, &st.family);
            let _ = write!(body, "\\plain\\f{}\\fs{}", f, (st.size * 2.0).round() as i32);
            if st.bold {
                body.push_str("\\b");
            }
            if st.italic {
                body.push_str("\\i");
            }
            if st.underline {
                body.push_str("\\ul");
            }
            if st.strike {
                body.push_str("\\strike");
            }
            match st.script {
                1 => body.push_str("\\super"),
                -1 => body.push_str("\\sub"),
                _ => {}
            }
            if st.color != [0, 0, 0, 255] {
                let _ = write!(body, "\\cf{}", color_ix(&mut colors, st.color));
            }
            if let Some(h) = st.highlight {
                let i = color_ix(&mut colors, h);
                let _ = write!(body, "\\highlight{i}\\cb{i}");
            }
            body.push(' ');
            escape(&mut body, &p.text[a..b]);
        }
    }
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0\n{\\fonttbl");
    if fonts.is_empty() {
        fonts.push(crate::rich::DEFAULT_FAMILY.into());
    }
    for (i, f) in fonts.iter().enumerate() {
        let _ = write!(out, "{{\\f{i}\\fnil\\fcharset0 ");
        escape(&mut out, f);
        out.push_str(";}");
    }
    out.push_str("}\n");
    // Estilos (os nomes do Word, para os títulos entrarem na navegação dele).
    out.push_str("{\\stylesheet");
    for r in [Role::Body, Role::Chapter, Role::Section, Role::Subsection, Role::Title, Role::Quote, Role::Toc] {
        let (n, name) = role_style(r);
        let lvl = r.outline().map(|l| format!("\\outlinelevel{l}")).unwrap_or_default();
        let _ = write!(out, "{{\\s{n}{lvl} {name};}}");
    }
    out.push_str("}\n");
    if doc.paras.iter().any(|p| p.style.list.is_some()) {
        out.push_str(&list_table());
    }
    out.push_str("{\\colortbl;");
    for c in &colors {
        let _ = write!(out, "\\red{}\\green{}\\blue{};", c[0], c[1], c[2]);
    }
    out.push_str("}\n");
    let pg = &doc.page;
    let _ = writeln!(out, "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}\\viewkind1", tw(pg.width), tw(pg.height), tw(pg.margin_left), tw(pg.margin_right), tw(pg.margin_top), tw(pg.margin_bottom));
    if doc.title_page {
        out.push_str("\\titlepg\n");
    }
    let col = pg.width - pg.margin_left - pg.margin_right;
    for (word, hf) in [("header", &doc.header), ("footer", &doc.footer)] {
        if hf.is_empty() {
            continue;
        }
        // O do Word: esquerda, centro e direita separados por tabulações.
        let _ = write!(out, "{{\\{word}\\pard\\plain\\tqc\\tx{}\\tqr\\tx{}\\f0\\fs20 ", tw(col / 2.0), tw(col));
        for (k, part) in [&hf.left, &hf.center, &hf.right].iter().enumerate() {
            if k > 0 {
                out.push_str("\\tab ");
            }
            write_fields(&mut out, part);
        }
        out.push_str("\\par}\n");
        // O do Editor (exato), que os outros programas ignoram.
        let _ = write!(out, "{{\\*\\glove{word}");
        for (tag, part) in [("left", &hf.left), ("center", &hf.center), ("right", &hf.right)] {
            let _ = write!(out, "{{\\*\\glove{tag} ");
            escape(&mut out, part);
            out.push('}');
        }
        out.push_str("}\n");
    }
    out.push_str(&body);
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Leitura
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Dest {
    Text,
    Skip,
    FontTbl,
    ColorTbl,
    /// Texto de uma nota de rodapé.
    Note,
    /// Tabela de estilos (número → nome).
    StyleSheet,
    /// Texto do marcador de um item de lista do Word.
    ListText,
    /// Nossa marca de lista.
    GloveList,
    /// Modelo de cabeçalho (0) ou rodapé (1) do Editor; e a parte (esquerda, centro, direita).
    Hf(u8),
    HfPart(u8, u8),
}

#[derive(Clone)]
struct State {
    dest: Dest,
    cs: CharStyle,
    ps: ParaStyle,
    /// Quantos caracteres de reserva seguem um \u.
    uc: usize,
    /// Número da fonte sendo definida (tabela de fontes).
    font_def: i32,
}

/// Caracteres 0x80–0x9F da página de código 1252 (o resto é igual ao Latin-1).
const CP1252: [u16; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

fn cp1252(b: u8) -> char {
    if (0x80..0xA0).contains(&b) {
        char::from_u32(CP1252[(b - 0x80) as usize] as u32).unwrap_or('?')
    } else {
        b as char
    }
}

struct Reader {
    paras: Vec<Para>,
    text: String,
    runs: Vec<Run>,
    page: PageSetup,
    fonts: HashMap<i32, String>,
    font_name: String,
    colors: Vec<Option<[u8; 3]>>,
    rgb: [u8; 3],
    rgb_set: bool,
    deff: i32,
    /// Caracteres de reserva ainda a pular (depois de \u).
    skip: usize,
    /// Metade alta de um par UTF-16 (caractere fora do plano básico).
    high: Option<u16>,
    /// O próximo parágrafo começa numa página nova (\page).
    page_break: bool,
    /// Estilo de parágrafo do texto que está sendo juntado.
    cur_ps: ParaStyle,
    notes: Vec<Note>,
    note_text: String,
    styles: HashMap<i32, String>,
    style_def: i32,
    style_name: String,
    /// Lista do parágrafo atual: a nossa (exata) ou a do Word (pelo marcador).
    list: Option<List>,
    glove_list: bool,
    listtext: String,
    list_numbered: Option<bool>,
    ilvl: u8,
    hf: [[String; 3]; 2],
    title_page: bool,
}

impl Reader {
    fn push_char(&mut self, st: &State, c: char) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        match st.dest {
            Dest::FontTbl => {
                if c == ';' {
                    let name = self.font_name.trim().to_string();
                    if !name.is_empty() {
                        self.fonts.insert(st.font_def, name);
                    }
                    self.font_name.clear();
                } else {
                    self.font_name.push(c);
                }
            }
            Dest::ColorTbl => {
                if c == ';' {
                    self.colors.push(self.rgb_set.then_some(self.rgb));
                    self.rgb = [0, 0, 0];
                    self.rgb_set = false;
                }
            }
            Dest::Skip | Dest::Hf(_) | Dest::GloveList => {}
            Dest::ListText => self.listtext.push(c),
            Dest::StyleSheet => {
                if c == ';' {
                    let name = self.style_name.trim().to_string();
                    self.styles.insert(self.style_def, name);
                    self.style_name.clear();
                } else {
                    self.style_name.push(c);
                }
            }
            Dest::Note => self.note_text.push(c),
            Dest::HfPart(w, k) => self.hf[w as usize][k as usize].push(c),
            Dest::Text => {
                if self.text.is_empty() && self.runs.is_empty() {
                    self.cur_ps = st.ps.clone();
                }
                let style = &st.cs;
                match self.runs.last_mut() {
                    Some(r) if *r.style == *style => r.len += c.len_utf8(),
                    _ => self.runs.push(Run { len: c.len_utf8(), style: Rc::new(style.clone()) }),
                }
                self.text.push(c);
                // Estilo do parágrafo: o que vale quando o texto dele aparece.
                self.cur_ps = st.ps.clone();
                self.cur_ps.list = self.list;
            }
        }
    }

    fn end_para(&mut self, st: &State) {
        let text = std::mem::take(&mut self.text);
        let mut runs = std::mem::take(&mut self.runs);
        let mut ps = if text.is_empty() { ParaStyle { list: self.list, ..st.ps.clone() } } else { self.cur_ps.clone() };
        if std::mem::take(&mut self.page_break) {
            ps.page_break_before = true;
        }
        // Lista: o marcador fica no recuo (sem recuo negativo da primeira linha).
        if ps.list.is_some() && ps.first_indent < 0.0 {
            ps.first_indent = 0.0;
        }
        if runs.is_empty() {
            runs.push(Run { len: 0, style: Rc::new(st.cs.clone()) });
        }
        let mut p = Para::new("", runs[0].style.clone(), ps);
        p.text = text;
        p.runs = runs;
        self.paras.push(p);
    }
}

/// Lê um RTF. Devolve os parágrafos e o papel.
/// O que um RTF traz.
pub struct Read {
    pub paras: Vec<Para>,
    pub page: PageSetup,
    pub notes: Vec<Note>,
    pub header: HeaderFooter,
    pub footer: HeaderFooter,
    pub title_page: bool,
}

pub fn read(src: &[u8]) -> Result<Read, String> {
    let start = src.iter().position(|&b| b == b'{').ok_or("não é um RTF")?;
    if !src[start..].starts_with(b"{\\rtf") {
        return Err("não é um RTF".into());
    }
    let base = State { dest: Dest::Text, cs: CharStyle::default(), ps: ParaStyle::default(), uc: 1, font_def: 0 };
    let mut stack: Vec<State> = Vec::new();
    let mut st = base.clone();
    let mut r = Reader {
        paras: Vec::new(),
        text: String::new(),
        runs: Vec::new(),
        page: PageSetup::default(),
        fonts: HashMap::new(),
        font_name: String::new(),
        colors: Vec::new(),
        rgb: [0, 0, 0],
        rgb_set: false,
        deff: 0,
        skip: 0,
        high: None,
        page_break: false,
        cur_ps: ParaStyle::default(),
        notes: Vec::new(),
        note_text: String::new(),
        styles: HashMap::new(),
        style_def: 0,
        style_name: String::new(),
        list: None,
        glove_list: false,
        listtext: String::new(),
        list_numbered: None,
        ilvl: 0,
        hf: Default::default(),
        title_page: false,
    };
    // Fonte padrão por número (a tabela pode vir depois de \deff).
    let font_of = |r: &Reader, n: i32| r.fonts.get(&n).cloned();
    let color_of = |r: &Reader, n: i32| -> Option<[u8; 3]> { r.colors.get(n.max(0) as usize).copied().flatten() };
    let mut i = start;
    // Grupo começando: o primeiro controle decide se é um destino a pular.
    let mut group_start = false;
    let mut star = false;
    while i < src.len() {
        let b = src[i];
        match b {
            b'{' => {
                stack.push(st.clone());
                group_start = true;
                star = false;
                i += 1;
                continue;
            }
            b'}' => {
                if st.dest == Dest::FontTbl && !r.font_name.trim().is_empty() {
                    let name = r.font_name.trim().to_string();
                    r.fonts.insert(st.font_def, name);
                    r.font_name.clear();
                }
                let closing = st.dest;
                st = stack.pop().unwrap_or_else(|| base.clone());
                r.skip = 0;
                // Fim do marcador de uma lista do Word: número ou símbolo?
                if closing == Dest::ListText && st.dest != Dest::ListText {
                    let t = std::mem::take(&mut r.listtext);
                    let t = t.trim();
                    r.list_numbered = Some(t.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()));
                    if !r.glove_list {
                        r.list = Some(List { numbered: r.list_numbered.unwrap_or(false), level: r.ilvl });
                    }
                }
                // Fim de uma nota: a marca entra no texto, onde a nota estava.
                if closing == Dest::Note && st.dest != Dest::Note {
                    let id = r.notes.len() as u32 + 1;
                    let text = std::mem::take(&mut r.note_text).trim().to_string();
                    r.notes.push(Note { id, text });
                    if st.dest == Dest::Text {
                        let mut s2 = st.clone();
                        s2.cs.note = Some(id);
                        s2.cs.script = 0;
                        r.push_char(&s2, NOTE_CHAR);
                    }
                }
                group_start = false;
                i += 1;
                continue;
            }
            b'\\' => {
                i += 1;
                let Some(&c) = src.get(i) else { break };
                if c.is_ascii_alphabetic() {
                    let s = i;
                    while i < src.len() && src[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word = std::str::from_utf8(&src[s..i]).unwrap_or("");
                    let ns = i;
                    if i < src.len() && (src[i] == b'-' || src[i].is_ascii_digit()) {
                        i += 1;
                        while i < src.len() && src[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let param: Option<i32> = std::str::from_utf8(&src[ns..i]).ok().and_then(|p| p.parse().ok());
                    if i < src.len() && src[i] == b' ' {
                        i += 1;
                    }
                    let first = std::mem::take(&mut group_start);
                    control(&mut r, &mut st, word, param, first, star, &font_of, &color_of);
                    star = false;
                } else {
                    i += 1;
                    match c {
                        b'*' => {
                            star = true;
                            // \* sem destino conhecido: pula o grupo inteiro.
                            continue;
                        }
                        b'\'' => {
                            let hex = std::str::from_utf8(src.get(i..i + 2).unwrap_or(b"3f")).unwrap_or("3f");
                            i += 2;
                            if let Ok(v) = u8::from_str_radix(hex, 16) {
                                let ch = cp1252(v);
                                r.push_char(&st, ch);
                            }
                        }
                        b'~' => r.push_char(&st, '\u{a0}'),
                        b'_' => r.push_char(&st, '-'),
                        b'-' => {}
                        b'\n' | b'\r' => {
                            if st.dest == Dest::Text {
                                r.end_para(&st);
                            }
                        }
                        other => r.push_char(&st, other as char),
                    }
                    group_start = false;
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                // Texto comum (bytes UTF-8 crus também são aceitos).
                let s = i;
                while i < src.len() && !matches!(src[i], b'{' | b'}' | b'\\' | b'\r' | b'\n') {
                    i += 1;
                }
                let chunk = &src[s..i];
                let text: String = match std::str::from_utf8(chunk) {
                    Ok(t) => t.to_string(),
                    Err(_) => chunk.iter().map(|&b| cp1252(b)).collect(),
                };
                for ch in text.chars() {
                    r.push_char(&st, ch);
                }
                group_start = false;
            }
        }
    }
    // O último parágrafo (sem \par no fim).
    if !r.text.is_empty() || r.paras.is_empty() || !r.runs.is_empty() {
        let s = st.clone();
        r.end_para(&s);
    }
    let _ = r.deff;
    let hf = |a: &[String; 3]| HeaderFooter { left: a[0].clone(), center: a[1].clone(), right: a[2].clone() };
    Ok(Read { header: hf(&r.hf[0]), footer: hf(&r.hf[1]), paras: r.paras, page: r.page, notes: r.notes, title_page: r.title_page })
}

#[allow(clippy::too_many_arguments)]
fn control(r: &mut Reader, st: &mut State, word: &str, param: Option<i32>, first: bool, star: bool, font_of: &dyn Fn(&Reader, i32) -> Option<String>, color_of: &dyn Fn(&Reader, i32) -> Option<[u8; 3]>) {
    let n = param.unwrap_or(0);
    let on = param != Some(0);
    let pt = |t: i32| t as f32 / 20.0;
    if first {
        // Destinos: o que o grupo é.
        match word {
            "fonttbl" => {
                st.dest = Dest::FontTbl;
                return;
            }
            "colortbl" => {
                st.dest = Dest::ColorTbl;
                return;
            }
"footnote" => {
                st.dest = Dest::Note;
                return;
            }
            "stylesheet" => {
                st.dest = Dest::StyleSheet;
                return;
            }
            "listtext" => {
                st.dest = Dest::ListText;
                return;
            }
            "glovelist" => {
                st.dest = Dest::GloveList;
                r.glove_list = true;
                r.list = Some(List { numbered: false, level: 0 });
                return;
            }
            "gloveheader" | "glovefooter" => {
                st.dest = Dest::Hf(u8::from(word == "glovefooter"));
                return;
            }
            "gloveleft" | "glovecenter" | "gloveright" => {
                if let Dest::Hf(w) = st.dest {
                    st.dest = Dest::HfPart(w, match word {
                        "gloveleft" => 0,
                        "glovecenter" => 1,
                        _ => 2,
                    });
                }
                return;
            }
            "info" | "pict" | "object" | "header" | "headerl" | "headerr" | "headerf" | "footer" | "footerl" | "footerr" | "footerf" | "listtable" | "listoverridetable" | "revtbl" | "pntext" | "pn" | "xmlnstbl" | "rsidtbl" | "generator" | "fldinst" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "bkmkstart" | "bkmkend" => {
                st.dest = Dest::Skip;
                return;
            }
            _ if star => {
                st.dest = Dest::Skip;
                return;
            }
            _ => {}
        }
    }
    if st.dest == Dest::GloveList {
        if let Some(l) = r.list.as_mut() {
            match word {
                "glovenum" => l.numbered = n == 1,
                "glovelvl" => l.level = n.clamp(0, 8) as u8,
                _ => {}
            }
        }
        return;
    }
    if st.dest == Dest::StyleSheet {
        if word == "s" {
            r.style_def = n;
            r.style_name.clear();
        }
        return;
    }
    if matches!(st.dest, Dest::Skip | Dest::Hf(_)) {
        return;
    }
    // Dentro de nota ou do modelo do cabeçalho: só o texto (e as quebras).
    if matches!(st.dest, Dest::Note | Dest::HfPart(..)) {
        match word {
            "par" | "line" => r.push_char(st, '\n'),
            "tab" => r.push_char(st, '\t'),
            "u" => {
                let v = if n < 0 { (n + 65536) as u16 } else { n as u16 };
                if let Some(ch) = char::from_u32(v as u32) {
                    r.push_char(st, ch);
                }
                r.skip = st.uc;
            }
            _ => {}
        }
        return;
    }
    if st.dest == Dest::ColorTbl {
        match word {
            "red" => r.rgb[0] = n.clamp(0, 255) as u8,
            "green" => r.rgb[1] = n.clamp(0, 255) as u8,
            "blue" => r.rgb[2] = n.clamp(0, 255) as u8,
            _ => {}
        }
        if matches!(word, "red" | "green" | "blue") {
            r.rgb_set = true;
        }
        return;
    }
    if st.dest == Dest::FontTbl {
        if word == "f" {
            st.font_def = n;
            r.font_name.clear();
        }
        return;
    }
    match word {
        "deff" => r.deff = n,
        "titlepg" => r.title_page = true,
        // O número automático da nota (a marca já vem pela própria nota).
        "chftn" => {}
        "paperw" => r.page.width = pt(n),
        "paperh" => r.page.height = pt(n),
        "margl" => r.page.margin_left = pt(n),
        "margr" => r.page.margin_right = pt(n),
        "margt" => r.page.margin_top = pt(n),
        "margb" => r.page.margin_bottom = pt(n),
        "uc" => st.uc = n.max(0) as usize,
        "u" => {
            let v = if n < 0 { (n + 65536) as u16 } else { n as u16 };
            let ch = if (0xD800..0xDC00).contains(&v) {
                r.high = Some(v);
                None
            } else if (0xDC00..0xE000).contains(&v) {
                r.high.take().and_then(|h| char::decode_utf16([h, v]).next()?.ok())
            } else {
                char::from_u32(v as u32)
            };
            if let Some(ch) = ch {
                r.push_char(st, ch);
            }
            r.skip = st.uc;
        }
        "par" | "line" => r.end_para(st),
        "page" => {
            if !r.text.is_empty() {
                r.end_para(st);
            }
            r.page_break = true;
        }
        "tab" => r.push_char(st, '\t'),
        "emdash" => r.push_char(st, '—'),
        "endash" => r.push_char(st, '–'),
        "bullet" => r.push_char(st, '•'),
        "lquote" => r.push_char(st, '‘'),
        "rquote" => r.push_char(st, '’'),
        "ldblquote" => r.push_char(st, '“'),
        "rdblquote" => r.push_char(st, '”'),
        "pard" => {
            let cell = st.ps.cell;
            st.ps = ParaStyle { cell, ..ParaStyle::default() };
            r.list = None;
            r.glove_list = false;
            r.list_numbered = None;
            r.ilvl = 0;
        }
        "s" => st.ps.role = r.styles.get(&n).map(|nm| role_of_name(nm)).unwrap_or(Role::Body),
        "outlinelevel" => {
            if st.ps.role == Role::Body {
                st.ps.role = match n {
                    0 => Role::Chapter,
                    1 => Role::Section,
                    2 => Role::Subsection,
                    _ => Role::Body,
                };
            }
        }
        "ilvl" => {
            r.ilvl = n.clamp(0, 8) as u8;
            if let Some(l) = r.list.as_mut().filter(|_| !r.glove_list) {
                l.level = r.ilvl;
            }
        }
        "ls" => {
            if !r.glove_list {
                r.list = Some(List { numbered: r.list_numbered.unwrap_or(false), level: r.ilvl });
            }
        }
        "plain" => {
            st.cs = CharStyle { family: font_of(r, r.deff).unwrap_or_else(|| crate::rich::DEFAULT_FAMILY.into()), ..CharStyle::default() };
        }
        "f" => {
            if let Some(f) = font_of(r, n) {
                st.cs.family = f;
            }
        }
        "fs" => st.cs.size = (n as f32 / 2.0).clamp(1.0, 1000.0),
        "b" => st.cs.bold = on,
        "i" => st.cs.italic = on,
        "ul" | "uld" | "uldb" | "ulw" | "ulth" => st.cs.underline = on,
        "ulnone" => st.cs.underline = false,
        "strike" | "striked" => st.cs.strike = on,
        "super" => st.cs.script = 1,
        "sub" => st.cs.script = -1,
        "nosupersub" => st.cs.script = 0,
        "up" => st.cs.script = if n > 0 { 1 } else { 0 },
        "dn" => st.cs.script = if n > 0 { -1 } else { 0 },
        "cf" => {
            st.cs.color = match color_of(r, n) {
                Some(c) => [c[0], c[1], c[2], 255],
                None => [0, 0, 0, 255],
            }
        }
        "highlight" | "cb" | "chcbpat" => st.cs.highlight = if n == 0 { None } else { color_of(r, n).map(|c| [c[0], c[1], c[2], 255]) },
        "ql" => st.ps.align = Align::Left,
        "qc" => st.ps.align = Align::Center,
        "qr" => st.ps.align = Align::Right,
        "qj" => st.ps.align = Align::Justify,
        "fi" => st.ps.first_indent = pt(n),
        "li" => st.ps.left_indent = pt(n),
        "ri" => st.ps.right_indent = pt(n),
        "sb" => st.ps.space_before = pt(n),
        "sa" => st.ps.space_after = pt(n),
        "sl" => {
            // Com \slmult1 (que vem depois) é múltiplo de 240; sem, é altura exata em twips.
            st.ps.line_spacing = if n == 0 { 1.0 } else { (n.abs() as f32 / 240.0).clamp(0.5, 5.0) };
        }
        "tx" => {
            st.ps.tabs.push(pt(n));
            st.ps.tabs.sort_by(f32::total_cmp);
        }
        "pagebb" => st.ps.page_break_before = true,
        _ => {}
    }
}

/// Lê um arquivo RTF num documento.
pub fn read_doc(src: &[u8]) -> Result<Doc, String> {
    let r = read(src)?;
    let mut d = Doc::from_paras(r.paras);
    d.page = r.page;
    d.notes = r.notes;
    d.header = r.header;
    d.footer = r.footer;
    d.title_page = r.title_page;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich::Pos;

    #[test]
    fn grava_e_le_igual() {
        let mut d = Doc::from_plain("Título do livro\nAção e emoção: “aspas” 🙂\tfim", Rc::new(CharStyle::default()), ParaStyle::default());
        d.anchor = Pos::new(0, 0);
        d.cursor = Pos::new(0, 7);
        d.set_char_style(&|s| {
            s.bold = true;
            s.size = 24.0;
            s.family = "Times New Roman".into();
        });
        d.anchor = Pos::new(1, 0);
        d.cursor = Pos::new(1, 6);
        d.set_char_style(&|s| {
            s.italic = true;
            s.color = [200, 0, 0, 255];
            s.highlight = Some([255, 240, 110, 255]);
        });
        d.set_para_style(&|s| {
            s.align = Align::Justify;
            s.first_indent = 35.5;
            s.line_spacing = 1.5;
            s.space_after = 6.0;
            s.tabs = vec![72.0, 144.0];
        });
        d.page.margin_left = 85.05;
        let rtf = write(&d);
        let back = read_doc(rtf.as_bytes()).unwrap();
        assert_eq!(back.plain_text(), d.plain_text());
        assert_eq!(back.paras.len(), 2);
        let s0 = back.paras[0].style_after(0);
        assert!(s0.bold && s0.size == 24.0 && s0.family == "Times New Roman");
        assert!(!back.paras[0].style_after(7).bold);
        let s1 = back.paras[1].style_after(0);
        assert!(s1.italic && s1.color == [200, 0, 0, 255] && s1.highlight == Some([255, 240, 110, 255]));
        let p1 = &back.paras[1].style;
        assert_eq!(p1.align, Align::Justify);
        assert!((p1.first_indent - 35.5).abs() < 0.06);
        assert!((p1.line_spacing - 1.5).abs() < 0.01);
        assert_eq!(p1.tabs, vec![72.0, 144.0]);
        assert!((back.page.margin_left - 85.05).abs() < 0.06);
    }

    #[test]
    fn le_rtf_do_word() {
        let src = br#"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\froman\fcharset0 Times New Roman;}{\f1\fswiss Arial;}}
{\colortbl ;\red255\green0\blue0;}
{\*\generator Microsoft Word;}{\info{\author Fulano}}
\pard\qc\b\f1\fs32 Relat\'f3rio\b0\par
\pard\qj\fi709\f0\fs24 A\u231?\u227?o {\cf1 vermelha} e {\field{\*\fldinst HYPERLINK "x"}{\fldrslt link}}.\par
}"#;
        let d = read_doc(src).unwrap();
        assert_eq!(d.plain_text(), "Relatório\nAção vermelha e link.");
        assert_eq!(d.paras[0].style.align, Align::Center);
        assert!(d.paras[0].style_after(0).bold);
        assert_eq!(d.paras[0].style_after(0).family, "Arial");
        assert_eq!(d.paras[0].style_after(0).size, 16.0);
        assert_eq!(d.paras[1].style.align, Align::Justify);
        assert!((d.paras[1].style.first_indent - 35.45).abs() < 0.01);
        let red = d.paras[1].text.find("vermelha").unwrap();
        assert_eq!(d.paras[1].style_after(red).color, [255, 0, 0, 255]);
        assert_eq!(d.paras[1].style_after(0).family, "Times New Roman");
    }

    /// Manual: grava um RTF de exemplo em $RTF_OUT (para abrir no LibreOffice/Word).
    #[test]
    #[ignore]
    fn gravar_exemplo() {
        let Some(out) = std::env::var_os("RTF_OUT") else { return };
        let mut d = Doc::from_plain("Relatório Final\nEste parágrafo é justificado, com recuo de primeira linha e espaçamento de 1,5 entre as linhas, para conferir se o outro programa lê igual.\nTexto em vermelho e marcado.", Rc::new(CharStyle::default()), ParaStyle::default());
        d.anchor = Pos::new(0, 0);
        d.cursor = Pos::new(0, d.paras[0].text.len());
        d.set_char_style(&|s| {
            s.bold = true;
            s.size = 20.0;
        });
        d.set_para_style(&|s| s.align = Align::Center);
        d.anchor = Pos::new(1, 0);
        d.cursor = Pos::new(1, 3);
        d.set_para_style(&|s| {
            s.align = Align::Justify;
            s.first_indent = 35.4;
            s.line_spacing = 1.5;
        });
        d.anchor = Pos::new(2, 9);
        d.cursor = Pos::new(2, 21);
        d.set_char_style(&|s| s.color = [220, 0, 0, 255]);
        d.anchor = Pos::new(2, 20);
        d.cursor = Pos::new(2, 27);
        d.set_char_style(&|s| s.highlight = Some([255, 240, 110, 255]));
        d.set_cursor(Pos::new(1, 14), false);
        d.insert_note("Conforme a norma da ABNT.");
        let n = d.paras.len();
        d.set_cursor(Pos::new(n - 1, d.paras[n - 1].text.len()), false);
        d.insert_text("\nObjetivos\nPrimeiro objetivo\nDetalhe do primeiro\nSegundo objetivo\nUm marcador\nOutro marcador");
        d.set_cursor(Pos::new(n, 0), false);
        d.apply_role(Role::Chapter);
        d.anchor = Pos::new(n + 1, 0);
        d.cursor = Pos::new(n + 3, 0);
        d.set_list(Some(true));
        d.set_cursor(Pos::new(n + 2, 0), false);
        d.shift_list(1);
        d.anchor = Pos::new(n + 4, 0);
        d.cursor = Pos::new(n + 5, 0);
        d.set_list(Some(false));
        d.header.right = "Relatório — {data}".into();
        d.footer.center = "Página {página} de {páginas}".into();
        std::fs::write(out, write(&d)).unwrap();
    }

    /// Manual: lê o RTF em $RTF_IN e mostra parágrafos e estilos.
    #[test]
    #[ignore]
    fn ler_arquivo() {
        let Some(p) = std::env::var_os("RTF_IN") else { return };
        let d = read_doc(&std::fs::read(p).unwrap()).unwrap();
        for p in &d.paras {
            println!("[{:?} fi={} ls={}] {:?}", p.style.align, p.style.first_indent, p.style.line_spacing, p.text);
            for (a, b, st) in p.spans() {
                println!("    {:?} {} {} b={} i={} cor={:?} marca={:?}", &p.text[a..b], st.family, st.size, st.bold, st.italic, st.color, st.highlight);
            }
        }
        println!("papel {:?}", d.page);
    }

    #[test]
    fn notas_cabecalho_e_rodape() {
        let mut d = Doc::from_plain("Texto com nota.", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 9), false);
        d.insert_note("Fonte: IBGE, 2020.");
        d.header.right = "Meu TCC".into();
        d.footer.center = "Página {página} de {páginas}".into();
        d.title_page = true;
        let rtf = write(&d);
        assert!(rtf.contains("\\footnote"));
        assert!(rtf.contains("fldinst PAGE"));
        let back = read_doc(rtf.as_bytes()).unwrap();
        assert_eq!(back.plain_text(), d.plain_text());
        assert_eq!(back.notes.len(), 1);
        assert_eq!(back.notes[0].text, "Fonte: IBGE, 2020.");
        assert_eq!(back.note_order(), vec![back.notes[0].id]);
        assert_eq!(back.header.right, "Meu TCC");
        assert_eq!(back.footer.center, "Página {página} de {páginas}");
        assert!(back.title_page);
    }

    #[test]
    fn estilos_e_listas() {
        let mut d = Doc::from_plain("Capítulo 1\nitem um\nitem dois\nsubitem\nfim", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 0), false);
        d.apply_role(Role::Chapter);
        d.anchor = Pos::new(1, 0);
        d.cursor = Pos::new(3, 0);
        d.set_list(Some(true));
        d.set_cursor(Pos::new(3, 0), false);
        d.shift_list(1);
        let rtf = write(&d);
        assert!(rtf.contains("heading 1"));
        assert!(rtf.contains("\\listtable"));
        let back = read_doc(rtf.as_bytes()).unwrap();
        assert_eq!(back.plain_text(), d.plain_text());
        assert_eq!(back.paras[0].style.role, Role::Chapter);
        assert!(back.paras[0].style_after(0).bold);
        assert_eq!(back.paras[1].style.list, Some(List { numbered: true, level: 0 }));
        assert_eq!(back.paras[3].style.list, Some(List { numbered: true, level: 1 }));
        assert_eq!(back.paras[4].style.list, None);
    }

    #[test]
    fn le_lista_do_word() {
        let src = br#"{\rtf1\ansi{\fonttbl{\f0 Arial;}}{\stylesheet{\s0 Normal;}{\s1 heading 1;}}
\pard\s1 Intro\par
\pard{\listtext\pard\plain \'b7\tab}\ls1\ilvl0\li720\fi-360 um\par
\pard{\listtext\pard\plain 1.\tab}\ls2\ilvl1\li1440\fi-360 dois\par
\pard texto}"#;
        let d = read_doc(src).unwrap();
        assert_eq!(d.plain_text(), "Intro\num\ndois\ntexto");
        assert_eq!(d.paras[0].style.role, Role::Chapter);
        assert_eq!(d.paras[1].style.list, Some(List { numbered: false, level: 0 }));
        assert_eq!(d.paras[2].style.list, Some(List { numbered: true, level: 1 }));
        assert_eq!(d.paras[1].style.first_indent, 0.0);
        assert_eq!(d.paras[3].style.list, None);
    }

    #[test]
    fn recusa_o_que_nao_e_rtf() {
        assert!(read(b"texto comum").is_err());
    }
}
