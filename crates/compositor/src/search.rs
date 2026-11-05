//! Busca Rápida (como a do 10.4): lupa no canto da barra ou Super+Espaço.
//! Acha apps instalados (`.desktop` do XDG e os do Glove), painéis das
//! Preferências e arquivos da pasta pessoal (índice leve feito numa thread).
//! Enter abre o primeiro resultado.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use glove_paint::{
    chrome::{self, QsRow},
    icons::Icon,
};
use smithay::{
    reexports::calloop::{
        channel::{channel, Event as ChannelEvent},
        timer::{TimeoutAction, Timer},
    },
    utils::{Logical, Point},
};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// Índice de arquivos: no máximo isso de itens, até essa profundidade.
const MAX_FILES: usize = 30_000;
const MAX_DEPTH: usize = 6;
/// Índice mais velho que isso é refeito ao abrir a busca.
const REINDEX: Duration = Duration::from_secs(300);
const PER_SECTION: usize = 5;
/// Linhas no painel, no máximo.
const MAX_ROWS: usize = 18;

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub name: String,
    /// Palavras extras para achar (nome genérico, palavras-chave), já normalizadas.
    pub keywords: String,
    pub exec: String,
    pub icon: Icon,
    /// app_id do Glove (abre pelo Dock, trazendo para frente se já estiver aberto).
    pub glove_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    App(App),
    Prefs(&'static str),
    Path(PathBuf),
}

struct FileEntry {
    norm: String,
    path: PathBuf,
    dir: bool,
}

#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub query: String,
    pub selected: Option<usize>,
    pub results: Vec<(QsRow, Target)>,
    apps: Vec<App>,
    files: Vec<FileEntry>,
    indexed: Option<Instant>,
    pub indexing: bool,
    opened: Option<Instant>,
    key: Option<(String, Option<usize>, usize, bool, bool, u64)>,
    pub tex: Option<Tex>,
    pub size: (i32, i32),
}

/// Minúsculas e sem acentos, para comparar ("Preferências" acha com "preferencias").
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Nota do nome para a busca (0 = não serve): começo do nome > começo de palavra > meio.
pub fn score(name_norm: &str, q: &str) -> u32 {
    if q.is_empty() {
        return 0;
    }
    if name_norm == q {
        return 100;
    }
    if name_norm.starts_with(q) {
        return 80;
    }
    if name_norm.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(q)) {
        return 60;
    }
    if name_norm.contains(q) {
        return 30;
    }
    0
}

/// Tira os códigos de campo do Exec (%f, %U…).
fn clean_exec(exec: &str) -> String {
    let mut out = String::new();
    let mut it = exec.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            match it.next() {
                Some('%') => out.push('%'),
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lê um `.desktop` (só o grupo [Desktop Entry]). None se não deve aparecer.
pub fn parse_desktop(text: &str) -> Option<App> {
    let mut in_entry = false;
    let mut get = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            get.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    let val = |k: &str| get.get(k).map(String::as_str);
    let localized = |k: &str| val(&format!("{k}[pt_BR]")).or(val(&format!("{k}[pt]"))).or(val(k));
    if val("Type") != Some("Application") || val("NoDisplay") == Some("true") || val("Hidden") == Some("true") || val("Terminal") == Some("true") {
        return None;
    }
    // Painéis feitos só para outro desktop (ex.: configurações do GNOME) ficam de fora.
    if let Some(only) = val("OnlyShowIn") {
        if !only.split(';').any(|d| d.eq_ignore_ascii_case("glove")) {
            return None;
        }
    }
    let name = localized("Name")?.to_string();
    let exec = clean_exec(val("Exec")?);
    if exec.is_empty() {
        return None;
    }
    let keywords = normalize(&format!("{} {}", localized("GenericName").unwrap_or(""), localized("Keywords").unwrap_or("").replace(';', " ")));
    Some(App { name, keywords, exec, icon: Icon::Window, glove_id: None })
}

/// Pastas de apps do XDG, na ordem de prioridade.
fn app_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(d) = data_home {
        v.push(d.join("applications"));
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        v.push(PathBuf::from(d).join("applications"));
    }
    v
}

fn installed_apps(glove_apps: &[glove_config::DockApp]) -> Vec<App> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<App> = glove_apps
        .iter()
        .map(|a| App { name: a.name.clone(), keywords: normalize(&a.app_id), exec: a.command.clone(), icon: Icon::from_name(&a.icon), glove_id: Some(a.app_id.clone()) })
        .collect();
    for dir in app_dirs() {
        let mut files = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if let Ok(sub) = std::fs::read_dir(&p) {
                        files.extend(sub.flatten().map(|e| e.path()));
                    }
                } else {
                    files.push(p);
                }
            }
        }
        for p in files {
            if p.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            // Mesmo id em pastas diferentes: vale o primeiro (a do usuário).
            let id = p.strip_prefix(&dir).map(|r| r.to_string_lossy().replace('/', "-")).unwrap_or_default();
            if !seen.insert(id) {
                continue;
            }
            if let Some(app) = std::fs::read_to_string(&p).ok().and_then(|t| parse_desktop(&t)) {
                // Os apps do Glove já estão na lista com o nome em português.
                if !out.iter().any(|a| a.name.eq_ignore_ascii_case(&app.name)) {
                    out.push(app);
                }
            }
        }
    }
    out
}

