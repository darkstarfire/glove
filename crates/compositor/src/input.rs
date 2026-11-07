//! Teclado, mouse e atalhos.

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, Focus, GrabStartData, MotionEvent},
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER},
};

use glove_paint::chrome;

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    shell::{deco_top, Hit, MenuKey},
    state::Glove,
};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

#[derive(Clone, Copy, Debug, PartialEq)]
enum KeyAction {
    Terminal,
    Close,
    Minimize,
    Zoom,
    Cycle,
    Preferences,
    Screenshot,
    /// Captura de uma área (Shift+Print).
    ScreenshotArea,
    /// Esc com a área da captura sendo escolhida.
    RegionEsc,
    ForceQuit,
    Hide,
    HideOthers,
    Quit,
    /// Trocar para o console N (Ctrl+Alt+F1…F12, só na sessão própria).
    Vt(i32),
    /// Ctrl+F2: abre a barra de menu pelo teclado.
    MenuBar,
    Lock,
    /// Próximo layout do Menu de Entrada.
    InputNext,
    ZoomToggle,
    ZoomIn,
    ZoomOut,
    InvertColors,
    /// Tecla com a troca de apps aberta (ou Super+Shift+Tab para abrir voltando).
    Switcher(crate::switcher::SwitcherKey),
    SwitcherBack,
    ExposeAll,
    ExposeApp,
    ExposeDesktop,
    ExposeEsc,
    /// Super+Espaço: abre ou fecha a Busca Rápida.
    Search,
    SearchKey(crate::lock::LockKey),
    SearchMove(i32),
    /// Tecla com a tela bloqueada.
    LockKey(crate::lock::LockKey),
    /// Tecla com um menu aberto.
    Menu(MenuKey),
    /// Tecla com o painel Forçar Encerrar aberto (nome xkb).
    ForceQuitKey(&'static str),
    VolumeUp,
    VolumeDown,
    Mute,
    BrightnessUp,
    BrightnessDown,
    /// Tecla com a mesa em foco.
    Desk(crate::desk::DeskKey),
    /// F12: abre ou fecha o Painel de Widgets.
    Dashboard,
    /// Ctrl+`: a próxima janela do mesmo app.
    CycleWindows,
    /// Tecla com o Painel aberto.
    DashKey(crate::desk::DeskKey),
    /// F4: abre ou fecha a grade de apps.
    Grade,
    /// Tecla com a grade aberta.
    GradeKey(crate::grade::GradeKey),
}

impl KeyAction {
    fn from_id(s: &str) -> Option<Self> {
        Some(match s {
            "terminal" => Self::Terminal,
            "close" => Self::Close,
            "minimize" => Self::Minimize,
            "zoom" => Self::Zoom,
            "cycle" => Self::Cycle,
            "preferences" => Self::Preferences,
            "screenshot" => Self::Screenshot,
            "screenshot_area" => Self::ScreenshotArea,
            "force_quit" => Self::ForceQuit,
            "hide" => Self::Hide,
            "lock" => Self::Lock,
            "input_next" => Self::InputNext,
            "zoom_toggle" => Self::ZoomToggle,
            "zoom_in" => Self::ZoomIn,
            "zoom_out" => Self::ZoomOut,
            "invert_colors" => Self::InvertColors,
            "search" => Self::Search,
            "expose_all" => Self::ExposeAll,
            "expose_app" => Self::ExposeApp,
            "expose_desktop" => Self::ExposeDesktop,
            "dashboard" => Self::Dashboard,
            "grade" => Self::Grade,
            "cycle_windows" => Self::CycleWindows,
            "hide_others" => Self::HideOthers,
            "quit" => Self::Quit,
            _ => return None,
        })
    }
}

/// Um atalho já interpretado: modificadores + nome da tecla (minúsculo).
#[derive(Clone, Debug)]
pub struct Binding {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
    key: String,
    action: KeyAction,
}

/// "Super+Shift+Q" → Binding. Nomes de tecla seguem o xkb (Return, Tab, Up, comma…).
pub fn parse_bindings(c: &glove_config::Config) -> Vec<Binding> {
    let mut out = Vec::new();
    for s in &c.shortcuts {
        let Some(action) = KeyAction::from_id(&s.action) else { continue };
        let mut b = Binding { ctrl: false, alt: false, shift: false, logo: false, key: String::new(), action };
        for part in s.keys.split('+').map(str::trim) {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => b.ctrl = true,
                "alt" => b.alt = true,
                "shift" => b.shift = true,
                "super" | "logo" | "meta" | "win" => b.logo = true,
                "" => {}
                k => b.key = k.to_string(),
            }
        }
        if !b.key.is_empty() {
            out.push(b);
        }
    }
    out
}

impl Glove {
    /// Solta todas as teclas pressionadas (evita Ctrl/Shift "presos").
    pub fn release_all_keys(&mut self) {
        let Some(keyboard) = self.seat.get_keyboard() else { return };
        let t = self.start_time.elapsed().as_millis() as u32;
        for code in keyboard.pressed_keys() {
            self.on_key(code.raw(), KeyState::Released, t);
        }
        self.suppressed_keys.clear();
    }

