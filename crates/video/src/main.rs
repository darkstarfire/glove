//! Vídeo: o mpv mostra a imagem (na janela dele, sem os controles dele) e
//! esta janela é o painel de controle da era: tocar/pausar, a barra do tempo,
//! o volume e a tela cheia. Fala com o mpv pelo soquete JSON dele.
//!
//! Os menus têm o resto: abrir arquivo ou endereço, recentes, o próximo e o
//! anterior da pasta, velocidade, repetir, tamanho e proporção da imagem,
//! captura de quadro, faixas de áudio e de legenda, carregar legenda, atrasos
//! e tamanho da legenda. As Preferências guardam o volume, continuar de onde
//! parou, a decodificação pela placa de vídeo e os idiomas preferidos.

use glove_config::tr;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, Color},
    run, AlertKind, App, Choice, FileBrowser, Menu, MenuItem, Rect, Ticks, Ui, WindowOptions,
};
use serde_json::{json, Value};

const EXTS: [&str; 13] = ["mp4", "mkv", "webm", "avi", "mov", "m4v", "ogv", "mpg", "mpeg", "wmv", "flv", "ts", "3gp"];
const SUB_EXTS: [&str; 6] = ["srt", "ass", "ssa", "vtt", "sub", "idx"];
const SPEEDS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// Proporções: (valor do mpv, nome).
const ASPECTS: [(&str, &str); 5] = [("-1", "Automática"), ("16:9", "16:9"), ("4:3", "4:3"), ("2.35:1", "2,35:1"), ("1:1", "1:1")];
const PREFS_WIN: u32 = 1;
const MAX_RECENTS: usize = 10;

/// Uma faixa do arquivo (vídeo, áudio ou legenda).
#[derive(Clone, Debug, Default, PartialEq)]
struct Track {
    id: i64,
    kind: String,
    title: String,
    lang: String,
    codec: String,
    selected: bool,
    external: bool,
}

impl Track {
    /// "Português — Comentários (srt)", "Faixa 2 (aac)".
    fn label(&self) -> String {
        let lang = lang_name(&self.lang);
        let mut s = match (lang.is_empty(), self.title.is_empty()) {
            (false, false) => format!("{lang} — {}", self.title),
            (false, true) => lang,
            (true, false) => self.title.clone(),
            (true, true) => tr!("Faixa {}", self.id).to_string(),
        };
        if !self.codec.is_empty() {
            s.push_str(&format!(" ({})", self.codec));
        }
        if self.external {
            s.push_str(&format!(" — {}", tr!("arquivo")));
        }
        s
    }
}

/// Nome do idioma pelo código (os mais comuns); senão, o próprio código.
fn lang_name(code: &str) -> String {
    let n = match code.to_lowercase().as_str() {
        "pt" | "por" | "pob" | "pt-br" | "pt_br" => tr!("Português"),
        "en" | "eng" => tr!("Inglês"),
        "es" | "spa" => tr!("Espanhol"),
        "fr" | "fre" | "fra" => tr!("Francês"),
        "de" | "ger" | "deu" => tr!("Alemão"),
        "it" | "ita" => tr!("Italiano"),
        "ja" | "jpn" => tr!("Japonês"),
        "" | "und" => "",
        _ => return code.to_string(),
    };
    n.to_string()
}

fn parse_tracks(v: &Value) -> Vec<Track> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|t| Track {
                    id: t.get("id").and_then(Value::as_i64).unwrap_or(0),
                    kind: t.get("type").and_then(Value::as_str).unwrap_or_default().to_string(),
                    title: t.get("title").and_then(Value::as_str).unwrap_or_default().to_string(),
                    lang: t.get("lang").and_then(Value::as_str).unwrap_or_default().to_string(),
                    codec: t.get("codec").and_then(Value::as_str).unwrap_or_default().to_string(),
                    selected: t.get("selected").and_then(Value::as_bool).unwrap_or(false),
                    external: t.get("external").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
struct State {
    pos: f64,
    dur: f64,
    paused: bool,
    title: String,
    ended: bool,
    tracks: Vec<Track>,
    speed: f64,
    looping: bool,
    sub_visible: bool,
    sub_delay: f64,
    audio_delay: f64,
    mute: bool,
    /// Volume do mpv (0 a 1), quando muda por ele (teclas na janela da imagem).
    volume: Option<f32>,
}

/// Aplica um evento do mpv ("property-change") ao estado.
fn apply_event(s: &mut State, line: &str) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    if v.get("event").and_then(Value::as_str) != Some("property-change") {
        return;
    }
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    let num = data.as_f64();
    match v.get("name").and_then(Value::as_str) {
        Some("time-pos") => s.pos = num.unwrap_or(s.pos),
        Some("duration") => s.dur = num.unwrap_or(0.0),
        Some("pause") => s.paused = data.as_bool().unwrap_or(false),
        Some("media-title") => s.title = data.as_str().unwrap_or_default().to_string(),
        Some("eof-reached") => s.ended = data.as_bool().unwrap_or(false),
        Some("track-list") => s.tracks = parse_tracks(&data),
        Some("speed") => s.speed = num.unwrap_or(1.0),
        Some("loop-file") => s.looping = !matches!(&data, Value::Bool(false) | Value::Null) && data.as_str() != Some("no"),
        Some("sub-visibility") => s.sub_visible = data.as_bool().unwrap_or(false),
        Some("sub-delay") => s.sub_delay = num.unwrap_or(0.0),
        Some("audio-delay") => s.audio_delay = num.unwrap_or(0.0),
        Some("mute") => s.mute = data.as_bool().unwrap_or(false),
        Some("volume") => s.volume = num.map(|v| (v / 100.0).clamp(0.0, 1.0) as f32),
        _ => {}
    }
}

