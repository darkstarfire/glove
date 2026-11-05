//! Forçar Encerrar: detecção de apps que não respondem (ping do xdg-shell), o
//! cursor de espera sobre as janelas deles e o painel desenhado pelo compositor.

use std::time::{Duration, Instant};

use glove_paint::{
    chrome::{self, FqHit, ForceQuitRow, TitleButton},
    icons::Icon,
};
use smithay::{
    desktop::Window,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::shell::xdg::ShellClient,
};

use crate::{
    shell::{app_display_name, to_tex, window_info, Tex},
    state::Glove,
};

/// Sem resposta ao ping nesse tempo = "não responde".
const HUNG_AFTER: Duration = Duration::from_secs(2);
/// Intervalo mínimo entre pings ao mesmo app.
const PING_EVERY: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_millis(250);

struct PingEntry {
    client: ShellClient,
    sent: Option<Instant>,
    last: Instant,
    hung: bool,
}

#[derive(Default)]
pub struct Pinger {
    entries: Vec<PingEntry>,
    timer: bool,
}

/// Painel aberto.
pub struct ForceQuit {
    /// App escolhido (pelo pid, para não mudar quando a lista muda).
    pub selected: Option<u32>,
    /// Mostrando a pergunta de confirmação.
    pub confirm: bool,
    pub pressed: Option<FqHit>,
    pub title_hover: bool,
    /// Canto do conteúdo (sem a sombra).
    pub origin: Point<i32, Logical>,
    pub size: (i32, i32),
    pub rows: Vec<(u32, ForceQuitRow)>,
    key: Option<(Vec<(u32, ForceQuitRow)>, Option<u32>, bool, Option<FqHit>, bool, u64)>,
    pub tex: Option<Tex>,
}

/// Cursor de espera: textura e passo da animação.
#[derive(Default)]
pub struct WaitCursor {
    pub shown: bool,
    pub tex: Option<Tex>,
    key: Option<(i32, u64)>,
}

impl Glove {
    fn all_windows(&self) -> Vec<Window> {
        self.space.elements().cloned().chain(self.minimized.iter().chain(&self.hidden).map(|(w, _)| w.clone())).collect()
    }

    /// Pergunta ao app da janela se ele está vivo (no máximo uma vez por segundo).
    pub fn ping_window(&mut self, w: &Window) {
        let Some(t) = w.toplevel() else { return };
        let client = t.client();
        if !client.alive() {
            return;
        }
        let now = Instant::now();
        let i = match self.pinger.entries.iter().position(|e| e.client == client) {
            Some(i) => i,
            None => {
                self.pinger.entries.push(PingEntry { client, sent: None, last: now - PING_EVERY, hung: false });
                self.pinger.entries.len() - 1
            }
        };
        let e = &mut self.pinger.entries[i];
        if e.sent.is_some() || e.last.elapsed() < PING_EVERY {
            return;
        }
        if e.client.send_ping(SERIAL_COUNTER.next_serial()).is_ok() {
            e.sent = Some(now);
            e.last = now;
        }
        self.ensure_ping_timer();
    }

    pub fn on_pong(&mut self, client: ShellClient) {
        if let Some(e) = self.pinger.entries.iter_mut().find(|e| e.client == client) {
            e.sent = None;
            if e.hung {
                e.hung = false;
                tracing::info!("app voltou a responder");
                self.dirty = true;
            }
        }
    }

    pub fn window_hung(&self, w: &Window) -> bool {
        let Some(t) = w.toplevel() else { return false };
        let client = t.client();
        self.pinger.entries.iter().any(|e| e.hung && e.client == client)
    }

    fn ensure_ping_timer(&mut self) {
        if self.pinger.timer {
            return;
        }
        self.pinger.timer = true;
        self.loop_handle
            .insert_source(Timer::from_duration(TICK), |_, _, data| {
                if data.state.ping_tick() {
                    TimeoutAction::ToDuration(TICK)
                } else {
                    data.state.pinger.timer = false;
                    TimeoutAction::Drop
                }
            })
            .ok();
    }