    /// Comandos de teste (ver GLOVE_DEBUG_CTL em main.rs).
    pub fn debug_command(&mut self, line: &str) {
        let mut it = line.split_whitespace();
        let t = self.start_time.elapsed().as_millis() as u32;
        match it.next() {
            Some("shot") => {
                if let Some(p) = it.next() {
                    self.screenshot_request = Some(p.into());
                    self.dirty = true;
                }
            }
            Some("move") => {
                let x = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                self.on_motion((x, y).into(), t);
            }
            // "shot2 CAMINHO X Y L A ESCALA": o que um monitor extra nessa área mostraria.
            Some("shot2") => {
                let p = it.next().map(std::path::PathBuf::from);
                let n: Vec<f64> = it.filter_map(|v| v.parse().ok()).collect();
                if let (Some(p), [x, y, w, h, sc]) = (p, n.as_slice()) {
                    let area = smithay::utils::Rectangle::new((*x as i32, *y as i32).into(), (*w as i32, *h as i32).into());
                    self.extra_shot = Some((p, area, *sc));
                    self.dirty = true;
                }
            }
            // "mem": quanto ocupa cada cache e o heap (no registro). "trim": devolve
            // ao sistema a memória livre que o malloc segura, e mede de novo.
            Some(cmd @ ("mem" | "trim")) => {
                if cmd == "trim" {
                    unsafe { libc::malloc_trim(0) };
                }
                let mi = unsafe { libc::mallinfo2() };
                let rss = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS")).map(|l| l.to_string())).unwrap_or_default();
                let kb = |b: usize| b / 1024;
                let (gn, gb) = self.fonts.glyph_bytes();
                let (sn, sb) = glove_paint::chrome::shadow_bytes();
                let (mn, mb) = glove_paint::draw::metal_bytes();
                let (rn, rb) = glove_paint::sysicons::raster_bytes();
                #[cfg(feature = "memstats")]
                tracing::info!("mem rust: vivo {} KB, pico {} KB", kb(crate::memstats::LIVE.load(std::sync::atomic::Ordering::Relaxed)), kb(crate::memstats::PEAK.load(std::sync::atomic::Ordering::Relaxed)));
                tracing::info!(
                    "mem: {rss}; heap total {} KB, em uso {} KB, livre retido {} KB, mmap {} KB; ícones {} KB; sombras {sn} = {} KB; metal {mn} = {} KB; tema {rn} = {} KB; letras {gn} = {} KB",
                    kb(mi.arena), kb(mi.uordblks), kb(mi.fordblks), kb(mi.hblkhd),
                    kb(glove_paint::icons::cache_bytes()), kb(sb), kb(mb), kb(rb), kb(gb)
                );
            }
            // "run comando": abre um programa nesta sessão (como o Dock abriria).
            Some("run") => {
                let cmd = line.splitn(2, ' ').nth(1).unwrap_or("").trim();
                if !cmd.is_empty() {
                    self.spawn(cmd);
                }
            }
            // "shrink": a janela ativa vai para o tamanho mínimo que o app pediu.
            Some("shrink") => {
                if let Some(t) = self.focused_window().and_then(|w| w.toplevel().cloned()) {
                    let min = smithay::wayland::compositor::with_states(t.wl_surface(), |st| {
                        st.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>().current().min_size
                    });
                    t.with_pending_state(|s| s.size = Some(min));
                    t.send_pending_configure();
                    self.dirty = true;
                }
            }
            Some("down") => self.on_button(BTN_LEFT, ButtonState::Pressed, t),
            Some("up") => self.on_button(BTN_LEFT, ButtonState::Released, t),
            Some("type") => {
                let text: String = line.splitn(2, ' ').nth(1).unwrap_or("").to_string();
                for ch in text.chars() {
                    let (code, shift) = debug_keycode(&ch.to_string());
                    if code == 0 {
                        continue;
                    }
                    if shift {
                        self.on_key(42 + 8, KeyState::Pressed, t);
                    }
                    self.on_key(code + 8, KeyState::Pressed, t);
                    self.on_key(code + 8, KeyState::Released, t);
                    if shift {
                        self.on_key(42 + 8, KeyState::Released, t);
                    }
                }
            }
            Some("key") => {
                // ex.: "key ctrl+shift+z", "key enter", "key left"
                let combo = it.next().unwrap_or("");
                let parts: Vec<&str> = combo.split('+').collect();
                let mut held = Vec::new();
                for m in &parts[..parts.len().saturating_sub(1)] {
                    let code = match *m {
                        "ctrl" => 29,
                        "shift" => 42,
                        "alt" => 56,
                        "super" => 125,
                        _ => 0,
                    };
                    if code != 0 {
                        self.on_key(code + 8, KeyState::Pressed, t);
                        held.push(code);
                    }
                }
                let (code, _) = debug_keycode(parts.last().copied().unwrap_or(""));
                if code != 0 {
                    self.on_key(code + 8, KeyState::Pressed, t);
                    self.on_key(code + 8, KeyState::Released, t);
                }
                for code in held.into_iter().rev() {
                    self.on_key(code + 8, KeyState::Released, t);
                }
            }
            // "press super" / "release super": segura e solta uma tecla.
            Some(cmd @ ("press" | "release")) => {
                let name = it.next().unwrap_or("");
                let code = match name {
                    "ctrl" => 29,
                    "shift" => 42,
                    "alt" => 56,
                    "super" => 125,
                    k => debug_keycode(k).0,
                };
                if code != 0 {
                    let st = if cmd == "press" { KeyState::Pressed } else { KeyState::Released };
                    self.on_key(code + 8, st, t);
                }
            }
            Some("rclick") => {
                self.on_button(BTN_RIGHT, ButtonState::Pressed, t);
                self.on_button(BTN_RIGHT, ButtonState::Released, t + 1);
            }
            Some("click") => {
                self.on_button(BTN_LEFT, ButtonState::Pressed, t);
                self.on_button(BTN_LEFT, ButtonState::Released, t + 1);
            }
            _ => {}
        }
    }

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => self.on_key(event.key_code().raw(), event.state(), event.time_msec()),
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let Some(output) = self.primary() else { return };
                let Some(geo) = self.space.output_geometry(output) else { return };
                let pos = event.position_transformed(geo.size) + geo.loc.to_f64();
                self.on_motion(pos, event.time_msec());
            }
            // Mouse e touchpad de verdade (sessão própria): movimento relativo.
            InputEvent::PointerMotion { event, .. } => {
                // Pode passar de um monitor para o outro; nunca sai de todos.
                let pos = self.clamp_to_outputs(self.pointer_pos + event.delta());
                self.on_motion(pos, event.time_msec());
                self.dirty = true;
            }
            InputEvent::PointerButton { event, .. } => self.on_button(event.button_code(), event.state(), event.time_msec()),
            InputEvent::SwitchToggle { event } => {
                use smithay::backend::input::{Switch, SwitchState, SwitchToggleEvent};
                if event.switch() == Some(Switch::Lid) {
                    self.lid(event.state() == SwitchState::On);
                }
            }
            InputEvent::PointerAxis { event, .. } => {
                if self.note_input() || self.locked() {
                    return;
                }
                let source = event.source();
                // Na sessão própria quem inverte é o libinput (mouse e touchpad separados).
                let sign = if self.nested && self.config.pointer.natural_scroll { -1.0 } else { 1.0 };
                // Velocidade de rolagem das Preferências (só nos valores contínuos;
                // os "cliques" da roda continuam sendo um clique cada).
                let speed = self.config.pointer.scroll_speed.clamp(0.25, 4.0);
                let amount = |a| sign * speed * event.amount(a).unwrap_or_else(|| event.amount_v120(a).unwrap_or(0.0) * 15.0 / 120.0);
                // Grade aberta: a rolagem troca de página (vertical ou horizontal).
                if self.grade_active() {
                    let v = amount(Axis::Vertical) + amount(Axis::Horizontal);
                    self.grade_scroll(v);
                    return;
                }
                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let v = amount(axis);
                    if v != 0.0 {
                        frame = frame.value(axis, v);
                        if let Some(d) = event.amount_v120(axis) {
                            frame = frame.v120(axis, (sign * d) as i32);
                        }
                    }
                    if source == AxisSource::Finger && event.amount(axis) == Some(0.0) {
                        frame = frame.stop(axis);
                    }
                }
                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    fn on_key(&mut self, code: u32, state: KeyState, time: u32) {
        // Tela de encerramento: nada mais responde.
        if self.ending_screen_shown() {
            return;
        }
        if state == KeyState::Pressed {
            self.note_input();
        }
        // Iniciando sessão: as teclas esperam a mesa aparecer.
        if self.startup_covering() {
            return;
        }
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        let action = keyboard.input::<Option<KeyAction>, _>(self, code.into(), state, serial, time, |st, mods, handle| {
            if state == KeyState::Released {
                // Soltou o Super com a troca de apps aberta: troca (depois de entregar a tecla).
                if st.switcher.is_some() && !mods.logo && !(st.nested && mods.alt) {
                    st.switcher_commit = true;
                }
                if let Some(i) = st.suppressed_keys.iter().position(|k| *k == code) {
                    st.suppressed_keys.remove(i);
                    return FilterResult::Intercept(None);
                }
                return FilterResult::Forward;
            }
            // Ctrl+Alt+F1…F12 (o xkb já entrega como XF86Switch_VT_N).
            let vt = handle.modified_sym().raw();
            if !st.nested && (0x1008FE01..=0x1008FE0C).contains(&vt) {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::Vt((vt - 0x1008FE00) as i32)));
            }
            // Tela de login: nenhum atalho; tudo vai para o app de login.
            if st.greeter {
                return FilterResult::Forward;
            }
            // Nome da tecla sem modificadores (independe do layout para letras).
            let raw = handle.raw_latin_sym_or_raw_current_sym().unwrap_or_else(|| handle.modified_sym());
            let name = smithay::input::keyboard::xkb::keysym_get_name(raw).to_lowercase();
            // Teclas de volume e brilho (valem sempre).
            let media = match name.as_str() {
                "xf86audioraisevolume" => Some(KeyAction::VolumeUp),
                "xf86audiolowervolume" => Some(KeyAction::VolumeDown),
                "xf86audiomute" => Some(KeyAction::Mute),
                "xf86monbrightnessup" => Some(KeyAction::BrightnessUp),
                "xf86monbrightnessdown" => Some(KeyAction::BrightnessDown),
                _ => None,
            };
            if media.is_some() {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(media);
            }
            // Bloqueada: toda tecla vai para o campo de senha (nada para os apps,
            // nenhum atalho; só a troca de console e as de volume/brilho, acima).
            if st.lock.is_some() {
                let k = match name.as_str() {
                    "backspace" => Some(crate::lock::LockKey::Backspace),
                    "return" | "kp_enter" => Some(crate::lock::LockKey::Enter),
                    "escape" => Some(crate::lock::LockKey::Escape),
                    _ => handle.modified_sym().key_char().map(crate::lock::LockKey::Char),
                };
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(k.map(KeyAction::LockKey));
            }
            // Troca de apps aberta: as teclas são dela.
            if st.switcher.is_some() {
                use crate::switcher::SwitcherKey as K;
                let k = match name.as_str() {
                    "tab" | "iso_left_tab" if mods.shift => Some(K::Prev),
                    "tab" | "right" => Some(K::Next),
                    "iso_left_tab" | "left" => Some(K::Prev),
                    "q" => Some(K::Quit),
                    "h" => Some(K::Hide),
                    "escape" => Some(K::Cancel),
                    _ => None,
                };
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(k.map(KeyAction::Switcher));
            }
            // Super+Shift+Tab: abre voltando.
            if (mods.logo || (st.nested && mods.alt)) && mods.shift && matches!(name.as_str(), "tab" | "iso_left_tab") {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::SwitcherBack));
            }
            // Escolhendo a área da captura: Esc cancela.
            if st.region.is_some() && name == "escape" {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::RegionEsc));
            }
            // Painel aberto: F12 fecha; as outras teclas vão para o widget escolhido.
            if st.dashboard.open && !mods.logo && !(st.nested && mods.alt) {
                if name == "f12" {
                    st.suppressed_keys.push(code);
                    return FilterResult::Intercept(Some(KeyAction::Dashboard));
                }
                let ch = handle.modified_sym().key_char();
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(crate::desk::desk_key_of(&name, ch, mods.ctrl, mods.shift, true).map(KeyAction::DashKey));
            }
            // Grade de apps aberta: o texto vai para a busca; as teclas F seguem
            // para os atalhos (F4 fecha, F12 troca pelo Painel…).
            if st.grade.open && !mods.logo && !mods.ctrl && !(st.nested && mods.alt) {
                use crate::grade::GradeKey as G;
                let fkey = name.len() > 1 && name.starts_with('f') && name[1..].chars().all(|c| c.is_ascii_digit());
                if !fkey {
                    let k = match name.as_str() {
                        "left" | "kp_left" => Some(G::Move(-1, 0)),
                        "right" | "kp_right" => Some(G::Move(1, 0)),
                        "up" | "kp_up" => Some(G::Move(0, -1)),
                        "down" | "kp_down" => Some(G::Move(0, 1)),
                        "prior" | "kp_prior" => Some(G::Page(-1)),
                        "next" | "kp_next" => Some(G::Page(1)),
                        "backspace" => Some(G::Backspace),
                        "return" | "kp_enter" => Some(G::Enter),
                        "escape" => Some(G::Escape),
                        _ => handle.modified_sym().key_char().map(G::Char),
                    };
                    st.suppressed_keys.push(code);
                    return FilterResult::Intercept(k.map(KeyAction::GradeKey));
                }
            }
            // Exposé: Esc volta.
            if st.expose.is_some() && name == "escape" {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::ExposeEsc));
            }
            // Busca Rápida aberta: o texto vai para o campo (atalhos com Super/Ctrl seguem).
            if st.search.open && !mods.logo && !mods.ctrl && !(st.nested && mods.alt) {
                let k = match name.as_str() {
                    "up" => Some(KeyAction::SearchMove(-1)),
                    "down" => Some(KeyAction::SearchMove(1)),
                    "backspace" => Some(KeyAction::SearchKey(crate::lock::LockKey::Backspace)),
                    "return" | "kp_enter" => Some(KeyAction::SearchKey(crate::lock::LockKey::Enter)),
                    "escape" => Some(KeyAction::SearchKey(crate::lock::LockKey::Escape)),
                    _ => handle.modified_sym().key_char().map(|c| KeyAction::SearchKey(crate::lock::LockKey::Char(c))),
                };
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(k);
            }
            // Menu aberto: as teclas são dele (setas, Enter, Esc).
            if st.chrome.menu.is_some() {
                let k = match name.as_str() {
                    "up" | "kp_up" => MenuKey::Up,
                    "down" | "kp_down" => MenuKey::Down,
                    "left" | "kp_left" => MenuKey::Left,
                    "right" | "kp_right" => MenuKey::Right,
                    "return" | "kp_enter" | "space" => MenuKey::Activate,
                    "escape" => MenuKey::Close,
                    _ if mods.ctrl && name == "f2" => MenuKey::Close,
                    _ => MenuKey::Other,
                };
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::Menu(k)));
            }
            // Painel Forçar Encerrar: setas, Enter e Esc são dele.
            if st.force_quit.is_some() && !mods.ctrl && !mods.alt && !mods.logo {
                let k = match name.as_str() {
                    "up" => Some("up"),
                    "down" => Some("down"),
                    "return" | "kp_enter" => Some("return"),
                    "escape" => Some("escape"),
                    _ => None,
                };
                if let Some(k) = k {
                    st.suppressed_keys.push(code);
                    return FilterResult::Intercept(Some(KeyAction::ForceQuitKey(k)));
                }
            }
            // Ctrl+F2: barra de menu pelo teclado (como no Mac OS X).
            if mods.ctrl && !mods.alt && !mods.shift && !mods.logo && name == "f2" {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::MenuBar));
            }
            // Primeiro o atalho exato (Super+Alt+espaço não vira Super+espaço).
            let exact = st.bindings.iter().find_map(|b| (b.key == name && b.ctrl == mods.ctrl && b.shift == mods.shift && b.logo == mods.logo && b.alt == mods.alt).then_some(b.action));
            let action = exact.or_else(|| {
                st.bindings.iter().find_map(|b| {
                    // Dentro de outro desktop o Super costuma ser capturado: Alt vale como Super.
                    let logo_ok = if b.logo { mods.logo || (st.nested && mods.alt && !b.alt) } else { !mods.logo };
                    let alt_ok = if b.logo && st.nested && !b.alt { true } else { b.alt == mods.alt };
                    (b.key == name && b.ctrl == mods.ctrl && b.shift == mods.shift && logo_ok && alt_ok).then_some(b.action)
                })
            });
            match action {
                Some(a) => {
                    st.suppressed_keys.push(code);
                    FilterResult::Intercept(Some(a))
                }
                // Mesa em foco (nenhuma janela): as teclas do Arquivos valem para ela.
                None if st.desk_active() && !mods.alt && !mods.logo => {
                    let ch = handle.modified_sym().key_char();
                    match crate::desk::desk_key_of(&name, ch, mods.ctrl, mods.shift, st.desk.rename.is_some()) {
                        Some(k) => {
                            st.suppressed_keys.push(code);
                            FilterResult::Intercept(Some(KeyAction::Desk(k)))
                        }
                        None => FilterResult::Forward,
                    }
                }
                None => FilterResult::Forward,
            }
        });

        if std::mem::take(&mut self.switcher_commit) {
            self.switcher_key(crate::switcher::SwitcherKey::Commit);
        }
        // Tecla entregue a um app: aproveita para ver se ele responde.
        if state == KeyState::Pressed && action.as_ref().is_some_and(|a| a.is_none()) {
            if let Some(w) = self.focused_window() {
                self.ping_window(&w);
            }
        }
        match action.flatten() {
            Some(KeyAction::Terminal) => {
                let t = self.config.terminal.clone();
                self.spawn(&t);
            }
            Some(KeyAction::Close) => {
                if let Some(w) = self.focused_window() {
                    self.close_window(&w);
                }
            }
            Some(KeyAction::Minimize) => {
                if let Some(w) = self.focused_window() {
                    self.minimize_window(&w);
                }
            }
            Some(KeyAction::Cycle) => self.switcher_open_or_next(false),
            Some(KeyAction::SwitcherBack) => self.switcher_open_or_next(true),
            Some(KeyAction::Switcher(k)) => self.switcher_key(k),
            Some(KeyAction::Zoom) => {
                if let Some(w) = self.focused_window() {
                    self.toggle_zoom(&w);
                }
            }
            Some(KeyAction::Preferences) => self.open_preferences(),
            Some(KeyAction::Screenshot) => self.request_screenshot(),
            Some(KeyAction::ScreenshotArea) => self.start_region_shot(false, false),
            Some(KeyAction::RegionEsc) => self.cancel_region(),
            Some(KeyAction::Hide) => {
                if let Some(pid) = self.focused_pid() {
                    self.hide_pid(pid);
                }
            }
            Some(KeyAction::HideOthers) => {
                if let Some(pid) = self.focused_pid() {
                    self.hide_others(pid);
                }
            }
            Some(KeyAction::ForceQuit) => self.run_menu_action(crate::shell::MenuAction::ForceQuit),
            Some(KeyAction::Quit) => self.loop_signal.stop(),
            Some(KeyAction::Vt(n)) => self.vt_request = Some(n),
            Some(KeyAction::MenuBar) => self.open_menubar_by_key(),
            Some(KeyAction::Lock) => self.lock_screen(),
            Some(KeyAction::ZoomToggle) => self.change_access(|a| a.zoom = !a.zoom),
            Some(KeyAction::ZoomIn) => self.change_access(|a| {
                a.zoom = true;
                a.zoom_level = (a.zoom_level * 1.25).min(8.0);
            }),
            Some(KeyAction::ZoomOut) => self.change_access(|a| {
                a.zoom_level = (a.zoom_level / 1.25).max(1.0);
                if a.zoom_level < 1.05 {
                    a.zoom = false;
                    a.zoom_level = 2.0;
                }
            }),
            Some(KeyAction::InvertColors) => self.change_access(|a| a.invert = !a.invert),
            Some(KeyAction::InputNext) => {
                if let Some((l, v)) = glove_config::next_layout(&self.config.keyboard) {
                    self.set_layout(&l, &v);
                }
            }
            Some(KeyAction::Search) => self.toggle_search(),
            Some(KeyAction::ExposeAll) => self.toggle_expose(crate::expose::ExposeMode::All),
            Some(KeyAction::ExposeApp) => self.toggle_expose(crate::expose::ExposeMode::App),
            Some(KeyAction::ExposeDesktop) => self.toggle_expose(crate::expose::ExposeMode::Desktop),
            Some(KeyAction::ExposeEsc) => self.leave_expose(None),
            Some(KeyAction::SearchKey(k)) => self.search_key(k),
            Some(KeyAction::Desk(k)) => self.desk_key(k),
            Some(KeyAction::Dashboard) => self.toggle_dashboard(),
            Some(KeyAction::Grade) => self.toggle_grade(),
            Some(KeyAction::GradeKey(k)) => self.grade_key(k),
            Some(KeyAction::CycleWindows) => self.cycle_app_windows(),
            Some(KeyAction::DashKey(k)) => self.dashboard_key(k),
            Some(KeyAction::SearchMove(d)) => self.search_move(d),
            Some(KeyAction::LockKey(k)) => self.lock_key(k),
            Some(KeyAction::Menu(k)) => self.menu_key(k),
            Some(KeyAction::VolumeUp) => self.step_volume(1.0),
            Some(KeyAction::VolumeDown) => self.step_volume(-1.0),
            Some(KeyAction::Mute) => self.toggle_mute(),
            Some(KeyAction::BrightnessUp) => self.step_brightness(1.0),
            Some(KeyAction::BrightnessDown) => self.step_brightness(-1.0),
            Some(KeyAction::ForceQuitKey(k)) => {
                self.force_quit_key(k);
            }
            None => {}
        }
    }

    fn update_hover(&mut self, pos: Point<f64, Logical>) {
        // Arrastando o controle de volume: segue o mouse mesmo fora dele.
        if self.extras.slider.as_ref().is_some_and(|s| s.dragging) {
            let y = (pos.y - self.extras.slider.as_ref().unwrap().origin.y as f64) as f32;
            self.volume_slider_drag(y, false);
            return;
        }
        let hit = self.hit_test(pos);

        // Menu aberto
        // Menu aberto: passar sobre outro título troca de menu (como na época).
        if let (Some(cur), Hit::MenuBar { target: Some(t) }) = (self.chrome.menu.as_ref().map(|m| m.target), &hit) {
            if *t != cur {
                self.open_bar_menu(*t);
            }
        }
        if let Some(m) = &mut self.chrome.menu {
            match &hit {
                Hit::Menu(l, i) => self.menu_hover(*l, *i),
                _ => {
                    // Fora dos menus: apaga só o realce do painel mais interno (os
                    // outros continuam marcando o item que abriu o submenu).
                    if let Some(lv) = m.levels.last_mut() {
                        if lv.hover.take().is_some() {
                            self.dirty = true;
                        }
                    }
                }
            }
        }

        // Busca Rápida: o mouse escolhe a linha.
        if let Hit::Search(Some(i)) = hit {
            if self.search.selected != Some(i) {
                self.search.selected = Some(i);
                self.dirty = true;
            }
        }

        // Painel Forçar Encerrar: realce dos botões da barra de título.
        let fq_hover = matches!(hit, Hit::ForceQuit(chrome::FqHit::Title(_)));
        if let Some(fq) = &mut self.force_quit {
            if fq.title_hover != fq_hover {
                fq.title_hover = fq_hover;
                self.dirty = true;
            }
        }

        // App travado sob o mouse: cursor de espera (e um ping de vez em quando).
        let under = match &hit {
            Hit::Client(w) | Hit::Title { window: w, .. } => Some(w.clone()),
            _ => None,
        };
        if let Some(w) = &under {
            self.ping_window(w);
        }
        self.update_wait_cursor(under.as_ref());

        // Dock (ampliação segue o mouse)
        match &hit {
            Hit::Dock(i) => {
                let d = self.chrome.dock_pos();
                let (lx, ly) = ((pos.x - d.x as f64) as f32, (pos.y - d.y as f64) as f32);
                // Ao longo do Dock: x embaixo, y nas laterais.
                let along = if self.chrome.dock_side.vertical() { ly } else { lx };
                self.chrome.dock_hover_x = Some(along);
                self.chrome.dock_last_x = along;
                self.chrome.animate_dock_zoom(1.0);
                // Recalcula já para o item sob o mouse ficar certo.
                self.prepare_dock();
                let d = self.chrome.dock_pos();
                let idx = self
                    .chrome
                    .dock_layout
                    .as_ref()
                    .and_then(|l| chrome::dock_item_at(l, (pos.x - d.x as f64) as f32, (pos.y - d.y as f64) as f32))
                    .or(*i);
                self.chrome.dock_hover = idx;
                self.dirty = true;
            }
            _ => {
                if self.chrome.dock_hover_x.take().is_some() {
                    self.chrome.dock_hover = None;
                    self.chrome.animate_dock_zoom(0.0);
                    self.dirty = true;
                }
            }
        }

        // Ocultar automaticamente: aparece com o mouse na borda, some ao sair.
        if self.config.dock.autohide {
            let (w, h) = self.output_size();
            let at_edge = match self.chrome.dock_side {
                chrome::DockSide::Bottom => pos.y >= h as f64 - 2.0,
                chrome::DockSide::Left => pos.x <= 1.0,
                chrome::DockSide::Right => pos.x >= w as f64 - 2.0,
            };
            let keep = at_edge || matches!(hit, Hit::Dock(_)) || self.dock_menu_open() || self.chrome.dock_pressed.is_some();
            self.chrome.animate_dock_hide(if keep { 0.0 } else { 1.0 });
            self.dirty = true;
        }

        // Símbolos nos botões da janela
        let hb = match &hit {
            Hit::Title { window, group: true, .. } => Some(window.clone()),
            _ => None,
        };
        if self.chrome.hover_buttons != hb {
            self.chrome.hover_buttons = hb;
            self.dirty = true;
        }
    }

    fn on_motion(&mut self, pos: Point<f64, Logical>, time: u32) {
        self.note_input();
        self.pointer_pos = pos;
        // Escolhendo a área da captura: o mouse só desenha o retângulo.
        if self.region.is_some() {
            self.region_motion(pos);
            return;
        }
        // Com zoom, a tela acompanha o mouse.
        if self.config.access.zoom {
            self.dirty = true;
        }
        if !self.locked() && !self.greeter {
            self.hot_corner(pos);
        }
        if self.grade_active() {
            // Grade aberta: nenhum app vê o mouse.
            self.grade_motion(pos);
            let pointer = self.seat.get_pointer().unwrap();
            let serial = SERIAL_COUNTER.next_serial();
            pointer.motion(self, None, &MotionEvent { location: pos, serial, time });
            pointer.frame(self);
            return;
        }
        if self.dashboard_active() {
            // Painel aberto: o mouse é dele (arrastar widgets).
            self.dashboard_motion(pos);
            let pointer = self.seat.get_pointer().unwrap();
            let serial = SERIAL_COUNTER.next_serial();
            pointer.motion(self, None, &MotionEvent { location: pos, serial, time });
            pointer.frame(self);
            return;
        }
        if self.expose_active() {
            // Nenhum app vê o mouse durante o Exposé.
            self.expose_motion(pos);
            let pointer = self.seat.get_pointer().unwrap();
            let serial = SERIAL_COUNTER.next_serial();
            pointer.motion(self, None, &MotionEvent { location: pos, serial, time });
            pointer.frame(self);
            self.dirty = true;
            return;
        }
        if self.locked() {
            // Nenhum app vê o mouse com a tela bloqueada.
            let pointer = self.seat.get_pointer().unwrap();
            let serial = SERIAL_COUNTER.next_serial();
            pointer.motion(self, None, &MotionEvent { location: pos, serial, time });
            pointer.frame(self);
            if let Some(l) = &mut self.lock {
                if l.pressed {
                    l.pressed = false;
                }
            }
            self.dirty = true;
            return;
        }
        // Arrastando o ícone do documento (do título): vira arrastar do sistema.
        if let Some((doc, at)) = self.proxy_press.clone() {
            if (pos.x - at.x).powi(2) + (pos.y - at.y).powi(2) > 25.0 {
                self.proxy_press = None;
                self.desk_start_drag(vec![doc], at);
            }
        }
        self.desk_motion(pos);
        let pointer = self.seat.get_pointer().unwrap();
        if !pointer.is_grabbed() {
            self.update_hover(pos);
        }
        let under = self.surface_under(pos);
        let serial = SERIAL_COUNTER.next_serial();
        pointer.motion(self, under, &MotionEvent { location: pos, serial, time });
        pointer.frame(self);
    }

    fn on_button(&mut self, button: u32, state: ButtonState, time: u32) {
        if self.ending_screen_shown() {
            return;
        }
        if self.startup_covering() {
            return;
        }
        // O clique que acende a tela só acende.
        if state == ButtonState::Pressed && self.note_input() {
            self.swallow_wake_release = true;
            return;
        }
        if self.region.is_some() {
            if button == BTN_LEFT {
                self.region_button(state == ButtonState::Pressed);
            }
            return;
        }
        if state == ButtonState::Released && std::mem::take(&mut self.swallow_wake_release) {
            return;
        }
        if self.grade_active() {
            if button == BTN_LEFT && state == ButtonState::Pressed {
                let pos = self.pointer_pos;
                self.grade_button(pos);
                self.swallow_wake_release = true;
            }
            return;
        }
        if self.dashboard_active() {
            if button == BTN_LEFT {
                let pos = self.pointer_pos;
                self.dashboard_button(pos, state == ButtonState::Pressed);
            }
            return;
        }
        if self.expose_active() {
            if state == ButtonState::Pressed {
                let pos = self.pointer_pos;
                self.expose_click(pos);
                self.swallow_wake_release = true;
            }
            return;
        }
        if self.locked() {
            let pos = self.pointer_pos;
            let on_button = self.lock_button_at(pos.x, pos.y);
            if let Some(l) = &mut self.lock {
                match state {
                    ButtonState::Pressed => l.pressed = on_button,
                    ButtonState::Released => {
                        let go = l.pressed && on_button;
                        l.pressed = false;
                        if go {
                            self.try_unlock();
                        }
                    }
                }
            }
            self.dirty = true;
            return;
        }
        let pointer = self.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let pos = pointer.current_location();
        let mut swallow = false;

        enum Then {
            Nothing,
            Move(smithay::desktop::Window),
            Resize(smithay::desktop::Window, crate::grabs::ResizeEdge),
        }
        let mut then = Then::Nothing;

        if state == ButtonState::Pressed && !pointer.is_grabbed() && self.search.open {
            // Busca aberta: clique num resultado abre; na lupa ou fora, fecha.
            match self.hit_test(pos) {
                Hit::Search(Some(i)) => self.search_open_result(i),
                Hit::Search(None) => {}
                _ => self.close_search(),
            }
            self.chrome.swallow_release = true;
            return;
        }
        if state == ButtonState::Pressed && !pointer.is_grabbed() && self.extras.slider.is_some() {
            // Controle de volume aberto: clicar nele ajusta; fora, fecha.
            match self.hit_test(pos) {
                Hit::VolumeSlider(y) => {
                    if let Some(s) = &mut self.extras.slider {
                        s.dragging = true;
                    }
                    self.volume_slider_drag(y, false);
                }
                Hit::MenuBar { target: Some(t) } if Some(t) != self.volume_extra_index().map(crate::shell::BarTarget::Extra) => {
                    self.open_bar_menu(t);
                }
                _ => self.close_volume_slider(),
            }
            self.chrome.swallow_release = true;
            return;
        }
        if state == ButtonState::Released && self.extras.slider.as_ref().is_some_and(|s| s.dragging) {
            let y = (pos.y - self.extras.slider.as_ref().unwrap().origin.y as f64) as f32;
            self.volume_slider_drag(y, true);
            if let Some(s) = &mut self.extras.slider {
                s.dragging = false;
            }
            return;
        }
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            let hit = self.hit_test(pos);
            if let Some(cur) = self.chrome.menu.as_ref().map(|m| m.target) {
                // Com o menu aberto: clicar no mesmo título fecha, em outro troca;
                // fora dos menus, só fecha.
                match hit {
                    Hit::Menu(..) | Hit::VolumeSlider(_) | Hit::Search(_) => {}
                    Hit::Dock(Some(i)) if button == BTN_RIGHT => self.open_dock_menu(i),
                    Hit::MenuBar { target: Some(t) } if t != cur => self.open_bar_menu(t),
                    _ => self.close_menu(),
                }
                swallow = true;
            } else {
                match hit {
                    Hit::SearchButton => {
                        self.open_search();
                        swallow = true;
                    }
                    Hit::MenuBar { target } => {
                        if let (Some(t), BTN_LEFT) = (target, button) {
                            self.open_bar_menu(t);
                        }
                        swallow = true;
                    }
                    Hit::Dock(Some(i)) if button == BTN_RIGHT => {
                        self.open_dock_menu(i);
                        swallow = true;
                    }
                    Hit::Dock(i) => {
                        self.chrome.dock_pressed = i;
                        // Clique segurado: abre o menu do ícone (e soltar não abre o app).
                        if let Some(i) = i {
                            let at = std::time::Instant::now();
                            self.dock_press_at = Some(at);
                            let _ = self.loop_handle.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(600)), move |_, _, data| {
                                let st = &mut data.state;
                                if st.dock_press_at == Some(at) && st.chrome.dock_pressed == Some(i) {
                                    st.chrome.dock_pressed = None;
                                    st.open_dock_menu(i);
                                }
                                smithay::reexports::calloop::timer::TimeoutAction::Drop
                            });
                        }
                        swallow = true;
                    }
                    // Pílula: esconde/mostra a barra de ferramentas do app.
                    Hit::Title { window, pill: true, .. } => {
                        self.focus_window(Some(window.clone()));
                        if let Some(pid) = self.window_pid(&window) {
                            if let Some(c) = self.menu_clients.iter_mut().rev().find(|c| c.pid == pid) {
                                c.activate("toggle-toolbar");
                            }
                        }
                        swallow = true;
                    }
                    // Ícone do documento: Ctrl+clique mostra as pastas do caminho; arrastar leva o arquivo.
                    Hit::Title { window, proxy: Some(doc), button: None, .. } => {
                        self.focus_window(Some(window.clone()));
                        let ctrl = self.seat.get_keyboard().is_some_and(|k| k.modifier_state().ctrl || k.modifier_state().logo);
                        if ctrl || button == BTN_RIGHT {
                            self.open_path_menu(&doc, pos);
                        } else if button == BTN_LEFT {
                            self.proxy_press = Some((doc, pos));
                        }
                        swallow = true;
                    }
                    Hit::Title { window, button: Some(b), .. } => {
                        self.focus_window(Some(window.clone()));
                        self.chrome.pressed_button = Some((window, b));
                        swallow = true;
                    }
                    Hit::Title { window, button: None, .. } => {
                        self.focus_window(Some(window.clone()));
                        if button == BTN_LEFT {
                            // Clique duplo na barra de título minimiza (se ligado em Aparência).
                            let now = std::time::Instant::now();
                            let double = self.title_click.as_ref().is_some_and(|(w, t)| *w == window && now.duration_since(*t).as_millis() < self.config.pointer.double_click_ms as u128);
                            if double && self.config.appearance.double_click_minimizes {
                                self.title_click = None;
                                self.minimize_window(&window);
                                swallow = true;
                            } else {
                                self.title_click = Some((window.clone(), now));
                                then = Then::Move(window);
                            }
                        }
                    }
                    Hit::Resize(window, edges) => {
                        self.focus_window(Some(window.clone()));
                        if button == BTN_LEFT {
                            then = Then::Resize(window, edges);
                        }
                    }
                    Hit::ForceQuit(h) => {
                        self.force_quit_press(h);
                        swallow = true;
                    }
                    Hit::Client(window) => {
                        self.ping_window(&window);
                        if self.focused_window().as_ref() != Some(&window) {
                            self.focus_window(Some(window));
                        }
                    }
                    Hit::DesktopIcon(i) => {
                        self.focus_window(None);
                        self.desk_press(Some(i), button, pos);
                        swallow = true;
                    }
                    // Tela de login: o app de login nunca perde o foco.
                    Hit::Desktop if self.greeter => {}
                    Hit::Desktop => {
                        self.focus_window(None);
                        self.desk_press(None, button, pos);
                    }
                    Hit::Menu(..) | Hit::VolumeSlider(_) | Hit::Search(_) => {}
                }
            }
            self.chrome.swallow_release = swallow;
        } else if state == ButtonState::Released {
            self.proxy_press = None;
            self.desk_release();
            let hit = self.hit_test(pos);
            // Menu: escolher um item ao soltar
            if let (Some(_), Hit::Menu(l, Some(i))) = (&self.chrome.menu, &hit) {
                self.menu_activate(*l, *i);
            }
            if self.force_quit.as_ref().is_some_and(|f| f.pressed.is_some()) {
                let h = if let Hit::ForceQuit(h) = hit { Some(h) } else { None };
                self.force_quit_release(h);
            }
            if let Some(p) = self.chrome.dock_pressed.take() {
                if let Hit::Dock(Some(i)) = hit {
                    if i == p {
                        if let Some(a) = self.chrome.dock_actions.get(i).cloned() {
                            self.run_dock_action(a);
                        }
                    }
                }
                self.dirty = true;
            }
            if let Some((w, b)) = self.chrome.pressed_button.take() {
                if let Hit::Title { window, button: Some(b2), .. } = &hit {
                    if *window == w && *b2 == b {
                        match b {
                            chrome::TitleButton::Close => self.close_window(&w),
                            chrome::TitleButton::Minimize => self.minimize_window(&w),
                            chrome::TitleButton::Zoom => self.toggle_zoom(&w),
                        }
                    }
                }
                self.dirty = true;
            }
            swallow = std::mem::take(&mut self.chrome.swallow_release);
        }

        if !swallow {
            pointer.button(self, &ButtonEvent { button, state, serial, time });
        }
        pointer.frame(self);

        let start = |location| GrabStartData { focus: None, button, location };
        match then {
            Then::Move(window) => {
                let Some(loc) = self.space.element_location(&window) else { return };
                let min_y = glove_paint::theme::MENUBAR_H + deco_top(&window);
                let grab = MoveSurfaceGrab::new(start(pos), window, loc, min_y);
                pointer.set_grab(self, grab, serial, Focus::Clear);
            }
            Then::Resize(window, edges) => {
                let Some(loc) = self.space.element_location(&window) else { return };
                let size = window.geometry().size;
                if let Some(t) = window.toplevel() {
                    t.with_pending_state(|s| {
                        s.states.set(xdg_toplevel::State::Resizing);
                    });
                    t.send_pending_configure();
                }
                let grab = ResizeSurfaceGrab::start(start(pos), window, edges, Rectangle::new(loc, size));
                pointer.set_grab(self, grab, serial, Focus::Clear);
            }
            Then::Nothing => {}
        }
    }
}