fn mmss(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// "0,1" ou "0.1", conforme o idioma.
fn decimal(v: f64, places: usize) -> String {
    format!("{v:.places$}").replace('.', glove_config::virgula())
}

fn has_mpv() -> bool {
    Command::new("mpv").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Pasta de uma "user dir" do XDG (PICTURES, VIDEOS), ou um dos nomes dados.
fn user_dir(key: &str, names: &[&str]) -> PathBuf {
    let home = home();
    let xdg = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok().and_then(|t| {
        t.lines().find_map(|l| l.strip_prefix(&format!("XDG_{key}_DIR="))).map(|v| PathBuf::from(v.trim_matches('"').replace("$HOME", &home.to_string_lossy())))
    });
    xdg.filter(|p| p.is_dir()).or_else(|| names.iter().map(|d| home.join(d)).find(|p| p.is_dir())).unwrap_or(home)
}

// ---------------------------------------------------------------------------
// Preferências e recentes
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    volume: f32,
    /// Continuar de onde parou (o mpv guarda a posição de cada arquivo).
    resume: bool,
    /// Decodificar pela placa de vídeo (mais leve; desligar se der imagem estranha).
    hwdec: bool,
    /// Legendas da mesma pasta com nome parecido (não só o nome igual).
    auto_subs: bool,
    /// Ao terminar, o próximo vídeo da pasta.
    auto_next: bool,
    sub_lang: String,
    audio_lang: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { volume: 0.8, resume: true, hwdec: true, auto_subs: true, auto_next: false, sub_lang: "pt,por,pob,en,eng".into(), audio_lang: String::new() }
    }
}

impl Settings {
    fn path() -> PathBuf {
        glove_config::dir().join("video.conf")
    }

    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let on = v == "sim" || v == "true";
            match k {
                "volume" => s.volume = v.parse::<f32>().unwrap_or(s.volume).clamp(0.0, 1.0),
                "continuar" => s.resume = on,
                "placa_de_video" => s.hwdec = on,
                "legendas_da_pasta" => s.auto_subs = on,
                "proximo_ao_terminar" => s.auto_next = on,
                "idiomas_legenda" => s.sub_lang = v.to_string(),
                "idiomas_audio" => s.audio_lang = v.to_string(),
                _ => {}
            }
        }
        s
    }

    fn text(&self) -> String {
        let b = |v: bool| if v { "sim" } else { "nao" };
        format!(
            "# Preferências do Vídeo (o Glove reescreve este arquivo)\nvolume = {:.2}\ncontinuar = {}\nplaca_de_video = {}\nlegendas_da_pasta = {}\nproximo_ao_terminar = {}\nidiomas_legenda = {}\nidiomas_audio = {}\n",
            self.volume,
            b(self.resume),
            b(self.hwdec),
            b(self.auto_subs),
            b(self.auto_next),
            self.sub_lang,
            self.audio_lang
        )
    }

    fn load() -> Self {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(glove_config::dir());
        let _ = std::fs::write(Self::path(), self.text());
    }

    /// Argumentos do mpv que vêm das preferências.
    fn mpv_args(&self) -> Vec<String> {
        let mut a = vec![format!("--volume={}", (self.volume * 100.0).round() as i32)];
        if self.resume {
            let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/state"));
            a.push("--save-position-on-quit".into());
            a.push(format!("--watch-later-dir={}", state.join("glove/video-posicoes").display()));
        }
        a.push(format!("--hwdec={}", if self.hwdec { "auto-safe" } else { "no" }));
        a.push(format!("--sub-auto={}", if self.auto_subs { "fuzzy" } else { "exact" }));
        if !self.sub_lang.trim().is_empty() {
            a.push(format!("--slang={}", self.sub_lang.trim()));
        }
        if !self.audio_lang.trim().is_empty() {
            a.push(format!("--alang={}", self.audio_lang.trim()));
        }
        a
    }
}

