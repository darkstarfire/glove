//! RTF: gravar e ler documentos ricos no formato do TextEdit (que o Word e o
//! LibreOffice também abrem). O leitor entende o que o Editor usa (fontes,
//! cores, estilos de caractere e de parágrafo, papel e margens) e pula com
//! segurança o resto (imagens, campos, informações…).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::rich::{Align, Cell, CharStyle, Doc, HeaderFooter, List, Note, PageSetup, Para, ParaStyle, Role, Run, TableProps, NOTE_CHAR};

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
/// Índice da fonte na tabela de fontes (acrescenta se for nova).
fn font_ix(fonts: &mut Vec<String>, f: &str) -> usize {
    match fonts.iter().position(|x| x == f) {
        Some(i) => i,
        None => {
            fonts.push(f.to_string());
            fonts.len() - 1
        }
    }
}

/// Índice da cor na tabela de cores (a 0 é a automática).
fn color_ix(colors: &mut Vec<[u8; 3]>, c: [u8; 4]) -> usize {
    let c = [c[0], c[1], c[2]];
    match colors.iter().position(|x| *x == c) {
        Some(i) => i + 1,
        None => {
            colors.push(c);
            colors.len()
        }
    }
}

/// Um parágrafo (sem o \\par do fim). `intbl` = dentro de uma célula.
fn write_para(body: &mut String, doc: &Doc, p: &Para, intbl: bool, fonts: &mut Vec<String>, colors: &mut Vec<[u8; 3]>, counters: &mut [usize; 9]) {
    let ps = &p.style;
    body.push_str("\\pard");
    if intbl {
        body.push_str("\\intbl");
    }
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
            escape(body, &label);
            let _ = write!(body, "\\tab}}{{\\*\\glovelist\\glovenum{}\\glovelvl{lv}}}", u8::from(l.numbered));
        }
        None => *counters = [0; 9],
    }
    for (a, b, st) in p.spans() {
        // Imagem: os bytes do arquivo em hexadecimal, com o tamanho na página.
        if let Some(img) = &st.image {
            for _ in p.text[a..b].chars() {
                write_pict(body, img);
            }
            continue;
        }
        // Marca de nota: a nota inteira entra aqui (o Word numera sozinho).
        if let Some(id) = st.note {
            let text = doc.note_text(id).unwrap_or("");
            let f = font_ix(fonts, &st.family);
            let _ = write!(body, "{{\\plain\\f{f}\\fs{}\\super\\chftn}}{{\\footnote\\pard\\plain\\f{f}\\fs18 {{\\super\\chftn}} ", (st.size * 2.0).round() as i32);
            for (k, line) in text.split('\n').enumerate() {
                if k > 0 {
                    body.push_str("\\par ");
                }
                escape(body, line);
            }
            body.push('}');
            let _ = (a, b);
            continue;
        }
        let f = font_ix(fonts, &st.family);
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
            let _ = write!(body, "\\cf{}", color_ix(colors, st.color));
        }
        if let Some(h) = st.highlight {
            let i = color_ix(colors, h);
            let _ = write!(body, "\\highlight{i}\\cb{i}");
        }
        body.push(' ');
        escape(body, &p.text[a..b]);
    }
}

/// O documento em RTF.
/// Uma imagem como grupo \\pict (PNG ou JPEG, como o Word e o TextEdit gravam).
fn write_pict(body: &mut String, img: &crate::rich::Image) {
    let d = &img.data;
    let _ = write!(body, "{{\\pict{}\\picw{}\\pich{}\\picwgoal{}\\pichgoal{}\n", if d.jpeg { "\\jpegblip" } else { "\\pngblip" }, d.px.0, d.px.1, tw(img.w), tw(img.h));
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for line in d.bytes.chunks(64) {
        for &b in line {
            body.push(HEX[(b >> 4) as usize] as char);
            body.push(HEX[(b & 15) as usize] as char);
        }
        body.push('\n');
    }
    body.push('}');
}

