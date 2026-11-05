//! Lembretes: notas adesivas coloridas, salvas sozinhas.
//!
//! Cada lembrete é uma janela (um processo leve). Os textos ficam em
//! ~/.local/share/glove/lembretes/<n>.txt e a cor em <n>.cor.

use std::path::PathBuf;

use glove_kit::{run, App, Choice, FileBrowser, Menu, MenuItem, Rect, TextBuffer, Ui, WindowOptions};

const COLORS: [(&str, &str, (u8, u8, u8)); 6] = [
    ("amarelo", "Amarelo", (255, 244, 160)),
    ("azul", "Azul", (190, 222, 255)),
    ("verde", "Verde", (200, 245, 190)),
    ("rosa", "Rosa", (255, 205, 225)),
    ("roxo", "Roxo", (225, 210, 255)),
    ("cinza", "Cinza", (232, 232, 232)),
];

fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("glove/lembretes")
}

fn existing() -> Vec<u32> {
    let mut v: Vec<u32> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.path().file_name()?.to_str()?.strip_suffix(".txt")?.parse().ok())
        .collect();
    v.sort();
    v
}

fn next_id() -> u32 {
    existing().last().map(|n| n + 1).unwrap_or(1)
}

fn spawn(id: Option<u32>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| "glove-notes".into());
    let mut c = std::process::Command::new(exe);
    match id {
        Some(n) => c.arg(n.to_string()),
        None => c.arg("novo"),
    };
    let _ = c.spawn();
}

struct Note {
    id: u32,
    buf: TextBuffer,
    color: usize,
    saved_len: usize,
    deleted: bool,
}

impl Note {
    fn path(&self, ext: &str) -> PathBuf {
        dir().join(format!("{}.{ext}", self.id))
    }

    fn load(id: u32) -> Self {
        let mut n = Self { id, buf: TextBuffer::new(""), color: 0, saved_len: 0, deleted: false };
        if let Ok(t) = std::fs::read_to_string(n.path("txt")) {
            n.buf = TextBuffer::new(t);
        }
        if let Ok(c) = std::fs::read_to_string(n.path("cor")) {
            n.color = COLORS.iter().position(|(id, _, _)| *id == c.trim()).unwrap_or(0);
        }
        n.buf.paper = Some(COLORS[n.color].2);
        n.buf.font_px = 14.0;
        n.saved_len = n.buf.len();
        n
    }

    /// Grava texto e cor (arquivo temporário + renomear).
    fn save(&mut self) {
        if self.deleted {
            return;
        }
        let _ = std::fs::create_dir_all(dir());
        let write = |p: PathBuf, t: &str| {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        };
        write(self.path("txt"), self.buf.text());
        write(self.path("cor"), COLORS[self.color].0);
        self.buf.modified = false;
        self.saved_len = self.buf.len();
    }

    fn title(&self) -> String {
        let first = self.buf.text().lines().find(|l| !l.trim().is_empty()).unwrap_or("Lembrete");
        let t: String = first.chars().take(40).collect();
        t
    }
}

struct Notes {
    note: Note,
    last_title: String,
    /// Exportar/Importar: o diálogo, se é exportar e o tamanho da janela antes dele.
    file: Option<(FileBrowser, bool, (f32, f32))>,
}

