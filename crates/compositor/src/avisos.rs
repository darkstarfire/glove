//! Avisos (notificações): o Glove atende o serviço padrão do Linux
//! (org.freedesktop.Notifications, o do `notify-send`) numa thread e mostra cada
//! aviso num balão no canto de cima à direita. Os avisos ficam guardados na
//! central, que abre pelo balãozinho da barra de menu (com a contagem dos não
//! lidos). Na central: Não perturbe (sem balões) e Limpar.
//!
//! Clicar num balão (ou num aviso da central) responde ao app ("default") e traz
//! o app à frente; o × dispensa.

use glove_config::tr;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use glove_paint::{draw::*, icons::Icon, theme::MENUBAR_H};
use smithay::{
    reexports::calloop::{
        channel::{channel, Event as ChannelEvent, Sender},
        timer::{TimeoutAction, Timer},
    },
    utils::{Logical, Point, Rectangle},
};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

const BUBBLE_W: f32 = 320.0;
const GAP: f32 = 8.0;
const MAX_BUBBLES: usize = 4;
const KEEP: usize = 60;
const SHOW: Duration = Duration::from_secs(6);
const FADE: Duration = Duration::from_millis(300);
const CENTER_W: f32 = 340.0;
const ROW_H: f32 = 74.0;
const HEAD_H: f32 = 40.0;

#[derive(Clone, Debug)]
pub struct Aviso {
    pub id: u32,
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    /// Pares (chave, rótulo).
    pub actions: Vec<(String, String)>,
    /// 0 baixa, 1 normal, 2 crítica.
    pub urgency: u8,
    /// None = fica até clicar.
    pub expire: Option<Duration>,
    pub time: String,
    /// Ainda vale responder ao app (não foi fechado).
    pub live: bool,
}

pub struct Bubble {
    pub id: u32,
    since: Instant,
    fading: Option<Instant>,
    pub tex: Option<Tex>,
    pub h: i32,
}

pub enum Msg {
    Ready(zbus::blocking::Connection),
    Notify(Aviso),
    Close(u32),
}

#[derive(Default)]
pub struct Avisos {
    pub list: Vec<Aviso>,
    pub bubbles: Vec<Bubble>,
    pub unread: u32,
    pub quiet: bool,
    pub conn: Option<zbus::blocking::Connection>,
    /// Central aberta: rolagem e a textura.
    pub center: Option<Center>,
    anim: bool,
}

pub struct Center {
    pub scroll: usize,
    pub tex: Option<(u64, Tex)>,
    pub size: (i32, i32),
}

/// Tira a marcação simples que alguns apps mandam (<b>, <a>…) e os escapes.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

