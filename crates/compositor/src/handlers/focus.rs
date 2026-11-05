//! Foco do teclado: uma superfície Wayland ou uma janela X11 (que também
//! precisa do foco de entrada do X, além da superfície do Xwayland).

use std::borrow::Cow;

use smithay::{
    backend::input::KeyState,
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        Seat,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
    xwayland::X11Surface,
};

use crate::state::Glove;

#[derive(Clone, Debug, PartialEq)]
pub enum KbFocus {
    Wl(WlSurface),
    X11(X11Surface),
}

impl From<WlSurface> for KbFocus {
    fn from(s: WlSurface) -> Self {
        KbFocus::Wl(s)
    }
}

impl IsAlive for KbFocus {
    fn alive(&self) -> bool {
        match self {
            KbFocus::Wl(s) => s.alive(),
            KbFocus::X11(x) => x.alive(),
        }
    }
}

impl WaylandFocus for KbFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            KbFocus::Wl(s) => Some(Cow::Borrowed(s)),
            KbFocus::X11(x) => x.wl_surface().map(Cow::Owned),
        }
    }
}

impl KeyboardTarget<Glove> for KbFocus {
    fn enter(&self, seat: &Seat<Glove>, data: &mut Glove, keys: Vec<KeysymHandle<'_>>, serial: Serial) {
        match self {
            KbFocus::Wl(s) => KeyboardTarget::enter(s, seat, data, keys, serial),
            KbFocus::X11(x) => KeyboardTarget::enter(x, seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<Glove>, data: &mut Glove, serial: Serial) {
        match self {
            KbFocus::Wl(s) => KeyboardTarget::leave(s, seat, data, serial),
            KbFocus::X11(x) => KeyboardTarget::leave(x, seat, data, serial),
        }
    }

    fn key(&self, seat: &Seat<Glove>, data: &mut Glove, key: KeysymHandle<'_>, state: KeyState, serial: Serial, time: u32) {
        match self {
            KbFocus::Wl(s) => KeyboardTarget::key(s, seat, data, key, state, serial, time),
            KbFocus::X11(x) => KeyboardTarget::key(x, seat, data, key, state, serial, time),
        }
    }

    fn modifiers(&self, seat: &Seat<Glove>, data: &mut Glove, modifiers: ModifiersState, serial: Serial) {
        match self {
            KbFocus::Wl(s) => KeyboardTarget::modifiers(s, seat, data, modifiers, serial),
            KbFocus::X11(x) => KeyboardTarget::modifiers(x, seat, data, modifiers, serial),
        }
    }
}
