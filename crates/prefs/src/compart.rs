//! Painel Compartilhamento: nome do computador e os serviços instalados
//! (sessão remota por SSH, arquivos pelo Samba), ligados pelo systemctl.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{paint::draw::*, Rect, Ui};

/// (unidades do systemd possíveis, nome, o que faz).
const SERVICES: [(&[&str], &str, &str); 3] = [
    (&["ssh", "sshd"], "Sessão Remota", "Outros computadores entram neste pelo SSH."),
    (&["smbd", "smb"], "Compartilhamento de Arquivos", "Pastas compartilhadas com Windows e outros (Samba)."),
    (&["cups-browsed"], "Compartilhamento de Impressoras", "Outros computadores usam as impressoras deste."),
];

#[derive(Clone, Debug, PartialEq)]
struct Service {
    unit: String,
    name: &'static str,
    about: &'static str,
    active: bool,
}

fn installed(unit: &str) -> bool {
    Command::new("systemctl").args(["cat", &format!("{unit}.service")]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

fn active(unit: &str) -> bool {
    Command::new("systemctl").args(["is-active", unit]).output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
}

/// Endereço IPv4 da primeira interface conectada (para dizer como entrar).
fn local_ip() -> Option<String> {
    let out = Command::new("hostname").arg("-I").output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().find(|a| a.contains('.')).map(String::from)
}

fn run(args: &[&str]) -> Result<(), String> {
    let out = Command::new(args[0]).args(&args[1..]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let msg = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("o sistema recusou").trim().to_string();
    if msg.contains("nteractive authentication") || msg.contains("ccess denied") || msg.contains("not authorized") {
        return Err("o sistema pediu permissão de administrador e ela não foi dada.".into());
    }
    Err(msg)
}

#[derive(Default)]
pub struct Compart {
    host: String,
    pretty: String,
    edit: String,
    services: Vec<Service>,
    ip: Option<String>,
    loaded: bool,
    busy: bool,
    done: Arc<Mutex<Option<Result<String, String>>>>,
    message: Option<(bool, String)>,
}

impl Compart {
    pub fn refresh(&mut self) {
        let get = |a: &str| Command::new("hostnamectl").arg(a).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        self.host = get("--static");
        self.pretty = get("--pretty");
        self.edit = if self.pretty.is_empty() { self.host.clone() } else { self.pretty.clone() };
        self.services = SERVICES
            .iter()
            .filter_map(|(units, name, about)| units.iter().find(|u| installed(u)).map(|u| Service { unit: u.to_string(), name, about, active: active(u) }))
            .collect();
        self.ip = local_ip();
        self.loaded = true;
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

    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.loaded {
            self.refresh();
        }
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
        const LW: f32 = 170.0;
        ui.form_row("Nome do computador", LW, |ui| {
            ui.horizontal(|ui| {
                ui.text_field(&mut self.edit, 220.0);
                let changed = self.edit.trim() != if self.pretty.is_empty() { &self.host } else { &self.pretty } && !self.edit.trim().is_empty();
                ui.set_active(changed && !self.busy);
                if ui.button("Mudar") {
                    let name = self.edit.trim().to_string();
                    self.start("Nome do computador trocado.".into(), move || run(&["hostnamectl", "set-hostname", "--pretty", &name]));
                }
                ui.set_active(true);
            });
        });
        let t = format!("Na rede: {}.local", self.host);
        ui.form_note(LW, &t);
        ui.space(10.0);
        ui.label("Serviços:");
        let w = ui.available_width();
        let row = 46.0;
        let h = (self.services.len().max(1) as f32 * row + 6.0).max(60.0);
        let r = ui.alloc(w, h);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(rgba(255, 255, 255, 255)));
        if self.services.is_empty() {
            ui.fonts.draw(ui.c, "Nenhum serviço de compartilhamento instalado.", r.x + 12.0, r.y + 22.0, 12.0, false, rgba(90, 90, 96, 255));
            ui.fonts.draw(ui.c, "Instale o OpenSSH (sessão remota) ou o Samba (arquivos) pelo sistema.", r.x + 12.0, r.y + 40.0, 11.0, false, rgba(120, 120, 126, 255));
        }
        let mut toggle = None;
        for (k, s) in self.services.iter().enumerate() {
            let y = r.y + 3.0 + k as f32 * row;
            let rr = Rect::new(r.x + 1.0, y, r.w - 2.0, row);
            // Caixinha de ligado.
            let cb = Rect::new(rr.x + 12.0, y + 9.0, 14.0, 14.0);
            ui.c.fill(rrect(cb.x, cb.y, cb.w, cb.h, 3.0), &solid(if s.active { ui.theme.accent } else { rgba(255, 255, 255, 255) }));
            ui.c.stroke(rrect(cb.x + 0.5, cb.y + 0.5, cb.w - 1.0, cb.h - 1.0, 3.0), &solid(rgba(90, 90, 96, 255)), 1.0);
            if s.active {
                ui.fonts.draw(ui.c, "✓", cb.x + 2.0, cb.y + 12.0, 12.0, true, rgba(255, 255, 255, 255));
            }
            ui.fonts.draw(ui.c, s.name, rr.x + 36.0, y + 20.0, 13.0, true, rgba(20, 20, 20, 255));
            let about = if s.active && s.unit.starts_with("ssh") {
                match &self.ip {
                    Some(ip) => format!("Ligado. Para entrar: ssh {}@{ip}", std::env::var("USER").unwrap_or_default()),
                    None => "Ligado.".to_string(),
                }
            } else {
                s.about.to_string()
            };
            ui.fonts.draw(ui.c, &about, rr.x + 36.0, y + 37.0, 11.0, false, rgba(90, 90, 96, 255));
            if ui.hit(rr).clicked && !self.busy {
                toggle = Some(k);
            }
        }
        if let Some(k) = toggle {
            let s = self.services[k].clone();
            let (verb, ok) = if s.active { ("stop", format!("{} desligado.", s.name)) } else { ("start", format!("{} ligado.", s.name)) };
            let unit = s.unit.clone();
            self.start(ok, move || run(&["systemctl", verb, &unit]));
        }
        ui.space(6.0);
        if self.busy {
            ui.horizontal(|ui| {
                ui.spinner(14.0);
                ui.small("Aplicando…");
            });
        } else if let Some((ok, m)) = self.message.clone() {
            let t = if ok { m } else { format!("Não deu certo: {m}") };
            ui.small(&t);
        }
    }
}