struct Server {
    tx: Sender<Msg>,
    next: Arc<AtomicU32>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into(), "persistence".into()]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(&self, app_name: String, replaces_id: u32, app_icon: String, summary: String, body: String, actions: Vec<String>, hints: HashMap<String, zbus::zvariant::OwnedValue>, expire_timeout: i32) -> u32 {
        let id = if replaces_id > 0 { replaces_id } else { self.next.fetch_add(1, Ordering::Relaxed) };
        let urgency = hints.get("urgency").and_then(|v| u8::try_from(v.clone()).ok()).unwrap_or(1);
        let entry = hints.get("desktop-entry").and_then(|v| String::try_from(v.clone()).ok()).unwrap_or_default();
        let expire = match expire_timeout {
            _ if urgency >= 2 => None,
            0 => None,
            t if t > 0 => Some(Duration::from_millis(t.max(2000) as u64)),
            _ => Some(SHOW),
        };
        let (.., h, m, _) = crate::shell::local_now();
        let icon = if !app_icon.is_empty() { app_icon } else { entry };
        let pairs = actions.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect();
        let a = Aviso { id, app: app_name, icon, summary: plain(&summary), body: plain(&body), actions: pairs, urgency, expire, time: format!("{h:02}:{m:02}"), live: true };
        let _ = self.tx.send(Msg::Notify(a));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(Msg::Close(id));
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("Glove".into(), "Glove".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

const PATH: &str = "/org/freedesktop/Notifications";
const IFACE: &str = "org.freedesktop.Notifications";

impl Glove {
    /// Liga o serviço de avisos (se outro já atende, o Glove não mostra avisos).
    pub fn start_avisos(&mut self) {
        let (tx, rx) = channel::<Msg>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, data| {
            if let ChannelEvent::Msg(m) = ev {
                data.state.avisos_msg(m);
            }
        });
        std::thread::spawn(move || {
            let server = Server { tx: tx.clone(), next: Arc::new(AtomicU32::new(1)) };
            let built = zbus::blocking::connection::Builder::session().and_then(|b| b.name("org.freedesktop.Notifications")).and_then(|b| b.serve_at(PATH, server)).and_then(|b| b.build());
            match built {
                Ok(conn) => {
                    tracing::info!("avisos: atendendo org.freedesktop.Notifications");
                    let _ = tx.send(Msg::Ready(conn));
                    loop {
                        std::thread::park();
                    }
                }
                Err(e) => tracing::info!("avisos: outro serviço já atende ({e})"),
            }
        });
    }

    fn avisos_msg(&mut self, m: Msg) {
        match m {
            Msg::Ready(c) => {
                self.avisos.conn = Some(c);
                self.dirty = true;
            }
            Msg::Notify(a) => self.add_aviso(a),
            Msg::Close(id) => {
                if let Some(a) = self.avisos.list.iter_mut().find(|a| a.id == id) {
                    a.live = false;
                }
                self.dismiss_bubble(id, 3);
            }
        }
    }

    fn add_aviso(&mut self, a: Aviso) {
        let id = a.id;
        let expire = a.expire;
        let quiet = self.avisos.quiet && a.urgency < 2;
        match self.avisos.list.iter_mut().find(|x| x.id == id) {
            Some(old) => *old = a,
            None => {
                self.avisos.list.push(a);
                self.avisos.unread += 1;
            }
        }
        if self.avisos.list.len() > KEEP {
            let extra = self.avisos.list.len() - KEEP;
            self.avisos.list.drain(..extra);
        }
        if let Some(c) = &mut self.avisos.center {
            c.tex = None;
        }
        if !quiet {
            let b = &mut self.avisos.bubbles;
            match b.iter_mut().find(|x| x.id == id) {
                Some(x) => {
                    x.since = Instant::now();
                    x.fading = None;
                    x.tex = None;
                }
                None => {
                    b.insert(0, Bubble { id, since: Instant::now(), fading: None, tex: None, h: 0 });
                    b.truncate(MAX_BUBBLES);
                }
            }
            if let Some(d) = expire {
                let since = self.avisos.bubbles.iter().find(|x| x.id == id).map(|x| x.since);
                let _ = self.loop_handle.insert_source(Timer::from_duration(d), move |_, _, data| {
                    let st = &mut data.state;
                    if st.avisos.bubbles.iter().any(|x| x.id == id && Some(x.since) == since) {
                        st.fade_bubble(id);
                    }
                    TimeoutAction::Drop
                });
            }
        }
        self.dirty = true;
    }

    fn fade_bubble(&mut self, id: u32) {
        if let Some(b) = self.avisos.bubbles.iter_mut().find(|b| b.id == id) {
            b.fading.get_or_insert(Instant::now());
        }
        if !self.avisos.anim {
            self.avisos.anim = true;
            let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
                let st = &mut data.state;
                st.dirty = true;
                if st.avisos.bubbles.iter().any(|b| b.fading.is_some()) {
                    TimeoutAction::ToDuration(Duration::from_millis(16))
                } else {
                    st.avisos.anim = false;
                    TimeoutAction::Drop
                }
            });
        }
    }

    /// Tira o balão e avisa o app (motivo: 1 expirou, 2 dispensado, 3 fechado pelo app).
    fn dismiss_bubble(&mut self, id: u32, reason: u32) {
        let had = self.avisos.bubbles.len();
        self.avisos.bubbles.retain(|b| b.id != id);
        if had != self.avisos.bubbles.len() || reason == 2 {
            self.signal("NotificationClosed", (id, reason));
        }
        self.dirty = true;
    }

    fn signal<B: serde::Serialize + zbus::zvariant::DynamicType>(&self, name: &str, body: B) {
        if let Some(c) = &self.avisos.conn {
            let _ = c.emit_signal(None::<&str>, PATH, IFACE, name, &body);
        }
    }

    /// Clique no aviso: responde ao app e traz ele à frente.
    fn activate_aviso(&mut self, id: u32) {
        let Some(a) = self.avisos.list.iter().find(|a| a.id == id).cloned() else { return };
        if a.live {
            let key = a.actions.iter().find(|(k, _)| k == "default").or(a.actions.first()).map(|(k, _)| k.clone());
            if let Some(k) = key {
                self.signal("ActionInvoked", (id, k));
            }
        }
        // O app pelo nome: alguma janela cujo app_id ou nome bata.
        let want = crate::search::normalize(&a.app);
        let target = self.space.elements().chain(self.stashed()).find(|w| {
            let (_, id) = crate::shell::window_info(w);
            let n = crate::search::normalize(&id);
            !want.is_empty() && (n == want || n.ends_with(&format!(".{want}")) || crate::search::normalize(&crate::shell::app_display_name(w)) == want)
        });
        if let Some(pid) = target.and_then(|w| self.window_pid(w)) {
            self.activate_app(pid);
        }
        self.avisos.list.retain(|x| x.id != id);
        self.dismiss_bubble(id, 2);
        if let Some(c) = &mut self.avisos.center {
            c.tex = None;
        }
    }

    pub fn toggle_avisos_center(&mut self) {
        if self.avisos.center.is_some() {
            self.avisos.center = None;
        } else {
            self.close_menu();
            self.avisos.center = Some(Center { scroll: 0, tex: None, size: (0, 0) });
            self.avisos.unread = 0;
            // Com a central aberta, os balões somem (estão todos nela).
            self.avisos.bubbles.clear();
        }
        self.dirty = true;
    }

    // -------------------------------------------------------------------
    // Posições e cliques
    // -------------------------------------------------------------------

    fn bubble_rects(&self) -> Vec<Rectangle<i32, Logical>> {
        let (sw, _) = self.output_size();
        let x = sw - BUBBLE_W as i32 - 12;
        let mut y = MENUBAR_H + 10;
        self.avisos
            .bubbles
            .iter()
            .map(|b| {
                let r = Rectangle::new((x, y).into(), (BUBBLE_W as i32, b.h.max(1)).into());
                y += b.h + GAP as i32;
                r
            })
            .collect()
    }

    pub fn center_origin(&self) -> Point<i32, Logical> {
        let (sw, _) = self.output_size();
        (sw - CENTER_W as i32 - 4, MENUBAR_H).into()
    }

    /// Clique: num balão ou na central. Devolve se o clique era deles.
    pub fn avisos_click(&mut self, pos: Point<f64, Logical>) -> bool {
        if let Some(c) = &self.avisos.center {
            let o = self.center_origin();
            let (x, y) = ((pos.x - o.x as f64) as f32, (pos.y - o.y as f64) as f32);
            let inside = x >= 0.0 && y >= 0.0 && x < c.size.0 as f32 && y < c.size.1 as f32;
            if !inside {
                self.avisos.center = None;
                self.dirty = true;
                // O clique só fecha a central (no ícone da barra, não reabre).
                return true;
            }
            if y < HEAD_H {
                // Cabeçalho: Não perturbe (à esquerda do Limpar) e Limpar.
                if x > CENTER_W - 70.0 {
                    self.avisos.list.clear();
                } else if x > CENTER_W - 200.0 {
                    self.avisos.quiet = !self.avisos.quiet;
                }
            } else {
                let row = ((y - HEAD_H) / ROW_H) as usize + c.scroll;
                let ids: Vec<u32> = self.avisos.list.iter().rev().map(|a| a.id).collect();
                if let Some(&id) = ids.get(row) {
                    // × no canto da linha: só tira da lista.
                    let in_row_y = (y - HEAD_H) % ROW_H;
                    if x > CENTER_W - 30.0 && in_row_y < 26.0 {
                        self.avisos.list.retain(|a| a.id != id);
                    } else {
                        self.activate_aviso(id);
                    }
                }
            }
            if let Some(c) = &mut self.avisos.center {
                c.tex = None;
            }
            self.dirty = true;
            return true;
        }
        let rects = self.bubble_rects();
        for (i, r) in rects.iter().enumerate() {
            if r.to_f64().contains(pos) {
                let id = self.avisos.bubbles[i].id;
                let close = pos.x - (r.loc.x as f64) < 26.0 && pos.y - (r.loc.y as f64) < 26.0;
                if close {
                    self.dismiss_bubble(id, 2);
                } else {
                    self.activate_aviso(id);
                }
                return true;
            }
        }
        false
    }

    pub fn avisos_scroll(&mut self, v: f64) -> bool {
        let n = self.avisos.list.len();
        let Some(c) = &mut self.avisos.center else { return false };
        let step = if v > 0.0 { 1 } else if v < 0.0 { -1 } else { 0 };
        let s = (c.scroll as i32 + step).clamp(0, n.saturating_sub(1) as i32) as usize;
        if s != c.scroll {
            c.scroll = s;
            c.tex = None;
            self.dirty = true;
        }
        true
    }

    // -------------------------------------------------------------------
    // Desenho
    // -------------------------------------------------------------------

    pub fn prepare_avisos(&mut self) {
        let sc = self.scale() as f32;
        // Balões que acabaram de sumir.
        let gone: Vec<u32> = self.avisos.bubbles.iter().filter(|b| b.fading.is_some_and(|t| t.elapsed() >= FADE)).map(|b| b.id).collect();
        for id in gone {
            self.dismiss_bubble(id, 1);
        }
        for b in &mut self.avisos.bubbles {
            if b.tex.is_none() {
                if let Some(a) = self.avisos.list.iter().find(|a| a.id == b.id) {
                    if let Some(c) = draw_bubble(&self.fonts, a, sc) {
                        b.h = c.h.round() as i32;
                        b.tex = Some(to_tex(c));
                    }
                }
            }
        }
        let (_, sh) = self.output_size();
        let max_h = (sh - MENUBAR_H - 60) as f32;
        let quiet = self.avisos.quiet;
        let list: Vec<Aviso> = self.avisos.list.iter().rev().cloned().collect();
        if let Some(c) = &mut self.avisos.center {
            if c.tex.is_none() {
                let rows = ((max_h - HEAD_H) / ROW_H).floor().max(1.0) as usize;
                let shown: Vec<&Aviso> = list.iter().skip(c.scroll).take(rows).collect();
                let h = HEAD_H + (shown.len().max(1) as f32) * ROW_H + 6.0;
                c.size = (CENTER_W as i32, h as i32);
                c.tex = draw_center(&self.fonts, &shown, quiet, list.len(), h, sc).map(|t| (0, to_tex(t)));
            }
        }
    }

    /// Balões para desenhar: (onde, textura, opacidade).
    pub fn bubble_draws(&self) -> Vec<(Point<i32, Logical>, &Tex, f32)> {
        let rects = self.bubble_rects();
        self.avisos
            .bubbles
            .iter()
            .zip(rects)
            .filter_map(|(b, r)| {
                let a = match b.fading {
                    Some(t) => 1.0 - (t.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0),
                    None => (b.since.elapsed().as_secs_f32() / 0.2).min(1.0),
                };
                b.tex.as_ref().map(|t| (r.loc, t, a))
            })
            .collect()
    }

    /// Ainda entrando (para pedir mais quadros).
    pub fn avisos_animating(&self) -> bool {
        self.avisos.bubbles.iter().any(|b| b.fading.is_some() || b.since.elapsed() < Duration::from_millis(220))
    }
}

