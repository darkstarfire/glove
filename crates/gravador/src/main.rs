//! Gravador: grava a voz (o microfone padrão) em WAV na pasta Música ▸ Gravações,
//! mostrando o tempo, o nível e a onda enquanto grava. Embaixo, as gravações: ouvir,
//! apagar (vai para a Lixeira) e mostrar no Arquivos.
//!
//! Grava pelo PipeWire (`pw-record`), ou pelo PulseAudio (`parecord`) ou ALSA
//! (`arecord`); toca pelo `pw-play`, `paplay` ou `aplay`. O nível vem do próprio
//! arquivo crescendo (as últimas amostras gravadas).

use glove_config::tr;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use glove_ui::{
    paint::{draw::*, icons::Icon},
    run, App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

const RATE: u32 = 48_000;
const TOP_H: f32 = 160.0;

fn has(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

/// Pasta das gravações: Música/Gravações (a pasta de música do usuário).
fn folder() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let dirs = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).unwrap_or_default();
    let music = dirs
        .lines()
        .find_map(|l| l.strip_prefix("XDG_MUSIC_DIR="))
        .map(|v| PathBuf::from(v.trim().trim_matches('"').replace("$HOME", &home.to_string_lossy())))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| ["Música", "Music"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or_else(|| home.join("Music")));
    music.join(tr!("Gravações|Gravador"))
}

fn local(t: SystemTime) -> (i32, u32, u32, u32, u32, u32) {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    (tm.tm_year + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32)
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Duração de um WAV de 16 bits pelo tamanho e pelo cabeçalho.
fn wav_duration(p: &Path) -> Option<Duration> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = [0u8; 44];
    f.read_exact(&mut h).ok()?;
    if &h[0..4] != b"RIFF" || &h[8..12] != b"WAVE" {
        return None;
    }
    let ch = u16::from_le_bytes([h[22], h[23]]).max(1) as u64;
    let rate = u32::from_le_bytes([h[24], h[25], h[26], h[27]]).max(1) as u64;
    let bits = u16::from_le_bytes([h[34], h[35]]).max(8) as u64;
    let len = f.metadata().ok()?.len().saturating_sub(44);
    Some(Duration::from_millis(len * 1000 / (rate * ch * bits / 8)))
}