fn recents_path() -> PathBuf {
    glove_config::dir().join("video-recentes")
}

fn load_recents() -> Vec<String> {
    std::fs::read_to_string(recents_path()).map(|t| t.lines().filter(|l| !l.trim().is_empty()).take(MAX_RECENTS).map(String::from).collect()).unwrap_or_default()
}

fn save_recents(r: &[String]) {
    let _ = std::fs::create_dir_all(glove_config::dir());
    let _ = std::fs::write(recents_path(), r.join("\n") + "\n");
}

/// Nome curto para mostrar (o arquivo, ou o endereço).
fn short_name(target: &str) -> String {
    if target.contains("://") {
        return target.to_string();
    }
    Path::new(target).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| target.to_string())
}

/// Os vídeos da mesma pasta, em ordem.
fn siblings(file: &Path) -> Vec<PathBuf> {
    let Some(dir) = file.parent() else { return Vec::new() };
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| EXTS.contains(&x.to_ascii_lowercase().as_str())))
        .collect();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    v
}

// ---------------------------------------------------------------------------
// O mpv
// ---------------------------------------------------------------------------

struct Player {
    child: Child,
    sock: UnixStream,
}

const OBSERVED: [&str; 13] = ["time-pos", "duration", "pause", "media-title", "eof-reached", "track-list", "speed", "loop-file", "sub-visibility", "sub-delay", "audio-delay", "mute", "volume"];

