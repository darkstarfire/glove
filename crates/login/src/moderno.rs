//! A tela de login nos temas modernos: o papel de parede desfocado na tela inteira, os
//! usuários lado a lado no meio (fotos redondas com o nome embaixo), a senha embaixo da
//! foto escolhida e os botões de energia no pé da tela. Nos temas clássicos continua o
//! painel da época (main.rs).

use super::*;
use glove_ui::paint::tiny_skia::{FillRule, FilterQuality, PathBuilder};

/// Foto na lista e na senha.
const FOTO: f32 = 96.0;
const FOTO_SENHA: f32 = 120.0;
/// Distância entre os usuários na lista (encolhe se não couberem).
const PASSO: f32 = 150.0;
/// Campo da senha.
const CAMPO_W: f32 = 220.0;
const CAMPO_H: f32 = 28.0;

fn branco(a: u8) -> glove_ui::paint::Color {
    rgba(255, 255, 255, a)
}

/// Texto claro com uma sombra leve (legível sobre qualquer papel de parede).
fn texto(ui: &mut Ui, t: &str, cx: f32, y: f32, px: f32, bold: bool) {
    let tw = ui.text_width(t, px, bold);
    ui.fonts.draw(ui.c, t, cx - tw / 2.0, y + 1.0, px, bold, rgba(0, 0, 0, 110));
    ui.fonts.draw(ui.c, t, cx - tw / 2.0, y, px, bold, branco(255));
}

/// A foto (ou o avatar desenhado; ou a figura genérica de "Outro…") num círculo.
fn foto_redonda(ui: &mut Ui, photo: Option<&Pixmap>, login: Option<&str>, x: f32, y: f32, d: f32) {
    glove_ui::paint::screens::round_photo(ui.c, photo, login, x, y, d);
}

