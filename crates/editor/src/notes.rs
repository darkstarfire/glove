//! Folhas da nota de rodapé, do cabeçalho e do rodapé, do número de página, do
//! sumário e da tabela.

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

/// O que a folha decidiu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    Ok,
    Cancel,
    /// Tirar o elemento do documento.
    Remove,
}

/// Cancelar e OK à direita e, se o elemento já existe, "Remover…" à esquerda.
fn buttons_remove(ui: &mut Ui, sx: f32, sw: f32, by: f32, remove: Option<&str>) -> Option<Done> {
    let (cancel, ok) = buttons(ui, sx, sw, by);
    let mut rm = false;
    if let Some(t) = remove {
        let w = ui.text_width(t, 13.0, false) + 36.0;
        ui.area(Rect::new(sx + 18.0, by, w, 24.0), |ui| rm = ui.button(t));
    }
    if cancel {
        Some(Done::Cancel)
    } else if rm {
        Some(Done::Remove)
    } else if ok {
        Some(Done::Ok)
    } else {
        None
    }
}

/// Um rótulo alinhado à direita em `x` (os formulários da era).
fn rlabel(ui: &mut Ui, t: &str, x: f32, y: f32) {
    let w = ui.text_width(t, 12.0, false);
    ui.fonts.draw(ui.c, t, x - w, y + 15.0, 12.0, false, rgba(20, 20, 24, 255));
}

const SIZES: [f32; 8] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0];
const COLORS: [[u8; 3]; 5] = [[80, 80, 86], [0, 0, 0], [30, 60, 140], [140, 30, 30], [40, 110, 60]];

fn cm(pt: f32) -> String {
    format!("{:.1}", pt / 28.3465).replace('.', glove_config::virgula())
}

fn pt(cm: &str) -> Option<f32> {
    cm.trim().replace(',', ".").parse::<f32>().ok().map(|v| (v * 28.3465).max(0.0))
}

/// Cabeçalho ou rodapé: os textos e a aparência (abre com duplo clique na área dele ou
/// pelo menu Inserir).
pub struct HfSheet {
    pub footer: bool,
    pub hf: HeaderFooter,
    pub title_page: bool,
    /// O elemento já existia (aparece "Remover").
    pub existed: bool,
    families: Vec<String>,
    family: usize,
    size: usize,
    color: usize,
    offset: String,
    indent: String,
}

impl HfSheet {
    /// `families`: as do documento (a lista ganha as comuns e "a do texto").
    pub fn new(footer: bool, hf: &HeaderFooter, title_page: bool, existed: bool, doc_families: &[String]) -> Self {
        let mut families = vec![String::new()];
        for f in doc_families.iter().map(String::as_str).chain(["Helvetica", "Times New Roman", "Arial", "Liberation Sans", "Liberation Serif", "DejaVu Sans", "DejaVu Serif", "Courier New"]) {
            if !f.is_empty() && !families.iter().any(|x| x == f) {
                families.push(f.to_string());
            }
        }
        if !hf.family.is_empty() && !families.contains(&hf.family) {
            families.push(hf.family.clone());
        }
        let family = families.iter().position(|f| *f == hf.family).unwrap_or(0);
        let size = SIZES.iter().position(|s| (*s - hf.size).abs() < 0.1).unwrap_or(2);
        let color = COLORS.iter().position(|c| *c == hf.color).unwrap_or(0);
        Self { footer, hf: hf.clone(), title_page, existed, families, family, size, color, offset: if hf.offset > 0.0 { cm(hf.offset) } else { String::new() }, indent: cm(hf.indent) }
    }
}

