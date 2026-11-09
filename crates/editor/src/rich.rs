//! O documento rico: uma lista de parágrafos, cada um com o texto, os trechos de
//! estilo (fonte, tamanho, negrito…) e o estilo do parágrafo (alinhamento,
//! recuos, espaçamento…).
//!
//! Toda edição é uma de duas operações: trocar um trecho por outro (texto e
//! estilos juntos) ou trocar o estilo de alguns parágrafos. Guardando o que saiu
//! e o que entrou, desfazer e refazer são exatos.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Estilo dos caracteres de um trecho. Medidas em pontos (1/72 de polegada).
#[derive(Clone, Debug, PartialEq)]
pub struct CharStyle {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// Cor do texto (RGBA).
    pub color: [u8; 4],
    /// Marca-texto atrás do texto.
    pub highlight: Option<[u8; 4]>,
    /// 1 = sobrescrito, -1 = subscrito.
    pub script: i8,
    /// Marca de nota de rodapé: o caractere é o número da nota.
    pub note: Option<u32>,
    /// Imagem no meio do texto: o caractere (`NOTE_CHAR`) é a imagem.
    pub image: Option<Image>,
}

/// Os bytes de uma imagem do documento (PNG ou JPEG, como estavam no arquivo).
#[derive(Debug)]
pub struct ImageData {
    /// Único no programa (para guardar a imagem já aberta).
    pub id: u64,
    pub bytes: Vec<u8>,
    pub jpeg: bool,
    /// Tamanho em pixels.
    pub px: (u32, u32),
}

/// Imagem no texto: os bytes e o tamanho na página (pontos).
#[derive(Clone, Debug)]
pub struct Image {
    pub data: Rc<ImageData>,
    pub w: f32,
    pub h: f32,
}

impl PartialEq for Image {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.data, &o.data) && self.w == o.w && self.h == o.h
    }
}

impl Image {
    /// Imagem a partir dos bytes do arquivo (PNG ou JPEG), no tamanho natural
    /// (72 pontos por polegada a 96 pixels por polegada). WebP vira PNG.
    pub fn from_bytes(bytes: Vec<u8>) -> Option<Image> {
        let (bytes, jpeg) = match glove_ui::image::sniff(&bytes)? {
            glove_ui::image::Format::Png => (bytes, false),
            glove_ui::image::Format::Jpeg => (bytes, true),
            glove_ui::image::Format::WebP => (glove_ui::image::decode_bytes(&bytes).ok()?.encode_png().ok()?, false),
        };
        let px = image_size(&bytes, jpeg)?;
        let (w, h) = (px.0 as f32 * 0.75, px.1 as f32 * 0.75);
        Some(Image { data: Rc::new(ImageData { id: next(), bytes, jpeg, px }), w, h })
    }

    /// O mesmo tamanho, cabendo em `max_w` × `max_h` (sem deformar).
    pub fn fit(&self, max_w: f32, max_h: f32) -> (f32, f32) {
        let k = (max_w / self.w).min(max_h / self.h).min(1.0);
        (self.w * k, self.h * k)
    }
}

/// Largura e altura (pixels) lidas do cabeçalho de um PNG ou JPEG.
pub fn image_size(b: &[u8], jpeg: bool) -> Option<(u32, u32)> {
    if !jpeg {
        if b.len() < 24 || &b[12..16] != b"IHDR" {
            return None;
        }
        let be = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        return Some((be(16), be(20))).filter(|&(w, h)| w > 0 && h > 0);
    }
    // JPEG: procura o marcador SOFn com as dimensões.
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if matches!(m, 0xD0..=0xD9 | 0x01) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((w, h)).filter(|&(w, h)| w > 0 && h > 0);
        }
        i += 2 + len;
    }
    None
}

/// Família padrão dos documentos novos (a do TextEdit da era).
pub const DEFAULT_FAMILY: &str = "Helvetica";
pub const DEFAULT_SIZE: f32 = 12.0;

impl Default for CharStyle {
    fn default() -> Self {
        Self { family: DEFAULT_FAMILY.into(), size: DEFAULT_SIZE, bold: false, italic: false, underline: false, strike: false, color: [0, 0, 0, 255], highlight: None, script: 0, note: None, image: None }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Papel do parágrafo (estilo): muda a aparência e os títulos entram no sumário.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Body,
    Title,
    Chapter,
    Section,
    Subsection,
    Quote,
    /// Linha do sumário (gerada; "Atualizar Sumário" refaz).
    Toc,
}

impl Role {
    pub const MENU: [Role; 6] = [Role::Body, Role::Title, Role::Chapter, Role::Section, Role::Subsection, Role::Quote];

    pub fn name(self) -> &'static str {
        match self {
            Role::Body => "Corpo",
            Role::Title => "Título",
            Role::Chapter => "Capítulo",
            Role::Section => "Seção",
            Role::Subsection => "Subseção",
            Role::Quote => "Citação",
            Role::Toc => "Sumário",
        }
    }

    /// Nível no sumário (capítulo 0, seção 1, subseção 2).
    pub fn outline(self) -> Option<usize> {
        match self {
            Role::Chapter => Some(0),
            Role::Section => Some(1),
            Role::Subsection => Some(2),
            _ => None,
        }
    }

    /// A aparência do estilo: o que muda nos caracteres e no parágrafo.
    pub fn apply(self, cs: &mut CharStyle, ps: &mut ParaStyle) {
        ps.role = self;
        let (size, bold, italic) = match self {
            Role::Title => (24.0, true, false),
            Role::Chapter => (18.0, true, false),
            Role::Section => (14.0, true, false),
            Role::Subsection => (12.0, true, true),
            Role::Quote => (10.0, false, false),
            Role::Body | Role::Toc => (12.0, false, false),
        };
        cs.size = size;
        cs.bold = bold;
        cs.italic = italic;
        match self {
            Role::Title => {
                ps.align = Align::Center;
                ps.space_before = 0.0;
                ps.space_after = 18.0;
                ps.first_indent = 0.0;
            }
            Role::Chapter => {
                ps.space_before = 18.0;
                ps.space_after = 8.0;
                ps.first_indent = 0.0;
                if ps.align == Align::Justify {
                    ps.align = Align::Left;
                }
            }
            Role::Section | Role::Subsection => {
                ps.space_before = 12.0;
                ps.space_after = 4.0;
                ps.first_indent = 0.0;
                if ps.align == Align::Justify {
                    ps.align = Align::Left;
                }
            }
            // Citação longa (como a da ABNT): recuo de 4 cm, letra menor, espaço simples.
            Role::Quote => {
                ps.left_indent = 4.0 * 28.3465;
                ps.first_indent = 0.0;
                ps.line_spacing = 1.0;
                ps.align = Align::Justify;
            }
            Role::Body | Role::Toc => {
                if ps.left_indent == 4.0 * 28.3465 {
                    ps.left_indent = 0.0;
                }
            }
        }
    }
}

