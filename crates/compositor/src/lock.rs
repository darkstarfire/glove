//! Tela apagada por inatividade, bloqueio de tela com senha (PAM) e tampa do notebook.
//!
//! - Parado por `energy.screen_off_minutes`: a tela apaga (DPMS na sessão própria;
//!   dentro de outro desktop, só fica preta) e, com `require_password`, bloqueia.
//! - Bloqueado: só o papel de parede e o painel de desbloqueio aparecem; nenhum app
//!   recebe teclado ou mouse. A senha é conferida pelo PAM numa thread à parte.
//! - Tampa fechada: apaga e bloqueia; aberta: acende. Com monitor externo, só a tela
//!   embutida desliga; se o externo sair com a tampa fechada, apaga e bloqueia.
//! - Repouso ou hibernação do sistema, peça quem pedir (tampa pelo logind, botão de
//!   energia, bateria no fim, `systemctl`): bloqueia antes (`watch_sleep`).

use glove_config::tr;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    time::{Duration, Instant},
};

use glove_paint::chrome;
use smithay::reexports::calloop::{
    channel::{channel, Event as ChannelEvent},
    timer::{TimeoutAction, Timer},
};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// Balanço do painel quando a senha está errada.
const SHAKE: Duration = Duration::from_millis(500);

pub struct Lock {
    password: String,
    /// Conferindo a senha (o PAM pode demorar alguns segundos quando ela está errada).
    pub busy: bool,
    pub message: Option<String>,
    shake_since: Option<Instant>,
    pub pressed: bool,
    since: Instant,
    pub name: String,
    key: Option<(usize, Option<String>, bool, bool, bool, i32, u64)>,
    pub tex: Option<Tex>,
}

impl Lock {
    /// Deslocamento horizontal do balanço (lógico).
    pub fn shake_offset(&self) -> i32 {
        let Some(t) = self.shake_since else { return 0 };
        let e = t.elapsed().as_secs_f32();
        if e >= SHAKE.as_secs_f32() {
            return 0;
        }
        let damp = 1.0 - e / SHAKE.as_secs_f32();
        ((e * 40.0).sin() * 14.0 * damp).round() as i32
    }
}

/// O que o logind avisa sobre repouso e hibernação (`watch_sleep`).
pub enum SleepEvent {
    /// O pedido de espera foi aceito (na partida e a cada volta do repouso).
    Hold(Option<zbus::zvariant::OwnedFd>),
    /// O sistema vai suspender ou hibernar.
    Going,
    /// O sistema voltou.
    Woke,
}

/// Quanto o sistema espera o quadro do bloqueio ir para a tela antes de suspender.
const SLEEP_HOLD: Duration = Duration::from_millis(300);

#[derive(Default)]
pub struct Idle {
    pub last_input: Option<Instant>,
    timer: bool,
    /// A tela está apagada.
    pub screen_off: bool,
    /// Já pediu o repouso nesta parada (só pede de novo depois de uma entrada).
    slept: bool,
    /// Já pediu para sair da sessão nesta parada (Segurança).
    logout_asked: bool,
}

/// Teclas que o painel de desbloqueio entende.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockKey {
    Char(char),
    Backspace,
    Enter,
    Escape,
}

impl Glove {
    // -----------------------------------------------------------------------
    // Inatividade e tela apagada
    // -----------------------------------------------------------------------

    /// Começa a contar o tempo parado (uma vez, na partida).
    pub fn start_idle_timer(&mut self) {
        self.idle.last_input = Some(Instant::now());
        if self.idle.timer {
            return;
        }
        self.idle.timer = true;
        let first = self.idle_timeouts().0.unwrap_or(Duration::from_secs(600));
        self.loop_handle
            .insert_source(Timer::from_duration(first), |_, _, data| TimeoutAction::ToDuration(data.state.idle_check()))
            .ok();
    }

    /// (apagar a tela, repouso) para a fonte de energia atual; None = nunca.
    fn idle_timeouts(&self) -> (Option<Duration>, Option<Duration>) {
        let on_battery = self.extras.battery.as_ref().is_some_and(|b| !b.on_ac);
        let (screen, sleep) = self.config.energy.timeouts(on_battery);
        let d = |m: u32| (m > 0).then(|| Duration::from_secs(m as u64 * 60));
        (d(screen), d(sleep))
    }

