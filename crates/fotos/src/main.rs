//! Fotos: a biblioteca de imagens da pasta Imagens. Miniaturas em grade, por mês,
//! com os álbuns (as subpastas) na lateral. Clique duplo abre a foto grande: setas
//! passam, Espaço liga a apresentação, Esc volta.
//!
//! As miniaturas são feitas em duas threads e só as que aparecem ficam na memória
//! (as menos usadas saem quando passam de um limite).

use glove_config::tr;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use glove_ui::{
    paint::{
        draw::*,
        icons::Icon,
        tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform},
    },
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const SIDE_W: f32 = 190.0;
const BOTTOM_H: f32 = 34.0;
/// Lado maior das miniaturas guardadas e quantas ficam na memória.
const THUMB: u32 = 220;
const KEEP: usize = 260;
const SLIDE: Duration = Duration::from_secs(4);

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// A pasta de imagens do usuário.
fn pictures() -> PathBuf {
    let h = home();
    let dirs = std::fs::read_to_string(h.join(".config/user-dirs.dirs")).unwrap_or_default();
    dirs.lines()
        .find_map(|l| l.strip_prefix("XDG_PICTURES_DIR="))
        .map(|v| PathBuf::from(v.trim().trim_matches('"').replace("$HOME", &h.to_string_lossy())))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| ["Imagens", "Pictures", "Fotos"].iter().map(|d| h.join(d)).find(|p| p.is_dir()).unwrap_or_else(|| h.join("Pictures")))
}

#[derive(Clone)]
struct Foto {
    path: PathBuf,
    time: SystemTime,
    /// Subpasta de primeiro nível (o álbum), vazio = solta na pasta Imagens.
    album: String,
}

fn scan(root: &Path) -> Vec<Foto> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() && depth < 5 {
                stack.push((p, depth + 1));
            } else if ft.is_file() {
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                if glove_ui::image::EXTENSIONS.contains(&ext.as_str()) {
                    let time = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                    let album = p.strip_prefix(root).ok().and_then(|r| r.components().next()).filter(|_| p.parent() != Some(root)).map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
                    out.push(Foto { path: p, time, album });
                }
            }
        }
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out
}

/// Reduz até o lado maior caber em `side` (metades sucessivas e o resto bilinear).
fn shrink(mut img: Pixmap, side: u32) -> Pixmap {
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    let step = |src: &Pixmap, w: u32, h: u32| {
        let mut out = Pixmap::new(w.max(1), h.max(1))?;
        out.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_scale(w.max(1) as f32 / src.width() as f32, h.max(1) as f32 / src.height() as f32), None);
        Some(out)
    };
    while img.width().max(img.height()) > side * 2 {
        match step(&img, img.width() / 2, img.height() / 2) {
            Some(n) => img = n,
            None => break,
        }
    }
    let k = (side as f32 / img.width().max(img.height()) as f32).min(1.0);
    if k < 1.0 {
        if let Some(n) = step(&img, (img.width() as f32 * k).round() as u32, (img.height() as f32 * k).round() as u32) {
            img = n;
        }
    }
    img
}

/// Fila de miniaturas para as threads (a mais nova primeiro: o que está na tela).
#[derive(Default)]
struct Fila {
    q: Mutex<VecDeque<PathBuf>>,
    cv: Condvar,
}

struct Thumbs {
    fila: Arc<Fila>,
    rx: Receiver<(PathBuf, Option<Pixmap>)>,
    ready: HashMap<PathBuf, Option<Pixmap>>,
    order: VecDeque<PathBuf>,
    asked: HashSet<PathBuf>,
}

impl Thumbs {
    fn new() -> Self {
        let fila = Arc::new(Fila::default());
        let (tx, rx): (Sender<(PathBuf, Option<Pixmap>)>, _) = channel();
        for _ in 0..2 {
            let (f, tx) = (fila.clone(), tx.clone());
            std::thread::spawn(move || loop {
                let p = {
                    let mut q = f.q.lock().unwrap();
                    while q.is_empty() {
                        q = f.cv.wait(q).unwrap();
                    }
                    q.pop_front().unwrap()
                };
                let img = glove_ui::image::decode(&p).ok().map(|i| shrink(i, THUMB));
                if tx.send((p, img)).is_err() {
                    return;
                }
            });
        }
        Self { fila, rx, ready: HashMap::new(), order: VecDeque::new(), asked: HashSet::new() }
    }

