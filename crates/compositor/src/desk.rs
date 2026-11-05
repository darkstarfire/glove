//! A mesa como o Arquivos (o Finder do Tiger): os itens da pasta Mesa e os
//! discos, numa grade a partir do canto superior direito; cada ícone fica onde o
//! usuário o deixou. Clique escolhe, clique duplo abre, arrastar move (para uma
//! pasta, para o Lixo, para um app do Dock ou para uma janela), retângulo
//! escolhe vários, Enter renomeia no lugar, botão direito abre o menu.
//!
//! O compositor desenha e guarda as posições; o que precisa de janela (copiar
//! com progresso, Obter Informações, Opções de Visualização) fica com o
//! `glove-files`, chamado com `--abrir`, `--transferir`, `--info`…

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use glove_paint::{
    chrome::{self, DeskStyle, MenuItem},
    icons::{self, Icon},
    theme::MENUBAR_H,
};
use smithay::utils::{Logical, Point, Rectangle};

use crate::{
    shell::{to_tex, BarTarget, DockAction, Hit, MenuAction, MenuNode, Tex},
    state::Glove,
};

/// Vão entre as células da grade.
const GAP: i32 = 6;
/// Margem da grade na borda da área útil.
const MARGIN: i32 = 12;
/// Distância que o mouse anda até um clique virar arrastar.
const DRAG_START: f64 = 5.0;

