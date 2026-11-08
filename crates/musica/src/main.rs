//! Música: a biblioteca (a pasta Música e as outras que o usuário juntar) e um
//! tocador leve, como o tocador de música da era. Tudo em Rust: o symphonia
//! decodifica, o pw-cat toca.
//!
//! - Barra lateral: a Biblioteca, Tocadas Recentemente e as listas do usuário
//!   (criar, renomear, apagar; pôr e tirar músicas pelos menus).
//! - Navegador (Exibir ▸ Mostrar Navegador): Artistas e Álbuns acima da tabela.
//! - Aleatório e Repetir (nada, todas ou uma), no rodapé e no menu Controles.
//! - Preferências: as pastas da biblioteca e o aviso de troca de música.
//!
//! Guarda tudo em `~/.config/glove`: `musica.conf` (preferências), `musica/listas/*.m3u`
//! e `musica/recentes.m3u`.

use glove_config::tr;
mod player;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, Color},
    run,
    ui::Key,
    App, Choice, Column, FileBrowser, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ticks, Ui, WindowOptions, SEARCH_H,
};
use player::{Cmd, Info, Status};

const TOP_H: f32 = 74.0;
const SIDE_W: f32 = 170.0;
const FOOT_H: f32 = 24.0;
const BROWSER_H: f32 = 132.0;
const EXTS: [&str; 7] = ["mp3", "flac", "ogg", "oga", "wav", "m4a", "aac"];
const MAX_RECENT: usize = 50;
const PREFS_WIN: u32 = 1;

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

fn is_music(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()).is_some_and(|x| EXTS.contains(&x.to_ascii_lowercase().as_str()))
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
            } else if is_music(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Sem etiquetas de artista ou álbum: vêm das pastas (Artista/Álbum/música).
fn with_folder_tags(mut info: Info, path: &Path, root: &Path) -> Info {
    let parts: Vec<String> = path.strip_prefix(root).ok().map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    if info.artist.is_empty() && parts.len() >= 2 {
        info.artist = parts[0].clone();
    }
    if info.album.is_empty() && parts.len() >= 3 {
        info.album = parts[parts.len() - 2].clone();
    }
    info
}

/// "3:07" ou "1:02:30".
pub fn mmss(s: f32) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Embaralha (xorshift com a hora como semente: não precisa ser criptográfico).
fn shuffle<T>(v: &mut [T]) {
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) | 1;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

// ---------------------------------------------------------------------------
// Preferências, listas e recentes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    browser: bool,
    /// Avisar (na central de avisos) quando a música troca com a janela atrás.
    notify: bool,
    /// Pastas além da pasta Música.
    folders: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { volume: 0.8, shuffle: false, repeat: Repeat::Off, browser: false, notify: true, folders: Vec::new() }
    }
}

impl Settings {
    fn path() -> PathBuf {
        glove_config::dir().join("musica.conf")
    }

    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let on = v == "sim";
            match k {
                "volume" => s.volume = v.parse::<f32>().unwrap_or(s.volume).clamp(0.0, 1.0),
                "aleatorio" => s.shuffle = on,
                "repetir" => s.repeat = match v { "todas" => Repeat::All, "uma" => Repeat::One, _ => Repeat::Off },
                "navegador" => s.browser = on,
                "avisar" => s.notify = on,
                "pasta" => s.folders.push(PathBuf::from(v)),
                _ => {}
            }
        }
        s
    }

    fn text(&self) -> String {
        let b = |v: bool| if v { "sim" } else { "nao" };
        let rep = match self.repeat { Repeat::Off => "nao", Repeat::All => "todas", Repeat::One => "uma" };
        let mut t = format!("# Preferências da Música (o Glove reescreve este arquivo)\nvolume = {:.2}\naleatorio = {}\nrepetir = {rep}\nnavegador = {}\navisar = {}\n", self.volume, b(self.shuffle), b(self.browser), b(self.notify));
        for f in &self.folders {
            t.push_str(&format!("pasta = {}\n", f.display()));
        }
        t
    }

    fn load() -> Self {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(glove_config::dir());
        let _ = std::fs::write(Self::path(), self.text());
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Playlist {
    name: String,
    items: Vec<PathBuf>,
}

fn data_dir() -> PathBuf {
    glove_config::dir().join("musica")
}

fn lists_dir() -> PathBuf {
    data_dir().join("listas")
}

fn read_m3u(p: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(p).map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(PathBuf::from).collect()).unwrap_or_default()
}

fn write_m3u(p: &Path, items: &[PathBuf]) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut t = String::from("#EXTM3U\n");
    for i in items {
        t.push_str(&format!("{}\n", i.display()));
    }
    let _ = std::fs::write(p, t);
}

fn load_lists() -> Vec<Playlist> {
    let mut v: Vec<Playlist> = std::fs::read_dir(lists_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "m3u"))
        .map(|p| Playlist { name: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), items: read_m3u(&p) })
        .collect();
    v.sort_by_key(|l| l.name.to_lowercase());
    v
}

fn list_path(name: &str) -> PathBuf {
    lists_dir().join(format!("{}.m3u", name.replace('/', "-")))
}

/// Um nome livre: "Lista sem título", "Lista sem título 2"...
fn free_name(lists: &[Playlist], base: &str) -> String {
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") }).find(|n| !lists.iter().any(|l| l.name.eq_ignore_ascii_case(n))).unwrap()
}

// ---------------------------------------------------------------------------
// O app
// ---------------------------------------------------------------------------

struct Song {
    path: PathBuf,
    info: Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Library,
    Recent,
    List(usize),
}

