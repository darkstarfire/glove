//! glove-login: a tela de login do Glove (roda dentro de `glove --greeter`,
//! iniciado pelo greetd). Lista de usuários com foto, senha, balanço no erro e
//! os botões Repouso, Reiniciar e Desligar.
//!
//! `glove-login --fake` simula o greetd (senha "glove"), para testar numa janela.
//! O comando da sessão vem de $GLOVE_SESSION (padrão: glove-session).

mod greetd;
mod sessoes;
mod users;

use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use glove_kit::{
    paint::{
        chrome::{avatar, avatar_seed, user_picture},
        draw::*,
        tiny_skia::{Pixmap, PixmapPaint, Transform},
    },
    run, App, Rect, Ui, WindowOptions,
};
use users::User;

const W: f32 = 460.0;
const H: f32 = 470.0;
/// Balanço do painel na senha errada.
const SHAKE: Duration = Duration::from_millis(450);

enum Screen {
    List,
    /// Senha do usuário escolhido (None = "Outro…", com o campo do nome).
    Password(Option<usize>),
}

struct Login {
    users: Vec<User>,
    photos: Vec<Option<Pixmap>>,
    host: String,
    screen: Screen,
    other_name: String,
    password: String,
    message: Option<String>,
    /// Conferindo com o greetd (numa thread; o PAM pode demorar).
    pending: Option<Receiver<Result<(), String>>>,
    shake: Option<Instant>,
    focus_password: bool,
    fake: bool,
    session: Vec<String>,
    /// Ambientes gráficos instalados e o escolhido.
    sessions: Vec<sessoes::Session>,
    session_idx: usize,
}

impl Login {
    fn chosen_login(&self) -> String {
        match self.screen {
            Screen::Password(Some(i)) => self.users[i].login.clone(),
            _ => self.other_name.trim().to_string(),
        }
    }

    fn submit(&mut self) {
        let user = self.chosen_login();
        if user.is_empty() || self.pending.is_some() {
            return;
        }
        // A senha sai daqui; a cópia local é apagada.
        let password = std::mem::take(&mut self.password);
        self.message = None;
        let (tx, rx) = channel();
        let chosen = self.sessions.get(self.session_idx).cloned();
        let (cmd, env) = match &chosen {
            Some(s) if s.id != "glove" => s.command(),
            _ => (self.session.clone(), Vec::new()),
        };
        if let Some(s) = &chosen {
            sessoes::remember(s);
        }
        let fake = self.fake;
        std::thread::spawn(move || {
            let r = if fake {
                std::thread::sleep(Duration::from_millis(700));
                eprintln!("glove-login (--fake): sessão {cmd:?} {env:?}");
                if password == "glove" { Ok(()) } else { Err("Senha incorreta.".into()) }
            } else {
                greetd::log_in(&user, &password, &cmd, &env)
            };
            let mut bytes = password.into_bytes();
            bytes.iter_mut().for_each(|b| *b = 0);
            let _ = tx.send(r);
        });
        self.pending = Some(rx);
    }

    fn shake_offset(&self) -> f32 {
        let Some(t) = self.shake else { return 0.0 };
        let e = t.elapsed().as_secs_f32();
        if e >= SHAKE.as_secs_f32() {
            return 0.0;
        }
        (e * 42.0).sin() * 16.0 * (1.0 - e / SHAKE.as_secs_f32())
    }
}

/// Foto do usuário, ou o avatar desenhado dele (escolhido pelo login).
fn user_image(ui: &mut Ui, photo: Option<&Pixmap>, login: &str, x: f32, y: f32, s: f32) {
    match photo {
        Some(_) => picture(ui, photo, x, y, s),
        None => avatar(ui.c, x, y, s, avatar_seed(login)),
    }
}

/// Foto (ou retrato genérico) num quadrado de `s` pontos.
fn picture(ui: &mut Ui, photo: Option<&Pixmap>, x: f32, y: f32, s: f32) {
    match photo {
        Some(p) => {
            let sc = ui.c.scale;
            let k = s * sc / p.width().max(p.height()) as f32;
            let t = Transform::from_scale(k, k).post_translate(x * sc, y * sc);
            ui.c.draw_pixmap_t(p.as_ref(), &PixmapPaint { quality: glove_kit::paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t);
            ui.c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, 3.0), &solid(rgba(0, 0, 0, 90)), 1.0);
        }
        None => {
            let accent = ui.theme.accent;
            user_picture(ui.c, x, y, s, accent);
        }
    }
}

fn center_text(ui: &mut Ui, t: &str, cx: f32, y: f32, px: f32, bold: bool, c: glove_kit::paint::Color) {
    let tw = ui.text_width(t, px, bold);
    ui.fonts.draw(ui.c, t, cx - tw / 2.0, y, px, bold, c);
}

