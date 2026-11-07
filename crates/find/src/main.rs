//! Busca: encontra arquivos e pastas pelo nome, em segundo plano.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}},
    run,
    ui::Key,
    App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

/// Altura do painel de caminho (embaixo dos resultados).
const PATH_H: f32 = 96.0;
const MAX_RESULTS: usize = 5000;
const SKIP: [&str; 5] = ["/proc", "/sys", "/dev", "/run", "/snap"];

/// Um resultado, já com o que as colunas mostram.
#[derive(Clone)]
struct Hit {
    path: PathBuf,
    dir: bool,
    size: u64,
    modified: Option<std::time::SystemTime>,
}

#[derive(Default)]
struct Shared {
    results: Vec<Hit>,
    done: bool,
}

fn human_size(b: u64) -> String {
    let f = b as f64;
    let s = if f >= 1e9 { format!("{:.1} GB", f / 1e9) } else if f >= 1e6 { format!("{:.1} MB", f / 1e6) } else if f >= 1e3 { format!("{:.0} KB", f / 1e3) } else { format!("{b} bytes") };
    s.replace('.', ",")
}

/// "Hoje, 14:30", "Ontem, 09:05" ou "05/11/2026, 14:30".
fn human_date(t: Option<std::time::SystemTime>) -> String {
    let Some(t) = t else { return "--".into() };
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let tm = |s: i64| unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let t = s as libc::time_t;
        libc::localtime_r(&t, &mut tm);
        tm
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (a, n, y) = (tm(secs), tm(now), tm(now - 86_400));
    let same = |x: &libc::tm, z: &libc::tm| x.tm_year == z.tm_year && x.tm_yday == z.tm_yday;
    let hm = format!("{:02}:{:02}", a.tm_hour, a.tm_min);
    if same(&a, &n) {
        format!("Hoje, {hm}")
    } else if same(&a, &y) {
        format!("Ontem, {hm}")
    } else {
        format!("{:02}/{:02}/{}, {hm}", a.tm_mday, a.tm_mon + 1, a.tm_year + 1900)
    }
}

fn kind_name(h: &Hit) -> &'static str {
    if h.dir {
        return "Pasta";
    }
    match h.path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => "Imagem",
        Some("txt" | "md") => "Documento de texto",
        Some("rs" | "py" | "c" | "h" | "js" | "sh" | "toml" | "json") => "Código",
        Some("pdf") => "PDF",
        Some("mp3" | "ogg" | "flac" | "wav") => "Áudio",
        Some("mp4" | "mkv" | "webm" | "avi") => "Vídeo",
        Some("zip" | "gz" | "xz" | "tar" | "7z") => "Arquivo compactado",
        _ => "Documento",
    }
}

