//! Lembretes: notas adesivas coloridas, salvas sozinhas.
//!
//! Um processo só para todos os lembretes: cada um é uma janela do mesmo app,
//! com moldura própria (a faixa colorida no alto, como no 10.4). Abrir o app
//! de novo só pede ao processo que já está rodando (por um socket) para abrir
//! os lembretes fechados ou um novo. Os textos ficam em
//! ~/.local/share/glove/lembretes/<n>.txt e a cor em <n>.cor.

use glove_config::tr;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use glove_kit::paint::draw::*;
use glove_kit::{run, App, Choice, ColorPanel, FileBrowser, Menu, MenuItem, Rect, TextBuffer, Ui, WindowOptions};

const COLORS: [(&str, &str, (u8, u8, u8)); 6] = [
    ("amarelo", "Amarelo", (255, 244, 160)),
    ("azul", "Azul", (190, 222, 255)),
    ("verde", "Verde", (200, 245, 190)),
    ("rosa", "Rosa", (255, 205, 225)),
    ("roxo", "Roxo", (225, 210, 255)),
    ("cinza", "Cinza", (232, 232, 232)),
];

/// Altura da faixa colorida do alto.
const STRIP: f32 = 16.0;
const SIZE: (u32, u32) = (260, 220);

fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("glove/lembretes")
}

fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("glove-lembretes.sock")
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

fn next_id(open: &[u32]) -> u32 {
    existing().into_iter().chain(open.iter().copied()).max().map(|n| n + 1).unwrap_or(1)
}

struct Note {
    id: u32,
    buf: TextBuffer,
    color: usize,
    deleted: bool,
    /// Recolhido (só a faixa): a altura de antes.
    collapsed: Option<f32>,
    /// Cor escolhida no Painel de Cores (em vez das seis de sempre).
    custom: Option<(u8, u8, u8)>,
}

impl Note {
    fn path(&self, ext: &str) -> PathBuf {
        dir().join(format!("{}.{ext}", self.id))
    }

