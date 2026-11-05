//! Música: a biblioteca da pasta Música e um tocador leve (como o iTunes da
//! era, só o essencial). Tudo em Rust: o symphonia decodifica, o pw-cat toca.

mod player;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, Color},
    run,
    ui::Key,
    App, Column, Glyph, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ticks, Ui, WindowOptions, SEARCH_H,
};
use player::{Cmd, Info, Status};

const TOP_H: f32 = 74.0;
const EXTS: [&str; 7] = ["mp3", "flac", "ogg", "oga", "wav", "m4a", "aac"];

/// Pasta Música (XDG), ou a pedida na linha de comando.
fn music_dir() -> PathBuf {
    if let Some(a) = std::env::args_os().nth(1) {
        return PathBuf::from(a);
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let xdg = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok().and_then(|t| {
        t.lines().find_map(|l| l.strip_prefix("XDG_MUSIC_DIR=")).map(|v| PathBuf::from(v.trim_matches('"').replace("$HOME", &home.to_string_lossy())))
    });
    xdg.filter(|p| p.is_dir()).or_else(|| ["Música", "Music"].iter().map(|d| home.join(d)).find(|p| p.is_dir())).unwrap_or(home.join("Música"))
}

fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                if !e.file_name().to_string_lossy().starts_with('.') {
                    stack.push(p);
                }
            } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| EXTS.contains(&x.to_ascii_lowercase().as_str())) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// "3:07" ou "1:02:30".
pub fn mmss(s: f32) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

struct Song {
    path: PathBuf,
    info: Info,
}

struct Musica {
    dir: PathBuf,
    /// Biblioteca: as tags chegam de uma thread.
    songs: Arc<Mutex<Vec<Song>>>,
    scanning: Arc<Mutex<bool>>,
    table: TableState,
    selected: Option<PathBuf>,
    filter: String,
    status: Arc<Mutex<Status>>,
    tx: std::sync::mpsc::Sender<Cmd>,
    volume: f32,
    /// Ordem de tocar (a da tabela na hora do play).
    queue: Vec<PathBuf>,
}

impl Musica {
    fn new() -> Self {
        let dir = music_dir();
        let songs: Arc<Mutex<Vec<Song>>> = Arc::default();
        let scanning = Arc::new(Mutex::new(true));
        {
            let (songs, scanning, dir) = (songs.clone(), scanning.clone(), dir.clone());
            std::thread::spawn(move || {
                for p in scan(&dir) {
                    let info = player::read_info(&p);
                    songs.lock().unwrap().push(Song { path: p, info });
                }
                *scanning.lock().unwrap() = false;
            });
        }
        let status: Arc<Mutex<Status>> = Arc::default();
        let tx = player::spawn(status.clone());
        Self { dir, songs, scanning, table: TableState::sorted_by(1, true), selected: None, filter: String::new(), status, tx, volume: 0.8, queue: Vec::new() }
    }