/// Painéis das Preferências: (nome, argumento do glove-prefs).
const PREFS: [(&str, &str); 13] = [
    ("Data e Hora (relógio, fuso)", "datahora"),
    ("Rede (Wi-Fi)", "rede"),
    ("Economizador de Energia", "energia"),
    ("Som (volume)", "som"),
    ("Aparência", "aparencia"),
    ("Mesa (papel de parede)", "mesa"),
    ("Monitores", "monitor"),
    ("Dock", "dock"),
    ("Teclado", "teclado"),
    ("Mouse", "mouse"),
    ("Trackpad", "trackpad"),
    ("Atalhos de teclado", "atalhos"),
    ("Terminal", "terminal"),
];

/// Pasta pessoal sem itens ocultos nem pastas pesadas de código, até MAX_FILES.
fn index_home() -> Vec<FileEntry> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![(home, 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "target" | "__pycache__" | "snap") {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let dir_entry = ft.is_dir();
            if dir_entry && depth + 1 < MAX_DEPTH {
                stack.push((e.path(), depth + 1));
            }
            out.push(FileEntry { norm: normalize(&name), path: e.path(), dir: dir_entry });
            if out.len() >= MAX_FILES {
                return out;
            }
        }
    }
    out
}

fn file_kind(p: &Path, dir: bool) -> (&'static str, Icon) {
    if dir {
        return ("Pastas", Icon::Folder);
    }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => ("Imagens", Icon::Picture),
        "txt" | "md" | "pdf" | "odt" | "doc" | "docx" | "rtf" | "csv" | "ods" | "xls" | "xlsx" => ("Documentos", Icon::Document),
        _ => ("Outros", Icon::File),
    }
}

impl Glove {
    pub fn toggle_search(&mut self) {
        if self.search.open { self.close_search() } else { self.open_search() }
    }