/// Lua crescente (o círculo grande menos o pequeno), em contorno fechado.
fn lua(cx: f32, cy: f32, r: f32) -> Option<glove_ui::paint::tiny_skia::Path> {
    let (c2x, c2y, r2) = (cx + r * 0.5, cy - r * 0.36, r * 0.86);
    let n = 96;
    let ponto = |ox: f32, oy: f32, rr: f32, i: usize| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        (ox + rr * a.cos(), oy + rr * a.sin())
    };
    let fora_do_pequeno = |(x, y): (f32, f32)| (x - c2x).powi(2) + (y - c2y).powi(2) >= r2 * r2;
    let dentro_do_grande = |(x, y): (f32, f32)| (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    // O arco do círculo grande que fica de fora do pequeno, começando logo depois de uma
    // das pontas.
    let grande: Vec<(f32, f32)> = (0..n).map(|i| ponto(cx, cy, r, i)).collect();
    let inicio = (0..n).find(|&i| fora_do_pequeno(grande[i]) && !fora_do_pequeno(grande[(i + n - 1) % n]))?;
    let arco1: Vec<(f32, f32)> = (0..n).map(|k| grande[(inicio + k) % n]).take_while(|p| fora_do_pequeno(*p)).collect();
    // O arco do pequeno que fica dentro do grande, da ponta final de volta à inicial.
    let pequeno: Vec<(f32, f32)> = (0..n).map(|i| ponto(c2x, c2y, r2, i)).collect();
    let ini2 = (0..n).find(|&i| dentro_do_grande(pequeno[i]) && !dentro_do_grande(pequeno[(i + n - 1) % n]))?;
    let mut arco2: Vec<(f32, f32)> = (0..n).map(|k| pequeno[(ini2 + k) % n]).take_while(|p| dentro_do_grande(*p)).collect();
    let fim = *arco1.last()?;
    let d = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2);
    if d(*arco2.first()?, fim) > d(*arco2.last()?, fim) {
        arco2.reverse();
    }
    let mut pb = PathBuilder::new();
    pb.move_to(arco1[0].0, arco1[0].1);
    for p in arco1.iter().skip(1).chain(arco2.iter()) {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

impl Login {
    /// The wallpaper behind: the chosen user's (in the list, the last one who entered),
    /// from the copy their session keeps for the login screen; without one, the login
    /// screen's own. As the path of a picture or a style of Glove.
    fn fundo_de(&self) -> String {
        let user = self.usuario_do_fundo();
        user.and_then(|u| glove_config::login_wallpaper::of(&u.login, u.uid)).map(|p| p.display().to_string()).unwrap_or_else(|| self.wallpaper.clone())
    }

    /// The wallpaper softly blurred (painted at 1/4 of the size, blurred and enlarged) and a
    /// little darker, as on the login screen of the Mac.
    fn fundo_moderno(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        // In the login screen of the Glove the wallpaper is drawn by the Glove itself, on
        // every monitor (this window goes to the monitor with the mouse): only the clock and
        // the rest go here, over a transparent window.
        if std::env::var_os("GLOVE_GREETER").is_some() {
            ui.c.clear(glove_ui::paint::Color::TRANSPARENT);
            self.relogio(ui, w, h);
            return;
        }
        let fonte = self.fundo_de();
        let key = (w.round() as u32, h.round() as u32, fonte.clone());
        if self.fundo.as_ref().map(|f| &f.0) != Some(&key) {
            let theme = ui.theme.clone();
            self.fundo = glove_ui::paint::screens::blurred_wallpaper(&theme, &fonte, w, h, 3, 0.12).map(|pm| (key, pm));
        }
        match &self.fundo {
            Some((_, pm)) => {
                let sc = ui.c.scale;
                let t = Transform::from_scale(w * sc / pm.width() as f32, h * sc / pm.height() as f32);
                ui.c.draw_pixmap_t(pm.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t);
            }
            None => ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(46, 70, 110, 255))),
        }
        self.relogio(ui, w, h);
    }

    /// The date and the time on top (as on the lock screen), redrawn every minute.
    fn relogio(&self, ui: &mut Ui, w: f32, h: f32) {
        let (time, date) = glove_ui::paint::screens::clock_now();
        if let Some(c) = glove_ui::paint::screens::lock_clock(ui.fonts, &time, &date, ui.c.scale) {
            let cw = c.pm.width() as f32 / ui.c.scale;
            ui.c.draw_canvas(&c, ((w - cw) / 2.0).round(), (h * 0.08).round());
        }
        ui.wake_after(Duration::from_secs(20));
    }

    pub(crate) fn ui_moderno(&mut self, ui: &mut Ui) {
        self.fundo_moderno(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        let busy = self.pending.is_some();
        match self.screen {
            Screen::List => self.lista_moderna(ui, w, h),
            Screen::Password(sel) => self.senha_moderna(ui, sel, w, h, busy),
        }
        self.energia_moderna(ui, w, h, busy);
    }

    /// Os usuários lado a lado (e "Outro…" no fim).
    fn lista_moderna(&mut self, ui: &mut Ui, w: f32, h: f32) {
        let n = self.users.len() + 1;
        let passo = PASSO.min((w - 40.0) / n as f32);
        let x0 = w / 2.0 - passo * n as f32 / 2.0;
        let y = (h * 0.62 - FOTO / 2.0).round();
        for i in 0..n {
            let cx = x0 + passo * (i as f32 + 0.5);
            let resp = ui.hit(Rect::new(cx - passo / 2.0 + 6.0, y - 10.0, passo - 12.0, FOTO + 50.0));
            if resp.hover {
                ui.c.fill(circle(cx, y + FOTO / 2.0, FOTO / 2.0 + 7.0), &solid(branco(if resp.down { 80 } else { 50 })));
            }
            let nome = match self.users.get(i) {
                Some(u) => {
                    let (photo, login) = (self.photos[i].as_ref(), u.login.clone());
                    foto_redonda(ui, photo, Some(&login), cx - FOTO / 2.0, y, FOTO);
                    u.name.clone()
                }
                None => {
                    foto_redonda(ui, None, None, cx - FOTO / 2.0, y, FOTO);
                    tr!("Outro…").to_string()
                }
            };
            let nome = ui.fonts.ellipsize(&nome, 14.0, true, passo - 16.0);
            texto(ui, &nome, cx, y + FOTO + 26.0, 14.0, true);
            if resp.clicked {
                self.screen = Screen::Password((i < self.users.len()).then_some(i));
                self.password.clear();
                self.message = None;
                self.focus_password = i < self.users.len();
            }
        }
        if self.users.len() == 1 && ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Enter) {
            self.screen = Screen::Password(Some(0));
            self.focus_password = true;
        }
    }

    /// A foto escolhida, o nome, a senha (com a seta de entrar) e Cancelar.
    fn senha_moderna(&mut self, ui: &mut Ui, sel: Option<usize>, w: f32, h: f32, busy: bool) {
        let cx = (w / 2.0 + self.shake_offset()).round();
        let mut y = (h * 0.5 - FOTO_SENHA / 2.0).round();
        match sel {
            Some(i) => {
                let (photo, login) = (self.photos[i].as_ref(), self.users[i].login.clone());
                foto_redonda(ui, photo, Some(&login), cx - FOTO_SENHA / 2.0, y, FOTO_SENHA);
                let nome = self.users[i].name.clone();
                texto(ui, &nome, cx, y + FOTO_SENHA + 28.0, 16.0, true);
                y += FOTO_SENHA + 46.0;
            }
            None => {
                foto_redonda(ui, None, None, cx - FOTO_SENHA / 2.0, y, FOTO_SENHA);
                y += FOTO_SENHA + 22.0;
                let mut name = std::mem::take(&mut self.other_name);
                ui.set_enabled(!busy);
                ui.area(Rect::new(cx - CAMPO_W / 2.0, y, CAMPO_W, CAMPO_H), |ui| {
                    ui.text_field(&mut name, CAMPO_W);
                });
                if name.is_empty() {
                    ui.fonts.draw(ui.c, tr!("Nome"), cx - CAMPO_W / 2.0 + 10.0, y + 18.0, 13.0, false, rgba(140, 140, 146, 255));
                }
                self.other_name = name;
                y += CAMPO_H + 10.0;
            }
        }
        // A senha, com a seta de entrar à direita.
        ui.set_enabled(!busy);
        let focus = std::mem::take(&mut self.focus_password);
        let mut pw = std::mem::take(&mut self.password);
        let campo = Rect::new(cx - CAMPO_W / 2.0, y, CAMPO_W, CAMPO_H);
        let mut go = ui.password_field_in(campo, &mut pw, focus);
        if pw.is_empty() && !busy {
            ui.fonts.draw(ui.c, tr!("Digite a senha"), campo.x + 10.0, campo.y + 18.0, 13.0, false, rgba(140, 140, 146, 255));
        }
        if pw != self.password && !pw.is_empty() {
            self.message = None;
        }
        self.password = pw;
        let (sx, sy, sr) = (campo.x + campo.w + 20.0, campo.y + CAMPO_H / 2.0, 13.0);
        let seta = ui.hit(Rect::new(sx - sr, sy - sr, sr * 2.0, sr * 2.0));
        let pronta = !self.password.is_empty() || sel.is_none() && !self.other_name.trim().is_empty();
        ui.c.fill(circle(sx, sy, sr), &solid(branco(if seta.down { 130 } else if pronta { 95 } else { 45 })));
        let mut pb = PathBuilder::new();
        pb.move_to(sx - 5.5, sy);
        pb.line_to(sx + 5.0, sy);
        pb.move_to(sx + 0.5, sy - 4.5);
        pb.line_to(sx + 5.0, sy);
        pb.line_to(sx + 0.5, sy + 4.5);
        if let Some(p) = pb.finish() {
            ui.c.stroke_round(Some(p), &solid(branco(if pronta { 255 } else { 150 })), 2.0);
        }
        go |= seta.clicked;
        y += CAMPO_H + 14.0;
        ui.set_enabled(true);
        if let Some(m) = self.message.clone() {
            texto(ui, &m, cx, y + 12.0, 13.0, true);
            y += 22.0;
        }
        // Ambiente gráfico (só com mais de um instalado).
        if self.sessions.len() > 1 {
            let names: Vec<String> = self.sessions.iter().map(|s| s.name.clone()).collect();
            let mut i = self.session_idx;
            ui.set_enabled(!busy);
            ui.area(Rect::new(cx - CAMPO_W / 2.0, y, CAMPO_W, 24.0), |ui| {
                ui.popup(&mut i, &names, CAMPO_W);
            });
            ui.set_enabled(true);
            self.session_idx = i;
            y += 34.0;
        }
        if busy {
            ui.spinner_at(cx, y + 18.0, 18.0);
            return;
        }
        // Cancelar (volta à lista): um × num círculo, com o nome embaixo.
        let (bx, by, br) = (cx, y + 26.0, 15.0);
        let resp = ui.hit(Rect::new(bx - 40.0, by - br, 80.0, br * 2.0 + 22.0));
        ui.c.fill(circle(bx, by, br), &solid(branco(if resp.down { 110 } else if resp.hover { 80 } else { 55 })));
        let mut pb = PathBuilder::new();
        pb.move_to(bx - 5.0, by - 5.0);
        pb.line_to(bx + 5.0, by + 5.0);
        pb.move_to(bx + 5.0, by - 5.0);
        pb.line_to(bx - 5.0, by + 5.0);
        if let Some(p) = pb.finish() {
            ui.c.stroke_round(Some(p), &solid(branco(255)), 2.0);
        }
        texto(ui, tr!("Cancelar"), bx, by + br + 16.0, 12.0, false);
        if resp.clicked || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            self.screen = Screen::List;
            self.password.clear();
            self.message = None;
        } else if go {
            self.submit();
        }
    }

    /// Repouso, Reiniciar e Desligar no pé da tela.
    fn energia_moderna(&mut self, ui: &mut Ui, w: f32, h: f32, busy: bool) {
        let labels = [(tr!("Repouso"), "suspend"), (tr!("Reiniciar"), "reboot"), (tr!("Desligar"), "poweroff")];
        let (by, r) = (h - 96.0, 21.0);
        for (k, (label, verb)) in labels.iter().enumerate() {
            let bx = w / 2.0 + (k as f32 - 1.0) * 110.0;
            let resp = ui.hit(Rect::new(bx - 44.0, by - r, 88.0, r * 2.0 + 26.0));
            // No tema escuro o botão é grafite com o ícone branco; as cores já vêm
            // certas, então o remapeamento do escuro fica desligado aqui.
            let escuro = glove_ui::paint::theme::dark();
            let (fundo, tinta) = if escuro {
                let f = if resp.down { rgba(46, 46, 52, 255) } else if resp.hover { rgba(84, 84, 92, 255) } else { rgba(64, 64, 70, 255) };
                (f, rgba(255, 255, 255, 255))
            } else {
                let f = if resp.down { rgba(214, 214, 220, 255) } else if resp.hover { rgba(255, 255, 255, 255) } else { rgba(238, 238, 242, 255) };
                (f, rgba(54, 54, 60, 255))
            };
            let remap = std::mem::replace(&mut ui.c.dark_remap, false);
            ui.c.fill(circle(bx, by, r), &solid(fundo));
            if k == 0 {
                ui.c.fill_rule(lua(bx, by, 8.0), &solid(tinta), FillRule::Winding);
            } else {
                power_glyph(ui, k, bx, by, tinta, fundo);
            }
            ui.c.dark_remap = remap;
            texto(ui, label, bx, by + r + 20.0, 12.0, false);
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
