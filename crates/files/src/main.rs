//! Arquivos: navegador de pastas do Glove (colunas, lista e ícones).

mod apps;
mod info;
mod ops;
mod smart;

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}, Color, Theme, Variant},
    run,
    ui::Key,
    App, Column, Glyph, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions, SCROLLBAR_W, SEARCH_H, SEGMENT_H, SEGMENT_W,
};

/// Barra de ferramentas em metal, emendada no título (como no Tiger).
const TOOLBAR_H: f32 = 40.0;
/// Faixa de baixo em metal, com o status.
const BOTTOM_H: f32 = 22.0;
/// Barra lateral com discos e lugares.
const SIDEBAR_W: f32 = 158.0;
const SIDE_ROW: f32 = 22.0;
const ROW_H: f32 = 18.0;
const COL_W: f32 = 200.0;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Icons,
    List,
    Columns,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Folder,
    Text,
    Code,
    Picture,
    Audio,
    Video,
    Archive,
    Pdf,
    /// App da pasta Aplicativos.
    App,
    Other,
}

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    kind: Kind,
    size: u64,
    modified: Option<SystemTime>,
    /// Atalho (link simbólico): ganha a setinha no ícone.
    link: bool,
    /// Ícone próprio (apps da pasta Aplicativos).
    app_icon: Option<Icon>,
}

impl Entry {
    fn icon(&self) -> Icon {
        match self.kind {
            Kind::Folder => special_folder(&self.path).map(Icon::SpecialFolder).unwrap_or(Icon::Folder),
            Kind::Text => Icon::Document,
            Kind::Code => Icon::CodeFile,
            Kind::Picture => Icon::Picture,
            Kind::Audio => Icon::AudioFile,
            Kind::Video => Icon::VideoFile,
            Kind::Archive => Icon::ArchiveFile,
            Kind::Pdf => Icon::PdfFile,
            Kind::App => self.app_icon.unwrap_or(Icon::Window),
            Kind::Other => Icon::File,
        }
    }
    fn type_name(&self) -> &'static str {
        match self.kind {
            Kind::Folder => "Pasta",
            Kind::Text => "Documento de texto",
            Kind::Code => "Código-fonte",
            Kind::Picture => "Imagem",
            Kind::Audio => "Áudio",
            Kind::Video => "Vídeo",
            Kind::Archive => "Arquivo compactado",
            Kind::Pdf => "Documento PDF",
            Kind::App => "Aplicativo",
            Kind::Other => "Documento",
        }
    }
    /// Desenha o ícone (com a marca de atalho, se for um link).
    fn draw_icon(&self, c: &mut glove_kit::paint::draw::Canvas, fonts: &glove_kit::paint::Fonts, x: f32, y: f32, s: f32) {
        icons::draw(c, fonts, self.icon(), x, y, s, None);
        if self.link {
            icons::alias_badge(c, x, y, s);
        }
    }
}

fn kind_of(path: &Path, is_dir: bool) -> Kind {
    if is_dir {
        return Kind::Folder;
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" | "log" | "conf" | "ini" | "csv" | "desktop" | "rtf" => Kind::Text,
        "rs" | "toml" | "json" | "c" | "h" | "cpp" | "hpp" | "py" | "sh" | "xml" | "html" | "css" | "js" | "ts" | "yaml" | "yml" | "go" | "java" | "rb" | "lua" => Kind::Code,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "tif" | "tiff" => Kind::Picture,
        "mp3" | "ogg" | "oga" | "flac" | "wav" | "m4a" | "opus" | "aac" => Kind::Audio,
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "ogv" => Kind::Video,
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" | "zst" | "deb" | "rpm" => Kind::Archive,
        "pdf" => Kind::Pdf,
        _ => Kind::Other,
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}


/// Pasta especial (Mesa, Documentos…) desta pasta, se for uma delas.
/// As pastas do usuário são lidas uma vez só.
fn special_folder(p: &Path) -> Option<icons::FolderKind> {
    use icons::FolderKind as F;
    static MAP: std::sync::OnceLock<Vec<(PathBuf, F)>> = std::sync::OnceLock::new();
    let map = MAP.get_or_init(|| {
        let mut v = vec![(home(), F::Home), (apps::apps_dir(), F::Applications), (apps::utils_dir(), F::Applications)];
        for (key, k) in [("DESKTOP", F::Desktop), ("DOCUMENTS", F::Documents), ("DOWNLOAD", F::Downloads), ("PICTURES", F::Pictures), ("MUSIC", F::Music), ("VIDEOS", F::Videos), ("PUBLICSHARE", F::Public), ("TEMPLATES", F::Templates)] {
            if let Some(d) = user_dir(key) {
                v.push((d, k));
            }
        }
        v
    });
    map.iter().find(|(d, _)| d == p).map(|(_, k)| *k)
}

/// Pasta do usuário pelo ~/.config/user-dirs.dirs (ex.: "MUSIC" → ~/Músicas).
/// None se não estiver definida ou for a própria pasta pessoal.
fn user_dir(key: &str) -> Option<PathBuf> {
    let h = home();
    let text = std::fs::read_to_string(h.join(".config/user-dirs.dirs")).ok()?;
    let line = text.lines().find(|l| l.starts_with(&format!("XDG_{key}_DIR=")))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => h.join(rest.trim_start_matches('/')),
        None => PathBuf::from(v),
    };
    (p != h && p.is_dir()).then_some(p)
}

fn trash_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
    base.join("Trash")
}

fn display_name(p: &Path) -> String {
    if let Some(n) = apps::display_name(p) {
        return n.into();
    }
    if smart::is_smart(p) {
        let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return if p.starts_with("/:Busca/") { format!("Busca: {n}") } else { n };
    }
    if p == Path::new("/") {
        return "Computador".into();
    }
    if p == home() {
        return "Início".into();
    }
    if p == trash_dir().join("files") {
        return "Lixeira".into();
    }
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn human_size(n: u64) -> String {
    let f = n as f64;
    let s = if n < 1024 {
        format!("{n} bytes")
    } else if f < 1024.0 * 1024.0 {
        format!("{:.0} KB", f / 1024.0)
    } else if f < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB", f / 1024.0 / 1024.0)
    } else {
        format!("{:.1} GB", f / 1024.0 / 1024.0 / 1024.0)
    };
    s.replace('.', ",")
}

fn human_date(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "—".into() };
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        let mut today: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut today);
        let mut yest: libc::tm = std::mem::zeroed();
        libc::localtime_r(&(now - 86_400), &mut yest);
        let same = |a: &libc::tm| a.tm_year == tm.tm_year && a.tm_yday == tm.tm_yday;
        let hm = format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
        if same(&today) {
            format!("Hoje, {hm}")
        } else if same(&yest) {
            format!("Ontem, {hm}")
        } else {
            format!("{:02}/{:02}/{}, {hm}", tm.tm_mday, tm.tm_mon + 1, tm.tm_year + 1900)
        }
    }
}

/// Dá para criar e apagar itens nesta pasta?
fn writable(p: &Path) -> bool {
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

fn free_space(p: &Path) -> Option<u64> {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).ok()?;
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(c.as_ptr(), &mut st) == 0).then(|| st.f_bavail as u64 * st.f_frsize as u64)
    }
}

fn read_dir(dir: &Path, hidden: bool) -> Result<Vec<Entry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("Não foi possível abrir: {e}"))?;
    let mut v: Vec<Entry> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                return None;
            }
            let path = e.path();
            let link = e.file_type().is_ok_and(|t| t.is_symlink());
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            Some(Entry {
                kind: kind_of(&path, is_dir),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                name,
                path,
                link,
                app_icon: None,
            })
        })
        .collect();
    v.sort_by(|a, b| (b.kind == Kind::Folder).cmp(&(a.kind == Kind::Folder)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(v)
}

/// Move para a Lixeira no padrão freedesktop (outros apps Linux enxergam).
fn move_to_trash(p: &Path) -> Result<(), String> {
    let t = trash_dir();
    let (files, info) = (t.join("files"), t.join("info"));
    std::fs::create_dir_all(&files).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&info).map_err(|e| e.to_string())?;
    let base = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "item".into());
    let mut name = base.clone();
    let mut n = 2;
    while files.join(&name).exists() || info.join(format!("{name}.trashinfo")).exists() {
        name = format!("{base} {n}");
        n += 1;
    }
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let date = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
    };
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    std::fs::write(info.join(format!("{name}.trashinfo")), format!("[Trash Info]\nPath={}\nDeletionDate={date}\n", abs.display())).map_err(|e| e.to_string())?;
    let dest = files.join(&name);
    if std::fs::rename(p, &dest).is_err() {
        // Outro disco: arquivos comuns são copiados e apagados.
        if p.is_file() {
            std::fs::copy(p, &dest).map_err(|e| e.to_string())?;
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
        } else {
            let _ = std::fs::remove_file(info.join(format!("{name}.trashinfo")));
            return Err("Esta pasta está em outro disco e não pode ir para a Lixeira.".into());
        }
    }
    Ok(())
}

/// "%C3%A9" → "é" (o Path do .trashinfo pode vir codificado como URL).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// "~/Doc" → /home/x/Doc.
fn expand_tilde(s: &str) -> PathBuf {
    match s.strip_prefix('~') {
        Some(rest) => home().join(rest.trim_start_matches('/')),
        None => PathBuf::from(s),
    }
}

/// Completa o caminho com Tab: o nome único que começa com o fim digitado, ou o
/// pedaço em comum entre os candidatos (só pastas).
fn complete_path(text: &str) -> String {
    let full = expand_tilde(text);
    let (dir, prefix) = if text.ends_with('/') {
        (full.clone(), String::new())
    } else {
        (full.parent().map(Path::to_path_buf).unwrap_or_else(|| "/".into()), full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
    };
    let Ok(rd) = std::fs::read_dir(&dir) else { return text.to_string() };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix) && (prefix.starts_with('.') || !n.starts_with('.')))
        .collect();
    names.sort();
    let Some(first) = names.first().cloned() else { return text.to_string() };
    let common: String = if names.len() == 1 {
        format!("{first}/")
    } else {
        let mut c = first.clone();
        for n in &names[1..] {
            while !n.starts_with(&c) {
                c.pop();
            }
        }
        c
    };
    // Mantém o jeito que foi escrito (com ~ ou não).
    let base = &text[..text.len() - prefix.len()];
    format!("{base}{common}")
}

/// Caminho de um file:// (com %XX).
fn ops_decode(s: &str) -> String {
    percent_decode(s)
}

/// Devolver: põe um item da Lixeira de volta onde estava. Devolve o destino.
fn put_back(item: &Path) -> Result<PathBuf, String> {
    put_back_in(&trash_dir(), item)
}