fn icon_of(a: &Aviso) -> Icon {
    let name = if a.icon.starts_with('/') || a.icon.starts_with("file:") { String::new() } else { a.icon.clone() };
    if !name.is_empty() {
        return Icon::from_name(&name);
    }
    // Apps do Glove: pelo nome mostrado ou pelo app_id ("Correio", "glove.Correio").
    let n = crate::search::normalize(&a.app);
    let glove = glove_config::default_dock_apps().into_iter().chain(glove_config::extra_apps()).find(|d| {
        let id = crate::search::normalize(&d.app_id);
        crate::search::normalize(&glove_config::tr_dyn(&d.name)) == n || crate::search::normalize(&d.name) == n || id == n || id.ends_with(&format!(".{n}"))
    });
    if let Some(d) = glove {
        return Icon::from_name(&d.icon);
    }
    match glove_config::desktop_icon(&a.app) {
        Some(n) => Icon::from_name(&n),
        None => Icon::from_name(&a.app.to_lowercase()),
    }
}

fn wrap(f: &glove_paint::font::Fonts, t: &str, px: f32, max: f32, lines: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in t.lines() {
        out.extend(glove_paint::chrome::wrap_lines(f, para, px, false, max));
    }
    if out.len() > lines {
        out.truncate(lines);
        if let Some(l) = out.last_mut() {
            *l = f.ellipsize(&format!("{l}…"), px, false, max);
        }
    }
    out
}

