//! Editor de texto do Glove.

use std::path::{Path, PathBuf};

use glove_kit::{
    paint::draw::*,
    run,
    ui::Key,
    AlertKind, App, Menu, MenuItem, Rect, TextBuffer, Ui, WindowOptions,
};

/// Folha (diálogo que desce da barra de título).
enum Sheet {
    None,
    Open { path: String, error: Option<String> },
    SaveAs { path: String, error: Option<String>, then_close: bool },
    AskSave,
}

struct Editor {
    buf: TextBuffer,
    path: Option<PathBuf>,
    sheet: Sheet,
    last_title: String,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn documents_dir() -> PathBuf {
    let h = home();
    ["Documentos", "Documents"].iter().map(|d| h.join(d)).find(|p| p.is_dir()).unwrap_or(h)
}

fn expand_tilde(s: &str) -> PathBuf {
    match s.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(s),
    }
}

fn display_path(p: &Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

/// Grava de forma segura: arquivo temporário ao lado + renomear.
fn save_file(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("{}.glove-tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl Editor {
    fn new(path: Option<PathBuf>) -> Self {
        let mut ed = Self { buf: TextBuffer::new(""), path: None, sheet: Sheet::None, last_title: String::new() };
        if let Some(p) = path {
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    ed.buf = TextBuffer::new(t);
                    ed.path = Some(p);
                }
                // Arquivo novo: começa vazio e salva nesse caminho.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ed.path = Some(p),
                Err(e) => ed.sheet = Sheet::Open { path: p.display().to_string(), error: Some(format!("Não consegui abrir: {e}")) },
            }
        }
        ed
    }

    fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Sem título".into())
    }

    fn save(&mut self, then_close: bool, ui: &mut Ui) {
        match self.path.clone() {
            Some(p) => match save_file(&p, self.buf.text()) {
                Ok(()) => {
                    self.buf.modified = false;
                    if then_close {
                        ui.close = true;
                    }
                }
                Err(e) => self.sheet = Sheet::SaveAs { path: display_path(&p), error: Some(format!("Não consegui salvar: {e}")), then_close },
            },
            None => {
                let suggestion = documents_dir().join("Sem título.txt");
                self.sheet = Sheet::SaveAs { path: display_path(&suggestion), error: None, then_close };
            }
        }
    }

    fn open(&mut self, path: &str) -> Result<(), String> {
        let p = expand_tilde(path.trim());
        let t = std::fs::read_to_string(&p).map_err(|e| format!("Não consegui abrir: {e}"))?;
        // Documento atual vazio e intocado: abre aqui; senão, numa janela nova.
        if self.buf.is_empty() && !self.buf.modified && self.path.is_none() {
            self.buf = TextBuffer::new(t);
            self.path = Some(p);
        } else {
            spawn_self(Some(&p));
        }
        Ok(())
    }

    fn font_px(&self) -> f32 {
        if self.buf.font_px > 0.0 { self.buf.font_px } else { 13.0 }
    }

    fn run_action(&mut self, action: Option<&str>, ui: &mut Ui) {
        match action {
            Some("new") => spawn_self(None),
            Some("open") => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(documents_dir);
                self.sheet = Sheet::Open { path: format!("{}/", display_path(&dir)), error: None };
            }
            Some("save") => self.save(false, ui),
            Some("close") => {
                if self.buf.modified {
                    self.sheet = Sheet::AskSave;
                } else {
                    ui.close = true;
                }
            }
            Some("minimize") => ui.minimize(),
            Some("bigger") => self.buf.font_px = (self.font_px() + 2.0).min(48.0),
            Some("smaller") => self.buf.font_px = (self.font_px() - 2.0).max(9.0),
            _ => {}
        }
    }

    /// Folha centralizada no topo, com o fundo da janela escurecido.
    fn sheet(&mut self, ui: &mut Ui) {
        if matches!(self.sheet, Sheet::None) {
            return;
        }
        if matches!(self.sheet, Sheet::AskSave) {
            let title = format!("Deseja salvar as alterações feitas no documento “{}”?", self.name());
            match ui.alert(AlertKind::Caution, &title, "Se você não salvar, as alterações serão perdidas.", &["Não salvar", "Cancelar", "Salvar"]) {
                Some(0) => ui.close = true,
                Some(1) => {
                    self.sheet = Sheet::None;
                    ui.redraw = true;
                }
                Some(_) => {
                    self.sheet = Sheet::None;
                    self.save(true, ui);
                    ui.redraw = true;
                }
                None => {}
            }
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
        let sw = (w - 40.0).min(440.0);
        let sh = 150.0;
        let sx = (w - sw) / 2.0;
        if let Some(shadow) = Canvas::shadow(-14.0, 0.0, sw + 28.0, sh + 18.0, sw, sh, &ShadowSpec { blur: 14.0, offset_y: 4.0, radius: 6.0, alpha: 0.45 }, ui.c.scale) {
            ui.c.draw_canvas(&shadow, sx - 14.0, 0.0);
        }
        ui.c.fill(rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 7.0, 7.0]), &solid(rgba(238, 238, 238, 248)));
        ui.c.pinstripes(sx, 0.0, sw, sh - 7.0, rgba(232, 232, 232, 255), rgba(244, 244, 244, 255));
        ui.c.stroke(rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 7.0, 7.0]), &solid(rgba(0, 0, 0, 70)), 1.0 / ui.c.scale);

        let enter = ui.keys().iter().any(|k| k.key == Key::Enter);
        let escape = ui.keys().iter().any(|k| k.key == Key::Escape);
        let name = self.name();
        let mut next: Option<Sheet> = None;
        let mut do_save: Option<(PathBuf, bool)> = None;
        let mut do_open: Option<String> = None;

        ui.set_enabled(true);
        ui.area(Rect::new(sx + 20.0, 18.0, sw - 40.0, sh - 30.0), |ui| match &mut self.sheet {
            Sheet::Open { path, error } => {
                ui.heading("Abrir arquivo");
                ui.text_field(path, sw - 40.0);
                if let Some(e) = error {
                    ui.small(e);
                } else {
                    ui.small("Digite o caminho do arquivo.");
                }
                ui.horizontal(|ui| {
                    ui.space(sw - 40.0 - 170.0);
                    if ui.button("Cancelar") || escape {
                        next = Some(Sheet::None);
                    }
                    if ui.default_button("Abrir") || enter {
                        do_open = Some(path.clone());
                    }
                });
            }
            Sheet::SaveAs { path, error, then_close } => {
                ui.heading(&format!("Salvar “{name}” como:"));
                ui.text_field(path, sw - 40.0);
                if let Some(e) = error {
                    ui.small(e);
                } else {
                    ui.small("Digite o caminho completo, com o nome do arquivo.");
                }
                let tc = *then_close;
                ui.horizontal(|ui| {
                    ui.space(sw - 40.0 - 170.0);
                    if ui.button("Cancelar") || escape {
                        next = Some(Sheet::None);
                    }
                    if ui.default_button("Salvar") || enter {
                        do_save = Some((expand_tilde(path.trim()), tc));
                    }
                });
            }
            Sheet::AskSave | Sheet::None => {}
        });

        if let Some(p) = do_open {
            match self.open(&p) {
                Ok(()) => next = Some(Sheet::None),
                Err(e) => {
                    if let Sheet::Open { error, .. } = &mut self.sheet {
                        *error = Some(e);
                    }
                }
            }
        }
        if let Some((p, then_close)) = do_save {
            if !p.as_os_str().is_empty() {
                self.path = Some(p);
            }
            self.sheet = Sheet::None;
            self.save(then_close, ui);
            if matches!(self.sheet, Sheet::SaveAs { .. }) {
                next = None;
            }
        }
        if let Some(s) = next {
            self.sheet = s;
            ui.redraw = true;
        }
    }
}