impl Player {
    fn start(target: &str, state: Arc<Mutex<State>>, settings: &Settings) -> Option<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("glove-video-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let shots = user_dir("PICTURES", &["Imagens", "Pictures"]);
        let child = Command::new("mpv")
            .arg(format!("--input-ipc-server={}", path.display()))
            .args(["--osc=no", "--keep-open=yes", "--force-window=yes", "--wayland-app-id=glove.VideoTela", "--title=${media-title}"])
            .arg(format!("--screenshot-directory={}", shots.display()))
            .arg(format!("--screenshot-template={} %F %wH.%wM.%wS", tr!("Vídeo")))
            .args(settings.mpv_args())
            .arg("--")
            .arg(target)
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
        for (i, p) in OBSERVED.iter().enumerate() {
            let _ = writeln!(sock, "{}", json!({ "command": ["observe_property", i + 1, p] }));
        }
        let reader = sock.try_clone().ok()?;
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                apply_event(&mut state.lock().unwrap(), &line);
            }
        });
        Some(Self { child, sock })
    }

    fn cmd(&mut self, args: Value) {
        let _ = writeln!(self.sock, "{}", json!({ "command": args }));
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // "quit" deixa o mpv guardar a posição (continuar de onde parou).
        self.cmd(json!(["quit"]));
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// O app
// ---------------------------------------------------------------------------

struct Video {
    mpv: bool,
    player: Option<Player>,
    state: Arc<Mutex<State>>,
    /// O que está aberto: caminho de arquivo ou endereço.
    target: Option<String>,
    settings: Settings,
    recents: Vec<String>,
    open_fb: Option<FileBrowser>,
    sub_fb: Option<FileBrowser>,
    /// Abrir Endereço: o texto digitado.
    url: Option<String>,
    aspect: usize,
    /// Aviso no rodapé: (texto, é erro).
    note: Option<(String, bool)>,
}

impl Video {
    fn local_file(&self) -> Option<PathBuf> {
        self.target.as_ref().filter(|t| !t.contains("://")).map(PathBuf::from)
    }

    fn load(&mut self, target: String) {
        self.player = None;
        *self.state.lock().unwrap() = State::default();
        self.note = None;
        self.aspect = 0;
        if !self.mpv {
            return;
        }
        self.recents.retain(|r| *r != target);
        self.recents.insert(0, target.clone());
        self.recents.truncate(MAX_RECENTS);
        save_recents(&self.recents);
        self.player = Player::start(&target, self.state.clone(), &self.settings);
        if self.player.is_none() {
            self.note = Some((tr!("O mpv não abriu esse vídeo.").into(), true));
        }
        self.target = Some(target);
    }

    fn cmd(&mut self, args: Value) {
        if let Some(p) = &mut self.player {
            p.cmd(args);
        }
    }

    fn step_file(&mut self, delta: isize) {
        let Some(cur) = self.local_file() else { return };
        let list = siblings(&cur);
        let Some(i) = list.iter().position(|p| *p == cur) else { return };
        let j = i as isize + delta;
        if j >= 0 && (j as usize) < list.len() {
            self.load(list[j as usize].to_string_lossy().into_owned());
        } else {
            self.note = Some((if delta > 0 { tr!("Este é o último vídeo da pasta.") } else { tr!("Este é o primeiro vídeo da pasta.") }.into(), false));
        }
    }

    fn start_dir(&self) -> PathBuf {
        self.local_file().and_then(|f| f.parent().map(Path::to_path_buf)).unwrap_or_else(|| user_dir("VIDEOS", &["Vídeos", "Videos"]))
    }

    fn act(&mut self, a: &str, ui: &mut Ui) {
        let (speed, looping) = {
            let s = self.state.lock().unwrap();
            (if s.speed > 0.0 { s.speed } else { 1.0 }, s.looping)
        };
        match a {
            "open" => self.open_fb = Some(FileBrowser::open(Some(self.start_dir()), &EXTS)),
            "url" => self.url = Some(String::new()),
            "recent-clear" => {
                self.recents.clear();
                save_recents(&self.recents);
            }
            a if a.starts_with("recent:") => {
                if let Some(t) = a[7..].parse::<usize>().ok().and_then(|i| self.recents.get(i).cloned()) {
                    self.load(t);
                }
            }
            "reveal" => {
                if let Some(dir) = self.local_file().and_then(|f| f.parent().map(Path::to_path_buf)) {
                    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-files".into());
                    let _ = Command::new(exe).arg(dir).spawn();
                }
            }
            "close" => ui.close = true,
            "prefs" => ui.open_window(PREFS_WIN, WindowOptions { title: tr!("Preferências do Vídeo").into(), app_id: "glove.Video".into(), size: (460, 380), fixed: false, min_size: Some((420, 370)) }),
            "toggle" => match self.player.is_some() {
                true => self.cmd(json!(["cycle", "pause"])),
                false => match self.target.clone() {
                    Some(t) => self.load(t),
                    None => self.act("open", ui),
                },
            },
            "stop" => {
                self.player = None;
                *self.state.lock().unwrap() = State::default();
            }
            "fwd" => self.cmd(json!(["seek", 10, "relative"])),
            "back" => self.cmd(json!(["seek", -10, "relative"])),
            "fwd60" => self.cmd(json!(["seek", 60, "relative"])),
            "back60" => self.cmd(json!(["seek", -60, "relative"])),
            "start" => self.cmd(json!(["seek", 0, "absolute"])),
            "next-file" => self.step_file(1),
            "prev-file" => self.step_file(-1),
            a if a.starts_with("speed:") => {
                if let Some(v) = a[6..].parse::<usize>().ok().and_then(|i| SPEEDS.get(i)) {
                    self.cmd(json!(["set_property", "speed", v]));
                }
            }
            "slower" | "faster" => {
                let i = SPEEDS.iter().position(|v| (*v - speed).abs() < 0.01).unwrap_or(2);
                let j = if a == "faster" { (i + 1).min(SPEEDS.len() - 1) } else { i.saturating_sub(1) };
                self.cmd(json!(["set_property", "speed", SPEEDS[j]]));
            }
            "loop" => self.cmd(json!(["set_property", "loop-file", if looping { "no" } else { "inf" }])),
            "vol-up" | "vol-down" => {
                let v = (self.settings.volume + if a == "vol-up" { 0.05 } else { -0.05 }).clamp(0.0, 1.0);
                self.settings.volume = v;
                self.cmd(json!(["set_property", "volume", (v * 100.0).round()]));
            }
            "mute" => self.cmd(json!(["cycle", "mute"])),
            "full" => self.cmd(json!(["cycle", "fullscreen"])),
            a if a.starts_with("size:") => {
                if let Ok(k) = a[5..].parse::<f64>() {
                    self.cmd(json!(["set_property", "window-scale", k]));
                }
            }
            a if a.starts_with("aspect:") => {
                if let Some((i, (v, _))) = a[7..].parse::<usize>().ok().and_then(|i| ASPECTS.get(i).map(|x| (i, x))) {
                    self.aspect = i;
                    self.cmd(json!(["set_property", "video-aspect-override", v]));
                }
            }
            "shot" => {
                self.cmd(json!(["screenshot"]));
                self.note = Some((tr!("Quadro salvo na pasta Imagens.").into(), false));
            }
            a if a.starts_with("atrack:") => {
                if let Ok(id) = a[7..].parse::<i64>() {
                    self.cmd(json!(["set_property", "aid", id]));
                }
            }
            "adelay+" => self.cmd(json!(["add", "audio-delay", 0.1])),
            "adelay-" => self.cmd(json!(["add", "audio-delay", -0.1])),
            "adelay0" => self.cmd(json!(["set_property", "audio-delay", 0])),
            "subs" => self.cmd(json!(["cycle", "sub-visibility"])),
            "strack:no" => self.cmd(json!(["set_property", "sid", "no"])),
            a if a.starts_with("strack:") => {
                if let Ok(id) = a[7..].parse::<i64>() {
                    self.cmd(json!(["set_property", "sid", id]));
                    self.cmd(json!(["set_property", "sub-visibility", true]));
                }
            }
            "sub-load" => self.sub_fb = Some(FileBrowser::open(Some(self.start_dir()), &SUB_EXTS)),
            "sdelay+" => self.cmd(json!(["add", "sub-delay", 0.1])),
            "sdelay-" => self.cmd(json!(["add", "sub-delay", -0.1])),
            "sdelay0" => self.cmd(json!(["set_property", "sub-delay", 0])),
            "sub-bigger" => self.cmd(json!(["add", "sub-scale", 0.1])),
            "sub-smaller" => self.cmd(json!(["add", "sub-scale", -0.1])),
            _ => {}
        }
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

    /// Abrir Endereço: um campo por cima do painel.
    fn url_sheet(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(ui.theme.body_b));
        ui.c.pinstripes(0.0, 0.0, w, h, ui.theme.body_a, ui.theme.body_b);
        ui.fonts.draw(ui.c, tr!("Endereço do vídeo (link ou caminho do arquivo):"), 16.0, 24.0, 12.0, true, rgba(20, 20, 24, 255));
        let mut text = self.url.take().unwrap_or_default();
        let (_, enter) = ui.text_field_in(Rect::new(16.0, 34.0, w - 32.0, 22.0), &mut text, true);
        let mut open = enter;
        let mut cancel = false;
        ui.area(Rect::new(w - 220.0, h - 36.0, 96.0, 24.0), |ui| cancel = ui.button(tr!("Cancelar")));
        ui.area(Rect::new(w - 116.0, h - 36.0, 100.0, 24.0), |ui| open |= ui.button(tr!("Abrir")));
        if ui.keys().iter().any(|k| k.combo == "Escape") {
            cancel = true;
        }
        if cancel {
            ui.redraw = true;
        } else if open && !text.trim().is_empty() {
            let t = text.trim().to_string();
            let t = t.strip_prefix("file://").map(String::from).unwrap_or(t);
            self.load(t);
            ui.redraw = true;
        } else {
            self.url = Some(text);
        }
    }

    /// A janela de Preferências.
    fn prefs_ui(&mut self, ui: &mut Ui) {
        let before = self.settings.clone();
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, ui.c.h, &solid(ui.theme.body_b));
        ui.c.pinstripes(0.0, 0.0, w, ui.c.h, ui.theme.body_a, ui.theme.body_b);
        ui.area(Rect::new(20.0, 16.0, w - 40.0, ui.c.h - 32.0), |ui| {
            let s = &mut self.settings;
            ui.label_ex(tr!("Reprodução"), 13.0, true, rgba(20, 20, 24, 255));
            ui.space(6.0);
            ui.checkbox(&mut s.resume, tr!("Continuar de onde parou"));
            ui.checkbox(&mut s.auto_next, tr!("Ao terminar, tocar o próximo vídeo da pasta"));
            ui.checkbox(&mut s.hwdec, tr!("Usar a placa de vídeo para decodificar (mais leve)"));
            ui.space(12.0);
            ui.label_ex(tr!("Legendas e áudio"), 13.0, true, rgba(20, 20, 24, 255));
            ui.space(6.0);
            ui.checkbox(&mut s.auto_subs, tr!("Carregar legendas da pasta com nome parecido"));
            ui.space(6.0);
            ui.label(tr!("Idiomas preferidos das legendas (códigos, na ordem):"));
            ui.text_field(&mut s.sub_lang, w - 40.0);
            ui.space(4.0);
            ui.label(tr!("Idiomas preferidos do áudio (vazio = o padrão do vídeo):"));
            ui.text_field(&mut s.audio_lang, w - 40.0);
            ui.space(8.0);
            ui.small(tr!("Exemplos: pt, en, es. As mudanças valem a partir do próximo vídeo."));
        });
        if self.settings != before {
            self.settings.save();
        }
    }
}