/// Balão: fumê escuro com o ícone do app, o título e até 3 linhas do texto.
fn draw_bubble(f: &glove_paint::font::Fonts, a: &Aviso, sc: f32) -> Option<Canvas> {
    let w = BUBBLE_W;
    let text_x = 58.0;
    let body = wrap(f, &a.body, 12.0, w - text_x - 14.0, 3);
    let h = (18.0 + 18.0 + body.len() as f32 * 15.0 + 12.0).max(64.0);
    let mut c = Canvas::new(w, h, sc)?;
    c.fill(rrect(0.0, 0.0, w, h, 10.0), &solid(rgba(18, 18, 22, 215)));
    c.stroke(rrect(0.5, 0.5, w - 1.0, h - 1.0, 10.0), &solid(rgba(255, 255, 255, if a.urgency >= 2 { 140 } else { 50 })), 1.0);
    // Brilho de vidro em cima.
    c.fill(rrect(1.0, 1.0, w - 2.0, 18.0, 9.0), &vgrad(1.0, 19.0, &[(0.0, rgba(255, 255, 255, 30)), (1.0, rgba(255, 255, 255, 0))]));
    glove_paint::icons::draw_bare(&mut c, f, icon_of(a), 14.0, 14.0, 36.0, a.app.chars().next());
    let title = if a.summary.is_empty() { a.app.clone() } else { a.summary.clone() };
    let title = f.ellipsize(&title, 13.0, true, w - text_x - 14.0);
    f.draw(&mut c, &title, text_x, 26.0, 13.0, true, rgba(255, 255, 255, 255));
    for (k, l) in body.iter().enumerate() {
        f.draw(&mut c, l, text_x, 44.0 + k as f32 * 15.0, 12.0, false, rgba(215, 215, 220, 255));
    }
    // × de dispensar, no canto.
    c.fill(circle(12.0, 12.0, 7.0), &solid(rgba(255, 255, 255, 40)));
    for (x0, y0, x1, y1) in [(9.0f32, 9.0f32, 15.0f32, 15.0f32), (15.0, 9.0, 9.0, 15.0)] {
        for k in 0..=8 {
            let t = k as f32 / 8.0;
            c.fill(circle(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, 0.7), &solid(rgba(255, 255, 255, 200)));
        }
    }
    Some(c)
}

