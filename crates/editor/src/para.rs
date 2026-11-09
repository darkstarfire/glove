//! Parágrafos: a régua com os recuos e as tabulações (arrastáveis) e a folha de
//! Espaçamento (entre linhas, antes, depois e os recuos em centímetros).

use glove_config::tr;
use glove_ui::paint::draw::*;
use glove_ui::paint::Color;
use glove_ui::{Rect, Ui};

use crate::view;
use crate::Editor;

/// Pontos por centímetro.
pub const CM: f32 = 28.3465;

/// O que está sendo arrastado na régua.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    First,
    Left,
    Right,
    Tab(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct RulerDrag {
    pub marker: Marker,
    /// Posição atual do marcador (pontos a partir da margem esquerda do texto).
    pub at: f32,
    /// Saiu da régua: a tabulação vai ser apagada.
    pub away: bool,
}

/// Encaixa a cada 0,25 cm.
fn snap(pt: f32) -> f32 {
    let step = CM / 4.0;
    (pt / step).round() * step
}

/// Número com vírgula (1,5) ou ponto (1.5).
pub fn parse_num(s: &str) -> Option<f32> {
    s.trim().replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Número para mostrar, com vírgula e sem zeros sobrando.
pub fn fmt_num(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// Folha de Espaçamento: os campos como texto (vírgula decimal).
pub struct SpacingSheet {
    pub line: String,
    pub before: String,
    pub after: String,
    pub first: String,
    pub left: String,
    pub right: String,
    focus: bool,
    /// O primeiro campo ainda tem o valor inicial: a primeira tecla o troca.
    fresh: bool,
}

impl SpacingSheet {
    pub fn from(ps: &crate::rich::ParaStyle) -> Self {
        Self {
            line: fmt_num(ps.line_spacing),
            before: fmt_num(ps.space_before),
            after: fmt_num(ps.space_after),
            first: fmt_num(ps.first_indent / CM),
            left: fmt_num(ps.left_indent / CM),
            right: fmt_num(ps.right_indent / CM),
            focus: true,
            fresh: true,
        }
    }
}

/// Linha de entrelinhas do menu da barra.
pub const LINE_OPTIONS: [f32; 4] = [1.0, 1.15, 1.5, 2.0];

fn down_tri(c: &mut Canvas, x: f32, y: f32, fill: Color) {
    let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
    pb.move_to(x - 5.0, y);
    pb.line_to(x + 5.0, y);
    pb.line_to(x, y + 6.0);
    pb.close();
    let p = pb.finish();
    c.fill(p.clone(), &solid(fill));
    c.stroke(p, &solid(rgba(60, 60, 70, 255)), 1.0);
}

impl Editor {
    /// A régua: centímetros, recuos e tabulações do parágrafo do cursor.
    pub fn draw_ruler(&mut self, ui: &mut Ui, y: f32, h: f32) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, y, w, h, &vgrad(y, y + h, &[(0.0, rgba(244, 244, 246, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(0.0, y + h - 1.0, w, rgba(150, 150, 156, 255));
        let z = self.view.zoom;
        let x0 = self.view.text_left();
        let col = self.view.column_width();
        let right_edge = w - glove_ui::SCROLLBAR_W;
        // Faixa da coluna de texto (branca), como no TextEdit.
        ui.c.fill_rect(x0, y + 3.0, (col * z).min(right_edge - x0), h - 8.0, &solid(rgba(255, 255, 255, 170)));
        // Centímetros, com o meio centímetro mais curto.
        let cm = CM * z;
        let ink = rgba(90, 90, 96, 255);
        let mut k = 0;
        while x0 + k as f32 * cm / 2.0 < right_edge {
            let x = x0 + k as f32 * cm / 2.0;
            let tall = k % 2 == 0;
            ui.c.vline(x.round() + 0.5, y + h - if tall { 8.0 } else { 5.0 }, if tall { 7.0 } else { 4.0 }, ink);
            if tall && k > 0 {
                let t = (k / 2).to_string();
                let tw = ui.text_width(&t, 9.0, false);
                ui.fonts.draw(ui.c, &t, x - tw / 2.0, y + 11.0, 9.0, false, ink);
            }
            k += 1;
        }

        // Marcadores do parágrafo do cursor (ou o que está sendo arrastado).
        let ps = view::para_style(&self.doc).clone();
        let sx = |pt: f32| x0 + pt * z;
        let drag = self.ruler_drag;
        let pos = |m: Marker, normal: f32| match drag {
            Some(d) if d.marker == m => d.at,
            _ => normal,
        };
        let left = pos(Marker::Left, ps.left_indent);
        let first = match drag {
            Some(d) if d.marker == Marker::First => d.at,
            // Arrastando o recuo esquerdo, a primeira linha vai junto.
            Some(d) if d.marker == Marker::Left => d.at + ps.first_indent,
            _ => ps.left_indent + ps.first_indent,
        };
        let right = pos(Marker::Right, col - ps.right_indent);
        let blue = rgba(140, 170, 220, 255);
        // Primeira linha: barrinha em cima; recuos: triângulos embaixo.
        ui.c.fill_rect(sx(first) - 5.0, y + 2.0, 10.0, 4.0, &solid(blue));
        ui.c.stroke(rrect(sx(first) - 5.0, y + 2.0, 10.0, 4.0, 0.0), &solid(rgba(60, 60, 70, 255)), 1.0);
        down_tri(ui.c, sx(left), y + h - 8.0, blue);
        down_tri(ui.c, sx(right), y + h - 8.0, blue);
        // Tabulações: um "L".
        for (i, t) in ps.tabs.iter().enumerate() {
            let t = match drag {
                Some(d) if d.marker == Marker::Tab(i) => {
                    if d.away {
                        continue;
                    }
                    d.at
                }
                _ => *t,
            };
            let x = sx(t).round();
            ui.c.fill_rect(x, y + h - 10.0, 1.5, 7.0, &solid(rgba(30, 30, 36, 255)));
            ui.c.fill_rect(x, y + h - 4.5, 5.0, 1.5, &solid(rgba(30, 30, 36, 255)));
        }

        // Interação: pegar um marcador, ou clicar no vazio para pôr uma tabulação.
        let area = Rect::new(0.0, y, right_edge, h);
        let hit = ui.hit(area);
        let (px, py) = ui.pointer();
        if hit.pressed {
            let near = |x: f32| (px - x).abs() <= 6.0;
            let top_half = py < y + h / 2.0;
            let m = if top_half && near(sx(first)) {
                Some((Marker::First, first))
            } else if near(sx(left)) && !top_half {
                Some((Marker::Left, left))
            } else if near(sx(right)) && !top_half {
                Some((Marker::Right, right))
            } else {
                ps.tabs.iter().position(|t| near(sx(*t))).map(|i| (Marker::Tab(i), ps.tabs[i]))
            };
            match m {
                Some((marker, at)) => self.ruler_drag = Some(RulerDrag { marker, at, away: false }),
                None if px >= x0 && px <= sx(col) => {
                    // Tabulação nova onde clicou (já pode ser arrastada).
                    let at = snap((px - x0) / z).clamp(0.0, col);
                    let mut tabs = ps.tabs.clone();
                    tabs.push(at);
                    tabs.sort_by(f32::total_cmp);
                    let i = tabs.iter().position(|t| *t == at).unwrap_or(0);
                    self.doc.set_para_style(&|s| s.tabs = tabs.clone());
                    self.ruler_drag = Some(RulerDrag { marker: Marker::Tab(i), at, away: false });
                }
                None => {}
            }
        }
        if let Some(mut d) = self.ruler_drag {
            if ui.mouse_down() {
                let v = snap((px - x0) / z);
                d.at = match d.marker {
                    Marker::First => v.clamp(0.0, col - 20.0),
                    Marker::Left => v.clamp(0.0, right - 20.0),
                    Marker::Right => v.clamp(left + 20.0, col),
                    Marker::Tab(_) => v.clamp(0.0, col),
                };
                d.away = matches!(d.marker, Marker::Tab(_)) && (py < y - 12.0 || py > y + h + 20.0);
                self.ruler_drag = Some(d);
            } else {
                // Soltou: aplica nos parágrafos selecionados (um passo do Desfazer).
                self.ruler_drag = None;
                let at = d.at;
                match d.marker {
                    Marker::First => {
                        let l = ps.left_indent;
                        self.doc.set_para_style(&move |s| s.first_indent = at - l);
                    }
                    Marker::Left => self.doc.set_para_style(&move |s| s.left_indent = at),
                    Marker::Right => self.doc.set_para_style(&move |s| s.right_indent = (col - at).max(0.0)),
                    Marker::Tab(i) => {
                        let mut tabs = ps.tabs.clone();
                        if i < tabs.len() {
                            if d.away {
                                tabs.remove(i);
                            } else {
                                tabs[i] = at;
                            }
                        }
                        tabs.sort_by(f32::total_cmp);
                        tabs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
                        self.doc.set_para_style(&move |s| s.tabs = tabs.clone());
                    }
                }
            }
        }
    }

    /// A folha de Espaçamento. Devolve verdadeiro quando fechou.
    pub fn spacing_sheet(ui: &mut Ui, f: &mut SpacingSheet) -> Option<bool> {
        let (w, h) = (ui.c.w, ui.c.h);
        let (sw, sh) = (360.0f32.min(w - 20.0), 262.0);
        let sx = ((w - sw) / 2.0).round();
        ui.set_enabled(true);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
        let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
        ui.c.fill(shape(), &solid(ui.theme.body_b));
        ui.c.pinstripes(sx, 0.0, sw, sh - 6.0, ui.theme.body_a, ui.theme.body_b);
        ui.c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0);
        let ink = rgba(20, 20, 24, 255);
        ui.fonts.draw(ui.c, tr!("Espaçamento"), sx + 18.0, 26.0, 13.0, true, ink);
        let focus = std::mem::take(&mut f.focus);
        if f.fresh && (!ui.typed().is_empty() || ui.keys().iter().any(|k| matches!(k.key, glove_ui::ui::Key::Backspace | glove_ui::ui::Key::Delete))) {
            f.line.clear();
            f.fresh = false;
        }
        if ui.mouse_pressed() {
            f.fresh = false;
        }
        let mut enter = false;
        let mut row = |ui: &mut Ui, y: f32, label: &str, unit: &str, v: &mut String, first: bool| {
            let lw = ui.text_width(label, 12.0, false);
            ui.fonts.draw(ui.c, label, sx + 190.0 - lw, y + 15.0, 12.0, false, ink);
            let (_, e) = ui.text_field_in(Rect::new(sx + 200.0, y, 70.0, 22.0), v, first);
            ui.fonts.draw(ui.c, unit, sx + 278.0, y + 15.0, 12.0, false, rgba(90, 90, 96, 255));
            enter |= e;
        };
        row(ui, 42.0, tr!("Entre linhas:"), tr!("× a linha"), &mut f.line, focus);
        row(ui, 70.0, tr!("Antes do parágrafo:"), "pt", &mut f.before, false);
        row(ui, 98.0, tr!("Depois do parágrafo:"), "pt", &mut f.after, false);
        row(ui, 136.0, tr!("Recuo da primeira linha:"), "cm", &mut f.first, false);
        row(ui, 164.0, tr!("Recuo esquerdo:"), "cm", &mut f.left, false);
        row(ui, 192.0, tr!("Recuo direito:"), "cm", &mut f.right, false);
        let by = sh - 38.0;
        let (mut cancel, mut ok) = (false, false);
        ui.area(Rect::new(sx + sw - 220.0, by, 95.0, 24.0), |ui| cancel = ui.button(tr!("Cancelar")));
        ui.area(Rect::new(sx + sw - 115.0, by, 97.0, 24.0), |ui| ok = ui.default_button("OK"));
        if ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            cancel = true;
        }
        ui.set_enabled(false);
        if cancel {
            return Some(false);
        }
        if ok || enter {
            return Some(true);
        }
        None
    }
}
