//! Cópias de Segurança: cópias automáticas da pasta pessoal num disco externo,
//! a cada hora, com uma linha do tempo para voltar no tempo e restaurar arquivos
//! (o motor está em `motor.rs`).
//!
//! `glove-copias --auto` (o Glove chama a cada hora): faz a cópia, se ligada,
//! se o disco estiver conectado e se a última tiver mais de uma hora; no fim avisa.

mod motor;

use glove_config::tr;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use glove_ui::{
    filebrowser::{Choice, FileBrowser},
    paint::{draw::*, icons::Icon},
    run, AlertKind, App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};
use motor::{Prefs, Progress, Snapshot};

const TOP_H: f32 = 86.0;
const LIST_W: f32 = 230.0;

/// "Hoje, 14:05", "Ontem, 09:12" ou "5 de novembro de 2026, 10:00".
fn when(t: SystemTime) -> String {
    let (y, mo, d, h, mi, ..) = motor::local(t);
    let (ty, tmo, td, ..) = motor::local(SystemTime::now());
    let (yy, ymo, yd, ..) = motor::local(SystemTime::now() - Duration::from_secs(86_400));
    let hm = format!("{h:02}:{mi:02}");
    if (y, mo, d) == (ty, tmo, td) {
        tr!("Hoje, {}", hm)
    } else if (y, mo, d) == (yy, ymo, yd) {
        tr!("Ontem, {}", hm)
    } else {
        let meses = [tr!("janeiro"), tr!("fevereiro"), tr!("março"), tr!("abril"), tr!("maio"), tr!("junho"), tr!("julho"), tr!("agosto"), tr!("setembro"), tr!("outubro"), tr!("novembro"), tr!("dezembro")];
        tr!("{0} de {1} de {2}, {3}", d, meses[(mo as usize + 11) % 12], y, hm)
    }
}

fn human_size(n: u64) -> String {
    let v = n as f64;
    let s = if v < 1024.0 { format!("{n} B") } else if v < 1024.0 * 1024.0 { format!("{:.0} KB", v / 1024.0) } else if v < 1024.0 * 1024.0 * 1024.0 { format!("{:.1} MB", v / 1048576.0) } else { format!("{:.1} GB", v / 1073741824.0) };
    s.replace('.', glove_config::virgula())
}

/// Aviso na central do Glove (ou de outro desktop).
fn notify(summary: &str, body: &str) {
    use std::collections::HashMap;
    let Ok(c) = zbus::blocking::Connection::session() else { return };
    let hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
    let _ = c.call_method(Some("org.freedesktop.Notifications"), "/org/freedesktop/Notifications", Some("org.freedesktop.Notifications"), "Notify", &(tr!("Cópias de Segurança"), 0u32, "glove-copias", summary, body, Vec::<&str>::new(), hints, -1i32));
}

enum Sheet {
    None,
    Dest(FileBrowser),
    Message(String),
}

struct Copias {
    prefs: Prefs,
    snaps: Vec<Snapshot>,
    sel: Option<usize>,
    /// Pasta aberta dentro da cópia escolhida.
    dir: Option<PathBuf>,
    item: Option<usize>,
    entries: Vec<(String, PathBuf, bool, u64, Option<SystemTime>)>,
    table: TableState,
    list: TableState,
    job: Option<Arc<Progress>>,
    sheet: Sheet,
}

impl Copias {
    fn new() -> Self {
        let mut c = Self { prefs: Prefs::load(), snaps: Vec::new(), sel: None, dir: None, item: None, entries: Vec::new(), table: TableState::default(), list: TableState::default(), job: None, sheet: Sheet::None };
        c.reload();
        c
    }

    fn reload(&mut self) {
        self.snaps = self.prefs.dest.as_deref().map(motor::snapshots).unwrap_or_default();
        if self.sel.is_none_or(|i| i >= self.snaps.len()) {
            self.sel = (!self.snaps.is_empty()).then_some(0);
        }
        self.dir = self.sel.map(|i| self.snaps[i].path.clone());
        self.read_dir();
    }

