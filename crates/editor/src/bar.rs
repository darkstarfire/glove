//! Barra de formatação (em cima da régua): fonte, tamanho, negrito, itálico,
//! sublinhado, cor do texto, marca-texto e alinhamento.

use glove_ui::paint::draw::*;
use glove_ui::{Glyph, Rect, Ui, SEGMENT_H, SEGMENT_W};

use crate::fontbook::FontBook;
use crate::rich::{Align, CharStyle};
use crate::view;
use crate::{ColorTarget, Editor};

/// Altura da barra.
pub const BAR_H: f32 = 32.0;

/// Tamanhos do menu de tamanho (pontos).
pub const SIZES: [f32; 16] = [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 64.0, 72.0];

/// Cores do marca-texto (menu Formatar > Fonte > Marca-texto).
pub const HIGHLIGHTS: [(&str, [u8; 4]); 5] = [
    ("Amarelo", [255, 240, 110, 255]),
    ("Verde", [170, 240, 150, 255]),
    ("Azul", [160, 210, 255, 255]),
    ("Rosa", [255, 180, 215, 255]),
    ("Laranja", [255, 200, 130, 255]),
];

/// Botão de alternar (N, I, S) desenhado com a própria letra no estilo.
fn toggle(ui: &mut Ui, fb: &FontBook, r: Rect, letter: &str, style: CharStyle, on: bool, tip_ink: bool) -> bool {
    let h = ui.hit(r);
    let pressed = h.down || on;
    let (top, bot) = if pressed { (rgba(196, 204, 218, 255), rgba(214, 220, 232, 255)) } else { (rgba(252, 252, 253, 255), rgba(226, 226, 230, 255)) };
    ui.c.fill(rrect(r.x, r.y, r.w, r.h, 4.0), &solid(rgba(140, 140, 148, 255)));
    ui.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 3.0), &vgrad(r.y, r.y + r.h, &[(0.0, top), (1.0, bot)]));
    let face = fb.face(&style);
    let px = 13.0;
    let tw = face.measure(letter, px, false);
    let ink = if tip_ink { rgba(20, 20, 24, 255) } else { rgba(120, 120, 126, 255) };
    let x = r.x + (r.w - tw) / 2.0;
    let base = r.y + r.h / 2.0 + 4.5;
    face.draw(ui.c, letter, x, base, px, false, ink);
    if style.underline {
        ui.c.fill_rect(x, base + 1.5, tw, 1.0, &solid(ink));
    }
    h.clicked
}

/// Poço de cor: o quadrado com a cor atual.
fn well(ui: &mut Ui, r: Rect, color: Option<[u8; 4]>, marker: bool) -> bool {
    let h = ui.hit(r);
    ui.c.fill(rrect(r.x, r.y, r.w, r.h, 3.0), &solid(rgba(140, 140, 148, 255)));
    ui.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 2.0), &solid(rgba(250, 250, 250, 255)));
    let inner = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
    match color {
        Some(c) => ui.c.fill_rect(inner.x, inner.y, inner.w, inner.h, &solid(rgba(c[0], c[1], c[2], c[3]))),
        // Sem cor (sem marca-texto): risco vermelho na diagonal.
        None => {
            let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
            pb.move_to(inner.x, inner.y + inner.h);
            pb.line_to(inner.x + inner.w, inner.y);
            ui.c.stroke(pb.finish(), &solid(rgba(210, 40, 40, 255)), 1.5);
        }
    }
    if marker {
        // Faixa de "caneta" embaixo para diferenciar do poço de texto.
        ui.c.fill_rect(inner.x, inner.y + inner.h - 3.0, inner.w, 3.0, &solid(rgba(0, 0, 0, 60)));
    }
    h.clicked
}