    pub fn open_search(&mut self) {
        self.close_menu();
        self.close_volume_slider();
        self.search.open = true;
        self.search.query.clear();
        self.search.results.clear();
        self.search.selected = None;
        self.search.opened = Some(Instant::now());
        self.search.apps = installed_apps(&self.config.dock_apps);
        if !self.search.indexing && self.search.indexed.is_none_or(|t| t.elapsed() > REINDEX) {
            self.search.indexing = true;
            let (tx, rx) = channel::<Vec<FileEntry>>();
            std::thread::spawn(move || {
                let _ = tx.send(index_home());
            });
            self.loop_handle
                .insert_source(rx, |ev, _, data| {
                    if let ChannelEvent::Msg(files) = ev {
                        let s = &mut data.state.search;
                        tracing::info!("busca: {} itens na pasta pessoal", files.len());
                        s.files = files;
                        s.indexed = Some(Instant::now());
                        s.indexing = false;
                        data.state.run_search();
                    }
                })
                .ok();
        }
        // Pisca o cursor do campo enquanto aberta.
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(530)), |_, _, data| {
                if !data.state.search.open {
                    return TimeoutAction::Drop;
                }
                data.state.dirty = true;
                TimeoutAction::ToDuration(Duration::from_millis(530))
            })
            .ok();
        self.dirty = true;
    }

    pub fn close_search(&mut self) {
        if self.search.open {
            self.search.open = false;
            self.search.tex = None;
            self.search.key = None;
            self.dirty = true;
        }
    }

    /// Refaz a lista de resultados para o texto atual.
    pub fn run_search(&mut self) {
        let q = normalize(self.search.query.trim());
        let mut rows: Vec<(QsRow, Target)> = Vec::new();
        if !q.is_empty() {
            let row = |section: &str, icon, name: &str| QsRow { section: Some(section.to_string()), icon, name: name.to_string() };
            // Apps: nome primeiro, palavras-chave valem menos.
            let mut apps: Vec<(u32, &App)> = self
                .search
                .apps
                .iter()
                .map(|a| (score(&normalize(&a.name), &q).max(score(&a.keywords, &q) / 2), a))
                .filter(|(s, _)| *s > 0)
                .collect();
            // Empate: os apps do Glove primeiro, depois o nome mais curto.
            apps.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.glove_id.is_some().cmp(&a.1.glove_id.is_some())).then(a.1.name.len().cmp(&b.1.name.len())));
            let mut prefs: Vec<(u32, &(&str, &'static str))> = PREFS.iter().map(|p| (score(&normalize(p.0), &q), p)).filter(|(s, _)| *s > 0).collect();
            prefs.sort_by(|a, b| b.0.cmp(&a.0));
            // Arquivos: só quando o texto começa uma palavra do nome (menos ruído).
            let mut files: Vec<(u32, &FileEntry)> = self.search.files.iter().map(|f| (score(&f.norm, &q), f)).filter(|(s, _)| *s >= 60).collect();
            files.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.path.as_os_str().len().cmp(&b.1.path.as_os_str().len())));

            // Melhor resultado: o app de nota mais alta (ou o melhor arquivo).
            for (_, a) in apps.iter().take(PER_SECTION) {
                rows.push((row("Aplicativos", a.icon, &a.name), Target::App((*a).clone())));
            }
            for (_, (name, pane)) in prefs.iter().take(3) {
                rows.push((row("Preferências", Icon::Settings, name), Target::Prefs(pane)));
            }
            for sec in ["Pastas", "Documentos", "Imagens", "Outros"] {
                let mut n = 0;
                for (_, f) in &files {
                    let (s, icon) = file_kind(&f.path, f.dir);
                    if s != sec {
                        continue;
                    }
                    let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    rows.push((row(sec, icon, &name), Target::Path(f.path.clone())));
                    n += 1;
                    if n == PER_SECTION {
                        break;
                    }
                }
            }
            // Cabe em telas pequenas (os arquivos, no fim, são os cortados).
            rows.truncate(MAX_ROWS);
            // Nome da seção só na primeira linha de cada uma.
            let mut last = String::new();
            for (r, _) in &mut rows {
                let s = r.section.clone().unwrap_or_default();
                if s == last {
                    r.section = None;
                } else {
                    last = s;
                }
            }
        }
        self.search.selected = (!rows.is_empty()).then_some(0);
        self.search.results = rows;
        self.dirty = true;
    }

    pub fn search_key(&mut self, k: crate::lock::LockKey) {
        use crate::lock::LockKey;
        match k {
            LockKey::Char(c) if !c.is_control() && self.search.query.len() < 200 => {
                self.search.query.push(c);
                self.run_search();
            }
            LockKey::Backspace => {
                self.search.query.pop();
                self.run_search();
            }
            LockKey::Escape => {
                // Esc apaga o texto; com o campo vazio, fecha.
                if self.search.query.is_empty() {
                    self.close_search();
                } else {
                    self.search.query.clear();
                    self.run_search();
                }
            }
            LockKey::Enter => {
                if let Some(i) = self.search.selected {
                    self.search_open_result(i);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    pub fn search_move(&mut self, dir: i32) {
        let n = self.search.results.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.search.selected.map(|s| s as i32).unwrap_or(-1);
        self.search.selected = Some((cur + dir).clamp(0, n - 1) as usize);
        self.dirty = true;
    }

    pub fn search_open_result(&mut self, i: usize) {
        let Some((_, target)) = self.search.results.get(i).cloned() else { return };
        self.close_search();
        match target {
            Target::App(a) => match a.glove_id.as_deref().and_then(|id| self.config.dock_apps.iter().position(|d| d.app_id == id)) {
                Some(idx) => self.run_dock_action(crate::shell::DockAction::App(idx)),
                None => self.spawn(&a.exec),
            },
            Target::Prefs(pane) => self.spawn(&format!("glove-prefs {pane}")),
            Target::Path(p) if p.is_dir() => {
                let _ = std::process::Command::new(crate::shell::sibling_binary("glove-files")).arg(&p).env("WAYLAND_DISPLAY", &self.socket_name).spawn();
            }
            Target::Path(p) => {
                let _ = std::process::Command::new("xdg-open").arg(&p).env("WAYLAND_DISPLAY", &self.socket_name).spawn();
            }
        }
    }

    /// Canto do conteúdo do painel (encostado à direita, embaixo da lupa).
    pub fn search_origin(&self) -> Point<i32, Logical> {
        let (w, _) = self.output_size();
        ((w as f32 - chrome::QS_W - 2.0) as i32, glove_paint::theme::MENUBAR_H).into()
    }

    /// Linha do painel sob o ponto (Some(None) = dentro do painel, fora das linhas).
    pub fn search_hit(&self, pos: Point<f64, Logical>) -> Option<Option<usize>> {
        if !self.search.open {
            return None;
        }
        let o = self.search_origin();
        let (x, y) = ((pos.x - o.x as f64) as f32, (pos.y - o.y as f64) as f32);
        if x < 0.0 || y < 0.0 || x >= self.search.size.0 as f32 || y >= self.search.size.1 as f32 {
            return None;
        }
        Some(chrome::quick_search_row_at(self.search.results.len(), y))
    }

    pub fn prepare_search(&mut self) {
        if !self.search.open {
            return;
        }
        let sc = self.scale() as f32;
        let s = &mut self.search;
        let caret = s.opened.map(|t| (t.elapsed().as_millis() / 530) % 2 == 0).unwrap_or(true);
        let key = (s.query.clone(), s.selected, s.results.len(), caret, s.indexing, self.theme_gen);
        if s.key.as_ref() != Some(&key) {
            let rows: Vec<QsRow> = s.results.iter().map(|(r, _)| r.clone()).collect();
            let h = chrome::quick_search_height(rows.len(), s.query.is_empty(), rows.is_empty());
            s.size = (chrome::QS_W as i32, h.ceil() as i32);
            s.tex = chrome::quick_search(&self.fonts, &self.theme, &s.query, caret, &rows, s.selected, s.indexing && !s.query.is_empty(), sc).map(to_tex);
            s.key = Some(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notas() {
        assert_eq!(normalize("Preferências Ç"), "preferencias c");
        assert!(score("calculadora", "calc") > score("minha calculadora", "calc"));
        assert!(score("minha calculadora", "calc") > score("xcalcx", "calc"));
        assert_eq!(score("firefox", "zz"), 0);
    }

    #[test]
    fn desktop() {
        let t = "[Desktop Entry]\nType=Application\nName=Text Editor\nName[pt_BR]=Editor de texto\nExec=gnome-text-editor %U\nKeywords=text;\n[Desktop Action new]\nName=Nova\n";
        let a = parse_desktop(t).unwrap();
        assert_eq!((a.name.as_str(), a.exec.as_str()), ("Editor de texto", "gnome-text-editor"));
        assert!(parse_desktop("[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n").is_none());
        assert!(parse_desktop("[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;\n").is_none());
    }
}
