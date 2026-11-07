//! Rede: "Conectar ao Servidor" (Ir ▸ Conectar ao Servidor…, Ctrl+K) e a pasta
//! Rede da barra lateral. Monta pelo gvfs (`gio mount`), sem root: smb, sftp,
//! ftp, webdav… O Glove conversa com o `gio mount` e responde às perguntas dele
//! (usuário, domínio, senha) com a própria janela de login, como no Tiger.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{paint::draw::*, run, App, Rect, Ui, WindowOptions};

/// Pasta virtual da Rede na barra lateral.
pub const ROOT: &str = "/:Rede";

pub fn is_rede(p: &std::path::Path) -> bool {
    p.starts_with(ROOT)
}

/// Endereço guardado no caminho de um item da Rede ("/:Rede/smb://casa/").
pub fn uri_of(p: &std::path::Path) -> Option<String> {
    p.to_str()?.strip_prefix(ROOT)?.strip_prefix('/').map(str::to_string).filter(|u| !u.is_empty())
}

/// Computadores e compartilhamentos vistos na rede (gvfs: network:///): (nome, endereço).
pub fn scan() -> Vec<(String, String)> {
    let o = std::process::Command::new("gio")
        .args(["list", "-a", "standard::display-name,standard::target-uri", "network:///"])
        .env("LC_ALL", "C")
        .output();
    let Ok(o) = o else { return Vec::new() };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut v = Vec::new();
    for l in text.lines() {
        let attr = |k: &str| {
            let i = l.find(k)? + k.len();
            let rest = &l[i..];
            Some(rest.split(" standard::").next().unwrap_or(rest).trim().to_string())
        };
        if let (Some(name), Some(uri)) = (attr("standard::display-name="), attr("standard::target-uri=")) {
            if !uri.is_empty() {
                v.push((name, uri));
            }
        }
    }
    v.sort_by_key(|a| a.0.to_lowercase());
    v
}

#[derive(Clone, Default)]
pub struct Creds {
    pub user: String,
    pub domain: String,
    pub password: String,
    /// Entrar como convidado (sem senha).
    pub guest: bool,
}

pub enum Mounted {
    Ok(PathBuf),
    /// Pediu senha (e a dada, se houve, foi recusada).
    NeedAuth { wrong: bool },
    Err(String),
}

/// Pasta local (gvfs) de um endereço montado.
fn local_path(uri: &str) -> Option<PathBuf> {
    let o = std::process::Command::new("gio").args(["info", uri]).env("LC_ALL", "C").output().ok()?;
    String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.trim().strip_prefix("local path:").map(|p| PathBuf::from(p.trim())))
}

/// Uma tentativa de `gio mount` com as respostas já escritas na entrada (uma por
/// pergunta, na ordem em que o gio pergunta). Devolve (deu certo, saída, erro).
fn run_mount(uri: &str, guest: bool, input: &str) -> Option<(bool, String, String)> {
    let mut cmd = std::process::Command::new("gio");
    cmd.arg("mount");
    if guest {
        cmd.arg("-a");
    }
    cmd.arg(uri).env("LC_ALL", "C").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    if let Some(mut w) = child.stdin.take() {
        let _ = w.write_all(input.as_bytes());
        // Fechar a entrada: se faltar resposta, o gio desiste em vez de esperar.
    }
    let o = child.wait_with_output().ok()?;
    Some((o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()))
}

