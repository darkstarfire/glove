//! Dicionário (como o do Tiger): digita-se a palavra na busca, as entradas
//! parecidas aparecem à esquerda e a definição à direita. Lê dicionários
//! livres StarDict e DICT (dictd) instalados no sistema ou na pasta do Glove.

mod dict;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dict::Dict;
use glove_kit::{
    paint::{draw::*, Color},
    run, App, Choice, FileBrowser, Glyph, Menu, MenuItem, Rect, Ui, WindowOptions, SEARCH_H, SEGMENT_H, SEGMENT_W,
};

const BAR: f32 = 44.0;
const MAX_HITS: usize = 300;

/// Quebra o texto em linhas que caibam em `max` (palavra por palavra).
fn wrap(ui: &Ui, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let try_line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if ui.text_width(&try_line, px, bold) > max && !line.is_empty() {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = try_line;
        }
    }
    out.push(line);
    out
}

fn user_dir() -> PathBuf {
    dict::dirs().remove(0)
}

/// Copia um dicionário (o .ifo/.index e os arquivos irmãos) para a pasta do Glove.
fn install(p: &std::path::Path) -> Result<(), String> {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).ok_or("arquivo sem nome")?;
    let src = p.parent().ok_or("sem pasta")?;
    let dest = user_dir().join(&stem);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == stem || name.starts_with(&format!("{stem}.")) {
            std::fs::copy(e.path(), dest.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

struct Dicionario {
    dicts: Arc<Mutex<Option<Vec<Dict>>>>,
    /// Dicionário escolhido (None = todos).
    only: Option<usize>,
    query: String,
    /// Entrada mostrada: (dicionário, índice).
    shown: Option<(usize, usize)>,
    hits: Vec<(usize, usize)>,
    hits_for: Option<(String, Option<usize>)>,
    history: Vec<(usize, usize)>,
    hist_pos: usize,
    px: f32,
    scroll: f32,
    first: bool,
    open_fb: Option<FileBrowser>,
    error: Option<String>,
}

impl Dicionario {
    fn reload(&mut self) {
        let slot = self.dicts.clone();
        *slot.lock().unwrap() = None;
        self.hits_for = None;
        self.shown = None;
        self.history.clear();
        self.hist_pos = 0;
        std::thread::spawn(move || {
            let all = dict::load_all(&dict::dirs());
            *slot.lock().unwrap() = Some(all);
        });
    }

    fn show(&mut self, e: (usize, usize)) {
        if self.shown == Some(e) {
            return;
        }
        self.shown = Some(e);
        self.scroll = 0.0;
        self.history.truncate(self.hist_pos);
        self.history.push(e);
        self.hist_pos = self.history.len();
    }

    fn toolbar(&mut self, ui: &mut Ui, names: &[String]) -> Vec<&'static str> {
        let w = ui.c.w;
        let mut acts = Vec::new();
        ui.c.fill_rect(0.0, 0.0, w, BAR, &vgrad(0.0, BAR, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(204, 204, 208, 255))]));
        ui.c.hline(0.0, BAR - 1.0, w, rgba(140, 140, 146, 255));
        let y = (BAR - SEGMENT_H) / 2.0;
        let nav = [self.hist_pos > 1, self.hist_pos < self.history.len()];
        match ui.segmented_in(Rect::new(12.0, y, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &nav) {
            Some(0) => acts.push("back"),
            Some(_) => acts.push("fwd"),
            None => {}
        }
        match ui.segmented_in(Rect::new(20.0 + 2.0 * SEGMENT_W, y, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Minus, Glyph::Plus], None, &[self.px > 10.0, self.px < 22.0]) {
            Some(0) => acts.push("smaller"),
            Some(_) => acts.push("bigger"),
            None => {}
        }
        // Escolha do dicionário (quando há mais de um).
        let sx = 32.0 + 4.0 * SEGMENT_W;
        let sw = w - sx - 240.0;
        if names.len() > 1 && sw > 120.0 {
            let mut labels: Vec<String> = vec!["Todos".into()];
            labels.extend(names.iter().map(|n| ui.fonts.ellipsize(n, 12.0, false, 130.0)));
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let sel = Some(self.only.map(|i| i + 1).unwrap_or(0));
            let mut pick = None;
            ui.area(Rect::new(sx, y, sw, SEGMENT_H), |ui| {
                pick = ui.segmented_text(&refs, sel);
            });
            if let Some(p) = pick {
                self.only = p.checked_sub(1);
            }
        }
        let first = std::mem::take(&mut self.first);
        ui.search_field_focus(Rect::new(w - 226.0, (BAR - SEARCH_H) / 2.0, 214.0, SEARCH_H), &mut self.query, "Procurar palavra", first);
        acts
    }

    fn page(&mut self, ui: &mut Ui, dicts: &[Dict], r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        let grey = rgba(130, 130, 136, 255);
        let Some((d, i)) = self.shown else {
            let msg: &[&str] = if dicts.is_empty() {
                &[
                    "Nenhum dicionário instalado.",
                    "",
                    "O Dicionário lê dicionários livres nos formatos StarDict e DICT.",
                    "Instale um pelo sistema (por exemplo: sudo apt install dict-freedict-por-eng)",
                    "ou use Arquivo ▸ Adicionar Dicionário… e escolha o arquivo .ifo ou .index.",
                ]
            } else if self.query.trim().is_empty() {
                &["Digite uma palavra no campo de busca."]
            } else {
                &["Nenhuma entrada encontrada."]
            };
            for (k, l) in msg.iter().enumerate() {
                let (px, bold) = if k == 0 { (14.0, true) } else { (12.0, false) };
                let l = ui.fonts.ellipsize(l, px, bold, r.w - 40.0);
                ui.fonts.draw(ui.c, &l, r.x + 24.0, r.y + 36.0 + k as f32 * 20.0, px, bold, if k == 0 { rgba(60, 60, 66, 255) } else { grey });
            }
            return;
        };
        let e = &dicts[d].entries[i];
        let px = self.px;
        let max = r.w - 48.0;
        let mut lines: Vec<(String, f32, bool, Color)> = vec![(e.word.clone(), px * 1.8, true, rgba(20, 20, 24, 255)), (dicts[d].name.clone(), px * 0.85, false, grey), (String::new(), px * 0.5, false, grey)];
        for para in dicts[d].definition(i).split('\n') {
            let para = para.trim_end();
            if para.trim().is_empty() {
                lines.push((String::new(), px * 0.6, false, grey));
                continue;
            }
            let indent = para.len() - para.trim_start().len();
            let pre = " ".repeat(indent.min(8));
            for l in wrap(ui, para.trim_start(), px, false, max - indent.min(8) as f32 * px * 0.3) {
                lines.push((format!("{pre}{l}"), px, false, rgba(30, 30, 34, 255)));
            }
        }
        let total: f32 = lines.iter().map(|l| l.1 * 1.5).sum::<f32>() + 40.0;
        self.scroll = (self.scroll - ui.scroll_in(r)).clamp(0.0, (total - r.h).max(0.0));
        let mut y = r.y + 20.0 - self.scroll;
        for (t, px, bold, ink) in lines {
            y += px * 1.5;
            if y > r.y + 4.0 && y < r.y + r.h + px {
                ui.fonts.draw(ui.c, &t, r.x + 24.0, y - px * 0.4, px, bold, ink);
            }
        }
    }
}