/// A folha do cabeçalho ou do rodapé.
pub fn hf_sheet(ui: &mut Ui, f: &mut HfSheet) -> Option<Done> {
    let sw = 580.0f32.min(ui.c.w - 20.0);
    let sh = 316.0;
    let title = if f.footer { tr!("Rodapé") } else { tr!("Cabeçalho") };
    let sx = sheet_frame(ui, sw, sh, title);
    let grey = rgba(90, 90, 96, 255);
    // Os textos: à esquerda, no centro e à direita.
    let fw = ((sw - 36.0 - 16.0) / 3.0).floor();
    let x0 = sx + 18.0;
    for (i, t) in [tr!("Esquerda"), tr!("Centro"), tr!("Direita")].iter().enumerate() {
        ui.fonts.draw(ui.c, t, x0 + i as f32 * (fw + 8.0), 50.0, 11.0, false, grey);
    }
    for (i, v) in [&mut f.hf.left, &mut f.hf.center, &mut f.hf.right].into_iter().enumerate() {
        ui.text_field_in(Rect::new(x0 + i as f32 * (fw + 8.0), 56.0, fw, 22.0), v, false);
    }
    ui.fonts.draw(ui.c, tr!("Campos: {página}, {páginas}, {título} e {data}."), x0, 96.0, 11.0, false, grey);
    // A aparência.
    let lx = sx + 18.0 + ui.text_width(tr!("Distância da borda (cm):"), 12.0, false).max(ui.text_width(tr!("Recuo nas laterais (cm):"), 12.0, false)) + 10.0;
    rlabel(ui, tr!("Letra:"), lx - 8.0, 110.0);
    let names: Vec<String> = f.families.iter().map(|n| if n.is_empty() { tr!("A mesma do texto").to_string() } else { n.clone() }).collect();
    let sizes: Vec<String> = SIZES.iter().map(|s| format!("{} pt", *s as u32)).collect();
    let colors: Vec<String> = [tr!("Cinza"), tr!("Preto"), tr!("Azul-escuro"), tr!("Vinho"), tr!("Verde-escuro")].iter().map(|s| s.to_string()).collect();
    let (mut fam, mut size, mut color) = (f.family, f.size, f.color);
    let (mut bold, mut italic, mut line, mut tp) = (f.hf.bold, f.hf.italic, f.hf.line, f.title_page);
    let rw = sx + sw - 18.0 - lx;
    ui.area(Rect::new(lx, 108.0, rw, 24.0), |ui| {
        ui.horizontal(|ui| {
            ui.popup(&mut fam, &names, 200.0);
            ui.spacing(8.0);
            ui.popup(&mut size, &sizes, 74.0);
        });
    });
    rlabel(ui, tr!("Cor:"), lx - 8.0, 142.0);
    ui.area(Rect::new(lx, 140.0, rw, 24.0), |ui| {
        ui.horizontal(|ui| {
            ui.popup(&mut color, &colors, 130.0);
            ui.spacing(14.0);
            ui.checkbox(&mut bold, tr!("Negrito"));
            ui.spacing(6.0);
            ui.checkbox(&mut italic, tr!("Itálico"));
        });
    });
    rlabel(ui, tr!("Distância da borda (cm):"), lx - 8.0, 174.0);
    ui.text_field_in(Rect::new(lx, 172.0, 60.0, 22.0), &mut f.offset, false);
    ui.fonts.draw(ui.c, tr!("vazio: no meio da margem"), lx + 68.0, 188.0, 11.0, false, grey);
    rlabel(ui, tr!("Recuo nas laterais (cm):"), lx - 8.0, 204.0);
    ui.text_field_in(Rect::new(lx, 202.0, 60.0, 22.0), &mut f.indent, false);
    ui.area(Rect::new(lx, 232.0, rw, 44.0), |ui| {
        ui.checkbox(&mut line, tr!("Fio separando do texto"));
        ui.checkbox(&mut tp, tr!("Primeira página diferente (capa sem cabeçalho e rodapé)"));
    });
    f.family = fam;
    f.size = size;
    f.color = color;
    f.hf.family = f.families.get(fam).cloned().unwrap_or_default();
    f.hf.size = SIZES[size.min(SIZES.len() - 1)];
    f.hf.color = COLORS[color.min(COLORS.len() - 1)];
    f.hf.bold = bold;
    f.hf.italic = italic;
    f.hf.line = line;
    f.title_page = tp;
    f.hf.offset = if f.offset.trim().is_empty() { 0.0 } else { pt(&f.offset).unwrap_or(f.hf.offset) };
    f.hf.indent = pt(&f.indent).unwrap_or(f.hf.indent);
    let rm = if f.footer { tr!("Remover Rodapé") } else { tr!("Remover Cabeçalho") };
    let done = buttons_remove(ui, sx, sw, sh - 38.0, f.existed.then_some(rm));
    ui.set_enabled(false);
    done
}

/// O número de página: onde fica, o formato e de onde começa.
pub struct PageNumSheet {
    pub pn: crate::rich::PageNumber,
    pub existed: bool,
    start: String,
}

impl PageNumSheet {
    pub fn new(pn: &crate::rich::PageNumber) -> Self {
        let mut p = pn.clone();
        let existed = p.on;
        p.on = true;
        Self { start: p.start.to_string(), pn: p, existed }
    }
}