/// Monta `uri`, respondendo às perguntas do `gio mount` com `creds`.
pub fn mount(uri: &str, creds: Option<&Creds>) -> Mounted {
    if let Some(p) = local_path(uri).filter(|p| p.exists()) {
        return Mounted::Ok(p);
    }
    let guest = creds.is_some_and(|c| c.guest);
    // sftp: o nome vai no endereço (sftp://ana@srv) e o gio só pergunta a senha.
    let sftp = uri.starts_with("sftp://") || uri.starts_with("ssh://");
    let uri: String = match creds {
        Some(c) if sftp && !guest && !c.user.is_empty() && !uri.contains('@') => uri.replacen("://", &format!("://{}@", c.user), 1),
        _ => uri.to_string(),
    };
    let input = match creds {
        Some(c) if !guest => {
            let mut v = Vec::new();
            if !sftp {
                v.push(c.user.clone());
                if uri.starts_with("smb://") {
                    v.push(c.domain.clone());
                }
            }
            v.push(c.password.clone());
            v.iter().map(|l| format!("{l}\n")).collect::<String>()
        }
        _ => String::new(),
    };
    let Some((mut ok, mut out, mut err)) = run_mount(&uri, guest, &input) else { return Mounted::Err("Não foi possível usar o gio (gvfs).".into()) };
    // "Não dá para conferir a identidade do computador" (sftp na primeira vez): entra mesmo assim.
    if !ok && out.to_lowercase().contains("choice") {
        if let Some(r) = run_mount(&uri, guest, &format!("1\n{input}")) {
            (ok, out, err) = r;
        }
    }
    if ok || err.to_lowercase().contains("already mounted") {
        return match local_path(&uri) {
            Some(p) => Mounted::Ok(p),
            None => Mounted::Err("O servidor conectou, mas a pasta dele não apareceu.".into()),
        };
    }
    // Pediu a senha: sem senha dada, ou perguntou de novo (a dada foi recusada).
    // Pediu login: sem respostas dadas, ou perguntou de novo (as dadas foram recusadas).
    let asks = out.matches("User").count().max(out.matches("Password").count());
    if asks > 0 && (input.is_empty() || asks > 1) {
        return Mounted::NeedAuth { wrong: !input.is_empty() };
    }
    let msg = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let msg = msg.rsplit(": ").next().unwrap_or(msg).to_string();
    let low = msg.to_lowercase();
    if low.contains("permission") || low.contains("login") || low.contains("password") || low.contains("authentic") || low.contains("cancel") {
        return Mounted::NeedAuth { wrong: !input.is_empty() };
    }
    Mounted::Err(if msg.is_empty() { "Não foi possível conectar ao servidor.".into() } else { msg })
}

/// "smb://casa.local/fotos" → "casa.local".
fn host_of(uri: &str) -> String {
    let rest = uri.split_once("://").map(|x| x.1).unwrap_or(uri);
    let rest = rest.rsplit_once('@').map(|x| x.1).unwrap_or(rest);
    rest.split(['/', ':']).next().unwrap_or(rest).to_string()
}

/// "casa/fotos" → "smb://casa/fotos" (sem esquema, vale smb, como no Mac).
pub fn normalize(addr: &str) -> String {
    let a = addr.trim();
    if a.contains("://") { a.to_string() } else { format!("smb://{a}") }
}

enum Stage {
    Form,
    Connecting(Arc<Mutex<Option<Mounted>>>),
    Login { creds: Creds, wrong: bool, first: bool },
}

pub fn connect_window(addr: Option<String>, auto: bool) {
    let cfg = glove_config::Config::load();
    let addr = addr.or_else(|| cfg.files.recent_servers.first().cloned()).unwrap_or_default();
    let mut app = Connect { config: cfg, addr, fav: None, stage: Stage::Form, message: None, first: true, creds: None, repaint: 0 };
    if auto && !app.addr.is_empty() {
        app.start(None);
    }
    let opts = WindowOptions { title: "Conectar ao Servidor".into(), app_id: "glove.Files".into(), size: (480, 270), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-files: {e}");
    }
}

struct Connect {
    config: glove_config::Config,
    addr: String,
    fav: Option<usize>,
    stage: Stage,
    message: Option<String>,
    first: bool,
    creds: Option<Creds>,
    /// Quadros que ainda redesenham a janela inteira (a etapa acabou de mudar).
    repaint: u8,
}

impl Connect {
    fn start(&mut self, creds: Option<Creds>) {
        let uri = normalize(&self.addr);
        self.addr = uri.clone();
        self.message = None;
        self.creds = creds.clone();
        let slot = Arc::new(Mutex::new(None));
        let s2 = slot.clone();
        std::thread::spawn(move || {
            let r = mount(&uri, creds.as_ref());
            *s2.lock().unwrap() = Some(r);
        });
        self.stage = Stage::Connecting(slot);
        self.repaint = 3;
    }

