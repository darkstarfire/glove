//! Arquivos: navegador de pastas do Glove (colunas, lista e ícones).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}, Color},
    run,
    ui::Key,
    App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const TOOLBAR_H: f32 = 62.0;
const STATUS_H: f32 = 20.0;
const ROW_H: f32 = 19.0;
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
    Picture,
    Other,
}

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    kind: Kind,
    size: u64,
    modified: Option<SystemTime>,
}

impl Entry {
    fn icon(&self) -> Icon {
        match self.kind {
            Kind::Folder => Icon::Folder,
            Kind::Text => Icon::Document,
            Kind::Picture => Icon::Picture,
            Kind::Other => Icon::File,
        }
    }
    fn type_name(&self) -> &'static str {
        match self.kind {
            Kind::Folder => "Pasta",
            Kind::Text => "Documento de texto",
            Kind::Picture => "Imagem",
            Kind::Other => "Documento",
        }
    }
}

fn kind_of(path: &Path, is_dir: bool) -> Kind {
    if is_dir {
        return Kind::Folder;
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" | "rs" | "toml" | "json" | "c" | "h" | "cpp" | "py" | "sh" | "log" | "conf" | "ini" | "csv" | "xml" | "html" | "css" | "js" | "ts" | "yaml" | "yml" | "desktop" => Kind::Text,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "tif" | "tiff" => Kind::Picture,
        _ => Kind::Other,
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

fn xdg_dir(names: &[&str]) -> PathBuf {
    let h = home();
    names.iter().map(|n| h.join(n)).find(|p| p.is_dir()).unwrap_or(h)
}

fn trash_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
    base.join("Trash")
}

fn display_name(p: &Path) -> String {
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
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        format!("{:02}/{:02}/{} {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_year + 1900, tm.tm_hour, tm.tm_min)
    }
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
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            Some(Entry {
                kind: kind_of(&path, is_dir),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                name,
                path,
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

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

fn open_file(e: &Entry) {
    let _ = match e.kind {
        Kind::Text => std::process::Command::new(sibling("glove-edit")).arg(&e.path).spawn(),
        Kind::Picture if e.path.extension().and_then(|x| x.to_str()).is_some_and(|x| ["png", "jpg", "jpeg"].contains(&x.to_ascii_lowercase().as_str())) => {
            std::process::Command::new(sibling("glove-view")).arg(&e.path).spawn()
        }
        _ => std::process::Command::new("xdg-open").arg(&e.path).spawn(),
    };
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
    hidden: bool,
    cache: HashMap<PathBuf, (Option<SystemTime>, bool, Result<Vec<Entry>, String>)>,
    scroll: HashMap<PathBuf, f32>,
    message: Option<String>,
    last_title: String,
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
            hidden: false,
            cache: HashMap::new(),
            scroll: HashMap::new(),
            message: None,
            last_title: String::new(),
        }
    }

    /// Pasta "atual": a última coluna aberta.
    fn current(&self) -> PathBuf {
        self.columns.last().cloned().unwrap_or_else(|| self.base.clone())
    }

    /// Conteúdo de uma pasta, relido só se ela mudou (data de modificação).
    fn list(&mut self, dir: &Path) -> Result<Vec<Entry>, String> {
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
        let Some(sel) = self.selected.clone() else { return };
        match move_to_trash(&sel) {
            Ok(()) => {
                self.columns.retain(|c| !c.starts_with(&sel));
                if self.columns.is_empty() {
                    self.columns.push(self.base.clone());
                }
                self.selected = None;
                self.message = None;
            }
            Err(e) => self.message = Some(e),
        }
    }

    // -----------------------------------------------------------------------
    // Barra de ferramentas
    // -----------------------------------------------------------------------

    fn toolbar(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, TOOLBAR_H, &vgrad(0.0, TOOLBAR_H, &[(0.0, rgba(246, 246, 246, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.pinstripes(0.0, 0.0, w, TOOLBAR_H, rgba(255, 255, 255, 0), rgba(255, 255, 255, 60));
        ui.c.hline(0.0, TOOLBAR_H - 1.0, w, rgba(130, 130, 136, 255));
        let light = rgba(240, 240, 243, 255);
        let accent = ui.theme.accent;

        // Voltar / Avançar
        let (by, bh) = (12.0, 26.0);
        if ui.gel_button(Rect::new(12.0, by, 30.0, bh), "◀", light, 11.0, false) {
            self.go_back();
        }
        if ui.gel_button(Rect::new(44.0, by, 30.0, bh), "▶", light, 11.0, false) {
            self.go_forward();
        }
        // Modo de visualização (segmentado)
        let views = [(View::Icons, "Ícones"), (View::List, "Lista"), (View::Columns, "Colunas")];
        let mut x = 90.0;
        for (v, label) in views {
            let tw = ui.text_width(label, 12.0, true) + 22.0;
            if ui.gel_button(Rect::new(x, by, tw, bh), label, if self.view == v { accent } else { light }, 12.0, false) {
                self.view = v;
            }
            x += tw + 2.0;
        }
        // Lugares
        let places = [
            ("Computador", Icon::Disk, PathBuf::from("/")),
            ("Início", Icon::Folder, home()),
            ("Documentos", Icon::Folder, xdg_dir(&["Documentos", "Documents"])),
            ("Imagens", Icon::Folder, xdg_dir(&["Imagens", "Pictures"])),
        ];
        let mut px = w - 12.0;
        for (label, icon, path) in places.iter().rev() {
            let tw = (ui.text_width(label, 11.0, false) + 12.0).max(58.0);
            px -= tw;
            let r = Rect::new(px, 4.0, tw, TOOLBAR_H - 8.0);
            let resp = ui.hit(r);
            if resp.down || resp.hover {
                ui.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, if resp.down { 45 } else { 18 })));
            }
            icons::draw(ui.c, ui.fonts, *icon, r.x + (r.w - 32.0) / 2.0, r.y + 2.0, 32.0, None);
            let lw = ui.text_width(label, 11.0, false);
            ui.fonts.draw(ui.c, label, r.x + (r.w - lw) / 2.0, r.y + r.h - 5.0, 11.0, false, rgba(30, 30, 30, 255));
            if resp.clicked {
                self.go(path.clone());
            }
            px -= 4.0;
        }
    }

    fn status(&mut self, ui: &mut Ui, y: f32) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, y, w, STATUS_H, &vgrad(y, y + STATUS_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(0.0, y + STATUS_H - 1.0, w, rgba(150, 150, 156, 255));
        let cur = self.current();
        let n = self.list(&cur).map(|v| v.len()).unwrap_or(0);
        let mut text = format!("{n} {}", if n == 1 { "item" } else { "itens" });
        if let Some(free) = free_space(&cur) {
            text.push_str(&format!(", {} disponíveis", human_size(free)));
        }
        if let Some(m) = &self.message {
            text = m.clone();
        }
        let tw = ui.text_width(&text, 11.0, false);
        ui.fonts.draw(ui.c, &text, (w - tw) / 2.0, y + 14.0, 11.0, false, rgba(60, 60, 60, 255));
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
        off = (off + ui.scroll_in(r)).clamp(0.0, (content - r.h).max(0.0));
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
            }
        }
        let accent = ui.theme.accent;
        ui.clipped(ci as u64 * 7919 + 1, Rect::new(r.x, r.y, r.w - 1.0, r.h), |c, fonts| {
            c.clear(Color::WHITE);
            for (i, e) in entries.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * ROW_H - off;
                let on = chosen.as_ref() == Some(&e.path);
                if on {
                    let col = if last { accent } else { rgba(200, 204, 212, 255) };
                    c.fill_rect(0.0, y, r.w, ROW_H, &vgrad(y, y + ROW_H, &[(0.0, lighten(col, 0.15)), (1.0, col)]));
                }
                icons::draw(c, fonts, e.icon(), 6.0, y + 1.5, 16.0, None);
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
        if let Some((e, double)) = action {
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
        icons::draw(ui.c, ui.fonts, e.icon(), cx - 48.0, r.y + 24.0, 96.0, None);
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

    fn list_view(&mut self, ui: &mut Ui, area: Rect) {
        let dir = self.current();
        let entries = match self.list(&dir) {
            Ok(v) => v,
            Err(e) => {
                ui.fonts.draw(ui.c, &e, area.x + 10.0, area.y + 24.0, 12.0, false, rgba(150, 40, 40, 255));
                return;
            }
        };
        let header = 20.0;
        let cols = [("Nome", 0.0), ("Data de modificação", 0.45), ("Tamanho", 0.7), ("Tipo", 0.82)];
        ui.c.fill_rect(area.x, area.y, area.w, header, &vgrad(area.y, area.y + header, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(214, 216, 222, 255))]));
        ui.c.hline(area.x, area.y + header - 1.0, area.w, rgba(150, 150, 156, 255));
        for (i, (t, f)) in cols.iter().enumerate() {
            let x = area.x + area.w * f;
            if i > 0 {
                ui.c.vline(x, area.y + 3.0, header - 6.0, rgba(160, 160, 166, 255));
            }
            ui.fonts.draw(ui.c, t, x + 8.0, area.y + 14.0, 11.0, false, rgba(30, 30, 30, 255));
        }
        let body = Rect::new(area.x, area.y + header, area.w, area.h - header);
        let content = entries.len() as f32 * ROW_H;
        let mut off = *self.scroll.get(&dir).unwrap_or(&0.0);
        off = (off + ui.scroll_in(body)).clamp(0.0, (content - body.h).max(0.0));
        self.scroll.insert(dir.clone(), off);
        let first = (off / ROW_H).floor() as usize;
        let count = (body.h / ROW_H).ceil() as usize + 1;
        let mut action: Option<(Entry, bool)> = None;
        for (i, e) in entries.iter().enumerate().skip(first).take(count) {
            let y = body.y + i as f32 * ROW_H - off;
            let (top, bot) = (y.max(body.y), (y + ROW_H).min(body.y + body.h));
            if bot > top {
                let resp = ui.hit(Rect::new(body.x, top, body.w, bot - top));
                if resp.pressed {
                    action = Some((e.clone(), resp.double));
                }
            }
        }
        let sel = self.selected.clone();
        let accent = ui.theme.accent;
        ui.clipped(91, body, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, e) in entries.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * ROW_H - off;
                let on = sel.as_ref() == Some(&e.path);
                if on {
                    c.fill_rect(0.0, y, body.w, ROW_H, &vgrad(y, y + ROW_H, &[(0.0, lighten(accent, 0.15)), (1.0, accent)]));
                } else if i % 2 == 1 {
                    c.fill_rect(0.0, y, body.w, ROW_H, &solid(rgba(237, 243, 254, 255)));
                }
                let ink = if on { Color::WHITE } else { rgba(20, 20, 20, 255) };
                icons::draw(c, fonts, e.icon(), 6.0, y + 1.5, 16.0, None);
                let cells = [
                    e.name.clone(),
                    human_date(e.modified),
                    if e.kind == Kind::Folder { "—".into() } else { human_size(e.size) },
                    e.type_name().to_string(),
                ];
                for (j, (_, f)) in cols.iter().enumerate() {
                    let x = body.w * f + if j == 0 { 26.0 } else { 8.0 };
                    let next = cols.get(j + 1).map(|c| body.w * c.1).unwrap_or(body.w);
                    let t = fonts.ellipsize(&cells[j], 12.0, false, next - x - 6.0);
                    fonts.draw(c, &t, x, y + 14.0, 12.0, false, ink);
                }
            }
        });
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
        let per_row = ((area.w - 16.0) / cw).floor().max(1.0) as usize;
        let rows = entries.len().div_ceil(per_row);
        let content = rows as f32 * ch + 16.0;
        let mut off = *self.scroll.get(&dir).unwrap_or(&0.0);
        off = (off + ui.scroll_in(area)).clamp(0.0, (content - area.h).max(0.0));
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
                }
            }
        }
        let sel = self.selected.clone();
        let accent = ui.theme.accent;
        ui.clipped(92, area, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, e) in entries.iter().enumerate() {
                let (x, y) = cell(i);
                if y + ch < 0.0 || y > area.h {
                    continue;
                }
                let on = sel.as_ref() == Some(&e.path);
                if on {
                    c.fill(rrect(x + cw / 2.0 - 30.0, y, 60.0, 56.0, 6.0), &solid(rgba(0, 0, 0, 40)));
                }
                icons::draw(c, fonts, e.icon(), x + cw / 2.0 - 26.0, y + 2.0, 52.0, None);
                let name = fonts.ellipsize(&e.name, 12.0, false, cw - 8.0);
                let tw = fonts.measure(&name, 12.0, false);
                let (lx, ly) = (x + (cw - tw) / 2.0, y + 72.0);
                if on {
                    c.fill(rrect(lx - 5.0, ly - 12.0, tw + 10.0, 16.0, 8.0), &solid(accent));
                }
                fonts.draw(c, &name, lx, ly, 12.0, false, if on { Color::WHITE } else { rgba(20, 20, 20, 255) });
            }
        });
        if let Some((e, double)) = action {
            self.selected = Some(e.path.clone());
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
        let mut act: Vec<&str> = Vec::new();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => act.push("new-window"),
                "Ctrl+Shift+N" => act.push("new-folder"),
                "Ctrl+O" => act.push("open"),
                "Ctrl+BackSpace" => act.push("trash"),
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
                let _ = std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "glove-files".into())).arg(self.current()).spawn();
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
            "go-documents" => self.go(xdg_dir(&["Documentos", "Documents"])),
            "go-pictures" => self.go(xdg_dir(&["Imagens", "Pictures"])),
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
        self.toolbar(ui);
        self.status(ui, TOOLBAR_H);
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(0.0, TOOLBAR_H + STATUS_H, w, h - TOOLBAR_H - STATUS_H - 15.0);
        match self.view {
            View::Columns => self.columns_view(ui, area),
            View::List => self.list_view(ui, area),
            View::Icons => self.icons_view(ui, area),
        }
        // Faixa inferior (onde fica o puxador de redimensionar)
        ui.c.fill_rect(0.0, h - 15.0, w, 15.0, &vgrad(h - 15.0, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, h - 15.0, w, rgba(150, 150, 156, 255));
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
                    MenuItem::new("trash", "Mover para a Lixeira", "Ctrl+BackSpace").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar janela", "Ctrl+W"),
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
                    MenuItem::new("go-documents", "Documentos", ""),
                    MenuItem::new("go-pictures", "Imagens", ""),
                    MenuItem::new("go-trash", "Lixeira", ""),
                ],
            ),
        ]
    }
}

fn main() {
    let start = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.is_dir()).unwrap_or_else(home);
    let opts = WindowOptions { title: display_name(&start), app_id: "glove.Files".into(), size: (780, 470), fixed: false };
    if let Err(e) = run(opts, Files::new(start)) {
        eprintln!("glove-files: {e}");
    }
}
