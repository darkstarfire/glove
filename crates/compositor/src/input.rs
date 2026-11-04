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

#[derive(Clone, Copy, Debug, PartialEq)]
enum KeyAction {
    Terminal,
    Close,
    Minimize,
    Zoom,
    Cycle,
    Preferences,
    Screenshot,
    ForceQuit,
    Quit,
    /// Trocar para o console N (Ctrl+Alt+F1…F12, só na sessão própria).
    Vt(i32),
    /// Ctrl+F2: abre a barra de menu pelo teclado.
    MenuBar,
    /// Tecla com um menu aberto.
    Menu(MenuKey),
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
            "force_quit" => Self::ForceQuit,
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
                let Some(output) = self.space.outputs().next() else { return };
                let geo = self.space.output_geometry(output).unwrap();
                let pos = event.position_transformed(geo.size) + geo.loc.to_f64();
                self.on_motion(pos, event.time_msec());
            }
            // Mouse e touchpad de verdade (sessão própria): movimento relativo.
            InputEvent::PointerMotion { event, .. } => {
                let (w, h) = self.output_size();
                let mut pos = self.pointer_pos + event.delta();
                pos.x = pos.x.clamp(0.0, w as f64 - 1.0);
                pos.y = pos.y.clamp(0.0, h as f64 - 1.0);
                self.on_motion(pos, event.time_msec());
                self.dirty = true;
            }
            InputEvent::PointerButton { event, .. } => self.on_button(event.button_code(), event.state(), event.time_msec()),
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();
                // Na sessão própria quem inverte é o libinput (mouse e touchpad separados).
                let sign = if self.nested && self.config.pointer.natural_scroll { -1.0 } else { 1.0 };
                let amount = |a| sign * event.amount(a).unwrap_or_else(|| event.amount_v120(a).unwrap_or(0.0) * 15.0 / 120.0);
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
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        let action = keyboard.input::<Option<KeyAction>, _>(self, code.into(), state, serial, time, |st, mods, handle| {
            if state == KeyState::Released {
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
            // Nome da tecla sem modificadores (independe do layout para letras).
            let raw = handle.raw_latin_sym_or_raw_current_sym().unwrap_or_else(|| handle.modified_sym());
            let name = smithay::input::keyboard::xkb::keysym_get_name(raw).to_lowercase();
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
            // Ctrl+F2: barra de menu pelo teclado (como no Mac OS X).
            if mods.ctrl && !mods.alt && !mods.shift && !mods.logo && name == "f2" {
                st.suppressed_keys.push(code);
                return FilterResult::Intercept(Some(KeyAction::MenuBar));
            }
            let action = st.bindings.iter().find_map(|b| {
                // Dentro de outro desktop o Super costuma ser capturado: Alt vale como Super.
                let logo_ok = if b.logo { mods.logo || (st.nested && mods.alt && !b.alt) } else { !mods.logo };
                let alt_ok = if b.logo && st.nested && !b.alt { true } else { b.alt == mods.alt };
                (b.key == name && b.ctrl == mods.ctrl && b.shift == mods.shift && logo_ok && alt_ok).then_some(b.action)
            });
            match action {
                Some(a) => {
                    st.suppressed_keys.push(code);
                    FilterResult::Intercept(Some(a))
                }
                None => FilterResult::Forward,
            }
        });

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
            Some(KeyAction::Cycle) => {
                // A janela mais ao fundo vem para a frente.
                let bottom = self.space.elements().next().cloned();
                if let Some(w) = bottom {
                    self.focus_window(Some(w));
                }
            }
            Some(KeyAction::Zoom) => {
                if let Some(w) = self.focused_window() {
                    self.toggle_zoom(&w);
                }
            }
            Some(KeyAction::Preferences) => self.open_preferences(),
            Some(KeyAction::Screenshot) => self.request_screenshot(),
            Some(KeyAction::ForceQuit) => self.run_menu_action(crate::shell::MenuAction::ForceQuit),
            Some(KeyAction::Quit) => self.loop_signal.stop(),
            Some(KeyAction::Vt(n)) => self.vt_request = Some(n),
            Some(KeyAction::MenuBar) => self.open_menubar_by_key(),
            Some(KeyAction::Menu(k)) => self.menu_key(k),
            None => {}
        }
    }

    fn update_hover(&mut self, pos: Point<f64, Logical>) {
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

        // Dock (ampliação segue o mouse)
        match &hit {
            Hit::Dock(i) => {
                let x = (pos.x - self.chrome.dock_pos().x as f64) as f32;
                self.chrome.dock_hover_x = Some(x);
                self.chrome.dock_last_x = x;
                self.chrome.animate_dock_zoom(1.0);
                // Recalcula já para o item sob o mouse ficar certo.
                self.prepare_dock();
                let d = self.chrome.dock_pos();
                let idx = self
                    .chrome
                    .dock_layout
                    .as_ref()
                    .and_then(|l| chrome::dock_item_at(l, x, (pos.y - d.y as f64) as f32))
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
        self.pointer_pos = pos;
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

        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            let hit = self.hit_test(pos);
            if let Some(cur) = self.chrome.menu.as_ref().map(|m| m.target) {
                // Com o menu aberto: clicar no mesmo título fecha, em outro troca;
                // fora dos menus, só fecha.
                match hit {
                    Hit::Menu(..) => {}
                    Hit::MenuBar { target: Some(t) } if t != cur => self.open_bar_menu(t),
                    _ => self.close_menu(),
                }
                swallow = true;
            } else {
                match hit {
                    Hit::MenuBar { target } => {
                        if let (Some(t), BTN_LEFT) = (target, button) {
                            self.open_bar_menu(t);
                        }
                        swallow = true;
                    }
                    Hit::Dock(i) => {
                        self.chrome.dock_pressed = i;
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
                            then = Then::Move(window);
                        }
                    }
                    Hit::Resize(window, edges) => {
                        self.focus_window(Some(window.clone()));
                        if button == BTN_LEFT {
                            then = Then::Resize(window, edges);
                        }
                    }
                    Hit::Client(window) => {
                        if self.focused_window().as_ref() != Some(&window) {
                            self.focus_window(Some(window));
                        }
                    }
                    Hit::DesktopIcon => {
                        self.focus_window(None);
                        self.click_desk_icon();
                        swallow = true;
                    }
                    Hit::Desktop => {
                        self.focus_window(None);
                        if self.chrome.desk_selected {
                            self.chrome.desk_selected = false;
                            self.dirty = true;
                        }
                    }
                    Hit::Menu(..) => {}
                }
            }
            self.chrome.swallow_release = swallow;
        } else if state == ButtonState::Released {
            let hit = self.hit_test(pos);
            // Menu: escolher um item ao soltar
            if let (Some(_), Hit::Menu(l, Some(i))) = (&self.chrome.menu, &hit) {
                self.menu_activate(*l, *i);
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
                let grab = MoveSurfaceGrab { start_data: start(pos), window, initial_window_location: loc, min_y };
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
        _ => {}
    }
    let lower = k.to_lowercase();
    let shift = k.chars().count() == 1 && k != lower;
    ROW.iter().find(|(n, _)| *n == lower).map(|(_, c)| (*c, shift)).unwrap_or((0, false))
}
