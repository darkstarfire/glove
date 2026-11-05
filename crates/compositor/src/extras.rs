//! Extras da barra de menu (volume, bateria, monitor), o controle de volume que
//! desce do ícone, as teclas de volume e brilho e o aviso na tela (OSD).
//!
//! Volume pelo `wpctl` (PipeWire); brilho pelo logind (sem precisar de root);
//! bateria lida de /sys/class/power_supply. Tudo é relido uma vez por minuto
//! (junto com o relógio) e na hora em que o Glove muda algo.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use glove_paint::chrome::{self, Extra, OsdKind};
use smithay::{
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point},
};

use crate::{
    shell::{to_tex, BarTarget, MenuAction, MenuNode, Tex},
    state::Glove,
};
use glove_paint::chrome::MenuItem;

#[derive(Clone, Debug, PartialEq)]
pub struct BatteryInfo {
    pub percent: u8,
    pub charging: bool,
    pub full: bool,
    pub on_ac: bool,
    /// Minutos restantes (descarregando), quando o sistema informa.
    pub minutes_left: Option<u32>,
}

/// Qual extra está em cada posição da barra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraKind {
    Display,
    Volume,
    Battery,
}

pub struct VolumeSlider {
    /// Canto do conteúdo (sem a sombra).
    pub origin: Point<i32, Logical>,
    pub dragging: bool,
    last_set: Instant,
    key: Option<(u32, bool, u64)>,
    pub tex: Option<Tex>,
}

pub struct Osd {
    kind: OsdKind,
    level: f32,
    muted: bool,
    shown: Instant,
    key: Option<(u32, bool, i32, u64)>,
    pub tex: Option<Tex>,
}

#[derive(Default)]
pub struct Extras {
    /// (nível 0 a 1, sem som). None = sem PipeWire.
    pub volume: Option<(f32, bool)>,
    pub battery: Option<BatteryInfo>,
    pub slider: Option<VolumeSlider>,
    pub osd: Option<Osd>,
    osd_timer: bool,
}

/// O aviso fica 1,2 s e some em 0,3 s.
const OSD_HOLD: Duration = Duration::from_millis(1200);
const OSD_FADE: Duration = Duration::from_millis(300);
/// 16 passos, como os quadradinhos do aviso.
const STEP: f32 = 1.0 / 16.0;
const SINK: &str = "@DEFAULT_AUDIO_SINK@";

// ---------------------------------------------------------------------------
// Leitura do sistema
// ---------------------------------------------------------------------------

/// "Volume: 0.71 [MUTED]" → (0.71, true).
pub fn parse_wpctl(out: &str) -> Option<(f32, bool)> {
    let rest = out.trim().strip_prefix("Volume:")?.trim();
    let level: f32 = rest.split_whitespace().next()?.parse().ok()?;
    Some((level.clamp(0.0, 1.5), rest.contains("[MUTED]")))
}

fn read_volume() -> Option<(f32, bool)> {
    let out = Command::new("wpctl").args(["get-volume", SINK]).output().ok()?;
    parse_wpctl(&String::from_utf8_lossy(&out.stdout))
}

fn read_num(p: &Path) -> Option<f64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn read_str(p: &Path) -> String {
    std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Bateria (a primeira presente) e se o adaptador está ligado.
pub fn read_battery(root: &Path) -> Option<BatteryInfo> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    let on_ac = entries.iter().any(|d| read_str(&d.join("type")) == "Mains" && read_str(&d.join("online")) == "1");
    let bat = entries.iter().find(|d| read_str(&d.join("type")) == "Battery" && read_str(&d.join("present")) != "0")?;
    let percent = read_num(&bat.join("capacity"))?.clamp(0.0, 100.0) as u8;
    let status = read_str(&bat.join("status"));
    let charging = status == "Charging";
    let full = status == "Full" || (on_ac && percent >= 99 && !charging);
    let minutes_left = if status == "Discharging" {
        let left = read_num(&bat.join("energy_now")).zip(read_num(&bat.join("power_now"))).or_else(|| read_num(&bat.join("charge_now")).zip(read_num(&bat.join("current_now"))));
        left.filter(|(_, rate)| *rate > 0.0).map(|(now, rate)| (now / rate * 60.0) as u32)
    } else {
        None
    };
    Some(BatteryInfo { percent, charging, full, on_ac: on_ac || charging, minutes_left })
}

fn power_supply_root() -> PathBuf {
    std::env::var_os("GLOVE_POWER_SUPPLY").map(PathBuf::from).unwrap_or_else(|| "/sys/class/power_supply".into())
}

