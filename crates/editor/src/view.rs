//! A área de edição do documento rico: teclado, mouse, seleção, rolagem e o
//! desenho (só o que aparece na tela). Cada parágrafo guarda a sua diagramação
//! e só é refeito quando muda.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use glove_ui::paint::draw::*;
use glove_ui::ui::Key;
use glove_ui::{Rect, Ui};

use crate::fontbook::FontBook;
use crate::layout::{hit_line, layout_para, line_of, x_of, ParaLayout};
use crate::rich::{Doc, Frag, Para, Pos};

/// Margens do texto no modo "ajustar à janela" (pontos).
const MARGIN_X: f32 = 24.0;
const MARGIN_Y: f32 = 14.0;

thread_local! {
    /// O último pedaço copiado no Editor, com os estilos (os outros apps só veem o texto).
    static CLIP: std::cell::RefCell<Option<(String, Frag)>> = const { std::cell::RefCell::new(None) };
}

pub struct DocView {
    /// Rolagem (pixels da tela) e zoom (pixels por ponto).
    pub scroll: f32,
    pub zoom: f32,
    cache: HashMap<u64, ParaLayout>,
    /// Topo de cada parágrafo (pontos, a partir do topo do texto) e altura total.
    tops: Vec<f32>,
    total: f32,
    width: f32,
    /// Leva o cursor para dentro da área visível no próximo quadro.
    pub reveal: bool,
    want_x: Option<f32>,
    dragging: bool,
    /// Cliques seguidos: quando, onde e quantos (duplo, triplo).
    last_click: Option<(Instant, (f32, f32), u8)>,
    /// Esperando o texto colado.
    pasting: bool,
}

impl Default for DocView {
    fn default() -> Self {
        Self::new()
    }
}

/// Uma posição na tela: x, topo da linha e altura do texto (pontos).
#[derive(Clone, Copy, Debug)]
pub struct Caret {
    pub x: f32,
    pub y: f32,
    pub h: f32,
}

impl DocView {
    pub fn new() -> Self {
        Self { scroll: 0.0, zoom: 1.0, cache: HashMap::new(), tops: Vec::new(), total: 0.0, width: 0.0, reveal: false, want_x: None, dragging: false, last_click: None, pasting: false }
    }