fn tri(c: &mut Canvas, x: f32, y: f32, s: f32, dir: f32, col: Color) {
    let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
    pb.move_to(x - s * 0.5 * dir, y - s * 0.6);
    pb.line_to(x + s * 0.6 * dir, y);
    pb.line_to(x - s * 0.5 * dir, y + s * 0.6);
    pb.close();
    c.fill(pb.finish(), &solid(col));
}

/// Tecla (no formato dos atalhos) → ação.
fn key_action(combo: &str) -> Option<&'static str> {
    Some(match combo {
        "Ctrl+O" => "open",
        "Ctrl+L" => "url",
        "Ctrl+W" => "close",
        "Ctrl+comma" => "prefs",
        "Ctrl+F" => "full",
        "space" | "Space" => "toggle",
        "Ctrl+period" => "stop",
        "Right" => "fwd",
        "Left" => "back",
        "Shift+Right" => "fwd60",
        "Shift+Left" => "back60",
        "Home" => "start",
        "Ctrl+Right" => "next-file",
        "Ctrl+Left" => "prev-file",
        "Up" => "vol-up",
        "Down" => "vol-down",
        "M" => "mute",
        "bracketleft" => "slower",
        "bracketright" => "faster",
        "Ctrl+0" => "size:0.5",
        "Ctrl+1" => "size:1",
        "Ctrl+2" => "size:2",
        "Ctrl+Shift+S" => "shot",
        "V" => "subs",
        _ => return None,
    })
}