    fn load(id: u32) -> Self {
        let mut n = Self { id, buf: TextBuffer::new(""), color: 0, deleted: false, collapsed: None, custom: None };
        if let Ok(t) = std::fs::read_to_string(n.path("txt")) {
            n.buf = TextBuffer::new(t);
        }
        if let Ok(c) = std::fs::read_to_string(n.path("cor")) {
            let c = c.trim();
            n.color = COLORS.iter().position(|(id, _, _)| *id == c).unwrap_or(0);
            // "#rrggbb": cor do Painel de Cores.
            if let Some(hex) = c.strip_prefix('#').filter(|h| h.len() == 6) {
                let v = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
                if let (Some(r), Some(g), Some(b)) = (v(0), v(2), v(4)) {
                    n.custom = Some((r, g, b));
                }
            }
        }
        n.buf.paper = Some(n.paper());
        n.buf.font_px = 14.0;
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
        let cor = match self.custom {
            Some((r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
            None => COLORS[self.color].0.to_string(),
        };
        write(self.path("cor"), &cor);
        self.buf.modified = false;
    }

    fn paper(&self) -> (u8, u8, u8) {
        self.custom.unwrap_or(COLORS[self.color].2)
    }

    fn title(&self) -> String {
        let first = self.buf.text().lines().find(|l| !l.trim().is_empty()).unwrap_or("Lembrete");
        first.chars().take(40).collect()
    }

    fn options(&self) -> WindowOptions {
        WindowOptions { title: self.title(), app_id: "glove.Notes".into(), size: SIZE, fixed: false, min_size: Some((140, STRIP as u32)) }
    }
}

struct Notes {
    notes: Vec<Note>,
    /// Janela → lembrete (a primeira janela do kit é a 0; as outras usam o id do lembrete).
    windows: HashMap<u32, u32>,
    /// Lembretes abertos no começo que ainda faltam abrir.
    to_open: Vec<u32>,
    /// Pedidos de outros "glove-notes": "novo", "todos" ou um número.
    requests: Vec<String>,
    listener: Option<UnixListener>,
    /// Lembrete da janela com o foco (para os menus).
    active: u32,
    /// Exportar/Importar: (janela, diálogo, é exportar, tamanho antes).
    file: Option<(u32, FileBrowser, bool, (f32, f32))>,
    titles: HashMap<u32, String>,
    /// Painel de Cores: (janela, painel, tamanho da janela antes).
    colors: Option<(u32, ColorPanel, (f32, f32))>,
}

impl Notes {
    fn note_mut(&mut self, id: u32) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    /// Abre (ou cria) o lembrete numa janela nova.
    fn open(&mut self, ui: &mut Ui, id: u32) {
        if self.windows.values().any(|n| *n == id) {
            return;
        }
        if self.note_mut(id).is_none() {
            let mut n = Note::load(id);
            if !n.path("txt").exists() {
                n.save();
            }
            self.notes.push(n);
        }
        let opts = self.notes.iter().find(|n| n.id == id).map(Note::options).unwrap();
        self.windows.insert(id, id);
        ui.open_window(id, opts);
    }

    fn new_note(&mut self, ui: &mut Ui) {
        let open: Vec<u32> = self.notes.iter().map(|n| n.id).collect();
        let id = next_id(&open);
        self.open(ui, id);
    }

    fn handle_requests(&mut self, ui: &mut Ui) {
        for id in std::mem::take(&mut self.to_open) {
            self.open(ui, id);
        }
        for r in std::mem::take(&mut self.requests) {
            match r.as_str() {
                "novo" => self.new_note(ui),
                "todos" => {
                    let all = existing();
                    if all.is_empty() {
                        self.new_note(ui);
                    }
                    for id in all {
                        self.open(ui, id);
                    }
                }
                n => {
                    if let Ok(id) = n.parse() {
                        self.open(ui, id);
                    }
                }
            }
        }
    }

    /// Faixa colorida: fechar à esquerda, recolher à direita; arrasta a janela.
    fn frame(ui: &mut Ui, note: &mut Note) -> (bool, bool) {
        let (w, h) = (ui.c.w, ui.c.h);
        let (r, g, b) = note.paper();
        let paper = rgba(r, g, b, 255);
        let active = ui.window_active();
        let top = if active { darken(paper, 0.1) } else { darken(paper, 0.04) };
        ui.c.fill_rect(0.0, 0.0, w, STRIP, &vgrad(0.0, STRIP, &[(0.0, lighten(top, 0.15)), (1.0, top)]));
        ui.c.hline(0.0, STRIP - 1.0, w, darken(paper, 0.25));
        // Fechar (×, à esquerda) e recolher/abrir (– / +, à direita). Sempre à vista
        // (mais claros com a nota inativa), para dar para abrir uma nota recolhida.
        let ink = if active { darken(paper, 0.55) } else { darken(paper, 0.35) };
        let close_r = Rect::new(4.0, 3.0, 10.0, 10.0);
        let fold_r = Rect::new(w - 14.0, 3.0, 10.0, 10.0);
        let hc = ui.hit(close_r);
        let hf = ui.hit(fold_r);
        for (r, hot) in [(close_r, hc.hover), (fold_r, hf.hover)] {
            if hot {
                ui.c.fill(rrect(r.x + 0.5, r.y + 0.5, 9.0, 9.0, 1.0), &solid(with_alpha(ink, 0.18)));
            }
            ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, 9.0, 9.0, 1.0), &solid(ink), 1.0);
        }
        // ×
        for (x0, y0, x1, y1) in [(2.5, 2.5, 7.5, 7.5), (7.5, 2.5, 2.5, 7.5)] {
            let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
            pb.move_to(close_r.x + x0, close_r.y + y0);
            pb.line_to(close_r.x + x1, close_r.y + y1);
            ui.c.stroke(pb.finish(), &solid(ink), 1.0);
        }
        // – (recolher) ou + (abrir de novo)
        ui.c.hline(fold_r.x + 2.0, fold_r.y + 5.0, 6.0, ink);
        if note.collapsed.is_some() {
            ui.c.vline(fold_r.x + 5.0, fold_r.y + 2.0, 6.0, ink);
        }
        let close = hc.clicked;
        let mut fold = hf.clicked;
        // O resto da faixa arrasta a janela; clique duplo recolhe.
        let drag = Rect::new(18.0, 0.0, w - 36.0, STRIP);
        let hd = ui.hit(drag);
        if hd.double {
            fold = true;
        } else if hd.pressed {
            ui.start_move();
        }
        // Contorno discreto da folha.
        ui.c.stroke(rrect(0.5, 0.5, w - 1.0, h - 1.0, 0.0), &solid(darken(paper, 0.3)), 1.0);
        (close, fold)
    }
}

