//! Painel Bluetooth: ligar o rádio, aparelhos pareados (conectar, desconectar,
//! esquecer) e procurar novos, pelo `bluetoothctl`.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, Color},
    Rect, Ui,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

/// "Device AA:BB:… Nome" → (mac, nome).
pub fn parse_devices(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("Device ")?;
            let (mac, name) = rest.split_once(' ').unwrap_or((rest, ""));
            Some((mac.to_string(), name.trim().to_string()))
        })
        .collect()
}

fn ctl(args: &[&str]) -> Option<String> {
    let out = Command::new("bluetoothctl").args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run(args: &[&str]) -> Result<(), String> {
    let out = Command::new("bluetoothctl").args(args).output().map_err(|e| format!("Não foi possível usar o bluetoothctl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    // O bluetoothctl sai com 0 mesmo quando falha: confere a frase.
    if out.status.success() && !text.contains("Failed") && !text.contains("not available") {
        return Ok(());
    }
    Err(text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("o Bluetooth recusou").trim().to_string())
}

#[derive(Default)]
pub struct Bluetooth {
    /// None = sem adaptador (ou sem o bluetoothctl).
    powered: Option<bool>,
    name: String,
    devices: Vec<Device>,
    sel: Option<usize>,
    loaded: bool,
    busy: Option<String>,
    done: Arc<Mutex<Option<Result<String, String>>>>,
    message: Option<(bool, String)>,
}

impl Bluetooth {
    pub fn refresh(&mut self) {
        self.loaded = true;
        let Some(show) = ctl(&["show"]) else {
            self.powered = None;
            return;
        };
        if !show.contains("Controller") {
            self.powered = None;
            return;
        }
        let get = |k: &str| show.lines().find_map(|l| l.trim().strip_prefix(&format!("{k}: "))).unwrap_or("").to_string();
        self.powered = Some(get("Powered") == "yes");
        self.name = get("Alias");
        let paired = parse_devices(&ctl(&["devices", "Paired"]).unwrap_or_default());
        let all = parse_devices(&ctl(&["devices"]).unwrap_or_default());
        let connected = parse_devices(&ctl(&["devices", "Connected"]).unwrap_or_default());
        let sel_mac = self.sel.and_then(|i| self.devices.get(i)).map(|d| d.mac.clone());
        let mut devs: Vec<Device> = all
            .iter()
            .map(|(mac, name)| Device { mac: mac.clone(), name: if name.is_empty() || name.replace('-', ":") == *mac { "Aparelho sem nome".into() } else { name.clone() }, paired: paired.iter().any(|p| p.0 == *mac), connected: connected.iter().any(|c| c.0 == *mac) })
            .collect();
        // Pareados primeiro; os encontrados na busca depois.
        devs.sort_by_key(|d| (!d.paired, !d.connected, d.name.to_lowercase()));
        self.devices = devs;
        self.sel = sel_mac.and_then(|m| self.devices.iter().position(|d| d.mac == m));
    }

    fn start(&mut self, what: String, ok: String, job: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.busy = Some(what);
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
            self.busy = None;
            self.message = Some(match r {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            });
            self.refresh();
        }
        if self.busy.is_some() {
            ui.wake_after(Duration::from_millis(250));
        }
        let Some(powered) = self.powered else {
            ui.small("Nenhum adaptador Bluetooth encontrado (ou o bluetoothctl não está instalado).");
            return;
        };
        let mut on = powered;
        ui.horizontal(|ui| {
            ui.set_active(self.busy.is_none());
            if ui.checkbox(&mut on, "Bluetooth ligado") {
                let arg = if on { "on" } else { "off" };
                self.start(if on { "Ligando…".into() } else { "Desligando…".into() }, if on { "Bluetooth ligado.".into() } else { "Bluetooth desligado.".into() }, move || run(&["power", arg]));
            }
            ui.set_active(true);
        });
        let t = format!("Este computador aparece como “{}”.", self.name);
        ui.small(&t);
        ui.space(6.0);
        // Lista de aparelhos.
        let w = ui.available_width();
        let (row, lh) = (34.0, 200.0);
        let r = ui.alloc(w, lh);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        if !powered {
            ui.fonts.draw(ui.c, "O Bluetooth está desligado.", r.x + 12.0, r.y + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        } else if self.devices.is_empty() {
            ui.fonts.draw(ui.c, "Nenhum aparelho. Use “Procurar” para achar um por perto.", r.x + 12.0, r.y + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let mut clicked = None;
        for (k, d) in self.devices.iter().enumerate().take(((lh - 2.0) / row) as usize).filter(|_| powered) {
            let y = r.y + 1.0 + k as f32 * row;
            let rr = Rect::new(r.x + 1.0, y, r.w - 2.0, row);
            let sel = self.sel == Some(k);
            if sel {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(ui.theme.accent));
            } else if k % 2 == 1 {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(rgba(237, 243, 254, 255)));
            }
            let ink = if sel { Color::WHITE } else { rgba(20, 20, 20, 255) };
            let light = if d.connected { rgba(80, 190, 70, 255) } else if d.paired { rgba(200, 200, 204, 255) } else { rgba(120, 170, 240, 255) };
            gel_circle(ui.c, rr.x + 16.0, y + row / 2.0, 5.0, light);
            ui.fonts.draw(ui.c, &d.name, rr.x + 30.0, y + 15.0, 12.0, true, ink);
            let state = if d.connected { "Conectado" } else if d.paired { "Pareado, desconectado" } else { "Encontrado por perto" };
            ui.fonts.draw(ui.c, state, rr.x + 30.0, y + 29.0, 10.0, false, if sel { Color::WHITE } else { rgba(100, 100, 106, 255) });
            if ui.hit(rr).clicked {
                clicked = Some(k);
            }
        }
        if let Some(k) = clicked {
            self.sel = Some(k);
        }
        ui.space(6.0);
        let dev = self.sel.and_then(|i| self.devices.get(i)).cloned();
        let idle = self.busy.is_none() && powered;
        let (mut scan, mut connect, mut forget) = (false, false, false);
        ui.horizontal(|ui| {
            ui.set_active(idle);
            scan = ui.button("Procurar");
            ui.set_active(idle && dev.is_some());
            connect = ui.default_button(match &dev {
                Some(d) if d.connected => "Desconectar",
                Some(d) if !d.paired => "Parear",
                _ => "Conectar",
            });
            ui.set_active(idle && dev.as_ref().is_some_and(|d| d.paired));
            forget = ui.button("Esquecer");
            ui.set_active(true);
        });
        if scan {
            self.start("Procurando aparelhos (10 s)…".into(), "Busca concluída.".into(), || run(&["--timeout", "10", "scan", "on"]));
        }
        if let (true, Some(d)) = (connect, dev.clone()) {
            let mac = d.mac.clone();
            if d.connected {
                self.start(format!("Desconectando “{}”…", d.name), "Desconectado.".into(), move || run(&["disconnect", &mac]));
            } else if !d.paired {
                self.start(format!("Pareando “{}”…", d.name), "Pareado e conectado.".into(), move || {
                    run(&["--timeout", "20", "pair", &mac])?;
                    let _ = run(&["trust", &mac]);
                    run(&["--timeout", "15", "connect", &mac])
                });
            } else {
                self.start(format!("Conectando “{}”…", d.name), "Conectado.".into(), move || run(&["--timeout", "15", "connect", &mac]));
            }
        }
        if let (true, Some(d)) = (forget, dev) {
            let mac = d.mac.clone();
            self.start(format!("Esquecendo “{}”…", d.name), "Aparelho esquecido.".into(), move || run(&["remove", &mac]));
        }
        ui.space(4.0);
        if let Some(b) = &self.busy {
            let b = b.clone();
            ui.horizontal(|ui| {
                ui.spinner(14.0);
                ui.small(&b);
            });
        } else if let Some((ok, m)) = self.message.clone() {
            let t = if ok { m } else { format!("Não deu certo: {m}") };
            ui.small(&t);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn le_aparelhos() {
        let d = super::parse_devices("Device D4:17:61:7C:FE:8C POCO X7 Pro\nDevice F4:3D:26:FC:5D:B1 GD108BT5.0\nlixo\n");
        assert_eq!(d, [("D4:17:61:7C:FE:8C".to_string(), "POCO X7 Pro".to_string()), ("F4:3D:26:FC:5D:B1".to_string(), "GD108BT5.0".to_string())]);
    }
}
