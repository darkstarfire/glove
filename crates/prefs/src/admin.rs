//! Senha de administrador: uma folha na própria janela e o comando pelo
//! `sudo -S` (a senha vai só para a entrada do sudo, nunca para a linha de
//! comando). O Glove não tem agente do polkit: o pkexec ficaria esperando a
//! senha num terminal que ninguém vê.

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
}

pub enum Answer {
    Waiting,
    Cancel,
    /// Senha já conferida.
    Password(String),
}

impl Prompt {
    pub fn new(what: String) -> Self {
        Self { what, text: String::new(), error: false }
    }

    /// Desenha a folha (abaixo da barra de ferramentas) e trata Enter, OK, Cancelar e Esc.
    pub fn sheet(&mut self, ui: &mut Ui) -> Answer {
        let (sw, sh) = (420.0, 150.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = 78.0;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
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