impl App for Notes {
    fn ui(&mut self, ui: &mut Ui) {
        self.handle_requests(ui);
        let win = ui.window_id();
        let Some(&id) = self.windows.get(&win) else { return };
        if ui.window_active() {
            self.active = id;
        }
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => acts.push("new".into()),
                "Ctrl+W" => acts.push("close".into()),
                "Ctrl+M" => acts.push("minimize".into()),
                "Ctrl+Shift+E" => acts.push("export".into()),
                "Ctrl+Shift+I" => acts.push("import".into()),
                "Ctrl+Shift+C" => acts.push("cor-outra".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "new" => self.new_note(ui),
                "close" => {
                    if let Some(n) = self.note_mut(id) {
                        n.save();
                    }
                    ui.close = true;
                }
                "minimize" => ui.minimize(),
                "delete" => {
                    if let Some(n) = self.note_mut(id) {
                        let _ = std::fs::remove_file(n.path("txt"));
                        let _ = std::fs::remove_file(n.path("cor"));
                        n.deleted = true;
                    }
                    ui.close = true;
                }
                // A janela cresce enquanto o diálogo está aberto e volta depois.
                "export" | "import" if self.file.is_none() => {
                    let size = (ui.c.w, ui.c.h);
                    let title = self.note_mut(id).map(|n| n.title()).unwrap_or_default();
                    let fb = if a == "export" {
                        let name: String = title.chars().filter(|c| *c != '/').collect();
                        FileBrowser::save(None, &format!("{}.txt", name.trim()))
                    } else {
                        FileBrowser::open(None, &["txt", "md"])
                    };
                    self.file = Some((win, fb, a == "export", size));
                    ui.set_window_size(560.0, 420.0, true);
                }
                "cor-outra" if self.colors.is_none() => {
                    let rgb = self.note_mut(id).map(|n| n.paper()).unwrap_or((255, 244, 160));
                    let mut p = ColorPanel::new(rgb);
                    p.open = true;
                    // A janela cresce para caber o painel e volta depois.
                    self.colors = Some((win, p, (ui.c.w, ui.c.h)));
                    ui.set_window_size(280.0, 340.0, true);
                }
                c if c.starts_with("cor-") => {
                    if let (Some(i), Some(n)) = (COLORS.iter().position(|(cid, _, _)| c[4..] == **cid), self.note_mut(id)) {
                        n.color = i;
                        n.custom = None;
                        n.buf.paper = Some(n.paper());
                        n.save();
                    }
                }
                _ => {}
            }
        }
        if ui.close {
            return;
        }

        let (w, h) = (ui.c.w, ui.c.h);
        let Some(note) = self.notes.iter_mut().find(|n| n.id == id) else { return };
        // Diálogo de Exportar/Importar desta janela.
        if let Some((fw, fb, export, size)) = &mut self.file {
            if *fw == win {
                let (export, size) = (*export, *size);
                ui.set_enabled(false);
                ui.text_area(&mut note.buf, Rect::new(0.0, STRIP, w, h - STRIP));
                if let Some(c) = ui.file_sheet(fb) {
                    self.file = None;
                    ui.set_window_size(size.0, size.1, true);
                    if let Choice::Picked(p) = c {
                        if export {
                            let _ = std::fs::write(&p, note.buf.text());
                        } else if let Ok(t) = std::fs::read_to_string(&p) {
                            note.buf.insert(&t);
                            note.save();
                        }
                    }
                }
                ui.set_enabled(true);
                return;
            }
        }
        if note.collapsed.is_none() && ui.text_area(&mut note.buf, Rect::new(0.0, STRIP, w, h - STRIP)) {
            // Salva a cada mudança: arquivos minúsculos, nada se perde.
            note.save();
        }
        if note.collapsed.is_some() {
            // Recolhida, a faixa toda é a nota: sem o puxador por cima do botão de abrir.
            ui.hide_resize_grip();
        }
        let (close, fold) = Self::frame(ui, note);
        // Painel de Cores desta nota.
        if let Some((cw, panel, size)) = &mut self.colors {
            if *cw == win {
                if ui.color_panel(panel) {
                    note.custom = Some(panel.rgb);
                    note.buf.paper = Some(panel.rgb);
                    note.save();
                }
                if !panel.open {
                    ui.set_window_size(size.0, size.1, true);
                    self.colors = None;
                }
            }
        }
        if close {
            note.save();
            ui.close = true;
        }
        if fold {
            match note.collapsed.take() {
                Some(old_h) => ui.set_window_size(w, old_h, true),
                None => {
                    note.collapsed = Some(h);
                    ui.set_window_size(w, STRIP, true);
                }
            }
        }
        let t = note.title();
        if self.titles.get(&win) != Some(&t) {
            ui.set_title(&t);
            self.titles.insert(win, t);
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let color = self.notes.iter().find(|n| n.id == self.active).map(|n| n.color).unwrap_or(0);
        let custom = self.notes.iter().find(|n| n.id == self.active).is_some_and(|n| n.custom.is_some());
        let mut cores: Vec<MenuItem> = COLORS.iter().enumerate().map(|(i, (id, name, _))| MenuItem::new(&format!("cor-{id}"), tr(name), "").checked(i == color && !custom)).collect();
        cores.push(MenuItem::Separator);
        cores.push(MenuItem::new("cor-outra", tr!("Outra Cor…"), "Ctrl+Shift+C").checked(custom));
        vec![
            Menu::new(
                tr!("Arquivo"),
                vec![
                    MenuItem::new("new", tr!("Novo lembrete"), "Ctrl+N"),
                    MenuItem::Separator,
                    MenuItem::new("import", tr!("Importar texto…"), "Ctrl+Shift+I"),
                    MenuItem::new("export", tr!("Exportar como texto…"), "Ctrl+Shift+E"),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
                    MenuItem::new("delete", tr!("Apagar lembrete"), ""),
                ],
            ),
            Menu::new(
                tr!("Editar"),
                vec![
                    MenuItem::new("undo", tr!("Desfazer"), "Ctrl+Z"),
                    MenuItem::Separator,
                    MenuItem::new("cut", tr!("Recortar"), "Ctrl+X"),
                    MenuItem::new("copy", tr!("Copiar"), "Ctrl+C"),
                    MenuItem::new("paste", tr!("Colar"), "Ctrl+V"),
                    MenuItem::new("select-all", tr!("Selecionar tudo"), "Ctrl+A"),
                ],
            ),
            Menu::new(tr!("Cor"), cores),
        ]
    }