impl App for Dicionario {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.open_fb.is_none() {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+W" => acts.push("close".into()),
                    "Ctrl+[" | "Alt+Left" => acts.push("back".into()),
                    "Ctrl+]" | "Alt+Right" => acts.push("fwd".into()),
                    "Ctrl+plus" | "Ctrl+equal" | "Ctrl+=" | "Ctrl++" => acts.push("bigger".into()),
                    "Ctrl+minus" | "Ctrl+-" => acts.push("smaller".into()),
                    "Ctrl+F" => self.first = true,
                    _ => {}
                }
            }
        }
        let slot = self.dicts.clone();
        let guard = slot.lock().unwrap();
        let Some(dicts) = guard.as_ref() else {
            drop(guard);
            self.toolbar(ui, &[]);
            ui.fonts.draw(ui.c, "Carregando os dicionários…", 24.0, BAR + 40.0, 12.0, false, rgba(110, 110, 116, 255));
            ui.wake_after(Duration::from_millis(200));
            return;
        };
        let names: Vec<String> = dicts.iter().map(|d| d.name.clone()).collect();
        acts.extend(self.toolbar(ui, &names).into_iter().map(String::from));
        if self.only.is_some_and(|o| o >= dicts.len()) {
            self.only = None;
        }
        // Entradas que começam com o texto digitado.
        let q = dict::normalize(self.query.trim());
        let key = (q.clone(), self.only);
        if self.hits_for.as_ref() != Some(&key) {
            self.hits.clear();
            if !q.is_empty() {
                for (di, d) in dicts.iter().enumerate() {
                    if self.only.is_some_and(|o| o != di) {
                        continue;
                    }
                    self.hits.extend(d.prefix(&q, MAX_HITS).into_iter().map(|i| (di, i)));
                }
                // Exatas primeiro, depois em ordem alfabética.
                self.hits.sort_by(|a, b| {
                    let (ea, eb) = (&dicts[a.0].entries[a.1], &dicts[b.0].entries[b.1]);
                    (ea.norm != q).cmp(&(eb.norm != q)).then(ea.norm.cmp(&eb.norm)).then(a.0.cmp(&b.0))
                });
                self.hits.truncate(MAX_HITS);
            }
            self.hits_for = Some(key);
            match self.hits.first().copied() {
                Some(e) => self.show(e),
                None => self.shown = None,
            }
        }
        // Lista das entradas.
        let lw = 210.0;
        let rows: Vec<String> = self.hits.iter().map(|(d, i)| if dicts.len() > 1 && self.only.is_none() { format!("{}  ·  {}", dicts[*d].entries[*i].word, dicts[*d].name) } else { dicts[*d].entries[*i].word.clone() }).collect();
        let mut sel = self.shown.and_then(|s| self.hits.iter().position(|h| *h == s));
        let before = sel;
        ui.area(Rect::new(8.0, BAR + 8.0, lw, h - BAR - 16.0), |ui| {
            ui.list(&mut sel, &rows, lw, h - BAR - 16.0);
        });
        if sel != before {
            if let Some(e) = sel.and_then(|s| self.hits.get(s)).copied() {
                self.show(e);
            }
        }
        // A definição.
        let r = Rect::new(lw + 16.0, BAR + 8.0, w - lw - 24.0, h - BAR - 16.0);
        self.page(ui, dicts, r);
        ui.c.stroke(rrect(r.x - 0.5, r.y - 0.5, r.w + 1.0, r.h + 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        if let Some(e) = &self.error {
            let e = ui.fonts.ellipsize(e, 11.0, false, r.w - 20.0);
            ui.fonts.draw(ui.c, &e, r.x + 10.0, r.y + r.h - 10.0, 11.0, false, rgba(180, 40, 30, 255));
        }
        drop(guard);

        for a in acts {
            match a.as_str() {
                "close" => ui.close = true,
                "back" if self.hist_pos > 1 => {
                    self.hist_pos -= 1;
                    self.shown = Some(self.history[self.hist_pos - 1]);
                    self.scroll = 0.0;
                }
                "fwd" if self.hist_pos < self.history.len() => {
                    self.hist_pos += 1;
                    self.shown = Some(self.history[self.hist_pos - 1]);
                    self.scroll = 0.0;
                }
                "bigger" => self.px = (self.px + 2.0).min(22.0),
                "smaller" => self.px = (self.px - 2.0).max(10.0),
                "add" => self.open_fb = Some(FileBrowser::open(None, &["ifo", "index"])),
                "reload" => self.reload(),
                _ => {}
            }
        }
        if let Some(fb) = &mut self.open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.open_fb = None;
                if let Choice::Picked(p) = c {
                    match install(&p) {
                        Ok(()) => {
                            self.error = None;
                            self.reload();
                        }
                        Err(e) => self.error = Some(format!("Não deu para adicionar o dicionário: {e}")),
                    }
                }
                ui.redraw = true;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new("Arquivo", vec![MenuItem::new("add", "Adicionar Dicionário…", ""), MenuItem::new("reload", "Recarregar Dicionários", ""), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")]),
            Menu::new(
                "Ir",
                vec![MenuItem::new("back", "Voltar", "Ctrl+["), MenuItem::new("fwd", "Avançar", "Ctrl+]"), MenuItem::Separator, MenuItem::new("bigger", "Aumentar Fonte", "Ctrl+plus"), MenuItem::new("smaller", "Diminuir Fonte", "Ctrl+minus")],
            ),
        ]
    }
}

pub fn main() {
    let query = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let mut app = Dicionario {
        dicts: Arc::default(),
        only: None,
        query,
        shown: None,
        hits: Vec::new(),
        hits_for: None,
        history: Vec::new(),
        hist_pos: 0,
        px: 13.0,
        scroll: 0.0,
        first: true,
        open_fb: None,
        error: None,
    };
    app.reload();
    let opts = WindowOptions { title: "Dicionário".into(), app_id: "glove.Dicionario".into(), size: (720, 480), fixed: false, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-dicionario: {e}");
    }
}