/// Item de lista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct List {
    pub numbered: bool,
    /// Nível (0 = o mais de fora).
    pub level: u8,
}

/// Recuo de cada nível de lista (pontos; o marcador fica antes dele).
pub const LIST_INDENT: f32 = 22.0;

/// Célula de tabela: os parágrafos com a mesma célula ficam juntos nela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub table: u32,
    pub row: u16,
    pub col: u16,
    /// Quantas colunas a célula ocupa (células juntadas).
    pub span: u16,
}

/// Larguras das colunas (frações que somam 1) e se as células têm bordas.
#[derive(Clone, Debug, PartialEq)]
pub struct TableProps {
    pub widths: Vec<f32>,
    pub borders: bool,
}

impl TableProps {
    /// As frações para `cols` colunas (iguais se não houver o bastante).
    pub fn fractions(&self, cols: usize) -> Vec<f32> {
        let cols = cols.max(1);
        let mut w: Vec<f32> = if self.widths.len() == cols { self.widths.clone() } else { vec![1.0; cols] };
        let sum: f32 = w.iter().sum::<f32>().max(0.0001);
        for x in w.iter_mut() {
            *x /= sum;
        }
        w
    }
}

impl Default for TableProps {
    fn default() -> Self {
        Self { widths: Vec::new(), borders: true }
    }
}

/// Estilo de um parágrafo. Medidas em pontos.
#[derive(Clone, Debug, PartialEq)]
pub struct ParaStyle {
    pub align: Align,
    /// Recuo da primeira linha (negativo = deslocado), esquerdo e direito.
    pub first_indent: f32,
    pub left_indent: f32,
    pub right_indent: f32,
    /// Entre linhas, em múltiplos da altura da linha (1, 1,15, 1,5, 2…).
    pub line_spacing: f32,
    pub space_before: f32,
    pub space_after: f32,
    /// Paradas de tabulação a partir da margem esquerda.
    pub tabs: Vec<f32>,
    pub role: Role,
    pub list: Option<List>,
    /// Começa numa página nova.
    pub page_break_before: bool,
    pub cell: Option<Cell>,
}

impl Default for ParaStyle {
    fn default() -> Self {
        Self { align: Align::Left, first_indent: 0.0, left_indent: 0.0, right_indent: 0.0, line_spacing: 1.0, space_before: 0.0, space_after: 0.0, tabs: Vec::new(), role: Role::Body, list: None, page_break_before: false, cell: None }
    }
}

/// Papel e margens (pontos). Padrão: A4 com margens de 2,5 cm.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSetup {
    pub width: f32,
    pub height: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
}

impl Default for PageSetup {
    fn default() -> Self {
        let m = 2.5 * 28.3465;
        Self { width: 595.28, height: 841.89, margin_left: m, margin_right: m, margin_top: m, margin_bottom: m }
    }
}

/// Cabeçalho ou rodapé: texto à esquerda, no centro e à direita. Campos:
/// {página}, {páginas}, {título} e {data}.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderFooter {
    pub left: String,
    pub center: String,
    pub right: String,
}

impl HeaderFooter {
    pub fn is_empty(&self) -> bool {
        self.left.trim().is_empty() && self.center.trim().is_empty() && self.right.trim().is_empty()
    }

    /// As três partes com os campos trocados.
    pub fn fill(&self, page: usize, pages: usize, title: &str, date: &str) -> [String; 3] {
        let f = |s: &str| s.replace("{página}", &page.to_string()).replace("{páginas}", &pages.to_string()).replace("{título}", title).replace("{data}", date);
        [f(&self.left), f(&self.center), f(&self.right)]
    }
}

/// Nota de rodapé: o texto (a marca fica no documento, num caractere com `note`).
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u32,
    pub text: String,
}

/// O caractere que marca uma nota no texto (aparece como o número dela).
pub const NOTE_CHAR: char = '\u{FFFC}';

/// Um trecho de `len` bytes com o mesmo estilo.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub len: usize,
    pub style: Rc<CharStyle>,
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn next() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Um parágrafo (sem a quebra de linha no fim). Os trechos somam o texto inteiro;
/// parágrafo vazio guarda um trecho vazio, com o estilo do que se vai digitar.
#[derive(Clone, Debug)]
pub struct Para {
    pub text: String,
    pub runs: Vec<Run>,
    pub style: ParaStyle,
    /// Identidade (para guardar a diagramação) e versão (muda a cada alteração).
    pub id: u64,
    pub ver: u64,
}

impl PartialEq for Para {
    fn eq(&self, o: &Self) -> bool {
        self.text == o.text && self.style == o.style && self.runs == o.runs
    }
}

impl Para {
    pub fn new(text: &str, style: Rc<CharStyle>, pstyle: ParaStyle) -> Self {
        Self { text: text.into(), runs: vec![Run { len: text.len(), style }], style: pstyle, id: next(), ver: next() }
    }

    fn touch(&mut self) {
        self.ver = next();
    }

    /// Junta trechos vizinhos iguais e tira os vazios (deixa um, se o texto for vazio).
    fn normalize(&mut self) {
        let first = self.runs.first().map(|r| r.style.clone());
        let mut out: Vec<Run> = Vec::with_capacity(self.runs.len());
        for r in self.runs.drain(..) {
            if r.len == 0 {
                continue;
            }
            match out.last_mut() {
                Some(l) if Rc::ptr_eq(&l.style, &r.style) || *l.style == *r.style => l.len += r.len,
                _ => out.push(r),
            }
        }
        if out.is_empty() {
            out.push(Run { len: 0, style: first.unwrap_or_default() });
        }
        self.runs = out;
    }