pub const URI_LIST: &str = "text/uri-list";
const GNOME_FILES: &str = "x-special/gnome-copied-files";
const TEXT: &str = "text/plain;charset=utf-8";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeskKind {
    Disk,
    Removable,
    Folder,
    File,
    /// Atalho de app (.desktop).
    App,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeskItem {
    pub name: String,
    pub path: PathBuf,
    pub kind: DeskKind,
    pub icon: Icon,
    pub link: bool,
    pub modified: Option<SystemTime>,
    pub size: u64,
    /// Rótulo de cor (0 a 7).
    pub label: u8,
}

impl DeskItem {
    /// Chave da posição guardada: o nome do arquivo; para discos, o ponto de montagem.
    fn key(&self) -> String {
        match self.kind {
            DeskKind::Disk | DeskKind::Removable => format!(":disco:{}", self.path.display()),
            _ => self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }
    /// Um arquivo da pasta Mesa (pode ir para o Lixo, ser renomeado…).
    pub fn is_file(&self) -> bool {
        !matches!(self.kind, DeskKind::Disk | DeskKind::Removable)
    }
    /// Recebe itens soltos em cima (pastas e discos).
    fn is_container(&self) -> bool {
        matches!(self.kind, DeskKind::Folder | DeskKind::Disk | DeskKind::Removable)
    }
}

/// Comandos da mesa (menus, teclado).
#[derive(Clone, Debug, PartialEq)]
pub enum DeskCmd {
    Open,
    Info,
    Rename,
    Duplicate,
    Alias,
    Compress,
    Trash,
    Copy,
    Paste,
    SelectAll,
    NewFolder,
    CleanUp,
    Arrange(String),
    ViewOptions,
    NewWindow,
    Go(PathBuf),
    Background,
    /// Ejetar os pendrives escolhidos.
    Eject,
    /// Rótulo de cor (0 tira).
    Label(u8),
    /// Abrir com o app (id do .desktop).
    OpenWith(String),
}

struct Press {
    at: Point<f64, Logical>,
    item: Option<PathBuf>,
    /// Clique com Shift/Ctrl (soma à escolha).
    extend: bool,
    /// O item já estava escolhido no clique (soltar sem arrastar deixa só ele).
    was_selected: bool,
}

/// Itens da mesa sendo arrastados (o desenho deles segue o mouse).
pub struct DeskDrag {
    pub paths: Vec<PathBuf>,
    pub origin: Point<f64, Logical>,
}

#[derive(Default)]
pub struct Desk {
    pub dir: PathBuf,
    dir_stamp: Option<SystemTime>,
    mounts: Vec<(String, PathBuf)>,
    pub items: Vec<DeskItem>,
    /// Célula de cada item (mesma ordem de `items`).
    pub rects: Vec<Rectangle<i32, Logical>>,
    saved: HashMap<String, (i32, i32)>,
    saved_loaded: bool,
    pub selected: HashSet<PathBuf>,
    last_click: Option<(PathBuf, Instant)>,
    press: Option<Press>,
    /// Retângulo de escolher: (começo, ponta).
    pub rubber: Option<(Point<f64, Logical>, Point<f64, Logical>)>,
    pub rubber_tex: Option<((i32, i32), Tex)>,
    pub drag: Option<DeskDrag>,
    /// Soltou e um app aceitou: o desenho some (o app termina o resto).
    pub drag_dropped: bool,
    /// Pasta ou disco da mesa sob o que está sendo arrastado.
    pub hot: Option<PathBuf>,
    /// Renomeando: (item, texto, tudo escolhido).
    pub rename: Option<(PathBuf, String, bool)>,
    pub texs: HashMap<PathBuf, (u64, Tex)>,
    /// Copiado da mesa (Ctrl+C): a área de transferência é do Glove.
    pub clip: Vec<PathBuf>,
    /// Arquivos que um app está arrastando (lidos no começo do arrastar).
    pub incoming: Option<Vec<PathBuf>>,
    /// Soltaram na mesa antes da lista chegar: (ponto, copiar).
    pub pending_drop: Option<(Point<f64, Logical>, bool)>,
    /// Onde o menu de contexto do fundo foi aberto (a pasta nova nasce ali).
    menu_at: Option<Point<f64, Logical>>,
    typed: (String, Option<Instant>),
    /// Pendrives montados na última conferida (para notar o que sumiu sem ejetar).
    removable_prev: Vec<glove_config::discos::Mount>,
    /// Extensões à mostra na última leitura (mudou nas Preferências: relê os nomes).
    show_ext: Option<bool>,
    layout_key: Option<(Vec<String>, (i32, i32, i32, i32), (u32, u32, bool), String)>,
}

/// "a b'c" → 'a b'\''c' (para a linha de comando).
pub fn quote(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
}

/// file:///caminho%20com%20espa%C3%A7o
pub fn file_uri(p: &Path) -> String {
    let mut out = String::from("file://");
    for b in p.to_string_lossy().as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Lista text/uri-list → caminhos (só file://).
pub fn parse_uri_list(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("file://"))
        .map(|rest| {
            // file://host/caminho: pula o host.
            let rest = if rest.starts_with('/') { rest } else { rest.find('/').map(|i| &rest[i..]).unwrap_or("") };
            let bytes = rest.as_bytes();
            let mut out = Vec::with_capacity(bytes.len());
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' && i + 2 < bytes.len() {
                    if let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                }
                out.push(bytes[i]);
                i += 1;
            }
            PathBuf::from(String::from_utf8_lossy(&out).into_owned())
        })
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

/// O que vai na área de transferência / no arrastar para cada tipo pedido.
pub fn payload(paths: &[PathBuf], mime: &str) -> Vec<u8> {
    let uris: Vec<String> = paths.iter().map(|p| file_uri(p)).collect();
    match mime {
        URI_LIST => uris.iter().map(|u| format!("{u}\r\n")).collect::<String>().into_bytes(),
        GNOME_FILES => format!("copy\n{}", uris.join("\n")).into_bytes(),
        _ => paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n").into_bytes(),
    }
}

pub fn offered_mimes() -> Vec<String> {
    vec![URI_LIST.into(), GNOME_FILES.into(), TEXT.into(), "UTF8_STRING".into()]
}

/// Nome e ícone de um atalho de app (.desktop).
fn desktop_entry(p: &Path) -> Option<(String, Icon)> {
    let text = std::fs::read_to_string(p).ok()?;
    let mut name = None;
    let mut name_pt = None;
    let mut icon = None;
    let mut in_entry = false;
    for l in text.lines() {
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = l.strip_prefix("Name=") {
            name.get_or_insert(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("Name[pt_BR]=").or_else(|| l.strip_prefix("Name[pt]=")) {
            name_pt.get_or_insert(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("Icon=") {
            icon.get_or_insert(v.trim().to_string());
        }
    }
    let name = name_pt.or(name)?;
    Some((name, icon.map(|i| Icon::from_name(&i)).unwrap_or(Icon::Window)))
}

fn read_items(dir: &Path, mounts: &[(String, PathBuf)], show_ext: bool) -> Vec<DeskItem> {
    let mut v: Vec<DeskItem> = mounts
        .iter()
        .map(|(name, path)| DeskItem {
            name: name.clone(),
            path: path.clone(),
            kind: if path == Path::new("/") { DeskKind::Disk } else { DeskKind::Removable },
            icon: if path.starts_with(glove_config::discos::gvfs_dir()) { Icon::Globe } else { Icon::Disk },
            link: false,
            modified: None,
            size: 0,
            label: 0,
        })
        .collect();
    let mut files: Vec<DeskItem> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let fname = e.file_name().to_string_lossy().into_owned();
            if fname.starts_with('.') {
                return None;
            }
            let path = e.path();
            let link = e.file_type().is_ok_and(|t| t.is_symlink());
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let (name, kind, icon) = if !is_dir && fname.ends_with(".desktop") {
                match desktop_entry(&path) {
                    Some((n, i)) => (n, DeskKind::App, i),
                    None => (fname.clone(), DeskKind::File, Icon::File),
                }
            } else {
                (glove_config::arquivos::shown_name(&fname, is_dir, show_ext), if is_dir { DeskKind::Folder } else { DeskKind::File }, icons::file_icon(&path, is_dir))
            };
            let label = glove_config::arquivos::label_of(&path);
            Some(DeskItem { name, path, kind, icon, link, modified: meta.as_ref().and_then(|m| m.modified().ok()), size: meta.map(|m| m.len()).unwrap_or(0), label })
        })
        .collect();
    files.sort_by_key(|a| a.name.to_lowercase());
    v.extend(files);
    v
}

fn positions_path() -> PathBuf {
    glove_config::dir().join("mesa-posicoes")
}

impl Desk {
    fn load_saved(&mut self) {
        if self.saved_loaded {
            return;
        }
        self.saved_loaded = true;
        let text = std::fs::read_to_string(positions_path()).unwrap_or_default();
        for l in text.lines() {
            let mut f = l.splitn(3, '\t');
            if let (Some(x), Some(y), Some(k)) = (f.next(), f.next(), f.next()) {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    self.saved.insert(k.to_string(), (x, y));
                }
            }
        }
    }

    fn store_saved(&self) {
        let mut text = String::new();
        let mut keys: Vec<&String> = self.saved.keys().collect();
        keys.sort();
        for k in keys {
            let (x, y) = self.saved[k];
            text.push_str(&format!("{x}\t{y}\t{k}\n"));
        }
        let p = positions_path();
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    }

    pub fn index_of(&self, p: &Path) -> Option<usize> {
        self.items.iter().position(|i| i.path == p)
    }

    /// Itens escolhidos, na ordem da mesa.
    pub fn chosen(&self) -> Vec<DeskItem> {
        self.items.iter().filter(|i| self.selected.contains(&i.path)).cloned().collect()
    }
}

/// Células da grade, coluna por coluna a partir do canto superior direito.
fn grid_cells(area: Rectangle<i32, Logical>, cell: (i32, i32)) -> Vec<Point<i32, Logical>> {
    let (cw, ch) = cell;
    let right = area.loc.x + area.size.w - MARGIN;
    let top = area.loc.y + MARGIN;
    let rows = ((area.size.h - MARGIN) / (ch + GAP)).max(1);
    let cols = ((area.size.w - MARGIN) / (cw + GAP)).max(1);
    let mut v = Vec::with_capacity((rows * cols) as usize);
    for c in 0..cols {
        for r in 0..rows {
            v.push((right - cw - c * (cw + GAP), top + r * (ch + GAP)).into());
        }
    }
    v
}

impl Glove {
    pub fn desk_style(&self) -> DeskStyle {
        let d = &self.config.desk;
        DeskStyle { icon: d.icon_size.clamp(32, 128) as f32, text: d.text_size.clamp(10, 16) as f32, right: d.label_right }
    }

    fn desk_cell(&self) -> (i32, i32) {
        let (w, h) = self.desk_style().cell();
        (w.ceil() as i32, h.ceil() as i32)
    }

    /// A mesa responde ao teclado e à barra de menu: nenhuma janela em foco.
    pub fn desk_active(&self) -> bool {
        !self.greeter && self.lock.is_none() && self.focused_window().is_none()
    }

    /// Relê os discos (a cada 3 s) e a pasta Mesa (se mudou); refaz a grade.
    pub fn refresh_desk(&mut self, force: bool) {
        if self.greeter {
            return;
        }
        if self.desk.dir.as_os_str().is_empty() {
            self.desk.dir = glove_config::desktop_dir();
        }
        self.desk.load_saved();
        let d = &self.config.desk;
        let mut mounts = Vec::new();
        if d.show_disks {
            mounts.push(("Disco".to_string(), PathBuf::from("/")));
        }
        let removable_now = glove_config::discos::removable_mounts();
        // Sumiu um pendrive e o aparelho não existe mais, sem o Glove ter ejetado: arrancado.
        for m in &self.desk.removable_prev {
            if !removable_now.iter().any(|n| n.path == m.path) && !Path::new(&m.dev).exists() && !glove_config::discos::take_ejected(&m.dev) {
                self.spawn(&format!("glove-files --aviso-ejetar {}", quote(Path::new(&m.name))));
            }
        }
        self.desk.removable_prev = removable_now.clone();
        let removable: Vec<(String, PathBuf)> = removable_now.into_iter().map(|m| (m.name, m.path)).collect();
        // Servidores conectados (gvfs) também vão para a mesa, como no Tiger.
        let servers: Vec<(String, PathBuf)> = if self.config.files.servers_on_desk { glove_config::discos::server_mounts().into_iter().map(|m| (m.name, m.path)).collect() } else { Vec::new() };
        // Mídia Removível: um disco novo (não os que já estavam na partida) abre no Arquivos.
        if !self.desk.mounts.is_empty() || self.desk.dir_stamp.is_some() {
            let known: Vec<&PathBuf> = self.desk.mounts.iter().map(|m| &m.1).collect();
            {
                // Só pendrives (o Conectar ao Servidor já abre a janela do servidor).
                for (_, path) in removable.iter().filter(|m| !known.contains(&&m.1)) {
                    // CD ou DVD (aparelho /dev/sr*): as escolhas próprias dos discos.
                    let optical = self.desk.removable_prev.iter().any(|m| &m.path == path && m.dev.starts_with("/dev/sr"));
                    let video = optical && (path.join("VIDEO_TS").is_dir() || path.join("video_ts").is_dir());
                    let action = if video { self.config.media.video_dvd.clone() } else if optical { self.config.media.data_disc.clone() } else { self.config.removable_action.clone() };
                    match action.as_str() {
                        "video" => self.spawn(&format!("glove-video {}", quote(path))),
                        "abrir" => self.spawn(&format!("glove-files {}", quote(path))),
                        _ => {}
                    }
                }
            }
        }
        let shown_removable = if d.show_removable { removable.clone() } else { Vec::new() };
        mounts.extend(shown_removable);
        mounts.extend(servers.iter().cloned());
        let stamp = std::fs::metadata(&self.desk.dir).and_then(|m| m.modified()).ok();
        // `mounts` do Desk guarda todos os removíveis (para saber quando chega um novo).
        let mut all_mounts = mounts.clone();
        for m in removable.iter().chain(&servers) {
            if !all_mounts.contains(m) {
                all_mounts.push(m.clone());
            }
        }
        let show_ext = Some(self.config.files.show_extensions);
        let changed = force || stamp != self.desk.dir_stamp || all_mounts != self.desk.mounts || self.desk.items.is_empty() || show_ext != self.desk.show_ext;
        self.desk.show_ext = show_ext;
        self.desk.mounts = all_mounts;
        self.desk.dir_stamp = stamp;
        if changed {
            let items = read_items(&self.desk.dir, &mounts, self.config.files.show_extensions);
            if items != self.desk.items {
                self.desk.items = items;
                let alive: HashSet<PathBuf> = self.desk.items.iter().map(|i| i.path.clone()).collect();
                self.desk.selected.retain(|p| alive.contains(p));
                self.desk.texs.retain(|p, _| alive.contains(p));
                if self.desk.rename.as_ref().is_some_and(|r| !alive.contains(&r.0)) {
                    self.desk.rename = None;
                }
                self.desk.layout_key = None;
            }
        } else {
            // Rótulo mudado por outro app (não muda a data da pasta): confere os rótulos.
            for it in self.desk.items.iter_mut().filter(|i| i.is_file()) {
                let l = glove_config::arquivos::label_of(&it.path);
                if l != it.label {
                    it.label = l;
                    self.dirty = true;
                }
            }
        }
        self.layout_desk();
    }

    /// Posição de cada item: a guardada, senão a primeira célula livre da grade.
    pub fn layout_desk(&mut self) {
        let area = self.work_area();
        let cell = self.desk_cell();
        let d = &self.config.desk;
        let key = (
            self.desk.items.iter().map(|i| i.key()).collect::<Vec<_>>(),
            (area.loc.x, area.loc.y, area.size.w, area.size.h),
            (d.icon_size, d.text_size, d.label_right),
            d.arrange.clone(),
        );
        if self.desk.layout_key.as_ref() == Some(&key) {
            return;
        }
        let arrange = d.arrange.clone();
        let cells = grid_cells(area, cell);
        let size = (cell.0, cell.1);
        let mut rects: Vec<Option<Rectangle<i32, Logical>>> = vec![None; self.desk.items.len()];
        if !arrange.is_empty() {
            // Mantido organizado: a ordem pedida, na grade.
            let order = self.desk_order(&arrange);
            for (k, i) in order.into_iter().enumerate() {
                let loc = cells.get(k).copied().unwrap_or(cells[cells.len() - 1]);
                rects[i] = Some(Rectangle::new(loc, size.into()));
            }
        } else {
            let clamp = |x: i32, y: i32| -> Point<i32, Logical> {
                let x = x.clamp(area.loc.x, (area.loc.x + area.size.w - cell.0).max(area.loc.x));
                let y = y.clamp(area.loc.y, (area.loc.y + area.size.h - cell.1).max(area.loc.y));
                (x, y).into()
            };
            for (i, it) in self.desk.items.iter().enumerate() {
                if let Some(&(x, y)) = self.desk.saved.get(&it.key()) {
                    rects[i] = Some(Rectangle::new(clamp(x, y), size.into()));
                }
            }
            for i in 0..self.desk.items.len() {
                if rects[i].is_some() {
                    continue;
                }
                let taken: Vec<Rectangle<i32, Logical>> = rects.iter().flatten().copied().collect();
                let loc = cells.iter().copied().find(|c| !taken.iter().any(|r| r.overlaps(Rectangle::new(*c, size.into())))).unwrap_or(cells[cells.len() - 1]);
                rects[i] = Some(Rectangle::new(loc, size.into()));
            }
        }
        self.desk.rects = rects.into_iter().map(|r| r.unwrap()).collect();
        self.desk.layout_key = Some(key);
        self.dirty = true;
    }

    /// Índices dos itens na ordem de "Organizar por" (os discos sempre primeiro).
    fn desk_order(&self, by: &str) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.desk.items.len()).collect();
        let items = &self.desk.items;
        idx.sort_by(|&a, &b| {
            let (x, y) = (&items[a], &items[b]);
            let disk = |i: &DeskItem| !i.is_file();
            disk(y).cmp(&disk(x)).then_with(|| match by {
                "data" => y.modified.cmp(&x.modified),
                "tamanho" => y.size.cmp(&x.size),
                "tipo" => {
                    let ext = |i: &DeskItem| if i.kind == DeskKind::Folder { String::new() } else { i.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "~".into()) };
                    ext(x).cmp(&ext(y)).then_with(|| x.name.to_lowercase().cmp(&y.name.to_lowercase()))
                }
                _ => x.name.to_lowercase().cmp(&y.name.to_lowercase()),
            })
        });
        idx
    }

    /// Guarda a posição atual de todos os itens (depois de arrastar ou organizar).
    fn desk_save_all(&mut self) {
        for (it, r) in self.desk.items.iter().zip(&self.desk.rects) {
            self.desk.saved.insert(it.key(), (r.loc.x, r.loc.y));
        }
        // Itens que não existem mais saem do arquivo.
        let alive: HashSet<String> = self.desk.items.iter().map(|i| i.key()).collect();
        self.desk.saved.retain(|k, _| alive.contains(k));
        self.desk.store_saved();
        self.desk.layout_key = None;
    }

    /// Limpar: cada ícone vai para a célula livre mais perto.
    fn desk_clean_up(&mut self) {
        let area = self.work_area();
        let cell = self.desk_cell();
        let cells = grid_cells(area, cell);
        let mut order: Vec<usize> = (0..self.desk.items.len()).collect();
        order.sort_by_key(|&i| (self.desk.rects[i].loc.x / -(cell.0 + GAP), self.desk.rects[i].loc.y));
        let mut used = vec![false; cells.len()];
        for i in order {
            let r = self.desk.rects[i];
            let best = cells.iter().enumerate().filter(|(k, _)| !used[*k]).min_by_key(|(_, c)| {
                let (dx, dy) = ((c.x - r.loc.x) as i64, (c.y - r.loc.y) as i64);
                dx * dx + dy * dy
            });
            if let Some((k, c)) = best {
                used[k] = true;
                self.desk.rects[i] = Rectangle::new(*c, r.size);
            }
        }
        self.desk_save_all();
        self.dirty = true;
    }

    /// Organizar por (uma vez): a ordem pedida, na grade.
    fn desk_arrange_once(&mut self, by: &str) {
        let area = self.work_area();
        let cell = self.desk_cell();
        let cells = grid_cells(area, cell);
        for (k, i) in self.desk_order(by).into_iter().enumerate() {
            let loc = cells.get(k).copied().unwrap_or(cells[cells.len() - 1]);
            self.desk.rects[i] = Rectangle::new(loc, cell.into());
        }
        self.desk_save_all();
        self.dirty = true;
    }

    /// Item da mesa sob o ponto (só o desenho e o nome contam, não a célula toda).
    pub fn desk_item_at(&self, pos: Point<f64, Logical>) -> Option<usize> {
        let style = self.desk_style();
        let (ix, iy) = style.icon_at();
        (0..self.desk.items.len()).rev().find(|&i| {
            let r = self.desk.rects[i].to_f64();
            if !r.contains(pos) {
                return false;
            }
            let (lx, ly) = (pos.x - r.loc.x, pos.y - r.loc.y);
            let s = style.icon as f64;
            let on_icon = lx >= ix as f64 - 4.0 && lx <= (ix + style.icon) as f64 + 4.0 && ly >= iy as f64 - 3.0 && ly <= iy as f64 + s + 3.0;
            let on_label = if style.right { lx > (ix + style.icon) as f64 } else { ly > iy as f64 + s };
            on_icon || on_label
        })
    }

    // -------------------------------------------------------------------
    // Desenho
    // -------------------------------------------------------------------

    pub fn prepare_desk(&mut self) {
        if self.greeter {
            return;
        }
        self.layout_desk();
        let style = self.desk_style();
        let sc = self.scale() as f32;
        let tgen = self.theme_gen;
        for it in self.desk.items.clone() {
            let selected = self.desk.selected.contains(&it.path);
            let hot = self.desk.hot.as_ref() == Some(&it.path);
            let rename = self.desk.rename.as_ref().filter(|r| r.0 == it.path).map(|r| (r.1.clone(), r.2));
            let key = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                (&it.name, format!("{:?}", it.icon), it.link, it.label, selected, hot, &rename, tgen, sc.to_bits()).hash(&mut h);
                (style.icon.to_bits(), style.text.to_bits(), style.right).hash(&mut h);
                h.finish()
            };
            if self.desk.texs.get(&it.path).is_some_and(|(k, _)| *k == key) {
                continue;
            }
            let r = rename.as_ref().map(|(t, all)| (t.as_str(), *all));
            let tag = glove_config::arquivos::LABEL_COLORS.get((it.label as usize).wrapping_sub(1)).map(|&(r, g, b)| glove_paint::draw::rgba(r, g, b, 255));
            if let Some(c) = chrome::desk_icon(&self.fonts, &self.theme, &style, it.icon, it.link, &it.name, tag, selected, hot, r, sc) {
                self.desk.texs.insert(it.path.clone(), (key, to_tex(c)));
            }
        }
        // Retângulo de escolher.
        match self.desk.rubber {
            Some((a, b)) => {
                let (w, h) = (((a.x - b.x).abs()).round() as i32, ((a.y - b.y).abs()).round() as i32);
                if self.desk.rubber_tex.as_ref().map(|t| t.0) != Some((w, h)) {
                    self.desk.rubber_tex = chrome::rubber_band(w as f32, h as f32, sc).map(|c| ((w, h), to_tex(c)));
                }
            }
            None => self.desk.rubber_tex = None,
        }
    }

    /// Onde cada ícone arrastado está agora (acompanha o mouse).
    pub fn desk_drag_offset(&self) -> Option<Point<i32, Logical>> {
        let d = self.desk.drag.as_ref().filter(|_| !self.desk.drag_dropped)?;
        let p = self.pointer_pos;
        Some(((p.x - d.origin.x).round() as i32, (p.y - d.origin.y).round() as i32).into())
    }

    // -------------------------------------------------------------------
    // Mouse
    // -------------------------------------------------------------------

    /// Clique na mesa (num ícone ou no fundo).
    pub fn desk_press(&mut self, item: Option<usize>, button: u32, pos: Point<f64, Logical>) {
        const BTN_RIGHT: u32 = 0x111;
        self.desk_commit_rename();
        let (shift, ctrl) = self.seat.get_keyboard().map(|k| {
            let m = k.modifier_state();
            (m.shift, m.ctrl || m.logo)
        }).unwrap_or((false, false));
        let path = item.and_then(|i| self.desk.items.get(i)).map(|i| i.path.clone());
        if button == BTN_RIGHT {
            match &path {
                Some(p) if !self.desk.selected.contains(p) => {
                    self.desk.selected.clear();
                    self.desk.selected.insert(p.clone());
                }
                Some(_) => {}
                None => self.desk.selected.clear(),
            }
            self.dirty = true;
            self.open_desk_menu(path.is_some(), pos);
            return;
        }
        match &path {
            Some(p) => {
                let was = self.desk.selected.contains(p);
                let now = Instant::now();
                let double = !shift && !ctrl && self.desk.last_click.as_ref().is_some_and(|(q, t)| q == p && now.duration_since(*t).as_millis() < self.config.pointer.double_click_ms as u128);
                if shift || ctrl {
                    if was {
                        self.desk.selected.remove(p);
                    } else {
                        self.desk.selected.insert(p.clone());
                    }
                } else if !was {
                    self.desk.selected.clear();
                    self.desk.selected.insert(p.clone());
                }
                self.desk.last_click = if double { None } else { Some((p.clone(), now)) };
                if double {
                    self.desk_cmd(DeskCmd::Open);
                    return;
                }
                self.desk.press = Some(Press { at: pos, item: Some(p.clone()), extend: shift || ctrl, was_selected: was });
            }
            None => {
                if !shift && !ctrl {
                    self.desk.selected.clear();
                }
                self.desk.press = Some(Press { at: pos, item: None, extend: shift || ctrl, was_selected: false });
            }
        }
        self.dirty = true;
    }

    /// O mouse andou com o botão apertado na mesa: arrastar ícones ou escolher com o retângulo.
    pub fn desk_motion(&mut self, pos: Point<f64, Logical>) {
        // Algo sendo arrastado (da mesa ou de um app): acende a pasta de destino.
        if self.desk.drag.is_some() || self.dnd_icon.is_some() || self.desk.incoming.is_some() {
            let dragged: Vec<PathBuf> = self.desk.drag.as_ref().map(|d| d.paths.clone()).unwrap_or_default();
            let hot = match self.hit_test(pos) {
                Hit::DesktopIcon(i) => self.desk.items.get(i).filter(|it| it.is_container() && !dragged.contains(&it.path)).map(|it| it.path.clone()),
                _ => None,
            };
            if hot != self.desk.hot {
                self.desk.hot = hot.clone();
                self.dirty = true;
                // Pasta com mola: segurar em cima por um instante abre a pasta numa janela.
                if let Some(p) = hot.filter(|p| p.is_dir()) {
                    let _ = self.loop_handle.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(900)), move |_, _, data| {
                        let st = &mut data.state;
                        let dragging = st.desk.drag.is_some() || st.dnd_icon.is_some() || st.desk.incoming.is_some();
                        if dragging && st.desk.hot.as_ref() == Some(&p) {
                            st.desk.hot = None;
                            st.spawn(&format!("glove-files {}", quote(&p)));
                            st.dirty = true;
                        }
                        smithay::reexports::calloop::timer::TimeoutAction::Drop
                    });
                }
            }
            if self.desk.drag.is_some() {
                self.dirty = true;
            }
        }
        let Some(press) = &self.desk.press else { return };
        let (dx, dy) = (pos.x - press.at.x, pos.y - press.at.y);
        match press.item.clone() {
            Some(_) if dx * dx + dy * dy > DRAG_START * DRAG_START => {
                let origin = press.at;
                self.desk.press = None;
                let paths: Vec<PathBuf> = self.desk.chosen().into_iter().map(|i| i.path).collect();
                if !paths.is_empty() {
                    self.desk_start_drag(paths, origin);
                }
            }
            Some(_) => {}
            None => {
                let start = press.at;
                let extend = press.extend;
                self.desk.rubber = Some((start, pos));
                let r: Rectangle<f64, Logical> = Rectangle::new((start.x.min(pos.x), start.y.min(pos.y)).into(), ((start.x - pos.x).abs(), (start.y - pos.y).abs()).into());
                let style = self.desk_style();
                let (ix, iy) = style.icon_at();
                let mut sel: HashSet<PathBuf> = if extend { self.desk.selected.clone() } else { HashSet::new() };
                for (it, cell) in self.desk.items.iter().zip(&self.desk.rects) {
                    // Conta o desenho do ícone (não a célula inteira).
                    let icon = Rectangle::new((cell.loc.x as f64 + ix as f64, cell.loc.y as f64 + iy as f64).into(), (style.icon as f64, style.icon as f64).into());
                    if r.overlaps(icon) {
                        sel.insert(it.path.clone());
                    }
                }
                if sel != self.desk.selected {
                    self.desk.selected = sel;
                }
                self.dirty = true;
            }
        }
    }

    /// Soltou o botão (sem ter virado arrastar).
    pub fn desk_release(&mut self) -> bool {
        let Some(press) = self.desk.press.take() else {
            return if self.desk.rubber.take().is_some() {
                self.dirty = true;
                true
            } else {
                false
            };
        };
        if self.desk.rubber.take().is_some() {
            self.dirty = true;
        }
        // Clique simples num dos vários escolhidos: fica só ele.
        if let (Some(p), false, true) = (&press.item, press.extend, press.was_selected) {
            if self.desk.selected.len() > 1 {
                self.desk.selected.clear();
                self.desk.selected.insert(p.clone());
                self.dirty = true;
            }
        }
        true
    }

    /// Começa o arrastar do sistema com os itens: soltos numa janela, o app recebe
    /// a lista de arquivos; na mesa ou no Dock, o Glove cuida (ver `desk_drop`).
    pub(crate) fn desk_start_drag(&mut self, paths: Vec<PathBuf>, origin: Point<f64, Logical>) {
        use smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction;
        use smithay::wayland::selection::data_device::{start_dnd, SourceMetadata};
        self.desk.drag = Some(DeskDrag { paths, origin });
        self.desk.drag_dropped = false;
        // O soltar vai para o arrastar (não pode ser engolido).
        self.chrome.swallow_release = false;
        let dh = self.display_handle.clone();
        let seat = self.seat.clone();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let start = smithay::input::pointer::GrabStartData { focus: None, button: 0x110, location: origin };
        start_dnd(&dh, &seat, self, serial, Some(start), None, SourceMetadata { mime_types: offered_mimes(), dnd_action: DndAction::Copy | DndAction::Move });
        self.dirty = true;
    }

    /// Um app pediu os dados do arrastar da mesa.
    pub fn desk_send(&mut self, mime: &str, fd: std::os::fd::OwnedFd) {
        let Some(d) = &self.desk.drag else { return };
        let data = payload(&d.paths, mime);
        std::thread::spawn(move || {
            use std::io::Write;
            let mut f = std::fs::File::from(fd);
            let _ = f.write_all(&data);
        });
    }

    /// Soltou fora de qualquer app: na mesa, numa pasta da mesa ou no Dock.
    pub fn desk_drop(&mut self, pos: Point<f64, Logical>) {
        self.desk.hot = None;
        let Some(d) = self.desk.drag.take() else { return };
        self.dirty = true;
        let copy = self.seat.get_keyboard().is_some_and(|k| k.modifier_state().alt);
        match self.hit_test(pos) {
            Hit::DesktopIcon(i) if self.desk.items.get(i).is_some_and(|it| it.is_container() && !d.paths.contains(&it.path)) => {
                let dest = self.desk.items[i].path.clone();
                self.desk_transfer(&d.paths, &dest, !copy);
            }
            Hit::Desktop | Hit::DesktopIcon(_) => {
                if copy {
                    // Alt: copia para a própria mesa (vira "nome 2").
                    let dir = self.desk.dir.clone();
                    self.desk_transfer(&d.paths, &dir, false);
                    return;
                }
                if !self.config.desk.arrange.is_empty() {
                    return;
                }
                let (dx, dy) = ((pos.x - d.origin.x).round() as i32, (pos.y - d.origin.y).round() as i32);
                let area = self.work_area();
                for p in &d.paths {
                    if let Some(i) = self.desk.index_of(p) {
                        let r = self.desk.rects[i];
                        let x = (r.loc.x + dx).clamp(area.loc.x, area.loc.x + area.size.w - r.size.w);
                        let y = (r.loc.y + dy).clamp(area.loc.y, area.loc.y + area.size.h - r.size.h);
                        self.desk.rects[i] = Rectangle::new((x, y).into(), r.size);
                    }
                }
                self.desk_save_all();
            }
            Hit::Dock(Some(i)) => self.drop_on_dock(i, d.paths),
            _ => {}
        }
    }

    /// Arquivos soltos num ícone do Dock: no Lixo vão para a Lixeira; num app, abrem nele.
    pub fn drop_on_dock(&mut self, i: usize, paths: Vec<PathBuf>) {
        let files: Vec<PathBuf> = paths.into_iter().filter(|p| p.parent().is_some() && p != Path::new("/")).collect();
        if files.is_empty() {
            return;
        }
        let args = files.iter().map(|p| quote(p)).collect::<Vec<_>>().join(" ");
        match self.chrome.dock_actions.get(i).cloned() {
            Some(DockAction::Trash) => {
                // Pendrives arrastados para o Lixo são ejetados (como no Tiger); o disco do sistema fica.
                let mounts: Vec<PathBuf> = self.desk.mounts.iter().map(|m| m.1.clone()).collect();
                for p in files.iter().filter(|p| mounts.contains(p)) {
                    self.spawn(&format!("glove-files --ejetar {}", quote(p)));
                }
                let files: Vec<String> = files.iter().filter(|p| !mounts.contains(p)).map(|p| quote(p)).collect();
                if !files.is_empty() {
                    self.spawn(&format!("glove-files --lixo {}", files.join(" ")));
                }
                self.desk.selected.clear();
            }
            Some(DockAction::App(idx)) => {
                if let Some(app) = self.config.dock_apps.get(idx).cloned() {
                    self.spawn(&format!("{} {args}", app.command));
                }
            }
            _ => {}
        }
    }

    /// Arrastando só pendrives (o Lixo do Dock vira Ejetar).
    pub fn desk_dragging_disks(&self) -> bool {
        self.desk.drag.as_ref().is_some_and(|d| {
            !d.paths.is_empty() && d.paths.iter().all(|p| self.desk.items.iter().any(|i| &i.path == p && i.kind == DeskKind::Removable))
        })
    }

    /// Copiar ou mover para `dest` pelo Arquivos (janela de progresso se demorar).
    fn desk_transfer(&mut self, paths: &[PathBuf], dest: &Path, mv: bool) {
        if paths.is_empty() {
            return;
        }
        let args = paths.iter().map(|p| quote(p)).collect::<Vec<_>>().join(" ");
        self.spawn(&format!("glove-files --transferir {} {} {args}", if mv { "mover" } else { "copiar" }, quote(dest)));
    }

    /// Arquivos de um app soltos na mesa (a lista chegou ou já estava aqui).
    pub fn desk_incoming_drop(&mut self) {
        let Some((pos, copy)) = self.desk.pending_drop else { return };
        let Some(paths) = self.desk.incoming.clone() else { return };
        self.desk.pending_drop = None;
        self.desk.incoming = None;
        self.desk.hot = None;
        self.dirty = true;
        match self.hit_test(pos) {
            Hit::DesktopIcon(i) if self.desk.items.get(i).is_some_and(|it| it.is_container()) => {
                let dest = self.desk.items[i].path.clone();
                self.desk_transfer(&paths, &dest, !copy);
            }
            Hit::Desktop | Hit::DesktopIcon(_) => {
                // Os itens novos nascem onde foram soltos (um ao lado do outro).
                let cell = self.desk_cell();
                for (k, p) in paths.iter().enumerate() {
                    if let Some(name) = p.file_name() {
                        let x = (pos.x as i32 - cell.0 / 2) - k as i32 * (cell.0 + GAP);
                        let y = pos.y as i32 - self.desk_style().icon as i32 / 2 - 4;
                        self.desk.saved.insert(name.to_string_lossy().into_owned(), (x, y));
                    }
                }
                self.desk.store_saved();
                self.desk.layout_key = None;
                let dir = self.desk.dir.clone();
                self.desk_transfer(&paths, &dir, !copy);
            }
            Hit::Dock(Some(i)) => self.drop_on_dock(i, paths),
            _ => {}
        }
    }

    /// Um app começou a arrastar: pede a lista de arquivos (se for de arquivos)
    /// para o caso de soltarem na mesa.
    pub fn desk_watch_drag(&mut self, source: &smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource) {
        use smithay::wayland::selection::data_device::with_source_metadata;
        self.desk.incoming = None;
        self.desk.pending_drop = None;
        let files = with_source_metadata(source, |m| m.mime_types.iter().any(|t| t == URI_LIST)).unwrap_or(false);
        if !files {
            return;
        }
        let Some((read, write)) = pipe() else { return };
        source.send(URI_LIST.to_string(), std::os::fd::AsFd::as_fd(&write));
        drop(write);
        self.read_pipe(read, |st, data| {
            st.desk.incoming = Some(parse_uri_list(&String::from_utf8_lossy(&data)));
            st.desk_incoming_drop();
        });
    }

    /// Lê um pipe sem travar o compositor e entrega tudo no fim.
    pub fn read_pipe(&mut self, read: std::os::fd::OwnedFd, done: impl FnOnce(&mut Glove, Vec<u8>) + 'static) {
        use smithay::reexports::calloop::{generic::Generic, Interest, Mode, PostAction};
        let file = std::fs::File::from(read);
        let mut buf = Vec::new();
        let mut done = Some(done);
        let _ = self.loop_handle.insert_source(Generic::new(file, Interest::READ, Mode::Level), move |_, f, data| {
            use std::io::Read;
            let f: &mut std::fs::File = unsafe { f.get_mut() };
            let mut chunk = [0u8; 8192];
            loop {
                match f.read(&mut chunk) {
                    Ok(0) => {
                        if let Some(d) = done.take() {
                            d(&mut data.state, std::mem::take(&mut buf));
                        }
                        return Ok(PostAction::Remove);
                    }
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        if buf.len() > 4 << 20 {
                            return Ok(PostAction::Remove);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(PostAction::Continue),
                    Err(_) => return Ok(PostAction::Remove),
                }
            }
        });
    }

    // -------------------------------------------------------------------
    // Menus
    // -------------------------------------------------------------------

    fn open_desk_menu(&mut self, on_item: bool, pos: Point<f64, Logical>) {
        let nodes = if on_item { self.desk_item_nodes() } else { self.desk_background_nodes() };
        self.desk.menu_at = (!on_item).then_some(pos);
        self.show_context_menu(nodes, pos);
    }

    fn desk_item_nodes(&self) -> Vec<MenuNode> {
        let chosen = self.desk.chosen();
        let n = chosen.len();
        let one = n == 1;
        let files = chosen.iter().all(DeskItem::is_file);
        let many = |t: &str| if one { t.to_string() } else { format!("{t} {n} itens") };
        let act = |c: DeskCmd| Some(MenuAction::Desk(c));
        let on = |it: MenuItem, b: bool| if b { it } else { it.disabled() };
        let compress = match chosen.as_slice() {
            [one] => format!("Compactar “{}”", one.name),
            _ => format!("Compactar {n} itens"),
        };
        let mut v = vec![MenuNode::new(MenuItem::item(many("Abrir")), act(DeskCmd::Open))];
        // Pendrives: Ejetar logo abaixo de Abrir.
        if !chosen.is_empty() && chosen.iter().all(|i| i.kind == DeskKind::Removable) {
            let label = match chosen.as_slice() {
                [one] => format!("Ejetar “{}”", one.name),
                _ => format!("Ejetar {n} discos"),
            };
            v.push(MenuNode::new(MenuItem::item(label), act(DeskCmd::Eject)));
        }
        // Abrir com: só para um arquivo (não pastas nem discos).
        if let [it] = chosen.as_slice() {
            if matches!(it.kind, DeskKind::File) {
                v.push(MenuNode::submenu("Abrir com", self.open_with_nodes(&it.path)));
            }
        }
        v.extend([
            MenuNode::new(on(MenuItem::item("Mover para a Lixeira"), files), act(DeskCmd::Trash)),
            MenuNode::sep(),
            MenuNode::new(on(MenuItem::item("Obter Informações"), one), act(DeskCmd::Info)),
            MenuNode::new(on(MenuItem::item("Renomear"), one && files), act(DeskCmd::Rename)),
            MenuNode::new(on(MenuItem::item(compress), files), act(DeskCmd::Compress)),
            MenuNode::new(on(MenuItem::item("Duplicar"), files), act(DeskCmd::Duplicate)),
            MenuNode::new(on(MenuItem::item("Criar Atalho"), one), act(DeskCmd::Alias)),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item(many("Copiar")), act(DeskCmd::Copy)),
            MenuNode::sep(),
            MenuNode::submenu("Rótulo", self.label_nodes()),
        ]);
        v
    }

    /// "Abrir com ▸": os apps que abrem o tipo do item (o padrão primeiro).
    fn open_with_nodes(&self, p: &Path) -> Vec<MenuNode> {
        use glove_config::arquivos as a;
        let mime = a::mime_of(p);
        let apps = a::apps_for_mime(&mime);
        let def = a::default_app(&mime);
        let mut v = Vec::new();
        if let Some((id, name)) = def.as_ref().and_then(|d| apps.iter().find(|x| &x.0 == d)) {
            v.push(MenuNode::new(MenuItem::item(format!("{name} (padrão)")), Some(MenuAction::Desk(DeskCmd::OpenWith(id.clone())))));
            v.push(MenuNode::sep());
        }
        for (id, name) in apps.iter().filter(|x| Some(&x.0) != def.as_ref()) {
            v.push(MenuNode::new(MenuItem::item(name.clone()), Some(MenuAction::Desk(DeskCmd::OpenWith(id.clone())))));
        }
        if v.is_empty() {
            v.push(MenuNode::new(MenuItem::item("Nenhum app encontrado").disabled(), None));
        }
        v
    }

    /// "Rótulo ▸": nenhum e as sete cores (com o nome das Preferências do Arquivos).
    fn label_nodes(&self) -> Vec<MenuNode> {
        let chosen = self.desk.chosen();
        let cur = chosen.first().map(|i| i.label).filter(|l| chosen.iter().all(|i| i.label == *l));
        let item = |n: u8, t: String| MenuNode::new(MenuItem::check(t, cur == Some(n)), Some(MenuAction::Desk(DeskCmd::Label(n))));
        let mut v = vec![item(0, "Nenhum".into()), MenuNode::sep()];
        for (n, &(r, g, b)) in (1..=7u8).zip(glove_config::arquivos::LABEL_COLORS.iter()) {
            let mut node = item(n, self.config.files.label_name(n));
            node.item = node.item.with_swatch(glove_paint::draw::rgba(r, g, b, 255));
            v.push(node);
        }
        v
    }

    fn arrange_nodes(&self) -> Vec<MenuNode> {
        [("nome", "Nome"), ("data", "Data de Modificação"), ("tamanho", "Tamanho"), ("tipo", "Tipo")]
            .into_iter()
            .map(|(k, l)| MenuNode::new(MenuItem::item(l), Some(MenuAction::Desk(DeskCmd::Arrange(k.into())))))
            .collect()
    }

    fn desk_background_nodes(&self) -> Vec<MenuNode> {
        let act = |c: DeskCmd| Some(MenuAction::Desk(c));
        let free = self.config.desk.arrange.is_empty();
        let on = |it: MenuItem, b: bool| if b { it } else { it.disabled() };
        vec![
            MenuNode::new(MenuItem::item("Nova Pasta"), act(DeskCmd::NewFolder)),
            MenuNode::new(MenuItem::item("Obter Informações"), act(DeskCmd::Info)),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Colar"), act(DeskCmd::Paste)),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Alterar Fundo da Mesa…"), act(DeskCmd::Background)),
            MenuNode::new(on(MenuItem::item("Limpar"), free), act(DeskCmd::CleanUp)),
            MenuNode::submenu("Organizar por", self.arrange_nodes()),
            MenuNode::new(MenuItem::item("Mostrar Opções de Visualização"), act(DeskCmd::ViewOptions)),
        ]
    }

    /// Menus da barra com a mesa em foco (os do Arquivos, como no Finder).
    pub fn desk_bar_titles() -> Vec<String> {
        ["Arquivo", "Editar", "Visualizar", "Ir"].map(String::from).to_vec()
    }

    pub fn desk_bar_nodes(&self, title: &str) -> Vec<MenuNode> {
        let act = |c: DeskCmd| Some(MenuAction::Desk(c));
        let chosen = self.desk.chosen();
        let any = !chosen.is_empty();
        let files = any && chosen.iter().all(DeskItem::is_file);
        let one = chosen.len() == 1;
        let on = |it: MenuItem, b: bool| if b { it } else { it.disabled() };
        let free = self.config.desk.arrange.is_empty();
        match title {
            "Arquivo" => vec![
                MenuNode::new(MenuItem::item("Nova Janela do Arquivos").with_shortcut("Ctrl+N"), act(DeskCmd::NewWindow)),
                MenuNode::new(MenuItem::item("Nova Pasta").with_shortcut("Ctrl+Shift+N"), act(DeskCmd::NewFolder)),
                MenuNode::new(on(MenuItem::item("Abrir").with_shortcut("Ctrl+O"), any), act(DeskCmd::Open)),
                MenuNode::sep(),
                MenuNode::new(MenuItem::item("Obter Informações").with_shortcut("Ctrl+I"), act(DeskCmd::Info)),
                MenuNode::new(on(MenuItem::item("Duplicar").with_shortcut("Ctrl+D"), files), act(DeskCmd::Duplicate)),
                MenuNode::new(on(MenuItem::item("Criar Atalho").with_shortcut("Ctrl+L"), one), act(DeskCmd::Alias)),
                MenuNode::new(on(MenuItem::item("Compactar"), files), act(DeskCmd::Compress)),
                MenuNode::new(on(MenuItem::item("Ejetar").with_shortcut("Ctrl+E"), any && chosen.iter().all(|i| i.kind == DeskKind::Removable)), act(DeskCmd::Eject)),
                MenuNode::sep(),
                MenuNode::new(on(MenuItem::item("Mover para a Lixeira").with_shortcut("Ctrl+BackSpace"), files), act(DeskCmd::Trash)),
            ],
            "Editar" => vec![
                MenuNode::new(on(MenuItem::item("Copiar").with_shortcut("Ctrl+C"), any), act(DeskCmd::Copy)),
                MenuNode::new(MenuItem::item("Colar").with_shortcut("Ctrl+V"), act(DeskCmd::Paste)),
                MenuNode::new(MenuItem::item("Selecionar Tudo").with_shortcut("Ctrl+A"), act(DeskCmd::SelectAll)),
                MenuNode::sep(),
                MenuNode::new(on(MenuItem::item("Renomear").with_shortcut("Return"), one && files), act(DeskCmd::Rename)),
            ],
            "Visualizar" => vec![
                MenuNode::new(on(MenuItem::item("Limpar"), free), act(DeskCmd::CleanUp)),
                MenuNode::submenu("Organizar por", self.arrange_nodes()),
                MenuNode::sep(),
                MenuNode::new(MenuItem::item("Mostrar Opções de Visualização").with_shortcut("Ctrl+J"), act(DeskCmd::ViewOptions)),
            ],
            "Ir" => {
                let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
                let mut v = vec![
                    MenuNode::new(MenuItem::item("Computador").with_shortcut("Ctrl+Shift+C"), act(DeskCmd::Go("/".into()))),
                    MenuNode::new(MenuItem::item("Início").with_shortcut("Ctrl+Shift+H"), act(DeskCmd::Go(home.clone()))),
                    MenuNode::new(MenuItem::item("Mesa").with_shortcut("Ctrl+Shift+D"), act(DeskCmd::Go(self.desk.dir.clone()))),
                ];
                for (key, label) in [("DOCUMENTS", "Documentos"), ("DOWNLOAD", "Downloads"), ("PICTURES", "Imagens")] {
                    if let Some(p) = user_dir(key) {
                        v.push(MenuNode::new(MenuItem::item(label), act(DeskCmd::Go(p))));
                    }
                }
                v.push(MenuNode::sep());
                v.push(MenuNode::new(MenuItem::item("Lixeira"), Some(MenuAction::DockRun(DockAction::Trash))));
                v
            }
            _ => Vec::new(),
        }
    }

    /// Menu com o nome do app ("Arquivos") com a mesa em foco.
    pub fn desk_app_nodes(&self) -> Vec<MenuNode> {
        vec![
            MenuNode::new(MenuItem::item("Sobre o Arquivos"), Some(MenuAction::AboutApp("glove.Files".into(), "Arquivos".into()))),
            MenuNode::sep(),
            MenuNode::new(
                if crate::shell::trash_has_items() { MenuItem::item("Esvaziar Lixeira…") } else { MenuItem::item("Esvaziar Lixeira…").disabled() },
                Some(MenuAction::EmptyTrash),
            ),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Ocultar Outros").with_shortcut(self.binding("hide_others")), Some(MenuAction::HideOthers(0))),
            MenuNode::new(if self.hidden.is_empty() { MenuItem::item("Mostrar Todos").disabled() } else { MenuItem::item("Mostrar Todos") }, Some(MenuAction::ShowAll)),
        ]
    }

    /// Ctrl+clique no ícone do documento: o arquivo e as pastas até a raiz; escolher abre no Arquivos.
    pub fn open_path_menu(&mut self, doc: &Path, pos: Point<f64, Logical>) {
        let mut nodes = Vec::new();
        let mut cur = Some(doc);
        while let Some(p) = cur {
            let name = if p == Path::new("/") { "Disco".to_string() } else { p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() };
            let icon_dir = if p.is_dir() { p.to_path_buf() } else { p.parent().map(Path::to_path_buf).unwrap_or_default() };
            nodes.push(MenuNode::new(MenuItem::item(name), Some(MenuAction::Desk(DeskCmd::Go(icon_dir)))));
            cur = p.parent();
        }
        self.show_context_menu(nodes, (pos.x - 20.0, pos.y + 10.0).into());
    }

    /// Ctrl+`: traz a janela de baixo do app em foco (dá a volta nas janelas dele).
    pub fn cycle_app_windows(&mut self) {
        let Some(pid) = self.focused_pid() else { return };
        let wins = self.windows_of_pid(pid);
        if wins.len() < 2 {
            return;
        }
        // A de baixo da pilha (a primeira) vem para a frente.
        if let Some(w) = wins.first().cloned() {
            self.focus_window(Some(w));
        }
    }

    /// Menu de contexto no ponto (clamped à tela).
    pub fn show_context_menu(&mut self, nodes: Vec<MenuNode>, pos: Point<f64, Logical>) {
        self.close_menu();
        let mut level = self.new_menu_level(nodes);
        let (sw, sh) = self.output_size();
        let x = (pos.x as i32 + 1).min(sw - level.size.w - 4).max(0);
        let y = if pos.y as i32 + level.size.h > sh - 4 { (pos.y as i32 - level.size.h).max(MENUBAR_H) } else { pos.y as i32 + 1 };
        level.origin = (x, y).into();
        self.chrome.menu = Some(crate::shell::OpenMenu { target: BarTarget::Context, levels: vec![level], pointer_x: None });
        self.dirty = true;
    }

    // -------------------------------------------------------------------
    // Comandos
    // -------------------------------------------------------------------

    pub fn desk_cmd(&mut self, c: DeskCmd) {
        let chosen = self.desk.chosen();
        let files: Vec<PathBuf> = chosen.iter().filter(|i| i.is_file()).map(|i| i.path.clone()).collect();
        let args = |v: &[PathBuf]| v.iter().map(|p| quote(p)).collect::<Vec<_>>().join(" ");
        match c {
            DeskCmd::Open => {
                let all: Vec<PathBuf> = chosen.iter().map(|i| i.path.clone()).collect();
                if !all.is_empty() {
                    self.spawn(&format!("glove-files --abrir {}", args(&all)));
                }
            }
            DeskCmd::Info => {
                let targets: Vec<PathBuf> = if chosen.is_empty() { vec![self.desk.dir.clone()] } else { chosen.iter().map(|i| i.path.clone()).collect() };
                for p in targets.iter().take(10) {
                    self.spawn(&format!("glove-files --info {}", quote(p)));
                }
            }
            DeskCmd::Rename => {
                if let [it] = chosen.as_slice() {
                    if it.is_file() {
                        let name = it.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        self.desk.rename = Some((it.path.clone(), name, true));
                        self.dirty = true;
                    }
                }
            }
            DeskCmd::Duplicate if !files.is_empty() => self.spawn(&format!("glove-files --duplicar {}", args(&files))),
            DeskCmd::Compress if !files.is_empty() => self.spawn(&format!("glove-arquivo compactar {}", args(&files))),
            DeskCmd::Trash if !files.is_empty() => self.spawn(&format!("glove-files --lixo {}", args(&files))),
            DeskCmd::Alias => {
                if let [it] = chosen.as_slice() {
                    let name = it.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| it.name.clone());
                    let mut link = self.desk.dir.join(format!("{name} atalho"));
                    let mut k = 2;
                    while link.symlink_metadata().is_ok() {
                        link = self.desk.dir.join(format!("{name} atalho {k}"));
                        k += 1;
                    }
                    if std::os::unix::fs::symlink(&it.path, &link).is_ok() {
                        self.desk.selected = [link].into_iter().collect();
                        self.refresh_desk(true);
                    }
                }
            }
            DeskCmd::Copy => {
                let all: Vec<PathBuf> = chosen.iter().map(|i| i.path.clone()).collect();
                if !all.is_empty() {
                    self.desk.clip = all;
                    let dh = self.display_handle.clone();
                    smithay::wayland::selection::data_device::set_data_device_selection(&dh, &self.seat.clone(), offered_mimes(), ());
                }
            }
            DeskCmd::Paste => self.desk_paste(),
            DeskCmd::SelectAll => {
                self.desk.selected = self.desk.items.iter().map(|i| i.path.clone()).collect();
                self.dirty = true;
            }
            DeskCmd::NewFolder => self.desk_new_folder(),
            DeskCmd::CleanUp => self.desk_clean_up(),
            DeskCmd::Arrange(by) => {
                // Mantido organizado: muda o critério; senão, organiza uma vez.
                if self.config.desk.arrange.is_empty() {
                    self.desk_arrange_once(&by);
                } else {
                    self.config.desk.arrange = by;
                    let _ = self.config.save();
                    self.desk.layout_key = None;
                    self.dirty = true;
                }
            }
            DeskCmd::ViewOptions => self.spawn("glove-files --opcoes-mesa"),
            DeskCmd::NewWindow => self.spawn("glove-files"),
            DeskCmd::Go(p) => self.spawn(&format!("glove-files {}", quote(&p))),
            DeskCmd::Background => self.run_menu_action(MenuAction::OpenPrefs("mesa".into())),
            DeskCmd::Eject => {
                for it in chosen.iter().filter(|i| i.kind == DeskKind::Removable) {
                    self.spawn(&format!("glove-files --ejetar {}", quote(&it.path)));
                }
                self.desk.selected.clear();
                self.dirty = true;
            }
            DeskCmd::Label(n) => {
                for it in &chosen {
                    let _ = glove_config::arquivos::set_label(&it.path, n);
                }
                self.refresh_desk(true);
            }
            DeskCmd::OpenWith(id) => {
                let all: Vec<PathBuf> = chosen.iter().map(|i| i.path.clone()).collect();
                let cmd = glove_config::arquivos::open_with_command(&id, &all);
                self.spawn(&cmd);
            }
            _ => {}
        }
    }

    fn desk_new_folder(&mut self) {
        let dir = self.desk.dir.clone();
        let mut name = "Pasta sem título".to_string();
        let mut n = 2;
        while dir.join(&name).exists() {
            name = format!("Pasta sem título {n}");
            n += 1;
        }
        let path = dir.join(&name);
        if std::fs::create_dir(&path).is_err() {
            return;
        }
        // Pelo menu do fundo, nasce onde se clicou.
        if let Some(at) = self.desk.menu_at.take() {
            let cell = self.desk_cell();
            self.desk.saved.insert(name.clone(), (at.x as i32 - cell.0 / 2, at.y as i32 - 8));
            self.desk.store_saved();
        }
        self.desk.selected = [path.clone()].into_iter().collect();
        self.refresh_desk(true);
        // Como no Mac: a pasta nova já entra renomeando.
        self.desk.rename = Some((path, name, true));
        self.dirty = true;
    }

    /// Colar na mesa: os arquivos da área de transferência são copiados para cá.
    fn desk_paste(&mut self) {
        use smithay::wayland::selection::data_device::request_data_device_client_selection;
        let dir = self.desk.dir.clone();
        let Some((read, write)) = pipe() else { return };
        match request_data_device_client_selection::<Glove>(&self.seat, URI_LIST.to_string(), write) {
            Ok(()) => self.read_pipe(read, move |st, data| {
                let paths = parse_uri_list(&String::from_utf8_lossy(&data));
                st.desk_transfer(&paths, &dir, false);
            }),
            // A área de transferência é do próprio Glove (copiado da mesa).
            Err(_) => {
                let clip = self.desk.clip.clone();
                self.desk_transfer(&clip, &dir, false);
            }
        }
    }

    /// Termina o renomear (Enter ou clique fora).
    pub fn desk_commit_rename(&mut self) {
        let Some((path, text, _)) = self.desk.rename.take() else { return };
        self.dirty = true;
        let text = text.trim().to_string();
        let old = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if text.is_empty() || text == old || text.contains('/') {
            return;
        }
        let dest = path.with_file_name(&text);
        if dest.symlink_metadata().is_ok() {
            return;
        }
        if std::fs::rename(&path, &dest).is_ok() {
            // A posição acompanha o nome novo.
            if let Some(pos) = self.desk.saved.remove(&old) {
                self.desk.saved.insert(text, pos);
                self.desk.store_saved();
            }
            if self.desk.selected.remove(&path) {
                self.desk.selected.insert(dest);
            }
            self.refresh_desk(true);
        }
    }

    // -------------------------------------------------------------------
    // Teclado
    // -------------------------------------------------------------------

    /// Tecla com a mesa em foco. Devolve verdadeiro se a mesa usou a tecla.
    pub fn desk_key(&mut self, k: DeskKey) {
        if let Some((_, text, all)) = &mut self.desk.rename {
            match k.kind {
                DeskKeyKind::Char(c) if !k.ctrl && !c.is_control() => {
                    if *all {
                        text.clear();
                        *all = false;
                    }
                    if text.len() < 250 {
                        text.push(c);
                    }
                }
                DeskKeyKind::Back => {
                    if *all {
                        text.clear();
                        *all = false;
                    } else {
                        text.pop();
                    }
                }
                DeskKeyKind::Left | DeskKeyKind::Right => *all = false,
                DeskKeyKind::Enter => self.desk_commit_rename(),
                DeskKeyKind::Esc => self.desk.rename = None,
                _ => {}
            }
            self.dirty = true;
            return;
        }
        if k.ctrl {
            let cmd = match (k.kind, k.shift) {
                (DeskKeyKind::Char('n'), true) => Some(DeskCmd::NewFolder),
                (DeskKeyKind::Char('n'), false) => Some(DeskCmd::NewWindow),
                (DeskKeyKind::Char('o'), false) | (DeskKeyKind::Down, _) => Some(DeskCmd::Open),
                (DeskKeyKind::Char('i'), _) => Some(DeskCmd::Info),
                (DeskKeyKind::Char('d'), false) => Some(DeskCmd::Duplicate),
                (DeskKeyKind::Char('l'), _) => Some(DeskCmd::Alias),
                (DeskKeyKind::Char('e'), _) => Some(DeskCmd::Eject),
                (DeskKeyKind::Char('j'), _) => Some(DeskCmd::ViewOptions),
                (DeskKeyKind::Char('a'), _) => Some(DeskCmd::SelectAll),
                (DeskKeyKind::Char('c'), false) => Some(DeskCmd::Copy),
                (DeskKeyKind::Char('v'), _) => Some(DeskCmd::Paste),
                (DeskKeyKind::Back | DeskKeyKind::Del, _) => Some(DeskCmd::Trash),
                (DeskKeyKind::Char('c'), true) => Some(DeskCmd::Go("/".into())),
                (DeskKeyKind::Char('h'), true) => Some(DeskCmd::Go(std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default())),
                (DeskKeyKind::Char('d'), true) => Some(DeskCmd::Go(self.desk.dir.clone())),
                (DeskKeyKind::Char('o'), true) => user_dir("DOCUMENTS").map(DeskCmd::Go),
                _ => None,
            };
            if let Some(c) = cmd {
                self.desk_cmd(c);
            }
            return;
        }
        match k.kind {
            DeskKeyKind::Enter => self.desk_cmd(DeskCmd::Rename),
            DeskKeyKind::Esc => {
                self.desk.selected.clear();
                self.dirty = true;
            }
            DeskKeyKind::Up | DeskKeyKind::Down | DeskKeyKind::Left | DeskKeyKind::Right => self.desk_arrow(k.kind),
            DeskKeyKind::Char(c) if !c.is_control() && !c.is_whitespace() => self.desk_type_select(c),
            _ => {}
        }
    }

    /// Setas: vai para o ícone mais perto naquela direção.
    fn desk_arrow(&mut self, dir: DeskKeyKind) {
        if self.desk.items.is_empty() {
            return;
        }
        let center = |r: &Rectangle<i32, Logical>| (r.loc.x + r.size.w / 2, r.loc.y + r.size.h / 2);
        let cur = self.desk.items.iter().position(|i| self.desk.selected.contains(&i.path));
        let next = match cur {
            None => Some(0),
            Some(c) => {
                let (cx, cy) = center(&self.desk.rects[c]);
                (0..self.desk.items.len())
                    .filter(|&i| i != c)
                    .filter_map(|i| {
                        let (x, y) = center(&self.desk.rects[i]);
                        let (dx, dy) = (x - cx, y - cy);
                        let (along, across) = match dir {
                            DeskKeyKind::Up => (-dy, dx),
                            DeskKeyKind::Down => (dy, dx),
                            DeskKeyKind::Left => (-dx, dy),
                            _ => (dx, dy),
                        };
                        (along > 0 && across.abs() <= along * 2).then_some((i, along as i64 + 2 * across.abs() as i64))
                    })
                    .min_by_key(|(_, d)| *d)
                    .map(|(i, _)| i)
            }
        };
        if let Some(i) = next {
            self.desk.selected = [self.desk.items[i].path.clone()].into_iter().collect();
            self.dirty = true;
        }
    }

    /// Digitar o começo do nome escolhe o item.
    fn desk_type_select(&mut self, c: char) {
        let now = Instant::now();
        if self.desk.typed.1.is_none_or(|t| now.duration_since(t).as_millis() > 1000) {
            self.desk.typed.0.clear();
        }
        self.desk.typed.0.extend(c.to_lowercase());
        self.desk.typed.1 = Some(now);
        let q = crate::search::normalize(&self.desk.typed.0);
        let mut order: Vec<usize> = (0..self.desk.items.len()).collect();
        order.sort_by_key(|&i| self.desk.items[i].name.to_lowercase());
        if let Some(i) = order.into_iter().find(|&i| crate::search::normalize(&self.desk.items[i].name).starts_with(&q)) {
            self.desk.selected = [self.desk.items[i].path.clone()].into_iter().collect();
            self.dirty = true;
        }
    }
}