/// Pico (0 a 1) das últimas `n` amostras de 16 bits gravadas.
fn peak(p: &Path, n: u64) -> f32 {
    let Ok(mut f) = std::fs::File::open(p) else { return 0.0 };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len <= 44 {
        return 0.0;
    }
    let bytes = (n * 2).min(len - 44) & !1;
    if f.seek(SeekFrom::Start(len - bytes)).is_err() {
        return 0.0;
    }
    let mut b = vec![0u8; bytes as usize];
    if f.read_exact(&mut b).is_err() {
        return 0.0;
    }
    b.chunks_exact(2).map(|c| (i16::from_le_bytes([c[0], c[1]]) as i32).unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0
}

fn record_cmd(out: &Path) -> Option<Command> {
    let r = RATE.to_string();
    if has("pw-record") {
        let mut c = Command::new("pw-record");
        c.args(["--format", "s16", "--rate", &r, "--channels", "1"]).arg(out);
        Some(c)
    } else if has("parecord") {
        let mut c = Command::new("parecord");
        c.args(["--file-format=wav", "--format=s16le", &format!("--rate={r}"), "--channels=1"]).arg(out);
        Some(c)
    } else if has("arecord") {
        let mut c = Command::new("arecord");
        c.args(["-q", "-f", "S16_LE", "-r", &r, "-c", "1"]).arg(out);
        Some(c)
    } else {
        None
    }
}

fn play_cmd(p: &Path) -> Option<Command> {
    ["pw-play", "paplay", "aplay"].iter().find(|c| has(c)).map(|c| {
        let mut cmd = Command::new(c);
        cmd.arg(p);
        cmd
    })
}

/// Manda para a Lixeira (com o .trashinfo, para dar para pôr de volta).
fn to_trash(p: &Path) -> std::io::Result<()> {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")).join("Trash");
    std::fs::create_dir_all(base.join("files"))?;
    std::fs::create_dir_all(base.join("info"))?;
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut target = name.clone();
    let mut n = 2;
    while base.join("files").join(&target).exists() {
        target = format!("{n} {name}");
        n += 1;
    }
    let (y, mo, d, h, mi, s) = local(SystemTime::now());
    std::fs::write(base.join("info").join(format!("{target}.trashinfo")), format!("[Trash Info]\nPath={}\nDeletionDate={y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}\n", p.display()))?;
    std::fs::rename(p, base.join("files").join(&target))
}

struct Gravador {
    rec: Option<(Child, PathBuf, Instant)>,
    play: Option<(Child, PathBuf)>,
    /// Picos recentes (a onda que corre da direita para a esquerda).
    wave: Vec<f32>,
    level: f32,
    list: Vec<(PathBuf, SystemTime, Option<Duration>)>,
    sel: Option<usize>,
    table: TableState,
    error: Option<String>,
}

impl Gravador {
    fn new() -> Self {
        let mut g = Self { rec: None, play: None, wave: Vec::new(), level: 0.0, list: Vec::new(), sel: None, table: TableState::default(), error: None };
        g.reload();
        g
    }

    fn reload(&mut self) {
        self.list.clear();
        if let Ok(rd) = std::fs::read_dir(folder()) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                if !["wav", "ogg", "flac", "mp3", "oga", "m4a"].contains(&ext.as_str()) {
                    continue;
                }
                let t = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                let d = wav_duration(&p);
                self.list.push((p, t, d));
            }
        }
        self.list.sort_by(|a, b| b.1.cmp(&a.1));
        if self.sel.is_some_and(|i| i >= self.list.len()) {
            self.sel = None;
        }
    }

    fn start(&mut self) {
        let dir = folder();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.error = Some(e.to_string());
            return;
        }
        let (y, mo, d, h, mi, s) = local(SystemTime::now());
        let out = dir.join(format!("{} {y:04}-{mo:02}-{d:02} {h:02}.{mi:02}.{s:02}.wav", tr!("Gravação|Gravador")));
        let Some(mut cmd) = record_cmd(&out) else {
            self.error = Some(tr!("Nenhum programa de gravação encontrado (pw-record, parecord ou arecord).").into());
            return;
        };
        match cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(c) => {
                self.stop_play();
                self.rec = Some((c, out, Instant::now()));
                self.wave.clear();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn stop(&mut self) {
        if let Some((mut c, p, _)) = self.rec.take() {
            // Ctrl+C: o programa fecha o arquivo direitinho (o cabeçalho com o tamanho).
            unsafe { libc::kill(c.id() as i32, libc::SIGINT) };
            let _ = c.wait();
            self.level = 0.0;
            self.reload();
            self.sel = self.list.iter().position(|(q, ..)| *q == p);
        }
    }

    fn stop_play(&mut self) {
        if let Some((mut c, _)) = self.play.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn toggle_play(&mut self) {
        let Some(p) = self.sel.and_then(|i| self.list.get(i)).map(|x| x.0.clone()) else { return };
        let same = self.play.as_ref().is_some_and(|(_, q)| *q == p);
        self.stop_play();
        if !same {
            if let Some(mut cmd) = play_cmd(&p) {
                if let Ok(c) = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                    self.play = Some((c, p));
                }
            }
        }
    }

    fn top(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        ui.c.brushed_metal(0.0, 0.0, w, TOP_H, rgba(214, 214, 218, 255), rgba(186, 186, 192, 255));
        ui.c.hline(0.0, TOP_H - 1.0, w, rgba(120, 120, 126, 255));
        // O botão grande: gravar (bolinha vermelha) ou parar (quadrado).
        let (cx, cy, r) = (70.0, TOP_H / 2.0, 42.0);
        let b = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r);
        let h = ui.hit(b);
        let recording = self.rec.is_some();
        ui.c.fill(circle(cx, cy + 2.0, r + 2.0), &solid(rgba(0, 0, 0, 60)));
        gel_circle(ui.c, cx, cy, r, if h.down { rgba(150, 150, 156, 255) } else { rgba(210, 210, 216, 255) });
        if recording {
            ui.c.fill(rrect(cx - 15.0, cy - 15.0, 30.0, 30.0, 4.0), &solid(rgba(50, 50, 56, 255)));
        } else {
            gel_circle(ui.c, cx, cy, 20.0, rgba(220, 30, 30, 255));
        }
        if h.clicked {
            if recording { self.stop() } else { self.start() }
        }
        // Tempo e o estado.
        let x = 140.0;
        let t = match &self.rec {
            Some((_, _, since)) => clock(since.elapsed()),
            None => "00:00".into(),
        };
        ui.fonts.draw(ui.c, &t, x, 62.0, 34.0, true, rgba(30, 30, 34, 255));
        let st = if recording { tr!("Gravando…") } else { tr!("Clique no botão vermelho para gravar") };
        ui.fonts.draw(ui.c, st, x, 84.0, 12.0, false, rgba(70, 70, 76, 255));
        // Nível: barra de LEDs.
        let (mx, my, mw, mh) = (x, 100.0, (w - x - 20.0).min(420.0), 10.0);
        let n = 30;
        let seg = mw / n as f32;
        for i in 0..n {
            let on = (i as f32 / n as f32) < self.level.sqrt();
            let col = if i < n * 6 / 10 { rgba(60, 190, 70, 255) } else if i < n * 85 / 100 { rgba(240, 200, 40, 255) } else { rgba(230, 50, 40, 255) };
            ui.c.fill(rrect(mx + i as f32 * seg, my, seg - 2.0, mh, 2.0), &solid(if on { col } else { rgba(0, 0, 0, 40) }));
        }
        // A onda dos últimos segundos.
        let (wy, wh) = (my + 18.0, 30.0);
        ui.c.fill(rrect(mx, wy, mw, wh, 4.0), &solid(rgba(30, 32, 38, 220)));
        let k = self.wave.len().min((mw / 3.0) as usize);
        for (j, v) in self.wave.iter().rev().take(k).enumerate() {
            let hh = (v.sqrt() * (wh - 6.0)).max(1.0);
            ui.c.fill_rect(mx + mw - 3.0 - j as f32 * 3.0, wy + (wh - hh) / 2.0, 2.0, hh, &solid(rgba(110, 220, 120, 255)));
        }
    }
}

