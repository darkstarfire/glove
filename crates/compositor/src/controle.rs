//! Central de controle (o ícone de dois botões na barra de menu, como nos Mac mais
//! novos): Wi-Fi, Bluetooth e Não perturbe em botões; brilho da tela, luz do teclado
//! (quando o computador tem) e som em trilhos.
//!
//! Nada aqui roda um comando de fora na linha principal: a central abre na hora com o
//! que já se sabe (o que a barra já leu, os arquivos de brilho, o que se viu da última
//! vez) e o resto (Wi-Fi, Bluetooth, volume) é lido numa linha à parte e chega depois.
//! Um `bluetoothctl show` sem o serviço de Bluetooth rodando nunca termina; chamado da
//! linha principal, congelava a tela inteira ao abrir a central.

use std::time::{Duration, Instant};

use glove_paint::chrome::{self, ControlInfo, ControlPart};
use smithay::{
    reexports::calloop::channel::{channel, Event as ChannelEvent, Sender},
    utils::{Logical, Point},
};

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
    /// No que o usuário já mexeu (Wi-Fi, Bluetooth, volume): o que foi lido antes disso
    /// não vale mais para essa parte.
    touched: [bool; 3],
}

/// O que depende de comandos de fora, lido numa linha à parte.
pub struct SlowInfo {
    wifi: Option<(bool, String)>,
    bluetooth: Option<bool>,
    volume: Option<(f32, bool)>,
}

/// O que se viu na última leitura (a central abre com isto, antes da leitura nova).
#[derive(Default)]
pub struct ControlSeen {
    wifi_name: String,
    bluetooth: Option<bool>,
    /// Há uma leitura em curso.
    reading: bool,
    tx: Option<Sender<SlowInfo>>,
}

/// Limite dos comandos do Bluetooth: sem o serviço, o `bluetoothctl` espera para sempre.
const BLUETOOTH_LIMIT: Duration = Duration::from_secs(3);

/// O Bluetooth: Some(ligado), ou None sem adaptador (ou sem o bluetoothctl).
fn read_bluetooth() -> Option<bool> {
    let out = glove_config::comando::output_within(std::process::Command::new("bluetoothctl").arg("show"), BLUETOOTH_LIMIT)?;
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

/// Wi-Fi, Bluetooth e volume, pelos comandos de fora (demora; só numa linha à parte).
fn read_slow() -> SlowInfo {
    let wifi = crate::extras::read_wifi().map(|(on, _)| (on, if on { wifi_name() } else { String::new() }));
    SlowInfo { wifi, bluetooth: read_bluetooth(), volume: crate::extras::read_volume() }
}

impl Glove {
    /// O que dá para mostrar na hora, sem rodar nenhum comando: o Wi-Fi e o volume que a
    /// barra já leu, o brilho e a luz do teclado (arquivos) e o Bluetooth da última leitura.
    fn quick_control_info(&self) -> ControlInfo {
        let wifi = self.extras.wifi.map(|(on, _)| (on, if on { self.control_seen.wifi_name.clone() } else { String::new() }));
        let brightness = crate::extras::backlight().filter(|b| b.2 > 0).map(|(_, cur, max)| cur as f32 / max as f32);
        let keyboard = crate::extras::kbd_light().map(|(_, cur, max)| cur as f32 / max as f32);
        ControlInfo { wifi, bluetooth: self.control_seen.bluetooth, quiet: self.avisos.quiet, brightness, keyboard, volume: self.extras.volume }
    }

    /// Lê Wi-Fi, Bluetooth e volume numa linha à parte; o resultado chega em
    /// `control_arrived`. Uma leitura por vez.
    pub fn refresh_control(&mut self) {
        if self.control_seen.reading {
            return;
        }
        let tx = match &self.control_seen.tx {
            Some(tx) => tx.clone(),
            None => {
                let (tx, rx) = channel::<SlowInfo>();
                let _ = self.loop_handle.insert_source(rx, |ev, _, data| {
                    if let ChannelEvent::Msg(info) = ev {
                        data.state.control_arrived(info);
                    }
                });
                self.control_seen.tx = Some(tx.clone());
                tx
            }
        };
        self.control_seen.reading = true;
        std::thread::spawn(move || {
            let _ = tx.send(read_slow());
        });
    }

    /// A leitura chegou: guarda para a próxima abertura e, se a central está aberta e o
    /// usuário ainda não mexeu nela, mostra.
    fn control_arrived(&mut self, s: SlowInfo) {
        self.control_seen.reading = false;
        let [wifi, bluetooth, volume] = self.control.as_ref().map_or([false; 3], |c| c.touched);
        if !wifi {
            self.control_seen.wifi_name = s.wifi.as_ref().map(|w| w.1.clone()).unwrap_or_default();
        }
        if !bluetooth {
            self.control_seen.bluetooth = s.bluetooth;
        }
        if !volume && s.volume.is_some() && s.volume != self.extras.volume {
            self.extras.volume = s.volume;
        }
        if let Some(c) = self.control.as_mut() {
            if !wifi {
                c.info.wifi = s.wifi;
            }
            if !bluetooth {
                c.info.bluetooth = s.bluetooth;
            }
            if !volume {
                c.info.volume = s.volume.or(c.info.volume);
            }
        }
        self.dirty = true;
    }

    /// Abre a central embaixo do ícone (de novo no ícone, fecha).
    pub fn open_control(&mut self, k: usize) {
        if self.control.as_ref().is_some_and(|c| c.extra == k) {
            self.close_control();
            return;
        }
        self.close_menu();
        self.close_volume_slider();
        let info = self.quick_control_info();
        let right = self.chrome.extra_ranges.get(k).map(|r| r.1).unwrap_or(0.0) as i32;
        let (sw, _) = self.output_size();
        let x = (right - chrome::CONTROL_W as i32).clamp(4, (sw - chrome::CONTROL_W as i32 - 4).max(4));
        let origin = self.shift_point((x, glove_paint::theme::MENUBAR_H + 2).into(), chrome::CONTROL_W as i32);
        self.control = Some(ControlPanel { origin, info, drag: None, last_set: Instant::now() - Duration::from_secs(1), key: None, tex: None, extra: k, touched: [false; 3] });
        self.refresh_control();
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
            ControlPart::Wifi => c.touched[0] = true,
            ControlPart::Bluetooth => c.touched[1] = true,
            ControlPart::Volume => c.touched[2] = true,
            _ => {}
        }
        match part {
            ControlPart::Wifi => {
                if let Some((on, _)) = c.info.wifi.clone() {
                    c.info.wifi = Some((!on, String::new()));
                    self.control_seen.wifi_name.clear();
                    self.set_wifi(!on);
                }
            }
            ControlPart::Bluetooth => {
                if let Some(on) = c.info.bluetooth {
                    c.info.bluetooth = Some(!on);
                    self.control_seen.bluetooth = Some(!on);
                    std::thread::spawn(move || {
                        use glove_config::comando::output_within;
                        if !on {
                            // Bloqueado pelo rfkill, o "power on" não funciona.
                            let _ = output_within(std::process::Command::new("rfkill").args(["unblock", "bluetooth"]), BLUETOOTH_LIMIT);
                        }
                        let _ = output_within(std::process::Command::new("bluetoothctl").args(["power", if on { "off" } else { "on" }]), Duration::from_secs(15));
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
