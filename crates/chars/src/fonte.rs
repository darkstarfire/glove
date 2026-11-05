//! Visão "Fonte" do Caracteres: escolhe uma fonte instalada e mostra os caracteres
//! que ela desenha, com uma prévia grande e as linhas da tipografia (base,
//! altura do x, maiúsculas, subida e descida). "Instalar Fonte…" copia um
//! .ttf/.otf para a pasta pessoal e já mostra a fonte nova.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

use glove_kit::{
    paint::{draw::*, font::Fonts, Color},
    Choice, FileBrowser, Rect, Ui, SCROLLBAR_W,
};

const CELL: f32 = 40.0;
const LIST_W: f32 = 170.0;

/// Famílias instaladas (nome principal de cada uma), em ordem.
fn families() -> Vec<String> {
    let out = Command::new("fc-list").args([":", "family"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut v: Vec<String> = out.lines().filter_map(|l| l.split(',').next()).map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
    v.sort_by_key(|f| f.to_lowercase());
    v.dedup();
    v
}

fn user_fonts() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("fonts")
}

/// Copia a fonte para ~/.local/share/fonts, refaz o cache e devolve a família dela.
fn install(file: &Path) -> Result<String, String> {
    let family = Command::new("fc-scan").args(["--format", "%{family[0]}"]).arg(file).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if family.is_empty() {
        return Err("Esse arquivo não parece ser uma fonte.".into());
    }
    let dest = user_fonts();
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let name = file.file_name().ok_or("arquivo sem nome")?;
    std::fs::copy(file, dest.join(name)).map_err(|e| format!("Não consegui copiar: {e}"))?;
    let _ = Command::new("fc-cache").arg("-f").arg(&dest).status();
    Ok(family)
}

#[derive(Default)]
pub struct FontView {
    list: Option<Vec<String>>,
    list_rx: Option<Receiver<Vec<String>>>,
    family: String,
    fonts: Option<Fonts>,
    /// Caracteres que a fonte tem.
    chars: Vec<char>,
    scroll: f32,
    pub selected: Option<char>,
    pub copied: Option<char>,
    /// Folha "Instalar Fonte…" aberta.
    sheet: Option<FileBrowser>,
    installing: Option<Receiver<Result<String, String>>>,
    message: Option<String>,
}

impl FontView {
    /// Abre a folha para escolher um arquivo de fonte.
    pub fn ask_install(&mut self) {
        let start = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")).filter(|p| p.is_dir());
        self.sheet = Some(FileBrowser::open(start, &["ttf", "otf", "ttc"]));
    }

    fn pick(&mut self, family: &str) {
        self.family = family.to_string();
        self.fonts = Fonts::load_family(family);
        self.chars = self.fonts.as_ref().map(|f| f.chars()).unwrap_or_default();
        self.scroll = 0.0;
        self.selected = self.chars.iter().copied().find(|c| *c == 'A').or(self.chars.first().copied());
        self.copied = None;
    }

    /// Fonte para ver pela primeira vez: a do sistema (a mesma da interface).
    fn ensure_list(&mut self, ui: &mut Ui) {
        if self.list.is_none() && self.list_rx.is_none() {
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let _ = tx.send(families());
            });
            self.list_rx = Some(rx);
        }
        if let Some(rx) = &self.list_rx {
            match rx.try_recv() {
                Ok(l) => {
                    self.list = Some(l);
                    self.list_rx = None;
                    if self.family.is_empty() {
                        let list = self.list.as_ref().unwrap();
                        let first = ["DejaVu Sans", "Noto Sans", "Liberation Sans"].iter().find(|f| list.iter().any(|l| l == *f)).map(|f| f.to_string()).or_else(|| list.first().cloned());
                        if let Some(f) = first {
                            self.pick(&f);
                        }
                    }
                }
                Err(_) => ui.wake_after(std::time::Duration::from_millis(60)),
            }
        }
        if let Some(rx) = &self.installing {
            match rx.try_recv() {
                Ok(Ok(family)) => {
                    self.installing = None;
                    self.message = Some(format!("“{family}” instalada."));
                    self.list = Some(families());
                    self.pick(&family);
                }
                Ok(Err(e)) => {
                    self.installing = None;
                    self.message = Some(e);
                }
                Err(_) => ui.wake_after(std::time::Duration::from_millis(80)),
            }
        }
    }

    /// Desenha a visão em `r` (abaixo do seletor de modo).
    pub fn ui(&mut self, ui: &mut Ui, r: Rect) {
        self.ensure_list(ui);
        if self.sheet.is_some() {
            ui.set_enabled(false);
        }
        // Linha de cima: Instalar e quantos caracteres a fonte tem.
        let mut install = false;
        ui.area(Rect::new(r.x, r.y, 160.0, 24.0), |ui| install = ui.button("Instalar Fonte…"));
        if install {
            self.ask_install();
        }
        let info = match (&self.message, self.installing.is_some()) {
            (_, true) => "Instalando…".to_string(),
            (Some(m), _) => m.clone(),
            _ => format!("{}: {} caracteres", self.family, self.chars.len()),
        };
        let info = ui.fonts.ellipsize(&info, 11.0, false, r.w - 180.0);
        let iw = ui.text_width(&info, 11.0, false);
        ui.fonts.draw(ui.c, &info, r.x + r.w - iw, r.y + 16.0, 11.0, false, rgba(90, 90, 96, 255));

        // Fontes instaladas, à esquerda (lista rolável: costumam ser centenas).
        let names = self.list.clone().unwrap_or_default();
        let lr = Rect::new(r.x, r.y + 34.0, LIST_W, r.h - 34.0);
        if names.is_empty() {
            ui.fonts.draw(ui.c, "Lendo as fontes…", lr.x + 8.0, lr.y + 18.0, 12.0, false, rgba(110, 110, 110, 255));
        } else {
            let mut sel = names.iter().position(|n| *n == self.family);
            let before = sel;
            ui.area(lr, |ui| {
                ui.list(&mut sel, &names, lr.w, lr.h);
            });
            if sel != before {
                if let Some(i) = sel {
                    let f = names[i].clone();
                    self.pick(&f);
                }
            }
        }
        let r = Rect::new(r.x + LIST_W + 10.0, r.y, r.w - LIST_W - 10.0, r.h);

        // Grade com os caracteres, desenhados com a própria fonte.
        let pv_w = 170.0;
        let full = Rect::new(r.x, r.y + 34.0, r.w - pv_w - 12.0, r.h - 34.0);
        let per_row = ((full.w - SCROLLBAR_W) / CELL).floor().max(1.0) as usize;
        let rows = self.chars.len().div_ceil(per_row);
        let grid = ui.scroll_area(full, &mut self.scroll, rows as f32 * CELL, CELL);
        let first = (self.scroll / CELL).floor() as usize;
        let visible = (grid.h / CELL).ceil() as usize + 1;
        for (i, ch) in self.chars.iter().enumerate().skip(first * per_row).take(visible * per_row) {
            let (col, row) = (i % per_row, i / per_row);
            let y = grid.y + row as f32 * CELL - self.scroll;
            let (top, bot) = (y.max(grid.y), (y + CELL).min(grid.y + grid.h));
            if bot > top {
                let h = ui.hit(Rect::new(grid.x + col as f32 * CELL, top, CELL, bot - top));
                if h.pressed {
                    self.selected = Some(*ch);
                    self.copied = None;
                    if h.double {
                        ui.copy(&ch.to_string());
                        self.copied = Some(*ch);
                    }
                }
            }
        }
        let (sel, off, accent) = (self.selected, self.scroll, ui.theme.accent);
        let chars = &self.chars;
        let fonts = self.fonts.as_ref();
        ui.clipped(7, grid, |c, ui_fonts| {
            c.clear(Color::WHITE);
            let f = fonts.unwrap_or(ui_fonts);
            for (i, ch) in chars.iter().enumerate().skip(first * per_row).take(visible * per_row) {
                let (col, row) = (i % per_row, i / per_row);
                let (x, y) = (col as f32 * CELL, row as f32 * CELL - off);
                let on = sel == Some(*ch);
                if on {
                    c.fill_rect(x, y, CELL, CELL, &vgrad(y, y + CELL, &[(0.0, lighten(accent, 0.2)), (1.0, accent)]));
                }
                let s = ch.to_string();
                let tw = f.measure(&s, 22.0, false);
                f.draw(c, &s, x + (CELL - tw) / 2.0, y + 28.0, 22.0, false, if on { Color::WHITE } else { rgba(20, 20, 20, 255) });
                c.hline(x, y + CELL - 1.0, CELL, rgba(225, 225, 230, 255));
                c.vline(x + CELL - 1.0, y, CELL, rgba(225, 225, 230, 255));
            }
        });
        ui.c.stroke(rrect(full.x - 0.5, full.y - 0.5, full.w + 1.0, full.h + 1.0, 0.0), &solid(rgba(130, 130, 136, 255)), 1.0);

        // Prévia: o caractere grande com as linhas da fonte.
        let pv = Rect::new(r.x + r.w - pv_w, full.y, pv_w, (full.h - 70.0).max(120.0));
        ui.c.fill(rrect(pv.x, pv.y, pv.w, pv.h, 6.0), &solid(Color::WHITE));
        ui.c.stroke(rrect(pv.x, pv.y, pv.w, pv.h, 6.0), &solid(rgba(130, 130, 136, 255)), 1.0);
        let Some(fonts) = self.fonts.as_ref() else { return };
        if let Some(ch) = self.selected {
            let px = (pv.h * 0.5).min(pv.w * 0.6);
            let (asc, desc, xh, caps) = fonts.v_metrics(px);
            // A linha de base fica de modo que subida + descida caibam na caixa.
            let base = (pv.y + (pv.h - (asc + desc)) / 2.0 + asc).round();
            let line = |ui: &mut Ui, y: f32, label: &str, color: Color| {
                ui.c.hline(pv.x + 6.0, y.round(), pv.w - 12.0, color);
                ui.fonts.draw(ui.c, label, pv.x + 8.0, y.round() - 2.0, 9.0, false, color);
            };
            line(ui, base - asc, "subida", rgba(170, 170, 200, 255));
            if caps > 0.0 {
                line(ui, base - caps, "maiúsculas", rgba(120, 160, 220, 255));
            }
            if xh > 0.0 {
                line(ui, base - xh, "altura do x", rgba(90, 170, 120, 255));
            }
            line(ui, base, "linha de base", rgba(210, 90, 80, 255));
            line(ui, base + desc, "descida", rgba(170, 170, 200, 255));
            let s = ch.to_string();
            let tw = fonts.measure(&s, px, false);
            // Um pouco à direita do centro: os nomes das linhas ficam à esquerda.
            fonts.draw(ui.c, &s, (pv.x + (pv.w - tw) / 2.0 + 14.0).min(pv.x + pv.w - tw - 6.0), base, px, false, rgba(10, 10, 10, 235));
            // Código, nome da fonte e copiar.
            let ty = pv.y + pv.h + 18.0;
            let code = format!("U+{:04X}", ch as u32);
            ui.fonts.draw(ui.c, &code, pv.x, ty, 14.0, true, rgba(20, 20, 20, 255));
            if self.copied == Some(ch) {
                ui.fonts.draw(ui.c, "Copiado.", pv.x + 80.0, ty, 11.0, false, rgba(90, 90, 90, 255));
            }
            let mut go = false;
            ui.area(Rect::new(pv.x, ty + 10.0, 120.0, 24.0), |ui| go = ui.default_button("Copiar"));
            if go {
                ui.copy(&s);
                self.copied = Some(ch);
            }
        }
    }

    /// A folha "Instalar Fonte…", por cima de tudo.
    pub fn sheet(&mut self, ui: &mut Ui) {
        let Some(fb) = self.sheet.as_mut() else { return };
        if let Some(choice) = ui.file_sheet(fb) {
            self.sheet = None;
            if let Choice::Picked(p) = choice {
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let _ = tx.send(install(&p));
                });
                self.installing = Some(rx);
                self.message = None;
            }
        }
    }
}