impl App for Notes {
    fn ui(&mut self, ui: &mut Ui) {
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => acts.push("new".into()),
                "Ctrl+W" => acts.push("close".into()),
                "Ctrl+M" => acts.push("minimize".into()),
                "Ctrl+Shift+E" => acts.push("export".into()),
                "Ctrl+Shift+I" => acts.push("import".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "new" => spawn(None),
                "close" => {
                    self.note.save();
                    ui.close = true;
                }
                "minimize" => ui.minimize(),
                "delete" => {
                    let _ = std::fs::remove_file(self.note.path("txt"));
                    let _ = std::fs::remove_file(self.note.path("cor"));
                    self.note.deleted = true;
                    ui.close = true;
                }
                // A janela cresce enquanto o diálogo está aberto e volta depois.
                "export" | "import" if self.file.is_none() => {
                    let size = (ui.c.w, ui.c.h);
                    let fb = if a == "export" {
                        let name: String = self.note.title().chars().filter(|c| *c != '/').collect();
                        FileBrowser::save(None, &format!("{}.txt", name.trim()))
                    } else {
                        FileBrowser::open(None, &["txt", "md"])
                    };
                    self.file = Some((fb, a == "export", size));
                    ui.set_window_size(560.0, 420.0, true);
                }
                c if c.starts_with("cor-") => {
                    if let Some(i) = COLORS.iter().position(|(id, _, _)| c[4..] == **id) {
                        self.note.color = i;
                        self.note.buf.paper = Some(COLORS[i].2);
                        self.note.save();
                    }
                }
                _ => {}
            }
        }

        let (w, h) = (ui.c.w, ui.c.h);
        if let Some((fb, export, size)) = &mut self.file {
            let (export, size) = (*export, *size);
            ui.set_enabled(false);
            ui.text_area(&mut self.note.buf, Rect::new(0.0, 0.0, w, h));
            if let Some(c) = ui.file_sheet(fb) {
                self.file = None;
                ui.set_window_size(size.0, size.1, true);
                if let Choice::Picked(p) = c {
                    if export {
                        let _ = std::fs::write(&p, self.note.buf.text());
                    } else if let Ok(t) = std::fs::read_to_string(&p) {
                        self.note.buf.insert(&t);
                        self.note.save();
                    }
                }
            }
            ui.set_enabled(true);
            return;
        }
        if ui.text_area(&mut self.note.buf, Rect::new(0.0, 0.0, w, h)) {
            // Salva a cada mudança: arquivos minúsculos, nada se perde.
            self.note.save();
        }
        let t = self.note.title();
        if t != self.last_title {
            ui.set_title(&t);
            self.last_title = t;
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let cores: Vec<MenuItem> = COLORS.iter().map(|(id, name, _)| MenuItem::new(&format!("cor-{id}"), name, "").checked(COLORS[self.note.color].0 == *id)).collect();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new", "Novo lembrete", "Ctrl+N"),
                    MenuItem::Separator,
                    MenuItem::new("import", "Importar texto…", "Ctrl+Shift+I"),
                    MenuItem::new("export", "Exportar como texto…", "Ctrl+Shift+E"),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                    MenuItem::new("delete", "Apagar lembrete", ""),
                ],
            ),
            Menu::new(
                "Editar",
                vec![
                    MenuItem::new("undo", "Desfazer", "Ctrl+Z"),
                    MenuItem::Separator,
                    MenuItem::new("cut", "Recortar", "Ctrl+X"),
                    MenuItem::new("copy", "Copiar", "Ctrl+C"),
                    MenuItem::new("paste", "Colar", "Ctrl+V"),
                    MenuItem::new("select-all", "Selecionar tudo", "Ctrl+A"),
                ],
            ),
            Menu::new("Cor", cores),
        ]
    }

    fn close_requested(&mut self) -> bool {
        self.note.save();
        true
    }
}

fn main() {
    let arg = std::env::args().nth(1);
    let id = match arg.as_deref() {
        Some("novo") => next_id(),
        Some(n) => n.parse().unwrap_or_else(|_| next_id()),
        None => {
            // Sem argumento: reabre todos os lembretes (ou cria o primeiro).
            let all = existing();
            match all.split_first() {
                Some((first, rest)) => {
                    for n in rest {
                        spawn(Some(*n));
                    }
                    *first
                }
                None => next_id(),
            }
        }
    };
    let mut note = Note::load(id);
    if note.buf.is_empty() && !note.path("txt").exists() {
        note.save();
    }
    let opts = WindowOptions { title: note.title(), app_id: "glove.Notes".into(), size: (260, 220), fixed: false };
    if let Err(e) = run(opts, Notes { note, last_title: String::new(), file: None }) {
        eprintln!("glove-notes: {e}");
    }
}