impl Editor {
    /// A barra de formatação em `y`.
    pub fn draw_format_bar(&mut self, ui: &mut Ui, y: f32) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, y, w, BAR_H, &vgrad(y, y + BAR_H, &[(0.0, rgba(246, 246, 248, 255)), (1.0, rgba(228, 228, 232, 255))]));
        ui.c.hline(0.0, y + BAR_H - 1.0, w, rgba(176, 176, 182, 255));
        let cy = y + (BAR_H - 22.0) / 2.0;
        let mut x = 8.0;

        // Família: abre o painel de Fontes (que tem a busca).
        let family = view::common(&self.doc, |s| s.family.clone());
        let fr = Rect::new(x, cy, 150.0, 22.0);
        let h = ui.hit(fr);
        ui.c.fill(rrect(fr.x, fr.y, fr.w, fr.h, 4.0), &solid(rgba(140, 140, 148, 255)));
        ui.c.fill(rrect(fr.x + 1.0, fr.y + 1.0, fr.w - 2.0, fr.h - 2.0, 3.0), &vgrad(fr.y, fr.y + fr.h, &[(0.0, rgba(252, 252, 253, 255)), (1.0, rgba(230, 230, 234, 255))]));
        let label = family.clone().unwrap_or_else(|| "—".into());
        let shown = ui.fonts.ellipsize(&label, 12.0, false, fr.w - 24.0);
        // O nome na própria fonte (como no menu de fontes do Mac).
        let face = self.fb.face(&CharStyle { family: label.clone(), ..CharStyle::default() });
        face.draw(ui.c, &shown, fr.x + 7.0, fr.y + 15.5, 12.0, false, rgba(20, 20, 24, 255));
        ui.c.fill(tri(fr.x + fr.w - 12.0, fr.y + 11.0), &solid(rgba(60, 60, 66, 255)));
        if h.clicked {
            self.run_action(Some("fonts"), ui);
        }
        x += fr.w + 6.0;

        // Tamanho.
        let size = view::common(&self.doc, |s| s.size.to_bits()).map(f32::from_bits);
        let opts: Vec<String> = SIZES.iter().map(|s| format!("{}", *s as i32)).collect();
        let mut idx = size.and_then(|s| SIZES.iter().position(|v| (*v - s).abs() < 0.01)).unwrap_or(usize::MAX);
        let mut chosen = None;
        ui.area(Rect::new(x, cy, 58.0, 22.0), |ui| {
            let mut i = if idx == usize::MAX { 4 } else { idx };
            if ui.popup(&mut i, &opts, 58.0) {
                chosen = Some(i);
            }
        });
        if let Some(i) = chosen {
            idx = i;
            let v = SIZES[idx];
            self.doc.set_char_style(&|s| s.size = v);
        }
        x += 64.0;

        // Negrito, itálico, sublinhado (letras do TextEdit em português: N, I, S).
        let bold = view::common(&self.doc, |s| s.bold) == Some(true);
        let italic = view::common(&self.doc, |s| s.italic) == Some(true);
        let under = view::common(&self.doc, |s| s.underline) == Some(true);
        let fb = self.fb.clone();
        let b = CharStyle { bold: true, ..CharStyle::default() };
        if toggle(ui, &fb, Rect::new(x, cy, 26.0, 22.0), "N", b, bold, true) {
            self.run_action(Some("bold"), ui);
        }
        x += 28.0;
        let i = CharStyle { italic: true, ..CharStyle::default() };
        if toggle(ui, &fb, Rect::new(x, cy, 26.0, 22.0), "I", i, italic, true) {
            self.run_action(Some("italic"), ui);
        }
        x += 28.0;
        let u = CharStyle { underline: true, ..CharStyle::default() };
        if toggle(ui, &fb, Rect::new(x, cy, 26.0, 22.0), "S", u, under, true) {
            self.run_action(Some("underline"), ui);
        }
        x += 36.0;

        // Cor do texto e marca-texto.
        let color = view::common(&self.doc, |s| s.color);
        if well(ui, Rect::new(x, cy, 26.0, 22.0), Some(color.unwrap_or([0, 0, 0, 255])), false) {
            self.open_colors(ColorTarget::Text);
        }
        x += 30.0;
        let hl = view::common(&self.doc, |s| s.highlight).flatten();
        if well(ui, Rect::new(x, cy, 26.0, 22.0), hl, true) {
            self.open_colors(ColorTarget::Highlight);
        }
        x += 38.0;

        // Alinhamento do parágrafo.
        let cur = match view::para_style(&self.doc).align {
            Align::Left => 0,
            Align::Center => 1,
            Align::Right => 2,
            Align::Justify => 3,
        };
        let r = Rect::new(x, y + (BAR_H - SEGMENT_H) / 2.0, 4.0 * SEGMENT_W, SEGMENT_H);
        if let Some(i) = ui.segmented_in(r, &[Glyph::AlignLeft, Glyph::AlignCenter, Glyph::AlignRight, Glyph::AlignJustify], Some(cur), &[true, true, true, true]) {
            self.set_align([Align::Left, Align::Center, Align::Right, Align::Justify][i]);
        }
        x += 4.0 * SEGMENT_W + 8.0;

        // Entrelinhas (1,0 / 1,15 / 1,5 / 2,0 / Outro…).
        let line = view::para_style(&self.doc).line_spacing;
        let mut opts: Vec<String> = crate::para::LINE_OPTIONS.iter().map(|v| crate::para::fmt_num(*v)).collect();
        opts.push(glove_config::tr!("Outro…").to_string());
        let cur = crate::para::LINE_OPTIONS.iter().position(|v| (*v - line).abs() < 0.01).unwrap_or(opts.len() - 1);
        let mut chosen = None;
        ui.area(Rect::new(x, cy, 70.0, 22.0), |ui| {
            let mut i = cur;
            if ui.popup(&mut i, &opts, 70.0) {
                chosen = Some(i);
            }
        });
        match chosen {
            Some(i) if i < crate::para::LINE_OPTIONS.len() => {
                let v = crate::para::LINE_OPTIONS[i];
                self.doc.set_para_style(&move |s| s.line_spacing = v);
            }
            Some(_) => self.run_action(Some("spacing"), ui),
            None => {}
        }
        x += 76.0;
        self.bar_end = x;
    }
}

/// Triângulo pequeno apontando para baixo (botão que abre algo).
fn tri(cx: f32, cy: f32) -> Option<glove_ui::paint::tiny_skia::Path> {
    let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
    pb.move_to(cx - 3.5, cy - 2.0);
    pb.line_to(cx + 3.5, cy - 2.0);
    pb.line_to(cx, cy + 2.5);
    pb.close();
    pb.finish()
}
