//! Painel Contas: usuários (AccountsService), nome, senha (pelo `passwd` num
//! pseudoterminal), imagem e os Itens de Início. Cada um só muda a própria conta.

use glove_config::tr;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{chrome, draw::*, Color, Pixmap},
    Choice, FileBrowser, Rect, Ui,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    path: String,
    pub login: String,
    pub real: String,
    pub icon: String,
    pub admin: bool,
    pub me: bool,
}

/// `s "Fabio Carneiro"` → `Fabio Carneiro` (saída do busctl get-property).
pub fn busctl_string(out: &str) -> String {
    let t = out.trim();
    let t = t.strip_prefix("s ").unwrap_or(t).trim();
    t.trim_matches('"').replace("\\\"", "\"")
}

/// `ao 2 "/a" "/b"` → ["/a", "/b"].
pub fn busctl_paths(out: &str) -> Vec<String> {
    out.split('"').enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, s)| s.to_string()).collect()
}

fn prop(path: &str, name: &str) -> String {
    Command::new("busctl")
        .args(["get-property", "org.freedesktop.Accounts", path, "org.freedesktop.Accounts.User", name])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| busctl_string(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Administrador: num grupo de administradores ou, para a própria conta, aceito
/// pelo sudo (a conta pode estar no sudoers pelo nome).
fn is_admin(login: &str, me: &str) -> bool {
    (login == me && crate::admin::sudo_state() != crate::admin::Sudo::No)
        || Command::new("id").args(["-nG", login]).output().ok().is_some_and(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().any(|g| matches!(g, "sudo" | "wheel" | "admin")))
}

/// Usuários do AccountsService; sem ele, os de /etc/passwd (UID 1000 ou mais).
fn accounts() -> Vec<Account> {
    let me = std::env::var("USER").unwrap_or_default();
    let list = Command::new("busctl").args(["call", "org.freedesktop.Accounts", "/org/freedesktop/Accounts", "org.freedesktop.Accounts", "ListCachedUsers"]).output().ok().filter(|o| o.status.success());
    let mut out: Vec<Account> = match list {
        Some(o) => busctl_paths(&String::from_utf8_lossy(&o.stdout))
            .into_iter()
            .map(|p| {
                let login = prop(&p, "UserName");
                Account { real: prop(&p, "RealName"), icon: prop(&p, "IconFile"), admin: is_admin(&login, &me), me: login == me, login, path: p }
            })
            .collect(),
        None => std::fs::read_to_string("/etc/passwd")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                let uid: u32 = f.get(2)?.parse().ok()?;
                ((1000..60000).contains(&uid)).then(|| Account { login: f[0].into(), real: f[4].split(',').next().unwrap_or("").into(), admin: is_admin(f[0], &me), me: f[0] == me, ..Default::default() })
            })
            .collect(),
    };
    // A própria conta primeiro (como no 10.4: "Minha Conta").
    out.sort_by_key(|a| (!a.me, a.real.to_lowercase()));
    out
}

/// Troca a senha pelo `passwd`, respondendo às perguntas dele num pseudoterminal.
pub fn change_password(old: &str, new: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    unsafe {
        let (mut master, mut slave) = (0, 0);
        if libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) != 0 {
            return Err(tr!("não foi possível abrir um terminal para o passwd").into());
        }
        let pid = libc::fork();
        if pid < 0 {
            return Err(tr!("não foi possível abrir o passwd").into());
        }
        if pid == 0 {
            // Filho: o terminal novo vira o dele, em inglês (as perguntas são previsíveis).
            libc::setsid();
            libc::ioctl(slave, libc::TIOCSCTTY, 0);
            libc::dup2(slave, 0);
            libc::dup2(slave, 1);
            libc::dup2(slave, 2);
            libc::close(master);
            let prog = c"passwd";
            let lc = c"LC_ALL=C";
            let env = [lc.as_ptr(), std::ptr::null()];
            let argv = [prog.as_ptr(), std::ptr::null()];
            libc::execvpe(prog.as_ptr(), argv.as_ptr(), env.as_ptr());
            libc::_exit(127);
        }
        libc::close(slave);
        let mut f = std::fs::File::from_raw_fd(master);
        let mut seen = String::new();
        let mut answered = 0;
        let started = std::time::Instant::now();
        let mut buf = [0u8; 512];
        loop {
            let mut pfd = libc::pollfd { fd: master, events: libc::POLLIN, revents: 0 };
            if libc::poll(&mut pfd, 1, 200) > 0 {
                match f.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => seen.push_str(&String::from_utf8_lossy(&buf[..n])),
                }
            }
            // Cada "password:" pedido recebe a próxima resposta (atual, nova, nova).
            let asks = seen.to_lowercase().matches("password:").count();
            while answered < asks && answered < 3 {
                // Usuário comum: "Current password", "New password", "Retype new password".
                let answer = if answered == 0 { old } else { new };
                let _ = f.write_all(format!("{answer}\n").as_bytes());
                answered += 1;
            }
            let mut status = 0;
            if libc::waitpid(pid, &mut status, libc::WNOHANG) == pid {
                return if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 {
                    Ok(())
                } else {
                    let low = seen.to_lowercase();
                    Err(if low.contains("authentication") || low.contains("incorrect") {
                        tr!("a senha atual não confere.").into()
                    } else {
                        seen.lines().rev().find(|l| !l.trim().is_empty() && !l.to_lowercase().contains("password:")).unwrap_or(tr!("o passwd recusou a senha nova")).trim().trim_start_matches("passwd: ").to_string()
                    })
                };
            }
            if started.elapsed() > Duration::from_secs(20) {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
                return Err(tr!("o passwd não respondeu.").into());
            }
        }
        let mut status = 0;
        libc::waitpid(pid, &mut status, 0);
        if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 { Ok(()) } else { Err(tr!("a senha atual não confere.").into()) }
    }
}