    /// Estilo do caractere antes de `o` (no começo, o do primeiro).
    pub fn style_at(&self, o: usize) -> &Rc<CharStyle> {
        let mut at = 0;
        for r in &self.runs {
            if o > at && o <= at + r.len {
                return &r.style;
            }
            at += r.len;
        }
        &self.runs[0].style
    }

    /// Estilo do caractere depois de `o` (no fim, o do último).
    pub fn style_after(&self, o: usize) -> &Rc<CharStyle> {
        let mut at = 0;
        for r in &self.runs {
            if o >= at && o < at + r.len {
                return &r.style;
            }
            at += r.len;
        }
        &self.runs[self.runs.len() - 1].style
    }

    /// Corta em `o`: este fica com o começo, devolve o resto (mesmo estilo de parágrafo).
    pub fn split_off(&mut self, o: usize) -> Para {
        let tail_text = self.text.split_off(o);
        let edge = self.style_at(o).clone();
        let mut head = Vec::new();
        let mut tail = Vec::new();
        let mut at = 0;
        for r in self.runs.drain(..) {
            let end = at + r.len;
            if end <= o {
                head.push(r);
            } else if at >= o {
                tail.push(r);
            } else {
                head.push(Run { len: o - at, style: r.style.clone() });
                tail.push(Run { len: end - o, style: r.style });
            }
            at = end;
        }
        if head.is_empty() {
            head.push(Run { len: 0, style: edge.clone() });
        }
        if tail.is_empty() {
            tail.push(Run { len: 0, style: edge });
        }
        self.runs = head;
        self.normalize();
        self.touch();
        let mut t = Para { text: tail_text, runs: tail, style: self.style.clone(), id: next(), ver: next() };
        t.normalize();
        t
    }

    /// Acrescenta o texto e os trechos de `other` no fim (o estilo de parágrafo fica).
    pub fn append(&mut self, other: Para) {
        if other.text.is_empty() {
            return;
        }
        if self.text.is_empty() {
            self.runs = other.runs;
        } else {
            self.runs.extend(other.runs);
        }
        self.text.push_str(&other.text);
        self.normalize();
        self.touch();
    }

    /// Aplica `f` aos estilos do trecho [a, b).
    pub fn restyle(&mut self, a: usize, b: usize, f: &dyn Fn(&mut CharStyle)) {
        if self.text.is_empty() {
            let mut s = (*self.runs[0].style).clone();
            f(&mut s);
            self.runs[0].style = Rc::new(s);
            self.touch();
            return;
        }
        let mut out = Vec::new();
        let mut at = 0;
        for r in self.runs.drain(..) {
            let end = at + r.len;
            let (x0, x1) = (a.max(at), b.min(end));
            if x0 >= x1 {
                out.push(r);
            } else {
                if x0 > at {
                    out.push(Run { len: x0 - at, style: r.style.clone() });
                }
                let mut s = (*r.style).clone();
                f(&mut s);
                out.push(Run { len: x1 - x0, style: Rc::new(s) });
                if end > x1 {
                    out.push(Run { len: end - x1, style: r.style });
                }
            }
            at = end;
        }
        self.runs = out;
        self.normalize();
        self.touch();
    }

    /// Os trechos de estilo como (início, fim, estilo).
    pub fn spans(&self) -> impl Iterator<Item = (usize, usize, &Rc<CharStyle>)> {
        let mut at = 0;
        self.runs.iter().map(move |r| {
            let s = (at, at + r.len, &r.style);
            at += r.len;
            s
        })
    }
}

/// Posição no documento: parágrafo e byte dentro dele.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub p: usize,
    pub o: usize,
}

impl Pos {
    pub fn new(p: usize, o: usize) -> Self {
        Self { p, o }
    }
}

/// Um pedaço de documento (copiado, apagado, colado): parágrafos, o primeiro e o
/// último podem ser pedaços de parágrafo.
pub type Frag = Vec<Para>;

/// Onde termina `frag` colado em `at`.
fn frag_end(at: Pos, frag: &[Para]) -> Pos {
    match frag.len() {
        0 => at,
        1 => Pos::new(at.p, at.o + frag[0].text.len()),
        n => Pos::new(at.p + n - 1, frag[n - 1].text.len()),
    }
}

#[derive(Clone, Debug)]
enum Edit {
    /// Em `at`, o pedaço `old` virou `new`.
    Replace { at: Pos, old: Frag, new: Frag },
    /// Os parágrafos a partir de `first` trocaram de estilo.
    Paras { first: usize, old: Vec<ParaStyle>, new: Vec<ParaStyle> },
}

/// Um passo do Desfazer: as edições e onde estava o cursor antes e depois.
#[derive(Clone, Debug)]
struct Step {
    edits: Vec<Edit>,
    before: (Pos, Pos),
    after: (Pos, Pos),
}

pub struct Doc {
    pub paras: Vec<Para>,
    pub page: PageSetup,
    pub header: HeaderFooter,
    pub footer: HeaderFooter,
    /// Primeira página sem cabeçalho e rodapé (capa).
    pub title_page: bool,
    pub notes: Vec<Note>,
    /// Propriedades de cada tabela (larguras, bordas).
    pub tables: std::collections::HashMap<u32, TableProps>,
    /// Modelo do corpo do texto (letra e parágrafo): o Enter depois de um título e o
    /// estilo "Corpo" voltam a ele.
    pub body: (Rc<CharStyle>, ParaStyle),
    pub cursor: Pos,
    pub anchor: Pos,
    /// Estilo escolhido para o que se vai digitar (Negrito sem seleção…); sai ao mexer o cursor.
    pub typing: Option<Rc<CharStyle>>,
    pub modified: bool,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// Pode juntar a próxima letra digitada ao último passo (desfazer por palavra).
    coalesce: bool,
    /// Muda a cada alteração (para refazer o que depende do documento inteiro).
    pub version: u64,
}