impl App for Video {
    fn ui(&mut self, ui: &mut Ui) {
        if ui.window_id() == PREFS_WIN {
            self.prefs_ui(ui);
            return;
        }
        if self.url.is_some() {
            self.url_sheet(ui);
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.open_fb.is_none() && self.sub_fb.is_none() {
            acts.extend(ui.keys().iter().filter_map(|k| key_action(&k.combo)).map(String::from));
        }
        // O volume mudou pelo mpv (teclas na janela da imagem): o controle acompanha.
        if let Some(v) = self.state.lock().unwrap().volume.take() {
            self.settings.volume = v;
        }
        ui.c.brushed_metal(0.0, 0.0, w, h, rgba(226, 226, 228, 255), rgba(184, 184, 188, 255));
        let (pos, dur, paused, title, speed, looping, mute) = {
            let s = self.state.lock().unwrap();
            (s.pos, s.dur, s.paused, s.title.clone(), s.speed, s.looping, s.mute)
        };
        // Nome do vídeo.
        let name = if !title.is_empty() { title } else { self.target.as_deref().map(short_name).unwrap_or_else(|| tr!("Nenhum vídeo aberto").into()) };
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
                self.cmd(json!(["seek", (t * 100.0).round() / 100.0, "absolute"]));
            }
        }
        // Estado em letras miúdas: velocidade, repetir, sem som.
        let mut tags = Vec::new();
        if speed > 0.0 && (speed - 1.0).abs() > 0.01 {
            tags.push(format!("{}×", decimal(speed, 2).trim_end_matches('0').trim_end_matches([',', '.'])));
        }
        if looping {
            tags.push(tr!("Repetindo").to_string());
        }
        if mute {
            tags.push(tr!("Sem som").to_string());
        }
        if !tags.is_empty() {
            let t = tags.join(" · ");
            let tw = ui.text_width(&t, 10.0, false);
            ui.fonts.draw(ui.c, &t, (w - tw) / 2.0, 62.0, 10.0, false, rgba(60, 60, 70, 255));
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
        let mut vol = self.settings.volume;
        ui.area(Rect::new(16.0, cy - 9.0, 110.0, 24.0), |ui| {
            if ui.slider_ticks(&mut vol, 0.0, 1.0, 100.0, &Ticks { count: 0, labels: &[], snap: false }, true) {
                self.settings.volume = vol;
                if let Some(p) = &mut self.player {
                    p.cmd(json!(["set_property", "volume", (vol * 100.0).round()]));
                }
            }
        });
        ui.area(Rect::new(w - 110.0, cy - 11.0, 96.0, 24.0), |ui| {
            if ui.button(tr!("Tela Cheia")) {
                acts.push("full".into());
            }
        });
        if let Some((t, err)) = &self.note {
            let t = ui.fonts.ellipsize(t, 10.0, false, w - 24.0);
            ui.fonts.draw(ui.c, &t, 12.0, h - 6.0, 10.0, false, if *err { rgba(180, 40, 30, 255) } else { rgba(60, 60, 70, 255) });
        }

        for a in acts {
            self.act(&a, ui);
        }
        // Terminou: o próximo da pasta (se pedido).
        let ended = std::mem::take(&mut self.state.lock().unwrap().ended);
        if ended && self.settings.auto_next && !looping {
            self.step_file(1);
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
            if ui.alert_window(AlertKind::Caution, tr!("O Vídeo precisa do mpv."), tr!("Instale o pacote mpv (por exemplo: sudo apt install mpv) e abra o Vídeo de novo."), &["OK"]).is_some() {
                ui.close = true;
            }
            return;
        }
        if let Some(fb) = &mut self.open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.open_fb = None;
                if let Choice::Picked(p) = c {
                    self.load(p.to_string_lossy().into_owned());
                }
                ui.redraw = true;
            }
        }
        if let Some(fb) = &mut self.sub_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.sub_fb = None;
                if let Choice::Picked(p) = c {
                    self.cmd(json!(["sub-add", p.to_string_lossy(), "select"]));
                    self.cmd(json!(["set_property", "sub-visibility", true]));
                }
                ui.redraw = true;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let s = self.state.lock().unwrap();
        let open = self.player.is_some();
        let local = self.local_file().is_some();
        let speed = if s.speed > 0.0 { s.speed } else { 1.0 };
        let recents: Vec<MenuItem> = if self.recents.is_empty() {
            vec![MenuItem::new("none", tr!("Nenhum"), "").enabled(false)]
        } else {
            let mut v: Vec<MenuItem> = self.recents.iter().enumerate().map(|(i, r)| MenuItem::new(&format!("recent:{i}"), &short_name(r), "")).collect();
            v.push(MenuItem::Separator);
            v.push(MenuItem::new("recent-clear", tr!("Limpar Menu"), ""));
            v
        };
        let tracks = |kind: &str| -> Vec<&Track> { s.tracks.iter().filter(|t| t.kind == kind).collect() };
        let mut audio: Vec<MenuItem> = tracks("audio").iter().map(|t| MenuItem::new(&format!("atrack:{}", t.id), &t.label(), "").checked(t.selected)).collect();
        if audio.is_empty() {
            audio.push(MenuItem::new("none", tr!("Nenhuma"), "").enabled(false));
        }
        let subs = tracks("sub");
        let mut sub_items = vec![MenuItem::new("strack:no", tr!("Nenhuma"), "").checked(!subs.iter().any(|t| t.selected)).enabled(open)];
        sub_items.extend(subs.iter().map(|t| MenuItem::new(&format!("strack:{}", t.id), &t.label(), "").checked(t.selected)));
        let delay = |v: f64| tr!("Atraso atual: {} s", decimal(v, 1)).to_string();
        vec![
            Menu::app(vec![MenuItem::new("prefs", tr!("Preferências…"), "Ctrl+comma")]),
            Menu::new(
                tr!("Arquivo"),
                vec![
                    MenuItem::new("open", tr!("Abrir…"), "Ctrl+O"),
                    MenuItem::new("url", tr!("Abrir Endereço…"), "Ctrl+L"),
                    MenuItem::submenu(tr!("Abrir Recente"), recents),
                    MenuItem::Separator,
                    MenuItem::new("reveal", tr!("Mostrar no Arquivos"), "").enabled(local),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
                ],
            ),
            Menu::new(
                tr!("Controles"),
                vec![
                    MenuItem::new("toggle", if open && !s.paused { tr!("Pausar") } else { tr!("Tocar") }, "Space"),
                    MenuItem::new("stop", tr!("Parar"), "Ctrl+period").enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("back", tr!("Voltar 10 s"), "Left").enabled(open),
                    MenuItem::new("fwd", tr!("Avançar 10 s"), "Right").enabled(open),
                    MenuItem::new("back60", tr!("Voltar 1 min"), "Shift+Left").enabled(open),
                    MenuItem::new("fwd60", tr!("Avançar 1 min"), "Shift+Right").enabled(open),
                    MenuItem::new("start", tr!("Ir para o Início"), "Home").enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("prev-file", tr!("Vídeo Anterior da Pasta"), "Ctrl+Left").enabled(local),
                    MenuItem::new("next-file", tr!("Próximo Vídeo da Pasta"), "Ctrl+Right").enabled(local),
                    MenuItem::Separator,
                    MenuItem::submenu(
                        tr!("Velocidade|Vídeo"),
                        SPEEDS
                            .iter()
                            .enumerate()
                            .map(|(i, v)| MenuItem::new(&format!("speed:{i}"), &format!("{}×", decimal(*v, 2).trim_end_matches('0').trim_end_matches([',', '.'])), "").checked((v - speed).abs() < 0.01))
                            .collect(),
                    )
                    .enabled(open),
                    MenuItem::new("loop", tr!("Repetir"), "").checked(s.looping).enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("vol-up", tr!("Aumentar Volume"), "Up"),
                    MenuItem::new("vol-down", tr!("Diminuir Volume"), "Down"),
                    MenuItem::new("mute", tr!("Sem Som"), "M").checked(s.mute).enabled(open),
                ],
            ),
            Menu::new(
                tr!("Vídeo"),
                vec![
                    MenuItem::new("full", tr!("Tela Cheia"), "Ctrl+F").enabled(open),
                    MenuItem::submenu(
                        tr!("Tamanho"),
                        vec![MenuItem::new("size:0.5", tr!("Metade do Tamanho"), "Ctrl+0"), MenuItem::new("size:1", tr!("Tamanho Original"), "Ctrl+1"), MenuItem::new("size:2", tr!("Dobro do Tamanho"), "Ctrl+2")],
                    )
                    .enabled(open),
                    MenuItem::submenu(tr!("Proporção"), ASPECTS.iter().enumerate().map(|(i, (_, n))| MenuItem::new(&format!("aspect:{i}"), glove_config::tr_dyn(n).as_str(), "").checked(i == self.aspect)).collect()).enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("shot", tr!("Capturar Quadro"), "Ctrl+Shift+S").enabled(open),
                ],
            ),
            Menu::new(
                tr!("Áudio"),
                vec![
                    MenuItem::submenu(tr!("Faixa de Áudio"), audio).enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("info", &delay(s.audio_delay), "").enabled(false),
                    MenuItem::new("adelay+", tr!("Atrasar o Áudio 0,1 s"), "").enabled(open),
                    MenuItem::new("adelay-", tr!("Adiantar o Áudio 0,1 s"), "").enabled(open),
                    MenuItem::new("adelay0", tr!("Sem Atraso no Áudio"), "").enabled(open && s.audio_delay.abs() > 0.001),
                ],
            ),
            Menu::new(
                tr!("Legendas"),
                vec![
                    MenuItem::new("subs", tr!("Mostrar Legendas"), "V").checked(s.sub_visible).enabled(open),
                    MenuItem::submenu(tr!("Faixa de Legenda"), sub_items).enabled(open),
                    MenuItem::new("sub-load", tr!("Carregar Legenda…"), "").enabled(open),
                    MenuItem::Separator,
                    MenuItem::new("info", &delay(s.sub_delay), "").enabled(false),
                    MenuItem::new("sdelay+", tr!("Atrasar a Legenda 0,1 s"), "").enabled(open),
                    MenuItem::new("sdelay-", tr!("Adiantar a Legenda 0,1 s"), "").enabled(open),
                    MenuItem::new("sdelay0", tr!("Sem Atraso na Legenda"), "").enabled(open && s.sub_delay.abs() > 0.001),
                    MenuItem::Separator,
                    MenuItem::new("sub-bigger", tr!("Legenda Maior"), "").enabled(open),
                    MenuItem::new("sub-smaller", tr!("Legenda Menor"), "").enabled(open),
                ],
            ),
        ]
    }

    fn close_requested(&mut self) -> bool {
        self.settings.save();
        self.player = None;
        true
    }
}

