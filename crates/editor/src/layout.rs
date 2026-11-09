//! Diagramação de um parágrafo: quebra em linhas com fontes misturadas,
//! recuos, tabulações e alinhamento (inclusive justificado). Tudo em pontos;
//! o zoom só entra no desenho.

use std::rc::Rc;

use crate::fontbook::FontBook;
use crate::rich::{Align, CharStyle, Para};

/// Distância entre as tabulações padrão (meia polegada, como no TextEdit).
pub const DEFAULT_TAB: f32 = 36.0;

/// Pedaço de uma linha desenhado de uma vez (mesmo estilo).
#[derive(Clone, Debug)]
pub struct Seg {
    pub a: usize,
    pub b: usize,
    /// Posição e largura na linha (a partir da margem esquerda do texto).
    pub x: f32,
    pub w: f32,
    /// Largura natural (sem o esticado da justificação).
    pub nat: f32,
    pub style: Rc<CharStyle>,
    pub tab: bool,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub a: usize,
    pub b: usize,
    /// Topo da linha (a partir do topo do parágrafo, já com o espaço antes) e altura.
    pub y: f32,
    pub h: f32,
    /// Linha de base (a partir do topo da linha) e altura do texto (sem o entrelinha).
    pub base: f32,
    pub text_h: f32,
    /// Onde a linha começa e termina (para o cursor numa linha vazia).
    pub x0: f32,
    pub x1: f32,
    pub segs: Vec<Seg>,
}

#[derive(Clone, Debug)]
pub struct ParaLayout {
    pub lines: Vec<Line>,
    /// Altura total (espaço antes + linhas + espaço depois).
    pub h: f32,
    pub ver: u64,
    pub width: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Word,
    Space,
    Tab,
}

struct Tok {
    a: usize,
    b: usize,
    kind: Kind,
    w: f32,
    style: Rc<CharStyle>,
}

/// Próxima parada de tabulação depois de `x` (medida da margem esquerda).
fn next_tab(tabs: &[f32], x: f32) -> f32 {
    if let Some(t) = tabs.iter().copied().find(|&t| t > x + 0.5) {
        return t;
    }
    ((x + 0.5) / DEFAULT_TAB).floor() * DEFAULT_TAB + DEFAULT_TAB
}

fn tokens(p: &Para, fb: &FontBook) -> Vec<Tok> {
    let mut out = Vec::new();
    for (a, b, st) in p.spans() {
        let s = &p.text[a..b];
        let mut start = 0;
        let mut kind: Option<Kind> = None;
        let kind_of = |c: char| match c {
            '\t' => Kind::Tab,
            ' ' => Kind::Space,
            _ => Kind::Word,
        };
        let push = |out: &mut Vec<Tok>, from: usize, to: usize, k: Kind| {
            if from < to {
                // Cada tabulação é um token sozinho.
                if k == Kind::Tab {
                    for i in from..to {
                        out.push(Tok { a: a + i, b: a + i + 1, kind: k, w: 0.0, style: st.clone() });
                    }
                } else {
                    let w = fb.measure(st, &s[from..to]);
                    out.push(Tok { a: a + from, b: a + to, kind: k, w, style: st.clone() });
                }
            }
        };
        for (i, c) in s.char_indices() {
            let k = kind_of(c);
            if kind != Some(k) {
                if let Some(pk) = kind {
                    push(&mut out, start, i, pk);
                }
                start = i;
                kind = Some(k);
            }
        }
        if let Some(pk) = kind {
            push(&mut out, start, s.len(), pk);
        }
    }
    out
}