    fn remember(&mut self) {
        let f = &mut self.config.files;
        f.recent_servers.retain(|s| *s != self.addr);
        f.recent_servers.insert(0, self.addr.clone());
        f.recent_servers.truncate(10);
        let _ = self.config.save();
    }
}

impl App for Connect {
    fn tick(&self) -> Option<Duration> {
        matches!(self.stage, Stage::Connecting(_)).then(|| Duration::from_millis(100))
    }

    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        // A etapa mudou sem evento (resposta da thread): refaz a janela toda, não só a barra.
        if self.repaint > 0 {
            self.repaint -= 1;
            ui.redraw = true;
        }
        // Conectando: espera a resposta da thread.
        if let Stage::Connecting(slot) = &self.stage {
            let done = slot.lock().unwrap().take();
            match done {
                Some(Mounted::Ok(p)) => {
                    self.remember();
                    let _ = super::launch(std::process::Command::new(std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).unwrap_or_else(|| "glove-files".into())).arg(p));
                    ui.close = true;
                    return;
                }
                Some(Mounted::NeedAuth { wrong }) => {
                    let user = self.creds.as_ref().map(|c| c.user.clone()).unwrap_or_else(|| std::env::var("USER").unwrap_or_default());
                    self.stage = Stage::Login { creds: Creds { user, domain: "WORKGROUP".into(), ..Default::default() }, wrong, first: true };
                    self.repaint = 3;
                }
                Some(Mounted::Err(e)) => {
                    self.message = Some(e);
                    self.stage = Stage::Form;
                    self.repaint = 3;
                }
                None => {
                    ui.fonts.draw(ui.c, &format!("Conectando a “{}”…", host_of(&self.addr)), 20.0, 40.0, 13.0, true, rgba(20, 20, 20, 255));
                    ui.progress_in(Rect::new(20.0, 56.0, w - 40.0, 12.0), None);
                    ui.area(Rect::new(w - 110.0, h - 40.0, 92.0, 24.0), |ui| {
                        if ui.button("Cancelar") {
                            self.stage = Stage::Form;
                        }
                    });
                    return;
                }
            }
        }
        // Folha de login (como a do Tiger para servidores).
        if let Stage::Login { creds, wrong, first } = &mut self.stage {
            let smb = self.addr.starts_with("smb://");
            ui.fonts.draw(ui.c, &format!("Digite seu nome e senha para o servidor “{}”.", host_of(&self.addr)), 20.0, 30.0, 13.0, true, rgba(20, 20, 20, 255));
            if *wrong {
                ui.fonts.draw(ui.c, "O nome ou a senha não foram aceitos. Tente de novo.", 20.0, 50.0, 11.0, false, rgba(170, 30, 30, 255));
            }
            let mut go = false;
            let mut cancel = false;
            ui.area(Rect::new(20.0, 62.0, w - 40.0, h - 70.0), |ui| {
                let mut kind = creds.guest as usize;
                ui.horizontal(|ui| {
                    ui.label("Conectar como:");
                    ui.spacing(10.0);
                    if ui.radio_group(&mut kind, &["Usuário registrado", "Convidado"]) {
                        creds.guest = kind == 1;
                    }
                });
                ui.space(8.0);
                ui.set_enabled(!creds.guest);
                let lw = 80.0;
                let row = |ui: &mut Ui, label: &str| {
                    let (x, y) = ui.cursor();
                    ui.fonts.draw(ui.c, label, x + lw - ui.text_width(label, 12.0, false), y + 15.0, 12.0, false, rgba(20, 20, 20, 255));
                    Rect::new(x + lw + 8.0, y, 240.0, 22.0)
                };
                let r = row(ui, "Nome:");
                go |= ui.text_field_in(r, &mut creds.user, *first && !creds.guest).1;
                ui.alloc(10.0, 28.0);
                let r = row(ui, "Senha:");
                go |= ui.password_field_in(r, &mut creds.password, false);
                ui.alloc(10.0, 28.0);
                if smb {
                    let r = row(ui, "Domínio:");
                    go |= ui.text_field_in(r, &mut creds.domain, false).1;
                    ui.alloc(10.0, 28.0);
                }
                ui.set_enabled(true);
            });
            *first = false;
            ui.area(Rect::new(w - 220.0, h - 40.0, 200.0, 24.0), |ui| {
                ui.horizontal(|ui| {
                    cancel = ui.button("Cancelar");
                    ui.spacing(10.0);
                    go |= ui.default_button("Conectar");
                });
            });
            if cancel {
                self.stage = Stage::Form;
            } else if go {
                let c = creds.clone();
                self.start(Some(c));
            }
            return;
        }
        // Formulário: endereço, favoritos e botões.
        ui.fonts.draw(ui.c, "Endereço do servidor:", 20.0, 28.0, 12.0, true, rgba(20, 20, 20, 255));
        let mut connect = false;
        let (_, enter) = ui.text_field_in(Rect::new(20.0, 36.0, w - 80.0, 22.0), &mut self.addr, self.first);
        connect |= enter;
        self.first = false;
        // "+": guarda nos favoritos.
        ui.area(Rect::new(w - 52.0, 35.0, 32.0, 24.0), |ui| {
            if ui.small_button("+") && !self.addr.trim().is_empty() {
                let a = normalize(&self.addr);
                if !self.config.files.servers.contains(&a) {
                    self.config.files.servers.push(a);
                    let _ = self.config.save();
                }
            }
        });
        ui.fonts.draw(ui.c, "Servidores favoritos:", 20.0, 80.0, 12.0, true, rgba(20, 20, 20, 255));
        let mut rows: Vec<String> = self.config.files.servers.clone();
        // Os últimos usados, depois dos favoritos (em cinza não dá: vão com "↻").
        for r in &self.config.files.recent_servers {
            if !rows.contains(r) {
                rows.push(format!("↻ {r}"));
            }
        }
        let before = self.fav;
        ui.area(Rect::new(20.0, 88.0, w - 40.0, 110.0), |ui| {
            ui.list(&mut self.fav, &rows, w - 40.0, 110.0);
        });
        if self.fav != before {
            if let Some(r) = self.fav.and_then(|i| rows.get(i)) {
                self.addr = r.trim_start_matches("↻ ").to_string();
            }
        }
        if let Some(m) = &self.message {
            let m = ui.fonts.ellipsize(m, 11.0, false, w - 40.0);
            ui.fonts.draw(ui.c, &m, 20.0, 214.0, 11.0, false, rgba(170, 30, 30, 255));
        }
        ui.area(Rect::new(20.0, h - 40.0, w - 40.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                let fav = self.fav.filter(|&i| i < self.config.files.servers.len());
                ui.set_enabled(fav.is_some());
                if ui.button("Remover") {
                    if let Some(i) = fav {
                        self.config.files.servers.remove(i);
                        self.fav = None;
                        let _ = self.config.save();
                    }
                }
                ui.set_enabled(true);
                ui.spacing(w - 40.0 - 90.0 - 200.0);
                if ui.button("Navegar") {
                    let _ = super::launch(std::process::Command::new(std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).unwrap_or_else(|| "glove-files".into())).arg(ROOT));
                    ui.close = true;
                }
                ui.spacing(10.0);
                connect |= ui.default_button("Conectar");
            });
        });
        if connect && !self.addr.trim().is_empty() {
            self.start(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos() {
        assert_eq!(normalize("casa/fotos"), "smb://casa/fotos");
        assert_eq!(normalize("sftp://ana@srv"), "sftp://ana@srv");
        assert_eq!(host_of("sftp://ana@srv.local:22/home"), "srv.local");
        assert_eq!(uri_of(std::path::Path::new("/:Rede/smb://casa/")), Some("smb://casa/".into()));
    }
}