impl App for Login {
    fn ui(&mut self, ui: &mut Ui) {
        // Resposta do greetd.
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    // Sessão marcada: o greetd a inicia quando este app sair.
                    ui.close = true;
                    return;
                }
                Ok(Err(msg)) => {
                    self.pending = None;
                    self.message = Some(msg);
                    self.shake = Some(Instant::now());
                    self.focus_password = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(60)),
                Err(_) => self.pending = None,
            }
        }
        if self.shake.is_some_and(|t| t.elapsed() < SHAKE) {
            ui.wake_after(Duration::from_millis(16));
        }
        let busy = self.pending.is_some();
        let dx = self.shake_offset();
        let w = ui.c.w;
        let cx = w / 2.0 + dx;
        let ink = rgba(20, 20, 20, 255);
        let accent = ui.theme.accent;

        // Cabeçalho: orbe, nome e computador.
        gel_circle(ui.c, cx, 44.0, 24.0, accent);
        center_text(ui, "Glove", cx, 96.0, 22.0, true, ink);
        let host = self.host.clone();
        center_text(ui, &host, cx, 114.0, 11.0, false, rgba(90, 90, 90, 255));

        match self.screen {
            Screen::List => {
                // Lista de usuários (e "Outro…").
                let n = self.users.len() + 1;
                let row = 44.0;
                let (lx, ly, lw) = (cx - 150.0, 136.0, 300.0);
                let lh = (n.min(5) as f32) * row;
                ui.c.fill(rrect(lx, ly, lw, lh, 4.0), &solid(rgba(255, 255, 255, 255)));
                ui.c.stroke(rrect(lx + 0.5, ly + 0.5, lw - 1.0, lh - 1.0, 4.0), &solid(rgba(140, 140, 140, 255)), 1.0);
                for i in 0..n.min(5) {
                    let r = Rect::new(lx, ly + i as f32 * row, lw, row);
                    let resp = ui.hit(r);
                    if resp.hover {
                        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(ui.theme.menu_hot));
                    }
                    let text_c = if resp.hover { glove_kit::paint::Color::WHITE } else { ink };
                    let name = match self.users.get(i) {
                        Some(u) => {
                            let (photo, login) = (self.photos[i].as_ref(), u.login.clone());
                            user_image(ui, photo, &login, r.x + 8.0, r.y + 6.0, 32.0);
                            u.name.clone()
                        }
                        None => {
                            picture(ui, None, r.x + 8.0, r.y + 6.0, 32.0);
                            "Outro…".to_string()
                        }
                    };
                    ui.fonts.draw(ui.c, &name, r.x + 52.0, r.y + 27.0, 14.0, true, text_c);
                    if resp.clicked {
                        self.screen = Screen::Password((i < self.users.len()).then_some(i));
                        self.password.clear();
                        self.message = None;
                        self.focus_password = i < self.users.len();
                    }
                }
                // Enter com um usuário só: vai direto para a senha.
                if self.users.len() == 1 && ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Enter) {
                    self.screen = Screen::Password(Some(0));
                    self.focus_password = true;
                }
            }
            Screen::Password(sel) => {
                let mut y = 136.0;
                match sel {
                    Some(i) => {
                        let (photo, login) = (self.photos[i].as_ref(), self.users[i].login.clone());
                        user_image(ui, photo, &login, cx - 36.0, y, 72.0);
                        let name = self.users[i].name.clone();
                        center_text(ui, &name, cx, y + 92.0, 15.0, true, ink);
                        y += 106.0;
                    }
                    None => {
                        { let lw = ui.text_width("Nome:", 13.0, false); ui.fonts.draw(ui.c, "Nome:", cx - 108.0 - lw, y + 16.0, 13.0, false, ink); }
                        let mut name = std::mem::take(&mut self.other_name);
                        ui.area(Rect::new(cx - 100.0, y, 250.0, 24.0), |ui| {
                            ui.text_field(&mut name, 250.0);
                        });
                        self.other_name = name;
                        y += 40.0;
                    }
                }
                { let lw = ui.text_width("Senha:", 13.0, false); ui.fonts.draw(ui.c, "Senha:", cx - 108.0 - lw, y + 16.0, 13.0, false, ink); }
                ui.set_enabled(!busy);
                let focus = std::mem::take(&mut self.focus_password);
                let mut pw = std::mem::take(&mut self.password);
                let enter = ui.password_field_in(Rect::new(cx - 100.0, y, 250.0, 24.0), &mut pw, focus);
                if pw != self.password && !pw.is_empty() {
                    self.message = None;
                }
                self.password = pw;
                y += 34.0;
                // Ambiente gráfico (só aparece com mais de um instalado).
                if self.sessions.len() > 1 {
                    { let lw = ui.text_width("Sessão:", 13.0, false); ui.fonts.draw(ui.c, "Sessão:", cx - 108.0 - lw, y + 16.0, 13.0, false, ink); }
                    let names: Vec<String> = self.sessions.iter().map(|s| s.name.clone()).collect();
                    let mut i = self.session_idx;
                    ui.area(Rect::new(cx - 100.0, y, 250.0, 24.0), |ui| {
                        ui.popup(&mut i, &names, 250.0);
                    });
                    self.session_idx = i;
                    y += 34.0;
                }
                if let Some(m) = self.message.clone() {
                    center_text(ui, &m, cx, y + 12.0, 12.0, true, rgba(200, 30, 30, 255));
                }
                if busy {
                    ui.set_enabled(true);
                    ui.spinner_at(cx - 60.0, y + 44.0, 16.0);
                    ui.fonts.draw(ui.c, "Entrando…", cx - 46.0, y + 48.0, 13.0, false, ink);
                } else {
                    let mut back = false;
                    let mut go = enter;
                    ui.area(Rect::new(cx - 100.0, y + 30.0, 110.0, 24.0), |ui| back = ui.button("Voltar"));
                    ui.area(Rect::new(cx + 30.0, y + 30.0, 120.0, 24.0), |ui| go |= ui.default_button("Entrar"));
                    if back || ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Escape) {
                        self.screen = Screen::List;
                        self.password.clear();
                        self.message = None;
                    } else if go {
                        self.submit();
                    }
                }
                ui.set_enabled(true);
            }
        }

        // Repouso, Reiniciar e Desligar (botões redondos com nome embaixo).
        let labels = [("Repouso", "suspend"), ("Reiniciar", "reboot"), ("Desligar", "poweroff")];
        let by = H - 74.0;
        for (k, (label, verb)) in labels.iter().enumerate() {
            let bx = w / 2.0 + (k as f32 - 1.0) * 96.0;
            let r = Rect::new(bx - 17.0, by, 34.0, 34.0);
            let resp = ui.hit(r);
            let base = if resp.down { darken(rgba(236, 236, 240, 255), 0.2) } else { rgba(236, 236, 240, 255) };
            gel_circle(ui.c, bx, by + 17.0, 17.0, base);
            power_glyph(ui, k, bx, by + 17.0);
            center_text(ui, label, bx, by + 50.0, 11.0, false, ink);
            if resp.clicked && !busy {
                if self.fake {
                    eprintln!("glove-login (--fake): systemctl {verb}");
                } else {
                    let _ = std::process::Command::new("systemctl").arg(verb).spawn();
                }
            }
        }
    }
}