/// A central: cabeçalho (Não perturbe, Limpar) e os avisos, os mais novos em cima.
fn draw_center(f: &glove_paint::font::Fonts, rows: &[&Aviso], quiet: bool, total: usize, h: f32, sc: f32) -> Option<Canvas> {
    let w = CENTER_W;
    let mut c = Canvas::new(w, h, sc)?;
    c.fill(rrect4(0.0, 0.0, w, h, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(245, 245, 247, 255)));
    c.stroke(rrect4(0.5, -0.5, w - 1.0, h, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(0, 0, 0, 70)), 1.0);
    let ink = rgba(25, 25, 28, 255);
    let gray = rgba(120, 120, 126, 255);
    f.draw(&mut c, tr!("Avisos"), 14.0, 25.0, 13.0, true, ink);
    // Não perturbe: caixinha.
    let qx = w - 196.0;
    c.fill(rrect(qx, 13.0, 13.0, 13.0, 3.0), &solid(rgba(255, 255, 255, 255)));
    c.stroke(rrect(qx + 0.5, 13.5, 12.0, 12.0, 3.0), &solid(rgba(0, 0, 0, 90)), 1.0);
    if quiet {
        c.fill(rrect(qx + 3.0, 16.0, 7.0, 7.0, 1.5), &solid(rgba(40, 110, 220, 255)));
    }
    f.draw(&mut c, tr!("Não perturbe"), qx + 18.0, 24.5, 11.5, false, ink);
    let clear = tr!("Limpar|Avisos");
    let cw = f.measure(clear, 11.5, false) + 16.0;
    c.fill(rrect(w - 14.0 - cw, 11.0, cw, 18.0, 9.0), &vgrad(11.0, 29.0, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(225, 225, 230, 255))]));
    c.stroke(rrect(w - 14.0 - cw + 0.5, 11.5, cw - 1.0, 17.0, 9.0), &solid(rgba(0, 0, 0, if total > 0 { 90 } else { 40 })), 1.0);
    f.draw(&mut c, clear, w - 14.0 - cw + 8.0, 24.0, 11.5, false, if total > 0 { ink } else { gray });
    c.hline(0.0, HEAD_H - 1.0, w, rgba(0, 0, 0, 40));
    if rows.is_empty() {
        let t = tr!("Nenhum aviso");
        let tw = f.measure(t, 12.0, false);
        f.draw(&mut c, t, (w - tw) / 2.0, HEAD_H + ROW_H / 2.0 + 4.0, 12.0, false, gray);
    }
    for (k, a) in rows.iter().enumerate() {
        let y = HEAD_H + k as f32 * ROW_H;
        if k > 0 {
            c.hline(14.0, y, w - 28.0, rgba(0, 0, 0, 25));
        }
        glove_paint::icons::draw_bare(&mut c, f, icon_of(a), 12.0, y + 12.0, 34.0, a.app.chars().next());
        let app = f.ellipsize(&a.app, 10.5, false, w - 120.0);
        f.draw(&mut c, &app, 56.0, y + 18.0, 10.5, false, gray);
        let tw = f.measure(&a.time, 10.5, false);
        f.draw(&mut c, &a.time, w - 34.0 - tw, y + 18.0, 10.5, false, gray);
        f.draw(&mut c, "×", w - 24.0, y + 19.0, 13.0, false, gray);
        let title = f.ellipsize(if a.summary.is_empty() { &a.app } else { &a.summary }, 12.5, true, w - 70.0);
        f.draw(&mut c, &title, 56.0, y + 35.0, 12.5, true, ink);
        for (j, l) in wrap(f, &a.body, 11.5, w - 70.0, 2).iter().enumerate() {
            f.draw(&mut c, l, 56.0, y + 51.0 + j as f32 * 14.0, 11.5, false, rgba(60, 60, 66, 255));
        }
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sem_marcacao() {
        assert_eq!(super::plain("<b>Oi</b> &amp; tchau"), "Oi & tchau");
        assert_eq!(super::plain("a &lt; b"), "a < b");
    }
}
