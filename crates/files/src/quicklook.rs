//! Olhada Rápida: Espaço abre uma janela com a prévia do item escolhido (imagem,
//! texto, primeira página do PDF ou o ícone grande com os dados). Ela segue a
//! escolha: as setas, nela ou no Arquivos, passam para o próximo item. Espaço ou
//! Esc fecham; o botão do canto abre o arquivo no app dele.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use glove_ui::paint::draw::*;
use glove_ui::{tr, ui::Key, Rect, Ui, WindowOptions};

use crate::{human_date, human_size, thumb, Files, Kind};

/// Número da janela da Olhada Rápida (a do Arquivos é a 0).
pub const QL_WIN: u32 = 70;
/// Texto mostrado, no máximo (o começo do arquivo).
const MAX_TEXT: usize = 48 * 1024;

#[derive(Default)]
pub struct QuickLook {
    pub open: bool,
    /// O item que a janela mostrou por último (para avisar a outra janela quando muda).
    pub seen: Option<PathBuf>,
    /// Texto já lido: (arquivo, linhas) e a rolagem.
    text: Option<(PathBuf, Vec<String>)>,
    scroll: f32,
    /// PDF: (arquivo, imagem da primeira página quando pronta, pedido em andamento).
    pdf: Option<(PathBuf, Option<PathBuf>, Option<Receiver<Option<PathBuf>>>)>,
    /// Imagens da Olhada (separadas das da coluna de prévia).
    thumbs: thumb::Thumbs,
}

fn read_text(p: &Path) -> Option<Vec<String>> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(p).ok()?.take(MAX_TEXT as u64).read_to_end(&mut buf).ok()?;
    // Binário (muitos zeros): não é texto.
    if buf.iter().take(4096).filter(|b| **b == 0).count() > 2 {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).replace('\t', "    ").lines().map(String::from).collect())
}

/// A primeira página do PDF numa imagem (pdftoppm, do poppler), numa thread.
fn render_pdf(p: &Path) -> Receiver<Option<PathBuf>> {
    let (tx, rx) = channel();
    let src = p.to_path_buf();
    std::thread::spawn(move || {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        src.hash(&mut h);
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let base = dir.join(format!("glove-olhada-{:x}", h.finish()));
        let out = base.with_extension("png");
        let ok = out.exists()
            || std::process::Command::new("pdftoppm")
                .args(["-png", "-r", "90", "-f", "1", "-l", "1", "-singlefile"])
                .arg(&src)
                .arg(&base)
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
        let _ = tx.send((ok && out.exists()).then_some(out));
    });
    rx
}

/// Abre a janela da Olhada.
pub fn window_options(name: &str) -> WindowOptions {
    WindowOptions { title: name.to_string(), app_id: "glove.Files".into(), size: (640, 520), fixed: false, min_size: Some((320, 260)) }
}

impl Files {
    /// Espaço: abre ou fecha a Olhada Rápida.
    pub(crate) fn toggle_quicklook(&mut self, ui: &mut Ui) {
        if self.ql.open {
            self.ql.open = false;
            // A janela da Olhada se fecha no próximo quadro dela.
            ui.redraw_all_windows();
            return;
        }
        let Some(e) = self.selected_entry() else { return };
        self.ql.open = true;
        self.ql.seen = None;
        ui.open_window(QL_WIN, window_options(&e.name));
    }

    /// No fim do quadro da janela principal: se a escolha mudou, a Olhada acompanha.
    pub(crate) fn quicklook_follow(&mut self, ui: &mut Ui) {
        if self.ql.open && self.ql.seen != self.selected {
            ui.redraw_all_windows();
        }
    }

