//! Extras da barra de menu (volume, bateria, monitor), o controle de volume que
//! desce do ícone, as teclas de volume e brilho e o aviso na tela (OSD).
//!
//! Volume pelo `wpctl` (PipeWire); brilho pelo logind (sem precisar de root);
//! bateria lida de /sys/class/power_supply. Tudo é relido uma vez por minuto
//! (junto com o relógio) e na hora em que o Glove muda algo.

use glove_config::tr;
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
    Input,
    Display,
    Wifi,
    Volume,
    Battery,
    Clock,
    Notifications,
    User,
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
    /// Brilho de antes de baixar por causa da bateria.
    undimmed: Option<u32>,
    /// Wi-Fi: (ligado, sinal da rede em uso). None = sem placa Wi-Fi ou sem NetworkManager.
    pub wifi: Option<(bool, Option<u8>)>,
}

/// Estado do Wi-Fi para o ícone da barra (lê o NetworkManager).
fn read_wifi() -> Option<(bool, Option<u8>)> {
    let devs = glove_config::rede::devices()?;
    if !devs.iter().any(|d| d.kind == "wifi") {
        return None;
    }
    let on = glove_config::rede::wifi_enabled().unwrap_or(false);
    let signal = if on { glove_config::rede::wifi_list(false).into_iter().find(|w| w.in_use).map(|w| w.signal) } else { None };
    Some((on, signal))
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

/// Muda o brilho pelo logind (sem root).
fn set_backlight(name: &str, raw: u32) {
    let _ = Command::new("busctl")
        .args(["call", "org.freedesktop.login1", "/org/freedesktop/login1/session/auto", "org.freedesktop.login1.Session", "SetBrightness", "ssu", "backlight", name, &raw.to_string()])
        .spawn();
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
    tr!("Restam cerca de {}:{}", min / 60, format!("{:02}", min % 60))
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
        let wifi = if self.greeter { None } else { read_wifi() };
        if wifi != self.extras.wifi {
            self.extras.wifi = wifi;
            self.dirty = true;
        }
        // Saiu da tomada: o brilho baixa um pouco; voltou: volta ao de antes.
        let was_ac = self.extras.battery.as_ref().map(|b| b.on_ac);
        if let (Some(was), Some(now)) = (was_ac, bat.as_ref().map(|b| b.on_ac)) {
            if was != now && self.config.energy.dim_on_battery && !self.nested {
                self.dim_for_battery(!now);
            }
        }
        if vol != self.extras.volume || bat != self.extras.battery {
            self.extras.volume = vol;
            self.extras.battery = bat;
            self.dirty = true;
        }
    }

    /// Extras na ordem da barra (da esquerda para a direita, antes do relógio).
    pub fn extra_kinds(&self) -> Vec<ExtraKind> {
        let mut v = Vec::new();
        if self.config.keyboard.show_input_menu && !self.config.keyboard.input_menu.is_empty() {
            v.push(ExtraKind::Input);
        }
        if self.config.monitor.menu_extra {
            v.push(ExtraKind::Display);
        }
        if self.extras.wifi.is_some() && self.config.wifi_menu {
            v.push(ExtraKind::Wifi);
        }
        if self.extras.volume.is_some() && self.config.sound.menu_extra {
            v.push(ExtraKind::Volume);
        }
        if self.extras.battery.is_some() && self.config.energy.battery_menu {
            v.push(ExtraKind::Battery);
        }
        if self.config.clock.show {
            v.push(ExtraKind::Clock);
        }
        if self.avisos.conn.is_some() && !self.greeter {
            v.push(ExtraKind::Notifications);
        }
        if self.config.user_menu && !self.greeter {
            v.push(ExtraKind::User);
        }
        v
    }

    pub fn extras_for_bar(&self) -> Vec<Extra> {
        self.extra_kinds()
            .into_iter()
            .map(|k| match k {
                ExtraKind::Display => Extra::Display,
                ExtraKind::Input => Extra::Input { badge: glove_config::layout_badge(&self.config.keyboard.layout, &self.config.keyboard.variant) },
                ExtraKind::Clock => {
                    let c = &self.config.clock;
                    let (.., h, m, _) = crate::shell::local_now();
                    Extra::Clock { text: crate::shell::clock_text(c), analog: c.analog.then_some((h as u8, m as u8)) }
                }
                ExtraKind::Wifi => {
                    let (on, signal) = self.extras.wifi.unwrap_or((false, None));
                    Extra::Wifi { on, bars: signal.map_or(0, |s| 1 + (s.min(99) / 34)) }
                }
                ExtraKind::Volume => {
                    let (level, muted) = self.extras.volume.unwrap_or((0.0, true));
                    Extra::Volume { level, muted }
                }
                ExtraKind::Battery => {
                    let b = self.extras.battery.clone().unwrap();
                    Extra::Battery { percent: b.percent, charging: b.charging, show_percent: self.config.battery_percent }
                }
                ExtraKind::User => Extra::User { name: crate::shell::user_full_name() },
                ExtraKind::Notifications => Extra::Notifications { unread: self.avisos.unread, quiet: self.avisos.quiet },
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
            ExtraKind::Input => {
                let nodes = self.input_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
            ExtraKind::Clock => {
                let nodes = self.clock_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
            ExtraKind::Wifi => {
                self.extras.wifi = read_wifi().or(self.extras.wifi);
                let nodes = self.wifi_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
            ExtraKind::User => {
                let nodes = self.user_menu();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
            ExtraKind::Notifications => self.toggle_avisos_center(),
        }
    }

    /// Troca rápida de usuário: as contas (quem já tem sessão aberta vai direto
    /// para ela) e a janela de login. Dentro de outro desktop, só a lista.
    fn user_menu(&self) -> Vec<MenuNode> {
        let me = unsafe { libc::getuid() };
        let sessions = list_sessions();
        let my_vt = std::env::var("XDG_VTNR").ok().and_then(|v| v.parse::<u32>().ok());
        let mut v = Vec::new();
        for u in human_users() {
            let vt = sessions.iter().find(|s| s.0 == u.0 && Some(s.1) != my_vt).map(|s| s.1);
            let item = MenuItem::check(u.2.clone(), u.0 == me);
            let action = if u.0 == me || self.nested {
                None
            } else {
                Some(match vt {
                    Some(n) => MenuAction::SwitchToVt(n),
                    None => MenuAction::LoginWindow,
                })
            };
            let item = if action.is_none() && u.0 != me { item.disabled() } else { item };
            v.push(MenuNode::new(item, action));
        }
        v.push(MenuNode::sep());
        let login = MenuItem::item(tr!("Janela de Login…"));
        v.push(MenuNode::new(if self.nested { login.disabled() } else { login }, (!self.nested).then_some(MenuAction::LoginWindow)));
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item(tr!("Abrir Preferências de Contas…")), Some(MenuAction::OpenPrefs("contas".into()))));
        v
    }

    /// Bloqueia esta sessão e mostra a tela de entrar de outro usuário: a do
    /// gerenciador de login (se ele souber), senão um console de texto livre.
    pub fn login_window(&mut self) {
        self.lock_screen();
        let has = |cmd: &str| std::process::Command::new("sh").arg("-c").arg(format!("command -v {cmd}")).output().is_ok_and(|o| o.status.success());
        if has("dm-tool") {
            let _ = std::process::Command::new("dm-tool").arg("switch-to-greeter").spawn();
            return;
        }
        if has("gdmflexiserver") {
            let _ = std::process::Command::new("gdmflexiserver").spawn();
            return;
        }
        // Um console livre (o systemd abre a tela de "login:" nele).
        let used: Vec<u32> = list_sessions().iter().map(|s| s.1).collect();
        let mine = std::env::var("XDG_VTNR").ok().and_then(|v| v.parse::<u32>().ok());
        if let Some(vt) = (2..=6).find(|n| !used.contains(n) && Some(*n) != mine) {
            tracing::info!("troca de usuário: console {vt}");
            self.vt_request = Some(vt as i32);
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
            info(tr!("Bateria carregada").into())
        } else if b.charging {
            info(tr!("Carregando: {}%", b.percent))
        } else {
            info(tr!("Bateria: {}%", b.percent))
        }];
        if let Some(m) = b.minutes_left {
            v.push(info(format_left(m)));
        }
        v.push(info(tr!("Fonte de energia: {}", if b.on_ac { tr!("Adaptador") } else { tr!("Bateria") })));
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::check(tr!("Mostrar porcentagem"), self.config.battery_percent), Some(MenuAction::ToggleBatteryPercent)));
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item(tr!("Abrir Economizador de Energia…")), Some(MenuAction::OpenPrefs("energia".into()))));
        v
    }

    /// Menu de Entrada: os layouts escolhidos, com o atual marcado.
    fn input_menu(&self) -> Vec<MenuNode> {
        let k = &self.config.keyboard;
        let cur = glove_config::layout_id(&k.layout, &k.variant);
        let mut v: Vec<MenuNode> = k
            .input_menu
            .iter()
            .map(|id| {
                let (l, var) = glove_config::split_layout(id);
                let name = glove_config::KEYBOARD_LAYOUTS.iter().find(|x| x.0 == l && x.1 == var).map(|x| glove_config::tr(x.2).to_string()).unwrap_or_else(|| id.clone());
                MenuNode::new(MenuItem::check(name, *id == cur), Some(MenuAction::SetLayout(id.clone())))
            })
            .collect();
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item(tr!("Abrir Preferências de Teclado…")), Some(MenuAction::OpenPrefs("entrada".into()))));
        v
    }

    /// Troca o layout de teclado agora e guarda na configuração.
    pub fn set_layout(&mut self, layout: &str, variant: &str) {
        let mut c = self.config.clone();
        c.keyboard.layout = layout.to_string();
        c.keyboard.variant = variant.to_string();
        if let Err(e) = c.save() {
            tracing::warn!("não salvou o layout: {e}");
        }
        self.config.keyboard.layout = layout.to_string();
        self.config.keyboard.variant = variant.to_string();
        self.apply_keyboard();
        self.dirty = true;
    }

    /// Menu do relógio: a data por extenso, digital/analógico e o painel.
    fn clock_menu(&self) -> Vec<MenuNode> {
        let dias = [tr!("domingo"), tr!("segunda-feira"), tr!("terça-feira"), tr!("quarta-feira"), tr!("quinta-feira"), tr!("sexta-feira"), tr!("sábado")];
        let meses = [tr!("janeiro"), tr!("fevereiro"), tr!("março"), tr!("abril"), tr!("maio"), tr!("junho"), tr!("julho"), tr!("agosto"), tr!("setembro"), tr!("outubro"), tr!("novembro"), tr!("dezembro")];
        let (wd, d, mo, y, ..) = crate::shell::local_now();
        // {0} dia da semana, {1} dia, {2} mês, {3} ano (cada idioma põe na sua ordem).
        let date = tr!("{0}, {1} de {2} de {3}", dias[(wd % 7) as usize], d, meses[((mo + 11) % 12) as usize], y);
        let analog = self.config.clock.analog;
        vec![
            MenuNode::new(MenuItem::item(date).disabled(), None),
            MenuNode::sep(),
            MenuNode::new(MenuItem::check(tr!("Ver como Digital"), !analog), Some(MenuAction::ClockAnalog(false))),
            MenuNode::new(MenuItem::check(tr!("Ver como Analógico"), analog), Some(MenuAction::ClockAnalog(true))),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item(tr!("Abrir Data e Hora…")), Some(MenuAction::OpenPrefs("datahora".into()))),
        ]
    }

    /// Menu do Wi-Fi (como o do AirPort): ligar/desligar e as redes visíveis.
    fn wifi_menu(&self) -> Vec<MenuNode> {
        let info = |t: &str| MenuNode::new(MenuItem::item(t).disabled(), None);
        let on = self.extras.wifi.is_some_and(|w| w.0);
        let mut v = vec![info(if on { tr!("Wi-Fi: Ligado") } else { tr!("Wi-Fi: Desligado") })];
        v.push(MenuNode::new(MenuItem::item(if on { tr!("Desligar Wi-Fi") } else { tr!("Ligar Wi-Fi") }), Some(MenuAction::SetWifi(!on))));
        if on {
            v.push(MenuNode::sep());
            let nets = glove_config::rede::wifi_list(false);
            if nets.is_empty() {
                v.push(info(tr!("Nenhuma rede encontrada")));
            }
            for n in nets.into_iter().take(12) {
                v.push(MenuNode::new(MenuItem::check(n.ssid.clone(), n.in_use), Some(MenuAction::JoinWifi(n.ssid, n.secure))));
            }
            v.push(MenuNode::sep());
            v.push(MenuNode::new(MenuItem::item(tr!("Entrar em Outra Rede…")), Some(MenuAction::OpenPrefs("rede".into()))));
        }
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item(tr!("Abrir Preferências de Rede…")), Some(MenuAction::OpenPrefs("rede".into()))));
        v
    }

    /// Liga ou desliga o Wi-Fi (sem travar a tela: o nmcli roda numa thread).
    pub fn set_wifi(&mut self, on: bool) {
        std::thread::spawn(move || glove_config::rede::set_wifi(on));
        if let Some(w) = self.extras.wifi.as_mut() {
            *w = (on, None);
        }
        self.dirty = true;
    }

    /// Escolheu uma rede no menu: as abertas e as salvas conectam direto; as
    /// outras abrem as Preferências pedindo a senha.
    pub fn join_wifi(&mut self, ssid: &str, secure: bool) {
        if secure && !glove_config::rede::saved_connections().iter().any(|c| c == ssid) {
            let quoted = format!("'{}'", ssid.replace('\'', "'\\''"));
            self.spawn(&format!("glove-prefs rede --entrar {quoted}"));
        } else {
            // Rede salva que agora pede a senha (trocou, ou não ficou guardada): sem o
            // ícone de rede do GNOME, quem pergunta é a folha de senha das Preferências.
            let ssid = ssid.to_string();
            let prefs = crate::shell::sibling_binary("glove-prefs");
            std::thread::spawn(move || {
                if glove_config::rede::connect(&ssid, None).is_err() && secure {
                    let _ = std::process::Command::new(prefs).args(["rede", "--entrar", &ssid]).spawn().map(|mut c| c.wait());
                }
            });
        }
    }

    fn display_menu(&self) -> Vec<MenuNode> {
        let info = glove_config::OutputsInfo::load().unwrap_or_default();
        let mut v = Vec::new();
        // Cada monitor ligado com as resoluções dele (o principal primeiro).
        let mut outs: Vec<&glove_config::OutputInfo> = info.outputs.iter().filter(|o| o.active).collect();
        outs.sort_by_key(|o| !o.primary);
        if outs.is_empty() {
            outs.extend(info.outputs.first());
        }
        match outs.first() {
            Some(_) => {
                for (k, o) in outs.iter().enumerate() {
                    if k > 0 {
                        v.push(MenuNode::sep());
                    }
                    let name = if info.nested { tr!("Janela do Glove").to_string() } else if o.description.is_empty() { o.name.clone() } else { o.description.clone() };
                    v.push(MenuNode::new(MenuItem::item(name).disabled(), None));
                    let current = glove_config::parse_mode(&o.current).map(|(w, h, _)| (w, h));
                    for (w, h, hz, mode) in distinct_modes(&o.modes).into_iter().take(if outs.len() > 1 { 8 } else { 12 }) {
                        let label = if hz > 0 { format!("{w} × {h}, {hz} Hz") } else { format!("{w} × {h}") };
                        let mut it = MenuItem::check(format!("  {label}"), current == Some((w, h)));
                        if info.nested {
                            it = it.disabled();
                        }
                        v.push(MenuNode::new(it, Some(MenuAction::SetMode(o.name.clone(), mode))));
                    }
                }
            }
            None => v.push(MenuNode::new(MenuItem::item(tr!("Nenhum monitor")).disabled(), None)),
        }
        v.push(MenuNode::sep());
        v.push(MenuNode::new(MenuItem::item(tr!("Abrir Preferências de Monitor…")), Some(MenuAction::OpenPrefs("monitor".into()))));
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
            // O "pop" no volume novo (como no 10.4), se ligado no painel Som.
            if self.config.sound.volume_feedback && !muted {
                glove_config::sons::play("Volume", 1.0);
            }
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

    /// Na bateria, brilho a 75% do atual (e guarda o de antes para a volta).
    fn dim_for_battery(&mut self, on_battery: bool) {
        let Some((name, cur, max)) = backlight() else { return };
        let raw = if on_battery {
            self.extras.undimmed = Some(cur);
            ((cur as f32 * 0.75).round() as u32).clamp(1, max)
        } else {
            match self.extras.undimmed.take() {
                Some(v) => v.clamp(1, max),
                None => return,
            }
        };
        set_backlight(&name, raw);
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
        set_backlight(&name, raw);
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
        assert!(format_left(125).contains("2:05"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolucoes() {
        let m: Vec<String> = ["1920x1080@60", "1920x1080@144", "1280x720@60", "2560x1440@60"].iter().map(|s| s.to_string()).collect();
        let d = distinct_modes(&m);
        assert_eq!(d.iter().map(|x| x.3.as_str()).collect::<Vec<_>>(), ["2560x1440@60", "1920x1080@144", "1280x720@60"]);
    }
}

/// Contas de pessoas (uid de 1000 a 59999, com shell): (uid, login, nome).
pub fn human_users() -> Vec<(u32, String, String)> {
    let text = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    parse_passwd(&text)
}

pub fn parse_passwd(text: &str) -> Vec<(u32, String, String)> {
    let mut v: Vec<(u32, String, String)> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            let shell = f.get(6).copied().unwrap_or("");
            if !(1000..60000).contains(&uid) || shell.ends_with("nologin") || shell.ends_with("false") {
                return None;
            }
            let login = f[0].to_string();
            let real = f.get(4).and_then(|g| g.split(',').next()).filter(|n| !n.trim().is_empty()).map(|n| n.trim().to_string()).unwrap_or_else(|| login.clone());
            Some((uid, login, real))
        })
        .collect();
    v.sort_by_key(|u| u.2.to_lowercase());
    v
}

/// Sessões abertas em consoles: (uid, número do console).
fn list_sessions() -> Vec<(u32, u32)> {
    let out = std::process::Command::new("loginctl").args(["list-sessions", "--no-legend"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    parse_sessions(&out)
}

pub fn parse_sessions(text: &str) -> Vec<(u32, u32)> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let uid: u32 = f.get(1)?.parse().ok()?;
            let vt: u32 = f.iter().find_map(|w| w.strip_prefix("tty").and_then(|n| n.parse().ok()))?;
            Some((uid, vt))
        })
        .collect()
}

#[cfg(test)]
mod usuarios_tests {
    #[test]
    fn contas_e_sessoes() {
        let p = "root:x:0:0:root:/root:/bin/bash\nana:x:1000:1000:Ana Souza,,,:/home/ana:/bin/bash\nsrv:x:1001:1001::/srv:/usr/sbin/nologin\nbia:x:1002:1002::/home/bia:/bin/zsh\n";
        let u = super::parse_passwd(p);
        assert_eq!(u, vec![(1000, "ana".into(), "Ana Souza".into()), (1002, "bia".into(), "bia".into())]);
        let s = "      2 1000 ana  seat0 tty2 active no -\n      5 1002 bia  seat0 tty3 online yes 5min\n     c1  120 gdm  seat0 tty1 active no -\n";
        assert_eq!(super::parse_sessions(s), vec![(1000, 2), (1002, 3), (120, 1)]);
    }
}