    fn close_window(&mut self, win: u32) -> bool {
        if let Some(n) = self.windows.get(&win).copied().and_then(|id| self.note_mut(id)) {
            n.save();
        }
        true
    }

    fn window_closed(&mut self, win: u32) {
        if let Some(id) = self.windows.remove(&win) {
            self.notes.retain(|n| n.id != id);
            self.titles.remove(&win);
        }
    }

    fn watch_fd(&self) -> Option<RawFd> {
        // O socket vive enquanto o app existir.
        self.listener.as_ref().map(|l| l.as_raw_fd())
    }

    /// Outro "glove-notes" pediu algo: lê os pedidos (o próximo quadro atende).
    fn fd_ready(&mut self) -> bool {
        let Some(l) = &self.listener else { return false };
        while let Ok((mut s, _)) = l.accept() {
            let _ = s.set_read_timeout(Some(std::time::Duration::from_millis(200)));
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf);
            self.requests.extend(buf.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
        }
        true
    }
}

pub fn main() {
    let req = match std::env::args().nth(1).as_deref() {
        Some("novo") => "novo".to_string(),
        Some(n) if n.parse::<u32>().is_ok() => n.to_string(),
        _ => "todos".to_string(),
    };
    // Já tem um Lembretes rodando: só pede a ele e sai.
    let sock = socket_path();
    if let Ok(mut s) = UnixStream::connect(&sock) {
        let _ = s.write_all(format!("{req}\n").as_bytes());
        return;
    }
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).ok();
    if let Some(l) = &listener {
        let _ = l.set_nonblocking(true);
    }
    // Lembretes desta abertura: o primeiro vai na janela 0; os outros abrem no primeiro quadro.
    let mut ids: Vec<u32> = match req.as_str() {
        "novo" => vec![next_id(&[])],
        "todos" => existing(),
        n => vec![n.parse().unwrap_or(1)],
    };
    if ids.is_empty() {
        ids.push(next_id(&[]));
    }
    let first = ids.remove(0);
    let mut note = Note::load(first);
    if !note.path("txt").exists() {
        note.save();
    }
    let opts = note.options();
    let mut windows = HashMap::new();
    windows.insert(0, first);
    glove_kit::use_own_frame();
    let app = Notes { notes: vec![note], windows, to_open: ids, requests: Vec::new(), listener, active: first, file: None, titles: HashMap::new(), colors: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-notes: {e}");
    }
    let _ = std::fs::remove_file(&sock);
}