    fn read_dir(&mut self) {
        self.item = None;
        self.entries.clear();
        let Some(d) = &self.dir else { return };
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                // Itens ocultos ficam de fora da lista (como no Arquivos); estão na cópia.
                if e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let Ok(m) = e.metadata() else { continue };
                self.entries.push((e.file_name().to_string_lossy().into_owned(), e.path(), m.is_dir(), m.len(), m.modified().ok()));
            }
        }
        self.entries.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        self.table.scroll = 0.0;
    }

    fn start(&mut self) {
        if self.job.is_some() {
            return;
        }
        let p = Arc::new(Progress::default());
        let (prefs, pp) = (self.prefs.clone(), p.clone());
        std::thread::spawn(move || {
            let r = motor::backup(&prefs, &pp);
            *pp.result.lock().unwrap() = Some(r);
            pp.done.store(true, Ordering::Relaxed);
        });
        self.job = Some(p);
    }

    fn snapshot_root(&self) -> Option<&Path> {
        self.sel.and_then(|i| self.snaps.get(i)).map(|s| s.path.as_path())
    }

    fn top(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, TOP_H, &vgrad(0.0, TOP_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, TOP_H - 1.0, w, rgba(140, 140, 146, 255));
        glove_ui::paint::icons::draw(ui.c, ui.fonts, Icon::from_name("backup"), 16.0, 11.0, 64.0, None);
        let ink = rgba(25, 25, 28, 255);
        let gray = rgba(100, 100, 106, 255);
        let (line1, line2) = match (&self.prefs.dest, &self.job) {
            (None, _) => (tr!("Nenhum disco de cópias escolhido").to_string(), tr!("Escolha um disco externo (ou uma pasta) para guardar as cópias.").to_string()),
            (Some(_), Some(p)) => {
                let files = p.files.load(Ordering::Relaxed);
                let copied = p.copied.load(Ordering::Relaxed);
                (tr!("Fazendo a cópia…").to_string(), tr!("{} arquivos vistos, {} copiados ({})", files, copied, human_size(p.bytes.load(Ordering::Relaxed))))
            }
            (Some(d), None) => {
                let last = self.snaps.first().map(|s| when(s.time)).unwrap_or_else(|| tr!("nenhuma ainda").into());
                let free = motor::free_space(d).map(|f| tr!(" — {} livres", human_size(f))).unwrap_or_default();
                let conn = if d.is_dir() { String::new() } else { tr!(" (desconectado)").into() };
                (tr!("Última cópia: {}", last), format!("{}{conn}{free}", d.display()))
            }
        };
        ui.fonts.draw(ui.c, &line1, 96.0, 34.0, 15.0, true, ink);
        let l2 = ui.fonts.ellipsize(&line2, 11.5, false, w - 96.0 - 230.0);
        ui.fonts.draw(ui.c, &l2, 96.0, 53.0, 11.5, false, gray);
        let job = self.job.clone();
        let mut act = None;
        ui.area(Rect::new(96.0, 60.0, w - 330.0, 22.0), |ui| {
            match &job {
                Some(_) => ui.progress(None, (w - 330.0).min(260.0)),
                None => {
                    let mut auto = self.prefs.auto;
                    if ui.checkbox(&mut auto, tr!("Cópias automáticas a cada hora")) {
                        act = Some("auto");
                    }
                }
            }
        });
        ui.area(Rect::new(w - 220.0, 18.0, 204.0, 60.0), |ui| {
            if job.is_some() {
                if ui.button(tr!("Cancelar")) {
                    act = Some("cancel");
                }
            } else {
                if self.prefs.dest.is_some() && ui.default_button(tr!("Fazer Cópia Agora")) {
                    act = Some("now");
                }
                if ui.button(tr!("Escolher Disco…")) {
                    act = Some("dest");
                }
            }
        });
        match act {
            Some("auto") => {
                self.prefs.auto = !self.prefs.auto;
                self.prefs.save();
            }
            Some("cancel") => {
                if let Some(p) = &self.job {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            }
            Some("now") => self.start(),
            Some("dest") => self.sheet = Sheet::Dest(FileBrowser::folder(Some(PathBuf::from("/media")).filter(|p| p.is_dir()))),
            _ => {}
        }
    }

    fn body(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let y = TOP_H;
        // Linha do tempo: as cópias, da mais nova para a mais velha.
        let rows: Vec<TableRow> = self.snaps.iter().map(|s| TableRow { cells: vec![when(s.time)], icon: Some(Icon::Clock(10, 10)), depth: 0, children: None, tint: None }).collect();
        let cols = [Column::new(tr!("Cópias"), 1.0)];
        let r = ui.table(Rect::new(0.0, y, LIST_W, h - y), &cols, &rows, self.sel, &mut self.list, &TableStyle { row_h: 22.0, ..TableStyle::default() });
        if let Some(i) = r.pressed {
            if self.sel != Some(i) {
                self.sel = Some(i);
                self.dir = Some(self.snaps[i].path.clone());
                self.read_dir();
            }
        }
        ui.c.vline(LIST_W, y, h - y, rgba(140, 140, 146, 255));
        let x = LIST_W + 1.0;
        let cw = w - x;
        if self.snaps.is_empty() {
            let t = if self.prefs.dest.is_none() { tr!("Escolha um disco para começar.") } else { tr!("Nenhuma cópia ainda. Clique em Fazer Cópia Agora.") };
            let tw = ui.text_width(t, 13.0, false);
            ui.fonts.draw(ui.c, t, x + (cw - tw) / 2.0, y + (h - y) / 2.0, 13.0, false, rgba(110, 110, 116, 255));
            return;
        }
        // Caminho dentro da cópia e os botões.
        let bar = 34.0;
        let root = self.snapshot_root().map(Path::to_path_buf);
        let rel = match (&self.dir, &root) {
            (Some(d), Some(r)) => d.strip_prefix(r).map(|p| p.to_path_buf()).unwrap_or_default(),
            _ => PathBuf::new(),
        };
        let shown = if rel.as_os_str().is_empty() { tr!("Pasta pessoal").to_string() } else { format!("~/{}", rel.display()) };
        let mut act = None;
        ui.area(Rect::new(x + 8.0, y + 6.0, cw - 16.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                if ui.button(tr!("‹ Acima")) {
                    act = Some("up");
                }
                let base = matches!(self.sheet, Sheet::None);
                ui.set_enabled(base && self.item.is_some());
                if ui.button(tr!("Restaurar")) {
                    act = Some("restore");
                }
                ui.set_enabled(base);
                if ui.button(tr!("Mostrar no Arquivos")) {
                    act = Some("files");
                }
            });
        });
        let tx = x + 8.0 + 330.0;
        let st = ui.fonts.ellipsize(&shown, 11.5, true, (w - tx - 10.0).max(40.0));
        ui.fonts.draw(ui.c, &st, tx, y + 22.0, 11.5, true, rgba(60, 60, 66, 255));
        let rows: Vec<TableRow> = self
            .entries
            .iter()
            .map(|(n, p, dir, size, m)| TableRow {
                cells: vec![n.clone(), if *dir { "—".into() } else { human_size(*size) }, m.map(when).unwrap_or_default()],
                icon: Some(glove_ui::paint::icons::file_icon(p, *dir)),
                depth: 0,
                children: None,
                tint: None,
            })
            .collect();
        let cols = [Column::new(tr!("Nome"), 0.5), Column::new(tr!("Tamanho"), 0.18).right(), Column::new(tr!("Modificado"), 0.32)];
        let r = ui.table(Rect::new(x, y + bar, cw, h - y - bar), &cols, &rows, self.item, &mut self.table, &TableStyle::default());
        if let Some(i) = r.pressed {
            self.item = Some(i);
        }
        if let Some(i) = r.double {
            if self.entries[i].2 {
                self.dir = Some(self.entries[i].1.clone());
                self.read_dir();
            }
        }
        match act {
            Some("up") => {
                if let (Some(d), Some(r)) = (&self.dir, &root) {
                    if d != r {
                        self.dir = d.parent().map(Path::to_path_buf);
                        self.read_dir();
                    }
                }
            }
            Some("restore") => {
                if let (Some(i), Some(r)) = (self.item, &root) {
                    let item = self.entries[i].1.clone();
                    self.sheet = Sheet::Message(match motor::restore(r, &item) {
                        Ok(t) => tr!("Restaurado em {}.", t.display()),
                        Err(e) => tr!("Não foi possível restaurar: {}", e),
                    });
                }
            }
            Some("files") => {
                if let Some(d) = &self.dir {
                    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-files".into());
                    let _ = std::process::Command::new(exe).arg(d).spawn();
                }
            }
            _ => {}
        }
    }
}