pub fn main() {
    let mpv = has_mpv();
    let mut app = Video { mpv, player: None, state: Arc::default(), target: None, settings: Settings::load(), recents: load_recents(), open_fb: None, sub_fb: None, url: None, aspect: 0, note: None };
    if let Some(f) = std::env::args().nth(1) {
        app.load(f);
    }
    let opts = WindowOptions { title: tr!("Vídeo").into(), app_id: "glove.Video".into(), size: (440, 120), fixed: false, min_size: Some((380, 120)) };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-video: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_eventos_do_mpv() {
        let mut s = State::default();
        apply_event(&mut s, r#"{"event":"property-change","id":1,"name":"time-pos","data":12.5}"#);
        apply_event(&mut s, r#"{"event":"property-change","name":"pause","data":true}"#);
        apply_event(&mut s, r#"{"event":"property-change","name":"loop-file","data":"inf"}"#);
        apply_event(&mut s, r#"{"event":"property-change","name":"volume","data":55.0}"#);
        assert_eq!((s.pos, s.paused, s.looping, s.volume), (12.5, true, true, Some(0.55)));
        apply_event(&mut s, r#"{"event":"property-change","name":"loop-file","data":false}"#);
        assert!(!s.looping);
        // Faixas: duas de áudio e uma legenda escolhida.
        let tl = r#"{"event":"property-change","name":"track-list","data":[{"id":1,"type":"video","codec":"h264","selected":true},{"id":1,"type":"audio","lang":"por","codec":"aac","selected":true},{"id":2,"type":"audio","lang":"eng","title":"Comentários","codec":"ac3"},{"id":1,"type":"sub","lang":"pob","codec":"subrip","external":true,"selected":true}]}"#;
        apply_event(&mut s, tl);
                assert_eq!(s.tracks.len(), 4);
        let sub = s.tracks.iter().find(|t| t.kind == "sub").unwrap();
        assert!(sub.selected && sub.external);
        // Respostas a comandos (sem "event") não mudam nada.
        apply_event(&mut s, r#"{"request_id":0,"error":"success"}"#);
        assert_eq!(s.tracks.len(), 4);
    }

    #[test]
    fn preferencias_vao_e_voltam() {
        let mut p = Settings::default();
        p.volume = 0.42;
        p.resume = false;
        p.auto_next = true;
        p.sub_lang = "en".into();
        assert_eq!(Settings::parse(&p.text()), p);
        let args = p.mpv_args();
        assert!(args.contains(&"--volume=42".to_string()) && args.contains(&"--slang=en".to_string()));
        assert!(!args.iter().any(|a| a == "--save-position-on-quit"));
    }

    #[test]
    fn tempos_e_nomes() {
        assert_eq!(mmss(75.0), "1:15");
        assert_eq!(mmss(3725.0), "1:02:05");
        assert_eq!(short_name("/v/Filme.mkv"), "Filme.mkv");
        assert_eq!(short_name("https://x/y"), "https://x/y");
    }
}
