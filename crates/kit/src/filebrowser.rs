//! Diálogo Abrir/Salvar em colunas, como folha da janela (10.3): lugares à
//! esquerda, pastas em colunas, "Salvar como:" e Nova Pasta no modo salvar.
//! O app guarda um `FileBrowser` enquanto a folha está aberta e chama
//! `Ui::file_sheet` a cada quadro, até vir uma escolha.

use std::path::{Path, PathBuf};

use glove_paint::{
    draw::*,
    icons::{self, Icon},
};

use crate::{ui::Key, Rect, Ui};

const ROW: f32 = 18.0;
const COL_W: f32 = 168.0;
const SIDE_W: f32 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Open,
    Save,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Picked(PathBuf),
    Cancelled,
}

pub struct FileBrowser {
    pub mode: Mode,
    /// Colunas abertas (a primeira é a pasta inicial).
    columns: Vec<PathBuf>,
    /// Arquivo escolhido (modo abrir).
    selected: Option<PathBuf>,
    /// Nome do arquivo (modo salvar).
    pub name: String,
    /// Extensões aceitas ao abrir (vazio = todas), sem ponto e em minúsculas.
    pub filter: Vec<String>,
    scroll: Vec<f32>,
    error: Option<String>,
    first: bool,
    /// Nova pasta: o nome sendo digitado.
    new_folder: Option<String>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

/// Pasta do usuário (user-dirs.dirs), se existir.
fn user_dir(key: &str) -> Option<PathBuf> {
    let h = home();
    let text = std::fs::read_to_string(h.join(".config/user-dirs.dirs")).ok()?;
    let line = text.lines().find(|l| l.starts_with(&format!("XDG_{key}_DIR=")))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => h.join(rest.trim_start_matches('/')),
        None => PathBuf::from(v),
    };
    (p != h && p.is_dir()).then_some(p)
}

fn icon_for(p: &Path, dir: bool) -> Icon {
    if dir {
        return Icon::Folder;
    }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => Icon::Picture,
        "txt" | "md" | "rs" | "toml" | "json" | "log" | "conf" | "csv" | "sh" | "py" => Icon::Document,
        _ => Icon::File,
    }
}

/// Itens de uma pasta (sem ocultos): pastas primeiro, por nome.
fn list(dir: &Path) -> Vec<(String, PathBuf, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(String, PathBuf, bool)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then(|| {
                let p = e.path();
                let d = p.is_dir();
                (name, p, d)
            })
        })
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    v
}

impl FileBrowser {
    /// Abrir, começando em `dir` (ou na pasta pessoal).
    pub fn open(dir: Option<PathBuf>, filter: &[&str]) -> Self {
        Self::new(Mode::Open, dir, String::new(), filter)
    }

    /// Salvar como `name`, começando em `dir` (ou em Documentos).
    pub fn save(dir: Option<PathBuf>, name: &str) -> Self {
        Self::new(Mode::Save, dir.or_else(|| user_dir("DOCUMENTS")), name.to_string(), &[])
    }

    fn new(mode: Mode, dir: Option<PathBuf>, name: String, filter: &[&str]) -> Self {
        let dir = dir.filter(|d| d.is_dir()).unwrap_or_else(home);
        Self { mode, columns: vec![dir], selected: None, name, filter: filter.iter().map(|s| s.to_lowercase()).collect(), scroll: vec![0.0], error: None, first: true, new_folder: None }
    }

    /// Pasta em que se está (a última coluna).
    pub fn dir(&self) -> PathBuf {
        self.columns.last().cloned().unwrap_or_else(home)
    }

    fn accepts(&self, p: &Path) -> bool {
        self.filter.is_empty() || p.extension().is_some_and(|e| self.filter.contains(&e.to_string_lossy().to_lowercase()))
    }

    fn set_root(&mut self, p: PathBuf) {
        self.columns = vec![p];
        self.scroll = vec![0.0];
        self.selected = None;
    }

    /// Clique num item da coluna `ci`.
    fn click(&mut self, ci: usize, p: PathBuf, dir: bool) {
        self.columns.truncate(ci + 1);
        self.scroll.truncate(ci + 1);
        self.error = None;
        if dir {
            self.columns.push(p);
            self.scroll.push(0.0);
            self.selected = None;
        } else {
            if self.mode == Mode::Save {
                self.name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            }
            self.selected = Some(p);
        }
    }

