//! Folhas da nota de rodapé e do cabeçalho e rodapé.

use glove_config::tr;
use glove_ui::paint::draw::*;
use glove_ui::{Rect, TextBuffer, Ui};

use crate::rich::HeaderFooter;

/// Fundo escurecido e a folha listrada encostada no topo. Devolve o x da folha.
fn sheet_frame(ui: &mut Ui, sw: f32, sh: f32, title: &str) -> f32 {
    let (w, h) = (ui.c.w, ui.c.h);
    let sw = sw.min(w - 20.0);
    let sx = ((w - sw) / 2.0).round();
    ui.set_enabled(true);
    ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
    let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
    ui.c.fill(shape(), &solid(ui.theme.body_b));
    ui.c.pinstripes(sx, 0.0, sw, sh - 6.0, ui.theme.body_a, ui.theme.body_b);
    ui.c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0);
    ui.fonts.draw(ui.c, title, sx + 18.0, 26.0, 13.0, true, rgba(20, 20, 24, 255));
    sx
}

/// Botões Cancelar e OK no pé da folha. Devolve (cancelar, ok).
fn buttons(ui: &mut Ui, sx: f32, sw: f32, by: f32) -> (bool, bool) {
    let (mut cancel, mut ok) = (false, false);
    ui.area(Rect::new(sx + sw - 220.0, by, 95.0, 24.0), |ui| cancel = ui.button(tr!("Cancelar")));
    ui.area(Rect::new(sx + sw - 115.0, by, 97.0, 24.0), |ui| ok = ui.default_button("OK"));
    if ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
        cancel = true;
    }
    (cancel, ok)
}

pub struct NoteSheet {
    pub buf: TextBuffer,
    /// Editando uma nota que já existe (senão, é uma nota nova).
    pub id: Option<u32>,
}

/// A folha da nota. Some(Some(texto)) = OK; Some(None) = Cancelar.
pub fn note_sheet(ui: &mut Ui, f: &mut NoteSheet) -> Option<Option<String>> {
    let sw = 420.0f32.min(ui.c.w - 20.0);
    let sh = 220.0;
    let sx = sheet_frame(ui, sw, sh, if f.id.is_some() { tr!("Editar Nota de Rodapé") } else { tr!("Nota de Rodapé") });
    ui.fonts.draw(ui.c, tr!("O texto aparece no pé da página, com o número da nota."), sx + 18.0, 46.0, 11.0, false, rgba(90, 90, 96, 255));
    let r = Rect::new(sx + 18.0, 56.0, sw - 36.0, sh - 104.0);
    ui.c.fill_rect(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, &solid(rgba(130, 130, 136, 255)));
    ui.text_area(&mut f.buf, r);
    let (cancel, ok) = buttons(ui, sx, sw, sh - 38.0);
    ui.set_enabled(false);
    if cancel {
        return Some(None);
    }
    ok.then(|| Some(f.buf.text().trim_end().to_string()))
}

pub struct HfSheet {
    pub header: [String; 3],
    pub footer: [String; 3],
    pub title_page: bool,
}

impl HfSheet {
    pub fn from(h: &HeaderFooter, f: &HeaderFooter, title_page: bool) -> Self {
        Self { header: [h.left.clone(), h.center.clone(), h.right.clone()], footer: [f.left.clone(), f.center.clone(), f.right.clone()], title_page }
    }

    pub fn parts(&self) -> (HeaderFooter, HeaderFooter) {
        let hf = |a: &[String; 3]| HeaderFooter { left: a[0].clone(), center: a[1].clone(), right: a[2].clone() };
        (hf(&self.header), hf(&self.footer))
    }
}

/// A folha de cabeçalho e rodapé. Some(true) = OK; Some(false) = Cancelar.
pub fn hf_sheet(ui: &mut Ui, f: &mut HfSheet) -> Option<bool> {
    let sw = 520.0f32.min(ui.c.w - 20.0);
    let sh = 236.0;
    let sx = sheet_frame(ui, sw, sh, tr!("Cabeçalho e Rodapé"));
    let ink = rgba(20, 20, 24, 255);
    let fw = ((sw - 36.0 - 90.0 - 16.0) / 3.0).floor();
    let x0 = sx + 18.0 + 90.0;
    for (i, t) in [tr!("Esquerda"), tr!("Centro"), tr!("Direita")].iter().enumerate() {
        ui.fonts.draw(ui.c, t, x0 + i as f32 * (fw + 8.0), 52.0, 11.0, false, rgba(90, 90, 96, 255));
    }
    for (row, (label, vals)) in [(tr!("Cabeçalho:"), &mut f.header), (tr!("Rodapé:"), &mut f.footer)].into_iter().enumerate() {
        let y = 58.0 + row as f32 * 32.0;
        ui.fonts.draw(ui.c, label, sx + 18.0, y + 15.0, 12.0, false, ink);
        for (i, v) in vals.iter_mut().enumerate() {
            ui.text_field_in(Rect::new(x0 + i as f32 * (fw + 8.0), y, fw, 22.0), v, false);
        }
    }
    ui.fonts.draw(ui.c, tr!("Campos: {página}, {páginas}, {título} e {data}."), sx + 18.0, 140.0, 11.0, false, rgba(90, 90, 96, 255));
    let mut tp = f.title_page;
    ui.area(Rect::new(sx + 18.0, 152.0, sw - 36.0, 22.0), |ui| {
        ui.checkbox(&mut tp, tr!("Primeira página diferente (capa sem cabeçalho e rodapé)"));
    });
    f.title_page = tp;
    let (cancel, ok) = buttons(ui, sx, sw, sh - 38.0);
    ui.set_enabled(false);
    if cancel {
        return Some(false);
    }
    ok.then_some(true)
}

/// Inserir > Tabela: quantas linhas e colunas.
pub struct TableSheet {
    pub rows: String,
    pub cols: String,
}

/// A folha da tabela. Some(Some((linhas, colunas))) = OK; Some(None) = Cancelar.
pub fn table_sheet(ui: &mut Ui, f: &mut TableSheet) -> Option<Option<(usize, usize)>> {
    let sw = 300.0f32.min(ui.c.w - 20.0);
    let sh = 150.0;
    let sx = sheet_frame(ui, sw, sh, tr!("Inserir Tabela"));
    let ink = rgba(20, 20, 24, 255);
    let mut enter = false;
    for (i, (label, v)) in [(tr!("Linhas:"), &mut f.rows), (tr!("Colunas:"), &mut f.cols)].into_iter().enumerate() {
        let y = 44.0 + i as f32 * 30.0;
        let lw = ui.text_width(label, 12.0, false);
        ui.fonts.draw(ui.c, label, sx + 120.0 - lw, y + 15.0, 12.0, false, ink);
        let (_, e) = ui.text_field_in(Rect::new(sx + 130.0, y, 60.0, 22.0), v, i == 0 && v == "3" && !ui.has_focus());
        enter |= e;
    }
    let (cancel, ok) = buttons(ui, sx, sw, sh - 38.0);
    ui.set_enabled(false);
    if cancel {
        return Some(None);
    }
    if ok || enter {
        let r = f.rows.trim().parse::<usize>().unwrap_or(3);
        let c = f.cols.trim().parse::<usize>().unwrap_or(3);
        return Some(Some((r, c)));
    }
    None
}