    /// Confere o tempo parado e devolve quando conferir de novo (um acordar por
    /// período, não a cada segundo).
    fn idle_check(&mut self) -> Duration {
        let (screen, sleep) = self.idle_timeouts();
        let idle = self.idle.last_input.map(|t| t.elapsed()).unwrap_or_default();
        let mut next = Duration::from_secs(60);
        // Protetor de tela antes de apagar (se vier antes do apagar).
        let saver = self.config.saver.minutes;
        if saver > 0 && !self.greeter {
            let timeout = Duration::from_secs(saver as u64 * 60);
            if idle >= timeout {
                if self.saver.is_none() && !self.idle.screen_off && screen.is_none_or(|s| timeout < s) {
                    self.start_saver(None, false);
                }
            } else {
                next = next.min(timeout - idle);
            }
        }
        if let Some(timeout) = screen {
            if idle >= timeout {
                if !self.idle.screen_off {
                    tracing::info!("parado há {} min: apagando a tela", timeout.as_secs() / 60);
                    self.screen_off();
                }
            } else {
                next = next.min(timeout - idle);
            }
        }
        if let Some(timeout) = sleep {
            if idle >= timeout {
                // A tela de login e as sessões aninhadas não põem o computador para dormir.
                if !self.idle.slept && !self.greeter && !self.nested {
                    tracing::info!("parado há {} min: repouso", timeout.as_secs() / 60);
                    self.idle.slept = true;
                    self.sleep();
                }
            } else {
                next = next.min(timeout - idle);
            }
        }
        // Segurança: sair da sessão depois de tanto tempo parado (o glove-sair mostra
        // o aviso com a contagem; mexer no mouse cancela).
        let lm = self.config.security.logout_minutes;
        if lm > 0 && !self.greeter {
            let timeout = Duration::from_secs(lm as u64 * 60);
            if idle >= timeout {
                if !self.idle.logout_asked {
                    tracing::info!("parado há {lm} min: encerrando a sessão");
                    self.idle.logout_asked = true;
                    self.spawn(if self.nested { "glove-sair sair" } else { "glove-sair encerrar" });
                }
            } else {
                next = next.min(timeout - idle);
            }
        }
        next.max(Duration::from_secs(1))
    }

    pub fn screen_off(&mut self) {
        // Tela apagada: o protetor para de desenhar.
        self.saver = None;
        if self.config.energy.require_password {
            self.lock_screen();
        }
        self.close_menu();
        self.idle.screen_off = true;
        self.dirty = true;
    }

    /// Qualquer entrada: marca a hora e acende a tela. Devolve verdadeiro se a
    /// tela estava apagada (o evento só acorda, não vai adiante).
    pub fn note_input(&mut self) -> bool {
        self.idle.last_input = Some(Instant::now());
        self.idle.slept = false;
        self.idle.logout_asked = false;
        // Qualquer entrada tira o protetor (e só serve para isso).
        if self.stop_saver() {
            return true;
        }
        if self.idle.screen_off {
            self.idle.screen_off = false;
            self.dirty = true;
            return true;
        }
        false
    }

    /// Tampa do notebook: fechada apaga (e bloqueia); aberta acende.
    pub fn lid(&mut self, closed: bool) {
        tracing::info!("tampa {}", if closed { "fechada" } else { "aberta" });
        let was = std::mem::replace(&mut self.lid_closed, closed);
        // Com monitor externo, só a tela embutida liga e desliga (como no modo "tampa fechada").
        let external = self.space.outputs().any(|o| !["eDP", "LVDS", "DSI"].iter().any(|p| o.name().starts_with(p)));
        if external && !self.nested {
            if was != closed {
                self.reconfigure_outputs = true;
            }
            self.note_input();
            return;
        }
        if closed {
            self.screen_off();
        } else {
            self.note_input();
        }
    }

