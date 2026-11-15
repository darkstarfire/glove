//! glove-senha: o agente do polkit do Glove, a janela de senha do sistema.
//!
//! Programas que pedem permissão de administrador pelo polkit (o Discos pelo
//! udisks2, o gnome-disks, o Flatpak, o NetworkManager, o pkexec…) dependem de um
//! agente na sessão para perguntar a senha. Sem ele, o pedido é negado na hora.
//!
//!   glove-senha            o serviço: registra-se no polkit para esta sessão e fica
//!                          esperando pedidos (a sessão do Glove o inicia)
//!   glove-senha --pedir    a janela de um pedido (aberta pelo serviço; os dados
//!                          vêm pelo ambiente)
//!
//! A senha é conferida pelo `polkit-agent-helper-1` (do próprio polkit, pelo PAM):
//! o Glove nunca vê nem guarda nada além do que é digitado na janela.

use glove_config::tr;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{chrome::wrap_lines, draw::*, icons::{self, Icon}},
    run, App, Rect, Ui, WindowOptions,
};
use zbus::zvariant::{OwnedValue, Value};

const PATH: &str = "/org/glove/PolkitAgent";

pub fn main() {
    if std::env::args().any(|a| a == "--pedir") {
        std::process::exit(ask());
    }
    if let Err(e) = serve() {
        eprintln!("glove-senha: {e}");
        std::process::exit(1);
    }
}

// --- o serviço -------------------------------------------------------------------

struct Agent {
    /// A janela aberta de cada pedido (pelo cookie do polkit).
    windows: Arc<Mutex<HashMap<String, Child>>>,
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    /// O polkit pede a senha: abre a janela e só responde quando ela fecha
    /// (sem erro = autenticado; com erro = cancelado ou recusado).
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> zbus::fdo::Result<()> {
        let uids: Vec<u32> = identities
            .iter()
            .filter(|(kind, _)| kind == "unix-user")
            .filter_map(|(_, d)| d.get("uid").and_then(|v| v.downcast_ref::<u32>().ok()))
            .collect();
        // A própria conta, se ela puder; senão a primeira que o polkit aceita (um administrador).
        let me = unsafe { libc::getuid() };
        let uid = if uids.contains(&me) { me } else { *uids.first().ok_or_else(|| zbus::fdo::Error::Failed("ninguém pode autenticar".into()))? };
        let (user, real) = user_names(uid).ok_or_else(|| zbus::fdo::Error::Failed(format!("conta {uid} desconhecida")))?;
        let exe = std::env::current_exe().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let child = Command::new(exe)
            .arg0("glove-senha")
            .arg("--pedir")
            .env("GLOVE_SENHA_MENSAGEM", &message)
            .env("GLOVE_SENHA_ACAO", &action_id)
            .env("GLOVE_SENHA_USUARIO", &user)
            .env("GLOVE_SENHA_NOME", &real)
            .env("GLOVE_SENHA_COOKIE", &cookie)
            .spawn()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        self.windows.lock().unwrap().insert(cookie.clone(), child);
        loop {
            {
                let mut w = self.windows.lock().unwrap();
                let Some(c) = w.get_mut(&cookie) else {
                    return Err(zbus::fdo::Error::Failed("cancelado".into()));
                };
                if let Ok(Some(status)) = c.try_wait() {
                    w.remove(&cookie);
                    return if status.success() { Ok(()) } else { Err(zbus::fdo::Error::Failed("cancelado".into())) };
                }
            }
            async_io::Timer::after(Duration::from_millis(100)).await;
        }
    }