pub fn page_num_sheet(ui: &mut Ui, f: &mut PageNumSheet) -> Option<Done> {
    let sw = 460.0f32.min(ui.c.w - 20.0);
    let sh = 228.0;
    let sx = sheet_frame(ui, sw, sh, tr!("Número de Página"));
    let lx = sx + 120.0;
    let place: Vec<String> = [tr!("No alto da folha"), tr!("Embaixo da folha")].iter().map(|s| s.to_string()).collect();
    let align: Vec<String> = [tr!("À esquerda"), tr!("No centro"), tr!("À direita")].iter().map(|s| s.to_string()).collect();
    let formats: Vec<String> = (0..5u8).map(|k| crate::rich::PageNumber { format: k, ..crate::rich::PageNumber::default() }.text(0, 3)).collect();
    let (mut p, mut a, mut fm, mut first) = (usize::from(!f.pn.top), f.pn.align as usize, f.pn.format as usize, f.pn.first);
    rlabel(ui, tr!("Posição:"), lx - 8.0, 44.0);
    ui.area(Rect::new(lx, 42.0, 300.0, 24.0), |ui| {
        ui.horizontal(|ui| {
            ui.popup(&mut p, &place, 164.0);
            ui.spacing(6.0);
            ui.popup(&mut a, &align, 110.0);
        });
    });
    rlabel(ui, tr!("Formato:"), lx - 8.0, 76.0);
    ui.area(Rect::new(lx, 74.0, 200.0, 24.0), |ui| {
        ui.popup(&mut fm, &formats, 170.0);
    });
    rlabel(ui, tr!("Começar em:"), lx - 8.0, 108.0);
    ui.text_field_in(Rect::new(lx, 106.0, 60.0, 22.0), &mut f.start, false);
    ui.area(Rect::new(lx, 138.0, sw - 168.0, 22.0), |ui| {
        ui.checkbox(&mut first, tr!("Mostrar na primeira folha"));
    });
    ui.fonts.draw(ui.c, tr!("Usa a letra do cabeçalho ou do rodapé. Duplo clique no número abre esta janela."), sx + 18.0, 180.0, 11.0, false, rgba(90, 90, 96, 255));
    f.pn.top = p == 0;
    f.pn.align = a.min(2) as u8;
    f.pn.format = fm.min(4) as u8;
    f.pn.first = first;
    f.pn.start = f.start.trim().parse::<u32>().ok().filter(|n| *n >= 1).unwrap_or(f.pn.start);
    let done = buttons_remove(ui, sx, sw, sh - 38.0, f.existed.then_some(tr!("Remover Número")));
    ui.set_enabled(false);
    done
}

/// O sumário: o título, até que nível entra e se mostra as páginas.
pub struct TocSheet {
    pub toc: crate::rich::TocOptions,
    pub existed: bool,
}

pub fn toc_sheet(ui: &mut Ui, f: &mut TocSheet) -> Option<Done> {
    let sw = 440.0f32.min(ui.c.w - 20.0);
    let sh = 196.0;
    let sx = sheet_frame(ui, sw, sh, tr!("Sumário"));
    let lx = sx + 110.0;
    rlabel(ui, tr!("Título:"), lx - 8.0, 44.0);
    ui.text_field_in(Rect::new(lx, 42.0, sw - 128.0, 22.0), &mut f.toc.title, false);
    let levels: Vec<String> = [tr!("Capítulos"), tr!("Capítulos e seções"), tr!("Capítulos, seções e subseções")].iter().map(|s| s.to_string()).collect();
    let mut d = (f.toc.depth.clamp(1, 3) - 1) as usize;
    let mut pages = f.toc.pages;
    rlabel(ui, tr!("Incluir:"), lx - 8.0, 76.0);
    ui.area(Rect::new(lx, 74.0, sw - 128.0, 24.0), |ui| {
        ui.popup(&mut d, &levels, 240.0);
    });
    ui.area(Rect::new(lx, 108.0, sw - 128.0, 22.0), |ui| {
        ui.checkbox(&mut pages, tr!("Mostrar o número das páginas"));
    });
    f.toc.depth = d as u8 + 1;
    f.toc.pages = pages;
    let done = buttons_remove(ui, sx, sw, sh - 38.0, f.existed.then_some(tr!("Remover Sumário")));
    ui.set_enabled(false);
    done
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