#[derive(Default)]
pub struct Contas {
    users: Vec<Account>,
    sel: Option<usize>,
    pub tab: usize,
    name: String,
    photos: HashMap<String, Option<Pixmap>>,
    /// Folha da senha: (atual, nova, confirmação).
    pw: Option<[String; 3]>,
    file: Option<FileBrowser>,
    busy: bool,
    done: Arc<Mutex<Option<Result<String, String>>>>,
    message: Option<(bool, String)>,
}

impl Contas {
    pub fn refresh(&mut self) {
        self.users = accounts();
        if self.sel.is_none_or(|s| s >= self.users.len()) {
            self.sel = (!self.users.is_empty()).then_some(0);
        }
        self.name = self.current().map(|u| u.real.clone()).unwrap_or_default();
        self.photos.clear();
    }

    fn current(&self) -> Option<&Account> {
        self.sel.and_then(|i| self.users.get(i))
    }

    pub fn sheet_open(&self) -> bool {
        self.pw.is_some() || self.file.is_some()
    }

    fn start(&mut self, ok: String, job: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.busy = true;
        self.message = None;
        let done = self.done.clone();
        std::thread::spawn(move || {
            let r = job().map(|_| ok);
            *done.lock().unwrap() = Some(r);
        });
    }

    fn photo(&mut self, icon: &str) -> Option<Pixmap> {
        self.photos.entry(icon.to_string()).or_insert_with(|| Pixmap::load_png(icon).ok()).clone()
    }