/// Tecla para a mesa, já interpretada.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeskKeyKind {
    Char(char),
    Back,
    Del,
    Enter,
    Esc,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeskKey {
    pub kind: DeskKeyKind,
    pub ctrl: bool,
    pub shift: bool,
}

/// Interpreta a tecla (nome xkb sem modificadores) para a mesa; None = não é dela.
pub fn desk_key_of(name: &str, ch: Option<char>, ctrl: bool, shift: bool, renaming: bool) -> Option<DeskKey> {
    let kind = match name {
        "return" | "kp_enter" => DeskKeyKind::Enter,
        "escape" => DeskKeyKind::Esc,
        "backspace" => DeskKeyKind::Back,
        "delete" => DeskKeyKind::Del,
        "up" => DeskKeyKind::Up,
        "down" => DeskKeyKind::Down,
        "left" => DeskKeyKind::Left,
        "right" => DeskKeyKind::Right,
        // Com Ctrl vale a letra da tecla (independe do layout); sem, o caractere digitado.
        n if ctrl && n.chars().count() == 1 => DeskKeyKind::Char(n.chars().next().unwrap()),
        _ => DeskKeyKind::Char(ch.filter(|c| renaming || !c.is_control())?),
    };
    Some(DeskKey { kind, ctrl, shift })
}