impl Glove {
    /// Arrastar a janela a pedido do app (apps X11 que movem a si mesmos).
    pub fn start_move(&mut self, window: smithay::desktop::Window, serial: smithay::utils::Serial) {
        let Some(pointer) = self.seat.get_pointer() else { return };
        let Some(loc) = self.space.element_location(&window) else { return };
        let pos = pointer.current_location();
        let min_y = glove_paint::theme::MENUBAR_H + deco_top(&window);
        let start_data = GrabStartData { focus: None, button: 0x110, location: pos };
        let grab = MoveSurfaceGrab::new(start_data, window, loc, min_y);
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Redimensionar a pedido do app (apps X11 com bordas próprias).
    pub fn start_resize(&mut self, window: smithay::desktop::Window, edges: crate::grabs::ResizeEdge, serial: smithay::utils::Serial) {
        let Some(pointer) = self.seat.get_pointer() else { return };
        let Some(loc) = self.space.element_location(&window) else { return };
        let size = window.geometry().size;
        let pos = pointer.current_location();
        let start_data = GrabStartData { focus: None, button: 0x110, location: pos };
        let grab = ResizeSurfaceGrab::start(start_data, window, edges, Rectangle::new(loc, size));
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }
}

/// Código evdev de uma tecla (layout EUA), para o modo de depuração.
fn debug_keycode(k: &str) -> (u32, bool) {
    const ROW: [(&str, u32); 47] = [
        ("1", 2), ("2", 3), ("3", 4), ("4", 5), ("5", 6), ("6", 7), ("7", 8), ("8", 9), ("9", 10), ("0", 11),
        ("q", 16), ("w", 17), ("e", 18), ("r", 19), ("t", 20), ("y", 21), ("u", 22), ("i", 23), ("o", 24), ("p", 25),
        ("a", 30), ("s", 31), ("d", 32), ("f", 33), ("g", 34), ("h", 35), ("j", 36), ("k", 37), ("l", 38),
        ("z", 44), ("x", 45), ("c", 46), ("v", 47), ("b", 48), ("n", 49), ("m", 50),
        (" ", 57), ("space", 57), ("enter", 28), ("backspace", 14), ("tab", 15), ("esc", 1),
        ("left", 105), ("right", 106), ("up", 103), ("down", 108), ("home", 102),
    ];
    if k == "end" {
        return (107, false);
    }
    if k == "delete" {
        return (111, false);
    }
    if k == "." {
        return (52, false);
    }
    if k == "," {
        return (51, false);
    }
    match k {
        "-" => return (12, false),
        "=" => return (13, false),
        "/" => return (53, false),
        "|" => return (43, true),
        "f2" => return (60, false),
        "f9" => return (67, false),
        "f10" => return (68, false),
        "f11" => return (87, false),
        "f12" => return (88, false),
        "mute" => return (113, false),
        "voldown" => return (114, false),
        "volup" => return (115, false),
        "brightdown" => return (224, false),
        "brightup" => return (225, false),
        _ => {}
    }
    let lower = k.to_lowercase();
    let shift = k.chars().count() == 1 && k != lower;
    ROW.iter().find(|(n, _)| *n == lower).map(|(_, c)| (*c, shift)).unwrap_or((0, false))
}