struct Musica {
    songs: Arc<Mutex<Vec<Song>>>,
    scanning: Arc<Mutex<bool>>,
    table: TableState,
    selected: Option<PathBuf>,
    filter: String,
    status: Arc<Mutex<Status>>,
    tx: std::sync::mpsc::Sender<Cmd>,
    settings: Settings,
    /// Ordem de tocar (a da tabela na hora do play, embaralhada no aleatório).
    queue: Vec<PathBuf>,
    lists: Vec<Playlist>,
    recent: Vec<PathBuf>,
    source: Source,
    /// Navegador: artista e álbum escolhidos (None = todos).
    artist: Option<String>,
    album: Option<String>,
    /// Renomeando uma lista: (índice, texto).
    renaming: Option<(usize, String)>,
    /// A música que tocava no quadro anterior (para "recentes" e o aviso).
    last_playing: Option<PathBuf>,
    folder_fb: Option<FileBrowser>,
}

impl Musica {
    fn new() -> Self {
        let settings = Settings::load();
        let status: Arc<Mutex<Status>> = Arc::default();
        let tx = player::spawn(status.clone());
        let _ = tx.send(Cmd::Volume(settings.volume));
        let mut m = Self {
            songs: Arc::default(),
            scanning: Arc::new(Mutex::new(true)),
            table: TableState::sorted_by(1, true),
            selected: None,
            filter: String::new(),
            status,
            tx,
            settings,
            queue: Vec::new(),
            lists: load_lists(),
            recent: read_m3u(&data_dir().join("recentes.m3u")),
            source: Source::Library,
            artist: None,
            album: None,
            renaming: None,
            last_playing: None,
            folder_fb: None,
        };
        m.rescan();
        m
    }