/// Tela com luz de fundo ajustável: (nome, atual, máximo).
fn backlight() -> Option<(String, u32, u32)> {
    let dir = std::fs::read_dir("/sys/class/backlight").ok()?.flatten().next()?.path();
    let name = dir.file_name()?.to_string_lossy().into_owned();
    Some((name, read_num(&dir.join("brightness"))? as u32, read_num(&dir.join("max_brightness"))? as u32))
}

/// "Restam cerca de 2:05".
fn format_left(min: u32) -> String {
    format!("Restam cerca de {}:{:02}", min / 60, min % 60)
}

/// Resoluções distintas (a de maior frequência de cada uma), da maior para a menor.
pub fn distinct_modes(modes: &[String]) -> Vec<(u32, u32, u32, String)> {
    let mut out: Vec<(u32, u32, u32, String)> = Vec::new();
    for m in modes {
        let Some((w, h, hz)) = glove_config::parse_mode(m) else { continue };
        match out.iter_mut().find(|(a, b, _, _)| *a == w && *b == h) {
            Some(e) if hz > e.2 => *e = (w, h, hz, m.clone()),
            Some(_) => {}
            None => out.push((w, h, hz, m.clone())),
        }
    }
    out.sort_by(|a, b| (b.0 * b.1).cmp(&(a.0 * a.1)).then(b.0.cmp(&a.0)));
    out
}

// ---------------------------------------------------------------------------
// No compositor
// ---------------------------------------------------------------------------

impl Glove {
    /// Relê volume e bateria (na partida e uma vez por minuto).
    pub fn refresh_extras(&mut self) {
        let vol = read_volume();
        let bat = read_battery(&power_supply_root());
        if vol != self.extras.volume || bat != self.extras.battery {
            self.extras.volume = vol;
            self.extras.battery = bat;
            self.dirty = true;
        }
    }

    /// Extras na ordem da barra (da esquerda para a direita, antes do relógio).
    pub fn extra_kinds(&self) -> Vec<ExtraKind> {
        let mut v = vec![ExtraKind::Display];
        if self.extras.volume.is_some() {
            v.push(ExtraKind::Volume);
        }
        if self.extras.battery.is_some() {
            v.push(ExtraKind::Battery);
        }
        v
    }

    pub fn extras_for_bar(&self) -> Vec<Extra> {
        self.extra_kinds()
            .into_iter()
            .map(|k| match k {
                ExtraKind::Display => Extra::Display,
                ExtraKind::Volume => {
                    let (level, muted) = self.extras.volume.unwrap_or((0.0, true));
                    Extra::Volume { level, muted }
                }
                ExtraKind::Battery => {
                    let b = self.extras.battery.clone().unwrap();
                    Extra::Battery { percent: b.percent, charging: b.charging, show_percent: self.config.battery_percent }
                }
            })
            .collect()
    }

    /// Clique num extra: o controle de volume ou o menu do extra.
    pub fn open_extra(&mut self, k: usize) {
        let Some(kind) = self.extra_kinds().get(k).copied() else { return };
        match kind {
            ExtraKind::Volume => {
                self.close_menu();
                self.extras.volume = read_volume().or(self.extras.volume);
                let x = self.chrome.extra_ranges.get(k).map(|r| r.0).unwrap_or(0.0) as i32;
                let (sw, _) = self.output_size();
                let x = x.min(sw - chrome::VOLUME_SLIDER_W as i32);
                self.extras.slider = Some(VolumeSlider { origin: (x, glove_paint::theme::MENUBAR_H).into(), dragging: false, last_set: Instant::now() - Duration::from_secs(1), key: None, tex: None });
                self.dirty = true;
            }
            ExtraKind::Battery => {
                let nodes = self.battery_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
            ExtraKind::Display => {
                let nodes = self.display_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
        }
    }

    pub fn close_volume_slider(&mut self) {
        if self.extras.slider.take().is_some() {
            self.dirty = true;
        }
    }

    /// Índice do extra de volume na barra (para realçar o ícone com o controle aberto).
    pub fn volume_extra_index(&self) -> Option<usize> {
        self.extra_kinds().iter().position(|k| *k == ExtraKind::Volume)
    }

    fn battery_menu(&self) -> Vec<MenuNode> {
        let Some(b) = self.extras.battery.clone() else { return Vec::new() };
        let info = |t: String| MenuNode::new(MenuItem::item(t).disabled(), None);
        let mut v = vec![if b.full {
            info("Bateria carregada".into())
        } else if b.charging {
            info(format!("Carregando: {}%", b.percent))
        } else {
            info(format!("Bateria: {}%", b.percent))
        }];
        if let Some(m) = b.minutes_left {
            v.push(info(format_left(m)));
        }
        v.push(info(format!("Fonte de energia: {}", if b.on_ac { "Adaptador" } else { "Bateria" })));
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::check("Mostrar porcentagem", self.config.battery_percent), Some(MenuAction::ToggleBatteryPercent)));
        v.push(MenuNode::sep());
        // O painel Economizador de Energia chega na etapa 7.4.
        v.push(MenuNode::new(MenuItem::item("Abrir Economizador de Energia…").disabled(), None));
        v
    }