    /// O programa desistiu do pedido: fecha a janela.
    async fn cancel_authentication(&self, cookie: String) {
        if let Some(mut c) = self.windows.lock().unwrap().remove(&cookie) {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// (login, nome completo) da conta.
fn user_names(uid: u32) -> Option<(String, String)> {
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned();
        let gecos = if (*pw).pw_gecos.is_null() { String::new() } else { std::ffi::CStr::from_ptr((*pw).pw_gecos).to_string_lossy().into_owned() };
        let real = gecos.split(',').next().unwrap_or("").trim().to_string();
        Some((name.clone(), if real.is_empty() { name } else { real }))
    }
}

/// A sessão do logind deste processo (o polkit registra o agente por sessão).
fn session_id(conn: &zbus::blocking::Connection) -> zbus::Result<String> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let manager = zbus::blocking::Proxy::new(conn, "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager")?;
    let path: zbus::zvariant::OwnedObjectPath = manager.call("GetSessionByPID", &(std::process::id()))?;
    let session = zbus::blocking::Proxy::new(conn, "org.freedesktop.login1", path, "org.freedesktop.login1.Session")?;
    session.get_property::<String>("Id")
}

fn serve() -> zbus::Result<()> {
    let conn = zbus::blocking::connection::Builder::system()?.serve_at(PATH, Agent { windows: Arc::default() })?.build()?;
    let session = session_id(&conn)?;
    let subject: (&str, HashMap<&str, Value>) = ("unix-session", HashMap::from([("session-id", Value::from(session.as_str()))]));
    let locale = std::env::var("LC_MESSAGES").or_else(|_| std::env::var("LANG")).unwrap_or_else(|_| "C".into());
    // Outro agente já registrado nesta sessão (dentro de outro desktop): o dele vale.
    conn.call_method(
        Some("org.freedesktop.PolicyKit1"),
        "/org/freedesktop/PolicyKit1/Authority",
        Some("org.freedesktop.PolicyKit1.Authority"),
        "RegisterAuthenticationAgent",
        &(subject, locale.as_str(), PATH),
    )?;
    loop {
        std::thread::park();
    }
}

// --- a janela de um pedido ---------------------------------------------------------

/// Onde o polkit põe o ajudante que confere a senha (varia entre as distribuições).
const HELPERS: [&str; 3] = ["/usr/lib/polkit-1/polkit-agent-helper-1", "/usr/libexec/polkit-agent-helper-1", "/usr/lib/policykit-1/polkit-agent-helper-1"];

/// Confere a senha com o ajudante do polkit: Ok(true) autenticado, Ok(false) senha errada.
fn verify(user: &str, cookie: &str, password: &str) -> Result<bool, String> {
    let helper = HELPERS.iter().find(|h| std::path::Path::new(h).exists()).ok_or_else(|| tr!("O ajudante do polkit (polkit-agent-helper-1) não está instalado.").to_string())?;
    let mut c = Command::new(helper).arg(user).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let mut input = c.stdin.take().ok_or("stdin")?;
    let out = BufReader::new(c.stdout.take().ok_or("stdout")?);
    // O cookie vai pela entrada (nunca pela linha de comando, que outros veem).
    writeln!(input, "{cookie}").map_err(|e| e.to_string())?;
    let mut ok = false;
    for line in out.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.starts_with("PAM_PROMPT_ECHO_OFF") || line.starts_with("PAM_PROMPT_ECHO_ON") {
            writeln!(input, "{password}").map_err(|e| e.to_string())?;
        } else if line == "SUCCESS" {
            ok = true;
            break;
        } else if line == "FAILURE" {
            break;
        }
    }
    drop(input);
    let _ = c.wait();
    Ok(ok)
}

struct Ask {
    message: String,
    action: String,
    user: String,
    real: String,
    cookie: String,
    text: String,
    error: Option<String>,
    checking: Option<Receiver<Result<bool, String>>>,
    /// A resposta do ajudante, recebida no tique e tratada no quadro seguinte.
    outcome: Option<Result<bool, String>>,
    result: Arc<Mutex<i32>>,
}