    /// A janela da Olhada.
    pub(crate) fn quicklook_ui(&mut self, ui: &mut Ui) {
        if !self.ql.open {
            ui.close = true;
            return;
        }
        // Teclas: Espaço e Esc fecham; setas passam de item (na janela do Arquivos também).
        let mut moved = false;
        for k in ui.keys().to_vec() {
            match k.key {
                Key::Escape => ui.close = true,
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    let a = match k.key {
                        Key::Up => "sel-up",
                        Key::Down => "sel-down",
                        Key::Left => "sel-left",
                        _ => "sel-right",
                    };
                    self.action(a, ui);
                    moved = true;
                }
                _ if k.combo == "space" => ui.close = true,
                _ => {}
            }
        }
        if ui.close {
            self.ql.open = false;
            ui.redraw_all_windows();
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(38, 38, 40, 255)));
        let Some(e) = self.selected_entry() else {
            let t = tr!("Nenhum item escolhido");
            let tw = ui.text_width(t, 13.0, true);
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0, h / 2.0, 13.0, true, rgba(200, 200, 200, 255));
            self.ql.seen = None;
            return;
        };
        ui.set_document(Some(e.path.as_path()));
        if self.ql.seen.as_ref() != Some(&e.path) {
            ui.set_title(&e.name);
            self.ql.scroll = 0.0;
        }
        self.ql.seen = Some(e.path.clone());
        if moved {
            ui.redraw_all_windows();
        }
        // Faixa de baixo: nome, dados e o botão de abrir.
        const BAR: f32 = 44.0;
        let body = Rect::new(16.0, 16.0, w - 32.0, h - 32.0 - BAR);
        let mut dims = None;
        let drew = match e.kind {
            Kind::Picture if thumb::supported(&e.path) => {
                let path = e.path.clone();
                dims = self.draw_image(ui, &path, body);
                true
            }
            Kind::Pdf => self.draw_pdf(ui, &e.path, body),
            Kind::Text | Kind::Code | Kind::Other if e.size <= 64 * 1024 * 1024 => self.draw_text(ui, &e.path, body),
            _ => false,
        };
        if !drew {
            // Sem prévia: o ícone grande.
            let s = (body.w.min(body.h) * 0.6).clamp(64.0, 256.0);
            e.draw_icon(ui.c, ui.fonts, body.x + (body.w - s) / 2.0, body.y + (body.h - s) / 2.0, s);
        }
        let y = h - BAR;
        ui.c.hline(0.0, y, w, rgba(70, 70, 74, 255));
        let name = ui.fonts.ellipsize(&e.name, 13.0, true, w - 200.0);
        ui.fonts.draw(ui.c, &name, 16.0, y + 19.0, 13.0, true, rgba(240, 240, 240, 255));
        let mut info = format!("{} — {}", e.type_name(), if e.kind == Kind::Folder { String::new() } else { human_size(e.size) });
        if let Some((iw, ih)) = dims {
            info.push_str(&format!(" — {iw} × {ih}"));
        }
        info.push_str(&format!(" — {}", human_date(e.modified)));
        let info = ui.fonts.ellipsize(&info, 11.0, false, w - 200.0);
        ui.fonts.draw(ui.c, &info, 16.0, y + 35.0, 11.0, false, rgba(170, 170, 175, 255));
        let mut open = false;
        ui.area(Rect::new(w - 150.0, y + 11.0, 136.0, 24.0), |ui| {
            open = ui.button(tr!("Abrir"));
        });
        if open {
            self.open(&e);
            self.ql.open = false;
            ui.close = true;
            ui.redraw_all_windows();
        }
    }

    /// Imagem: cabe na área, sem distorcer. Devolve as dimensões originais.
    fn draw_image(&mut self, ui: &mut Ui, p: &Path, r: Rect) -> Option<(u32, u32)> {
        let s = ui.c.scale;
        let (img, busy) = self.ql.thumbs.get(p);
        let Some((src, orig)) = img else {
            if busy {
                ui.wake_after(std::time::Duration::from_millis(40));
                ui.spinner(24.0);
            }
            return None;
        };
        let orig = *orig;
        let (ow, oh) = (src.width() as f32 / s, src.height() as f32 / s);
        let fit = (r.w / ow).min(r.h / oh).min(2.0);
        let (dw, dh) = (ow * fit, oh * fit);
        let (dx, dy) = ((r.x + (r.w - dw) / 2.0).round(), (r.y + (r.h - dh) / 2.0).round());
        let k = dw * s / src.width() as f32;
        let paint = glove_ui::paint::tiny_skia::PixmapPaint { quality: glove_ui::paint::tiny_skia::FilterQuality::Bilinear, ..Default::default() };
        ui.c.fill_rect(dx + 2.0, dy + 3.0, dw, dh, &solid(rgba(0, 0, 0, 90)));
        ui.c.draw_pixmap_t(src.as_ref(), &paint, glove_ui::paint::tiny_skia::Transform::from_row(k, 0.0, 0.0, k, (dx * s).round(), (dy * s).round()));
        Some(orig)
    }

    fn draw_pdf(&mut self, ui: &mut Ui, p: &Path, r: Rect) -> bool {
        if self.ql.pdf.as_ref().is_none_or(|(src, _, _)| src != p) {
            self.ql.pdf = Some((p.to_path_buf(), None, Some(render_pdf(p))));
        }
        let Some((_, img, job)) = &mut self.ql.pdf else { return false };
        if let Some(rx) = job {
            match rx.try_recv() {
                Ok(out) => {
                    *img = out;
                    *job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.wake_after(std::time::Duration::from_millis(60));
                    return false;
                }
                Err(_) => *job = None,
            }
        }
        match img.clone() {
            Some(png) => {
                self.draw_image(ui, &png, r);
                true
            }
            None => false,
        }
    }

    /// Texto: o começo do arquivo numa folha branca, com rolagem.
    fn draw_text(&mut self, ui: &mut Ui, p: &Path, r: Rect) -> bool {
        if self.ql.text.as_ref().is_none_or(|(src, _)| src != p) {
            match read_text(p) {
                Some(lines) => self.ql.text = Some((p.to_path_buf(), lines)),
                None => {
                    self.ql.text = None;
                    return false;
                }
            }
        }
        let Some((_, lines)) = &self.ql.text else { return false };
        ui.c.fill_rect(r.x + 2.0, r.y + 3.0, r.w, r.h, &solid(rgba(0, 0, 0, 90)));
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(255, 255, 255, 255)));
        let px = 12.0;
        let lh = 16.0;
        let shown = ((r.h - 20.0) / lh).floor().max(1.0) as usize;
        let max_scroll = lines.len().saturating_sub(shown) as f32;
        let wheel = ui.scroll_in(r);
        if wheel != 0.0 {
            self.ql.scroll = (self.ql.scroll + wheel.signum() * 3.0).clamp(0.0, max_scroll);
        }
        let first = self.ql.scroll as usize;
        let old = ui.c.clip();
        let s = ui.c.scale;
        ui.c.set_clip(Some([(r.x * s) as i32, (r.y * s) as i32, ((r.x + r.w) * s) as i32, ((r.y + r.h) * s) as i32]));
        for (k, l) in lines.iter().skip(first).take(shown).enumerate() {
            let t = ui.fonts.ellipsize(l, px, false, r.w - 24.0);
            ui.doc_fonts.draw(ui.c, &t, r.x + 12.0, r.y + 22.0 + k as f32 * lh, px, false, rgba(30, 30, 30, 255));
        }
        ui.c.set_clip(old);
        true
    }
}