    fn dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![music_dir()];
        for f in &self.settings.folders {
            if !v.contains(f) {
                v.push(f.clone());
            }
        }
        v
    }

    /// Lê a biblioteca de novo (numa thread: as tags chegam aos poucos).
    fn rescan(&mut self) {
        let (songs, scanning, dirs) = (self.songs.clone(), self.scanning.clone(), self.dirs());
        songs.lock().unwrap().clear();
        *scanning.lock().unwrap() = true;
        std::thread::spawn(move || {
            let mut seen = BTreeSet::new();
            for d in dirs {
                for p in scan(&d) {
                    if seen.insert(p.clone()) {
                        let info = with_folder_tags(player::read_info(&p), &p, &d);
                        songs.lock().unwrap().push(Song { path: p, info });
                    }
                }
            }
            *scanning.lock().unwrap() = false;
        });
    }

    fn info_of(&self, p: &Path) -> Info {
        self.songs.lock().unwrap().iter().find(|s| s.path == p).map(|s| s.info.clone()).unwrap_or_else(|| Info { title: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), ..Info::default() })
    }

    fn matches(&self, i: &Info) -> bool {
        let f = self.filter.trim().to_lowercase();
        f.is_empty() || [&i.title, &i.artist, &i.album].iter().any(|t| t.to_lowercase().contains(&f))
    }

    /// Linhas visíveis (fonte, navegador, busca e ordem): (caminho, tags).
    fn rows(&self) -> Vec<(PathBuf, Info)> {
        let ordered = |paths: &[PathBuf]| -> Vec<(PathBuf, Info)> {
            let songs = self.songs.lock().unwrap();
            let by: HashMap<&Path, &Info> = songs.iter().map(|s| (s.path.as_path(), &s.info)).collect();
            paths
                .iter()
                .map(|p| (p.clone(), by.get(p.as_path()).map(|i| (*i).clone()).unwrap_or_else(|| Info { title: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), ..Info::default() })))
                .filter(|(_, i)| self.matches(i))
                .collect()
        };
        match self.source {
            Source::Recent => return ordered(&self.recent),
            Source::List(i) => return self.lists.get(i).map(|l| ordered(&l.items)).unwrap_or_default(),
            Source::Library => {}
        }
        let browse = self.settings.browser;
        let songs = self.songs.lock().unwrap();
        let mut v: Vec<(PathBuf, Info)> = songs
            .iter()
            .filter(|s| self.matches(&s.info))
            .filter(|s| !browse || self.artist.as_ref().is_none_or(|a| *a == s.info.artist))
            .filter(|s| !browse || self.album.as_ref().is_none_or(|a| *a == s.info.album))
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

    /// Artistas (ou álbuns de um artista) da biblioteca, em ordem.
    fn names(&self, album: bool) -> Vec<String> {
        let songs = self.songs.lock().unwrap();
        let set: BTreeSet<String> = songs
            .iter()
            .filter(|s| !album || self.artist.as_ref().is_none_or(|a| *a == s.info.artist))
            .map(|s| if album { s.info.album.clone() } else { s.info.artist.clone() })
            .filter(|n| !n.is_empty())
            .collect();
        let mut v: Vec<String> = set.into_iter().collect();
        v.sort_by_key(|n| n.to_lowercase());
        v
    }

    fn play(&mut self, p: PathBuf, rows: &[(PathBuf, Info)]) {
        self.queue = rows.iter().map(|r| r.0.clone()).collect();
        if self.settings.shuffle {
            self.queue.retain(|q| *q != p);
            shuffle(&mut self.queue);
            self.queue.insert(0, p.clone());
        }
        self.selected = Some(p.clone());
        let _ = self.tx.send(Cmd::Play(p));
    }

    /// A próxima (ou anterior) da fila; no fim, para, ou volta ao começo com Repetir Todas.
    fn step(&mut self, delta: isize) {
        let cur = self.status.lock().unwrap().playing.clone();
        let Some(cur) = cur.or_else(|| self.selected.clone()) else { return };
        if self.queue.is_empty() {
            let rows = self.rows();
            self.queue = rows.into_iter().map(|r| r.0).collect();
        }
        let n = self.queue.len() as isize;
        let Some(i) = self.queue.iter().position(|p| *p == cur) else { return };
        let mut j = i as isize + delta;
        if (j < 0 || j >= n) && self.settings.repeat == Repeat::All && n > 0 {
            j = j.rem_euclid(n);
        }
        if j >= 0 && j < n {
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

    fn set_volume(&mut self, v: f32) {
        self.settings.volume = v.clamp(0.0, 1.0);
        let _ = self.tx.send(Cmd::Volume(self.settings.volume));
    }

    /// Começou outra música: entra nas recentes e, com a janela atrás, vira aviso.
    fn track_changed(&mut self, p: &Path, active: bool) {
        self.recent.retain(|r| r != p);
        self.recent.insert(0, p.to_path_buf());
        self.recent.truncate(MAX_RECENT);
        write_m3u(&data_dir().join("recentes.m3u"), &self.recent);
        if self.settings.notify && !active {
            let i = self.info_of(p);
            let body = [i.artist.as_str(), i.album.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" — ");
            let _ = Command::new("notify-send").args(["-a", &tr!("Música"), "-i", "audio-x-generic", &i.title, &body]).spawn();
        }
    }

    fn new_list(&mut self, with: Option<PathBuf>) {
        let name = free_name(&self.lists, &tr!("Lista sem título"));
        let l = Playlist { name: name.clone(), items: with.into_iter().collect() };
        write_m3u(&list_path(&name), &l.items);
        self.lists.push(l);
        self.lists.sort_by_key(|l| l.name.to_lowercase());
        let i = self.lists.iter().position(|l| l.name == name).unwrap_or(0);
        self.source = Source::List(i);
        // O campo começa vazio: o que for digitado vira o nome (vazio = fica o padrão).
        self.renaming = Some((i, String::new()));
    }

    fn act(&mut self, a: &str, rows: &[(PathBuf, Info)], ui: &mut Ui) {
        match a {
            "toggle" => self.toggle(rows),
            "next" => self.step(1),
            "prev" => {
                // Mais de 3 s tocados: volta ao começo; senão, a anterior.
                let pos = self.status.lock().unwrap().pos;
                if pos > 3.0 {
                    let _ = self.tx.send(Cmd::Seek(0.0));
                } else {
                    self.step(-1)
                }
            }
            "stop" => {
                let _ = self.tx.send(Cmd::Stop);
            }
            "play-sel" => {
                if let Some(p) = self.selected.clone() {
                    self.play(p, rows);
                }
            }
            "shuffle" => {
                self.settings.shuffle = !self.settings.shuffle;
                // A fila acompanha: a música atual continua, o resto muda de ordem.
                if let Some(cur) = self.status.lock().unwrap().playing.clone() {
                    let mut q: Vec<PathBuf> = rows.iter().map(|r| r.0.clone()).collect();
                    if self.settings.shuffle {
                        q.retain(|p| *p != cur);
                        shuffle(&mut q);
                        q.insert(0, cur);
                    }
                    self.queue = q;
                }
                self.settings.save();
            }
            "repeat" => {
                self.settings.repeat = match self.settings.repeat { Repeat::Off => Repeat::All, Repeat::All => Repeat::One, Repeat::One => Repeat::Off };
                self.settings.save();
            }
            "repeat-off" | "repeat-all" | "repeat-one" => {
                self.settings.repeat = match a { "repeat-all" => Repeat::All, "repeat-one" => Repeat::One, _ => Repeat::Off };
                self.settings.save();
            }
            "vol-up" => self.set_volume(self.settings.volume + 0.1),
            "vol-down" => self.set_volume(self.settings.volume - 0.1),
            "browser" => {
                self.settings.browser = !self.settings.browser;
                self.settings.save();
            }
            "new-list" => self.new_list(None),
            "new-list-sel" => {
                let s = self.selected.clone();
                self.new_list(s);
            }
            "rename-list" => {
                if let Source::List(i) = self.source {
                    if let Some(l) = self.lists.get(i) {
                        self.renaming = Some((i, l.name.clone()));
                    }
                }
            }
            "delete-list" => {
                if let Source::List(i) = self.source {
                    if i < self.lists.len() {
                        let l = self.lists.remove(i);
                        let _ = std::fs::remove_file(list_path(&l.name));
                        self.source = Source::Library;
                    }
                }
            }
            a if a.starts_with("add-to:") => {
                if let (Some(i), Some(p)) = (a[7..].parse::<usize>().ok(), self.selected.clone()) {
                    if let Some(l) = self.lists.get_mut(i) {
                        if !l.items.contains(&p) {
                            l.items.push(p);
                            write_m3u(&list_path(&l.name), &l.items);
                        }
                    }
                }
            }
            "remove-from" => {
                if let (Source::List(i), Some(p)) = (self.source, self.selected.clone()) {
                    if let Some(l) = self.lists.get_mut(i) {
                        l.items.retain(|x| *x != p);
                        write_m3u(&list_path(&l.name), &l.items);
                    }
                } else if let (Source::Recent, Some(p)) = (self.source, self.selected.clone()) {
                    self.recent.retain(|x| *x != p);
                    write_m3u(&data_dir().join("recentes.m3u"), &self.recent);
                }
            }
            "clear-recent" => {
                self.recent.clear();
                write_m3u(&data_dir().join("recentes.m3u"), &self.recent);
            }
            "add-folder" => self.folder_fb = Some(FileBrowser::folder(Some(music_dir()))),
            "rescan" => self.rescan(),
            "reveal" => {
                if let Some(dir) = self.selected.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)) {
                    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-files".into());
                    let _ = Command::new(exe).arg(dir).spawn();
                }
            }
            "prefs" => ui.open_window(PREFS_WIN, WindowOptions { title: tr!("Preferências da Música").into(), app_id: "glove.Musica".into(), size: (460, 330), fixed: true, min_size: None }),
            "close" => ui.close = true,
            _ => {}
        }
    }

    /// Botão redondo de gel com um desenho (◀◀, ▶, ❚❚, ▶▶).
    fn round(ui: &mut Ui, cx: f32, cy: f32, r: f32, draw: impl Fn(&mut Canvas, f32, f32, Color)) -> bool {
        let hit = ui.hit(Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r));
        let base = if hit.down { rgba(190, 190, 196, 255) } else { rgba(244, 244, 246, 255) };
        ui.c.fill(circle(cx, cy + 1.0, r), &solid(rgba(0, 0, 0, 50)));
        gel_circle(ui.c, cx, cy, r, base);
        ui.c.stroke(circle(cx, cy, r), &solid(rgba(110, 110, 118, 255)), 1.0);
        draw(ui.c, cx, cy, rgba(40, 40, 46, 255));
        hit.clicked
    }

    /// Botão pequeno do rodapé (aceso quando ligado), com um desenho.
    fn foot_button(ui: &mut Ui, x: f32, y: f32, on: bool, draw: impl Fn(&mut Canvas, f32, f32, Color)) -> bool {
        let r = Rect::new(x, y, 28.0, 18.0);
        let hit = ui.hit(r);
        let (top, bot) = if hit.down { (rgba(190, 190, 196, 255), rgba(214, 214, 218, 255)) } else { (rgba(250, 250, 252, 255), rgba(214, 214, 220, 255)) };
        ui.c.fill(rrect(r.x, r.y, r.w, r.h, 4.0), &vgrad(r.y, r.y + r.h, &[(0.0, top), (1.0, bot)]));
        ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 4.0), &solid(rgba(120, 120, 128, 255)), 1.0);
        let ink = if on { rgba(40, 110, 220, 255) } else { rgba(80, 80, 88, 255) };
        draw(ui.c, r.x + r.w / 2.0, r.y + r.h / 2.0, ink);
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
            None => center(ui, tr!("Glove Música"), r.y + r.h / 2.0 + 5.0, 13.0, true),
            Some(p) => {
                let info = self.info_of(&p);
                center(ui, &info.title, r.y + 16.0, 12.0, true);
                let sub = [info.artist.as_str(), info.album.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" — ");
                center(ui, &sub, r.y + 31.0, 11.0, false);
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

    /// Barra lateral: a Biblioteca, as recentes e as listas.
    fn sidebar(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(214, 221, 230, 255)));
        ui.c.vline(r.x + r.w - 1.0, r.y, r.h, rgba(150, 156, 166, 255));
        let mut y = r.y + 8.0;
        let head = |ui: &mut Ui, t: &str, y: f32| ui.fonts.draw(ui.c, &t.to_uppercase(), r.x + 10.0, y + 12.0, 10.0, true, rgba(110, 118, 132, 255));
        head(ui, tr!("Biblioteca"), y);
        y += 18.0;
        let mut rows: Vec<(Source, String)> = vec![(Source::Library, tr!("Músicas").to_string())];
        let lib_rows = 1;
        rows.push((Source::Recent, tr!("Tocadas Recentemente").to_string()));
        rows.extend(self.lists.iter().enumerate().map(|(i, l)| (Source::List(i), l.name.clone())));
        for (k, (src, name)) in rows.iter().enumerate() {
            if k == lib_rows {
                y += 6.0;
                head(ui, tr!("Listas"), y);
                y += 18.0;
            }
            let row = Rect::new(r.x, y, r.w - 1.0, 20.0);
            if row.y + row.h > r.y + r.h {
                break;
            }
            let sel = self.source == *src;
            if sel {
                ui.c.fill_rect(row.x, row.y, row.w, row.h, &vgrad(row.y, row.y + row.h, &[(0.0, rgba(120, 160, 225, 255)), (1.0, rgba(70, 120, 205, 255))]));
            }
            let ink = if sel { rgba(255, 255, 255, 255) } else { rgba(30, 32, 40, 255) };
            // Ícone: nota (biblioteca), relógio (recentes) ou lista.
            let (ix, iy) = (row.x + 14.0, row.y + 10.0);
            match src {
                Source::Library => {
                    ui.c.fill(oval(ix - 5.0, iy + 1.0, 6.0, 5.0), &solid(ink));
                    ui.c.fill_rect(ix, iy - 7.0, 1.5, 10.0, &solid(ink));
                    ui.c.fill_rect(ix, iy - 7.0, 5.0, 2.0, &solid(ink));
                }
                Source::Recent => {
                    ui.c.stroke(circle(ix, iy, 5.5), &solid(ink), 1.3);
                    ui.c.fill_rect(ix - 0.6, iy - 4.0, 1.3, 4.5, &solid(ink));
                    ui.c.fill_rect(ix - 0.6, iy - 0.6, 3.5, 1.3, &solid(ink));
                }
                Source::List(_) => {
                    for l in 0..3 {
                        ui.c.fill_rect(ix - 5.0, iy - 4.0 + l as f32 * 3.5, 10.0, 1.5, &solid(ink));
                    }
                }
            }
            if let (Source::List(i), Some((ri, text))) = (src, &mut self.renaming) {
                if *i == *ri {
                    let mut t = std::mem::take(text);
                    let (_, enter) = ui.text_field_in(Rect::new(row.x + 26.0, row.y + 1.0, row.w - 32.0, 18.0), &mut t, true);
                    let esc = ui.keys().iter().any(|k| k.key == Key::Escape);
                    if enter || esc {
                        let i = *i;
                        self.renaming = None;
                        let new = t.trim().to_string();
                        let new = if new.is_empty() { self.lists[i].name.clone() } else { new };
                        if enter && !new.is_empty() && !self.lists.iter().enumerate().any(|(j, l)| j != i && l.name.eq_ignore_ascii_case(&new)) {
                            let old = list_path(&self.lists[i].name);
                            self.lists[i].name = new.clone();
                            let _ = std::fs::remove_file(old);
                            write_m3u(&list_path(&new), &self.lists[i].items);
                        }
                    } else if let Some((_, text)) = &mut self.renaming {
                        *text = t;
                    }
                    y += 20.0;
                    continue;
                }
            }
            let label = ui.fonts.ellipsize(name, 12.0, false, row.w - 34.0);
            ui.fonts.draw(ui.c, &label, row.x + 26.0, row.y + 14.5, 12.0, false, ink);
            let hit = ui.hit(row);
            if hit.pressed {
                if self.source != *src {
                    self.source = *src;
                    self.selected = None;
                }
                if hit.double {
                    if let Source::List(i) = src {
                        self.renaming = Some((*i, name.clone()));
                    }
                }
            }
            y += 20.0;
        }
    }

    /// Navegador: Artistas e Álbuns, lado a lado.
    fn browser(&mut self, ui: &mut Ui, r: Rect) {
        let artists = self.names(false);
        let albums = self.names(true);
        let half = (r.w / 2.0).floor();
        for (col, (title, names)) in [(tr!("Artista"), &artists), (tr!("Álbum"), &albums)].into_iter().enumerate() {
            let cr = Rect::new(r.x + col as f32 * half, r.y, if col == 0 { half } else { r.w - half }, r.h);
            ui.c.fill_rect(cr.x, cr.y, cr.w, cr.h, &solid(rgba(255, 255, 255, 255)));
            ui.c.fill_rect(cr.x, cr.y, cr.w, 18.0, &vgrad(cr.y, cr.y + 18.0, &[(0.0, rgba(246, 246, 248, 255)), (1.0, rgba(220, 220, 226, 255))]));
            ui.c.hline(cr.x, cr.y + 18.0, cr.w, rgba(160, 160, 168, 255));
            let tw = ui.text_width(title, 11.0, true);
            ui.fonts.draw(ui.c, title, cr.x + (cr.w - tw) / 2.0, cr.y + 13.0, 11.0, true, rgba(40, 40, 46, 255));
            if col == 1 {
                ui.c.vline(cr.x, cr.y, cr.h, rgba(160, 160, 168, 255));
            }
            let all = if col == 0 { tr!("Todos ({} artistas)", names.len()) } else { tr!("Todos ({} álbuns)", names.len()) };
            let cur = if col == 0 { self.artist.clone() } else { self.album.clone() };
            let mut y = cr.y + 19.0;
            for (k, n) in std::iter::once(None).chain(names.iter().map(Some)).enumerate() {
                if y + 17.0 > cr.y + cr.h {
                    break;
                }
                let row = Rect::new(cr.x + 1.0, y, cr.w - 2.0, 17.0);
                let sel = n.cloned() == cur;
                if sel {
                    ui.c.fill_rect(row.x, row.y, row.w, row.h, &solid(rgba(56, 117, 215, 255)));
                } else if k % 2 == 1 {
                    ui.c.fill_rect(row.x, row.y, row.w, row.h, &solid(rgba(237, 243, 254, 255)));
                }
                let t = n.cloned().unwrap_or_else(|| all.to_string());
                let t = ui.fonts.ellipsize(&t, 11.0, false, row.w - 12.0);
                ui.fonts.draw(ui.c, &t, row.x + 6.0, row.y + 12.5, 11.0, false, if sel { rgba(255, 255, 255, 255) } else { rgba(20, 20, 24, 255) });
                if ui.hit(row).pressed {
                    if col == 0 {
                        self.artist = n.cloned();
                        self.album = None;
                    } else {
                        self.album = n.cloned();
                    }
                }
                y += 17.0;
            }
        }
        ui.c.hline(r.x, r.y + r.h - 1.0, r.w, rgba(150, 150, 156, 255));
    }

    fn prefs_ui(&mut self, ui: &mut Ui) {
        let before = self.settings.clone();
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(ui.theme.body_b));
        ui.c.pinstripes(0.0, 0.0, w, h, ui.theme.body_a, ui.theme.body_b);
        let mut remove = None;
        let mut add = false;
        let mut rescan = false;
        let dirs = self.dirs();
        ui.area(Rect::new(20.0, 16.0, w - 40.0, h - 32.0), |ui| {
            ui.label_ex(tr!("Pastas da biblioteca"), 13.0, true, rgba(20, 20, 24, 255));
            ui.space(6.0);
            for (k, d) in dirs.iter().enumerate() {
                let t = ui.fonts.ellipsize(&d.display().to_string(), 12.0, false, w - 150.0);
                let (x, y) = ui.cursor();
                ui.fonts.draw(ui.c, &t, x, y + 15.0, 12.0, false, rgba(20, 20, 24, 255));
                if k > 0 {
                    ui.area(Rect::new(w - 40.0 - 80.0, y, 80.0, 22.0), |ui| {
                        if ui.small_button(tr!("Remover")) {
                            remove = Some(k - 1);
                        }
                    });
                } else {
                    ui.fonts.draw(ui.c, tr!("(pasta Música)"), w - 40.0 - 100.0, y + 15.0, 11.0, false, rgba(110, 110, 118, 255));
                }
                ui.alloc(w - 40.0, 24.0);
            }
            ui.space(4.0);
            let (x, y) = ui.cursor();
            ui.area(Rect::new(x, y, 170.0, 24.0), |ui| add = ui.button(tr!("Adicionar Pasta…")));
            ui.area(Rect::new(x + 180.0, y, 170.0, 24.0), |ui| rescan = ui.button(tr!("Ler de Novo")));
            ui.alloc(w - 40.0, 30.0);
            ui.space(12.0);
            ui.label_ex(tr!("Ao tocar"), 13.0, true, rgba(20, 20, 24, 255));
            ui.space(6.0);
            ui.checkbox(&mut self.settings.notify, tr!("Avisar a música nova quando a janela estiver atrás"));
            ui.checkbox(&mut self.settings.shuffle, tr!("Ordem aleatória"));
            let mut rep = match self.settings.repeat { Repeat::Off => 0, Repeat::All => 1, Repeat::One => 2 };
            ui.space(4.0);
            ui.label(tr!("Repetir:"));
            if ui.popup(&mut rep, &[tr!("Não repetir").to_string(), tr!("Todas").to_string(), tr!("Uma").to_string()], 180.0) {
                self.settings.repeat = [Repeat::Off, Repeat::All, Repeat::One][rep];
            }
        });
        if let Some(i) = remove {
            self.settings.folders.remove(i);
            self.rescan();
        }
        if add {
            self.folder_fb = Some(FileBrowser::folder(Some(music_dir())));
        }
        if rescan {
            self.rescan();
        }
        if let Some(fb) = &mut self.folder_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.folder_fb = None;
                if let Choice::Picked(p) = c {
                    if !self.dirs().contains(&p) {
                        self.settings.folders.push(p);
                        self.rescan();
                    }
                }
                ui.redraw = true;
            }
        }
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