pub fn write(doc: &Doc) -> String {
    // Tabelas de fontes e cores (na ordem em que aparecem).
    let mut fonts: Vec<String> = Vec::new();
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let mut body = String::new();
    let mut counters = [0usize; 9];
    let col_w = doc.page.width - doc.page.margin_left - doc.page.margin_right;
    let n = doc.paras.len();
    let mut need_par = false;
    let mut i = 0;
    while i < n {
        let p = &doc.paras[i];
        if let Some(c0) = p.style.cell {
            // Tabela: linha a linha (\\trowd … \\cell … \\row).
            let end = (i..n).find(|&k| doc.paras[k].style.cell.is_none_or(|c| c.table != c0.table)).unwrap_or(n);
            if need_par {
                body.push_str("\\par\n");
            }
            let cols = doc.paras[i..end].iter().filter_map(|p| p.style.cell.map(|c| (c.col + c.span.max(1)) as usize)).max().unwrap_or(1);
            let props = doc.tables.get(&c0.table).cloned().unwrap_or_default();
            let mut bounds = vec![0.0f32];
            for f in props.fractions(cols) {
                let last = *bounds.last().unwrap();
                bounds.push(last + f * col_w);
            }
            let mut k = i;
            while k < end {
                let row = doc.paras[k].style.cell.unwrap().row;
                let rend = (k..end).find(|&q| doc.paras[q].style.cell.unwrap().row != row).unwrap_or(end);
                let mut cells: Vec<(u16, u16, Vec<usize>)> = Vec::new();
                for q in k..rend {
                    let c = doc.paras[q].style.cell.unwrap();
                    match cells.last_mut() {
                        Some(cl) if cl.0 == c.col => cl.2.push(q),
                        _ => cells.push((c.col, c.span.max(1), vec![q])),
                    }
                }
                body.push_str("\\trowd\\trgaph80\\trleft0");
                for (col, span, _) in &cells {
                    if props.borders {
                        body.push_str("\\clbrdrt\\brdrs\\brdrw10\\clbrdrl\\brdrs\\brdrw10\\clbrdrb\\brdrs\\brdrw10\\clbrdrr\\brdrs\\brdrw10");
                    }
                    let right = bounds[((col + span) as usize).min(cols)];
                    let _ = write!(body, "\\cellx{}", tw(right));
                }
                body.push('\n');
                for (_, _, ps) in &cells {
                    for (j, q) in ps.iter().enumerate() {
                        if j > 0 {
                            body.push_str("\\par ");
                        }
                        write_para(&mut body, doc, &doc.paras[*q], true, &mut fonts, &mut colors, &mut counters);
                    }
                    body.push_str("\\cell ");
                }
                body.push_str("\\row\n");
                k = rend;
            }
            need_par = false;
            i = end;
            continue;
        }
        if need_par {
            body.push_str("\\par\n");
        }
        write_para(&mut body, doc, p, false, &mut fonts, &mut colors, &mut counters);
        need_par = true;
        i += 1;
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
    /// Os dados de uma imagem (\\pict).
    Pict,
}

/// A imagem sendo lida.
#[derive(Default)]
struct Pict {
    bytes: Vec<u8>,
    /// Meio byte já lido (hexadecimal vem de dois em dois).
    nibble: Option<u8>,
    /// Some(true) = JPEG, Some(false) = PNG; None = formato que não abrimos (WMF, EMF…).
    jpeg: Option<bool>,
    goal: (i32, i32),
    scale: (i32, i32),
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
    /// Tabelas: o parágrafo atual está numa célula; a tabela, linha e coluna atuais;
    /// os limites das células da linha (\\cellx) e os de cada linha lida.
    intbl: bool,
    table_n: u32,
    cur_table: Option<u32>,
    row: u16,
    col: u16,
    row_cellx: Vec<i32>,
    table_rows: Vec<(u32, u16, Vec<i32>)>,
    table_borders: Vec<u32>,
    pict: Pict,
    /// Bytes crus a ler depois de \\binN.
    bin: usize,
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
            Dest::Pict => {
                if let Some(v) = c.to_digit(16) {
                    let v = v as u8;
                    match self.pict.nibble.take() {
                        Some(h) => self.pict.bytes.push(h << 4 | v),
                        None => self.pict.nibble = Some(v),
                    }
                }
            }
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
        // Célula de tabela (a coluna e quantas ocupa são acertadas no fim, pelos \\cellx).
        if self.intbl {
            if self.cur_table.is_none() {
                self.table_n += 1;
                self.cur_table = Some(self.table_n);
                self.row = 0;
                self.col = 0;
            }
            ps.cell = Some(Cell { table: self.cur_table.unwrap(), row: self.row, col: self.col, span: 1 });
        } else {
            ps.cell = None;
            self.cur_table = None;
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
    pub tables: HashMap<u32, TableProps>,
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
        intbl: false,
        table_n: 0,
        cur_table: None,
        row: 0,
        col: 0,
        row_cellx: Vec::new(),
        table_rows: Vec::new(),
        table_borders: Vec::new(),
        pict: Pict::default(),
        bin: 0,
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
                // Fim de uma imagem: entra no texto (se for PNG ou JPEG).
                if closing == Dest::Pict && st.dest != Dest::Pict {
                    let p = std::mem::take(&mut r.pict);
                    if st.dest == Dest::Text && p.jpeg.is_some() {
                        if let Some(mut img) = crate::rich::Image::from_bytes(p.bytes) {
                            if p.goal.0 > 0 && p.goal.1 > 0 {
                                img.w = p.goal.0 as f32 / 20.0;
                                img.h = p.goal.1 as f32 / 20.0;
                            }
                            img.w = (img.w * p.scale.0.max(1) as f32 / 100.0).max(1.0);
                            img.h = (img.h * p.scale.1.max(1) as f32 / 100.0).max(1.0);
                            let mut s2 = st.clone();
                            s2.cs.image = Some(img);
                            s2.cs.note = None;
                            s2.cs.script = 0;
                            r.push_char(&s2, NOTE_CHAR);
                        }
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
                    // \\binN: N bytes crus (só valem dentro de uma imagem).
                    if r.bin > 0 {
                        let n = std::mem::take(&mut r.bin).min(src.len() - i);
                        if st.dest == Dest::Pict {
                            r.pict.bytes.extend_from_slice(&src[i..i + n]);
                        }
                        i += n;
                    }
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
    // Documento terminando numa tabela: um parágrafo comum depois (para escrever).
    if r.paras.last().is_some_and(|p| p.style.cell.is_some()) {
        let s = st.clone();
        r.intbl = false;
        r.end_para(&State { ps: ParaStyle::default(), ..s });
    }
    // Tabelas: as colunas de verdade saem de todos os limites (\\cellx) da tabela;
    // cada célula fica na coluna do seu limite esquerdo e ocupa até o direito.
    let mut tables: HashMap<u32, TableProps> = HashMap::new();
    for t in 1..=r.table_n {
        let mut all: Vec<i32> = vec![0];
        for (_, _, cx) in r.table_rows.iter().filter(|x| x.0 == t) {
            all.extend(cx.iter().copied());
        }
        all.sort_unstable();
        all.dedup();
        if all.len() < 2 {
            continue;
        }
        let total = (*all.last().unwrap() - all[0]).max(1) as f32;
        let widths: Vec<f32> = all.windows(2).map(|w| (w[1] - w[0]) as f32 / total).collect();
        for (_, row, cx) in r.table_rows.iter().filter(|x| x.0 == t) {
            for p in r.paras.iter_mut() {
                let Some(c) = p.style.cell.as_mut().filter(|c| c.table == t && c.row == *row) else { continue };
                let i = c.col as usize;
                let left = if i == 0 { 0 } else { cx.get(i - 1).copied().unwrap_or(0) };
                let right = cx.get(i).copied().unwrap_or(*all.last().unwrap());
                let a = all.iter().position(|x| *x >= left).unwrap_or(0);
                let b = all.iter().position(|x| *x >= right).unwrap_or(all.len() - 1);
                c.col = a as u16;
                c.span = (b.saturating_sub(a)).max(1) as u16;
            }
        }
        tables.insert(t, TableProps { widths, borders: r.table_borders.contains(&t) });
    }
    let hf = |a: &[String; 3]| HeaderFooter { left: a[0].clone(), center: a[1].clone(), right: a[2].clone() };
    Ok(Read { header: hf(&r.hf[0]), footer: hf(&r.hf[1]), paras: r.paras, page: r.page, notes: r.notes, title_page: r.title_page, tables })
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
            "info" | "nonshppict" | "object" | "header" | "headerl" | "headerr" | "headerf" | "footer" | "footerl" | "footerr" | "footerf" | "listtable" | "listoverridetable" | "revtbl" | "pntext" | "pn" | "xmlnstbl" | "rsidtbl" | "generator" | "fldinst" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "bkmkstart" | "bkmkend" => {
                st.dest = Dest::Skip;
                return;
            }
            "pict" => {
                st.dest = Dest::Pict;
                r.pict = Pict { scale: (100, 100), ..Pict::default() };
                return;
            }
            // A imagem nova do Word (\\shppict) vale; a de reserva (\\nonshppict), não.
            "shppict" => return,
            _ if star => {
                st.dest = Dest::Skip;
                return;
            }
            _ => {}
        }
    }
    if word == "bin" {
        r.bin = n.max(0) as usize;
        return;
    }
    if st.dest == Dest::Pict {
        match word {
            "pngblip" => r.pict.jpeg = Some(false),
            "jpegblip" => r.pict.jpeg = Some(true),
            "picwgoal" => r.pict.goal.0 = n,
            "pichgoal" => r.pict.goal.1 = n,
            "picscalex" => r.pict.scale.0 = n,
            "picscaley" => r.pict.scale.1 = n,
            _ => {}
        }
        return;
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
        "intbl" => r.intbl = true,
        "trowd" => r.row_cellx.clear(),
        "cellx" => r.row_cellx.push(n),
        "brdrs" | "brdrth" | "brdrdb" | "brdrsh" => {
            let t = r.cur_table.unwrap_or(r.table_n + 1);
            if !r.table_borders.contains(&t) {
                r.table_borders.push(t);
            }
        }
        "cell" => {
            r.intbl = true;
            r.end_para(st);
            r.col += 1;
        }
        "row" => {
            if let Some(t) = r.cur_table {
                r.table_rows.push((t, r.row, r.row_cellx.clone()));
            }
            r.row += 1;
            r.col = 0;
        }
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
            st.ps = ParaStyle::default();
            r.intbl = false;
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
    d.tables = r.tables;
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
        let n = d.paras.len();
        d.set_cursor(Pos::new(n - 1, d.paras[n - 1].text.len()), false);
        crate::table::insert(&mut d, 3, 3);
        let first = d.cursor.p;
        for (i, t) in ["Item", "Quantidade", "Valor", "Caneta", "10", "R$ 25,00", "Total geral", "", "R$ 25,00"].iter().enumerate() {
            d.set_cursor(Pos::new(first + i, 0), false);
            d.insert_text(t);
        }
        d.anchor = Pos::new(first + 6, 0);
        d.cursor = Pos::new(first + 7, 0);
        crate::table::merge(&mut d);
        let id = d.paras[first].style.cell.unwrap().table;
        d.tables.get_mut(&id).unwrap().widths = vec![0.5, 0.25, 0.25];
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
                println!("    {:?} {} {} b={} i={} cor={:?} marca={:?} img={:?}", &p.text[a..b], st.family, st.size, st.bold, st.italic, st.color, st.highlight, st.image.as_ref().map(|i| (i.w, i.h, i.data.px, i.data.jpeg)));
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

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut pm = glove_ui::paint::Pixmap::new(w, h).unwrap();
        pm.fill(glove_ui::paint::Color::from_rgba8(200, 30, 40, 255));
        pm.encode_png().unwrap()
    }

    #[test]
    fn imagens() {
        let mut d = Doc::new();
        d.insert_text("Gráfico: ");
        let img = crate::rich::Image::from_bytes(png(40, 20)).unwrap();
        assert_eq!(img.data.px, (40, 20));
        assert_eq!((img.w, img.h), (30.0, 15.0));
        d.insert_image(crate::rich::Image { w: 120.0, h: 60.0, ..img });
        d.insert_text(" fim");
        // O que se digita depois da imagem é texto.
        assert!(d.paras[0].style_at(d.cursor.o).image.is_none());
        let rtf = write(&d);
        assert!(rtf.contains("\\pngblip") && rtf.contains("\\picwgoal2400"));
        let back = read_doc(rtf.as_bytes()).unwrap();
        assert_eq!(back.paras[0].text, d.paras[0].text);
        let at = back.paras[0].text.find(NOTE_CHAR).unwrap();
        let bi = back.paras[0].style_after(at).image.clone().unwrap();
        assert_eq!((bi.w, bi.h, bi.data.px), (120.0, 60.0, (40, 20)));
        assert_eq!(bi.data.bytes, png(40, 20));
        // Mudar o tamanho é um passo do Desfazer.
        let mut back = back;
        back.resize_image(crate::rich::Pos::new(0, at), 60.0, 30.0);
        assert_eq!(back.paras[0].style_after(at).image.as_ref().unwrap().w, 60.0);
        back.undo();
        assert_eq!(back.paras[0].style_after(at).image.as_ref().unwrap().w, 120.0);
    }

    /// Manual: grava $RTF_OUT com as imagens de $IMGS (separadas por vírgula).
    #[test]
    #[ignore]
    fn gravar_com_imagens() {
        let mut d = Doc::new();
        d.insert_text("Relatório com imagens\n");
        for p in std::env::var("IMGS").unwrap().split(',') {
            let img = crate::rich::Image::from_bytes(std::fs::read(p).unwrap()).unwrap();
            let (w, h) = img.fit(300.0, 300.0);
            d.insert_image(crate::rich::Image { w, h, ..img });
            d.insert_text("\nLegenda da imagem.\n");
        }
        std::fs::write(std::env::var("RTF_OUT").unwrap(), write(&d)).unwrap();
    }

    #[test]
    fn imagem_do_word() {
        // Como o Word grava: \shppict com a imagem e \nonshppict com a reserva (WMF).
        let hex: String = png(8, 4).iter().map(|b| format!("{b:02x}")).collect();
        let src = format!("{{\\rtf1\\ansi {{\\*\\shppict{{\\pict{{\\*\\picprop{{\\sp{{\\sn x}}{{\\sv 1}}}}}}\\picscalex50\\picscaley50\\picw8\\pich4\\picwgoal1200\\pichgoal600\\pngblip\n{hex}}}}}{{\\nonshppict{{\\pict\\wmetafile8 0102}}}} texto\\par}}");
        let d = read_doc(src.as_bytes()).unwrap();
        let p = &d.paras[0];
        assert_eq!(p.text.matches(NOTE_CHAR).count(), 1);
        assert!(p.text.ends_with(" texto"));
        let img = p.style_after(0).image.clone().unwrap();
        assert_eq!((img.w, img.h), (30.0, 15.0));
        // E em binário (\bin).
        let mut raw = b"{\\rtf1 {\\pict\\jpegblip\\bin4 ".to_vec();
        raw.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xD9]);
        raw.extend_from_slice(b"}ok\\par}");
        let d = read_doc(&raw).unwrap();
        assert_eq!(d.paras[0].text, "ok");
    }

    #[test]
    fn tabelas() {
        let mut d = Doc::from_plain("antes", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 5), false);
        crate::table::insert(&mut d, 2, 3);
        for (i, t) in ["Nome", "Idade", "Cidade", "Ana", "30", "Recife"].iter().enumerate() {
            d.set_cursor(Pos::new(1 + i, 0), false);
            d.insert_text(t);
        }
        // Junta as duas últimas células da primeira linha.
        d.anchor = Pos::new(2, 0);
        d.cursor = Pos::new(3, 1);
        crate::table::merge(&mut d);
        let id = d.paras[1].style.cell.unwrap().table;
        d.tables.get_mut(&id).unwrap().widths = vec![0.5, 0.25, 0.25];
        let rtf = write(&d);
        assert!(rtf.contains("\\trowd") && rtf.contains("\\intbl") && rtf.contains("\\row"));
        let back = read_doc(rtf.as_bytes()).unwrap();
        assert_eq!(back.plain_text(), d.plain_text());
        let cells: Vec<(u16, u16, u16, String)> = back.paras.iter().filter_map(|p| p.style.cell.map(|c| (c.row, c.col, c.span, p.text.clone()))).collect();
        assert_eq!(cells[0], (0, 0, 1, "Nome".into()));
        assert_eq!(cells[1].1, 1);
        assert_eq!(cells[1].2, 2);
        assert_eq!(cells.last().unwrap(), &(1, 2, 1, "Recife".into()));
        let props = back.tables.values().next().unwrap();
        assert!(props.borders);
        assert!((props.widths[0] - 0.5).abs() < 0.01);
        assert!(back.paras.last().unwrap().style.cell.is_none());
    }

    #[test]
    fn recusa_o_que_nao_e_rtf() {
        assert!(read(b"texto comum").is_err());
    }
}