/// Diagrama o parágrafo numa coluna de `width` pontos.
pub fn layout_para(p: &Para, width: f32, fb: &FontBook) -> ParaLayout {
    let ps = &p.style;
    let toks = tokens(p, fb);
    let left = ps.left_indent;
    let right = (width - ps.right_indent).max(left + 20.0);
    let mut lines: Vec<Vec<Tok>> = Vec::new();
    let mut cur: Vec<Tok> = Vec::new();
    // Largura usada na linha atual (a partir do começo dela).
    let mut x = 0.0f32;
    let line_start = |n: usize| left + if n == 0 { ps.first_indent } else { 0.0 };
    let avail = |n: usize| right - line_start(n);
    let has_word = |cur: &Vec<Tok>| cur.iter().any(|t| t.kind != Kind::Space);
    for mut t in toks {
        let n = lines.len();
        match t.kind {
            Kind::Tab => {
                let abs = line_start(n) + x;
                t.w = (next_tab(&ps.tabs, abs) - abs).max(1.0);
                if x + t.w > avail(n) && has_word(&cur) {
                    lines.push(std::mem::take(&mut cur));
                    x = 0.0;
                    let abs = line_start(lines.len());
                    t.w = (next_tab(&ps.tabs, abs) - abs).max(1.0);
                }
                x += t.w;
                cur.push(t);
            }
            Kind::Space => {
                x += t.w;
                cur.push(t);
            }
            Kind::Word => {
                if x + t.w > avail(n) && has_word(&cur) {
                    lines.push(std::mem::take(&mut cur));
                    x = 0.0;
                }
                let n = lines.len();
                // Palavra maior que a linha: quebra entre as letras.
                if t.w > avail(n) && x == 0.0 {
                    let s = &p.text[t.a..t.b];
                    let mut from = 0;
                    let mut acc = String::new();
                    for (i, c) in s.char_indices() {
                        acc.push(c);
                        let w = fb.measure(&t.style, &acc);
                        if w > avail(lines.len()) && i > from {
                            let piece = &s[from..i];
                            let pw = fb.measure(&t.style, piece);
                            cur.push(Tok { a: t.a + from, b: t.a + i, kind: Kind::Word, w: pw, style: t.style.clone() });
                            lines.push(std::mem::take(&mut cur));
                            from = i;
                            acc.clear();
                            acc.push(c);
                        }
                    }
                    let piece = &s[from..];
                    let pw = fb.measure(&t.style, piece);
                    cur.push(Tok { a: t.a + from, b: t.b, kind: Kind::Word, w: pw, style: t.style.clone() });
                    x = pw;
                } else {
                    x += t.w;
                    cur.push(t);
                }
            }
        }
    }
    lines.push(cur);

    // Medidas e posições de cada linha.
    let empty_style = p.runs[0].style.clone();
    let n_lines = lines.len();
    let mut out = Vec::with_capacity(n_lines);
    let mut y = ps.space_before;
    let mut pos = 0usize;
    for (i, toks) in lines.into_iter().enumerate() {
        let (mut asc, mut desc) = (0.0f32, 0.0f32);
        for t in &toks {
            let (a, d) = fb.metrics(&t.style);
            asc = asc.max(a);
            desc = desc.max(d);
        }
        if toks.is_empty() {
            let (a, d) = fb.metrics(&empty_style);
            asc = a;
            desc = d;
        }
        let text_h = asc + desc;
        let h = text_h * ps.line_spacing.max(0.5);
        let start = line_start(i);
        // Largura sem os espaços do fim.
        let trail = toks.iter().rev().take_while(|t| t.kind == Kind::Space).count();
        let content: f32 = toks[..toks.len() - trail].iter().map(|t| t.w).sum();
        let room = right - start;
        let last = i + 1 == n_lines;
        let spaces = toks[..toks.len() - trail].iter().filter(|t| t.kind == Kind::Space).count();
        let justify = ps.align == Align::Justify && !last && spaces > 0 && !toks.iter().any(|t| t.kind == Kind::Tab);
        let extra = if justify { ((room - content) / spaces as f32).max(0.0) } else { 0.0 };
        let offset = match ps.align {
            Align::Center => ((room - content) / 2.0).max(0.0),
            Align::Right => (room - content).max(0.0),
            _ => 0.0,
        };
        let a = toks.first().map_or(pos, |t| t.a);
        let b = toks.last().map_or(pos, |t| t.b);
        let mut segs: Vec<Seg> = Vec::new();
        let mut cx = start + offset;
        let inner = toks.len() - trail;
        for (k, t) in toks.into_iter().enumerate() {
            // Só os espaços do meio esticam (os do fim da linha não contam).
            let stretch = if t.kind == Kind::Space && k < inner && extra > 0.0 { extra } else { 0.0 };
            let w = t.w + stretch;
            // Junta com o anterior se tiver o mesmo estilo (desenha de uma vez, com kerning).
            let merge = !justify && t.kind != Kind::Tab && segs.last().is_some_and(|s: &Seg| !s.tab && s.b == t.a && (Rc::ptr_eq(&s.style, &t.style) || *s.style == *t.style));
            if merge {
                let s = segs.last_mut().unwrap();
                s.b = t.b;
                s.w += w;
                s.nat += t.w;
            } else {
                segs.push(Seg { a: t.a, b: t.b, x: cx, w, nat: t.w, style: t.style, tab: t.kind == Kind::Tab });
            }
            cx += w;
        }
        // Mede de novo os pedaços juntados (o kerning entre tokens muda um pouco).
        if !justify {
            for s in segs.iter_mut().filter(|s| !s.tab) {
                let w = fb.measure(&s.style, &p.text[s.a..s.b]);
                s.w = w;
                s.nat = w;
            }
            let mut cx = start + offset;
            for s in segs.iter_mut() {
                s.x = cx;
                cx += s.w;
            }
        }
        let x1 = segs.last().map_or(start + offset, |s| s.x + s.w);
        out.push(Line { a, b, y, h, base: asc, text_h, x0: start + offset, x1, segs });
        y += h;
        pos = b;
    }
    // A última linha vai até o fim do texto (linha vazia depois de espaços etc.).
    if let Some(l) = out.last_mut() {
        l.b = p.text.len();
    }
    // As linhas cobrem o texto todo, sem buracos.
    for i in 1..out.len() {
        let prev_b = out[i - 1].b;
        out[i].a = prev_b;
    }
    if let Some(l) = out.first_mut() {
        l.a = 0;
    }
    ParaLayout { lines: out, h: y + ps.space_after, ver: p.ver, width }
}

