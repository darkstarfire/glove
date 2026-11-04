//! Busca: encontra arquivos e pastas pelo nome, em segundo plano.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}, Color},
    run,
    ui::Key,
    App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const ROW_H: f32 = 20.0;
const MAX_RESULTS: usize = 5000;
const SKIP: [&str; 5] = ["/proc", "/sys", "/dev", "/run", "/snap"];

#[derive(Default)]
struct Shared {
    results: Vec<PathBuf>,
    done: bool,
}

struct Find {
    query: String,
    place: usize,
    shared: Arc<Mutex<Shared>>,
    /// Geração da busca atual (buscas antigas param ao ver que mudou).
    generation: Arc<AtomicU64>,
    searching: bool,
    view: Vec<PathBuf>,
    selected: Option<usize>,
    scroll: f32,
    wake_r: OwnedFd,
    wake_w: Arc<OwnedFd>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

fn places() -> Vec<(String, PathBuf)> {
    vec![("Início".into(), home()), ("Computador".into(), "/".into())]
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

fn open(p: &Path) {
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let text = ["txt", "md", "rs", "toml", "json", "c", "h", "py", "sh", "log", "conf", "ini", "csv", "xml", "html", "css", "js", "yaml", "yml"];
    let _ = if p.is_dir() {
        std::process::Command::new(sibling("glove-files")).arg(p).spawn()
    } else if text.contains(&ext.as_str()) {
        std::process::Command::new(sibling("glove-edit")).arg(p).spawn()
    } else if ["png", "jpg", "jpeg"].contains(&ext.as_str()) {
        std::process::Command::new(sibling("glove-view")).arg(p).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(p).spawn()
    };
}

fn icon_for(p: &Path) -> Icon {
    if p.is_dir() {
        return Icon::Folder;
    }
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => Icon::Picture,
        Some("txt" | "md" | "rs" | "toml" | "json" | "py" | "sh" | "log" | "conf" | "csv") => Icon::Document,
        _ => Icon::File,
    }
}

/// Percorre as pastas procurando `needle` no nome (sem diferenciar maiúsculas).
fn walk(root: PathBuf, needle: String, geracao: u64, current: Arc<AtomicU64>, shared: Arc<Mutex<Shared>>, wake: Arc<OwnedFd>) {
    let ping = || unsafe {
        libc::write(wake.as_raw_fd(), b"x".as_ptr() as *const _, 1);
    };
    let mut stack = vec![root];
    let mut batch = Vec::new();
    let mut last = std::time::Instant::now();
    let mut found = 0;
    while let Some(dir) = stack.pop() {
        if current.load(Ordering::Relaxed) != geracao {
            return;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if name.to_lowercase().contains(&needle) {
                batch.push(path.clone());
                found += 1;
            }
            if is_dir && !SKIP.iter().any(|s| path == Path::new(s)) {
                stack.push(path);
            }
        }
        if !batch.is_empty() && (last.elapsed().as_millis() > 120 || found >= MAX_RESULTS) {
            shared.lock().unwrap().results.append(&mut batch);
            ping();
            last = std::time::Instant::now();
        }
        if found >= MAX_RESULTS {
            break;
        }
    }
    if current.load(Ordering::Relaxed) == geracao {
        let mut s = shared.lock().unwrap();
        s.results.append(&mut batch);
        s.done = true;
        drop(s);
        ping();
    }
}

impl Find {
    fn new() -> Self {
        let mut fds = [0; 2];
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        Self {
            query: String::new(),
            place: 0,
            shared: Arc::default(),
            generation: Arc::new(AtomicU64::new(0)),
            searching: false,
            view: Vec::new(),
            selected: None,
            scroll: 0.0,
            wake_r: r,
            wake_w: Arc::new(w),
        }
    }

    fn start(&mut self) {
        let q = self.query.trim().to_lowercase();
        let geracao = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.shared = Arc::default();
        self.view.clear();
        self.selected = None;
        self.scroll = 0.0;
        if q.is_empty() {
            self.searching = false;
            return;
        }
        self.searching = true;
        let root = places()[self.place].1.clone();
        let (cur, shared, wake) = (self.generation.clone(), self.shared.clone(), self.wake_w.clone());
        std::thread::spawn(move || walk(root, q, geracao, cur, shared, wake));
    }
}

impl App for Find {
    fn watch_fd(&self) -> Option<RawFd> {
        Some(self.wake_r.as_raw_fd())
    }

    fn fd_ready(&mut self) -> bool {
        let mut buf = [0u8; 256];
        while unsafe { libc::read(self.wake_r.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) } > 0 {}
        let s = self.shared.lock().unwrap();
        self.view = s.results.clone();
        if s.done {
            self.searching = false;
        }
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match (k.key, k.combo.as_str()) {
                (Key::Enter, _) => acts.push("search".into()),
                (_, "Ctrl+O") => acts.push("open".into()),
                (_, "Ctrl+W") => acts.push("close".into()),
                (_, "Ctrl+R") => acts.push("reveal".into()),
                _ => {}
            }
        }