impl App for Copias {
    fn ui(&mut self, ui: &mut Ui) {
        // Cópia terminou?
        if let Some(p) = self.job.clone() {
            if p.done.load(Ordering::Relaxed) {
                self.job = None;
                if let Some(Err(e)) = p.result.lock().unwrap().take() {
                    self.sheet = Sheet::Message(e);
                }
                self.sel = None;
                self.reload();
            } else {
                ui.wake_after(Duration::from_millis(250));
            }
        }
        let modal = !matches!(self.sheet, Sheet::None);
        ui.set_enabled(!modal);
        self.top(ui);
        self.body(ui);
        ui.set_enabled(true);
        match &mut self.sheet {
            Sheet::None => {}
            Sheet::Message(m) => {
                let m = m.clone();
                if ui.alert(AlertKind::Note, &m, "", &["OK"]).is_some() {
                    self.sheet = Sheet::None;
                }
            }
            Sheet::Dest(fb) => {
                if let Some(c) = ui.file_sheet(fb) {
                    if let Choice::Picked(p) = c {
                        self.prefs.dest = Some(p);
                        self.prefs.save();
                        self.sel = None;
                        self.reload();
                    }
                    self.sheet = Sheet::None;
                }
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(tr!("Arquivo"), vec![MenuItem::new("now", tr!("Fazer Cópia Agora"), "Ctrl+B").enabled(self.prefs.dest.is_some() && self.job.is_none()), MenuItem::new("dest", tr!("Escolher Disco…"), "")])]
    }
}

/// `--auto`: a cópia de hora em hora, sem janela.
fn auto() {
    let prefs = Prefs::load();
    let Some(dest) = prefs.dest.clone().filter(|d| prefs.auto && d.is_dir()) else { return };
    if motor::snapshots(&dest).first().is_some_and(|s| SystemTime::now().duration_since(s.time).unwrap_or_default() < Duration::from_secs(55 * 60)) {
        return;
    }
    // Uma por vez (o Glove pode chamar de novo enquanto uma cópia longa roda).
    let lock = motor::root(&dest).join(".andamento.lock");
    let _ = std::fs::create_dir_all(motor::root(&dest));
    if std::fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        let old = lock.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e > Duration::from_secs(6 * 3600));
        if !old {
            return;
        }
    }
    let p = Progress::default();
    let r = motor::backup(&prefs, &p);
    let _ = std::fs::remove_file(&lock);
    match r {
        Ok(_) => notify(tr!("Cópia de segurança feita"), &tr!("{} arquivos copiados ({}).", p.copied.load(Ordering::Relaxed), human_size(p.bytes.load(Ordering::Relaxed)))),
        Err(e) => notify(tr!("A cópia de segurança não foi feita"), &e),
    }
}

pub fn main() {
    if std::env::args().any(|a| a == "--auto") {
        auto();
        return;
    }
    let opts = WindowOptions { title: tr!("Cópias de Segurança").into(), app_id: "glove.Copias".into(), size: (760, 500), fixed: false, min_size: Some((640, 380)) };
    if let Err(e) = run(opts, Copias::new()) {
        eprintln!("glove-copias: {e}");
    }
}
