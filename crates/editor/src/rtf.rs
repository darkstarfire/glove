//! RTF: gravar e ler documentos ricos no formato do TextEdit (que o Word e o
//! LibreOffice também abrem). O leitor entende o que o Editor usa (fontes,
//! cores, estilos de caractere e de parágrafo, papel e margens) e pula com
//! segurança o resto (imagens, campos, informações…).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::rich::{Align, CharStyle, Doc, PageSetup, Para, ParaStyle, Run};

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
    for (pi, p) in doc.paras.iter().enumerate() {
        if pi > 0 {
            body.push_str("\\par\n");
        }
        let ps = &p.style;
        body.push_str("\\pard");
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
        for (a, b, st) in p.spans() {
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
    out.push_str("}\n{\\colortbl;");
    for c in &colors {
        let _ = write!(out, "\\red{}\\green{}\\blue{};", c[0], c[1], c[2]);
    }
    out.push_str("}\n");
    let pg = &doc.page;
    let _ = writeln!(out, "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}\\viewkind1", tw(pg.width), tw(pg.height), tw(pg.margin_left), tw(pg.margin_right), tw(pg.margin_top), tw(pg.margin_bottom));
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
            Dest::Skip => {}
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
            }
        }
    }

    fn end_para(&mut self, st: &State) {
        let text = std::mem::take(&mut self.text);
        let mut runs = std::mem::take(&mut self.runs);
        let mut ps = if text.is_empty() { st.ps.clone() } else { self.cur_ps.clone() };
        if std::mem::take(&mut self.page_break) {
            ps.page_break_before = true;
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
pub fn read(src: &[u8]) -> Result<(Vec<Para>, PageSetup), String> {
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
                st = stack.pop().unwrap_or_else(|| base.clone());
                r.skip = 0;
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
    Ok((r.paras, r.page))
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
            "stylesheet" | "info" | "pict" | "object" | "header" | "headerl" | "headerr" | "headerf" | "footer" | "footerl" | "footerr" | "footerf" | "footnote" | "listtable" | "listoverridetable" | "revtbl" | "listtext" | "pntext" | "pn" | "xmlnstbl" | "rsidtbl" | "generator" | "fldinst" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "bkmkstart" | "bkmkend" => {
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
    if st.dest == Dest::Skip {
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
    let (paras, page) = read(src)?;
    let mut d = Doc::from_paras(paras);
    d.page = page;
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
    fn recusa_o_que_nao_e_rtf() {
        assert!(read(b"texto comum").is_err());
    }
}