    /// Linhas visíveis (filtro e ordem da tabela): (caminho, linha).
    fn rows(&self) -> Vec<(PathBuf, Info)> {
        let f = self.filter.trim().to_lowercase();
        let songs = self.songs.lock().unwrap();
        let mut v: Vec<(PathBuf, Info)> = songs
            .iter()
            .filter(|s| f.is_empty() || [&s.info.title, &s.info.artist, &s.info.album].iter().any(|t| t.to_lowercase().contains(&f)))
            .map(|s| (s.path.clone(), s.info.clone()))
            .collect();
        let (col, asc) = self.table.sort.unwrap_or((1, true));
        v.sort_by(|a, b| {
            let o = match col {
                0 => a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase()),
                2 => a.1.album.to_lowercase().cmp(&b.1.album.to_lowercase()),
                3 => a.1.secs.partial_cmp(&b.1.secs).unwrap_or(std::cmp::Ordering::Equal),
                _ => a.1.artist.to_lowercase().cmp(&b.1.artist.to_lowercase()).then(a.1.album.to_lowercase().cmp(&b.1.album.to_lowercase())).then(a.0.cmp(&b.0)),
            };
            if asc { o } else { o.reverse() }
        });
        v
    }

    fn play(&mut self, p: PathBuf, rows: &[(PathBuf, Info)]) {
        self.queue = rows.iter().map(|r| r.0.clone()).collect();
        self.selected = Some(p.clone());
        let _ = self.tx.send(Cmd::Play(p));
    }

    fn step(&mut self, delta: isize) {
        let cur = self.status.lock().unwrap().playing.clone();
        let Some(cur) = cur.or_else(|| self.selected.clone()) else { return };
        if self.queue.is_empty() {
            self.queue = self.rows().into_iter().map(|r| r.0).collect();
        }
        let Some(i) = self.queue.iter().position(|p| *p == cur) else { return };
        let j = i as isize + delta;
        if j >= 0 && (j as usize) < self.queue.len() {
            let p = self.queue[j as usize].clone();
            self.selected = Some(p.clone());
            let _ = self.tx.send(Cmd::Play(p));
        } else {
            let _ = self.tx.send(Cmd::Stop);
        }
    }

    fn toggle(&mut self, rows: &[(PathBuf, Info)]) {
        let (playing, paused) = {
            let s = self.status.lock().unwrap();
            (s.playing.is_some(), s.paused)
        };
        if playing {
            let _ = self.tx.send(if paused { Cmd::Resume } else { Cmd::Pause });
        } else if let Some(p) = self.selected.clone().or_else(|| rows.first().map(|r| r.0.clone())) {
            self.play(p, rows);
        }
    }

    /// Botão redondo de gel com um desenho (◀◀, ▶, ❚❚, ▶▶).
    fn round(ui: &mut Ui, cx: f32, cy: f32, r: f32, draw: impl Fn(&mut glove_kit::paint::draw::Canvas, f32, f32, Color)) -> bool {
        let hit = ui.hit(Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r));
        let base = if hit.down { rgba(190, 190, 196, 255) } else { rgba(244, 244, 246, 255) };
        ui.c.fill(circle(cx, cy + 1.0, r), &solid(rgba(0, 0, 0, 50)));
        gel_circle(ui.c, cx, cy, r, base);
        ui.c.stroke(circle(cx, cy, r), &solid(rgba(110, 110, 118, 255)), 1.0);
        draw(ui.c, cx, cy, rgba(40, 40, 46, 255));
        hit.clicked
    }

    /// Visor da era: título, artista e álbum, e a barra do tempo (clique pula).
    fn display(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill(rrect(r.x, r.y, r.w, r.h, 8.0), &vgrad(r.y, r.y + r.h, &[(0.0, rgba(236, 240, 222, 255)), (1.0, rgba(214, 222, 196, 255))]));
        ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 8.0), &solid(rgba(110, 116, 100, 255)), 1.0);
        let (playing, pos, secs) = {
            let s = self.status.lock().unwrap();
            (s.playing.clone(), s.pos, s.secs)
        };
        let ink = rgba(40, 44, 36, 255);
        let center = |ui: &mut Ui, t: &str, y: f32, px: f32, bold: bool| {
            let t = ui.fonts.ellipsize(t, px, bold, r.w - 24.0);
            let tw = ui.text_width(&t, px, bold);
            ui.fonts.draw(ui.c, &t, r.x + (r.w - tw) / 2.0, y, px, bold, ink);
        };
        match playing {
            None => center(ui, "Glove Música", r.y + r.h / 2.0 + 5.0, 13.0, true),
            Some(p) => {
                let info = self.songs.lock().unwrap().iter().find(|s| s.path == p).map(|s| s.info.clone()).unwrap_or_default();
                center(ui, &info.title, r.y + 16.0, 12.0, true);
                let sub = [info.artist.as_str(), info.album.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" — ");
                center(ui, &sub, r.y + 31.0, 11.0, false);
                // Tempo.
                let bx = r.x + 46.0;
                let bw = r.w - 92.0;
                let by = r.y + r.h - 15.0;
                ui.fonts.draw(ui.c, &mmss(pos), r.x + 8.0, by + 5.0, 10.0, false, ink);
                let left = format!("-{}", mmss(secs - pos));
                let lw = ui.text_width(&left, 10.0, false);
                ui.fonts.draw(ui.c, &left, r.x + r.w - 8.0 - lw, by + 5.0, 10.0, false, ink);
                ui.c.fill(rrect(bx, by - 3.0, bw, 7.0, 3.5), &solid(rgba(255, 255, 255, 160)));
                ui.c.stroke(rrect(bx + 0.5, by - 2.5, bw - 1.0, 6.0, 3.0), &solid(rgba(90, 96, 80, 255)), 1.0);
                if secs > 0.0 {
                    let f = (pos / secs).clamp(0.0, 1.0);
                    ui.c.fill(rrect(bx, by - 3.0, (bw * f).max(7.0), 7.0, 3.5), &solid(rgba(70, 76, 60, 255)));
                    let hit = ui.hit(Rect::new(bx, by - 6.0, bw, 13.0));
                    if hit.pressed {
                        let t = ((ui.pointer().0 - bx) / bw).clamp(0.0, 1.0) * secs;
                        let _ = self.tx.send(Cmd::Seek(t));
                    }
                }
            }
        }
    }
}