    /// Recebe as prontas. Verdadeiro se ainda há pedidos andando.
    fn poll(&mut self) -> bool {
        while let Ok((p, img)) = self.rx.try_recv() {
            self.asked.remove(&p);
            self.order.push_back(p.clone());
            self.ready.insert(p, img);
        }
        while self.order.len() > KEEP {
            if let Some(old) = self.order.pop_front() {
                self.ready.remove(&old);
            }
        }
        !self.asked.is_empty()
    }

    fn get(&mut self, p: &Path) -> Option<&Pixmap> {
        if !self.ready.contains_key(p) && self.asked.insert(p.to_path_buf()) {
            self.fila.q.lock().unwrap().push_front(p.to_path_buf());
            self.fila.cv.notify_one();
        }
        self.ready.get(p).and_then(|o| o.as_ref())
    }

    /// Esquece os pedidos que ainda não começaram (rolou para longe).
    fn forget_pending(&mut self) {
        let mut q = self.fila.q.lock().unwrap();
        for p in q.drain(..) {
            self.asked.remove(&p);
        }
    }
}

fn local(t: SystemTime) -> (i32, u32) {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    (tm.tm_year + 1900, tm.tm_mon as u32 + 1)
}

fn month_title(y: i32, m: u32) -> String {
    let meses = [tr!("janeiro"), tr!("fevereiro"), tr!("março"), tr!("abril"), tr!("maio"), tr!("junho"), tr!("julho"), tr!("agosto"), tr!("setembro"), tr!("outubro"), tr!("novembro"), tr!("dezembro")];
    let t = tr!("{0} de {1}", meses[(m as usize + 11) % 12], y);
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Desenha `img` dentro de `r`, sem distorcer (centrado).
fn draw_fit(ui: &mut Ui, img: &Pixmap, r: Rect, shadow: bool) {
    let s = ui.c.scale;
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let k = (r.w * s / iw).min(r.h * s / ih);
    let (dw, dh) = (iw * k / s, ih * k / s);
    let (dx, dy) = ((r.x + (r.w - dw) / 2.0).round(), (r.y + (r.h - dh) / 2.0).round());
    if shadow {
        ui.c.fill_rect(dx + 1.0, dy + 2.0, dw, dh, &solid(rgba(0, 0, 0, 70)));
    }
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    ui.c.draw_pixmap_t(img.as_ref(), &paint, Transform::from_row(k, 0.0, 0.0, k, (dx * s).round(), (dy * s).round()));
    if shadow {
        ui.c.stroke(rrect(dx - 0.5, dy - 0.5, dw + 1.0, dh + 1.0, 0.0), &solid(rgba(255, 255, 255, 200)), 1.0);
    }
}

struct Fotos {
    root: PathBuf,
    all: Vec<Foto>,
    scanning: Option<Receiver<Vec<Foto>>>,
    album: Option<String>,
    sel: Option<usize>,
    size: f32,
    scroll: f32,
    thumbs: Thumbs,
    /// Foto aberta grande: índice (na lista filtrada), a imagem e o pedido.
    big: Option<(usize, Option<Pixmap>, Option<Receiver<Option<Pixmap>>>)>,
    slideshow: Option<Instant>,
}

impl Fotos {
    fn new() -> Self {
        let mut f = Self { root: pictures(), all: Vec::new(), scanning: None, album: None, sel: None, size: 120.0, scroll: 0.0, thumbs: Thumbs::new(), big: None, slideshow: None };
        f.rescan();
        f
    }

    fn rescan(&mut self) {
        let (tx, rx) = channel();
        let root = self.root.clone();
        std::thread::spawn(move || {
            let _ = tx.send(scan(&root));
        });
        self.scanning = Some(rx);
    }

    fn shown(&self) -> Vec<&Foto> {
        self.all.iter().filter(|f| self.album.as_ref().is_none_or(|a| &f.album == a)).collect()
    }

    fn open_big(&mut self, i: usize) {
        let Some(p) = self.shown().get(i).map(|f| f.path.clone()) else { return };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(glove_ui::image::decode(&p).ok().map(|i| shrink(i, 2560)));
        });
        self.big = Some((i, None, Some(rx)));
        self.sel = Some(i);
    }

    fn step_big(&mut self, d: i32) {
        let n = self.shown().len() as i32;
        if let Some((i, ..)) = &self.big {
            if n > 0 {
                let j = (*i as i32 + d).rem_euclid(n) as usize;
                self.open_big(j);
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let h = ui.c.h;
        ui.c.fill_rect(0.0, 0.0, SIDE_W, h, &solid(rgba(222, 228, 236, 255)));
        ui.c.vline(SIDE_W - 1.0, 0.0, h, rgba(150, 156, 166, 255));
        let mut albums: Vec<(String, usize)> = Vec::new();
        for f in &self.all {
            if f.album.is_empty() {
                continue;
            }
            match albums.iter_mut().find(|(a, _)| *a == f.album) {
                Some(x) => x.1 += 1,
                None => albums.push((f.album.clone(), 1)),
            }
        }
        albums.sort_by_key(|a| a.0.to_lowercase());
        let mut y = 10.0;
        let head = |ui: &mut Ui, t: &str, y: f32| ui.fonts.draw(ui.c, t, 12.0, y + 12.0, 10.5, true, rgba(110, 116, 128, 255));
        head(ui, &tr!("BIBLIOTECA"), y);
        y += 20.0;
        let mut pick = None;
        let row = |ui: &mut Ui, label: &str, n: usize, icon: Icon, on: bool, y: f32| {
            let r = Rect::new(0.0, y, SIDE_W - 1.0, 22.0);
            if on {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &vgrad(r.y, r.y + r.h, &[(0.0, rgba(110, 160, 230, 255)), (1.0, rgba(60, 115, 200, 255))]));
            }
            glove_ui::paint::icons::draw(ui.c, ui.fonts, icon, 12.0, y + 3.0, 16.0, None);
            let col = if on { rgba(255, 255, 255, 255) } else { rgba(30, 30, 34, 255) };
            let t = ui.fonts.ellipsize(label, 12.0, on, SIDE_W - 80.0);
            ui.fonts.draw(ui.c, &t, 34.0, y + 15.5, 12.0, on, col);
            let c = n.to_string();
            let cw = ui.text_width(&c, 10.5, false);
            ui.fonts.draw(ui.c, &c, SIDE_W - 14.0 - cw, y + 15.0, 10.5, false, if on { col } else { rgba(120, 120, 126, 255) });
            ui.hit(r).clicked
        };
        if row(ui, &tr!("Fotos"), self.all.len(), Icon::Photos, self.album.is_none(), y) {
            pick = Some(None);
        }
        y += 24.0;
        if !albums.is_empty() {
            y += 8.0;
            head(ui, &tr!("ÁLBUNS"), y);
            y += 20.0;
            for (a, n) in &albums {
                if row(ui, a, *n, Icon::Folder, self.album.as_ref() == Some(a), y) {
                    pick = Some(Some(a.clone()));
                }
                y += 24.0;
            }
        }
        if let Some(p) = pick {
            if self.album != p {
                self.album = p;
                self.scroll = 0.0;
                self.sel = None;
                self.thumbs.forget_pending();
            }
        }
    }

    fn grid(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(SIDE_W, 0.0, w - SIDE_W, h - BOTTOM_H);
        ui.c.fill_rect(area.x, area.y, area.w, area.h, &solid(rgba(250, 250, 250, 255)));
        let shown: Vec<Foto> = self.shown().into_iter().cloned().collect();
        if shown.is_empty() {
            let t = if self.scanning.is_some() { tr!("Procurando fotos…").to_string() } else { tr!("Nenhuma foto em {}.", self.root.display()) };
            let tw = ui.text_width(&t, 13.0, false);
            ui.fonts.draw(ui.c, &t, area.x + (area.w - tw) / 2.0, area.h / 2.0, 13.0, false, rgba(120, 120, 126, 255));
            return;
        }
        let cell = self.size + 14.0;
        let cols = ((area.w - 20.0) / cell).floor().max(1.0) as usize;
        let head_h = 34.0;
        // Posições: cabeçalho por mês e as fotos em linhas.
        let mut items: Vec<(f32, Option<usize>, String)> = Vec::new();
        let mut y = 8.0;
        let mut col = 0;
        let mut last = None;
        for (i, f) in shown.iter().enumerate() {
            let m = local(f.time);
            if last != Some(m) {
                if col != 0 {
                    y += cell;
                    col = 0;
                }
                items.push((y, None, month_title(m.0, m.1)));
                y += head_h;
                last = Some(m);
            }
            items.push((y, Some(i), String::new()));
            col += 1;
            if col == cols {
                col = 0;
                y += cell;
            }
        }
        let total = y + if col != 0 { cell } else { 0.0 } + 10.0;
        let max_scroll = (total - area.h).max(0.0);
        let wheel = ui.scroll_in(area);
        if wheel != 0.0 {
            self.scroll = (self.scroll + wheel * 3.0).clamp(0.0, max_scroll);
            self.thumbs.forget_pending();
        }
        self.scroll = self.scroll.min(max_scroll);
        let old = ui.c.clip();
        let s = ui.c.scale;
        ui.c.set_clip(Some([(area.x * s) as i32, (area.y * s) as i32, ((area.x + area.w) * s) as i32, ((area.y + area.h) * s) as i32]));
        let mut col_i = 0;
        let mut open = None;
        for (iy, it, title) in &items {
            let yy = area.y + iy - self.scroll;
            match it {
                None => {
                    col_i = 0;
                    if yy > -head_h && yy < area.h {
                        ui.fonts.draw(ui.c, title, area.x + 16.0, yy + 22.0, 14.0, true, rgba(40, 40, 46, 255));
                    }
                }
                Some(i) => {
                    let x = area.x + 10.0 + (col_i % cols) as f32 * cell;
                    col_i += 1;
                    if yy + cell < area.y || yy > area.y + area.h {
                        continue;
                    }
                    let r = Rect::new(x + 7.0, yy + 7.0, self.size, self.size);
                    if self.sel == Some(*i) {
                        ui.c.fill(rrect(x + 2.0, yy + 2.0, cell - 4.0, cell - 4.0, 6.0), &solid(rgba(60, 120, 220, 90)));
                    }
                    let path = shown[*i].path.clone();
                    if let Some(img) = self.thumbs.get(&path) {
                        let img = img.clone();
                        draw_fit(ui, &img, r, true);
                    } else {
                        ui.c.fill(rrect(r.x + r.w * 0.2, r.y + r.h * 0.2, r.w * 0.6, r.h * 0.6, 4.0), &solid(rgba(0, 0, 0, 18)));
                    }
                    let h = ui.hit(Rect::new(x, yy, cell, cell));
                    if h.pressed {
                        self.sel = Some(*i);
                    }
                    if h.double {
                        open = Some(*i);
                    }
                }
            }
        }
        ui.c.set_clip(old);
        if let Some(i) = open {
            self.open_big(i);
        }
    }

    fn bottom(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let y = h - BOTTOM_H;
        ui.c.fill_rect(SIDE_W, y, w - SIDE_W, BOTTOM_H, &vgrad(y, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(208, 208, 212, 255))]));
        ui.c.hline(SIDE_W, y, w - SIDE_W, rgba(150, 150, 156, 255));
        let n = self.shown().len();
        let t = tr!("{} fotos", n);
        ui.fonts.draw(ui.c, &t, SIDE_W + 14.0, y + 21.0, 11.5, false, rgba(70, 70, 76, 255));
        let mut size = self.size;
        ui.area(Rect::new(w - 190.0, y + 6.0, 180.0, 22.0), |ui| {
            if ui.slider(&mut size, 70.0, 230.0, 170.0) {}
        });
        if (size - self.size).abs() > 0.5 {
            self.size = size;
        }
    }

    fn big_view(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(24, 24, 26, 255)));
        let Some((i, img, rx)) = &mut self.big else { return };
        if let Some(r) = rx {
            match r.try_recv() {
                Ok(v) => {
                    *img = v;
                    *rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(40)),
                Err(_) => *rx = None,
            }
        }
        let i = *i;
        let bar = 40.0;
        if let Some(p) = img.clone() {
            draw_fit(ui, &p, Rect::new(20.0, 20.0, w - 40.0, h - 40.0 - bar), false);
        } else {
            ui.spinner(24.0);
        }
        let shown: Vec<Foto> = self.shown().into_iter().cloned().collect();
        let name = shown.get(i).map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).unwrap_or_default();
        let info = format!("{name}  —  {} / {}", i + 1, shown.len());
        let tw = ui.text_width(&info, 11.5, false);
        ui.fonts.draw(ui.c, &info, (w - tw) / 2.0, h - bar + 4.0, 11.5, false, rgba(200, 200, 206, 255));
        let mut act = None;
        let playing = self.slideshow.is_some();
        ui.area(Rect::new(12.0, h - bar + 8.0, w - 24.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                if ui.button(tr!("‹ Biblioteca")) {
                    act = Some("back");
                }
                if ui.button("◀") {
                    act = Some("prev");
                }
                if ui.button("▶") {
                    act = Some("next");
                }
                if ui.button(if playing { tr!("Parar Apresentação") } else { tr!("Apresentação") }) {
                    act = Some("slide");
                }
                if ui.button(tr!("Abrir no Visualizador")) {
                    act = Some("viewer");
                }
            });
        });
        let path = shown.get(i).map(|f| f.path.clone());
        match act {
            Some("back") => {
                self.big = None;
                self.slideshow = None;
            }
            Some("prev") => self.step_big(-1),
            Some("next") => self.step_big(1),
            Some("slide") => self.slideshow = if playing { None } else { Some(Instant::now()) },
            Some("viewer") => {
                if let Some(p) = path {
                    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("glove-view"))).filter(|e| e.exists()).unwrap_or_else(|| "glove-view".into());
                    let _ = std::process::Command::new(exe).arg(p).spawn();
                }
            }
            _ => {}
        }
    }
}