struct Find {
    query: String,
    /// Onde buscar: (nome, pasta). A pasta pedida na linha de comando vem primeiro.
    places: Vec<(String, PathBuf)>,
    place: usize,
    shared: Arc<Mutex<Shared>>,
    /// Geração da busca atual (buscas antigas param ao ver que mudou).
    generation: Arc<AtomicU64>,
    searching: bool,
    view: Vec<Hit>,
    /// Índice em `view` do item escolhido.
    selected: Option<usize>,
    table: TableState,
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
                let md = e.metadata().ok();
                batch.push(Hit { path: path.clone(), dir: is_dir, size: md.as_ref().map(|m| m.len()).unwrap_or(0), modified: md.and_then(|m| m.modified().ok()) });
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
    /// `glove-find [texto] [pasta]`: já abre buscando.
    fn from_args() -> Self {
        let mut args = std::env::args().skip(1);
        let mut f = Self::new();
        if let Some(q) = args.next() {
            f.query = q;
        }
        if let Some(dir) = args.next().map(PathBuf::from).filter(|d| d.is_dir()) {
            match f.places.iter().position(|(_, p)| *p == dir) {
                Some(i) => f.place = i,
                None => {
                    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string());
                    f.places.insert(0, (name, dir));
                    f.place = 0;
                }
            }
        }
        if !f.query.trim().is_empty() {
            f.start();
        }
        f
    }

    fn new() -> Self {
        let mut fds = [0; 2];
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        Self {
            query: String::new(),
            places: places(),
            place: 0,
            shared: Arc::default(),
            generation: Arc::new(AtomicU64::new(0)),
            searching: false,
            view: Vec::new(),
            selected: None,
            table: TableState::sorted_by(0, true),
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
        self.table.scroll = 0.0;
        if q.is_empty() {
            self.searching = false;
            return;
        }
        self.searching = true;
        let root = self.places[self.place].1.clone();
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
                ui.search_field(&mut self.query, "Parte do nome", (w - 360.0).max(140.0));
                ui.label_right("em", 24.0);
                let names: Vec<String> = self.places.iter().map(|p| p.0.clone()).collect();
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

        // Resultados numa tabela ordenável (Nome, Data, Tamanho, Tipo).
        let cols = [Column::new("Nome", 0.4), Column::new("Data de modificação", 0.26), Column::new("Tamanho", 0.14).right(), Column::new("Tipo", 0.2)];
        let mut order: Vec<usize> = (0..self.view.len()).collect();
        let (col, asc) = self.table.sort.unwrap_or((0, true));
        let v = &self.view;
        order.sort_by(|&a, &b| {
            let (x, y) = (&v[a], &v[b]);
            let o = match col {
                1 => x.modified.cmp(&y.modified),
                2 => x.size.cmp(&y.size),
                3 => kind_name(x).cmp(kind_name(y)),
                _ => x.path.file_name().map(|n| n.to_string_lossy().to_lowercase()).cmp(&y.path.file_name().map(|n| n.to_string_lossy().to_lowercase())),
            };
            if asc { o } else { o.reverse() }
        });
        let rows: Vec<TableRow> = order
            .iter()
            .map(|&i| {
                let h = &v[i];
                TableRow {
                    cells: vec![
                        h.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        human_date(h.modified),
                        if h.dir { "--".into() } else { human_size(h.size) },
                        kind_name(h).to_string(),
                    ],
                    icon: Some(icon_for(&h.path)),
                    ..TableRow::default()
                }
            })
            .collect();
        let sel_row = self.selected.and_then(|s| order.iter().position(|&i| i == s));
        let has_sel = self.selected.is_some();
        let path_h = if has_sel { PATH_H } else { 0.0 };
        let area = Rect::new(0.0, bar, w, h - bar - 22.0 - path_h);
        // Setas mudam a seleção.
        for k in ui.keys() {
            let n = order.len();
            if n == 0 {
                break;
            }
            let next = match (k.key, sel_row) {
                (Key::Down, Some(r)) => Some((r + 1).min(n - 1)),
                (Key::Up, Some(r)) => Some(r.saturating_sub(1)),
                (Key::Down | Key::Up, None) => Some(0),
                _ => None,
            };
            if let Some(r) = next {
                self.selected = Some(order[r]);
                self.table.reveal(r);
            }
        }
        let resp = ui.table(area, &cols, &rows, sel_row, &mut self.table, &TableStyle::default());
        if let Some(r) = resp.pressed {
            self.selected = Some(order[r]);
        }
        if let Some(r) = resp.double {
            self.selected = Some(order[r]);
            acts.push("open".into());
        }
        if self.view.is_empty() {
            let msg = if self.searching { "Buscando…" } else { "Digite parte do nome e pressione Enter." };
            let tw = ui.text_width(msg, 13.0, false);
            ui.fonts.draw(ui.c, msg, (w - tw) / 2.0, area.y + area.h / 2.0, 13.0, false, rgba(140, 140, 140, 255));
        }

        // Painel de caminho: da raiz até o item escolhido, cada pasta clicável (como no 10.4).
        if let Some(h) = self.selected.and_then(|i| self.view.get(i)).cloned() {
            let py = area.y + area.h;
            ui.c.fill_rect(0.0, py, w, PATH_H, &solid(rgba(244, 244, 246, 255)));
            ui.c.hline(0.0, py, w, rgba(150, 150, 156, 255));
            let comps: Vec<PathBuf> = h.path.ancestors().map(Path::to_path_buf).collect::<Vec<_>>().into_iter().rev().collect();
            // Só as últimas que cabem (a raiz sobe primeiro).
            let fit = ((PATH_H - 8.0) / 15.0) as usize;
            let skip = comps.len().saturating_sub(fit);
            for (k, c) in comps.iter().skip(skip).enumerate() {
                let y = py + 6.0 + k as f32 * 15.0;
                let x = 10.0 + k as f32 * 14.0;
                let is_item = *c == h.path;
                let name = if c.as_os_str() == "/" { "Computador".to_string() } else { c.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() };
                let icon = if is_item { icon_for(&h.path) } else if c.as_os_str() == "/" { Icon::Disk } else { Icon::Folder };
                icons::draw(ui.c, ui.fonts, icon, x, y, 13.0, None);
                ui.fonts.draw(ui.c, &name, x + 17.0, y + 11.0, 11.0, is_item, rgba(30, 30, 34, 255));
                let tw = ui.text_width(&name, 11.0, is_item);
                if ui.hit(Rect::new(x, y, tw + 20.0, 15.0)).double {
                    if c.is_dir() {
                        let _ = std::process::Command::new(sibling("glove-files")).arg(c).spawn();
                    } else {
                        open(c);
                    }
                }
            }
        }

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
                    if let Some(h) = self.selected.and_then(|i| self.view.get(i)) {
                        open(&h.path);
                    }
                }
                "reveal" => {
                    if let Some(p) = self.selected.and_then(|i| self.view.get(i)).and_then(|h| h.path.parent()) {
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

pub fn main() {
    let opts = WindowOptions { title: "Busca".into(), app_id: "glove.Find".into(), size: (680, 480), fixed: false, min_size: None };
    if let Err(e) = run(opts, Find::from_args()) {
        eprintln!("glove-find: {e}");
    }
}