/// Linha que contém a posição `o` (no ponto de quebra, a linha de baixo).
pub fn line_of(l: &ParaLayout, o: usize) -> usize {
    l.lines.iter().position(|ln| o < ln.b).unwrap_or(l.lines.len() - 1).min(l.lines.len() - 1)
}

/// x (pontos) da posição `o` na linha `li`.
pub fn x_of(p: &Para, l: &ParaLayout, fb: &FontBook, li: usize, o: usize) -> f32 {
    let ln = &l.lines[li];
    for s in &ln.segs {
        if o >= s.a && o <= s.b {
            if o == s.a {
                return s.x;
            }
            if o == s.b {
                return s.x + s.w;
            }
            if s.tab {
                return s.x + s.w;
            }
            let w = fb.measure(&s.style, &p.text[s.a..o]);
            let k = if s.nat > 0.0 { s.w / s.nat } else { 1.0 };
            return s.x + w * k;
        }
    }
    if o <= ln.a { ln.x0 } else { ln.x1 }
}

/// Posição (byte) mais perto de x na linha `li`.
pub fn hit_line(p: &Para, l: &ParaLayout, fb: &FontBook, li: usize, x: f32) -> usize {
    let ln = &l.lines[li];
    let last = li + 1 == l.lines.len();
    // No fim de uma linha quebrada, o cursor fica antes do espaço do fim
    // (a posição depois dele já é o começo da linha de baixo).
    let line_end = if last {
        ln.b
    } else {
        let t = &p.text[ln.a..ln.b];
        let trimmed = t.trim_end_matches(' ').len();
        // Com espaço no fim: antes dele; palavra quebrada no meio: antes da última letra.
        ln.a + if trimmed < t.len() { trimmed } else { t.char_indices().next_back().map_or(0, |(i, _)| i) }
    };
    if x <= ln.x0 {
        return ln.a;
    }
    for s in &ln.segs {
        if x > s.x + s.w {
            continue;
        }
        if s.tab {
            return if x < s.x + s.w / 2.0 { s.a } else { s.b.min(line_end) };
        }
        let k = if s.nat > 0.0 { s.w / s.nat } else { 1.0 };
        let text = &p.text[s.a..s.b];
        let mut prev = 0.0;
        let mut acc = String::new();
        for (i, c) in text.char_indices() {
            acc.push(c);
            let w = fb.measure(&s.style, &acc) * k;
            if x < s.x + (prev + w) / 2.0 {
                return (s.a + i).min(line_end);
            }
            prev = w;
        }
        return s.b.min(line_end);
    }
    line_end
}