/// Desenho do Aleatório: duas setas que se cruzam.
fn shuffle_glyph(c: &mut Canvas, x: f32, y: f32, col: Color) {
    let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
    pb.move_to(x - 8.0, y - 4.0);
    pb.cubic_to(x - 2.0, y - 4.0, x + 1.0, y + 4.0, x + 6.0, y + 4.0);
    pb.move_to(x - 8.0, y + 4.0);
    pb.cubic_to(x - 2.0, y + 4.0, x + 1.0, y - 4.0, x + 6.0, y - 4.0);
    c.stroke(pb.finish(), &solid(col), 1.6);
    tri(c, x + 7.0, y - 4.0, 4.5, 1.0, col);
    tri(c, x + 7.0, y + 4.0, 4.5, 1.0, col);
}

/// Desenho do Repetir: a volta com a seta (e "1" para repetir uma).
fn repeat_glyph(c: &mut Canvas, x: f32, y: f32, one: bool, col: Color) {
    c.stroke(rrect(x - 7.0, y - 4.5, 14.0, 9.0, 4.0), &solid(col), 1.6);
    tri(c, x + 2.0, y - 4.5, 4.5, 1.0, col);
    if one {
        c.fill(circle(x + 8.0, y + 4.0, 4.0), &solid(col));
        c.fill_rect(x + 7.4, y + 1.8, 1.4, 4.4, &solid(rgba(255, 255, 255, 255)));
    }
}