    /// Acompanha o repouso e a hibernação do sistema (sinal `PrepareForSleep` do logind),
    /// para bloquear antes mesmo quando não foi o Glove que pediu: fechar a tampa (quem
    /// suspende é o logind), o botão de energia, a bateria no fim, `systemctl hibernate`.
    /// O Glove segura o repouso por um instante (`Inhibit` com atraso) para o bloqueio
    /// já estar na tela: ao voltar, a mesa não aparece nem por um quadro.
    pub fn watch_sleep(&mut self) {
        let (tx, rx) = channel::<SleepEvent>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, data| {
            if let ChannelEvent::Msg(ev) = ev {
                data.state.sleep_event(ev);
            }
        });
        std::thread::spawn(move || {
            let run = || -> zbus::Result<()> {
                let conn = zbus::blocking::Connection::system()?;
                let logind = zbus::blocking::Proxy::new(&conn, "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager")?;
                let hold = || -> Option<zbus::zvariant::OwnedFd> {
                    logind
                        .call("Inhibit", &("sleep", "Glove", "Bloquear a tela antes do repouso", "delay"))
                        .map_err(|e| tracing::warn!("repouso: o sistema não vai esperar o bloqueio ({e})"))
                        .ok()
                };
                let signals = logind.receive_signal("PrepareForSleep")?;
                tracing::info!("repouso: acompanhando o logind");
                let _ = tx.send(SleepEvent::Hold(hold()));
                for msg in signals {
                    let Ok(going) = msg.body().deserialize::<bool>() else { continue };
                    if going {
                        let _ = tx.send(SleepEvent::Going);
                    } else {
                        let _ = tx.send(SleepEvent::Woke);
                        let _ = tx.send(SleepEvent::Hold(hold()));
                    }
                }
                Ok(())
            };
            if let Err(e) = run() {
                tracing::warn!("repouso: sem o aviso do logind, o bloqueio só vale para o repouso pedido pelo Glove ({e})");
            }
        });
    }

    pub fn sleep_event(&mut self, ev: SleepEvent) {
        match ev {
            SleepEvent::Hold(fd) => self.sleep_hold = fd,
            SleepEvent::Going => {
                tracing::info!("o sistema vai suspender ou hibernar");
                if self.config.energy.require_password && !self.greeter {
                    self.saver = None;
                    self.lock_screen();
                    // O sistema espera o quadro do bloqueio ir para a tela.
                    self.loop_handle
                        .insert_source(Timer::from_duration(SLEEP_HOLD), |_, _, data| {
                            data.state.sleep_hold = None;
                            TimeoutAction::Drop
                        })
                        .ok();
                } else {
                    self.sleep_hold = None;
                }
            }
            SleepEvent::Woke => {
                tracing::info!("o sistema voltou do repouso");
                // O tempo parado conta de novo a partir de agora; com a tampa aberta, a
                // tela acende (para o painel da senha aparecer).
                if self.lid_closed {
                    self.idle.last_input = Some(Instant::now());
                    self.idle.slept = false;
                } else {
                    self.note_input();
                }
                self.dirty = true;
            }
        }
    }

    /// Repouso: bloqueia antes (se pedido) e pede ao sistema para suspender.
    pub fn sleep(&mut self) {
        if self.config.energy.require_password {
            self.lock_screen();
        }
        let _ = std::process::Command::new("systemctl").arg("suspend").spawn();
    }

    // -----------------------------------------------------------------------
    // Bloqueio
    // -----------------------------------------------------------------------

    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }

    pub fn lock_screen(&mut self) {
        // A tela de login não bloqueia (não há sessão para proteger).
        if self.lock.is_some() || self.greeter {
            return;
        }
        tracing::info!("tela bloqueada");
        self.close_menu();
        self.close_force_quit();
        self.close_volume_slider();
        // Nenhum app fica com o teclado enquanto está bloqueado.
        self.focus_window(None);
        let name = crate::shell::user_full_name();
        self.lock = Some(Lock { password: String::new(), busy: false, message: None, shake_since: None, pressed: false, since: Instant::now(), name, key: None, tex: None });
        self.dirty = true;
        // Pisca o cursor e anima balanço/giratório só enquanto bloqueado.
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(100)), |_, _, data| {
                let st = &mut data.state;
                let Some(l) = &st.lock else { return TimeoutAction::Drop };
                let animating = l.busy || l.shake_since.is_some_and(|t| t.elapsed() < SHAKE);
                st.dirty = true;
                TimeoutAction::ToDuration(Duration::from_millis(if animating { 33 } else { 530 }))
            })
            .ok();
    }

    fn unlock(&mut self) {
        tracing::info!("tela desbloqueada");
        self.lock = None;
        self.focus_topmost(None);
        self.dirty = true;
    }

    pub fn lock_key(&mut self, k: LockKey) {
        let Some(l) = &mut self.lock else { return };
        if l.busy {
            return;
        }
        match k {
            LockKey::Char(c) if !c.is_control() && l.password.len() < 256 => {
                l.password.push(c);
                l.message = None;
            }
            LockKey::Backspace => {
                l.password.pop();
            }
            LockKey::Escape => {
                l.password.clear();
                l.message = None;
            }
            LockKey::Enter => self.try_unlock(),
            _ => {}
        }
        self.dirty = true;
    }

    /// Confere a senha numa thread (o PAM bloqueia) e responde pelo laço de eventos.
    pub fn try_unlock(&mut self) {
        let Some(l) = &mut self.lock else { return };
        if l.busy || l.password.is_empty() {
            return;
        }
        l.busy = true;
        let password = std::mem::take(&mut l.password);
        let user = std::env::var("USER").unwrap_or_default();
        let (tx, rx) = channel::<Result<(), String>>();
        std::thread::spawn(move || {
            let r = pam_check(&user, password);
            let _ = tx.send(r);
        });
        self.loop_handle
            .insert_source(rx, |ev, _, data| {
                if let ChannelEvent::Msg(r) = ev {
                    let st = &mut data.state;
                    match r {
                        Ok(()) => st.unlock(),
                        Err(e) => {
                            tracing::info!("senha recusada: {e}");
                            if let Some(l) = &mut st.lock {
                                l.busy = false;
                                l.message = Some(tr!("Senha incorreta. Tente de novo.").into());
                                l.shake_since = Some(Instant::now());
                            }
                            st.dirty = true;
                        }
                    }
                }
            })
            .ok();
        self.dirty = true;
    }

    /// Monitor (lógico) onde aparece o painel de senha: o que tem o mouse,
    /// como no original; sem ele, o principal.
    pub fn lock_output_rect(&self) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
        self.output_rect_at(self.pointer_pos)
            .or_else(|| self.primary().and_then(|o| self.space.output_geometry(o)))
            .unwrap_or_else(|| {
                let (w, h) = self.output_size();
                smithay::utils::Rectangle::from_size((w, h).into())
            })
    }

    /// Canto do painel (sem a sombra, em coordenadas da mesa), já com o balanço.
    pub fn lock_panel_pos(&self) -> (i32, i32) {
        let g = self.lock_output_rect();
        let (w, h) = (g.size.w, g.size.h);
        let shake = self.lock.as_ref().map(|l| l.shake_offset()).unwrap_or(0);
        (g.loc.x + ((w as f32 - chrome::LOCK_W) / 2.0) as i32 + shake, g.loc.y + ((h as f32 - chrome::LOCK_H) / 2.0 - h as f32 * 0.08) as i32)
    }

    /// O ponto cai no botão Desbloquear?
    pub fn lock_button_at(&self, x: f64, y: f64) -> bool {
        let (px, py) = self.lock_panel_pos();
        let (bx, by, bw, bh) = chrome::lock_button_rect();
        let (x, y) = ((x - px as f64) as f32, (y - py as f64) as f32);
        x >= bx && x < bx + bw && y >= by && y < by + bh
    }

    pub fn prepare_lock(&mut self) {
        let sc = self.scale() as f32;
        let tgen = self.theme_gen;
        let Some(l) = &mut self.lock else { return };
        let caret = (l.since.elapsed().as_millis() / 530) % 2 == 0;
        let spin = if l.busy { (l.since.elapsed().as_millis() / 83 % 12) as i32 } else { 0 };
        let key = (l.password.chars().count(), l.message.clone(), l.busy, caret, l.pressed, spin, tgen);
        if l.key.as_ref() != Some(&key) {
            l.tex = chrome::lock_panel(&self.fonts, &self.theme, &l.name, key.0, l.message.as_deref(), l.busy, caret, l.pressed, spin as f32 / 12.0, sc).map(to_tex);
            l.key = Some(key);
        }
        if l.shake_since.is_some_and(|t| t.elapsed() >= SHAKE) {
            l.shake_since = None;
        }
    }
}

