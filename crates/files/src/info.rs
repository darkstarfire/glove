//! Obter Informações (Ctrl+I): uma janela por item, com seções que abrem e
//! fecham — Geral, Nome e extensão, Abrir com, Permissões e Comentários.
//! A janela muda de altura conforme as seções abertas.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, SystemTime};

use glove_kit::{paint::icons, App, Rect, Ui};

const W: f32 = 300.0;
const LW: f32 = 92.0;
/// Atributo estendido padrão do freedesktop para comentários.
const COMMENT_ATTR: &str = "user.xdg.comment";
const ACCESS: [&str; 3] = ["Ler e escrever", "Só ler", "Nenhum acesso"];

pub struct Info {
    path: PathBuf,
    name: String,
    open: [bool; 5],
    size: Option<u64>,
    size_rx: Option<Receiver<(u64, usize)>>,
    items: usize,
    mime: String,
    /// Apps que abrem este tipo: (id do .desktop, nome).
    apps: Vec<(String, String)>,
    app: usize,
    default_app: usize,
    comment: String,
    message: Option<String>,
}

/// Linha de um comando (ex.: xdg-mime), sem a quebra de linha.
fn run(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// Apps instalados que abrem `mime` (pelo MimeType dos .desktop).
fn apps_for(mime: &str) -> Vec<(String, String)> {
    let mut dirs = vec![std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/applications")).unwrap_or_default()];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    let mut out: Vec<(String, String)> = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id.ends_with(".desktop") || out.iter().any(|(i, _)| *i == id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            let mut name = None;
            let mut name_pt = None;
            let mut mimes = "";
            let mut hidden = false;
            let mut in_entry = false;
            for l in text.lines() {
                if l.starts_with('[') {
                    in_entry = l == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = l.strip_prefix("Name=") { name.get_or_insert(v); }
                if let Some(v) = l.strip_prefix("Name[pt_BR]=") { name_pt = Some(v); }
                if let Some(v) = l.strip_prefix("MimeType=") { mimes = v; }
                if l == "NoDisplay=true" || l == "Hidden=true" { hidden = true; }
            }
            if !hidden && mimes.split(';').any(|m| m == mime) {
                if let Some(n) = name_pt.or(name) {
                    out.push((id, n.to_string()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out
}

/// Tamanho (e quantidade de itens) de uma pasta, somado numa thread.
fn tree_size(p: &Path, n: &mut usize) -> u64 {
    *n += 1;
    if *n > 300_000 {
        return 0;
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| tree_size(&e.path(), n)).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

fn get_comment(p: &Path) -> String {
    use std::ffi::CString;
    let (Ok(path), Ok(attr)) = (CString::new(p.as_os_str().as_encoded_bytes()), CString::new(COMMENT_ATTR)) else { return String::new() };
    let mut buf = vec![0u8; 4096];
    let n = unsafe { libc::getxattr(path.as_ptr(), attr.as_ptr(), buf.as_mut_ptr() as *mut _, buf.len()) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

fn set_comment(p: &Path, text: &str) -> Result<(), String> {
    use std::ffi::CString;
    let path = CString::new(p.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    let attr = CString::new(COMMENT_ATTR).unwrap();
    let r = unsafe {
        if text.is_empty() {
            let r = libc::removexattr(path.as_ptr(), attr.as_ptr());
            if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA) { 0 } else { r }
        } else {
            libc::setxattr(path.as_ptr(), attr.as_ptr(), text.as_ptr() as *const _, text.len(), 0)
        }
    };
    if r == 0 { Ok(()) } else { Err(format!("Este disco não guarda comentários ({})", std::io::Error::last_os_error())) }
}

/// Nome do usuário ou grupo pelo número (/etc/passwd, /etc/group).
fn name_of(file: &str, id: u32) -> String {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| t.lines().find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() > 2 && f[2].parse() == Ok(id)).then(|| f[0].to_string())
        }))
        .unwrap_or_else(|| id.to_string())
}

/// "05/11/2026 às 00:27".
fn when(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "--".into() };
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        format!("{:02}/{:02}/{} às {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_year + 1900, tm.tm_hour, tm.tm_min)
    }
}

/// Acesso (0 ler e escrever, 1 só ler, 2 nenhum) pelos bits rw de uma classe.
fn access(mode: u32, shift: u32) -> usize {
    let bits = (mode >> shift) & 0o6;
    if bits & 0o4 == 0 { 2 } else if bits & 0o2 != 0 { 0 } else { 1 }
}

fn with_access(mode: u32, shift: u32, a: usize) -> u32 {
    let bits = match a { 0 => 0o6, 1 => 0o4, _ => 0 };
    (mode & !(0o6 << shift)) | (bits << shift)
}

impl Info {
    pub fn new(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
        let is_dir = path.is_dir();
        let mime = if is_dir { "inode/directory".to_string() } else { run("xdg-mime", &["query", "filetype", &path.to_string_lossy()]) };
        let apps = if is_dir { Vec::new() } else { apps_for(&mime) };
        let def = run("xdg-mime", &["query", "default", &mime]);
        let default_app = apps.iter().position(|(id, _)| *id == def).unwrap_or(0);
        let (size, size_rx) = if is_dir {
            let (tx, rx) = channel();
            let p = path.clone();
            std::thread::spawn(move || {
                let mut n = 0;
                let s = tree_size(&p, &mut n);
                let _ = tx.send((s, n.saturating_sub(1)));
            });
            (None, Some(rx))
        } else {
            (std::fs::metadata(&path).ok().map(|m| m.len()), None)
        };
        let comment = get_comment(&path);
        Self { name, open: [true, false, false, false, false], size, size_rx, items: 0, mime, apps, app: default_app, default_app, comment, message: None, path }
    }

    fn rename(&mut self) {
        let new = self.name.trim().to_string();
        let old = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if new.is_empty() || new == old {
            return;
        }
        if new.contains('/') {
            self.message = Some("O nome não pode ter “/”.".into());
            return;
        }
        let dest = self.path.with_file_name(&new);
        if dest.exists() {
            self.message = Some(format!("Já existe um item chamado “{new}”."));
            return;
        }
        match std::fs::rename(&self.path, &dest) {
            Ok(()) => {
                self.path = dest;
                self.message = None;
            }
            Err(e) => self.message = Some(format!("Não foi possível renomear: {e}")),
        }
    }
}

impl App for Info {
    fn ui(&mut self, ui: &mut Ui) {
        if let Some(rx) = &self.size_rx {
            match rx.try_recv() {
                Ok((s, n)) => {
                    self.size = Some(s);
                    self.items = n;
                    self.size_rx = None;
                }
                Err(_) => ui.wake_after(Duration::from_millis(150)),
            }
        }
        let meta = std::fs::symlink_metadata(&self.path).ok();
        let is_dir = self.path.is_dir();
        let size_text = match self.size {
            Some(s) if is_dir => format!("{} em {} itens", super::human_size(s), self.items),
            Some(s) => format!("{} ({} bytes)", super::human_size(s), s),
            None => "Calculando…".into(),
        };
        let title = format!("Informações de {}", self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into()));
        ui.set_title(&title);

        let w = ui.c.w;
        let (_, h) = ui.area_size(Rect::new(14.0, 12.0, w - 28.0, 2000.0), |ui| {
            // Cabeçalho: ícone, nome e tamanho.
            let r = ui.alloc(w - 28.0, 52.0);
            let kind = super::kind_of(&self.path, is_dir);
            let e = super::Entry { name: self.name.clone(), path: self.path.clone(), kind, size: 0, modified: None, link: meta.as_ref().is_some_and(|m| m.file_type().is_symlink()), app_icon: None };
            let icon = e.icon();
            icons::draw(ui.c, ui.fonts, icon, r.x, r.y, 48.0, None);
            if e.link {
                icons::alias_badge(ui.c, r.x, r.y, 48.0);
            }
            let name = ui.fonts.ellipsize(&self.name, 13.0, true, w - 100.0);
            ui.fonts.draw(ui.c, &name, r.x + 58.0, r.y + 22.0, 13.0, true, glove_kit::paint::draw::rgba(20, 20, 20, 255));
            ui.fonts.draw(ui.c, &size_text, r.x + 58.0, r.y + 40.0, 11.0, false, glove_kit::paint::draw::rgba(80, 80, 80, 255));
            ui.separator();

            if ui.disclosure(&mut self.open[0], "Geral:") {}
            if self.open[0] {
                ui.form_row("Tipo:", LW, |ui| ui.small(e.type_name()));
                ui.form_row("Tamanho:", LW, |ui| ui.small(&size_text));
                let parent = self.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
                ui.form_row("Onde:", LW, |ui| {
                    let t = ui.fonts.ellipsize(&parent, 11.0, false, w - 28.0 - LW - 12.0);
                    ui.small(&t);
                });
                let full = std::fs::metadata(&self.path).ok();
                ui.form_row("Criado:", LW, |ui| ui.small(&when(full.as_ref().and_then(|m| m.created().ok()))));
                ui.form_row("Modificado:", LW, |ui| ui.small(&when(full.as_ref().and_then(|m| m.modified().ok()))));
                if e.link {
                    let target = std::fs::read_link(&self.path).map(|t| t.display().to_string()).unwrap_or_default();
                    ui.form_row("Atalho para:", LW, |ui| {
                        let t = ui.fonts.ellipsize(&target, 11.0, false, w - 28.0 - LW - 12.0);
                        ui.small(&t);
                    });
                }
            }

            ui.disclosure(&mut self.open[1], "Nome e extensão:");
            if self.open[1] {
                let mut name = self.name.clone();
                ui.text_field(&mut name, w - 28.0);
                self.name = name;
                if ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Enter) {
                    self.rename();
                }
            }

            if !is_dir {
                ui.disclosure(&mut self.open[2], "Abrir com:");
                if self.open[2] {
                    if self.apps.is_empty() {
                        ui.small("Nenhum app instalado abre este tipo.");
                    } else {
                        let names: Vec<String> = self.apps.iter().map(|a| a.1.clone()).collect();
                        let mut i = self.app;
                        ui.popup(&mut i, &names, w - 28.0);
                        self.app = i;
                        ui.small(&format!("Tipo: {}", self.mime));
                        if self.app != self.default_app && ui.small_button("Usar para todos deste tipo") {
                            let id = self.apps[self.app].0.clone();
                            run("xdg-mime", &["default", &id, &self.mime]);
                            self.default_app = self.app;
                        }
                    }
                }
            }

            ui.disclosure(&mut self.open[3], "Permissões:");
            if self.open[3] {
                if let Some(m) = &meta {
                    let mode = m.mode();
                    let me = unsafe { libc::getuid() };
                    let mine = m.uid() == me;
                    let you = if !mine { "Só o dono pode mudar as permissões." } else { "" };
                    let mut changed = None;
                    let owner = format!("{}:", name_of("/etc/passwd", m.uid()));
                    let group = format!("Grupo {}:", name_of("/etc/group", m.gid()));
                    for (shown, shift) in [(owner, 6u32), (group, 3), ("Outros:".to_string(), 0)] {
                        let mut a = access(mode, shift);
                        let opts: Vec<String> = ACCESS.iter().map(|s| s.to_string()).collect();
                        ui.form_row(&shown, LW, |ui| {
                            ui.set_enabled(mine);
                            if ui.popup(&mut a, &opts, w - 28.0 - LW - 12.0) {
                                changed = Some((shift, a));
                            }
                            ui.set_enabled(true);
                        });
                    }
                    if !you.is_empty() {
                        ui.small(you);
                    }
                    if let Some((shift, a)) = changed {
                        let new = with_access(mode, shift, a);
                        if let Err(e) = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(new & 0o7777)) {
                            self.message = Some(format!("Não foi possível mudar: {e}"));
                        }
                    }
                }
            }

            ui.disclosure(&mut self.open[4], "Comentários:");
            if self.open[4] {
                let mut c = self.comment.clone();
                if ui.text_field(&mut c, w - 28.0) {
                    if let Err(e) = set_comment(&self.path, &c) {
                        self.message = Some(e);
                    }
                    self.comment = c;
                }
            }
            if let Some(m) = self.message.clone() {
                ui.space(4.0);
                ui.label_ex(&m, 11.0, true, glove_kit::paint::draw::rgba(190, 40, 40, 255));
            }
        });
        // A janela acompanha as seções abertas.
        ui.set_window_size(W, (h + 26.0).max(120.0), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissoes() {
        assert_eq!(access(0o644, 6), 0);
        assert_eq!(access(0o644, 3), 1);
        assert_eq!(access(0o640, 0), 2);
        assert_eq!(with_access(0o644, 0, 2), 0o640);
        assert_eq!(with_access(0o755, 3, 0), 0o775);
    }

    #[test]
    fn comentario() {
        let p = std::env::temp_dir().join(format!("glove-info-{}", std::process::id()));
        std::fs::write(&p, "x").unwrap();
        // Alguns sistemas de arquivos (tmpfs antigo) não guardam atributos: só testa se der.
        if set_comment(&p, "lembrete").is_ok() {
            assert_eq!(get_comment(&p), "lembrete");
            set_comment(&p, "").unwrap();
            assert_eq!(get_comment(&p), "");
        }
        std::fs::remove_file(&p).unwrap();
    }
}