        // Barra de busca
        let bar = 50.0;
        ui.c.fill_rect(0.0, 0.0, w, bar, &vgrad(0.0, bar, &[(0.0, rgba(246, 246, 246, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, bar - 1.0, w, rgba(130, 130, 136, 255));
        let mut clicked = false;
        let mut place = self.place;
        ui.area(Rect::new(14.0, 14.0, w - 28.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                icons::draw(ui.c, ui.fonts, Icon::Search, 14.0, 10.0, 30.0, None);
                ui.space(34.0);
                ui.text_field(&mut self.query, (w - 360.0).max(140.0));
                ui.label_right("em", 24.0);
                let names: Vec<String> = places().into_iter().map(|p| p.0).collect();
                ui.popup(&mut place, &names, 130.0);
                clicked = ui.default_button("Buscar");
            });
        });
        if place != self.place {
            self.place = place;
            acts.push("search".into());
        }
        if clicked {
            acts.push("search".into());
        }

        // Resultados
        let content = self.view.len() as f32 * ROW_H;
        let area = ui.scroll_area(Rect::new(0.0, bar, w, h - bar - 22.0), &mut self.scroll, content, ROW_H);
        let first = (self.scroll / ROW_H).floor() as usize;
        let count = (area.h / ROW_H).ceil() as usize + 1;
        for i in first..(first + count).min(self.view.len()) {
            let y = area.y + i as f32 * ROW_H - self.scroll;
            let (top, bot) = (y.max(area.y), (y + ROW_H).min(area.y + area.h));
            if bot > top {
                let r = ui.hit(Rect::new(0.0, top, area.w, bot - top));
                if r.pressed {
                    self.selected = Some(i);
                    if r.double {
                        acts.push("open".into());
                    }
                }
            }
        }
        let (view, sel, off, accent) = (&self.view, self.selected, self.scroll, ui.theme.accent);
        let home_s = home().to_string_lossy().into_owned();
        ui.clipped(9, area, |c, fonts| {
            c.clear(Color::WHITE);
            for i in first..(first + count).min(view.len()) {
                let p = &view[i];
                let y = i as f32 * ROW_H - off;
                let on = sel == Some(i);
                if on {
                    c.fill_rect(0.0, y, area.w, ROW_H, &vgrad(y, y + ROW_H, &[(0.0, lighten(accent, 0.15)), (1.0, accent)]));
                } else if i % 2 == 1 {
                    c.fill_rect(0.0, y, area.w, ROW_H, &solid(rgba(237, 243, 254, 255)));
                }
                let ink = if on { Color::WHITE } else { rgba(20, 20, 20, 255) };
                let dim = if on { rgba(230, 236, 255, 255) } else { rgba(110, 110, 110, 255) };
                icons::draw(c, fonts, icon_for(p), 8.0, y + 2.0, 16.0, None);
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let name = fonts.ellipsize(&name, 12.0, false, area.w * 0.4 - 30.0);
                fonts.draw(c, &name, 30.0, y + 14.0, 12.0, false, ink);
                let parent = p.parent().map(|d| d.to_string_lossy().replacen(&home_s, "~", 1)).unwrap_or_default();
                let parent = fonts.ellipsize(&parent, 11.0, false, area.w * 0.6 - 16.0);
                fonts.draw(c, &parent, area.w * 0.4 + 8.0, y + 14.0, 11.0, false, dim);
            }
            if view.is_empty() {
                let msg = "Digite parte do nome e pressione Enter.";
                let tw = fonts.measure(msg, 13.0, false);
                fonts.draw(c, msg, (area.w - tw) / 2.0, area.h / 2.0, 13.0, false, rgba(140, 140, 140, 255));
            }
        });

        // Status
        let sy = h - 22.0;
        ui.c.fill_rect(0.0, sy, w, 22.0, &vgrad(sy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(218, 218, 222, 255))]));
        ui.c.hline(0.0, sy, w, rgba(150, 150, 156, 255));
        let n = self.view.len();
        let status = if self.searching {
            format!("Buscando… {n} encontrados")
        } else if self.query.trim().is_empty() {
            String::new()
        } else {
            format!("{n} {}{}", if n == 1 { "item encontrado" } else { "itens encontrados" }, if n >= MAX_RESULTS { " (limite)" } else { "" })
        };
        // Enquanto busca, o giratório gira ao lado do texto.
        let tx = if self.searching {
            ui.spinner_at(21.0, sy + 11.0, 14.0);
            34.0
        } else {
            14.0
        };
        ui.fonts.draw(ui.c, &status, tx, sy + 15.0, 11.0, false, rgba(60, 60, 60, 255));

        for a in acts {
            match a.as_str() {
                "search" => self.start(),
                "open" => {
                    if let Some(p) = self.selected.and_then(|i| self.view.get(i)) {
                        open(p);
                    }
                }
                "reveal" => {
                    if let Some(p) = self.selected.and_then(|i| self.view.get(i)).and_then(|p| p.parent()) {
                        let _ = std::process::Command::new(sibling("glove-files")).arg(p).spawn();
                    }
                }
                "close" => ui.close = true,
                _ => {}
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.selected.is_some();
        vec![Menu::new(
            "Arquivo",
            vec![
                MenuItem::new("search", "Buscar", ""),
                MenuItem::new("open", "Abrir", "Ctrl+O").enabled(sel),
                MenuItem::new("reveal", "Mostrar no Arquivos", "Ctrl+R").enabled(sel),
                MenuItem::Separator,
                MenuItem::new("close", "Fechar", "Ctrl+W"),
            ],
        )]
    }
}

fn main() {
    let opts = WindowOptions { title: "Busca".into(), app_id: "glove.Find".into(), size: (620, 420), fixed: false };
    if let Err(e) = run(opts, Find::new()) {
        eprintln!("glove-find: {e}");
    }
}