fn put_back_in(t: &Path, item: &Path) -> Result<PathBuf, String> {
    let name = item.file_name().ok_or("item inválido")?.to_string_lossy().into_owned();
    let info = t.join("info").join(format!("{name}.trashinfo"));
    let text = std::fs::read_to_string(&info).map_err(|_| "Não se sabe de onde este item veio.".to_string())?;
    let orig = text.lines().find_map(|l| l.strip_prefix("Path=")).ok_or("Não se sabe de onde este item veio.")?;
    let dest = PathBuf::from(percent_decode(orig));
    if dest.exists() {
        return Err(format!("Já existe um item chamado “{}” no lugar de origem.", dest.file_name().unwrap_or_default().to_string_lossy()));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(item, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(info);
    Ok(dest)
}

/// Esvazia a Lixeira: apaga os itens e os registros de origem.
fn empty_trash() -> Result<(), String> {
    empty_trash_in(&trash_dir())
}

fn empty_trash_in(t: &Path) -> Result<(), String> {
    for sub in ["files", "info"] {
        let Ok(rd) = std::fs::read_dir(t.join(sub)) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let r = if e.file_type().is_ok_and(|ft| ft.is_dir()) { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            r.map_err(|err| format!("{}: {err}", p.display()))?;
        }
    }
    Ok(())
}

/// Janela de confirmação de "Esvaziar Lixeira…" (glove-files --esvaziar-lixeira).
struct EmptyTrash {
    count: usize,
    error: Option<String>,
}

impl App for EmptyTrash {
    fn ui(&mut self, ui: &mut Ui) {
        use glove_kit::AlertKind;
        if let Some(e) = self.error.clone() {
            if ui.alert_window(AlertKind::Stop, "Não foi possível esvaziar a Lixeira.", &e, &["OK"]).is_some() {
                ui.close = true;
            }
            return;
        }
        let text = format!("{} {} na Lixeira. Esta ação não pode ser desfeita.", self.count, if self.count == 1 { "item" } else { "itens" });
        match ui.alert_window(AlertKind::Caution, "Tem certeza de que deseja apagar para sempre os itens da Lixeira?", &text, &["Cancelar", "Esvaziar Lixeira"]) {
            Some(1) => match empty_trash() {
                Ok(()) => ui.close = true,
                Err(e) => self.error = Some(e),
            },
            Some(_) => ui.close = true,
            None => {}
        }
    }
}

/// Abre um programa sem ficar esperando por ele; uma thread recolhe o
/// processo quando ele termina (sem zumbis).
fn launch(cmd: &mut std::process::Command) -> std::io::Result<()> {
    let mut child = cmd.spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

fn open_file(e: &Entry) {
    use std::process::Command;
    let mut cmd = match e.kind {
        Kind::Text => Command::new(sibling("glove-edit")),
        Kind::Picture if e.path.extension().and_then(|x| x.to_str()).is_some_and(|x| ["png", "jpg", "jpeg"].contains(&x.to_ascii_lowercase().as_str())) => Command::new(sibling("glove-view")),
        Kind::Archive if expandable(&e.path) => {
            let mut c = Command::new(sibling("glove-arquivo"));
            c.arg("expandir");
            c
        }
        _ => Command::new("xdg-open"),
    };
    let _ = launch(cmd.arg(&e.path));
}

/// Arquivos que o Utilitário de Arquivo sabe expandir.
fn expandable(p: &Path) -> bool {
    let low = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    [".zip", ".tar", ".tgz", ".txz", ".tbz2", ".tzst", ".gz", ".xz", ".bz2", ".zst"].iter().any(|e| low.ends_with(e))
}

struct Files {
    view: View,
    /// Pasta da janela (raiz das colunas).
    base: PathBuf,
    /// Colunas abertas (a primeira é `base`).
    columns: Vec<PathBuf>,
    /// Item selecionado (na última coluna, ou na pasta atual).
    selected: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    /// Texto do campo de busca da barra.
    search: String,
    hidden: bool,
    cache: HashMap<PathBuf, (Option<SystemTime>, bool, Result<Vec<Entry>, String>)>,
    scroll: HashMap<PathBuf, f32>,
    message: Option<String>,
    last_title: String,
    /// A seleção mudou pelo teclado: rolar a lista até ela.
    reveal: bool,
    /// Menu da engrenagem (ações) aberto.
    gear_open: bool,
    /// Copiar/Recortar: os itens e se é para mover ao colar.
    clip: Option<(Vec<PathBuf>, bool)>,
    /// Esperando o texto da área de transferência (colar vindo de outro app).
    paste_wait: bool,
    /// Nomes repetidos: a pergunta antes de copiar (itens, nomes, mover).
    ask: Option<(Vec<ops::Item>, Vec<String>, bool)>,
    /// Cópia em andamento: progresso, verbo ("Copiando"/"Movendo") e quantos itens.
    job: Option<(Arc<ops::Progress>, &'static str, usize)>,
    /// Arrastar: item, onde começou e se já virou arraste.
    drag: Option<(PathBuf, (f32, f32), bool)>,
    /// Pasta sob o mouse durante o arraste (neste quadro).
    drop_target: Option<PathBuf>,
    /// Menu de contexto: posição e o item (None = clique no vazio).
    ctx: Option<((f32, f32), Option<PathBuf>)>,
    /// Renomeando no lugar: item, texto e se o campo acabou de abrir.
    rename: Option<(PathBuf, String, bool)>,
    /// O campo de renomear apareceu neste quadro.
    rename_shown: bool,
    /// O nome está todo selecionado: a primeira tecla substitui (como no Mac).
    rename_pristine: bool,
    /// Pastas visitadas por último.
    recent: Vec<PathBuf>,
    /// "Ir para a pasta…": o texto e se a folha acabou de abrir.
    goto: Option<(String, bool)>,
    /// Lista: coluna e direção da ordenação (Nome crescente no começo).
    list_sort: (usize, bool),
    /// Lista: pastas abertas no lugar (triângulo).
    expanded: std::collections::HashSet<PathBuf>,
    /// Outros itens escolhidos (Ctrl/Shift e retângulo), além de `selected`.
    extra: Vec<PathBuf>,
    /// Retângulo elástico (ícones): onde começou.
    band: Option<(f32, f32)>,
    /// inotify: atualiza sozinho quando as pastas à vista mudam.
    notify_fd: i32,
    watched: HashMap<i32, PathBuf>,
    /// Onde a busca da barra procura (a pasta em que se estava).
    search_scope: PathBuf,
}

impl Files {
    fn new(start: PathBuf) -> Self {
        Self {
            view: View::Columns,
            columns: vec![start.clone()],
            base: start,
            selected: None,
            back: Vec::new(),
            forward: Vec::new(),
            search: String::new(),
            hidden: false,
            cache: HashMap::new(),
            scroll: HashMap::new(),
            message: None,
            last_title: String::new(),
            reveal: false,
            gear_open: false,
            clip: None,
            paste_wait: false,
            ask: None,
            job: None,
            drag: None,
            drop_target: None,
            ctx: None,
            rename: None,
            rename_shown: false,
            rename_pristine: false,
            recent: Vec::new(),
            goto: None,
            list_sort: (0, true),
            expanded: std::collections::HashSet::new(),
            extra: Vec::new(),
            band: None,
            notify_fd: unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) },
            watched: HashMap::new(),
            search_scope: home(),
        }
    }

    /// Colar/soltar `srcs` em `dir`: pergunta antes se algum nome já existe lá.
    fn transfer(&mut self, srcs: Vec<PathBuf>, dir: PathBuf, mv: bool) {
        if srcs.is_empty() || self.job.is_some() {
            return;
        }
        // Soltar na mesma pasta não faz nada.
        if mv && srcs.iter().all(|s| s.parent() == Some(dir.as_path())) {
            return;
        }
        let (items, clashes) = ops::plan(&srcs, &dir);
        if clashes.is_empty() {
            self.start_job(items, mv);
        } else {
            self.ask = Some((items, clashes, mv));
        }
    }

    fn start_job(&mut self, items: Vec<ops::Item>, mv: bool) {
        let n = items.len();
        self.job = Some((ops::start(items, mv), if mv { "Movendo" } else { "Copiando" }, n));
        if mv {
            self.clip = None;
        }
    }

    /// Alerta de nome repetido (folha da janela).
    fn ask_sheet(&mut self, ui: &mut Ui) {
        let Some((_, names, _)) = &self.ask else { return };
        let title = if names.len() == 1 {
            format!("Já existe um item chamado “{}” nesta pasta.", names[0])
        } else {
            format!("Já existem {} itens com esses nomes nesta pasta.", names.len())
        };
        let text = "Deseja substituir pelo que está sendo copiado? Substituir apaga o que está lá agora.";
        match ui.alert(glove_kit::AlertKind::Caution, &title, text, &["Parar", "Manter os dois", "Substituir"]) {
            Some(0) => self.ask = None,
            Some(c) => {
                let (mut items, _, mv) = self.ask.take().unwrap();
                ops::resolve(&mut items, if c == 1 { ops::Conflict::KeepBoth } else { ops::Conflict::Replace });
                self.start_job(items, mv);
            }
            None => {}
        }
    }

    /// Faixa das pastas de busca. Devolve a área que sobra para os itens.
    fn search_bar(&mut self, ui: &mut Ui, area: Rect) -> Rect {
        let bh = 30.0;
        ui.c.fill_rect(area.x, area.y, area.w, bh, &vgrad(area.y, area.y + bh, &[(0.0, rgba(236, 238, 242, 255)), (1.0, rgba(222, 225, 230, 255))]));
        ui.c.hline(area.x, area.y + bh - 1.0, area.w, rgba(170, 172, 178, 255));
        let base = self.base.clone();
        let Some((q, scope)) = smart::query_of(&base, &self.search_scope) else { return area };
        let temp = base.starts_with("/:Busca/");
        let text = format!("Buscando “{q}” em {}", display_name(&scope));
        let t = ui.fonts.ellipsize(&text, 12.0, false, area.w - 150.0);
        ui.fonts.draw(ui.c, &t, area.x + 12.0, area.y + 19.5, 12.0, false, rgba(40, 40, 40, 255));
        let mut clicked = false;
        let label = if temp { "Salvar" } else { "Apagar Busca" };
        ui.area(Rect::new(area.x + area.w - 126.0, area.y + 4.0, 116.0, 22.0), |ui| clicked = ui.small_button(label));
        if clicked {
            if temp {
                match smart::save(&q, &q, &scope) {
                    Ok(p) => {
                        self.set_base(p);
                        self.message = Some("Busca salva: ela aparece na barra lateral.".into());
                    }
                    Err(e) => self.message = Some(format!("Não foi possível salvar: {e}")),
                }
            } else {
                smart::delete(&base);
                self.go(home());
            }
        }
        Rect::new(area.x, area.y + bh, area.w, area.h - bh)
    }

    /// Folha "Ir para a pasta": campo com o caminho (Tab completa), Ir e Cancelar.
    fn goto_sheet(&mut self, ui: &mut Ui) {
        let Some((mut text, first)) = self.goto.clone() else { return };
        // Tab: completa com o nome que começa com o que foi digitado (ou o pedaço comum).
        if ui.keys().iter().any(|k| k.key == Key::Tab) {
            text = complete_path(&text);
        }
        let w = ui.c.w;
        let (pw, ph) = (420.0, 112.0);
        let x = (w - pw) / 2.0;
        let y = 0.0;
        ui.c.fill(rrect(x + 3.0, y + 4.0, pw, ph, 6.0), &solid(rgba(0, 0, 0, 60)));
        ui.c.fill(rrect4(x, y, pw, ph, [0.0, 0.0, 6.0, 6.0]), &solid(ui.theme.body_b));
        ui.c.pinstripes(x, y, pw, ph - 6.0, ui.theme.body_a, ui.theme.body_b);
        ui.c.stroke(rrect4(x + 0.5, y, pw - 1.0, ph - 0.5, [0.0, 0.0, 6.0, 6.0]), &solid(rgba(0, 0, 0, 90)), 1.0);
        ui.fonts.draw(ui.c, "Ir para a pasta:", x + 18.0, y + 24.0, 13.0, true, rgba(20, 20, 20, 255));
        let (_, enter) = ui.text_field_in(Rect::new(x + 18.0, y + 34.0, pw - 36.0, 22.0), &mut text, first || ui.keys().iter().any(|k| k.key == Key::Tab));
        let mut go = enter;
        let mut cancel = ui.keys().iter().any(|k| k.key == Key::Escape);
        ui.area(Rect::new(x + pw - 210.0, y + 72.0, 90.0, 24.0), |ui| cancel |= ui.button("Cancelar"));
        ui.area(Rect::new(x + pw - 110.0, y + 72.0, 90.0, 24.0), |ui| go |= ui.default_button("Ir"));
        if cancel {
            self.goto = None;
            return;
        }
        if go {
            let p = expand_tilde(text.trim());
            if p.is_dir() {
                self.goto = None;
                self.go(p);
                return;
            }
            self.message = Some(format!("A pasta “{}” não existe.", text.trim()));
        }
        self.goto = Some((text, false));
    }

    /// Painel de progresso da cópia, embaixo da janela, com Cancelar.
    fn job_panel(&mut self, ui: &mut Ui) {
        let Some((p, verb, n)) = &self.job else { return };
        if p.finished.load(Ordering::Relaxed) {
            if let Some(e) = p.error.lock().unwrap().clone() {
                self.message = Some(e);
            }
            self.job = None;
            self.cache.clear();
            ui.redraw = true;
            return;
        }
        ui.wake_after(std::time::Duration::from_millis(100));
        let (w, h) = (ui.c.w, ui.c.h);
        let (pw, ph) = (380.0, 92.0);
        let (x, y) = ((w - pw) / 2.0, h - BOTTOM_H - ph - 12.0);
        ui.c.fill(rrect(x + 2.0, y + 4.0, pw, ph, 8.0), &solid(rgba(0, 0, 0, 60)));
        ui.c.fill(rrect(x, y, pw, ph, 8.0), &solid(ui.theme.body_b));
        ui.c.pinstripes(x, y + 8.0, pw, ph - 16.0, ui.theme.body_a, ui.theme.body_b);
        ui.c.stroke(rrect(x + 0.5, y + 0.5, pw - 1.0, ph - 1.0, 8.0), &solid(rgba(0, 0, 0, 90)), 1.0);
        let label = format!("{verb} {n} {}…", if *n == 1 { "item" } else { "itens" });
        ui.fonts.draw(ui.c, &label, x + 16.0, y + 22.0, 13.0, true, rgba(20, 20, 20, 255));
        let cur = p.current.lock().unwrap().clone();
        let cur = ui.fonts.ellipsize(&cur, 11.0, false, pw - 32.0);
        ui.fonts.draw(ui.c, &cur, x + 16.0, y + 40.0, 11.0, false, rgba(70, 70, 70, 255));
        let total = p.total.load(Ordering::Relaxed);
        let frac = if total == 0 { None } else { Some((p.done.load(Ordering::Relaxed) as f32 / total as f32).min(1.0)) };
        ui.progress_in(Rect::new(x + 16.0, y + 54.0, pw - 130.0, 12.0), frac);
        let p2 = p.clone();
        ui.area(Rect::new(x + pw - 104.0, y + 50.0, 90.0, 22.0), |ui| {
            if ui.button("Cancelar") {
                p2.cancel.store(true, Ordering::Relaxed);
            }
        });
    }

    /// Arrastar: começa depois de 5 pontos; mostra o item seguindo o mouse; soltar
    /// numa pasta move (com Ctrl, copia).
    fn drag_frame(&mut self, ui: &mut Ui) {
        let Some((path, start, active)) = self.drag.clone() else { return };
        let (px, py) = ui.pointer();
        if ui.mouse_down() && !active && ((px - start.0).powi(2) + (py - start.1).powi(2)) > 25.0 {
            self.drag = Some((path.clone(), start, true));
        }
        if active {
            // O item "fantasma" seguindo o mouse.
            let name = display_name(&path);
            let tw = ui.text_width(&name, 12.0, false);
            ui.c.fill(rrect(px + 10.0, py + 6.0, tw + 34.0, 22.0, 4.0), &solid(rgba(60, 110, 200, 150)));
            let kind = kind_of(&path, path.is_dir());
            let e = Entry { name: name.clone(), path: path.clone(), kind, size: 0, modified: None, link: false, app_icon: None };
            e.draw_icon(ui.c, ui.fonts, px + 13.0, py + 9.0, 16.0);
            ui.fonts.draw(ui.c, &name, px + 34.0, py + 21.0, 12.0, false, Color::WHITE);
            ui.wake_after(std::time::Duration::from_millis(30));
        }
        if ui.mouse_released() {
            self.drag = None;
            if let (true, Some(target)) = (active, self.drop_target.clone()) {
                if target != path {
                    let copy = ui.mods().ctrl;
                    // Arrastar um dos escolhidos leva todos eles.
                    let items = if self.is_chosen(&path) { self.all_selected() } else { vec![path] };
                    self.transfer(items, target, !copy);
                }
            }
        }
    }

    /// "Compactar “Fotos”" ou "Compactar 3 itens".
    fn compress_label(&self) -> String {
        match self.all_selected().as_slice() {
            [] => "Compactar".into(),
            [one] => format!("Compactar “{}”", one.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            v => format!("Compactar {} itens", v.len()),
        }
    }

    /// Todos os itens escolhidos (o principal primeiro).
    fn all_selected(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.selected.iter().cloned().collect();
        for e in &self.extra {
            if !v.contains(e) {
                v.push(e.clone());
            }
        }
        v
    }

    fn is_chosen(&self, p: &Path) -> bool {
        self.selected.as_deref() == Some(p) || self.extra.iter().any(|e| e == p)
    }

    /// Clique com Ctrl (alterna o item) ou Shift (faixa a partir do principal),
    /// entre itens da mesma pasta. Devolve verdadeiro se tratou o clique.
    fn multi_click(&mut self, ui: &Ui, e: &Entry, entries: &[Entry]) -> bool {
        let m = ui.mods();
        let same_dir = self.selected.as_ref().is_some_and(|s| s.parent() == e.path.parent());
        if !(m.ctrl || m.shift) || !same_dir {
            self.extra.clear();
            return false;
        }
        if m.ctrl {
            if self.selected.as_deref() == Some(e.path.as_path()) {
                self.selected = if self.extra.is_empty() { None } else { Some(self.extra.remove(0)) };
            } else if let Some(i) = self.extra.iter().position(|x| *x == e.path) {
                self.extra.remove(i);
            } else {
                self.extra.push(e.path.clone());
            }
        } else if let Some(anchor) = self.selected.clone() {
            let a = entries.iter().position(|x| x.path == anchor);
            let b = entries.iter().position(|x| x.path == e.path);
            if let (Some(a), Some(b)) = (a, b) {
                let (lo, hi) = (a.min(b), a.max(b));
                self.extra = entries[lo..=hi].iter().map(|x| x.path.clone()).filter(|p| *p != anchor).collect();
            }
        }
        true
    }

    /// Mantém o inotify observando as pastas à vista (colunas e as abertas na lista).
    fn sync_watches(&mut self) {
        if self.notify_fd < 0 {
            return;
        }
        let mut want: Vec<PathBuf> = self.columns.clone();
        want.extend(self.expanded.iter().cloned());
        want.retain(|p| !apps::is_virtual(p) && !smart::is_smart(p));
        let stale: Vec<i32> = self.watched.iter().filter(|(_, p)| !want.contains(p)).map(|(wd, _)| *wd).collect();
        for wd in stale {
            unsafe { libc::inotify_rm_watch(self.notify_fd, wd) };
            self.watched.remove(&wd);
        }
        for p in want {
            if self.watched.values().any(|w| *w == p) {
                continue;
            }
            let Ok(c) = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()) else { continue };
            let mask = libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB;
            let wd = unsafe { libc::inotify_add_watch(self.notify_fd, c.as_ptr(), mask) };
            if wd >= 0 {
                self.watched.insert(wd, p);
            }
        }
    }

    /// O item escolhido está dentro da Lixeira?
    fn in_trash(&self) -> bool {
        let files = trash_dir().join("files");
        self.selected.as_ref().is_some_and(|p| p.parent() == Some(files.as_path()))
    }

    /// Pasta "atual": a última coluna aberta.
    fn current(&self) -> PathBuf {
        self.columns.last().cloned().unwrap_or_else(|| self.base.clone())
    }

    /// Conteúdo de uma pasta, relido só se ela mudou (data de modificação).
    fn list(&mut self, dir: &Path) -> Result<Vec<Entry>, String> {
        // Pasta inteligente: a busca roda uma vez e fica guardada até algo mudar.
        if smart::is_smart(dir) {
            if let Some((_, _, v)) = self.cache.get(dir) {
                return v.clone();
            }
            let Some((q, scope)) = smart::query_of(dir, &self.search_scope) else { return Err("Busca não encontrada.".into()) };
            let mut v: Vec<Entry> = smart::search(&q, &scope)
                .into_iter()
                .map(|path| {
                    let meta = std::fs::metadata(&path).ok();
                    let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                    Entry {
                        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        kind: kind_of(&path, is_dir),
                        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                        modified: meta.as_ref().and_then(|m| m.modified().ok()),
                        link: std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()),
                        app_icon: None,
                        path,
                    }
                })
                .collect();
            v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            self.cache.insert(dir.to_path_buf(), (None, self.hidden, Ok(v.clone())));
            return Ok(v);
        }
        // Pasta Aplicativos: montada com os apps instalados (sem cache: é rápida).
        if apps::is_virtual(dir) {
            let utils = *dir == apps::utils_dir();
            let mut v: Vec<Entry> = Vec::new();
            if !utils {
                v.push(Entry { name: "Utilitários".into(), path: apps::utils_dir(), kind: Kind::Folder, size: 0, modified: None, link: false, app_icon: None });
            }
            v.extend(apps::all().into_iter().filter(|a| a.utility == utils).map(|a| Entry { name: a.name, path: a.path, kind: Kind::App, size: 0, modified: None, link: false, app_icon: Some(a.icon) }));
            return Ok(v);
        }
        let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
        if let Some((t, h, v)) = self.cache.get(dir) {
            if *t == mtime && *h == self.hidden {
                return v.clone();
            }
        }
        let v = read_dir(dir, self.hidden);
        self.cache.insert(dir.to_path_buf(), (mtime, self.hidden, v.clone()));
        v
    }

    fn go(&mut self, p: PathBuf) {
        if p == self.base && self.columns.len() == 1 {
            return;
        }
        // Pastas recentes (Ir › Pastas Recentes).
        self.recent.retain(|r| *r != p);
        self.recent.insert(0, p.clone());
        self.recent.truncate(8);
        self.back.push(self.base.clone());
        self.forward.clear();
        self.set_base(p);
    }

    fn set_base(&mut self, p: PathBuf) {
        self.columns = vec![p.clone()];
        self.base = p;
        self.selected = None;
        self.message = None;
    }

    fn go_back(&mut self) {
        if let Some(p) = self.back.pop() {
            self.forward.push(self.base.clone());
            self.set_base(p);
        }
    }

    fn go_forward(&mut self) {
        if let Some(p) = self.forward.pop() {
            self.back.push(self.base.clone());
            self.set_base(p);
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.current().parent().map(Path::to_path_buf) {
            self.go(parent);
        }
    }

    /// Seleciona `e` dentro da coluna `col` (abre a pasta na coluna seguinte).
    fn select_in_column(&mut self, col: usize, e: &Entry) {
        self.columns.truncate(col + 1);
        self.selected = Some(e.path.clone());
        if e.kind == Kind::Folder {
            self.columns.push(e.path.clone());
        }
    }

    fn open(&mut self, e: &Entry) {
        if e.kind == Kind::Folder {
            if self.view != View::Columns {
                self.go(e.path.clone());
            }
        } else if e.kind == Kind::App {
            if let Err(m) = apps::open(&e.path) {
                self.message = Some(m);
            }
        } else {
            open_file(e);
        }
    }

    fn selected_entry(&mut self) -> Option<Entry> {
        let sel = self.selected.clone()?;
        let dir = sel.parent()?.to_path_buf();
        self.list(&dir).ok()?.into_iter().find(|e| e.path == sel)
    }

    fn new_folder(&mut self) {
        let dir = match (self.view, self.selected_entry()) {
            (View::Columns, Some(e)) if e.kind == Kind::Folder => e.path,
            _ => self.current(),
        };
        let mut name = "Pasta sem título".to_string();
        let mut n = 2;
        while dir.join(&name).exists() {
            name = format!("Pasta sem título {n}");
            n += 1;
        }
        match std::fs::create_dir(dir.join(&name)) {
            Ok(()) => self.message = None,
            Err(e) => self.message = Some(format!("Não foi possível criar a pasta: {e}")),
        }
    }

    fn trash_selected(&mut self) {
        for sel in self.all_selected() {
            match move_to_trash(&sel) {
                Ok(()) => {
                    self.columns.retain(|c| !c.starts_with(&sel));
                    if self.columns.is_empty() {
                        self.columns.push(self.base.clone());
                    }
                    self.message = None;
                }
                Err(e) => {
                    self.message = Some(e);
                    break;
                }
            }
        }
        self.selected = None;
        self.extra.clear();
    }

    // -----------------------------------------------------------------------
    // Barra de ferramentas
    // -----------------------------------------------------------------------

    fn toolbar(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        // Continua o degradê do título em metal (o Glove desenha o título deste app em metal).
        let metal = Theme::new(Variant::Metal);
        ui.c.brushed_metal(0.0, 0.0, w, TOOLBAR_H, metal.metal_bottom, darken(metal.metal_bottom, 0.1));
        ui.c.hline(0.0, TOOLBAR_H - 1.0, w, rgba(110, 110, 116, 255));
        let cy = (TOOLBAR_H - SEGMENT_H) / 2.0;
        // Voltar / Avançar: segmentado momentâneo, apagado quando não há para onde ir.
        let nav = [!self.back.is_empty(), !self.forward.is_empty()];
        match ui.segmented_in(Rect::new(12.0, cy, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &nav) {
            Some(0) => self.go_back(),
            Some(_) => self.go_forward(),
            None => {}
        }
        // Modo de visualização
        let views = [View::Icons, View::List, View::Columns];
        let sel = views.iter().position(|v| *v == self.view);
        let vx = 12.0 + 2.0 * SEGMENT_W + 12.0;
        if let Some(i) = ui.segmented_in(Rect::new(vx, cy, 3.0 * SEGMENT_W, SEGMENT_H), &[Glyph::ViewIcons, Glyph::ViewList, Glyph::ViewColumns], sel, &[]) {
            self.view = views[i];
        }
        // Engrenagem: menu de ações.
        let gx = vx + 3.0 * SEGMENT_W + 12.0;
        if ui.segmented_in(Rect::new(gx, cy, SEGMENT_W + 8.0, SEGMENT_H), &[Glyph::Action], None, &[]).is_some() {
            self.gear_open = !self.gear_open;
        }
        // Busca em cápsula, à direita: abre a Busca já procurando nesta pasta.
        let sw = (w - gx - SEGMENT_W - 40.0).min(190.0);
        if sw >= 90.0 && ui.search_field_in(Rect::new(w - 12.0 - sw, (TOOLBAR_H - SEARCH_H) / 2.0, sw, SEARCH_H), &mut self.search, "Buscar") {
            // Busca: os resultados aparecem como uma pasta (que pode ser salva).
            let q = self.search.trim().to_string();
            if !q.is_empty() {
                let cur = self.current();
                if !apps::is_virtual(&cur) && !smart::is_smart(&cur) {
                    self.search_scope = cur;
                }
                self.cache.remove(&smart::temp_path(&q));
                self.go(smart::temp_path(&q));
            }
        }
    }

    /// Menu da engrenagem, por cima de tudo, logo abaixo do botão.
    fn gear_menu(&mut self, ui: &mut Ui) {
        if !self.gear_open {
            return;
        }
        let sel = self.selected.is_some();
        let items: Vec<(&str, String, bool)> = vec![
            ("new-folder", "Nova pasta".into(), true),
            ("open", "Abrir".into(), sel),
            ("rename", "Renomear".into(), sel),
            ("duplicate", "Duplicar".into(), sel && !self.in_trash()),
            ("alias", "Criar Atalho".into(), sel && !self.in_trash()),
            ("compress", self.compress_label(), sel && !self.in_trash()),
            if self.in_trash() { ("put-back", "Devolver".into(), sel) } else { ("trash", "Mover para a Lixeira".into(), sel) },
            ("", String::new(), false),
            ("hidden", if self.hidden { "Esconder arquivos ocultos".into() } else { "Mostrar arquivos ocultos".into() }, true),
        ];
        let x = 12.0 + 5.0 * SEGMENT_W + 24.0;
        if let Some(id) = Self::popup_menu(ui, x, TOOLBAR_H - 4.0, &items) {
            self.gear_open = false;
            self.action(&id, ui);
        } else if ui.keys().iter().any(|k| k.key == Key::Escape) {
            self.gear_open = false;
        }
    }

    /// Menu suspenso desenhado no app (engrenagem e menu de contexto). Itens:
    /// (ação, rótulo, ativo); ação vazia = divisória. Devolve a ação escolhida.
    fn popup_menu(ui: &mut Ui, x: f32, y0: f32, items: &[(&str, String, bool)]) -> Option<String> {
        let (mw, row) = (220.0, 20.0);
        let mh = items.iter().map(|i| if i.0.is_empty() { 9.0 } else { row }).sum::<f32>() + 8.0;
        // Cabe na janela.
        let x = x.min(ui.c.w - mw - 4.0).max(0.0);
        let y0 = if y0 + mh > ui.c.h - 4.0 { (y0 - mh).max(0.0) } else { y0 };
        ui.c.fill_rect(x + 3.0, y0 + 4.0, mw, mh, &solid(rgba(0, 0, 0, 50)));
        ui.c.fill_rect(x, y0, mw, mh, &solid(rgba(246, 247, 249, 252)));
        ui.c.stroke(rrect(x + 0.5, y0 + 0.5, mw - 1.0, mh - 1.0, 0.0), &solid(rgba(0, 0, 0, 80)), 1.0);
        let mut y = y0 + 4.0;
        let mut chosen = None;
        for (id, label, on) in items {
            if id.is_empty() {
                ui.c.hline(x + 1.0, y + 4.0, mw - 2.0, rgba(214, 214, 218, 255));
                y += 9.0;
                continue;
            }
            let r = Rect::new(x, y, mw, row);
            let h = ui.hit(r);
            if h.hover && *on {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(ui.theme.menu_hot));
            }
            let col = if !on { rgba(160, 160, 160, 255) } else if h.hover { Color::WHITE } else { rgba(20, 20, 20, 255) };
            ui.fonts.draw(ui.c, label, x + 16.0, y + 14.5, 13.0, false, col);
            if h.clicked && *on {
                chosen = Some(id.to_string());
            }
            y += row;
        }
        chosen
    }

    fn context_menu(&mut self, ui: &mut Ui) {
        let Some(((x, y), item)) = self.ctx.clone() else { return };
        let items: Vec<(&str, String, bool)> = match &item {
            Some(_) => vec![
                ("open", "Abrir".into(), true),
                ("", String::new(), false),
                ("info", "Obter Informações".into(), true),
                ("rename", "Renomear".into(), true),
                ("duplicate", "Duplicar".into(), !self.in_trash()),
                ("alias", "Criar Atalho".into(), !self.in_trash()),
                ("compress", self.compress_label(), !self.in_trash()),
                ("", String::new(), false),
                ("copy", "Copiar".into(), true),
                ("cut", "Recortar".into(), true),
                ("", String::new(), false),
                if self.in_trash() { ("put-back", "Devolver".into(), true) } else { ("trash", "Mover para a Lixeira".into(), true) },
            ],
            None => vec![
                ("new-folder", "Nova pasta".into(), true),
                ("paste", "Colar".into(), self.clip.is_some()),
                ("", String::new(), false),
                ("hidden", if self.hidden { "Esconder arquivos ocultos".into() } else { "Mostrar arquivos ocultos".into() }, true),
            ],
        };
        if let Some(a) = Self::popup_menu(ui, x, y, &items) {
            self.ctx = None;
            self.action(&a, ui);
        } else if ui.keys().iter().any(|k| k.key == Key::Escape) || (ui.mouse_pressed() && !ui.right_clicked(Rect::new(0.0, 0.0, ui.c.w, ui.c.h))) {
            // Clique fora fecha (o clique dentro escolhe no quadro em que solta).
            let (px, py) = ui.pointer();
            let inside = px >= x.min(ui.c.w - 224.0) && px < x.min(ui.c.w - 224.0) + 220.0 && (py - y).abs() < 260.0;
            if !inside || ui.keys().iter().any(|k| k.key == Key::Escape) {
                self.ctx = None;
            }
        }
    }

    /// Campo de renomear sobre o nome do item, se ele estiver nesta área.
    fn rename_field(&mut self, ui: &mut Ui, path: &Path, r: Rect) {
        let Some((target, mut text, first)) = self.rename.clone() else { return };
        if target != path {
            return;
        }
        self.rename_shown = true;
        // Nome todo selecionado: digitar substitui, Backspace apaga, setas tiram a seleção.
        let mut focus_now = first;
        if self.rename_pristine {
            let typed = !ui.typed().is_empty();
            let back = ui.keys().iter().any(|k| matches!(k.key, Key::Backspace | Key::Delete));
            let moved = ui.keys().iter().any(|k| matches!(k.key, Key::Left | Key::Right | Key::Home | Key::End));
            if typed || back {
                text.clear();
                focus_now = true;
                self.rename_pristine = false;
            } else if moved {
                self.rename_pristine = false;
            }
        }
        let (_, enter) = ui.text_field_in(r, &mut text, focus_now);
        if self.rename_pristine {
            let tw = ui.text_width(&text, (r.h * 0.62).min(13.0), false);
            ui.c.fill_rect(r.x + 3.0, r.y + 1.0, tw + 2.0, r.h - 2.0, &solid(with_alpha(ui.theme.accent, 0.35)));
        }
        let esc = ui.keys().iter().any(|k| k.key == Key::Escape);
        if esc {
            self.rename = None;
        } else if enter || (!first && !ui.has_focus()) {
            self.rename = None;
            self.finish_rename(&target, text.trim());
        } else {
            self.rename = Some((target, text, false));
        }
    }

    fn finish_rename(&mut self, path: &Path, name: &str) {
        let old = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.is_empty() || name == old {
            return;
        }
        if name.contains('/') || name == "." || name == ".." {
            self.message = Some("O nome não pode ter “/”.".into());
            return;
        }
        let dest = path.with_file_name(name);
        if dest.exists() {
            self.message = Some(format!("Já existe um item chamado “{name}” nesta pasta."));
            return;
        }
        match std::fs::rename(path, &dest) {
            Ok(()) => {
                if self.selected.as_deref() == Some(path) {
                    self.selected = Some(dest.clone());
                }
                for c in &mut self.columns {
                    if c.starts_with(path) {
                        *c = dest.join(c.strip_prefix(path).unwrap());
                    }
                }
                self.cache.clear();
            }
            Err(e) => self.message = Some(format!("Não foi possível renomear: {e}")),
        }
    }

    /// Lugares da barra lateral: (nome, ícone, pasta); None = divisória.
    fn places() -> Vec<Option<(String, Icon, PathBuf)>> {
        let user = std::env::var("USER").unwrap_or_else(|_| "Início".into());
        let mut v = vec![Some(("Disco".to_string(), Icon::Disk, PathBuf::from("/"))), None];
        let mut add = |name: &str, icon: Icon, p: PathBuf| {
            if p.is_dir() || apps::is_virtual(&p) || smart::is_smart(&p) {
                v.push(Some((name.to_string(), icon, p)));
            }
        };
        // Pastas do usuário como o sistema as define (nomes e caminhos variam por idioma).
        let add_user = |name: &str, key: &str, add: &mut dyn FnMut(&str, Icon, PathBuf)| {
            if let Some(p) = user_dir(key) {
                add(name, Icon::Folder, p);
            }
        };
        add_user("Mesa", "DESKTOP", &mut add);
        add(&user, Icon::Folder, home());
        add("Aplicativos", Icon::SpecialFolder(icons::FolderKind::Applications), apps::apps_dir());
        add_user("Documentos", "DOCUMENTS", &mut add);
        add_user("Downloads", "DOWNLOAD", &mut add);
        add_user("Imagens", "PICTURES", &mut add);
        add_user("Música", "MUSIC", &mut add);
        add_user("Vídeos", "VIDEOS", &mut add);
        // Pastas inteligentes salvas.
        let saved = smart::saved();
        if !saved.is_empty() {
            v.push(None);
            for n in saved {
                v.push(Some((n.clone(), Icon::SpecialFolder(icons::FolderKind::Smart), smart::saved_path(&n))));
            }
        }
        v
    }

    fn sidebar(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(231, 235, 241, 255)));
        ui.c.vline(r.x + r.w - 1.0, r.y, r.h, rgba(160, 162, 168, 255));
        let mut y = r.y + 6.0;
        for place in Self::places() {
            let Some((name, icon, path)) = place else {
                ui.c.hline(r.x + 10.0, y + 4.0, r.w - 20.0, rgba(196, 198, 204, 255));
                y += 10.0;
                continue;
            };
            let row = Rect::new(r.x, y, r.w - 1.0, SIDE_ROW);
            let here = self.base == path;
            let h = ui.hit(row);
            if h.hover && self.drag.as_ref().is_some_and(|d| d.2) {
                self.drop_target = Some(path.clone());
                ui.c.stroke(rrect(row.x + 2.0, row.y + 1.0, row.w - 4.0, row.h - 2.0, 4.0), &solid(ui.theme.accent), 2.0);
            }
            if here {
                ui.c.fill_rect(row.x, row.y, row.w, row.h, &vgrad(row.y, row.y + row.h, &[(0.0, lighten(ui.theme.accent, 0.25)), (1.0, ui.theme.accent)]));
            }
            let icon = if icon == Icon::Folder { special_folder(&path).map(Icon::SpecialFolder).unwrap_or(icon) } else { icon };
            icons::draw_bare(ui.c, ui.fonts, icon, r.x + 12.0, y + 3.0, 16.0, None);
            let col = if here { Color::WHITE } else { rgba(30, 30, 34, 255) };
            ui.fonts.draw(ui.c, &name, r.x + 34.0, y + 15.5, 12.0, here, col);
            if h.clicked {
                self.go(path);
            }
            y += SIDE_ROW;
        }
    }

    fn status(&mut self, ui: &mut Ui, y: f32) {
        let w = ui.c.w;
        let metal = Theme::new(Variant::Metal);
        ui.c.brushed_metal(0.0, y, w, BOTTOM_H, metal.metal_top, metal.metal_bottom);
        ui.c.hline(0.0, y, w, rgba(110, 110, 116, 255));
        let cur = self.current();
        let n = self.list(&cur).map(|v| v.len()).unwrap_or(0);
        let mut text = if self.selected.as_ref().is_some_and(|s| s.parent() == Some(cur.as_path())) {
            format!("1 de {n} selecionado")
        } else {
            format!("{n} {}", if n == 1 { "item" } else { "itens" })
        };
        // Pasta em que não se pode escrever (como o cadeado do Mac).
        if !apps::is_virtual(&cur) && !smart::is_smart(&cur) && !writable(&cur) {
            text.push_str(" · somente leitura");
        }
        if let Some(free) = free_space(&cur) {
            text.push_str(&format!(", {} disponíveis", human_size(free)));
        }
        if let Some(m) = &self.message {
            text = m.clone();
        }
        let tw = ui.text_width(&text, 11.0, false);
        ui.fonts.draw(ui.c, &text, (w - tw) / 2.0, y + 15.0, 11.0, false, rgba(40, 40, 40, 255));
    }

    // -----------------------------------------------------------------------
    // Colunas
    // -----------------------------------------------------------------------

    fn columns_view(&mut self, ui: &mut Ui, area: Rect) {
        ui.c.fill_rect(area.x, area.y, area.w, area.h, &solid(Color::WHITE));
        // Mostra as últimas colunas que cabem (+ a de detalhes, se houver arquivo).
        let sel = self.selected_entry();
        let preview = sel.as_ref().is_some_and(|e| e.kind != Kind::Folder);
        let total = self.columns.len() + preview as usize;
        let fit = ((area.w / COL_W).floor() as usize).max(1);
        let first = total.saturating_sub(fit);
        let cols = self.columns.clone();
        let mut x = area.x;
        for (ci, dir) in cols.iter().enumerate().skip(first) {
            let r = Rect::new(x, area.y, COL_W, area.h);
            self.column(ui, ci, dir, r);
            ui.c.vline(x + COL_W - 1.0, area.y, area.h, rgba(190, 190, 196, 255));
            x += COL_W;
        }
        if preview {
            if let Some(e) = sel {
                self.preview(ui, &e, Rect::new(x, area.y, (area.x + area.w - x).max(COL_W), area.h));
            }
        }
    }

    fn column(&mut self, ui: &mut Ui, ci: usize, dir: &Path, r: Rect) {
        let entries = match self.list(dir) {
            Ok(v) => v,
            Err(e) => {
                ui.fonts.draw(ui.c, &e, r.x + 8.0, r.y + 20.0, 11.0, false, rgba(150, 40, 40, 255));
                return;
            }
        };
        let content = entries.len() as f32 * ROW_H;
        let mut off = *self.scroll.get(dir).unwrap_or(&0.0);
        let r = ui.scroll_area(r, &mut off, content, ROW_H);
        self.scroll.insert(dir.to_path_buf(), off);

        // Qual item desta coluna está "no caminho" (selecionado ou aberto à direita)?
        let chosen = self.columns.get(ci + 1).cloned().or_else(|| self.selected.clone().filter(|s| s.parent() == Some(dir)));
        let last = ci + 1 == self.columns.len() || (ci + 2 == self.columns.len() && chosen.as_ref() == self.columns.last());
        let first = (off / ROW_H).floor() as usize;
        let count = (r.h / ROW_H).ceil() as usize + 1;
        let mut action: Option<(Entry, bool)> = None;
        for (i, e) in entries.iter().enumerate().skip(first).take(count) {
            let y = r.y + i as f32 * ROW_H - off;
            let top = y.max(r.y);
            let bot = (y + ROW_H).min(r.y + r.h);
            if bot <= top {
                continue;
            }
            let resp = ui.hit(Rect::new(r.x, top, r.w - 1.0, bot - top));
            if resp.pressed {
                action = Some((e.clone(), resp.double));
                self.drag = Some((e.path.clone(), ui.pointer(), false));
            }
            if resp.hover && e.kind == Kind::Folder && self.drag.as_ref().is_some_and(|d| d.2) {
                self.drop_target = Some(e.path.clone());
            }
            if ui.right_clicked(Rect::new(r.x, top, r.w - 1.0, bot - top)) {
                action = Some((e.clone(), false));
                self.ctx = Some((ui.pointer(), Some(e.path.clone())));
            }
        }
        let accent = ui.theme.accent;
        let extra = self.extra.clone();
        ui.clipped(ci as u64 * 7919 + 1, Rect::new(r.x, r.y, r.w - 1.0, r.h), |c, fonts| {
            c.clear(Color::WHITE);
            for (i, e) in entries.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * ROW_H - off;
                let on = chosen.as_ref() == Some(&e.path) || extra.contains(&e.path);
                if on {
                    let col = if last { accent } else { rgba(200, 204, 212, 255) };
                    c.fill_rect(0.0, y, r.w, ROW_H, &vgrad(y, y + ROW_H, &[(0.0, lighten(col, 0.15)), (1.0, col)]));
                }
                e.draw_icon(c, fonts, 6.0, y + 1.5, 16.0);
                let ink = if on && last { Color::WHITE } else { rgba(20, 20, 20, 255) };
                let name = fonts.ellipsize(&e.name, 12.0, false, r.w - 44.0);
                fonts.draw(c, &name, 26.0, y + 14.0, 12.0, false, ink);
                if e.kind == Kind::Folder {
                    let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
                    let (ax, ay) = (r.w - 14.0, y + ROW_H / 2.0);
                    pb.move_to(ax, ay - 4.0);
                    pb.line_to(ax + 5.0, ay);
                    pb.line_to(ax, ay + 4.0);
                    pb.close();
                    c.fill(pb.finish(), &solid(if on && last { Color::WHITE } else { rgba(90, 90, 96, 255) }));
                }
            }
        });
        // Renomeando um item desta coluna: o campo fica por cima do nome.
        if let Some(i) = self.rename.as_ref().and_then(|(p, _, _)| entries.iter().position(|e| &e.path == p)) {
            let y = r.y + i as f32 * ROW_H - off;
            if y >= r.y && y + ROW_H <= r.y + r.h {
                let path = entries[i].path.clone();
                self.rename_field(ui, &path, Rect::new(r.x + 24.0, y + 1.0, r.w - 44.0, ROW_H - 2.0));
            }
        }
        if let Some((e, double)) = action {
            if !double && self.multi_click(ui, &e, &entries) {
                ui.redraw = true;
                return;
            }
            self.select_in_column(ci, &e);
            if double {
                self.open(&e);
            }
            ui.redraw = true;
        }
    }

    fn preview(&mut self, ui: &mut Ui, e: &Entry, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        let cx = r.x + r.w / 2.0;
        e.draw_icon(ui.c, ui.fonts, cx - 48.0, r.y + 24.0, 96.0);
        let mut y = r.y + 140.0;
        let name = ui.fonts.ellipsize(&e.name, 13.0, true, r.w - 20.0);
        let tw = ui.text_width(&name, 13.0, true);
        ui.fonts.draw(ui.c, &name, cx - tw / 2.0, y, 13.0, true, rgba(20, 20, 20, 255));
        y += 26.0;
        for (k, v) in [("Tipo:", e.type_name().to_string()), ("Tamanho:", human_size(e.size)), ("Modificado:", human_date(e.modified))] {
            let kw = ui.text_width(k, 11.0, false);
            ui.fonts.draw(ui.c, k, cx - 6.0 - kw, y, 11.0, false, rgba(110, 110, 110, 255));
            ui.fonts.draw(ui.c, &v, cx, y, 11.0, false, rgba(20, 20, 20, 255));
            y += 17.0;
        }
    }

    // -----------------------------------------------------------------------
    // Lista
    // -----------------------------------------------------------------------

    /// Itens da lista com as pastas abertas no lugar: (item, nível), na ordem escolhida.
    fn list_rows(&mut self, dir: &Path, depth: u16, out: &mut Vec<(Entry, u16)>) {
        let Ok(mut entries) = self.list(dir) else { return };
        let (col, asc) = self.list_sort;
        entries.sort_by(|a, b| {
            let o = match col {
                1 => a.modified.cmp(&b.modified),
                2 => a.size.cmp(&b.size),
                3 => a.type_name().cmp(b.type_name()).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };
            if asc { o } else { o.reverse() }
        });
        for e in entries {
            let open = e.kind == Kind::Folder && self.expanded.contains(&e.path);
            let path = e.path.clone();
            out.push((e, depth));
            if open && depth < 12 {
                self.list_rows(&path, depth + 1, out);
            }
        }
    }

    fn list_view(&mut self, ui: &mut Ui, area: Rect) {
        let dir = self.current();
        if let Err(e) = self.list(&dir) {
            ui.fonts.draw(ui.c, &e, area.x + 10.0, area.y + 24.0, 12.0, false, rgba(150, 40, 40, 255));
            return;
        }
        let mut flat = Vec::new();
        self.list_rows(&dir, 0, &mut flat);
        let entries: Vec<Entry> = flat.iter().map(|(e, _)| e.clone()).collect();
        // Nas buscas, a última coluna diz onde cada item está (como no Tiger).
        let searching = smart::is_smart(&dir);
        let cols = [
            Column::new("Nome", 0.45),
            Column::new("Data de modificação", 0.25),
            Column::new("Tamanho", 0.12).right(),
            Column::new(if searching { "Onde" } else { "Tipo" }, 0.18),
        ];
        let rows: Vec<TableRow> = flat
            .iter()
            .map(|(e, depth)| TableRow {
                cells: vec![
                    e.name.clone(),
                    human_date(e.modified),
                    if e.kind == Kind::Folder { "--".into() } else { human_size(e.size) },
                    if searching { e.path.parent().map(display_name).unwrap_or_default() } else { e.type_name().to_string() },
                ],
                icon: Some(e.icon()),
                depth: *depth,
                children: (e.kind == Kind::Folder && !apps::is_virtual(&e.path)).then(|| self.expanded.contains(&e.path)),
            })
            .collect();
        let sel = self.selected.as_ref().and_then(|s| entries.iter().position(|e| &e.path == s));
        let mut state = TableState::sorted_by(self.list_sort.0, self.list_sort.1);
        state.scroll = *self.scroll.get(&dir).unwrap_or(&0.0);
        // O Arquivos não usa linhas alternadas (como o Finder).
        let r = ui.table(area, &cols, &rows, sel, &mut state, &TableStyle { row_h: ROW_H, px: 12.0, zebra: false });
        if let (Some(i), true) = (sel, std::mem::take(&mut self.reveal)) {
            state.reveal(i);
            ui.redraw = true;
        }
        self.scroll.insert(dir.clone(), state.scroll);
        // Linha sob o ponto (botão direito e campo de renomear).
        let row_top = |i: usize| area.y + glove_kit::TABLE_HEADER_H + i as f32 * ROW_H - state.scroll;
        if ui.right_clicked(area) {
            let py = ui.pointer().1;
            let i = ((py - area.y - glove_kit::TABLE_HEADER_H + state.scroll) / ROW_H).floor();
            if i >= 0.0 && (i as usize) < entries.len() && py > area.y + glove_kit::TABLE_HEADER_H {
                let e = &entries[i as usize];
                self.selected = Some(e.path.clone());
                self.ctx = Some((ui.pointer(), Some(e.path.clone())));
            }
        }
        if let Some(i) = self.rename.as_ref().and_then(|(p, _, _)| entries.iter().position(|e| &e.path == p)) {
            let y = row_top(i);
            let path = entries[i].path.clone();
            if y >= area.y + glove_kit::TABLE_HEADER_H && y + ROW_H <= area.y + area.h {
                self.rename_field(ui, &path, Rect::new(area.x + 26.0, y + 1.0, area.w * 0.4, ROW_H - 2.0));
            } else {
                // Fora da vista: desiste (as teclas voltam para a janela).
                self.rename = None;
            }
        }
        // Cabeçalho clicado: guarda a nova ordenação (vale no próximo quadro).
        if r.sorted {
            if let Some(sort) = state.sort {
                self.list_sort = sort;
                ui.redraw = true;
            }
        }
        // Triângulo: abre ou fecha a pasta no lugar.
        if let Some(i) = r.toggled {
            let p = entries[i].path.clone();
            if !self.expanded.remove(&p) {
                self.expanded.insert(p);
            }
            ui.redraw = true;
        }
        let action = r.pressed.map(|i| (entries[i].clone(), r.double == Some(i)));
        if let Some((e, double)) = action {
            self.selected = Some(e.path.clone());
            if double {
                self.open(&e);
            }
            ui.redraw = true;
        }
    }

    // -----------------------------------------------------------------------
    // Ícones
    // -----------------------------------------------------------------------

    fn icons_view(&mut self, ui: &mut Ui, area: Rect) {
        let dir = self.current();
        let entries = match self.list(&dir) {
            Ok(v) => v,
            Err(e) => {
                ui.fonts.draw(ui.c, &e, area.x + 10.0, area.y + 24.0, 12.0, false, rgba(150, 40, 40, 255));
                return;
            }
        };
        let (cw, ch) = (104.0, 92.0);
        let per_row = ((area.w - SCROLLBAR_W - 16.0) / cw).floor().max(1.0) as usize;
        let rows = entries.len().div_ceil(per_row);
        let content = rows as f32 * ch + 16.0;
        let mut off = *self.scroll.get(&dir).unwrap_or(&0.0);
        let area = ui.scroll_area(area, &mut off, content, ROW_H);
        self.scroll.insert(dir.clone(), off);
        let mut action: Option<(Entry, bool)> = None;
        let cell = |i: usize| (8.0 + (i % per_row) as f32 * cw, 8.0 + (i / per_row) as f32 * ch - off);
        for (i, e) in entries.iter().enumerate() {
            let (x, y) = cell(i);
            if y + ch < 0.0 || y > area.h {
                continue;
            }
            let r = Rect::new(area.x + x, (area.y + y).max(area.y), cw, ch.min(area.y + area.h - (area.y + y).max(area.y)));
            if r.h > 0.0 {
                let resp = ui.hit(r);
                if resp.pressed {
                    action = Some((e.clone(), resp.double));
                    self.drag = Some((e.path.clone(), ui.pointer(), false));
                }
                if resp.hover && e.kind == Kind::Folder && self.drag.as_ref().is_some_and(|d| d.2) {
                    self.drop_target = Some(e.path.clone());
                }
                if ui.right_clicked(r) {
                    action = Some((e.clone(), false));
                    self.ctx = Some((ui.pointer(), Some(e.path.clone())));
                }
            }
        }
        // Retângulo elástico: arrastar no vazio escolhe os ícones que ele toca.
        let any_item = action.is_some();
        if ui.mouse_pressed() && !any_item {
            let (px, py) = ui.pointer();
            if px >= area.x && px < area.x + area.w && py >= area.y && py < area.y + area.h && self.ctx.is_none() {
                self.band = Some((px, py));
                self.selected = None;
                self.extra.clear();
            }
        }
        let mut band_rect = None;
        if let Some((bx, by)) = self.band {
            let (px, py) = ui.pointer();
            let r = Rect::new(bx.min(px), by.min(py), (px - bx).abs(), (py - by).abs());
            let hits: Vec<PathBuf> = entries
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    let (x, y) = cell(*i);
                    let c = Rect::new(area.x + x + cw / 2.0 - 30.0, area.y + y, 60.0, 80.0);
                    c.x < r.x + r.w && c.x + c.w > r.x && c.y < r.y + r.h && c.y + c.h > r.y
                })
                .map(|(_, e)| e.path.clone())
                .collect();
            self.selected = hits.first().cloned();
            self.extra = hits.into_iter().skip(1).collect();
            band_rect = Some(r);
            if ui.mouse_released() || !ui.mouse_down() {
                self.band = None;
            }
            ui.wake_after(std::time::Duration::from_millis(30));
        }
        let sel = self.selected.clone();
        let extra = self.extra.clone();
        let accent = ui.theme.accent;
        ui.clipped(92, area, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, e) in entries.iter().enumerate() {
                let (x, y) = cell(i);
                if y + ch < 0.0 || y > area.h {
                    continue;
                }
                let on = sel.as_ref() == Some(&e.path) || extra.contains(&e.path);
                if on {
                    c.fill(rrect(x + cw / 2.0 - 30.0, y, 60.0, 56.0, 6.0), &solid(rgba(0, 0, 0, 40)));
                }
                e.draw_icon(c, fonts, x + cw / 2.0 - 26.0, y + 2.0, 52.0);
                let name = fonts.ellipsize(&e.name, 12.0, false, cw - 8.0);
                let tw = fonts.measure(&name, 12.0, false);
                let (lx, ly) = (x + (cw - tw) / 2.0, y + 72.0);
                if on {
                    c.fill(rrect(lx - 5.0, ly - 12.0, tw + 10.0, 16.0, 8.0), &solid(accent));
                }
                fonts.draw(c, &name, lx, ly, 12.0, false, if on { Color::WHITE } else { rgba(20, 20, 20, 255) });
            }
        });
        if let Some(i) = self.rename.as_ref().and_then(|(p, _, _)| entries.iter().position(|e| &e.path == p)) {
            let (x, y) = cell(i);
            let path = entries[i].path.clone();
            self.rename_field(ui, &path, Rect::new(area.x + x + 2.0, area.y + y + 60.0, cw - 4.0, 18.0));
        }
        if let Some(r) = band_rect {
            ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(with_alpha(ui.theme.accent, 0.18)));
            ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w, r.h, 0.0), &solid(with_alpha(ui.theme.accent, 0.7)), 1.0);
        }
        if let Some((e, double)) = action {
            if double || !self.multi_click(ui, &e, &entries) {
                self.selected = Some(e.path.clone());
            }
            if double {
                self.open(&e);
            }
            ui.redraw = true;
        }
    }

    // -----------------------------------------------------------------------
    // Teclado
    // -----------------------------------------------------------------------

    fn keyboard(&mut self, ui: &mut Ui) {
        // Renomeando ou no "Ir para a pasta": as teclas são do campo.
        if self.rename.is_some() || self.goto.is_some() {
            return;
        }
        let mut act: Vec<&str> = Vec::new();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => act.push("new-window"),
                "Ctrl+Shift+N" => act.push("new-folder"),
                "Ctrl+O" => act.push("open"),
                "Ctrl+BackSpace" => act.push("trash"),
                "Ctrl+Shift+BackSpace" => act.push("empty-trash"),
                "F2" => act.push("rename"),
                "Ctrl+I" => act.push("info"),
                "Ctrl+D" => act.push("duplicate"),
                "Ctrl+L" => act.push("alias"),
                "Ctrl+C" => act.push("copy"),
                "Ctrl+X" => act.push("cut"),
                "Ctrl+V" => act.push("paste"),
                "Ctrl+W" => act.push("close"),
                "Ctrl+1" => act.push("view-icons"),
                "Ctrl+2" => act.push("view-list"),
                "Ctrl+3" => act.push("view-columns"),
                "Ctrl+H" => act.push("hidden"),
                "Ctrl+bracketleft" => act.push("back"),
                "Ctrl+bracketright" => act.push("forward"),
                "Ctrl+Up" => act.push("up"),
                "Ctrl+Shift+C" => act.push("go-computer"),
                "Ctrl+Shift+H" => act.push("go-home"),
                "Ctrl+Shift+D" => act.push("go-desktop"),
                "Ctrl+Shift+A" => act.push("go-apps"),
                "Ctrl+Shift+U" => act.push("go-utils"),
                "Ctrl+Shift+O" => act.push("go-documents"),
                "Ctrl+Shift+L" => act.push("go-downloads"),
                "Ctrl+Shift+G" => act.push("go-folder"),
                _ => match k.key {
                    Key::Up => act.push("sel-up"),
                    Key::Down => act.push("sel-down"),
                    Key::Right => act.push("sel-right"),
                    Key::Left => act.push("sel-left"),
                    Key::Enter => act.push("open"),
                    _ => {}
                },
            }
        }
        let menu: Vec<String> = ui.menu_actions().to_vec();
        for a in act.into_iter().map(String::from).chain(menu) {
            self.action(&a, ui);
        }
    }

    fn action(&mut self, a: &str, ui: &mut Ui) {
        match a {
            "new-window" => {
                let _ = launch(&mut std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "glove-files".into())).arg(self.current()));
            }
            "new-folder" => self.new_folder(),
            "open" => {
                if let Some(e) = self.selected_entry() {
                    if e.kind == Kind::Folder {
                        self.go(e.path);
                    } else {
                        open_file(&e);
                    }
                }
            }
            "trash" => self.trash_selected(),
            "copy" | "cut" => {
                let all = self.all_selected();
                if !all.is_empty() {
                    // Para outros apps, os caminhos vão também como texto (file://, um por linha).
                    ui.copy(&all.iter().map(|p| format!("file://{}", p.display())).collect::<Vec<_>>().join("\n"));
                    self.clip = Some((all, a == "cut"));
                    self.message = Some(if a == "cut" { "Recortado: cole (Ctrl+V) na pasta de destino.".into() } else { "Copiado.".into() });
                }
            }
            "rename" => {
                if let Some(p) = self.selected.clone() {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    self.rename = Some((p, name, true));
                    self.rename_pristine = true;
                }
            }
            "duplicate" => {
                if let Some(p) = self.selected.clone() {
                    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    let mut dest = p.with_file_name(format!("{stem} cópia{ext}"));
                    if dest.exists() {
                        dest = ops::free_name(&dest);
                    }
                    self.start_job(vec![ops::Item { src: p, dest, replace: false }], false);
                }
            }
            "compress" => {
                let items = self.all_selected();
                if !items.is_empty() {
                    if let Err(e) = launch(std::process::Command::new(sibling("glove-arquivo")).arg("compactar").args(&items)) {
                        self.message = Some(format!("Não foi possível abrir o Utilitário de Arquivo: {e}"));
                    }
                }
            }
            "alias" => {
                if let Some(p) = self.selected.clone() {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let mut link = p.with_file_name(format!("{name} atalho"));
                    if link.exists() {
                        link = ops::free_name(&link);
                    }
                    match std::os::unix::fs::symlink(&p, &link) {
                        Ok(()) => {
                            self.cache.clear();
                            self.selected = Some(link);
                        }
                        Err(e) => self.message = Some(format!("Não foi possível criar o atalho: {e}")),
                    }
                }
            }
            // Uma janela de informações por item (como no Mac).
            "info" => {
                if let Some(p) = self.selected.clone().or_else(|| Some(self.current())) {
                    let _ = launch(&mut std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "glove-files".into())).arg("--info").arg(p));
                }
            }
            "paste" => match self.clip.clone() {
                Some((srcs, mv)) => {
                    let dir = self.current();
                    self.transfer(srcs, dir, mv);
                }
                None => {
                    ui.request_paste();
                    self.paste_wait = true;
                }
            },
            "put-back" => {
                if let Some(p) = self.selected.clone() {
                    match put_back(&p) {
                        Ok(dest) => {
                            self.message = Some(format!("“{}” voltou para {}", display_name(&dest), dest.parent().map(display_name).unwrap_or_default()));
                            self.selected = None;
                            self.cache.clear();
                        }
                        Err(e) => self.message = Some(e),
                    }
                }
            }
            "empty-trash" => {
                let _ = launch(&mut std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "glove-files".into())).arg("--esvaziar-lixeira"));
            }
            "close" => ui.close = true,
            "view-icons" => self.view = View::Icons,
            "view-list" => self.view = View::List,
            "view-columns" => self.view = View::Columns,
            "hidden" => {
                self.hidden = !self.hidden;
                self.cache.clear();
            }
            "back" => self.go_back(),
            "forward" => self.go_forward(),
            "up" => self.go_up(),
            "go-computer" => self.go("/".into()),
            "go-home" => self.go(home()),
            "go-documents" => self.go(user_dir("DOCUMENTS").unwrap_or_else(home)),
            "go-pictures" => self.go(user_dir("PICTURES").unwrap_or_else(home)),
            "go-desktop" => self.go(user_dir("DESKTOP").unwrap_or_else(home)),
            "go-downloads" => self.go(user_dir("DOWNLOAD").unwrap_or_else(home)),
            "go-apps" => self.go(apps::apps_dir()),
            "go-utils" => self.go(apps::utils_dir()),
            "go-folder" => self.goto = Some((format!("{}/", self.current().display()).replace("//", "/"), true)),
            a if a.starts_with("recent:") => {
                if let Some(p) = a[7..].parse::<usize>().ok().and_then(|i| self.recent.get(i).cloned()) {
                    self.go(p);
                }
            }
            "go-trash" => self.go(trash_dir().join("files")),
            "sel-up" | "sel-down" => {
                let dir = match (self.view, &self.selected) {
                    (_, Some(s)) => s.parent().map(Path::to_path_buf).unwrap_or_else(|| self.current()),
                    _ => self.current(),
                };
                let Ok(list) = self.list(&dir) else { return };
                if list.is_empty() {
                    return;
                }
                let pos = self.selected.as_ref().and_then(|s| list.iter().position(|e| &e.path == s));
                let next = match (pos, a) {
                    (None, _) => 0,
                    (Some(p), "sel-up") => p.saturating_sub(1),
                    (Some(p), _) => (p + 1).min(list.len() - 1),
                };
                let e = list[next].clone();
                if self.view == View::Columns {
                    let ci = self.columns.iter().position(|c| *c == dir).unwrap_or(0);
                    self.select_in_column(ci, &e);
                } else {
                    self.selected = Some(e.path);
                }
                self.reveal = true;
            }
            "sel-right" if self.view == View::Columns => {
                if let Some(e) = self.selected_entry().filter(|e| e.kind == Kind::Folder) {
                    if let Ok(first) = self.list(&e.path).map(|v| v.first().cloned()) {
                        if let Some(f) = first {
                            let ci = self.columns.len() - 1;
                            self.select_in_column(ci, &f);
                        }
                    }
                }
            }
            "sel-left" if self.view == View::Columns => {
                // Volta a seleção para a pasta que contém o item (fecha a coluna à direita).
                if let Some(parent) = self.selected.as_ref().and_then(|s| s.parent()).map(Path::to_path_buf) {
                    if parent != self.base {
                        if let Some(k) = self.columns.iter().position(|c| *c == parent) {
                            self.columns.truncate(k + 1);
                            self.selected = Some(parent);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl App for Files {
    fn ui(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        let title = display_name(&self.current());
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        self.drop_target = None;
        self.sync_watches();
        // Colar vindo de outro app: o texto chega num quadro seguinte.
        if self.paste_wait {
            if let Some(t) = ui.take_paste() {
                self.paste_wait = false;
                let srcs: Vec<PathBuf> = t
                    .lines()
                    .map(|l| l.trim())
                    .filter_map(|l| l.strip_prefix("file://").map(|p| PathBuf::from(ops_decode(p))).or_else(|| l.starts_with('/').then(|| PathBuf::from(l))))
                    .filter(|p| p.exists())
                    .collect();
                let dir = self.current();
                self.transfer(srcs, dir, false);
            }
        }
        // Com o menu da engrenagem aberto, um clique fora só o fecha.
        // (O menu fica embaixo da engrenagem; fora dele e da barra, fecha.)
        if self.gear_open && ui.mouse_pressed() {
            let (px, py) = ui.pointer();
            let mx = 12.0 + 5.0 * SEGMENT_W + 24.0;
            let inside_menu = px >= mx && px < mx + 210.0 && py >= TOOLBAR_H - 4.0 && py < TOOLBAR_H + 120.0;
            if !inside_menu && py > TOOLBAR_H {
                self.gear_open = false;
            }
        }
        self.toolbar(ui);
        let body_h = h - TOOLBAR_H - BOTTOM_H;
        self.sidebar(ui, Rect::new(0.0, TOOLBAR_H, SIDEBAR_W, body_h));
        let mut area = Rect::new(SIDEBAR_W, TOOLBAR_H, w - SIDEBAR_W, body_h);
        // Pasta de busca: faixa com o que se procura e Salvar (ou Apagar a busca salva).
        if smart::is_smart(&self.base) {
            area = self.search_bar(ui, area);
        }
        match self.view {
            View::Columns => self.columns_view(ui, area),
            View::List => self.list_view(ui, area),
            View::Icons => self.icons_view(ui, area),
        }
        // Faixa de baixo em metal com o status (e o puxador de redimensionar).
        self.status(ui, h - BOTTOM_H);
        // O item sendo renomeado saiu da vista: desiste (as teclas voltam para a janela).
        if self.rename.is_some() && !std::mem::take(&mut self.rename_shown) {
            self.rename = None;
        }
        // Botão direito no vazio da área de itens: menu da pasta.
        if self.ctx.is_none() && ui.right_clicked(area) {
            self.ctx = Some((ui.pointer(), None));
        }
        self.gear_menu(ui);
        self.context_menu(ui);
        self.job_panel(ui);
        self.goto_sheet(ui);
        self.drag_frame(ui);
        ui.set_enabled(self.ask.is_none());
        self.ask_sheet(ui);
    }

    fn watch_fd(&self) -> Option<std::os::fd::RawFd> {
        (self.notify_fd >= 0).then_some(self.notify_fd)
    }

    /// Algo mudou numa pasta à vista: relê (o cache confere a data da pasta).
    fn fd_ready(&mut self) -> bool {
        let mut buf = [0u8; 4096];
        while unsafe { libc::read(self.notify_fd, buf.as_mut_ptr() as *mut _, buf.len()) } > 0 {}
        self.cache.clear();
        true
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.selected.is_some();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new-window", "Nova janela", "Ctrl+N"),
                    MenuItem::new("new-folder", "Nova pasta", "Ctrl+Shift+N"),
                    MenuItem::new("open", "Abrir", "Ctrl+O").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("trash", "Mover para a Lixeira", "Ctrl+BackSpace").enabled(sel && !self.in_trash()),
                    MenuItem::new("put-back", "Devolver", "").enabled(sel && self.in_trash()),
                    MenuItem::Separator,
                    MenuItem::new("info", "Obter Informações", "Ctrl+I"),
                    MenuItem::new("rename", "Renomear", "F2").enabled(sel),
                    MenuItem::new("duplicate", "Duplicar", "Ctrl+D").enabled(sel && !self.in_trash()),
                    MenuItem::new("alias", "Criar Atalho", "Ctrl+L").enabled(sel && !self.in_trash()),
                    MenuItem::new("compress", &self.compress_label(), "").enabled(sel && !self.in_trash()),
                    MenuItem::new("empty-trash", "Esvaziar Lixeira…", "Ctrl+Shift+BackSpace"),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar janela", "Ctrl+W"),
                ],
            ),
            Menu::new(
                "Editar",
                vec![
                    MenuItem::new("cut", "Recortar", "Ctrl+X").enabled(sel),
                    MenuItem::new("copy", "Copiar", "Ctrl+C").enabled(sel),
                    MenuItem::new("paste", "Colar", "Ctrl+V"),
                ],
            ),
            Menu::new(
                "Visualizar",
                vec![
                    MenuItem::new("view-icons", "como Ícones", "Ctrl+1").checked(self.view == View::Icons),
                    MenuItem::new("view-list", "como Lista", "Ctrl+2").checked(self.view == View::List),
                    MenuItem::new("view-columns", "como Colunas", "Ctrl+3").checked(self.view == View::Columns),
                    MenuItem::Separator,
                    MenuItem::new("hidden", "Mostrar arquivos ocultos", "Ctrl+H").checked(self.hidden),
                ],
            ),
            Menu::new(
                "Ir",
                vec![
                    MenuItem::new("back", "Voltar", "Ctrl+bracketleft").enabled(!self.back.is_empty()),
                    MenuItem::new("forward", "Avançar", "Ctrl+bracketright").enabled(!self.forward.is_empty()),
                    MenuItem::new("up", "Pasta acima", "Ctrl+Up"),
                    MenuItem::Separator,
                    MenuItem::new("go-computer", "Computador", "Ctrl+Shift+C"),
                    MenuItem::new("go-home", "Início", "Ctrl+Shift+H"),
                    MenuItem::new("go-desktop", "Mesa", "Ctrl+Shift+D"),
                    MenuItem::new("go-apps", "Aplicativos", "Ctrl+Shift+A"),
                    MenuItem::new("go-utils", "Utilitários", "Ctrl+Shift+U"),
                    MenuItem::new("go-documents", "Documentos", "Ctrl+Shift+O"),
                    MenuItem::new("go-downloads", "Downloads", "Ctrl+Shift+L"),
                    MenuItem::new("go-pictures", "Imagens", ""),
                    MenuItem::new("go-trash", "Lixeira", ""),
                    MenuItem::Separator,
                    if self.recent.len() > 1 {
                        MenuItem::submenu("Pastas Recentes", self.recent.iter().enumerate().skip(1).map(|(i, p)| MenuItem::new(&format!("recent:{i}"), &display_name(p), "")).collect())
                    } else {
                        MenuItem::submenu("Pastas Recentes", Vec::new()).enabled(false)
                    },
                    MenuItem::Separator,
                    MenuItem::new("go-folder", "Ir para a Pasta…", "Ctrl+Shift+G"),
                ],
            ),
        ]
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--info") {
        let Some(p) = args.get(i + 1).map(PathBuf::from).filter(|p| std::fs::symlink_metadata(p).is_ok()) else { return };
        let opts = WindowOptions { title: "Informações".into(), app_id: "glove.Files".into(), size: (300, 360), fixed: true };
        if let Err(e) = run(opts, info::Info::new(p)) {
            eprintln!("glove-files: {e}");
        }
        return;
    }
    if std::env::args().any(|a| a == "--esvaziar-lixeira") {
        let count = std::fs::read_dir(trash_dir().join("files")).map(|d| d.count()).unwrap_or(0);
        if count == 0 {
            return;
        }
        let opts = WindowOptions { title: "Esvaziar Lixeira".into(), app_id: "glove.Files".into(), size: (440, 130), fixed: true };
        if let Err(e) = run(opts, EmptyTrash { count, error: None }) {
            eprintln!("glove-files: {e}");
        }
        return;
    }
    let start = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.is_dir()).unwrap_or_else(home);
    let opts = WindowOptions { title: display_name(&start), app_id: "glove.Files".into(), size: (860, 480), fixed: false };
    if let Err(e) = run(opts, Files::new(start)) {
        eprintln!("glove-files: {e}");
    }
}

