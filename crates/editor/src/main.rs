//! Editor de texto do Glove.

use std::path::{Path, PathBuf};

use glove_kit::{run, AlertKind, App, Choice, FileBrowser, Menu, MenuItem, Rect, TextBuffer, Ui, WindowOptions};

/// Folha (diálogo que desce da barra de título).
enum Sheet {
    None,
    Open(FileBrowser),
    SaveAs { fb: FileBrowser, then_close: bool },
    /// Erro ao abrir ou salvar (alerta com OK).
    Error(String),
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
                Err(e) => ed.sheet = Sheet::Error(format!("Não consegui abrir “{}”: {e}", p.display())),
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
                Err(e) => self.sheet = Sheet::Error(format!("Não consegui salvar “{}”: {e}", p.display())),
            },
            None => {
                self.sheet = Sheet::SaveAs { fb: FileBrowser::save(Some(documents_dir()), "Sem título.txt"), then_close };
            }
        }
    }

    fn open(&mut self, p: PathBuf) -> Result<(), String> {
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
                self.sheet = Sheet::Open(FileBrowser::open(Some(dir), &[]));
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
        if let Sheet::Error(msg) = &self.sheet {
            let msg = msg.clone();
            if ui.alert(AlertKind::Stop, &msg, "", &["OK"]).is_some() {
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        // Abrir e Salvar como: o diálogo em colunas do kit.
        let choice = match &mut self.sheet {
            Sheet::Open(fb) | Sheet::SaveAs { fb, .. } => ui.file_sheet(fb),
            _ => None,
        };
        let Some(choice) = choice else { return };
        let sheet = std::mem::replace(&mut self.sheet, Sheet::None);
        ui.redraw = true;
        match (sheet, choice) {
            (_, Choice::Cancelled) => {}
            (Sheet::Open(_), Choice::Picked(p)) => {
                if let Err(e) = self.open(p) {
                    self.sheet = Sheet::Error(e);
                }
            }
            (Sheet::SaveAs { then_close, .. }, Choice::Picked(p)) => {
                self.path = Some(p);
                self.save(then_close, ui);
            }
            _ => {}
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
                        let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).or_else(|| Some(documents_dir()));
                        self.sheet = Sheet::SaveAs { fb: FileBrowser::save(dir, &self.name()), then_close: false };
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
