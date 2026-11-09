//! Configurar Página: papel, orientação e margens.

use glove_config::tr;
use glove_ui::paint::draw::*;
use glove_ui::{Rect, Ui};

use crate::para::{fmt_num, parse_num, CM};
use crate::rich::PageSetup;

/// Papéis (nome, largura, altura em pontos, em retrato).
pub const PAPERS: [(&str, f32, f32); 4] = [("A4", 595.28, 841.89), ("Carta", 612.0, 792.0), ("Ofício", 612.0, 1008.0), ("A5", 419.53, 595.28)];

pub struct PageSheet {
    pub paper: usize,
    pub landscape: bool,
    pub top: String,
    pub bottom: String,
    pub left: String,
    pub right: String,
}

impl PageSheet {
    pub fn from(pg: &PageSetup) -> Self {
        let landscape = pg.width > pg.height;
        let (w, h) = if landscape { (pg.height, pg.width) } else { (pg.width, pg.height) };
        let paper = PAPERS.iter().position(|(_, pw, ph)| (pw - w).abs() < 2.0 && (ph - h).abs() < 2.0).unwrap_or(0);
        Self { paper, landscape, top: fmt_num(pg.margin_top / CM), bottom: fmt_num(pg.margin_bottom / CM), left: fmt_num(pg.margin_left / CM), right: fmt_num(pg.margin_right / CM) }
    }

    /// O papel escolhido (as margens que não forem número ficam como estavam).
    pub fn apply(&self, pg: &PageSetup) -> PageSetup {
        let (_, w, h) = PAPERS[self.paper.min(PAPERS.len() - 1)];
        let (w, h) = if self.landscape { (h, w) } else { (w, h) };
        let m = |s: &str, old: f32| parse_num(s).map(|v| (v * CM).clamp(0.0, w.min(h) / 3.0)).unwrap_or(old);
        PageSetup { width: w, height: h, margin_top: m(&self.top, pg.margin_top), margin_bottom: m(&self.bottom, pg.margin_bottom), margin_left: m(&self.left, pg.margin_left), margin_right: m(&self.right, pg.margin_right) }
    }
}

/// A folha. Devolve Some(true) = OK, Some(false) = Cancelar.
pub fn page_sheet(ui: &mut Ui, f: &mut PageSheet) -> Option<bool> {
    let (w, h) = (ui.c.w, ui.c.h);
    let (sw, sh) = (360.0f32.min(w - 20.0), 250.0);
    let sx = ((w - sw) / 2.0).round();
    ui.set_enabled(true);
    ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
    let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
    ui.c.fill(shape(), &solid(ui.theme.body_b));
    ui.c.pinstripes(sx, 0.0, sw, sh - 6.0, ui.theme.body_a, ui.theme.body_b);
    ui.c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0);
    let ink = rgba(20, 20, 24, 255);
    ui.fonts.draw(ui.c, tr!("Configurar Página"), sx + 18.0, 26.0, 13.0, true, ink);
    let label = |ui: &mut Ui, y: f32, t: &str| {
        let lw = ui.text_width(t, 12.0, false);
        ui.fonts.draw(ui.c, t, sx + 130.0 - lw, y + 15.0, 12.0, false, ink);
    };
    label(ui, 42.0, tr!("Papel:"));
    let names: Vec<String> = PAPERS.iter().map(|(n, w, h)| format!("{} ({} × {} cm)", glove_config::tr_dyn(n), fmt_num(w / CM), fmt_num(h / CM))).collect();
    let mut paper = f.paper;
    ui.area(Rect::new(sx + 140.0, 42.0, 200.0, 22.0), |ui| {
        ui.popup(&mut paper, &names, 200.0);
    });
    f.paper = paper;
    label(ui, 72.0, tr!("Orientação:"));
    let mut o = usize::from(f.landscape);
    ui.area(Rect::new(sx + 140.0, 72.0, 200.0, 22.0), |ui| {
        if let Some(i) = ui.segmented_text(&[tr!("Retrato"), tr!("Paisagem")], Some(o)) {
            o = i;
        }
    });
    f.landscape = o == 1;
    let row = |ui: &mut Ui, y: f32, t: &str, v: &mut String| {
        label(ui, y, t);
        ui.text_field_in(Rect::new(sx + 140.0, y, 60.0, 22.0), v, false);
        ui.fonts.draw(ui.c, "cm", sx + 206.0, y + 15.0, 12.0, false, rgba(90, 90, 96, 255));
    };
    row(ui, 108.0, tr!("Margem de cima:"), &mut f.top);
    row(ui, 134.0, tr!("Margem de baixo:"), &mut f.bottom);
    row(ui, 160.0, tr!("Margem esquerda:"), &mut f.left);
    row(ui, 186.0, tr!("Margem direita:"), &mut f.right);
    let by = sh - 34.0;
    let (mut cancel, mut ok) = (false, false);
    ui.area(Rect::new(sx + sw - 220.0, by, 95.0, 24.0), |ui| cancel = ui.button(tr!("Cancelar")));
    ui.area(Rect::new(sx + sw - 115.0, by, 97.0, 24.0), |ui| ok = ui.default_button("OK"));
    let keys: Vec<glove_ui::ui::Key> = ui.keys().iter().map(|k| k.key).collect();
    if keys.contains(&glove_ui::ui::Key::Escape) {
        cancel = true;
    }
    if keys.contains(&glove_ui::ui::Key::Enter) && !ui.has_focus() {
        ok = true;
    }
    ui.set_enabled(false);
    if cancel {
        return Some(false);
    }
    ok.then_some(true)
}
