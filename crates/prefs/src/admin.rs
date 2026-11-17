//! Senha de administrador: a janela de senha do sistema (o `glove-senha --sudo`, a mesma
//! do polkit) e o comando pelo `sudo -S` (a senha vai só para a entrada do sudo, nunca
//! para a linha de comando). Sem o glove-senha instalado, uma folha na própria janela.

use glove_config::tr;
use glove_ui::{paint::draw::*, Rect, Ui};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// Pedido de senha aberto: o que vai ser feito (frase) e o que foi digitado.
pub struct Prompt {
    pub what: String,
    text: String,
    error: bool,
    /// A janela de senha aberta (sem ela, a folha).
    window: Option<glove_config::senha::Pedido>,
}

pub enum Answer {
    Waiting,
    Cancel,
    /// Senha já conferida.
    Password(String),
}

impl Prompt {
    pub fn new(what: String) -> Self {
        let window = glove_config::senha::Pedido::novo(&what);
        Self { what, text: String::new(), error: false, window }
    }

    /// A resposta da janela de senha (que já confere a senha); sem ela, a folha
    /// (abaixo da barra de ferramentas), que trata Enter, OK, Cancelar e Esc.
    pub fn sheet(&mut self, ui: &mut Ui) -> Answer {
        if let Some(w) = &mut self.window {
            use glove_config::senha::Resposta;
            return match w.resposta() {
                Resposta::Esperando => {
                    ui.wake_after(std::time::Duration::from_millis(150));
                    Answer::Waiting
                }
                Resposta::Cancelado => Answer::Cancel,
                Resposta::Senha(p) => Answer::Password(p),
            };
        }
        let (sw, sh) = (420.0, 150.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = 78.0;
        // A folha: sombra suave em volta e os cantos de baixo arredondados.
        ui.sheet_panel(Rect::new(x, y, sw, sh));
        let ink = rgba(20, 20, 20, 255);
        ui.fonts.draw(ui.c, tr!("Digite a sua senha de administrador"), x + 20.0, y + 26.0, 13.0, true, ink);
        let what = ui.fonts.ellipsize(&self.what, 11.5, false, sw - 40.0);
        ui.fonts.draw(ui.c, &what, x + 20.0, y + 44.0, 11.5, false, rgba(80, 80, 86, 255));
        ui.fonts.draw(ui.c, tr!("Senha:"), x + 20.0, y + 76.0, 13.0, false, ink);
        let enter = ui.password_field_in(Rect::new(x + 80.0, y + 60.0, sw - 100.0, 22.0), &mut self.text, true);
        if self.error {
            ui.fonts.draw(ui.c, tr!("Senha incorreta. Tente de novo."), x + 80.0, y + 98.0, 11.0, false, rgba(180, 30, 30, 255));
        }
        let (mut cancel, mut ok) = (false, false);
        ui.area(Rect::new(x + sw - 210.0, y + sh - 40.0, 200.0, 30.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok = ui.default_button("OK");
            });
        });
        if cancel || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            return Answer::Cancel;
        }
        if (ok || enter) && !self.text.is_empty() {
            if password_ok(&self.text) {
                return Answer::Password(std::mem::take(&mut self.text));
            }
            self.error = true;
            self.text.clear();
        }
        Answer::Waiting
    }
}

/// O que o sudo responde a esta conta.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sudo {
    /// Funciona sem senha (o Glove ao vivo, por exemplo).
    Free,
    /// Funciona com a senha da conta.
    Password,
    /// A conta não pode usar o sudo.
    No,
}

/// Pergunta ao próprio sudo (e não aos grupos: a conta pode estar no sudoers pelo nome).
pub fn sudo_state() -> Sudo {
    let Ok(o) = Command::new("sudo").args(["-n", "-v"]).env("LC_ALL", "C").stdin(Stdio::null()).output() else { return Sudo::No };
    if o.status.success() {
        Sudo::Free
    } else if String::from_utf8_lossy(&o.stderr).contains("password is required") {
        Sudo::Password
    } else {
        Sudo::No
    }
}

/// Confere a senha (sudo -v), sem aproveitar uma confirmação anterior.
fn password_ok(password: &str) -> bool {
    let Ok(mut c) = Command::new("sudo").args(["-S", "-k", "-v", "-p", ""]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
    if let Some(mut i) = c.stdin.take() {
        let _ = writeln!(i, "{password}");
    }
    c.wait().is_ok_and(|s| s.success())
}

/// Roda o comando como administrador numa linha à parte; o resultado chega pelo
/// canal. `password`: None quando o sudo não pede senha.
pub fn run(args: Vec<String>, password: Option<String>) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_now(&args, password.as_deref()));
    });
    rx
}

fn run_now(args: &[String], password: Option<&str>) -> Result<(), String> {
    let mut c = Command::new("sudo");
    match password {
        Some(_) => c.args(["-S", "-k", "-p", ""]),
        None => c.arg("-n"),
    };
    let mut child = c.args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("sudo: {e}"))?;
    if let Some(mut i) = child.stdin.take() {
        if let Some(p) = password {
            let _ = writeln!(i, "{p}");
        }
    }
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string())
    }
}

/// Andamento de um comando de administrador: cada linha que ele escreve, e o fim.
pub enum Step {
    Line(String),
    Done(Result<(), String>),
}

/// Como `run`, mas manda cada linha da saída (padrão e de erro) enquanto o comando
/// roda: para a barra de progresso das atualizações (o apt escreve o andamento).
pub fn run_stream(args: Vec<String>, password: Option<String>) -> Receiver<Step> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut c = Command::new("sudo");
        match password {
            Some(_) => c.args(["-S", "-k", "-p", ""]),
            None => c.arg("-n"),
        };
        let spawned = c.args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Step::Done(Err(format!("sudo: {e}"))));
                return;
            }
        };
        if let Some(mut i) = child.stdin.take() {
            if let Some(p) = &password {
                let _ = writeln!(i, "{p}");
            }
        }
        // O erro numa linha à parte (guardando a última, para a mensagem).
        let last_err = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let err_thread = child.stderr.take().map(|e| {
            let (tx, last) = (tx.clone(), last_err.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        *last.lock().unwrap() = line.trim().to_string();
                    }
                    let _ = tx.send(Step::Line(line));
                }
            })
        });
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(Step::Line(line));
            }
        }
        if let Some(t) = err_thread {
            let _ = t.join();
        }
        let ok = child.wait().is_ok_and(|s| s.success());
        let msg = last_err.lock().unwrap().clone();
        let _ = tx.send(Step::Done(if ok { Ok(()) } else { Err(msg) }));
    });
    rx
}