    fn lay<'a>(&'a self, p: &Para) -> &'a ParaLayout {
        &self.cache[&p.id]
    }

    /// Diagrama o que mudou e recalcula os topos.
    fn ensure(&mut self, doc: &Doc, fb: &FontBook, width: f32) {
        let width = width.max(60.0);
        if (width - self.width).abs() > 0.01 {
            self.width = width;
        }
        self.tops.clear();
        let mut y = 0.0;
        for p in &doc.paras {
            let fresh = self.cache.get(&p.id).is_some_and(|l| l.ver == p.ver && (l.width - width).abs() < 0.01);
            if !fresh {
                self.cache.insert(p.id, layout_para(p, width, fb));
            }
            self.tops.push(y);
            y += self.cache[&p.id].h;
        }
        self.total = y;
        // Esquece os parágrafos que não existem mais (de vez em quando).
        if self.cache.len() > doc.paras.len() * 2 + 64 {
            let ids: std::collections::HashSet<u64> = doc.paras.iter().map(|p| p.id).collect();
            self.cache.retain(|k, _| ids.contains(k));
        }
    }

    /// Onde fica a posição (pontos, a partir do canto do texto).
    pub fn caret(&self, doc: &Doc, fb: &FontBook, pos: Pos) -> Caret {
        let p = &doc.paras[pos.p];
        let l = self.lay(p);
        let li = line_of(l, pos.o);
        let ln = &l.lines[li];
        Caret { x: x_of(p, l, fb, li, pos.o), y: self.tops[pos.p] + ln.y, h: ln.h }
    }

    /// Posição mais perto do ponto (pontos, a partir do canto do texto).
    pub fn hit(&self, doc: &Doc, fb: &FontBook, x: f32, y: f32) -> Pos {
        if y < 0.0 {
            return Pos::new(0, 0);
        }
        let pi = self.tops.partition_point(|&t| t <= y).saturating_sub(1).min(doc.paras.len() - 1);
        let p = &doc.paras[pi];
        let l = self.lay(p);
        let ly = y - self.tops[pi];
        let li = l.lines.iter().position(|ln| ly < ln.y + ln.h).unwrap_or(l.lines.len() - 1);
        Pos::new(pi, hit_line(p, l, fb, li, x))
    }

    /// Uma linha acima ou abaixo, mantendo o x.
    fn vertical(&mut self, doc: &Doc, fb: &FontBook, pos: Pos, delta: i32) -> Pos {
        let c = self.caret(doc, fb, pos);
        let x = *self.want_x.get_or_insert(c.x);
        let p = &doc.paras[pos.p];
        let l = self.lay(p);
        let li = line_of(l, pos.o) as i32 + delta;
        if li >= 0 && (li as usize) < l.lines.len() {
            return Pos::new(pos.p, hit_line(p, l, fb, li as usize, x));
        }
        if delta < 0 {
            if pos.p == 0 {
                return Pos::new(0, 0);
            }
            let q = &doc.paras[pos.p - 1];
            let lq = self.lay(q);
            Pos::new(pos.p - 1, hit_line(q, lq, fb, lq.lines.len() - 1, x))
        } else {
            if pos.p + 1 >= doc.paras.len() {
                return doc.end();
            }
            let q = &doc.paras[pos.p + 1];
            Pos::new(pos.p + 1, hit_line(q, self.lay(q), fb, 0, x))
        }
    }

    /// Começo e fim da linha (na tela) da posição.
    fn line_bounds(&self, doc: &Doc, pos: Pos) -> (Pos, Pos) {
        let p = &doc.paras[pos.p];
        let l = self.lay(p);
        let li = line_of(l, pos.o);
        let ln = &l.lines[li];
        let end = if li + 1 == l.lines.len() {
            ln.b
        } else {
            let t = &p.text[ln.a..ln.b];
            let trimmed = t.trim_end_matches(' ').len();
            ln.a + if trimmed < t.len() { trimmed } else { t.char_indices().next_back().map_or(0, |(i, _)| i) }
        };
        (Pos::new(pos.p, ln.a), Pos::new(pos.p, end))
    }

    /// Onde começa o texto na área (pixels a partir da esquerda dela).
    pub fn text_left(&self) -> f32 {
        MARGIN_X * self.zoom
    }

    /// Largura da coluna de texto (pontos).
    pub fn column_width(&self) -> f32 {
        self.width
    }

    /// Altura do documento em pixels (com as margens).
    pub fn content_h(&self) -> f32 {
        (self.total + 2.0 * MARGIN_Y) * self.zoom
    }

    /// A área de edição em `r`. Devolve verdadeiro se o documento mudou.
    pub fn ui(&mut self, ui: &mut Ui, doc: &mut Doc, fb: &FontBook, r: Rect, enabled: bool) -> bool {
        let z = self.zoom;
        let sb_w = glove_ui::SCROLLBAR_W;
        let width = (r.w - sb_w) / z - 2.0 * MARGIN_X;
        self.ensure(doc, fb, width);
        let ver = doc.version;

        // Barra de rolagem (e a roda do mouse).
        let mut scroll = self.scroll;
        let inner = ui.scroll_area(r, &mut scroll, self.content_h().max(r.h), 30.0);
        self.scroll = scroll;
        let origin = (inner.x + MARGIN_X * z, inner.y + MARGIN_Y * z);
        let to_doc = |sx: f32, sy: f32, scroll: f32| ((sx - origin.0) / z, (sy - origin.1 + scroll) / z);

        // Mouse
        // O controle existe sempre (a numeração dos controles não pode mudar entre
        // quadros, senão um campo que pega o foco sozinho acerta o vizinho).
        let hit = ui.hit(inner);
        let resp = enabled.then_some(hit);
        if let Some(resp) = resp {
            if resp.pressed {
                let pt = ui.pointer();
                let n = match self.last_click {
                    Some((t, p, n)) if t.elapsed() < Duration::from_millis(450) && (p.0 - pt.0).abs() < 5.0 && (p.1 - pt.1).abs() < 5.0 => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), pt, n));
                let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                let pos = self.hit(doc, fb, x, y);
                match n {
                    2 => doc.select_word(pos),
                    3 => doc.select_para(pos.p),
                    _ => doc.set_cursor(pos, ui.mods().shift),
                }
                self.want_x = None;
                self.dragging = true;
            }
        }
        if self.dragging {
            if ui.mouse_down() {
                if ui.moved() {
                    let pt = ui.pointer();
                    // Arrastando para fora: rola junto.
                    if pt.1 < inner.y {
                        self.scroll = (self.scroll - (inner.y - pt.1)).max(0.0);
                    } else if pt.1 > inner.y + inner.h {
                        self.scroll += pt.1 - inner.y - inner.h;
                    }
                    let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                    let pos = self.hit(doc, fb, x, y);
                    if pos != doc.cursor {
                        doc.cursor = pos;
                        doc.typing = None;
                    }
                }
            } else {
                self.dragging = false;
            }
        }

        // Teclado (só com nenhum campo de texto com o cursor).
        if enabled && !ui.has_focus() {
            self.keyboard(ui, doc, fb, inner);
        }

        let changed = doc.version != ver;
        if changed {
            self.ensure(doc, fb, width);
        }
        // Cursor sempre à vista depois de mexer pelo teclado.
        if self.reveal {
            self.reveal = false;
            let c = self.caret(doc, fb, doc.cursor);
            let top = (MARGIN_Y + c.y) * z;
            let bot = (MARGIN_Y + c.y + c.h) * z;
            if top < self.scroll {
                self.scroll = top - 4.0;
            } else if bot > self.scroll + inner.h {
                self.scroll = bot - inner.h + 4.0;
            }
        }
        self.scroll = self.scroll.clamp(0.0, (self.content_h() - inner.h).max(0.0));

        // Desenho
        let active = ui.window_active();
        let show_caret = enabled && !ui.has_focus();
        let scroll = self.scroll;
        let me = &*self;
        let docr = &*doc;
        ui.clipped(0xd0c_u64, inner, |c, _| {
            c.fill_rect(0.0, 0.0, inner.w, inner.h, &solid(rgba(255, 255, 255, 255)));
            me.draw(c, docr, fb, (MARGIN_X * z, MARGIN_Y * z - scroll), inner.h, active, show_caret);
        });
        changed
    }

    fn keyboard(&mut self, ui: &mut Ui, doc: &mut Doc, fb: &FontBook, inner: Rect) {
        // Texto colado que chegou.
        if self.pasting {
            if let Some(t) = ui.take_paste() {
                self.pasting = false;
                let rich = CLIP.with(|c| c.borrow().as_ref().filter(|(s, _)| *s == t).map(|(_, f)| f.clone()));
                match rich {
                    Some(f) => doc.insert_frag(f),
                    None => {
                        doc.insert_text(&t.replace("\r\n", "\n"));
                        doc.break_undo();
                    }
                }
                self.reveal = true;
            }
        }
        let typed: String = ui.typed().chars().filter(|c| !c.is_control()).collect();
        if !typed.is_empty() {
            for ch in typed.chars() {
                doc.insert_text(&ch.to_string());
            }
            self.want_x = None;
            self.reveal = true;
        }
        let keys = ui.keys().to_vec();
        for k in keys {
            let combo = k.combo.as_str();
            let shift = combo.contains("Shift+");
            let ctrl = combo.contains("Ctrl+");
            let mut moved: Option<Pos> = None;
            let mut keep_x = false;
            match k.key {
                Key::Left => {
                    moved = Some(if doc.has_selection() && !shift { doc.selection().0 } else if ctrl { doc.word_left(doc.cursor) } else { doc.prev_pos(doc.cursor) });
                }
                Key::Right => {
                    moved = Some(if doc.has_selection() && !shift { doc.selection().1 } else if ctrl { doc.word_right(doc.cursor) } else { doc.next_pos(doc.cursor) });
                }
                Key::Up => {
                    moved = Some(if ctrl { Pos::new(doc.cursor.p.saturating_sub(if doc.cursor.o == 0 { 1 } else { 0 }), 0) } else { self.vertical(doc, fb, doc.cursor, -1) });
                    keep_x = !ctrl;
                }
                Key::Down => {
                    moved = Some(if ctrl {
                        let p = (doc.cursor.p + 1).min(doc.paras.len() - 1);
                        if p == doc.cursor.p { doc.end() } else { Pos::new(p, 0) }
                    } else {
                        self.vertical(doc, fb, doc.cursor, 1)
                    });
                    keep_x = !ctrl;
                }
                Key::Home => moved = Some(if ctrl { Pos::new(0, 0) } else { self.line_bounds(doc, doc.cursor).0 }),
                Key::End => moved = Some(if ctrl { doc.end() } else { self.line_bounds(doc, doc.cursor).1 }),
                Key::PageUp | Key::PageDown => {
                    let c = self.caret(doc, fb, doc.cursor);
                    let x = *self.want_x.get_or_insert(c.x);
                    let page = (inner.h / self.zoom - 20.0).max(20.0);
                    let dy = if k.key == Key::PageUp { -page } else { page };
                    self.scroll = (self.scroll + dy * self.zoom).max(0.0);
                    moved = Some(self.hit(doc, fb, x, c.y + c.h / 2.0 + dy));
                    keep_x = true;
                }
                Key::Backspace => {
                    if doc.has_selection() {
                        doc.delete_selection();
                    } else {
                        let to = if ctrl { doc.word_left(doc.cursor) } else { doc.prev_pos(doc.cursor) };
                        doc.delete_range(to, doc.cursor);
                    }
                    self.want_x = None;
                    self.reveal = true;
                }
                Key::Delete => {
                    if doc.has_selection() {
                        doc.delete_selection();
                    } else {
                        let to = if ctrl { doc.word_right(doc.cursor) } else { doc.next_pos(doc.cursor) };
                        doc.delete_range(doc.cursor, to);
                    }
                    self.want_x = None;
                    self.reveal = true;
                }
                Key::Enter if !ctrl => {
                    doc.insert_text("\n");
                    self.want_x = None;
                    self.reveal = true;
                }
                Key::Tab if !ctrl => {
                    doc.insert_text("\t");
                    self.want_x = None;
                    self.reveal = true;
                }
                _ => match combo {
                    "Ctrl+A" => doc.select_all(),
                    "Ctrl+C" | "Ctrl+X" => {
                        if doc.has_selection() {
                            let text = doc.selected_text();
                            let frag = doc.selected_frag();
                            ui.copy(&text);
                            CLIP.with(|c| *c.borrow_mut() = Some((text, frag)));
                            if combo == "Ctrl+X" {
                                doc.delete_selection();
                                self.reveal = true;
                            }
                        }
                    }
                    "Ctrl+V" => {
                        ui.request_paste();
                        self.pasting = true;
                    }
                    "Ctrl+Z" => {
                        doc.undo();
                        self.reveal = true;
                    }
                    "Ctrl+Shift+Z" | "Ctrl+Y" => {
                        doc.redo();
                        self.reveal = true;
                    }
                    _ => {}
                },
            }
            if let Some(p) = moved {
                // Com seleção, Shift estende; sem Shift, a seta só desfaz a seleção.
                doc.set_cursor(p, shift);
                if !keep_x {
                    self.want_x = None;
                }
                self.reveal = true;
            }
        }
    }

    /// Desenha o documento: `off` = onde fica o canto do texto (pixels do canvas).
    fn draw(&self, c: &mut Canvas, doc: &Doc, fb: &FontBook, off: (f32, f32), view_h: f32, active: bool, show_caret: bool) {
        let z = self.zoom;
        let (sel_a, sel_b) = doc.selection();
        let has_sel = doc.has_selection();
        let sel_color = if active { rgba(181, 213, 255, 255) } else { rgba(212, 212, 216, 255) };
        // Primeiro parágrafo visível.
        let first = self.tops.partition_point(|&t| off.1 + t * z < 0.0).saturating_sub(1);
        for pi in first..doc.paras.len() {
            let top = off.1 + self.tops[pi] * z;
            if top > view_h {
                break;
            }
            let p = &doc.paras[pi];
            let l = self.lay(p);
            for (li, ln) in l.lines.iter().enumerate() {
                let ly = top + ln.y * z;
                if ly > view_h || ly + ln.h * z < 0.0 {
                    continue;
                }
                // Seleção: da posição inicial até a final; linha inteira no meio.
                if has_sel && Pos::new(pi, ln.a) <= sel_b && Pos::new(pi, ln.b) >= sel_a && !(sel_b == Pos::new(pi, ln.a) && ln.a != ln.b) {
                    let last_line = li + 1 == l.lines.len();
                    let x0 = if sel_a >= Pos::new(pi, ln.a) && sel_a.p == pi { x_of(p, l, fb, li, sel_a.o) } else { 0.0 };
                    let x1 = if sel_b.p == pi && sel_b.o <= ln.b && !(sel_b.o == ln.b && !last_line) {
                        x_of(p, l, fb, li, sel_b.o)
                    } else {
                        self.width
                    };
                    if x1 > x0 {
                        c.fill_rect(off.0 + x0 * z, ly, (x1 - x0) * z, ln.h * z, &solid(sel_color));
                    }
                }
                let base = ly + ln.base * z;
                for s in &ln.segs {
                    let st = &s.style;
                    let x = off.0 + s.x * z;
                    if let Some(h) = st.highlight {
                        // Da altura da própria letra (numa linha com letras maiores, não cobre tudo).
                        let (asc, desc) = fb.metrics(st);
                        c.fill_rect(x, base - asc * z, s.w * z, (asc + desc) * z, &solid(rgba(h[0], h[1], h[2], h[3])));
                    }
                    if s.tab {
                        continue;
                    }
                    let size = FontBook::eff_size(st) * z;
                    let shift = match st.script {
                        1 => -st.size * 0.33 * z,
                        -1 => st.size * 0.15 * z,
                        _ => 0.0,
                    };
                    let color = rgba(st.color[0], st.color[1], st.color[2], st.color[3]);
                    let text = &p.text[s.a..s.b];
                    let face = fb.face(st);
                    if s.w > s.nat + 0.01 && text.contains(' ') {
                        // Justificado: cada palavra no seu lugar.
                        face.draw(c, text.trim_end_matches(' '), x, base + shift, size, false, color);
                    } else {
                        face.draw(c, text, x, base + shift, size, false, color);
                    }
                    let th = (st.size / 14.0 * z).max(1.0);
                    if st.underline {
                        let w = if text.ends_with(' ') && s.b == ln.b { fb.measure(st, text.trim_end_matches(' ')) * z } else { s.w * z };
                        c.fill_rect(x, base + shift + 1.5 * z, w, th, &solid(color));
                    }
                    if st.strike {
                        c.fill_rect(x, base + shift - st.size * 0.3 * z, s.w * z, th, &solid(color));
                    }
                }
            }
        }
        // Cursor
        if show_caret && !has_sel {
            let cpos = self.caret(doc, fb, doc.cursor);
            let p = &doc.paras[doc.cursor.p];
            let l = self.lay(p);
            let ln = &l.lines[line_of(l, doc.cursor.o)];
            let x = (off.0 + cpos.x * z).round();
            let y = off.1 + cpos.y * z;
            c.fill_rect(x, y, 1.0, ln.text_h * z, &solid(rgba(0, 0, 0, if active { 255 } else { 120 })));
        }
    }
}

/// Estilo comum de um pedaço do documento (para os menus marcarem Negrito etc.).
pub fn common<T: PartialEq + Clone>(doc: &Doc, f: impl Fn(&crate::rich::CharStyle) -> T) -> Option<T> {
    if !doc.has_selection() {
        return Some(f(&doc.typing_style()));
    }
    let (a, b) = doc.selection();
    let mut out: Option<T> = None;
    for pi in a.p..=b.p {
        let p = &doc.paras[pi];
        let (s, e) = (if pi == a.p { a.o } else { 0 }, if pi == b.p { b.o } else { p.text.len() });
        for (x, y, st) in p.spans() {
            if y <= s || x >= e {
                if !(p.text.is_empty()) {
                    continue;
                }
            }
            let v = f(st);
            match &out {
                None => out = Some(v),
                Some(o) if *o != v => return None,
                _ => {}
            }
        }
    }
    out.or_else(|| Some(f(&doc.typing_style())))
}

/// Para a régua e os menus: o estilo do parágrafo do cursor.
pub fn para_style(doc: &Doc) -> &crate::rich::ParaStyle {
    &doc.paras[doc.cursor.p].style
}