// ---------------------------------------------------------------------------
// PAM (carregado na hora com dlopen: o Glove não depende da biblioteca para compilar)
// ---------------------------------------------------------------------------

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

#[repr(C)]
struct PamConv {
    conv: extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int,
    appdata: *mut c_void,
}

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_CONV_ERR: c_int = 19;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

/// Responde às perguntas do PAM: a senha para "sem eco"; vazio para as outras.
extern "C" fn conversation(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, appdata: *mut c_void) -> c_int {
    if n <= 0 || n > 32 || msgs.is_null() || resp.is_null() {
        return PAM_CONV_ERR;
    }
    unsafe {
        // O PAM libera as respostas com free(): precisam vir do malloc.
        let out = libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if out.is_null() {
            return PAM_BUF_ERR;
        }
        let password = &*(appdata as *const CString);
        for i in 0..n as usize {
            let m = &**msgs.add(i);
            let r = &mut *out.add(i);
            r.resp = match m.msg_style {
                PAM_PROMPT_ECHO_OFF => libc::strdup(password.as_ptr()),
                PAM_PROMPT_ECHO_ON => libc::strdup(c"".as_ptr()),
                _ => std::ptr::null_mut(),
            };
        }
        *resp = out;
    }
    PAM_SUCCESS
}