fn user_dir(key: &str) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let text = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok()?;
    let line = text.lines().find(|l| l.starts_with(&format!("XDG_{key}_DIR=")))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(v),
    };
    (p != home && p.is_dir()).then_some(p)
}

/// Pipe com a ponta de leitura sem bloquear.
pub fn pipe() -> Option<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    use std::os::fd::FromRawFd;
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return None;
    }
    unsafe {
        let fl = libc::fcntl(fds[0], libc::F_GETFL);
        libc::fcntl(fds[0], libc::F_SETFL, fl | libc::O_NONBLOCK);
        Some((std::os::fd::OwnedFd::from_raw_fd(fds[0]), std::os::fd::OwnedFd::from_raw_fd(fds[1])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_vao_e_voltam() {
        let p = PathBuf::from("/home/ana/Área de trabalho/nota 1.txt");
        let u = file_uri(&p);
        assert_eq!(u, "file:///home/ana/%C3%81rea%20de%20trabalho/nota%201.txt");
        assert_eq!(parse_uri_list(&format!("{u}\r\n# comentário\r\nfile://host/tmp/x\r\n")), vec![p, PathBuf::from("/tmp/x")]);
    }

    #[test]
    fn grade_comeca_no_canto_direito() {
        let area = Rectangle::new((0, 22).into(), (1000, 600).into());
        let cells = grid_cells(area, (90, 80));
        assert_eq!(cells[0], (1000 - MARGIN - 90, 22 + MARGIN).into());
        // A segunda célula fica embaixo da primeira (coluna por coluna).
        assert_eq!(cells[1].x, cells[0].x);
        assert!(cells[1].y > cells[0].y);
    }

    #[test]
    fn teclas_da_mesa() {
        assert_eq!(desk_key_of("n", Some('N'), true, true, false).map(|k| k.kind), Some(DeskKeyKind::Char('n')));
        assert_eq!(desk_key_of("a", Some('á'), false, false, true).map(|k| k.kind), Some(DeskKeyKind::Char('á')));
        assert_eq!(desk_key_of("f5", None, false, false, false), None);
    }
}