impl Default for Doc {
    fn default() -> Self {
        Self::new()
    }
}

impl Doc {
    pub fn new() -> Self {
        Self::with_style(Rc::new(CharStyle::default()), ParaStyle::default())
    }

    pub fn with_style(style: Rc<CharStyle>, pstyle: ParaStyle) -> Self {
        Self { paras: vec![Para::new("", style.clone(), pstyle.clone())], page: PageSetup::default(), header: HeaderFooter::default(), footer: HeaderFooter::default(), title_page: false, notes: Vec::new(), tables: std::collections::HashMap::new(), body: (style.clone(), pstyle.clone()), cursor: Pos::default(), anchor: Pos::default(), typing: None, modified: false, undo: Vec::new(), redo: Vec::new(), coalesce: false, version: next() }
    }

    /// Documento de texto simples, todo num estilo.
    pub fn from_plain(text: &str, style: Rc<CharStyle>, pstyle: ParaStyle) -> Self {
        let mut d = Self::with_style(style.clone(), pstyle.clone());
        d.paras = text.split('\n').map(|l| Para::new(l.strip_suffix('\r').unwrap_or(l), style.clone(), pstyle.clone())).collect();
        d
    }

    /// Documento a partir de parágrafos prontos (lidos de um arquivo).
    #[allow(dead_code)] // usado a partir da 12.4 (ler arquivos)
    pub fn from_paras(paras: Vec<Para>) -> Self {
        let mut d = Self::new();
        if !paras.is_empty() {
            // O modelo do corpo: o primeiro parágrafo de corpo comum do arquivo.
            if let Some(p) = paras.iter().find(|p| p.style.role == Role::Body && p.style.list.is_none() && p.style.cell.is_none() && !p.text.is_empty()) {
                d.body = (p.style_after(0).clone(), ParaStyle { page_break_before: false, tabs: Vec::new(), ..p.style.clone() });
            }
            d.paras = paras;
        }
        d
    }