    /// Confirma: no abrir, o arquivo escolhido; no salvar, a pasta + o nome.
    fn confirm(&mut self) -> Option<Choice> {
        match self.mode {
            Mode::Open => {
                let p = self.selected.clone()?;
                if !self.accepts(&p) {
                    self.error = Some("Este app não abre esse tipo de arquivo.".into());
                    return None;
                }
                Some(Choice::Picked(p))
            }
            Mode::Save => {
                let name = self.name.trim();
                if name.is_empty() {
                    self.error = Some("Digite um nome.".into());
                    return None;
                }
                if name.contains('/') {
                    self.error = Some("O nome não pode ter “/”.".into());
                    return None;
                }
                Some(Choice::Picked(self.dir().join(name)))
            }
        }
    }
}

impl Ui<'_> {
    /// Desenha a folha Abrir/Salvar. Devolve a escolha quando o usuário decide.
    /// Enquanto aberta, nada embaixo responde.
    pub fn file_sheet(&mut self, fb: &mut FileBrowser) -> Option<Choice> {
        self.set_enabled(true);
        let (w, h) = (self.c.w, self.c.h);
        let sw = (w - 24.0).min(620.0).max(320.0);
        let sh = (h - 8.0).min(if fb.mode == Mode::Save { 400.0 } else { 370.0 });
        let sx = ((w - sw) / 2.0).round();
        let ink = rgba(20, 20, 20, 255);

        // Fundo escurecido e a folha listrada encostada no topo.
        self.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
        let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
        self.c.fill(shape(), &solid(self.theme.body_b));
        self.c.pinstripes(sx, 0.0, sw, sh - 6.0, self.theme.body_a, self.theme.body_b);
        self.c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0 / self.c.scale);

        let mut y = 14.0;
        if fb.mode == Mode::Save {
            self.fonts.draw(self.c, "Salvar como:", sx + 18.0, y + 15.0, 13.0, false, ink);
            let mut name = std::mem::take(&mut fb.name);
            let (_, enter) = self.text_field_in(Rect::new(sx + 112.0, y, sw - 130.0, 22.0), &mut name, fb.first);
            fb.name = name;
            fb.first = false;
            if enter {
                if let Some(c) = fb.confirm() {
                    return Some(c);
                }
            }
            y += 34.0;
        } else {
            self.fonts.draw(self.c, "Abrir", sx + 18.0, y + 15.0, 13.0, true, ink);
            y += 26.0;
        }

        // Lugares (à esquerda) e as colunas.
        let body = Rect::new(sx + 14.0, y, sw - 28.0, sh - y - 52.0);
        self.c.fill_rect(body.x, body.y, body.w, body.h, &solid(rgba(255, 255, 255, 255)));
        self.c.stroke(rrect(body.x - 0.5, body.y - 0.5, body.w + 1.0, body.h + 1.0, 0.0), &solid(rgba(130, 130, 136, 255)), 1.0);
        self.c.fill_rect(body.x, body.y, SIDE_W, body.h, &solid(rgba(231, 235, 241, 255)));
        let mut places: Vec<(&str, PathBuf)> = vec![("Disco", "/".into()), ("Início", home())];
        for (label, key) in [("Mesa", "DESKTOP"), ("Documentos", "DOCUMENTS"), ("Downloads", "DOWNLOAD"), ("Imagens", "PICTURES"), ("Música", "MUSIC")] {
            if let Some(p) = user_dir(key) {
                places.push((label, p));
            }
        }
        let mut py = body.y + 4.0;
        for (label, p) in places {
            let r = Rect::new(body.x, py, SIDE_W, ROW + 2.0);
            let here = fb.columns.first() == Some(&p);
            if here {
                self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(self.theme.menu_hot));
            }
            icons::draw_bare(self.c, self.fonts, if label == "Disco" { Icon::Disk } else { Icon::Folder }, r.x + 8.0, r.y + 2.0, 16.0, None);
            self.fonts.draw(self.c, label, r.x + 30.0, r.y + 14.5, 12.0, false, if here { glove_paint::Color::WHITE } else { ink });
            if self.hit(r).clicked {
                fb.set_root(p);
            }
            py += ROW + 2.0;
        }

        // Colunas: mostra as últimas que cabem.
        let area_x = body.x + SIDE_W + 1.0;
        let fit = (((body.w - SIDE_W - 1.0) / COL_W).floor() as usize).max(1);
        let first = fb.columns.len().saturating_sub(fit);
        let cols = fb.columns.clone();
        let mut click: Option<(usize, PathBuf, bool, bool)> = None;
        for (ci, dir) in cols.iter().enumerate().skip(first) {
            let x = area_x + (ci - first) as f32 * COL_W;
            let items = list(dir);
            let r = Rect::new(x, body.y, COL_W, body.h);
            let mut off = fb.scroll.get(ci).copied().unwrap_or(0.0);
            let inner = self.scroll_area(r, &mut off, items.len() as f32 * ROW, ROW);
            if let Some(s) = fb.scroll.get_mut(ci) {
                *s = off;
            }
            let chosen = fb.columns.get(ci + 1).cloned().or_else(|| fb.selected.clone().filter(|s| s.parent() == Some(dir)));
            for (i, (name, p, is_dir)) in items.iter().enumerate() {
                let iy = inner.y + i as f32 * ROW - off;
                if iy + ROW < inner.y || iy > inner.y + inner.h {
                    continue;
                }
                let row = Rect::new(inner.x, iy.max(inner.y), inner.w, (iy + ROW).min(inner.y + inner.h) - iy.max(inner.y));
                let ok = *is_dir || fb.mode == Mode::Save || fb.accepts(p);
                let on = chosen.as_ref() == Some(p);
                if on {
                    self.c.fill_rect(row.x, row.y, row.w, row.h, &solid(self.theme.menu_hot));
                }
                if row.h >= ROW - 0.5 {
                    icons::draw_bare(self.c, self.fonts, icon_for(p, *is_dir), inner.x + 4.0, iy + 1.0, 16.0, None);
                    let col = if on { glove_paint::Color::WHITE } else if ok { ink } else { rgba(160, 160, 164, 255) };
                    let t = self.fonts.ellipsize(name, 12.0, false, inner.w - 36.0);
                    self.fonts.draw(self.c, &t, inner.x + 24.0, iy + 13.5, 12.0, false, col);
                    if *is_dir {
                        crate::ui::disclosure_triangle(self.c, inner.x + inner.w - 8.0, iy + ROW / 2.0, false, if on { glove_paint::Color::WHITE } else { rgba(90, 90, 96, 255) });
                    }
                }
                let resp = self.hit(row);
                if resp.pressed && ok {
                    click = Some((ci, p.clone(), *is_dir, resp.double));
                }
            }
            self.c.vline(x + COL_W - 1.0, body.y, body.h, rgba(200, 200, 206, 255));
        }
        let mut result = None;
        if let Some((ci, p, dir, double)) = click {
            fb.click(ci, p, dir);
            if double && !dir {
                result = fb.confirm();
            }
        }

        // Rodapé: Nova Pasta (salvar), aviso, Cancelar e Abrir/Salvar.
        let by = sh - 38.0;
        if let Some(e) = fb.error.clone() {
            self.fonts.draw(self.c, &e, sx + 18.0, by - 6.0, 11.0, true, rgba(190, 40, 40, 255));
        }
        if fb.mode == Mode::Save {
            match fb.new_folder.clone() {
                Some(mut n) => {
                    let (_, enter) = self.text_field_in(Rect::new(sx + 18.0, by, 160.0, 22.0), &mut n, false);
                    if enter && !n.trim().is_empty() {
                        let p = fb.dir().join(n.trim());
                        match std::fs::create_dir(&p) {
                            Ok(()) => {
                                fb.columns.push(p);
                                fb.scroll.push(0.0);
                                fb.new_folder = None;
                            }
                            Err(e) => {
                                fb.error = Some(format!("Não foi possível criar a pasta: {e}"));
                                fb.new_folder = Some(n);
                            }
                        }
                    } else {
                        fb.new_folder = Some(n);
                    }
                }
                None => {
                    let mut clicked = false;
                    self.area(Rect::new(sx + 18.0, by, 110.0, 24.0), |ui| clicked = ui.button("Nova Pasta"));
                    if clicked {
                        fb.new_folder = Some("pasta sem título".into());
                    }
                }
            }
        }
        let (mut cancel, mut ok) = (false, false);
        let ok_label = if fb.mode == Mode::Save { "Salvar" } else { "Abrir" };
        self.area(Rect::new(sx + sw - 220.0, by, 95.0, 24.0), |ui| cancel = ui.button("Cancelar"));
        self.area(Rect::new(sx + sw - 115.0, by, 97.0, 24.0), |ui| ok = ui.default_button(ok_label));
        let keys: Vec<Key> = self.keys().iter().map(|k| k.key).collect();
        if keys.contains(&Key::Escape) && fb.new_folder.is_none() {
            cancel = true;
        }
        if keys.contains(&Key::Enter) && fb.mode == Mode::Open {
            ok = true;
        }
        if cancel {
            return Some(Choice::Cancelled);
        }
        if ok && result.is_none() {
            result = fb.confirm();
        }
        // Nada embaixo responde enquanto a folha está aberta.
        self.set_enabled(false);
        result
    }
}