/// Serviço do PAM: o do Glove, se instalado (data/pam.d/glove), ou o de login.
fn pam_service() -> &'static CStr {
    if std::path::Path::new("/etc/pam.d/glove").exists() { c"glove" } else { c"login" }
}

/// Confere usuário e senha. A senha é apagada da memória no fim.
fn pam_check(user: &str, password: String) -> Result<(), String> {
    type Start = unsafe extern "C" fn(*const c_char, *const c_char, *const PamConv, *mut *mut c_void) -> c_int;
    type Call = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
    let mut bytes = password.into_bytes();
    let secret = CString::new(bytes.clone()).map_err(|_| "senha inválida".to_string());
    bytes.iter_mut().for_each(|b| *b = 0);
    let secret = secret?;
    let user = CString::new(user).map_err(|_| "usuário inválido".to_string())?;
    let result = unsafe {
        let lib = libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if lib.is_null() {
            return Err("libpam não encontrada".into());
        }
        let sym = |name: &CStr| libc::dlsym(lib, name.as_ptr());
        let (start, auth, acct, end) = (sym(c"pam_start"), sym(c"pam_authenticate"), sym(c"pam_acct_mgmt"), sym(c"pam_end"));
        if start.is_null() || auth.is_null() || acct.is_null() || end.is_null() {
            return Err("libpam incompleta".into());
        }
        let start: Start = std::mem::transmute(start);
        let auth: Call = std::mem::transmute(auth);
        let acct: Call = std::mem::transmute(acct);
        let end: Call = std::mem::transmute(end);
        let conv = PamConv { conv: conversation, appdata: &secret as *const CString as *mut c_void };
        let mut h: *mut c_void = std::ptr::null_mut();
        let mut rc = start(pam_service().as_ptr(), user.as_ptr(), &conv, &mut h);
        if rc == PAM_SUCCESS {
            rc = auth(h, 0);
            if rc == PAM_SUCCESS {
                rc = acct(h, 0);
            }
            end(h, rc);
        }
        // A biblioteca fica carregada (o PAM pode ter deixado módulos apontando para ela).
        rc
    };
    // Apaga a cópia da senha.
    let mut raw = secret.into_bytes();
    raw.iter_mut().for_each(|b| *b = 0);
    if result == PAM_SUCCESS { Ok(()) } else { Err(format!("PAM {result}")) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn senha_errada_e_recusada() {
        // Usuário que não existe: o PAM tem de recusar (e nunca aceitar).
        assert!(super::pam_check("glove-usuario-que-nao-existe", "x".into()).is_err());
    }
}