    pub fn plain_text(&self) -> String {
        let mut s = String::with_capacity(self.paras.iter().map(|p| p.text.len() + 1).sum());
        for (i, p) in self.paras.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(&p.text);
        }
        s
    }

    /// Posição de um byte do texto simples (`plain_text`).
    pub fn pos_of_offset(&self, mut off: usize) -> Pos {
        for (i, p) in self.paras.iter().enumerate() {
            if off <= p.text.len() {
                return Pos::new(i, off);
            }
            off -= p.text.len() + 1;
        }
        self.end()
    }

    /// Byte do texto simples de uma posição.
    pub fn offset_of(&self, pos: Pos) -> usize {
        self.paras[..pos.p].iter().map(|p| p.text.len() + 1).sum::<usize>() + pos.o
    }

    pub fn end(&self) -> Pos {
        let p = self.paras.len() - 1;
        Pos::new(p, self.paras[p].text.len())
    }

    pub fn is_empty(&self) -> bool {
        self.paras.len() == 1 && self.paras[0].text.is_empty()
    }

    #[allow(dead_code)] // usado a partir da 12.4 (ler arquivos)
    pub fn len(&self) -> usize {
        self.paras.iter().map(|p| p.text.len() + 1).sum::<usize>() - 1
    }

    // -------------------------------------------------------------------
    // Seleção
    // -------------------------------------------------------------------

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    /// A seleção em ordem, sempre dentro do texto.
    pub fn selection(&self) -> (Pos, Pos) {
        let (c, a) = (self.clamp(self.cursor), self.clamp(self.anchor));
        if c <= a { (c, a) } else { (a, c) }
    }

    pub fn set_cursor(&mut self, p: Pos, extend: bool) {
        let p = self.clamp(p);
        self.cursor = p;
        if !extend {
            self.anchor = p;
        }
        self.typing = None;
        self.coalesce = false;
    }

    pub fn select_all(&mut self) {
        self.anchor = Pos::default();
        self.cursor = self.end();
        self.typing = None;
    }

    pub fn clamp(&self, p: Pos) -> Pos {
        let pi = p.p.min(self.paras.len() - 1);
        let t = &self.paras[pi].text;
        let mut o = p.o.min(t.len());
        while !t.is_char_boundary(o) {
            o -= 1;
        }
        Pos::new(pi, o)
    }

    /// O pedaço [a, b) como fragmento (copiar).
    pub fn frag(&self, a: Pos, b: Pos) -> Frag {
        if a >= b {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in a.p..=b.p {
            let mut p = self.paras[i].clone();
            if i == b.p {
                p.split_off(b.o);
            }
            if i == a.p {
                p = p.split_off(a.o);
            }
            out.push(p);
        }
        out
    }

    pub fn text_of(&self, a: Pos, b: Pos) -> String {
        self.frag(a, b).iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n")
    }

    pub fn selected_frag(&self) -> Frag {
        let (a, b) = self.selection();
        self.frag(a, b)
    }

    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        self.text_of(a, b)
    }

    /// Estilo para o que se digita agora: o escolhido ou o do caractere antes do cursor.
    pub fn typing_style(&self) -> Rc<CharStyle> {
        if let Some(t) = &self.typing {
            return t.clone();
        }
        let (a, _) = self.selection();
        let p = &self.paras[a.p];
        // Com seleção: o estilo do primeiro caractere selecionado.
        let s = if self.has_selection() { p.style_after(a.o) } else { p.style_at(a.o) };
        // O que se digita depois de uma marca de nota não é nota.
        if s.note.is_some() || s.image.is_some() {
            return Rc::new(CharStyle { note: None, image: None, ..(**s).clone() });
        }
        s.clone()
    }

    /// Põe uma imagem no lugar da seleção (ou no cursor).
    pub fn insert_image(&mut self, img: Image) {
        let style = Rc::new(CharStyle { image: Some(img), note: None, script: 0, ..(*self.typing_style()).clone() });
        let (a, _) = self.selection();
        let pstyle = self.paras[a.p].style.clone();
        let mut ch = [0u8; 4];
        let frag = vec![Para::new(NOTE_CHAR.encode_utf8(&mut ch), style, pstyle)];
        self.replace_selection(frag, false);
        self.coalesce = false;
    }

    /// Muda o tamanho da imagem que começa em `at` (um passo do Desfazer).
    pub fn resize_image(&mut self, at: Pos, w: f32, h: f32) {
        let b = Pos::new(at.p, at.o + NOTE_CHAR.len_utf8());
        if self.paras[at.p].style_after(at.o).image.is_none() {
            return;
        }
        let keep = (self.cursor, self.anchor);
        self.anchor = at;
        self.cursor = b;
        self.set_char_style(&|s| {
            if let Some(i) = &mut s.image {
                i.w = w;
                i.h = h;
            }
        });
        self.cursor = keep.0;
        self.anchor = keep.1;
    }

    /// A imagem selecionada sozinha (posição e imagem).
    pub fn selected_image(&self) -> Option<(Pos, Image)> {
        let (a, b) = self.selection();
        if a.p != b.p || b.o != a.o + NOTE_CHAR.len_utf8() {
            return None;
        }
        self.paras[a.p].style_after(a.o).image.clone().map(|i| (a, i))
    }

    /// Insere uma nota de rodapé no cursor. Devolve o número interno dela.
    pub fn insert_note(&mut self, text: &str) -> u32 {
        let id = self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        self.notes.push(Note { id, text: text.to_string() });
        let style = Rc::new(CharStyle { note: Some(id), script: 0, ..(*self.typing_style()).clone() });
        let (a, _) = self.selection();
        let pstyle = self.paras[a.p].style.clone();
        let mut ch = [0u8; 4];
        let frag = vec![Para::new(NOTE_CHAR.encode_utf8(&mut ch), style, pstyle)];
        let (_, b) = self.selection();
        // A nota entra depois da seleção (não a apaga).
        self.set_cursor(b, false);
        self.replace_selection(frag, false);
        self.coalesce = false;
        id
    }

    /// As notas na ordem em que aparecem no texto (o número de cada uma é a posição + 1).
    pub fn note_order(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for p in &self.paras {
            for (_, _, st) in p.spans() {
                if let Some(id) = st.note {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
        out
    }

    pub fn note_text(&self, id: u32) -> Option<&str> {
        self.notes.iter().find(|n| n.id == id).map(|n| n.text.as_str())
    }

    pub fn set_note_text(&mut self, id: u32, text: &str) {
        if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
            if n.text != text {
                n.text = text.to_string();
                self.modified = true;
                self.version = next();
            }
        }
    }

    // -------------------------------------------------------------------
    // Operações básicas (sem Desfazer)
    // -------------------------------------------------------------------

    fn cut(&mut self, a: Pos, b: Pos) -> Frag {
        if a >= b {
            return Vec::new();
        }
        if a.p == b.p {
            let p = &mut self.paras[a.p];
            let mut mid = p.split_off(a.o);
            let right = mid.split_off(b.o - a.o);
            p.append(right);
            mid.style = p.style.clone();
            return vec![mid];
        }
        let first = self.paras[a.p].split_off(a.o);
        let mut last = self.paras.remove(b.p);
        let right = last.split_off(b.o);
        let middle: Vec<Para> = self.paras.drain(a.p + 1..b.p).collect();
        self.paras[a.p].append(right);
        let mut out = Vec::with_capacity(middle.len() + 2);
        out.push(first);
        out.extend(middle);
        out.push(last);
        out
    }

    fn paste_at(&mut self, at: Pos, frag: &[Para]) -> Pos {
        let n = frag.len();
        if n == 0 {
            return at;
        }
        let right = self.paras[at.p].split_off(at.o);
        let mut first = frag[0].clone();
        first.id = next();
        self.paras[at.p].append(first);
        if n == 1 {
            self.paras[at.p].append(right);
            return Pos::new(at.p, at.o + frag[0].text.len());
        }
        for (k, f) in frag[1..n - 1].iter().enumerate() {
            let mut f = f.clone();
            f.id = next();
            f.touch();
            self.paras.insert(at.p + 1 + k, f);
        }
        let mut last = frag[n - 1].clone();
        last.id = next();
        last.touch();
        let end = Pos::new(at.p + n - 1, last.text.len());
        last.append(right);
        self.paras.insert(at.p + n - 1, last);
        end
    }

    fn apply(&mut self, e: &Edit, reverse: bool) {
        match e {
            Edit::Replace { at, old, new } => {
                let (rm, put) = if reverse { (new, old) } else { (old, new) };
                let end = frag_end(*at, rm);
                self.cut(*at, end);
                self.paste_at(*at, put);
            }
            Edit::Paras { first, old, new } => {
                let put = if reverse { old } else { new };
                for (k, s) in put.iter().enumerate() {
                    if let Some(p) = self.paras.get_mut(first + k) {
                        p.style = s.clone();
                        p.touch();
                    }
                }
            }
        }
        self.version = next();
    }

    // -------------------------------------------------------------------
    // Edição com Desfazer
    // -------------------------------------------------------------------

    fn push(&mut self, edits: Vec<Edit>, before: (Pos, Pos), coalesce: bool) {
        self.redo.clear();
        self.modified = true;
        self.version = next();
        let after = (self.cursor, self.anchor);
        if coalesce && self.coalesce {
            if let Some(last) = self.undo.last_mut() {
                last.edits.extend(edits);
                last.after = after;
                return;
            }
        }
        self.undo.push(Step { edits, before, after });
        if self.undo.len() > 2000 {
            self.undo.remove(0);
        }
    }

    /// Troca a seleção por `frag` (colar, digitar). O cursor vai para o fim.
    pub fn replace_selection(&mut self, frag: Frag, coalesce: bool) {
        let before = (self.cursor, self.anchor);
        let (a, b) = self.selection();
        let old = self.cut(a, b);
        let end = self.paste_at(a, &frag);
        let had_sel = !old.is_empty();
        self.cursor = end;
        self.anchor = end;
        self.push(vec![Edit::Replace { at: a, old, new: frag }], before, coalesce && !had_sel);
    }

    /// Digita `text` (pode ter quebras de linha) no estilo de digitação.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let style = self.typing_style();
        let (a, _) = self.selection();
        let pstyle = self.paras[a.p].style.clone();
        let frag: Frag = text.split('\n').map(|l| Para::new(l, style.clone(), pstyle.clone())).collect();
        // Por palavra no Desfazer: espaço e quebra de linha fecham o passo.
        let breaks = text.contains(['\n', ' ', '\t']);
        let typing = self.typing.take();
        // Só letras digitadas uma a uma se juntam (colar é um passo próprio).
        self.replace_selection(frag, text.chars().count() == 1);
        self.typing = typing;
        self.coalesce = !breaks;
        // Parágrafo novo de um título volta ao corpo (como no Pages).
        if text.contains('\n') {
            let p = self.cursor.p;
            if matches!(self.paras[p].style.role, Role::Title | Role::Chapter | Role::Section | Role::Subsection) && self.paras[p].text.is_empty() {
                // Volta ao modelo do corpo do documento.
                let s = self.body.1.clone();
                let cs = (*self.body.0).clone();
                self.set_para_styles(p, p, vec![s]);
                // A letra do parágrafo novo também volta ao corpo.
                self.paras[p].runs[0].style = Rc::new(cs);
                self.paras[p].touch();
                self.typing = None;
                self.coalesce = false;
            }
        }
    }

    /// Cola um fragmento (com estilos).
    pub fn insert_frag(&mut self, frag: Frag) {
        self.replace_selection(frag, false);
        self.coalesce = false;
    }

    /// Apaga a seleção, ou de `to` até o cursor.
    pub fn delete_range(&mut self, a: Pos, b: Pos) {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        if a == b {
            return;
        }
        self.anchor = a;
        self.cursor = b;
        self.replace_selection(Vec::new(), false);
        self.coalesce = false;
    }

    pub fn delete_selection(&mut self) {
        let (a, b) = self.selection();
        self.delete_range(a, b);
    }

    /// Aplica `f` ao estilo dos caracteres selecionados (sem seleção: ao que se vai digitar).
    pub fn set_char_style(&mut self, f: &dyn Fn(&mut CharStyle)) {
        if !self.has_selection() {
            let mut s = (*self.typing_style()).clone();
            f(&mut s);
            self.typing = Some(Rc::new(s));
            return;
        }
        let before = (self.cursor, self.anchor);
        let (a, b) = self.selection();
        let old = self.frag(a, b);
        let mut new = old.clone();
        for p in new.iter_mut() {
            let len = p.text.len();
            p.restyle(0, len, f);
        }
        self.cut(a, b);
        self.paste_at(a, &new);
        self.cursor = before.0;
        self.anchor = before.1;
        self.push(vec![Edit::Replace { at: a, old, new }], before, false);
        self.coalesce = false;
    }

    /// Troca o estilo dos parágrafos [p0, p1] (um estilo para cada).
    pub fn set_para_styles(&mut self, p0: usize, p1: usize, styles: Vec<ParaStyle>) {
        let before = (self.cursor, self.anchor);
        let old: Vec<ParaStyle> = self.paras[p0..=p1].iter().map(|p| p.style.clone()).collect();
        if old == styles {
            return;
        }
        let e = Edit::Paras { first: p0, old, new: styles };
        self.apply(&e, false);
        self.push(vec![e], before, false);
        self.coalesce = false;
    }

    /// Aplica `f` ao estilo dos parágrafos tocados pela seleção.
    pub fn set_para_style(&mut self, f: &dyn Fn(&mut ParaStyle)) {
        let (a, b) = self.selection();
        let styles: Vec<ParaStyle> = self.paras[a.p..=b.p]
            .iter()
            .map(|p| {
                let mut s = p.style.clone();
                f(&mut s);
                s
            })
            .collect();
        self.set_para_styles(a.p, b.p, styles);
    }

    /// Aplica um estilo aos parágrafos tocados pela seleção (texto e parágrafo, um passo só).
    pub fn apply_role(&mut self, role: Role) {
        let (a, b) = self.selection();
        let (cur, anc) = (self.cursor, self.anchor);
        let (p0, p1) = (a.p, b.p);
        let body = self.body.clone();
        self.group(|d| {
            let styles: Vec<ParaStyle> = d.paras[p0..=p1]
                .iter()
                .map(|p| {
                    // Corpo: o modelo do documento (mantém lista, célula e quebra de página).
                    if role == Role::Body {
                        return ParaStyle { list: p.style.list, cell: p.style.cell, page_break_before: p.style.page_break_before, ..body.1.clone() };
                    }
                    let mut ps = p.style.clone();
                    let mut cs = CharStyle::default();
                    role.apply(&mut cs, &mut ps);
                    ps
                })
                .collect();
            d.set_para_styles(p0, p1, styles);
            // Os caracteres dos parágrafos inteiros.
            d.anchor = Pos::new(p0, 0);
            d.cursor = Pos::new(p1, d.paras[p1].text.len());
            if d.has_selection() {
                let b = body.0.clone();
                d.set_char_style(&|cs| {
                    let mut ps = ParaStyle::default();
                    let keep = (cs.family.clone(), cs.color, cs.underline, cs.strike, cs.highlight);
                    role.apply(cs, &mut ps);
                    if role == Role::Body {
                        (cs.size, cs.bold, cs.italic) = (b.size, b.bold, b.italic);
                    }
                    (cs.family, cs.color, cs.underline, cs.strike, cs.highlight) = keep;
                });
            }
            // Parágrafo vazio: o estilo vale para o que se digitar nele.
            for pi in p0..=p1 {
                if d.paras[pi].text.is_empty() {
                    let mut cs = (*d.paras[pi].runs[0].style).clone();
                    let mut ps = ParaStyle::default();
                    let fam = cs.family.clone();
                    role.apply(&mut cs, &mut ps);
                    cs.family = fam;
                    d.paras[pi].runs[0].style = Rc::new(cs);
                    d.paras[pi].touch();
                }
            }
        });
        self.cursor = self.clamp(cur);
        self.anchor = self.clamp(anc);
    }

    /// Liga, troca ou desliga a lista dos parágrafos tocados pela seleção.
    pub fn set_list(&mut self, list: Option<bool>) {
        let body = self.body.1.clone();
        self.set_para_style(&|ps| match list {
            Some(numbered) => {
                let level = ps.list.map_or(0, |l| l.level);
                ps.list = Some(List { numbered, level });
                ps.left_indent = LIST_INDENT * (level as f32 + 1.0);
                ps.first_indent = 0.0;
            }
            // Saindo da lista: o recuo volta ao do corpo do documento.
            None => {
                if ps.list.is_some() {
                    ps.left_indent = body.left_indent;
                    ps.first_indent = body.first_indent;
                }
                ps.list = None;
            }
        });
    }

    /// Aprofunda (+1) ou volta (-1) o nível dos itens de lista selecionados.
    pub fn shift_list(&mut self, delta: i32) {
        self.set_para_style(&|ps| {
            if let Some(l) = ps.list.as_mut() {
                let lv = (l.level as i32 + delta).clamp(0, 8) as u8;
                l.level = lv;
                ps.left_indent = LIST_INDENT * (lv as f32 + 1.0);
            }
        });
    }

    /// Fecha o passo atual do Desfazer (a próxima letra começa outro).
    pub fn break_undo(&mut self) {
        self.coalesce = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(step) = self.undo.pop() else { return };
        for e in step.edits.iter().rev() {
            self.apply(e, true);
        }
        (self.cursor, self.anchor) = step.before;
        self.cursor = self.clamp(self.cursor);
        self.anchor = self.clamp(self.anchor);
        self.redo.push(step);
        self.modified = true;
        self.coalesce = false;
        self.typing = None;
    }

    pub fn redo(&mut self) {
        let Some(step) = self.redo.pop() else { return };
        for e in &step.edits {
            self.apply(e, false);
        }
        (self.cursor, self.anchor) = step.after;
        self.cursor = self.clamp(self.cursor);
        self.anchor = self.clamp(self.anchor);
        self.undo.push(step);
        self.modified = true;
        self.coalesce = false;
        self.typing = None;
    }

    /// Junta várias edições num passo só do Desfazer (Substituir Tudo…).
    pub fn group(&mut self, f: impl FnOnce(&mut Self)) {
        let n = self.undo.len();
        let before = (self.cursor, self.anchor);
        f(self);
        if self.undo.len() > n + 1 {
            let steps: Vec<Step> = self.undo.drain(n..).collect();
            let edits = steps.into_iter().flat_map(|s| s.edits).collect();
            self.undo.push(Step { edits, before, after: (self.cursor, self.anchor) });
        }
        self.coalesce = false;
    }

    // -------------------------------------------------------------------
    // Movimento (o que não depende das linhas na tela)
    // -------------------------------------------------------------------

    pub fn prev_pos(&self, p: Pos) -> Pos {
        if p.o > 0 {
            let t = &self.paras[p.p].text;
            let o = t[..p.o].char_indices().next_back().map_or(0, |(i, _)| i);
            Pos::new(p.p, o)
        } else if p.p > 0 {
            Pos::new(p.p - 1, self.paras[p.p - 1].text.len())
        } else {
            p
        }
    }

    pub fn next_pos(&self, p: Pos) -> Pos {
        let t = &self.paras[p.p].text;
        if p.o < t.len() {
            Pos::new(p.p, p.o + t[p.o..].chars().next().map_or(1, char::len_utf8))
        } else if p.p + 1 < self.paras.len() {
            Pos::new(p.p + 1, 0)
        } else {
            p
        }
    }

    /// Começo da palavra antes de `p` (Ctrl+←, Ctrl+Backspace).
    pub fn word_left(&self, p: Pos) -> Pos {
        if p.o == 0 {
            return self.prev_pos(p);
        }
        let t = &self.paras[p.p].text[..p.o];
        let chars: Vec<(usize, char)> = t.char_indices().collect();
        let mut i = chars.len();
        while i > 0 && !chars[i - 1].1.is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && chars[i - 1].1.is_alphanumeric() {
            i -= 1;
        }
        Pos::new(p.p, chars.get(i).map_or(0, |c| c.0))
    }

    /// Fim da palavra depois de `p` (Ctrl+→, Ctrl+Delete).
    pub fn word_right(&self, p: Pos) -> Pos {
        let t = &self.paras[p.p].text;
        if p.o >= t.len() {
            return self.next_pos(p);
        }
        let mut it = t[p.o..].char_indices().peekable();
        let mut end = t.len() - p.o;
        while let Some(&(_, c)) = it.peek() {
            if c.is_alphanumeric() {
                break;
            }
            it.next();
        }
        while let Some(&(i, c)) = it.peek() {
            if !c.is_alphanumeric() {
                end = i;
                break;
            }
            it.next();
        }
        Pos::new(p.p, p.o + end)
    }

    /// Seleciona a palavra em `p` (clique duplo).
    pub fn select_word(&mut self, p: Pos) {
        let t = &self.paras[p.p].text;
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut a = p.o;
        while a > 0 {
            let (i, c) = t[..a].char_indices().next_back().unwrap();
            if !is_word(c) {
                break;
            }
            a = i;
        }
        let mut b = p.o;
        while b < t.len() {
            let c = t[b..].chars().next().unwrap();
            if !is_word(c) {
                break;
            }
            b += c.len_utf8();
        }
        if a == b && b < t.len() {
            b += t[b..].chars().next().map_or(0, char::len_utf8);
        }
        self.anchor = Pos::new(p.p, a);
        self.cursor = Pos::new(p.p, b);
        self.typing = None;
    }

    /// Seleciona o parágrafo de `p` (clique triplo).
    pub fn select_para(&mut self, p: usize) {
        self.anchor = Pos::new(p, 0);
        self.cursor = Pos::new(p, self.paras[p].text.len());
        self.typing = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> Rc<CharStyle> {
        Rc::new(CharStyle { bold: true, ..CharStyle::default() })
    }

    fn snapshot(d: &Doc) -> Vec<(String, Vec<(usize, bool)>, ParaStyle)> {
        d.paras.iter().map(|p| (p.text.clone(), p.runs.iter().map(|r| (r.len, r.style.bold)).collect(), p.style.clone())).collect()
    }

    #[test]
    fn digitar_e_desfazer() {
        let mut d = Doc::new();
        d.insert_text("Olá");
        d.insert_text(" mundo\nsegunda");
        assert_eq!(d.plain_text(), "Olá mundo\nsegunda");
        assert_eq!(d.paras.len(), 2);
        assert_eq!(d.cursor, Pos::new(1, 7));
        d.undo();
        assert_eq!(d.plain_text(), "Olá");
        d.undo();
        assert_eq!(d.plain_text(), "");
        d.redo();
        d.redo();
        assert_eq!(d.plain_text(), "Olá mundo\nsegunda");
    }

    #[test]
    fn negrito_num_trecho() {
        let mut d = Doc::from_plain("um dois três", Rc::new(CharStyle::default()), ParaStyle::default());
        d.anchor = Pos::new(0, 3);
        d.cursor = Pos::new(0, 7);
        d.set_char_style(&|s| s.bold = true);
        let runs: Vec<(usize, bool)> = d.paras[0].runs.iter().map(|r| (r.len, r.style.bold)).collect();
        assert_eq!(runs, [(3, false), (4, true), (6, false)]);
        // Digitar logo depois do trecho em negrito continua em negrito.
        d.set_cursor(Pos::new(0, 7), false);
        d.insert_text("X");
        assert!(d.paras[0].style_at(8).bold);
        d.undo();
        d.undo();
        assert_eq!(d.paras[0].runs.len(), 1);
    }

    #[test]
    fn negrito_sem_selecao_vale_para_o_que_se_digita() {
        let mut d = Doc::from_plain("ab", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 1), false);
        d.set_char_style(&|s| s.bold = true);
        d.insert_text("XY");
        let runs: Vec<(usize, bool)> = d.paras[0].runs.iter().map(|r| (r.len, r.style.bold)).collect();
        assert_eq!(runs, [(1, false), (2, true), (1, false)]);
    }

    #[test]
    fn apagar_entre_paragrafos_e_desfazer_volta_tudo() {
        let mut d = Doc::from_plain("primeiro\nsegundo\nterceiro", Rc::new(CharStyle::default()), ParaStyle::default());
        d.paras[1].style.align = Align::Center;
        d.anchor = Pos::new(0, 2);
        d.cursor = Pos::new(1, 0);
        d.set_char_style(&|s| s.bold = true);
        let antes = snapshot(&d);
        d.delete_range(Pos::new(0, 4), Pos::new(2, 3));
        assert_eq!(d.plain_text(), "primceiro");
        d.undo();
        assert_eq!(snapshot(&d), antes);
        d.redo();
        assert_eq!(d.plain_text(), "primceiro");
    }

    #[test]
    fn copiar_e_colar_mantem_estilos() {
        let mut d = Doc::from_plain("abc\ndef", Rc::new(CharStyle::default()), ParaStyle::default());
        d.paras[1].runs[0].style = bold();
        let frag = d.frag(Pos::new(0, 1), Pos::new(1, 2));
        assert_eq!(frag.len(), 2);
        assert_eq!(frag[0].text, "bc");
        assert_eq!(frag[1].text, "de");
        assert!(frag[1].runs[0].style.bold);
        d.set_cursor(d.end(), false);
        d.insert_frag(frag);
        assert_eq!(d.plain_text(), "abc\ndefbc\nde");
        assert!(d.paras[2].style_at(1).bold);
        d.undo();
        assert_eq!(d.plain_text(), "abc\ndef");
    }

    #[test]
    fn estilo_de_paragrafo_e_desfazer() {
        let mut d = Doc::from_plain("a\nb\nc", Rc::new(CharStyle::default()), ParaStyle::default());
        d.anchor = Pos::new(0, 0);
        d.cursor = Pos::new(1, 1);
        d.set_para_style(&|s| s.align = Align::Justify);
        assert_eq!(d.paras[0].style.align, Align::Justify);
        assert_eq!(d.paras[1].style.align, Align::Justify);
        assert_eq!(d.paras[2].style.align, Align::Left);
        d.undo();
        assert!(d.paras.iter().all(|p| p.style.align == Align::Left));
    }

    #[test]
    fn enter_herda_o_paragrafo_e_titulo_volta_ao_corpo() {
        let mut d = Doc::new();
        d.paras[0].style.align = Align::Center;
        d.insert_text("Título\n");
        assert_eq!(d.paras[1].style.align, Align::Center);
        let mut d = Doc::new();
        d.paras[0].style.role = Role::Chapter;
        d.insert_text("Capítulo 1\n");
        assert_eq!(d.paras[0].style.role, Role::Chapter);
        assert_eq!(d.paras[1].style.role, Role::Body);
    }

    #[test]
    fn notas_numeradas_pela_ordem() {
        let mut d = Doc::from_plain("um dois", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 7), false);
        let a = d.insert_note("nota do fim");
        d.set_cursor(Pos::new(0, 2), false);
        let b = d.insert_note("nota do começo");
        assert_eq!(d.note_order(), vec![b, a]);
        // O que se digita depois da marca não vira nota.
        d.insert_text("X");
        assert!(d.paras[0].style_at(d.cursor.o).note.is_none());
        assert_eq!(d.plain_text().chars().filter(|c| *c == NOTE_CHAR).count(), 2);
    }

    #[test]
    fn palavras() {
        let d = Doc::from_plain("ação, fim", Rc::new(CharStyle::default()), ParaStyle::default());
        assert_eq!(d.word_right(Pos::new(0, 0)), Pos::new(0, 6));
        assert_eq!(d.word_left(Pos::new(0, 11)), Pos::new(0, 8));
        assert_eq!(d.pos_of_offset(3), Pos::new(0, 3));
    }

    #[test]
    fn desfazer_junta_letras_de_uma_palavra() {
        let mut d = Doc::new();
        for c in "casa".chars() {
            d.insert_text(&c.to_string());
        }
        d.insert_text(" ");
        for c in "azul".chars() {
            d.insert_text(&c.to_string());
        }
        d.undo();
        assert_eq!(d.plain_text(), "casa ");
        d.undo();
        assert_eq!(d.plain_text(), "");
    }
}