    /// Marca quem não respondeu; com o painel aberto, pergunta a todos; anima o
    /// cursor de espera. Devolve falso quando não há mais nada a acompanhar.
    fn ping_tick(&mut self) -> bool {
        self.pinger.entries.retain(|e| e.client.alive());
        for e in &mut self.pinger.entries {
            if !e.hung && e.sent.is_some_and(|s| s.elapsed() > HUNG_AFTER) {
                e.hung = true;
                tracing::info!("app não responde");
                self.dirty = true;
            }
        }
        if self.force_quit.is_some() {
            for w in self.all_windows() {
                self.ping_window(&w);
            }
            self.dirty = true;
        }
        if self.wait_cursor.shown {
            self.dirty = true;
        }
        let pending = self.pinger.entries.iter().any(|e| e.sent.is_some());
        pending || self.force_quit.is_some() || self.wait_cursor.shown
    }

    /// Cursor de espera sobre janelas de apps que não respondem.
    pub fn update_wait_cursor(&mut self, under: Option<&Window>) {
        let shown = under.is_some_and(|w| self.window_hung(w));
        if shown != self.wait_cursor.shown {
            self.wait_cursor.shown = shown;
            self.dirty = true;
            if shown {
                self.ensure_ping_timer();
            }
        }
    }

    pub fn prepare_wait_cursor(&mut self) {
        if !self.wait_cursor.shown {
            return;
        }
        let step = (self.start_time.elapsed().as_millis() / 83 % 12) as i32;
        let key = (step, self.scale().to_bits());
        if self.wait_cursor.key != Some(key) {
            self.wait_cursor.tex = chrome::cursor_wait(step, self.scale() as f32).map(to_tex);
            self.wait_cursor.key = Some(key);
        }
    }

    // -----------------------------------------------------------------------
    // Painel
    // -----------------------------------------------------------------------

    /// Abre o painel (ou traz de volta à pergunta inicial), com o app da frente escolhido.
    pub fn open_force_quit(&mut self) {
        self.close_menu();
        let selected = self.focused_pid();
        self.force_quit = Some(ForceQuit { selected, confirm: false, pressed: None, title_hover: false, origin: (0, 0).into(), size: (0, 0), rows: Vec::new(), key: None, tex: None });
        for w in self.all_windows() {
            self.ping_window(&w);
        }
        self.ensure_ping_timer();
        self.dirty = true;
    }

    pub fn close_force_quit(&mut self) {
        if self.force_quit.take().is_some() {
            self.dirty = true;
        }
    }

    /// Apps com janela: (pid, linha), em ordem alfabética.
    fn force_quit_rows(&self) -> Vec<(u32, ForceQuitRow)> {
        let mut rows: Vec<(u32, ForceQuitRow)> = Vec::new();
        for w in self.all_windows() {
            let Some(pid) = self.window_pid(&w) else { continue };
            let hung = self.window_hung(&w);
            if let Some((_, r)) = rows.iter_mut().find(|(p, _)| *p == pid) {
                r.hung |= hung;
                continue;
            }
            let app_id = window_info(&w).1;
            let icon = glove_config::known_app(&self.config.dock_apps, &app_id).map(|a| Icon::from_name(&a.icon)).unwrap_or(Icon::Window);
            rows.push((pid, ForceQuitRow { icon, name: app_display_name(&w), hung }));
        }
        rows.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
        rows
    }

    fn fq_question(&self, fq: &ForceQuit) -> Option<String> {
        if !fq.confirm {
            return None;
        }
        let name = fq.rows.iter().find(|(p, _)| Some(*p) == fq.selected).map(|(_, r)| r.name.clone())?;
        Some(format!("Deseja forçar o encerramento de “{name}”?"))
    }