fn spawn_self(path: Option<&Path>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| "glove-edit".into());
    let mut cmd = std::process::Command::new(exe);
    if let Some(p) = path {
        cmd.arg(p);
    }
    let _ = cmd.spawn();
}

impl App for Editor {
    fn ui(&mut self, ui: &mut Ui) {
        let sheet_open = !matches!(self.sheet, Sheet::None);

        // Atalhos (e itens de menu, que chegam como os mesmos atalhos)
        if !sheet_open {
            let mut action = None;
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+N" => action = Some("new"),
                    "Ctrl+O" => action = Some("open"),
                    "Ctrl+S" => action = Some("save"),
                    "Ctrl+W" => action = Some("close"),
                    "Ctrl+M" => action = Some("minimize"),
                    "Ctrl+equal" | "Ctrl+plus" => action = Some("bigger"),
                    "Ctrl+minus" => action = Some("smaller"),
                    "Ctrl+Shift+S" => {
                        let p = self.path.clone().unwrap_or_else(|| documents_dir().join("Sem título.txt"));
                        self.sheet = Sheet::SaveAs { path: display_path(&p), error: None, then_close: false };
                    }
                    _ => {}
                }
            }
            for a in ui.menu_actions() {
                match a.as_str() {
                    "delete" => self.buf.delete(),
                    "font-reset" => self.buf.font_px = 0.0,
                    _ => {}
                }
            }
            self.run_action(action, ui);
        }

        let title = format!("{}{}", self.name(), if self.buf.modified { " — Editado" } else { "" });
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }

        // A janela é só o documento.
        ui.set_enabled(!sheet_open);
        let (w, h) = (ui.c.w, ui.c.h);
        ui.text_area(&mut self.buf, Rect::new(0.0, 0.0, w, h));
        self.sheet(ui);
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.buf.has_selection();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new", "Novo", "Ctrl+N"),
                    MenuItem::new("open", "Abrir…", "Ctrl+O"),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                    MenuItem::new("save", "Salvar", "Ctrl+S"),
                    MenuItem::new("save-as", "Salvar como…", "Ctrl+Shift+S"),
                ],
            ),
            Menu::new(
                "Editar",
                vec![
                    MenuItem::new("undo", "Desfazer", "Ctrl+Z").enabled(self.buf.can_undo()),
                    MenuItem::new("redo", "Refazer", "Ctrl+Shift+Z").enabled(self.buf.can_redo()),
                    MenuItem::Separator,
                    MenuItem::new("cut", "Recortar", "Ctrl+X").enabled(sel),
                    MenuItem::new("copy", "Copiar", "Ctrl+C").enabled(sel),
                    MenuItem::new("paste", "Colar", "Ctrl+V"),
                    MenuItem::new("delete", "Apagar", "").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("select-all", "Selecionar tudo", "Ctrl+A"),
                ],
            ),
            Menu::new(
                "Formatar",
                vec![
                    MenuItem::new("bigger", "Aumentar fonte", "Ctrl+equal"),
                    MenuItem::new("smaller", "Diminuir fonte", "Ctrl+minus"),
                    MenuItem::new("font-reset", "Tamanho normal", "").enabled(self.buf.font_px > 0.0),
                ],
            ),
            Menu::new("Janela", vec![MenuItem::new("minimize", "Minimizar", "Ctrl+M")]),
        ]
    }

    fn close_requested(&mut self) -> bool {
        if self.buf.modified {
            self.sheet = Sheet::AskSave;
            false
        } else {
            true
        }
    }
}

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let ed = Editor::new(path);
    let title = ed.name();
    let opts = WindowOptions { title, app_id: "glove.TextEditor".into(), size: (640, 480), fixed: false };
    if let Err(e) = run(opts, ed) {
        eprintln!("glove-edit: {e}");
    }
}