    fn draw_picture(&mut self, ui: &mut Ui, u: &Account, x: f32, y: f32, s: f32) {
        match self.photo(&u.icon) {
            Some(p) => {
                let sc = ui.c.scale;
                let k = s * sc / p.width().max(p.height()) as f32;
                let t = glove_ui::paint::tiny_skia::Transform::from_scale(k, k).post_translate(x * sc, y * sc);
                ui.c.draw_pixmap_t(p.as_ref(), &glove_ui::paint::tiny_skia::PixmapPaint { quality: glove_ui::paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t);
                ui.c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, 3.0), &solid(rgba(0, 0, 0, 90)), 1.0);
            }
            None => chrome::avatar(ui.c, x, y, s, chrome::avatar_seed(&u.login)),
        }
    }

    /// Lista de contas à esquerda; abas à direita. `inicio`: a aba Itens de Início.
    pub fn ui(&mut self, ui: &mut Ui, inicio: &mut crate::inicio::Inicio) {
        let finished = self.done.lock().unwrap().take();
        if let Some(r) = finished {
            self.busy = false;
            self.message = Some(match r {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            });
            self.refresh();
        }
        if self.busy {
            ui.wake_after(Duration::from_millis(200));
        }
        let (x0, y0) = ui.cursor();
        let w = ui.available_width();
        // Lista: "Minha Conta" e "Outras Contas".
        let lw = 170.0;
        // A aba Itens de Início cresce com as listas (a janela acompanha).
        // Janela aumentada: a lista de contas e a caixa crescem juntas (e, na aba Itens
        // de Início, a lista dela).
        let g = crate::grow();
        let lh = if self.tab == 2 { 330f32.max(inicio.used_height() + 70.0) } else { 330.0 } + g;
        ui.c.fill_rect(x0, y0, lw, lh, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(x0 + 1.0, y0 + 1.0, lw - 2.0, lh - 2.0, &solid(Color::WHITE));
        let mut y = y0 + 4.0;
        let mut clicked = None;
        let users = self.users.clone();
        for (i, u) in users.iter().enumerate() {
            let header = if i == 0 && u.me { Some(tr!("Minha Conta")) } else if (i == 0 && !u.me) || (i == 1 && users[0].me) { Some(tr!("Outras Contas")) } else { None };
            if let Some(h) = header {
                ui.fonts.draw(ui.c, h, x0 + 8.0, y + 12.0, 10.0, true, rgba(110, 110, 118, 255));
                y += 18.0;
            }
            let r = Rect::new(x0 + 1.0, y, lw - 2.0, 40.0);
            if self.sel == Some(i) {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(ui.theme.accent));
            }
            self.draw_picture(ui, u, r.x + 6.0, r.y + 4.0, 32.0);
            let ink = if self.sel == Some(i) { Color::WHITE } else { rgba(20, 20, 20, 255) };
            let name = ui.fonts.ellipsize(if u.real.is_empty() { &u.login } else { &u.real }, 12.0, true, lw - 52.0);
            ui.fonts.draw(ui.c, &name, r.x + 44.0, r.y + 17.0, 12.0, true, ink);
            let kind = if u.admin { tr!("Administrador") } else { tr!("Padrão") };
            ui.fonts.draw(ui.c, kind, r.x + 44.0, r.y + 32.0, 10.0, false, if self.sel == Some(i) { Color::WHITE } else { rgba(100, 100, 106, 255) });
            if ui.hit(r).clicked {
                clicked = Some(i);
            }
            y += 42.0;
        }
        if let Some(i) = clicked {
            self.sel = Some(i);
            self.name = self.users[i].real.clone();
            self.message = None;
        }
        let Some(u) = self.current().cloned() else {
            ui.alloc(w, lh);
            return;
        };
        let mut tab = self.tab;
        let shown = tab;
        let labels: &[&str] = if u.me { &[tr!("Senha"), tr!("Imagem"), tr!("Itens de Início")] } else { &[tr!("Senha"), tr!("Imagem")] };
        tab = tab.min(labels.len() - 1);
        let rx = x0 + lw + 14.0;
        let rw = w - lw - 14.0;
        ui.area(Rect::new(rx, y0, rw, lh), |ui| {
            ui.tab_box(&mut tab, labels, lh, |ui| match shown.min(labels.len() - 1) {
                0 => self.password_tab(ui, &u),
                1 => self.picture_tab(ui, &u),
                _ => inicio.ui(ui),
            });
        });
        self.tab = tab;
        ui.alloc(w, lh);
        if let Some((ok, m)) = self.message.clone() {
            ui.space(6.0);
            let t = if ok { m } else { tr!("Não deu certo: {}", m) };
            ui.small(&t);
        }
    }

    fn password_tab(&mut self, ui: &mut Ui, u: &Account) {
        const LW: f32 = 100.0;
        ui.space(4.0);
        if u.me {
            let before = self.name.clone();
            ui.form_row(tr!("Nome"), LW, |ui| {
                ui.text_field(&mut self.name, 220.0);
            });
            let _ = before;
        } else {
            ui.form_row(tr!("Nome"), LW, |ui| ui.label(&u.real));
        }
        ui.form_row(tr!("Nome curto"), LW, |ui| ui.label(&u.login));
        ui.form_row(tr!("Tipo"), LW, |ui| ui.label(if u.admin { tr!("Administrador") } else { tr!("Padrão") }));
        ui.space(10.0);
        if !u.me {
            ui.small(tr!("Só dá para mudar a própria conta por aqui."));
            return;
        }
        let changed = self.name.trim() != u.real && !self.name.trim().is_empty();
        let (mut save, mut pw) = (false, false);
        ui.horizontal(|ui| {
            ui.space(LW);
            ui.set_active(changed && !self.busy);
            save = ui.default_button(tr!("Salvar Nome"));
            ui.set_active(!self.busy);
            pw = ui.button(tr!("Mudar Senha…"));
            ui.set_active(true);
        });
        if save {
            let (path, name) = (u.path.clone(), self.name.trim().to_string());
            self.start(tr!("Nome salvo.").into(), move || {
                let out = Command::new("busctl").args(["call", "org.freedesktop.Accounts", &path, "org.freedesktop.Accounts.User", "SetRealName", "s", &name]).output().map_err(|e| e.to_string())?;
                if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
            });
        }
        if pw {
            self.pw = Some(Default::default());
        }
    }

    fn picture_tab(&mut self, ui: &mut Ui, u: &Account) {
        ui.space(4.0);
        let (x, y) = ui.cursor();
        self.draw_picture(ui, u, x, y, 72.0);
        ui.fonts.draw(ui.c, tr!("Imagem desta conta (na tela de entrada e de bloqueio)."), x + 88.0, y + 30.0, 11.0, false, rgba(80, 80, 86, 255));
        ui.alloc(ui.available_width(), 80.0);
        if !u.me {
            return;
        }
        ui.label(tr!("Escolha um retrato:"));
        // Retratos desenhados (os mesmos da tela de entrada, com cores diferentes).
        let (gx, gy) = ui.cursor();
        let s = 48.0;
        let mut pick = None;
        for k in 0..8u64 {
            let r = Rect::new(gx + (k % 6) as f32 * (s + 10.0), gy + (k / 6) as f32 * (s + 10.0), s, s);
            chrome::avatar(ui.c, r.x, r.y, s, k * 7 + 3);
            if ui.hit(r).clicked {
                pick = Some(k * 7 + 3);
            }
        }
        ui.alloc(ui.available_width(), 2.0 * (s + 10.0));
        let mut choose = false;
        ui.horizontal(|ui| {
            ui.set_active(!self.busy);
            choose = ui.button(tr!("Escolher Imagem…"));
            ui.set_active(true);
        });
        if choose {
            self.file = Some(FileBrowser::open(None, &["png"]));
        }
        if let Some(seed) = pick {
            // Desenha o retrato num PNG (~/.face) e entrega ao AccountsService.
            let face = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".face");
            if let Some(mut c) = Canvas::new(96.0, 96.0, 1.0) {
                chrome::avatar(&mut c, 0.0, 0.0, 96.0, seed);
                if let Err(e) = c.pm.save_png(&face) {
                    self.message = Some((false, tr!("não foi possível gravar o retrato: {}", e)));
                    return;
                }
                self.set_icon(u, face.to_string_lossy().into_owned());
            }
        }
    }

    fn set_icon(&mut self, u: &Account, file: String) {
        let path = u.path.clone();
        self.start(tr!("Imagem trocada.").into(), move || {
            let out = Command::new("busctl").args(["call", "org.freedesktop.Accounts", &path, "org.freedesktop.Accounts.User", "SetIconFile", "s", &file]).output().map_err(|e| e.to_string())?;
            if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
        });
    }

    /// Folhas: senha nova e escolha de imagem.
    pub fn sheets(&mut self, ui: &mut Ui) {
        if let Some(mut fb) = self.file.take() {
            match ui.file_sheet(&mut fb) {
                Some(Choice::Picked(p)) => {
                    if let Some(u) = self.current().cloned() {
                        self.set_icon(&u, p.to_string_lossy().into_owned());
                    }
                }
                Some(Choice::Cancelled) => {}
                None => self.file = Some(fb),
            }
            return;
        }
        let Some(mut f) = self.pw.take() else { return };
        let (sw, sh) = (420.0, 200.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = 78.0;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        ui.fonts.draw(ui.c, tr!("Mudar a senha desta conta"), x + 20.0, y + 26.0, 13.0, true, rgba(20, 20, 20, 255));
        let labels = [tr!("Senha atual:"), tr!("Senha nova:"), tr!("Confirmar:")];
        let mut enter = false;
        let fresh = f.iter().all(String::is_empty);
        for (k, l) in labels.iter().enumerate() {
            let yy = y + 44.0 + k as f32 * 32.0;
            let tw = ui.text_width(l, 13.0, false);
            ui.fonts.draw(ui.c, l, x + 130.0 - tw, yy + 16.0, 13.0, false, rgba(20, 20, 20, 255));
            enter |= ui.password_field_in(Rect::new(x + 140.0, yy, sw - 160.0, 22.0), &mut f[k], k == 0 && fresh);
        }
        let (mut cancel, mut ok) = (false, false);
        ui.area(Rect::new(x + sw - 230.0, y + sh - 40.0, 220.0, 30.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok = ui.default_button(tr!("Mudar Senha"));
            });
        });
        if cancel || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            return;
        }
        if ok || enter {
            if f[1].is_empty() || f[1] != f[2] {
                self.message = Some((false, tr!("a senha nova e a confirmação precisam ser iguais.").into()));
            } else {
                let (old, new) = (f[0].clone(), f[1].clone());
                self.start(tr!("Senha trocada.").into(), move || change_password(&old, &new));
                return;
            }
        }
        self.pw = Some(f);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn saidas_do_busctl() {
        assert_eq!(super::busctl_string("s \"Fabio Carneiro\"\n"), "Fabio Carneiro");
        assert_eq!(super::busctl_paths("ao 2 \"/org/a/User1000\" \"/org/a/User1001\"\n"), ["/org/a/User1000", "/org/a/User1001"]);
    }
}