    fn display_menu(&self) -> Vec<MenuNode> {
        let info = glove_config::OutputsInfo::load().unwrap_or_default();
        let mut v = Vec::new();
        let out = info.outputs.iter().find(|o| o.active).or(info.outputs.first());
        match out {
            Some(o) => {
                let name = if info.nested { "Janela do Glove".to_string() } else if o.description.is_empty() { o.name.clone() } else { o.description.clone() };
                v.push(MenuNode::new(MenuItem::item(name).disabled(), None));
                let current = glove_config::parse_mode(&o.current).map(|(w, h, _)| (w, h));
                for (w, h, hz, mode) in distinct_modes(&o.modes).into_iter().take(12) {
                    let label = if hz > 0 { format!("{w} × {h}, {hz} Hz") } else { format!("{w} × {h}") };
                    let mut it = MenuItem::check(format!("  {label}"), current == Some((w, h)));
                    if info.nested {
                        it = it.disabled();
                    }
                    v.push(MenuNode::new(it, Some(MenuAction::SetMode(o.name.clone(), mode))));
                }
            }
            None => v.push(MenuNode::new(MenuItem::item("Nenhum monitor").disabled(), None)),
        }
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item("Abrir Preferências de Monitor…"), Some(MenuAction::OpenPrefs("monitor".into()))));
        v
    }

    // -----------------------------------------------------------------------
    // Volume e brilho
    // -----------------------------------------------------------------------

    /// Muda o volume (0 a 1); com som desligado e nível > 0, liga o som.
    pub fn set_volume(&mut self, level: f32, show_osd: bool) {
        let Some((_, muted)) = self.extras.volume else { return };
        let level = level.clamp(0.0, 1.0);
        let _ = Command::new("wpctl").args(["set-volume", "-l", "1.0", SINK, &format!("{level:.3}")]).spawn();
        let muted = if muted && level > 0.0 {
            let _ = Command::new("wpctl").args(["set-mute", SINK, "0"]).spawn();
            false
        } else {
            muted
        };
        self.extras.volume = Some((level, muted));
        self.dirty = true;
        if show_osd {
            self.show_osd(OsdKind::Volume, level, muted);
        }
    }

    pub fn toggle_mute(&mut self) {
        let Some((level, muted)) = self.extras.volume else { return };
        let _ = Command::new("wpctl").args(["set-mute", SINK, if muted { "0" } else { "1" }]).spawn();
        self.extras.volume = Some((level, !muted));
        self.dirty = true;
        self.show_osd(OsdKind::Volume, level, !muted);
    }

    pub fn step_volume(&mut self, dir: f32) {
        if self.extras.volume.is_none() {
            self.extras.volume = read_volume();
        }
        let Some((level, _)) = self.extras.volume else { return };
        // Arredonda para o passo, para os quadradinhos andarem um por vez.
        let next = ((level / STEP).round() + dir) * STEP;
        self.set_volume(next, true);
    }

    /// Brilho pelo logind (a sessão ativa pode mudar sem root).
    pub fn step_brightness(&mut self, dir: f32) {
        let Some((name, cur, max)) = backlight() else { return };
        if max == 0 {
            return;
        }
        let level = cur as f32 / max as f32;
        let next = (((level / STEP).round() + dir) * STEP).clamp(STEP / 2.0, 1.0);
        let raw = ((next * max as f32).round() as u32).clamp(1, max);
        let _ = Command::new("busctl")
            .args(["call", "org.freedesktop.login1", "/org/freedesktop/login1/session/auto", "org.freedesktop.login1.Session", "SetBrightness", "ssu", "backlight", &name, &raw.to_string()])
            .spawn();
        self.show_osd(OsdKind::Brightness, raw as f32 / max as f32, false);
    }

    // -----------------------------------------------------------------------
    // Controle de volume (arrastar)
    // -----------------------------------------------------------------------

    /// Ponto dentro do controle? Devolve o y relativo ao conteúdo.
    pub fn volume_slider_y(&self, pos: Point<f64, Logical>) -> Option<f32> {
        let s = self.extras.slider.as_ref()?;
        let (x, y) = (pos.x - s.origin.x as f64, pos.y - s.origin.y as f64);
        (x >= 0.0 && y >= 0.0 && x < chrome::VOLUME_SLIDER_W as f64 && y < chrome::VOLUME_SLIDER_H as f64).then_some(y as f32)
    }

    pub fn volume_slider_drag(&mut self, y: f32, done: bool) {
        let Some(s) = &mut self.extras.slider else { return };
        let level = chrome::volume_slider_level(y);
        // Não dispara um wpctl a cada movimento: no máximo a cada 40 ms (e sempre no fim).
        if done || s.last_set.elapsed() >= Duration::from_millis(40) {
            s.last_set = Instant::now();
            self.set_volume(level, false);
        } else if let Some((_, m)) = self.extras.volume {
            self.extras.volume = Some((level, m));
            self.dirty = true;
        }
    }

    // -----------------------------------------------------------------------
    // Aviso na tela
    // -----------------------------------------------------------------------

    fn show_osd(&mut self, kind: OsdKind, level: f32, muted: bool) {
        self.extras.osd = Some(Osd { kind, level, muted, shown: Instant::now(), key: None, tex: None });
        self.dirty = true;
        if !self.extras.osd_timer {
            self.extras.osd_timer = true;
            self.loop_handle
                .insert_source(Timer::from_duration(Duration::from_millis(50)), |_, _, data| {
                    let st = &mut data.state;
                    st.dirty = true;
                    let alive = st.extras.osd.as_ref().is_some_and(|o| o.shown.elapsed() < OSD_HOLD + OSD_FADE);
                    if alive {
                        TimeoutAction::ToDuration(Duration::from_millis(50))
                    } else {
                        st.extras.osd = None;
                        st.extras.osd_timer = false;
                        TimeoutAction::Drop
                    }
                })
                .ok();
        }
    }

    /// Posição do aviso (canto de cima, em coordenadas lógicas).
    pub fn osd_pos(&self) -> Point<i32, Logical> {
        let (w, h) = self.output_size();
        let s = chrome::OSD_SIZE as i32;
        ((w - s) / 2, (h - s) / 2 + h / 6).into()
    }

    pub fn prepare_extras(&mut self) {
        let sc = self.scale() as f32;
        let tgen = self.theme_gen;
        if let Some(o) = &mut self.extras.osd {
            let t = o.shown.elapsed();
            let alpha = if t < OSD_HOLD { 1.0 } else { 1.0 - (t - OSD_HOLD).as_secs_f32() / OSD_FADE.as_secs_f32() };
            let a = (alpha.clamp(0.0, 1.0) * 10.0).round() as i32;
            let key = ((o.level * 1000.0) as u32, o.muted, a, tgen);
            if o.key != Some(key) {
                o.tex = chrome::osd(o.kind, o.level, o.muted, a as f32 / 10.0, sc).map(to_tex);
                o.key = Some(key);
            }
        }
        let (level, muted) = self.extras.volume.unwrap_or((0.0, true));
        if let Some(s) = &mut self.extras.slider {
            let key = ((level * 1000.0) as u32, muted, tgen);
            if s.key != Some(key) {
                s.tex = chrome::volume_slider(&self.theme, level, muted, sc).map(to_tex);
                s.key = Some(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpctl() {
        assert_eq!(parse_wpctl("Volume: 0.71 [MUTED]\n"), Some((0.71, true)));
        assert_eq!(parse_wpctl("Volume: 0.40"), Some((0.40, false)));
        assert_eq!(parse_wpctl("nada"), None);
    }

    #[test]
    fn bateria() {
        let dir = std::env::temp_dir().join(format!("glove-bat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let w = |p: &str, v: &str| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, v).unwrap();
        };
        w("AC/type", "Mains\n");
        w("AC/online", "0\n");
        w("BAT0/type", "Battery\n");
        w("BAT0/present", "1\n");
        w("BAT0/capacity", "42\n");
        w("BAT0/status", "Discharging\n");
        w("BAT0/energy_now", "20000000\n");
        w("BAT0/power_now", "10000000\n");
        let b = read_battery(&dir).unwrap();
        assert_eq!((b.percent, b.charging, b.on_ac, b.minutes_left), (42, false, false, Some(120)));
        assert_eq!(format_left(125), "Restam cerca de 2:05");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolucoes() {
        let m: Vec<String> = ["1920x1080@60", "1920x1080@144", "1280x720@60", "2560x1440@60"].iter().map(|s| s.to_string()).collect();
        let d = distinct_modes(&m);
        assert_eq!(d.iter().map(|x| x.3.as_str()).collect::<Vec<_>>(), ["2560x1440@60", "1920x1080@144", "1280x720@60"]);
    }
}