impl App for Gravador {
    fn tick(&self) -> Option<Duration> {
        (self.rec.is_some() || self.play.is_some()).then_some(Duration::from_millis(60))
    }

    fn on_tick(&mut self) {
        if let Some((_, p, _)) = &self.rec {
            self.level = peak(p, RATE as u64 / 15);
            self.wave.push(self.level);
            if self.wave.len() > 400 {
                self.wave.drain(..100);
            }
        }
        // Acabou de tocar?
        if let Some((c, _)) = &mut self.play {
            if c.try_wait().ok().flatten().is_some() {
                self.play = None;
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        for k in ui.keys().to_vec() {
            if k.combo == "space" {
                self.toggle_play();
            }
        }
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "record" => {
                    if self.rec.is_some() { self.stop() } else { self.start() }
                }
                "play" => self.toggle_play(),
                _ => {}
            }
        }
        self.top(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        let bar = 40.0;
        let rows: Vec<TableRow> = self
            .list
            .iter()
            .map(|(p, t, d)| {
                let (y, mo, dd, hh, mi, _) = local(*t);
                let playing = self.play.as_ref().is_some_and(|(_, q)| q == p);
                TableRow {
                    cells: vec![p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), d.map(clock).unwrap_or_else(|| "—".into()), format!("{dd:02}/{mo:02}/{y} {hh:02}:{mi:02}")],
                    icon: Some(if playing { Icon::Music } else { Icon::AudioFile }),
                    depth: 0,
                    children: None,
                    tint: None,
                }
            })
            .collect();
        let cols = [Column::new(tr!("Gravação|Gravador"), 0.55), Column::new(tr!("Duração|Gravador"), 0.15).right(), Column::new(tr!("Data"), 0.3)];
        let r = ui.table(Rect::new(0.0, TOP_H, w, h - TOP_H - bar), &cols, &rows, self.sel, &mut self.table, &TableStyle { row_h: 22.0, ..TableStyle::default() });
        if let Some(i) = r.pressed {
            self.sel = Some(i);
        }
        if let Some(i) = r.double {
            self.sel = Some(i);
            self.toggle_play();
        }
        let has_sel = self.sel.is_some();
        let playing = self.play.is_some();
        let mut act = None;
        ui.c.fill_rect(0.0, h - bar, w, bar, &vgrad(h - bar, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(210, 210, 214, 255))]));
        ui.area(Rect::new(10.0, h - bar + 8.0, w - 20.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_enabled(has_sel);
                if ui.button(if playing { tr!("Parar") } else { tr!("Ouvir") }) {
                    act = Some("play");
                }
                if ui.button(tr!("Mover para a Lixeira")) {
                    act = Some("trash");
                }
                ui.set_enabled(true);
                if ui.button(tr!("Mostrar no Arquivos")) {
                    act = Some("files");
                }
            });
        });
        match act {
            Some("play") => self.toggle_play(),
            Some("trash") => {
                if let Some(p) = self.sel.and_then(|i| self.list.get(i)).map(|x| x.0.clone()) {
                    self.stop_play();
                    if let Err(e) = to_trash(&p) {
                        self.error = Some(e.to_string());
                    }
                    self.reload();
                }
            }
            Some("files") => {
                let target = self.sel.and_then(|i| self.list.get(i)).map(|x| x.0.parent().map(Path::to_path_buf).unwrap_or_default()).unwrap_or_else(folder);
                let _ = std::fs::create_dir_all(&target);
                let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-files".into());
                let _ = Command::new(exe).arg(&target).spawn();
            }
            _ => {}
        }
        if self.list.is_empty() && self.rec.is_none() {
            let t = tr!("Nenhuma gravação ainda.");
            let tw = ui.text_width(t, 12.0, false);
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0, TOP_H + (h - TOP_H - bar) / 2.0, 12.0, false, rgba(120, 120, 126, 255));
        }
        if let Some(e) = self.error.clone() {
            if ui.alert(glove_ui::AlertKind::Stop, tr!("Não foi possível gravar."), &e, &["OK"]).is_some() {
                self.error = None;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            tr!("Gravação|Gravador"),
            vec![
                MenuItem::new("record", if self.rec.is_some() { tr!("Parar de Gravar") } else { tr!("Gravar") }, "Ctrl+R"),
                MenuItem::new("play", tr!("Ouvir / Parar"), "").enabled(self.sel.is_some()),
            ],
        )]
    }

    fn close_requested(&mut self) -> bool {
        self.stop();
        self.stop_play();
        true
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Gravador").into(), app_id: "glove.Gravador".into(), size: (560, 460), fixed: false, min_size: Some((460, 360)) };
    if let Err(e) = run(opts, Gravador::new()) {
        eprintln!("glove-gravador: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn relogio() {
        assert_eq!(super::clock(std::time::Duration::from_secs(75)), "01:15");
    }
}
