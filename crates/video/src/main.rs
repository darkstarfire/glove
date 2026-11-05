//! Vídeo: o mpv mostra a imagem (na janela dele, sem os controles dele) e
//! esta janela é o painel de controle da era: tocar/pausar, a barra do
//! tempo, o volume e a tela cheia. Fala com o mpv pelo soquete JSON dele.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, Color},
    run, AlertKind, App, Choice, FileBrowser, Menu, MenuItem, Rect, Ticks, Ui, WindowOptions,
};

const EXTS: [&str; 10] = ["mp4", "mkv", "webm", "avi", "mov", "m4v", "ogv", "mpg", "mpeg", "wmv"];

#[derive(Default)]
struct State {
    pos: f64,
    dur: f64,
    paused: bool,
    title: String,
    ended: bool,
}

/// Valor de `"chave":` numa linha JSON simples do mpv (número, bool ou texto).
pub fn json_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\":");
    let rest = &line[line.find(&pat)? + pat.len()..];
    let rest = rest.trim_start();
    if let Some(s) = rest.strip_prefix('"') {
        return s.find('"').map(|e| &s[..e]);
    }
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn mmss(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

fn has_mpv() -> bool {
    Command::new("mpv").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

struct Player {
    child: Child,
    sock: UnixStream,
}

impl Player {
    fn start(file: &PathBuf, state: Arc<Mutex<State>>, volume: f32) -> Option<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("glove-video-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let child = Command::new("mpv")
            .arg(format!("--input-ipc-server={}", path.display()))
            .args(["--osc=no", "--keep-open=yes", "--force-window=yes", "--wayland-app-id=glove.VideoTela", "--title=Vídeo"])
            .arg(format!("--volume={}", (volume * 100.0) as i32))
            .arg(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        // O soquete aparece logo depois que o mpv sobe.
        let mut sock = None;
        for _ in 0..50 {
            if let Ok(s) = UnixStream::connect(&path) {
                sock = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(60));
        }
        let mut sock = sock?;
        for (i, p) in ["time-pos", "duration", "pause", "media-title", "eof-reached"].iter().enumerate() {
            let _ = writeln!(sock, "{{\"command\":[\"observe_property\",{},\"{p}\"]}}", i + 1);
        }
        let reader = sock.try_clone().ok()?;
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if json_field(&line, "event") != Some("property-change") {
                    continue;
                }
                let data = json_field(&line, "data").unwrap_or("");
                let mut s = state.lock().unwrap();
                match json_field(&line, "name") {
                    Some("time-pos") => s.pos = data.parse().unwrap_or(s.pos),
                    Some("duration") => s.dur = data.parse().unwrap_or(0.0),
                    Some("pause") => s.paused = data == "true",
                    Some("media-title") => s.title = data.to_string(),
                    Some("eof-reached") => s.ended = data == "true",
                    _ => {}
                }
            }
        });
        Some(Self { child, sock })
    }

    fn cmd(&mut self, json: &str) {
        let _ = writeln!(self.sock, "{{\"command\":{json}}}");
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.cmd("[\"quit\"]");
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Video {
    mpv: bool,
    player: Option<Player>,
    state: Arc<Mutex<State>>,
    file: Option<PathBuf>,
    volume: f32,
    open_fb: Option<FileBrowser>,
    error: Option<String>,
}

impl Video {
    fn load(&mut self, p: PathBuf) {
        self.player = None;
        *self.state.lock().unwrap() = State::default();
        self.error = None;
        if !self.mpv {
            return;
        }
        self.player = Player::start(&p, self.state.clone(), self.volume);
        if self.player.is_none() {
            self.error = Some("O mpv não abriu esse arquivo.".into());
        }
        self.file = Some(p);
    }

    fn round(ui: &mut Ui, cx: f32, cy: f32, r: f32, draw: impl Fn(&mut Canvas, f32, f32, Color)) -> bool {
        let hit = ui.hit(Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r));
        let base = if hit.down { rgba(190, 190, 196, 255) } else { rgba(244, 244, 246, 255) };
        ui.c.fill(circle(cx, cy + 1.0, r), &solid(rgba(0, 0, 0, 50)));
        gel_circle(ui.c, cx, cy, r, base);
        ui.c.stroke(circle(cx, cy, r), &solid(rgba(110, 110, 118, 255)), 1.0);
        draw(ui.c, cx, cy, rgba(40, 40, 46, 255));
        hit.clicked
    }
}

fn tri(c: &mut Canvas, x: f32, y: f32, s: f32, dir: f32, col: Color) {
    let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
    pb.move_to(x - s * 0.5 * dir, y - s * 0.6);
    pb.line_to(x + s * 0.6 * dir, y);
    pb.line_to(x - s * 0.5 * dir, y + s * 0.6);
    pb.close();
    c.fill(pb.finish(), &solid(col));
}

impl App for Video {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.open_fb.is_none() {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+O" => acts.push("open".into()),
                    "Ctrl+W" => acts.push("close".into()),
                    "Ctrl+F" => acts.push("full".into()),
                    "space" | "Space" => acts.push("toggle".into()),
                    "Right" => acts.push("fwd".into()),
                    "Left" => acts.push("back".into()),
                    _ => {}
                }
            }
        }
        ui.c.brushed_metal(0.0, 0.0, w, h, rgba(226, 226, 228, 255), rgba(184, 184, 188, 255));
        let (pos, dur, paused, title) = {
            let s = self.state.lock().unwrap();
            (s.pos, s.dur, s.paused, s.title.clone())
        };
        // Nome do vídeo.
        let name = if !title.is_empty() { title } else { self.file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Nenhum vídeo aberto".into()) };
        let name = ui.fonts.ellipsize(&name, 12.0, true, w - 24.0);
        let tw = ui.text_width(&name, 12.0, true);
        ui.fonts.draw(ui.c, &name, (w - tw) / 2.0, 22.0, 12.0, true, rgba(30, 30, 34, 255));
        // Barra do tempo (clique ou arraste pula).
        let (bx, by, bw) = (60.0, 44.0, w - 120.0);
        ui.fonts.draw(ui.c, &mmss(pos), 12.0, by + 4.0, 10.0, false, rgba(50, 50, 56, 255));
        let rest = format!("-{}", mmss(dur - pos));
        let rw = ui.text_width(&rest, 10.0, false);
        ui.fonts.draw(ui.c, &rest, w - 12.0 - rw, by + 4.0, 10.0, false, rgba(50, 50, 56, 255));
        ui.c.fill(rrect(bx, by - 4.0, bw, 8.0, 4.0), &vgrad(by - 4.0, by + 4.0, &[(0.0, rgba(150, 150, 156, 255)), (1.0, rgba(214, 214, 218, 255))]));
        if dur > 0.0 {
            let f = (pos / dur).clamp(0.0, 1.0) as f32;
            ui.c.fill(rrect(bx, by - 4.0, (bw * f).max(8.0), 8.0, 4.0), &vgrad(by - 4.0, by + 4.0, &[(0.0, rgba(120, 170, 230, 255)), (1.0, rgba(40, 100, 200, 255))]));
            ui.c.fill(circle(bx + bw * f, by, 6.0), &solid(rgba(250, 250, 252, 255)));
            ui.c.stroke(circle(bx + bw * f, by, 6.0), &solid(rgba(100, 100, 108, 255)), 1.0);
            let hit = ui.hit(Rect::new(bx, by - 8.0, bw, 16.0));
            if hit.pressed || hit.down {
                let t = ((ui.pointer().0 - bx) / bw).clamp(0.0, 1.0) as f64 * dur;
                if let Some(p) = &mut self.player {
                    p.cmd(&format!("[\"seek\",{t:.2},\"absolute\"]"));
                }
            }
        }
        // Botões.
        let cy = 88.0;
        let mid = w / 2.0;
        if Self::round(ui, mid - 44.0, cy, 15.0, |c, x, y, col| {
            tri(c, x - 3.0, y, 8.0, -1.0, col);
            tri(c, x + 4.0, y, 8.0, -1.0, col);
        }) {
            acts.push("back".into());
        }
        let playing = self.player.is_some() && !paused;
        if Self::round(ui, mid, cy, 20.0, |c, x, y, col| {
            if playing {
                c.fill_rect(x - 6.0, y - 7.0, 4.0, 14.0, &solid(col));
                c.fill_rect(x + 2.0, y - 7.0, 4.0, 14.0, &solid(col));
            } else {
                tri(c, x + 1.0, y, 12.0, 1.0, col);
            }
        }) {
            acts.push("toggle".into());
        }
        if Self::round(ui, mid + 44.0, cy, 15.0, |c, x, y, col| {
            tri(c, x - 3.0, y, 8.0, 1.0, col);
            tri(c, x + 4.0, y, 8.0, 1.0, col);
        }) {
            acts.push("fwd".into());
        }
        // Volume e tela cheia.
        let mut vol = self.volume;
        ui.area(Rect::new(16.0, cy - 9.0, 110.0, 24.0), |ui| {
            if ui.slider_ticks(&mut vol, 0.0, 1.0, 100.0, &Ticks { count: 0, labels: &[], snap: false }, true) {
                self.volume = vol;
                if let Some(p) = &mut self.player {
                    p.cmd(&format!("[\"set_property\",\"volume\",{}]", (vol * 100.0) as i32));
                }
            }
        });
        ui.area(Rect::new(w - 110.0, cy - 11.0, 96.0, 24.0), |ui| {
            if ui.button("Tela Cheia") {
                acts.push("full".into());
            }
        });
        if let Some(e) = &self.error {
            ui.fonts.draw(ui.c, e, 12.0, h - 8.0, 10.0, false, rgba(180, 40, 30, 255));
        }

        for a in acts {
            match a.as_str() {
                "open" => acts_open(&mut self.open_fb, &self.file),
                "toggle" => match &mut self.player {
                    Some(p) => p.cmd("[\"cycle\",\"pause\"]"),
                    None => {
                        if let Some(f) = self.file.clone() {
                            self.load(f);
                        } else {
                            acts_open(&mut self.open_fb, &self.file);
                        }
                    }
                },
                "fwd" => {
                    if let Some(p) = &mut self.player {
                        p.cmd("[\"seek\",10,\"relative\"]");
                    }
                }
                "back" => {
                    if let Some(p) = &mut self.player {
                        p.cmd("[\"seek\",-10,\"relative\"]");
                    }
                }
                "full" => {
                    if let Some(p) = &mut self.player {
                        p.cmd("[\"cycle\",\"fullscreen\"]");
                    }
                }
                "close" => ui.close = true,
                _ => {}
            }
        }
        // O mpv foi fechado pela janela dele: o controle volta ao início.
        if let Some(p) = &mut self.player {
            if matches!(p.child.try_wait(), Ok(Some(_))) {
                self.player = None;
                *self.state.lock().unwrap() = State::default();
            }
        }
        if self.player.is_some() {
            ui.wake_after(Duration::from_millis(250));
        }
        if !self.mpv {
            if ui.alert(AlertKind::Caution, "O Vídeo precisa do mpv.", "Instale o pacote mpv (por exemplo: sudo apt install mpv) e abra o Vídeo de novo.", &["OK"]).is_some() {
                ui.close = true;
            }
            return;
        }
        if let Some(fb) = &mut self.open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.open_fb = None;
                if let Choice::Picked(p) = c {
                    self.load(p);
                }
                ui.redraw = true;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new("Arquivo", vec![MenuItem::new("open", "Abrir…", "Ctrl+O"), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")]),
            Menu::new(
                "Controles",
                vec![
                    MenuItem::new("toggle", "Tocar/Pausar", "Space"),
                    MenuItem::new("back", "Voltar 10 s", "Left"),
                    MenuItem::new("fwd", "Avançar 10 s", "Right"),
                    MenuItem::Separator,
                    MenuItem::new("full", "Tela Cheia", "Ctrl+F"),
                ],
            ),
        ]
    }
}

fn acts_open(fb: &mut Option<FileBrowser>, file: &Option<PathBuf>) {
    let dir = file.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Vídeos")));
    *fb = Some(FileBrowser::open(dir, &EXTS));
}

fn main() {
    let mut app = Video { mpv: has_mpv(), player: None, state: Arc::default(), file: None, volume: 0.8, open_fb: None, error: None };
    if let Some(p) = std::env::args().nth(1).map(PathBuf::from) {
        app.load(p);
    }
    let opts = WindowOptions { title: "Vídeo".into(), app_id: "glove.Video".into(), size: (440, 120), fixed: true };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-video: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::json_field;

    #[test]
    fn le_eventos_do_mpv() {
        let l = r#"{"event":"property-change","id":1,"name":"time-pos","data":12.5}"#;
        assert_eq!(json_field(l, "event"), Some("property-change"));
        assert_eq!(json_field(l, "name"), Some("time-pos"));
        assert_eq!(json_field(l, "data"), Some("12.5"));
        assert_eq!(json_field(r#"{"name":"pause","data":true}"#, "data"), Some("true"));
    }
}
