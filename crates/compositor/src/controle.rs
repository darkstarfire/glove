//! Central de controle (o ícone de dois botões na barra de menu, como nos Mac mais
//! novos): Wi-Fi, Bluetooth e Não perturbe em botões; brilho da tela, luz do teclado
//! (quando o computador tem) e som em trilhos. O estado é lido ao abrir e depois de
//! cada mudança; os comandos rodam em linhas à parte (a tela não trava).

use std::time::{Duration, Instant};

use glove_paint::chrome::{self, ControlInfo, ControlPart};
use smithay::utils::{Logical, Point};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

pub struct ControlPanel {
    /// Canto do conteúdo (sem a sombra).
    pub origin: Point<i32, Logical>,
    pub info: ControlInfo,
    /// O trilho sendo arrastado.
    pub drag: Option<ControlPart>,
    last_set: Instant,
    key: Option<(String, u64)>,
    pub tex: Option<Tex>,
    /// Qual extra da barra abriu (para realçar o ícone).
    pub extra: usize,
}

/// O Bluetooth: Some(ligado), ou None sem adaptador (ou sem o bluetoothctl).
fn read_bluetooth() -> Option<bool> {
    let out = std::process::Command::new("bluetoothctl").arg("show").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.contains("Controller") {
        return None;
    }
    Some(text.lines().any(|l| l.trim() == "Powered: yes"))
}

/// O nome da rede Wi-Fi em uso.
fn wifi_name() -> String {
    glove_config::rede::wifi_list(false).into_iter().find(|w| w.in_use).map(|w| w.ssid).unwrap_or_default()
}

impl Glove {
    fn read_control_info(&self) -> ControlInfo {
        let wifi = crate::extras::read_wifi().map(|(on, _)| (on, if on { wifi_name() } else { String::new() }));
        let brightness = crate::extras::backlight().filter(|b| b.2 > 0).map(|(_, cur, max)| cur as f32 / max as f32);
        let keyboard = crate::extras::kbd_light().map(|(_, cur, max)| cur as f32 / max as f32);
        ControlInfo { wifi, bluetooth: read_bluetooth(), quiet: self.avisos.quiet, brightness, keyboard, volume: crate::extras::read_volume().or(self.extras.volume) }
    }

    /// Abre a central embaixo do ícone (de novo no ícone, fecha).
    pub fn open_control(&mut self, k: usize) {
        if self.control.as_ref().is_some_and(|c| c.extra == k) {
            self.close_control();
            return;
        }
        self.close_menu();
        self.close_volume_slider();
        let info = self.read_control_info();
        let right = self.chrome.extra_ranges.get(k).map(|r| r.1).unwrap_or(0.0) as i32;
        let (sw, _) = self.output_size();
        let x = (right - chrome::CONTROL_W as i32).clamp(4, (sw - chrome::CONTROL_W as i32 - 4).max(4));
        self.control = Some(ControlPanel { origin: (x, glove_paint::theme::MENUBAR_H + 2).into(), info, drag: None, last_set: Instant::now() - Duration::from_secs(1), key: None, tex: None, extra: k });
        self.dirty = true;
    }

    pub fn close_control(&mut self) {
        if self.control.take().is_some() {
            self.dirty = true;
        }
    }

    /// O ponto cai na central? (a parte e o x relativo ao conteúdo; None fora dela).
    pub fn control_hit(&self, pos: Point<f64, Logical>) -> Option<(Option<ControlPart>, f32)> {
        let c = self.control.as_ref()?;
        let (x, y) = ((pos.x - c.origin.x as f64) as f32, (pos.y - c.origin.y as f64) as f32);
        let (parts, h) = chrome::control_layout(&c.info);
        if x < 0.0 || y < 0.0 || x > chrome::CONTROL_W || y > h {
            return None;
        }
        let part = parts.iter().find(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).map(|(p, _)| *p);
        Some((part, x))
    }

    /// Clique numa parte: os botões ligam e desligam; os trilhos começam a arrastar.
    pub fn control_press(&mut self, part: ControlPart, x: f32) {
        let Some(c) = self.control.as_mut() else { return };
        match part {
            ControlPart::Wifi => {
                if let Some((on, _)) = c.info.wifi.clone() {
                    c.info.wifi = Some((!on, String::new()));
                    self.set_wifi(!on);
                }
            }
            ControlPart::Bluetooth => {
                if let Some(on) = c.info.bluetooth {
                    c.info.bluetooth = Some(!on);
                    std::thread::spawn(move || {
                        if !on {
                            // Bloqueado pelo rfkill, o "power on" não funciona.
                            let _ = std::process::Command::new("rfkill").args(["unblock", "bluetooth"]).status();
                        }
                        let _ = std::process::Command::new("bluetoothctl").args(["power", if on { "off" } else { "on" }]).status();
                    });
                }
            }
            ControlPart::Quiet => {
                self.avisos.quiet = !self.avisos.quiet;
                c.info.quiet = self.avisos.quiet;
            }
            _ => {
                c.drag = Some(part);
                self.control_drag(x, false);
            }
        }
        self.dirty = true;
    }

    /// Arrastando um trilho (x relativo ao conteúdo). Os comandos saem no máximo a cada
    /// 40 ms (e sempre ao soltar).
    pub fn control_drag(&mut self, x: f32, done: bool) {
        let Some(c) = self.control.as_mut() else { return };
        let Some(part) = c.drag else { return };
        let (parts, _) = chrome::control_layout(&c.info);
        let Some(rect) = parts.iter().find(|(p, _)| *p == part).map(|(_, r)| *r) else { return };
        let level = chrome::control_level(rect, x);
        let send = done || c.last_set.elapsed() >= Duration::from_millis(40);
        if send {
            c.last_set = Instant::now();
        }
        match part {
            ControlPart::Brightness => {
                c.info.brightness = Some(level);
                if send {
                    if let Some((name, _, max)) = crate::extras::backlight() {
                        let raw = ((level * max as f32).round() as u32).clamp(1, max);
                        crate::extras::set_backlight(&name, raw);
                    }
                }
            }
            ControlPart::Keyboard => {
                c.info.keyboard = Some(level);
                if send {
                    if let Some((name, _, max)) = crate::extras::kbd_light() {
                        let raw = (level * max as f32).round() as u32;
                        std::thread::spawn(move || crate::extras::set_kbd_light(&name, raw));
                    }
                }
            }
            ControlPart::Volume => {
                c.info.volume = Some((level, false));
                if send {
                    self.set_volume(level, false);
                }
            }
            _ => {}
        }
        if done {
            if let Some(c) = self.control.as_mut() {
                c.drag = None;
            }
        }
        self.dirty = true;
    }

    /// A textura da central (refeita só quando o que ela mostra muda).
    pub fn prepare_control(&mut self, sc: f32, tgen: u64) {
        let Some(c) = self.control.as_mut() else { return };
        let key = (format!("{:?}", c.info), tgen);
        if c.key.as_ref() != Some(&key) {
            c.tex = chrome::control_panel(&self.theme, &self.fonts, &c.info, sc).map(to_tex);
            c.key = Some(key);
        }
    }
}