impl App for Fotos {
    fn tick(&self) -> Option<Duration> {
        self.slideshow.map(|_| Duration::from_millis(250))
    }

    fn on_tick(&mut self) {
        if let Some(t) = self.slideshow {
            if t.elapsed() >= SLIDE {
                self.step_big(1);
                self.slideshow = Some(Instant::now());
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        if let Some(rx) = &self.scanning {
            match rx.try_recv() {
                Ok(v) => {
                    self.all = v;
                    self.scanning = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(100)),
                Err(_) => self.scanning = None,
            }
        }
        if self.thumbs.poll() {
            ui.wake_after(Duration::from_millis(50));
        }
        let n = self.shown().len();
        for k in ui.keys().to_vec() {
            use glove_ui::ui::Key;
            match (k.key, k.combo.as_str()) {
                (Key::Escape, _) if self.big.is_some() => {
                    self.big = None;
                    self.slideshow = None;
                }
                (_, "space") if self.big.is_some() => self.slideshow = if self.slideshow.is_some() { None } else { Some(Instant::now()) },
                (Key::Right, _) if self.big.is_some() => self.step_big(1),
                (Key::Left, _) if self.big.is_some() => self.step_big(-1),
                (Key::Enter, _) | (_, "space") if self.big.is_none() => {
                    if let Some(i) = self.sel {
                        self.open_big(i);
                    }
                }
                (Key::Right, _) if n > 0 => self.sel = Some(self.sel.map_or(0, |i| (i + 1).min(n - 1))),
                (Key::Left, _) if n > 0 => self.sel = Some(self.sel.map_or(0, |i| i.saturating_sub(1))),
                _ => {}
            }
        }
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "rescan" => self.rescan(),
                "files" => {
                    let target = self.album.as_ref().map(|a| self.root.join(a)).unwrap_or_else(|| self.root.clone());
                    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("glove-files"))).filter(|e| e.exists()).unwrap_or_else(|| "glove-files".into());
                    let _ = std::process::Command::new(exe).arg(target).spawn();
                }
                _ => {}
            }
        }
        if self.big.is_some() {
            self.big_view(ui);
            return;
        }
        self.sidebar(ui);
        self.grid(ui);
        self.bottom(ui);
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(tr!("Fotos"), vec![MenuItem::new("rescan", tr!("Procurar Fotos Novas"), "Ctrl+R"), MenuItem::new("files", tr!("Mostrar no Arquivos"), "")])]
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Fotos").into(), app_id: "glove.Fotos".into(), size: (900, 600), fixed: false, min_size: Some((600, 400)) };
    if let Err(e) = run(opts, Fotos::new()) {
        eprintln!("glove-fotos: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mes() {
        assert!(!super::month_title(2026, 11).is_empty());
        assert!(super::month_title(2026, 11).contains("2026"));
    }
}