    pub fn prepare_force_quit(&mut self) {
        let Some(mut fq) = self.force_quit.take() else { return };
        fq.rows = self.force_quit_rows();
        if fq.selected.is_some_and(|s| !fq.rows.iter().any(|(p, _)| *p == s)) {
            fq.selected = None;
            fq.confirm = false;
        }
        let question = self.fq_question(&fq);
        let (w, h) = chrome::force_quit_size(&self.fonts, fq.rows.len(), question.as_deref());
        let (sw, sh) = self.output_size();
        fq.size = (w as i32, h as i32);
        fq.origin = (((sw as f32 - w) / 2.0) as i32, (glove_paint::theme::MENUBAR_H as f32 + (sh as f32 - h) / 4.0) as i32).into();
        let key = (fq.rows.clone(), fq.selected, fq.confirm, fq.pressed, fq.title_hover, self.theme_gen);
        if fq.key.as_ref() != Some(&key) {
            let sel = fq.selected.and_then(|s| fq.rows.iter().position(|(p, _)| *p == s));
            let rows: Vec<ForceQuitRow> = fq.rows.iter().map(|(_, r)| r.clone()).collect();
            fq.tex = chrome::force_quit_panel(&self.fonts, &self.theme, &rows, sel, question.as_deref(), fq.pressed, fq.title_hover, self.scale() as f32).map(to_tex);
            fq.key = Some(key);
        }
        self.force_quit = Some(fq);
    }

    /// O que há do painel no ponto (None = fora dele).
    pub fn force_quit_hit(&self, pos: Point<f64, Logical>) -> Option<FqHit> {
        let fq = self.force_quit.as_ref()?;
        let (x, y) = ((pos.x - fq.origin.x as f64) as f32, (pos.y - fq.origin.y as f64) as f32);
        if x < 0.0 || y < 0.0 || x >= fq.size.0 as f32 || y >= fq.size.1 as f32 {
            return None;
        }
        let question = self.fq_question(fq);
        Some(chrome::force_quit_hit(&self.fonts, fq.rows.len(), question.as_deref(), x, y))
    }

    pub fn force_quit_press(&mut self, h: FqHit) {
        let Some(fq) = &mut self.force_quit else { return };
        match h {
            FqHit::Row(i) if !fq.confirm => {
                fq.selected = fq.rows.get(i).map(|(p, _)| *p);
            }
            FqHit::Title(Some(_)) | FqHit::Confirm | FqHit::Cancel => fq.pressed = Some(h),
            _ => {}
        }
        self.dirty = true;
    }

    pub fn force_quit_release(&mut self, h: Option<FqHit>) {
        let Some(fq) = &mut self.force_quit else { return };
        let pressed = fq.pressed.take();
        self.dirty = true;
        if pressed.is_none() || pressed != h {
            return;
        }
        match pressed.unwrap() {
            FqHit::Title(Some(TitleButton::Close)) => self.close_force_quit(),
            FqHit::Confirm => self.force_quit_confirm(),
            FqHit::Cancel => fq.confirm = false,
            _ => {}
        }
    }

    /// Botão Forçar Encerrar (ou Enter): primeiro pergunta, depois encerra.
    fn force_quit_confirm(&mut self) {
        let Some(fq) = &mut self.force_quit else { return };
        let Some(pid) = fq.selected else { return };
        if !fq.confirm {
            fq.confirm = true;
            self.dirty = true;
            return;
        }
        fq.confirm = false;
        fq.selected = None;
        tracing::info!("forçando o encerramento do pid {pid}");
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
        self.dirty = true;
    }

    /// Teclas com o painel aberto: ↑/↓ escolhem, Enter encerra, Esc cancela ou fecha.
    /// Devolve verdadeiro se a tecla foi usada.
    pub fn force_quit_key(&mut self, name: &str) -> bool {
        let Some(fq) = &mut self.force_quit else { return false };
        match name {
            "up" | "down" if !fq.confirm => {
                let n = fq.rows.len();
                if n > 0 {
                    let cur = fq.selected.and_then(|s| fq.rows.iter().position(|(p, _)| *p == s));
                    let next = match (cur, name) {
                        (None, _) => 0,
                        (Some(i), "up") => i.saturating_sub(1),
                        (Some(i), _) => (i + 1).min(n - 1),
                    };
                    fq.selected = Some(fq.rows[next].0);
                }
            }
            "return" | "kp_enter" => self.force_quit_confirm(),
            "escape" => {
                if fq.confirm {
                    fq.confirm = false;
                } else {
                    self.close_force_quit();
                }
            }
            _ => return false,
        }
        self.dirty = true;
        true
    }
}