impl App for Musica {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        // A música acabou: a próxima.
        let ended = std::mem::take(&mut self.status.lock().unwrap().ended);
        if ended {
            self.step(1);
        }
        let rows = self.rows();
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match (k.key, k.combo.as_str()) {
                (_, "Ctrl+Right") => acts.push("next".into()),
                (_, "Ctrl+Left") => acts.push("prev".into()),
                (_, "Ctrl+W") => acts.push("close".into()),
                (Key::Enter, _) => acts.push("play-sel".into()),
                _ if k.combo == "space" || k.combo == "Space" => acts.push("toggle".into()),
                _ => {}
            }
        }
        // Barra de cima em metal: tocador, volume, visor e busca.
        ui.c.brushed_metal(0.0, 0.0, w, TOP_H, rgba(226, 226, 228, 255), rgba(184, 184, 188, 255));
        ui.c.hline(0.0, TOP_H - 1.0, w, rgba(120, 120, 126, 255));
        let cy = 30.0;
        let tri = |c: &mut glove_kit::paint::draw::Canvas, x: f32, y: f32, s: f32, dir: f32, col: Color| {
            let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
            pb.move_to(x - s * 0.5 * dir, y - s * 0.6);
            pb.line_to(x + s * 0.6 * dir, y);
            pb.line_to(x - s * 0.5 * dir, y + s * 0.6);
            pb.close();
            c.fill(pb.finish(), &solid(col));
        };
        if Self::round(ui, 26.0, cy, 15.0, |c, x, y, col| {
            tri(c, x - 3.0, y, 8.0, -1.0, col);
            tri(c, x + 4.0, y, 8.0, -1.0, col);
        }) {
            acts.push("prev".into());
        }
        let (playing, paused) = {
            let s = self.status.lock().unwrap();
            (s.playing.is_some(), s.paused)
        };
        if Self::round(ui, 64.0, cy, 19.0, |c, x, y, col| {
            if playing && !paused {
                c.fill_rect(x - 6.0, y - 7.0, 4.0, 14.0, &solid(col));
                c.fill_rect(x + 2.0, y - 7.0, 4.0, 14.0, &solid(col));
            } else {
                tri(c, x + 1.0, y, 12.0, 1.0, col);
            }
        }) {
            acts.push("toggle".into());
        }
        if Self::round(ui, 102.0, cy, 15.0, |c, x, y, col| {
            tri(c, x - 3.0, y, 8.0, 1.0, col);
            tri(c, x + 4.0, y, 8.0, 1.0, col);
        }) {
            acts.push("next".into());
        }
        // Volume.
        let mut vol = self.volume;
        ui.area(Rect::new(128.0, cy - 9.0, 110.0, 24.0), |ui| {
            if ui.slider_ticks(&mut vol, 0.0, 1.0, 100.0, &Ticks { count: 0, labels: &[], snap: false }, true) {
                self.volume = vol;
                let _ = self.tx.send(Cmd::Volume(vol));
            }
        });
        // Visor.
        let dw = (w - 250.0 - 190.0).max(200.0);
        self.display(ui, Rect::new(250.0, 8.0, dw, TOP_H - 18.0));
        // Busca.
        let sw = (w - 250.0 - dw - 24.0).max(120.0);
        ui.search_field_in(Rect::new(w - sw - 12.0, cy - SEARCH_H / 2.0, sw, SEARCH_H), &mut self.filter, "Buscar");

        // Biblioteca.
        let cols = [Column::new("Nome", 0.36), Column::new("Artista", 0.24), Column::new("Álbum", 0.28), Column::new("Duração", 0.12).right()];
        let cur = self.status.lock().unwrap().playing.clone();
        let trows: Vec<TableRow> = rows
            .iter()
            .map(|(p, i)| TableRow {
                cells: vec![format!("{}{}", if Some(p) == cur.as_ref() { "♪ " } else { "" }, i.title), i.artist.clone(), i.album.clone(), mmss(i.secs)],
                ..TableRow::default()
            })
            .collect();
        let sel = self.selected.as_ref().and_then(|s| rows.iter().position(|r| &r.0 == s));
        let area = Rect::new(0.0, TOP_H, w, h - TOP_H - 22.0);
        let resp = ui.table(area, &cols, &trows, sel, &mut self.table, &TableStyle::default());
        if let Some(i) = resp.pressed {
            self.selected = Some(rows[i].0.clone());
        }
        if let Some(i) = resp.double {
            self.play(rows[i].0.clone(), &rows);
        }
        if rows.is_empty() {
            let msg = if *self.scanning.lock().unwrap() { "Lendo a biblioteca…".to_string() } else { format!("Nenhuma música em {}. Ponha MP3, FLAC, OGG ou WAV nessa pasta.", self.dir.display()) };
            let msg = ui.fonts.ellipsize(&msg, 12.0, false, w - 40.0);
            let tw = ui.text_width(&msg, 12.0, false);
            ui.fonts.draw(ui.c, &msg, (w - tw) / 2.0, area.y + area.h / 2.0, 12.0, false, rgba(130, 130, 136, 255));
        }
        // Rodapé: quantas e quanto tempo.
        let fy = h - 22.0;
        ui.c.fill_rect(0.0, fy, w, 22.0, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        let total: f32 = rows.iter().map(|r| r.1.secs).sum();
        let t = format!("{} músicas, {}", rows.len(), mmss(total));
        let tw = ui.text_width(&t, 11.0, false);
        ui.fonts.draw(ui.c, &t, (w - tw) / 2.0, fy + 15.0, 11.0, false, rgba(60, 60, 66, 255));
        if let Some(e) = self.status.lock().unwrap().error.clone() {
            ui.fonts.draw(ui.c, &format!("Não deu para tocar: {e}"), 10.0, fy + 15.0, 10.0, false, rgba(180, 40, 30, 255));
        }

        for a in acts {
            match a.as_str() {
                "toggle" => self.toggle(&rows),
                "next" => self.step(1),
                "prev" => {
                    // Mais de 3 s tocados: volta ao começo; senão, a anterior.
                    let pos = self.status.lock().unwrap().pos;
                    if pos > 3.0 { let _ = self.tx.send(Cmd::Seek(0.0)); } else { self.step(-1) }
                }
                "stop" => {
                    let _ = self.tx.send(Cmd::Stop);
                }
                "play-sel" => {
                    if let Some(p) = self.selected.clone() {
                        self.play(p, &rows);
                    }
                }
                "close" => ui.close = true,
                _ => {}
            }
        }
        // Enquanto toca (ou lê a biblioteca), o visor anda.
        if playing || *self.scanning.lock().unwrap() {
            ui.wake_after(Duration::from_millis(500));
        }
        let _ = Glyph::Back;
    }

    fn menus(&self) -> Vec<Menu> {
        let (playing, paused) = {
            let s = self.status.lock().unwrap();
            (s.playing.is_some(), s.paused)
        };
        vec![Menu::new(
            "Controles",
            vec![
                MenuItem::new("toggle", if playing && !paused { "Pausar" } else { "Tocar" }, "space"),
                MenuItem::new("stop", "Parar", "").enabled(playing),
                MenuItem::Separator,
                MenuItem::new("next", "Próxima Música", "Ctrl+Right"),
                MenuItem::new("prev", "Música Anterior", "Ctrl+Left"),
            ],
        )]
    }

    fn close_requested(&mut self) -> bool {
        let _ = self.tx.send(Cmd::Stop);
        true
    }
}

fn main() {
    let opts = WindowOptions { title: "Música".into(), app_id: "glove.Musica".into(), size: (760, 480), fixed: false };
    if let Err(e) = run(opts, Musica::new()) {
        eprintln!("glove-musica: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tempos() {
        assert_eq!(super::mmss(187.4), "3:07");
        assert_eq!(super::mmss(3750.0), "1:02:30");
    }
}