#[cfg(test)]
mod lixeira_tests {
    use super::*;

    #[test]
    fn devolver_e_esvaziar() {
        let root = std::env::temp_dir().join(format!("glove-lixeira-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let t = root.join("Trash");
        std::fs::create_dir_all(t.join("files")).unwrap();
        std::fs::create_dir_all(t.join("info")).unwrap();
        // Um item que veio de "pasta com espaço/nota.txt" (Path codificado como URL).
        std::fs::write(t.join("files/nota.txt"), "oi").unwrap();
        let orig = root.join("pasta com espaço/nota.txt");
        let encoded = orig.to_string_lossy().replace(' ', "%20").replace('ç', "%C3%A7");
        std::fs::write(t.join("info/nota.txt.trashinfo"), format!("[Trash Info]\nPath={encoded}\nDeletionDate=2026-01-01T00:00:00\n")).unwrap();
        let dest = put_back_in(&t, &t.join("files/nota.txt")).unwrap();
        assert_eq!(dest, orig);
        assert!(orig.is_file() && !t.join("info/nota.txt.trashinfo").exists());
        // Destino ocupado: recusa.
        std::fs::write(t.join("files/nota.txt"), "de novo").unwrap();
        std::fs::write(t.join("info/nota.txt.trashinfo"), format!("[Trash Info]\nPath={}\n", orig.display())).unwrap();
        assert!(put_back_in(&t, &t.join("files/nota.txt")).is_err());
        // Esvaziar apaga itens e registros (pastas também).
        std::fs::create_dir_all(t.join("files/pasta/sub")).unwrap();
        empty_trash_in(&t).unwrap();
        assert_eq!(std::fs::read_dir(t.join("files")).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(t.join("info")).unwrap().count(), 0);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