impl App for Musica {
    fn ui(&mut self, ui: &mut Ui) {
        if ui.window_id() == PREFS_WIN {
            self.prefs_ui(ui);
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        // A música acabou: a mesma (Repetir Uma) ou a próxima.
        let (ended, now) = {
            let mut s = self.status.lock().unwrap();
            (std::mem::take(&mut s.ended), s.playing.clone())
        };
        if ended {
            match (self.settings.repeat, now.clone().or(self.last_playing.clone())) {
                (Repeat::One, Some(p)) => {
                    let _ = self.tx.send(Cmd::Play(p));
                }
                _ => self.step(1),
            }
        }
        if now.is_some() && now != self.last_playing {
            let p = now.clone().unwrap();
            self.track_changed(&p, ui.window_active());
        }
        if now.is_some() {
            self.last_playing = now;
        }
        let rows = self.rows();
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.renaming.is_none() && self.folder_fb.is_none() {
            for k in ui.keys() {
                let a = match (k.key, k.combo.as_str()) {
                    (_, "Ctrl+Right") => "next",
                    (_, "Ctrl+Left") => "prev",
                    (_, "Ctrl+W") => "close",
                    (_, "Ctrl+N") => "new-list",
                    (_, "Ctrl+B") => "browser",
                    (_, "Ctrl+Up") => "vol-up",
                    (_, "Ctrl+Down") => "vol-down",
                    (_, "Ctrl+R") => "reveal",
                    (_, "Ctrl+comma") => "prefs",
                    (_, "Ctrl+period") => "stop",
                    (Key::Delete, _) | (Key::Backspace, _) if self.source != Source::Library && self.filter.is_empty() => "remove-from",
                    (Key::Enter, _) => "play-sel",
                    _ if k.combo == "space" || k.combo == "Space" => "toggle",
                    _ => continue,
                };
                acts.push(a.into());
            }
        }
        // Barra de cima em metal: tocador, volume, visor e busca.
        ui.c.brushed_metal(0.0, 0.0, w, TOP_H, rgba(226, 226, 228, 255), rgba(184, 184, 188, 255));
        ui.c.hline(0.0, TOP_H - 1.0, w, rgba(120, 120, 126, 255));
        let cy = 30.0;
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
        let mut vol = self.settings.volume;
        ui.area(Rect::new(128.0, cy - 9.0, 110.0, 24.0), |ui| {
            if ui.slider_ticks(&mut vol, 0.0, 1.0, 100.0, &Ticks { count: 0, labels: &[], snap: false }, true) {
                self.set_volume(vol);
            }
        });
        let dw = (w - 250.0 - 190.0).max(200.0);
        self.display(ui, Rect::new(250.0, 8.0, dw, TOP_H - 18.0));
        let sw = (w - 250.0 - dw - 24.0).max(120.0);
        ui.search_field_in(Rect::new(w - sw - 12.0, cy - SEARCH_H / 2.0, sw, SEARCH_H), &mut self.filter, tr!("Buscar"));

        // Barra lateral, navegador e a tabela.
        let body_h = h - TOP_H - FOOT_H;
        self.sidebar(ui, Rect::new(0.0, TOP_H, SIDE_W, body_h));
        let mut ty = TOP_H;
        if self.settings.browser && self.source == Source::Library {
            self.browser(ui, Rect::new(SIDE_W, TOP_H, w - SIDE_W, BROWSER_H));
            ty += BROWSER_H;
        }
        let cols = [Column::new(tr!("Nome"), 0.36), Column::new(tr!("Artista"), 0.24), Column::new(tr!("Álbum"), 0.28), Column::new(tr!("Duração"), 0.12).right()];
        let cur = self.status.lock().unwrap().playing.clone();
        let trows: Vec<TableRow> = rows
            .iter()
            .map(|(p, i)| TableRow { cells: vec![format!("{}{}", if Some(p) == cur.as_ref() { "♪ " } else { "" }, i.title), i.artist.clone(), i.album.clone(), mmss(i.secs)], ..TableRow::default() })
            .collect();
        let sel = self.selected.as_ref().and_then(|s| rows.iter().position(|r| &r.0 == s));
        let area = Rect::new(SIDE_W, ty, w - SIDE_W, h - ty - FOOT_H);
        let resp = ui.table(area, &cols, &trows, sel, &mut self.table, &TableStyle::default());
        if let Some(i) = resp.pressed {
            self.selected = Some(rows[i].0.clone());
        }
        if let Some(i) = resp.double {
            self.play(rows[i].0.clone(), &rows);
        }
        if rows.is_empty() {
            let msg = match self.source {
                _ if !self.filter.trim().is_empty() => tr!("Nenhuma música encontrada.").to_string(),
                Source::Library if *self.scanning.lock().unwrap() => tr!("Lendo a biblioteca…").to_string(),
                Source::Library => tr!("Nenhuma música em {}. Ponha MP3, FLAC, OGG ou WAV nessa pasta.", music_dir().display()).to_string(),
                Source::Recent => tr!("As músicas que você tocar aparecem aqui.").to_string(),
                Source::List(_) => tr!("Lista vazia: escolha uma música e use Arquivo ▸ Adicionar à Lista.").to_string(),
            };
            let msg = ui.fonts.ellipsize(&msg, 12.0, false, area.w - 40.0);
            let tw = ui.text_width(&msg, 12.0, false);
            ui.fonts.draw(ui.c, &msg, area.x + (area.w - tw) / 2.0, area.y + area.h / 2.0, 12.0, false, rgba(130, 130, 136, 255));
        }
        // Rodapé: nova lista, aleatório e repetir; no meio, quantas e quanto tempo.
        let fy = h - FOOT_H;
        ui.c.fill_rect(0.0, fy, w, FOOT_H, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        if Self::foot_button(ui, 8.0, fy + 3.0, false, |c, x, y, col| {
            c.fill_rect(x - 5.0, y - 0.8, 10.0, 1.6, &solid(col));
            c.fill_rect(x - 0.8, y - 5.0, 1.6, 10.0, &solid(col));
        }) {
            acts.push("new-list".into());
        }
        let shuffle_on = self.settings.shuffle;
        if Self::foot_button(ui, 44.0, fy + 3.0, shuffle_on, shuffle_glyph) {
            acts.push("shuffle".into());
        }
        let rep = self.settings.repeat;
        if Self::foot_button(ui, 76.0, fy + 3.0, rep != Repeat::Off, |c, x, y, col| repeat_glyph(c, x, y, rep == Repeat::One, col)) {
            acts.push("repeat".into());
        }
        let total: f32 = rows.iter().map(|r| r.1.secs).sum();
        let t = tr!("{} músicas, {}", rows.len(), mmss(total));
        let tw = ui.text_width(&t, 11.0, false);
        ui.fonts.draw(ui.c, &t, SIDE_W + (w - SIDE_W - tw) / 2.0, fy + 16.0, 11.0, false, rgba(60, 60, 66, 255));
        if let Some(e) = self.status.lock().unwrap().error.clone() {
            let e = ui.fonts.ellipsize(&tr!("Não deu para tocar: {}", e), 10.0, false, 220.0);
            let ew = ui.text_width(&e, 10.0, false);
            ui.fonts.draw(ui.c, &e, w - ew - 10.0, fy + 16.0, 10.0, false, rgba(180, 40, 30, 255));
        }

        for a in acts {
            self.act(&a, &rows, ui);
        }
        if let (Some(fb), true) = (&mut self.folder_fb, ui.window_id() != PREFS_WIN) {
            if let Some(c) = ui.file_sheet(fb) {
                self.folder_fb = None;
                if let Choice::Picked(p) = c {
                    if !self.dirs().contains(&p) {
                        self.settings.folders.push(p);
                        self.settings.save();
                        self.rescan();
                    }
                }
                ui.redraw = true;
            }
        }
        // Enquanto toca (ou lê a biblioteca), o visor anda.
        if playing || *self.scanning.lock().unwrap() {
            ui.wake_after(Duration::from_millis(500));
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let (playing, paused) = {
            let s = self.status.lock().unwrap();
            (s.playing.is_some(), s.paused)
        };
        let has_sel = self.selected.is_some();
        let on_list = matches!(self.source, Source::List(_));
        let mut add_to: Vec<MenuItem> = self.lists.iter().enumerate().map(|(i, l)| MenuItem::new(&format!("add-to:{i}"), &l.name, "").enabled(has_sel)).collect();
        if !add_to.is_empty() {
            add_to.push(MenuItem::Separator);
        }
        add_to.push(MenuItem::new("new-list-sel", tr!("Nova Lista com a Música"), "").enabled(has_sel));
        let rep = self.settings.repeat;
        vec![
            Menu::app(vec![MenuItem::new("prefs", tr!("Preferências…"), "Ctrl+comma")]),
            Menu::new(
                tr!("Arquivo"),
                vec![
                    MenuItem::new("new-list", tr!("Nova Lista"), "Ctrl+N"),
                    MenuItem::submenu(tr!("Adicionar à Lista"), add_to),
                    MenuItem::new("remove-from", if self.source == Source::Recent { tr!("Tirar das Recentes") } else { tr!("Tirar da Lista") }, "Delete").enabled(has_sel && self.source != Source::Library),
                    MenuItem::new("rename-list", tr!("Renomear Lista"), "").enabled(on_list),
                    MenuItem::new("delete-list", tr!("Apagar Lista"), "").enabled(on_list),
                    MenuItem::new("clear-recent", tr!("Limpar as Recentes"), "").enabled(!self.recent.is_empty()),
                    MenuItem::Separator,
                    MenuItem::new("add-folder", tr!("Adicionar Pasta à Biblioteca…"), ""),
                    MenuItem::new("rescan", tr!("Ler a Biblioteca de Novo"), ""),
                    MenuItem::new("reveal", tr!("Mostrar no Arquivos"), "Ctrl+R").enabled(has_sel),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
                ],
            ),
            Menu::new(tr!("Exibir"), vec![MenuItem::new("browser", tr!("Mostrar Navegador"), "Ctrl+B").checked(self.settings.browser)]),
            Menu::new(
                tr!("Controles"),
                vec![
                    MenuItem::new("toggle", if playing && !paused { tr!("Pausar") } else { tr!("Tocar") }, "space"),
                    MenuItem::new("stop", tr!("Parar"), "Ctrl+period").enabled(playing),
                    MenuItem::Separator,
                    MenuItem::new("next", tr!("Próxima Música"), "Ctrl+Right"),
                    MenuItem::new("prev", tr!("Música Anterior"), "Ctrl+Left"),
                    MenuItem::Separator,
                    MenuItem::new("shuffle", tr!("Aleatório"), "").checked(self.settings.shuffle),
                    MenuItem::submenu(
                        tr!("Repetir"),
                        vec![
                            MenuItem::new("repeat-off", tr!("Não Repetir"), "").checked(rep == Repeat::Off),
                            MenuItem::new("repeat-all", tr!("Repetir Todas"), "").checked(rep == Repeat::All),
                            MenuItem::new("repeat-one", tr!("Repetir Uma"), "").checked(rep == Repeat::One),
                        ],
                    ),
                    MenuItem::Separator,
                    MenuItem::new("vol-up", tr!("Aumentar Volume"), "Ctrl+Up"),
                    MenuItem::new("vol-down", tr!("Diminuir Volume"), "Ctrl+Down"),
                ],
            ),
        ]
    }

    fn close_requested(&mut self) -> bool {
        let _ = self.tx.send(Cmd::Stop);
        self.settings.save();
        true
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Música").into(), app_id: "glove.Musica".into(), size: (900, 520), fixed: false, min_size: Some((640, 360)) };
    if let Err(e) = run(opts, Musica::new()) {
        eprintln!("glove-musica: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempos() {
        assert_eq!(mmss(187.4), "3:07");
        assert_eq!(mmss(3750.0), "1:02:30");
    }

    #[test]
    fn preferencias_vao_e_voltam() {
        let s = Settings { volume: 0.3, shuffle: true, repeat: Repeat::One, browser: true, notify: false, folders: vec![PathBuf::from("/mnt/musicas")] };
        assert_eq!(Settings::parse(&s.text()), s);
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn listas_m3u_e_nomes() {
        let dir = std::env::temp_dir().join(format!("glove-musica-{}", std::process::id()));
        let f = dir.join("teste.m3u");
        write_m3u(&f, &[PathBuf::from("/a/b.mp3"), PathBuf::from("/c d/e.flac")]);
        assert_eq!(read_m3u(&f), vec![PathBuf::from("/a/b.mp3"), PathBuf::from("/c d/e.flac")]);
        let _ = std::fs::remove_dir_all(&dir);
        let lists = vec![Playlist { name: "Lista".into(), items: vec![] }, Playlist { name: "Lista 2".into(), items: vec![] }];
        assert_eq!(free_name(&lists, "Lista"), "Lista 3");
        assert_eq!(free_name(&[], "Lista"), "Lista");
    }

    #[test]
    fn etiquetas_pelas_pastas() {
        let root = Path::new("/m");
        let i = with_folder_tags(Info { title: "X".into(), ..Info::default() }, Path::new("/m/Banda/Disco/x.mp3"), root);
        assert_eq!((i.artist.as_str(), i.album.as_str()), ("Banda", "Disco"));
        let i = with_folder_tags(Info { artist: "Tag".into(), ..Info::default() }, Path::new("/m/Banda/x.mp3"), root);
        assert_eq!((i.artist.as_str(), i.album.as_str()), ("Tag", ""));
        let i = with_folder_tags(Info::default(), Path::new("/m/x.mp3"), root);
        assert_eq!(i.artist, "");
    }

    #[test]
    fn aleatorio_mantem_tudo() {
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v);
        let mut s = v.clone();
        s.sort();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
    }
}