/// Símbolos dos botões de energia: lua (repouso), seta circular (reiniciar), liga/desliga.
fn power_glyph(ui: &mut Ui, k: usize, cx: f32, cy: f32) {
    let col = rgba(60, 60, 66, 255);
    let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
    match k {
        0 => {
            // Lua: dois círculos (o de cima apaga parte do outro).
            ui.c.fill(circle(cx, cy, 7.0), &solid(col));
            ui.c.fill(circle(cx + 3.5, cy - 2.5, 6.0), &solid(rgba(236, 236, 240, 255)));
            return;
        }
        1 => {
            // Arco de 300° com ponta de seta.
            let r = 6.5;
            let n = 20;
            for i in 0..=n {
                let a = -1.2 + i as f32 / n as f32 * 5.2;
                let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            ui.c.stroke_round(pb.finish(), &solid(col), 2.0);
            let a = -1.2f32;
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            let mut tri = glove_kit::paint::tiny_skia::PathBuilder::new();
            tri.move_to(x - 3.5, y - 1.0);
            tri.line_to(x + 3.0, y - 3.5);
            tri.line_to(x + 1.0, y + 3.5);
            tri.close();
            ui.c.fill(tri.finish(), &solid(col));
            return;
        }
        _ => {
            // Liga/desliga: arco aberto em cima e um traço.
            let r = 6.5;
            let n = 20;
            // Aberto em cima: de 35° à direita do topo, dando a volta por baixo até 35° à esquerda.
            let gap = 35f32.to_radians();
            let start = -std::f32::consts::FRAC_PI_2 + gap;
            let sweep = std::f32::consts::TAU - 2.0 * gap;
            for i in 0..=n {
                let a = start + i as f32 / n as f32 * sweep;
                let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            ui.c.stroke_round(pb.finish(), &solid(col), 2.0);
            let mut l = glove_kit::paint::tiny_skia::PathBuilder::new();
            l.move_to(cx, cy - 8.5);
            l.line_to(cx, cy - 1.5);
            ui.c.stroke_round(l.finish(), &solid(col), 2.0);
        }
    }
}

fn main() {
    let fake = std::env::args().any(|a| a == "--fake");
    let users = users::list();
    let photos = users.iter().map(|u| u.photo.as_ref().and_then(|p| Pixmap::load_png(p).ok())).collect();
    let host = std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).unwrap_or_default();
    let session: Vec<String> = std::env::var("GLOVE_SESSION").unwrap_or_else(|_| "glove-session".into()).split_whitespace().map(str::to_string).collect();
    let sessions = sessoes::list(&session);
    let session_idx = sessoes::last().and_then(|(id, x11)| sessions.iter().position(|s| s.id == id && s.x11 == x11)).unwrap_or(0);
    let app = Login { users, photos, host, screen: Screen::List, other_name: String::new(), password: String::new(), message: None, pending: None, shake: None, focus_password: false, fake, session, sessions, session_idx };
    let opts = WindowOptions { title: "Entrar".into(), app_id: "glove.Login".into(), size: (W as u32, H as u32), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-login: {e}");
        std::process::exit(1);
    }
}