impl App for Ask {
    fn ui(&mut self, ui: &mut Ui) {
        // A resposta do ajudante antes de desenhar (o aviso entra neste quadro).
        if let Some(r) = self.outcome.take() {
            match r {
                Ok(true) => {
                    *self.result.lock().unwrap() = 0;
                    ui.close = true;
                    return;
                }
                Ok(false) => self.error = Some(tr!("Senha incorreta. Tente de novo.").into()),
                Err(e) => self.error = Some(e),
            }
        }
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(238, 238, 240, 255)), (1.0, rgba(226, 226, 230, 255))]));
        icons::draw(ui.c, ui.fonts, Icon::Pref(icons::PrefKind::Security), 20.0, 18.0, 64.0, None);
        let ink = rgba(20, 20, 24, 255);
        let x = 100.0;
        let tw = w - x - 20.0;
        ui.fonts.draw(ui.c, tr!("Autenticação necessária"), x, 34.0, 14.0, true, ink);
        let mut y = 54.0;
        for l in wrap_lines(ui.fonts, &self.message, 12.0, false, tw).into_iter().take(3) {
            ui.fonts.draw(ui.c, &l, x, y, 12.0, false, rgba(50, 50, 56, 255));
            y += 16.0;
        }
        y += 6.0;
        let who = tr!("Digite a senha de {} para continuar.", self.real);
        for l in wrap_lines(ui.fonts, &who, 11.0, false, tw).into_iter().take(2) {
            ui.fonts.draw(ui.c, &l, x, y, 11.0, false, rgba(90, 90, 96, 255));
            y += 15.0;
        }
        y += 8.0;
        ui.fonts.draw(ui.c, tr!("Senha:"), x, y + 15.0, 12.0, false, ink);
        let busy = self.checking.is_some();
        let enter = ui.password_field_in(Rect::new(x + 52.0, y, tw - 52.0, 22.0), &mut self.text, true);
        y += 40.0;
        if let Some(e) = &self.error {
            let e = ui.fonts.ellipsize(e, 11.0, false, tw);
            ui.fonts.draw(ui.c, &e, x, y, 11.0, false, rgba(180, 30, 30, 255));
        } else if busy {
            ui.fonts.draw(ui.c, tr!("Conferindo…"), x, y, 11.0, false, rgba(90, 90, 96, 255));
        }
        let act = ui.fonts.ellipsize(&self.action, 9.5, false, x - 30.0);
        ui.fonts.draw(ui.c, &act, 20.0, h - 18.0, 9.5, false, rgba(140, 140, 146, 255));
        let (mut cancel, mut ok) = (false, false);
        ui.area(Rect::new(w - 220.0, h - 40.0, 200.0, 30.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ui.set_active(!busy && !self.text.is_empty());
                ok = ui.default_button(tr!("Autenticar"));
                ui.set_active(true);
            });
        });
        if cancel || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            *self.result.lock().unwrap() = 1;
            ui.close = true;
            return;
        }
        if (ok || enter) && !busy && !self.text.is_empty() {
            let (tx, rx) = channel();
            let (user, cookie, password) = (self.user.clone(), self.cookie.clone(), std::mem::take(&mut self.text));
            std::thread::spawn(move || {
                let _ = tx.send(verify(&user, &cookie, &password));
            });
            self.checking = Some(rx);
            self.error = None;
        }
    }

    fn tick(&self) -> Option<Duration> {
        self.checking.as_ref().map(|_| Duration::from_millis(100))
    }

    fn on_tick(&mut self) {
        if let Some(r) = self.checking.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.checking = None;
            self.outcome = Some(r);
        }
    }
}

fn ask() -> i32 {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let result = Arc::new(Mutex::new(1));
    let app = Ask {
        message: env("GLOVE_SENHA_MENSAGEM"),
        action: env("GLOVE_SENHA_ACAO"),
        user: env("GLOVE_SENHA_USUARIO"),
        real: env("GLOVE_SENHA_NOME"),
        cookie: env("GLOVE_SENHA_COOKIE"),
        text: String::new(),
        error: None,
        checking: None,
        outcome: None,
        result: result.clone(),
    };
    let opts = WindowOptions { title: tr!("Autenticação").into(), app_id: "glove.Senha".into(), size: (480, 230), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-senha: {e}");
        return 1;
    }
    let code = *result.lock().unwrap();
    code
}
