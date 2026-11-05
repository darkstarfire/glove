mod compositor;
pub mod focus;
mod x11;
mod xdg_shell;

use smithay::{
    delegate_data_device, delegate_fractional_scale, delegate_output, delegate_seat, delegate_viewporter,
    delegate_xdg_decoration,
    input::{Seat, SeatHandler, SeatState},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
        wayland_server::{protocol::wl_surface::WlSurface, Resource},
    },
    wayland::{
        compositor::with_states,
        fractional_scale::{with_fractional_scale, FractionalScaleHandler},
        output::OutputHandler,
        selection::{
            data_device::{set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler},
            SelectionHandler,
        },
        shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface},
    },
};

use crate::state::Glove;

impl SeatHandler for Glove {
    type KeyboardFocus = focus::KbFocus;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Glove> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        self.cursor_status = image;
        if !self.nested {
            self.dirty = true;
        }
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&focus::KbFocus>) {
        use smithay::wayland::seat::WaylandFocus;
        let dh = &self.display_handle;
        let client = focused.and_then(|f| f.wl_surface()).and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
        self.dirty = true;
    }
}
delegate_seat!(Glove);

impl SelectionHandler for Glove {
    type SelectionUserData = ();

    /// Um app quer colar o que foi copiado na mesa.
    fn send_selection(&mut self, _ty: smithay::wayland::selection::SelectionTarget, mime_type: String, fd: std::os::fd::OwnedFd, _seat: Seat<Self>, _user_data: &()) {
        let data = crate::desk::payload(&self.desk.clip, &mime_type);
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = std::fs::File::from(fd).write_all(&data);
        });
    }
}

impl DataDeviceHandler for Glove {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for Glove {
    /// Um app começou a arrastar: o ícone dele (se houver) segue o mouse, e a
    /// lista de arquivos é pedida já (para o caso de soltarem na mesa).
    fn started(&mut self, source: Option<smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource>, icon: Option<WlSurface>, _seat: Seat<Self>) {
        self.dnd_icon = icon;
        if let Some(s) = &source {
            self.desk_watch_drag(s);
        }
        self.dirty = true;
    }
    fn dropped(&mut self, target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        self.dnd_icon = None;
        // Fora de qualquer janela: a mesa (ou o Dock) recebe.
        if target.is_none() {
            let copy = self.seat.get_keyboard().is_some_and(|k| k.modifier_state().alt);
            self.desk.pending_drop = Some((self.pointer_pos, copy));
            self.desk_incoming_drop();
        } else {
            self.desk.incoming = None;
        }
        self.desk.hot = None;
        self.dirty = true;
    }
}
impl ServerDndGrabHandler for Glove {
    /// Arrastando da mesa: um app pediu os arquivos.
    fn send(&mut self, mime_type: String, fd: std::os::fd::OwnedFd, _seat: Seat<Self>) {
        self.desk_send(&mime_type, fd);
    }
    fn dropped(&mut self, _seat: Seat<Self>) {
        // Se nenhum app aceitar, vem `cancelled` logo depois e a mesa cuida.
        self.desk.drag_dropped = true;
        self.dirty = true;
    }
    fn cancelled(&mut self, _seat: Seat<Self>) {
        if self.desk.drag_dropped {
            let pos = self.pointer_pos;
            self.desk_drop(pos);
        }
        self.desk.drag = None;
        self.desk.drag_dropped = false;
        self.desk.hot = None;
        self.dirty = true;
    }
    fn finished(&mut self, _seat: Seat<Self>) {
        self.desk.drag = None;
        self.desk.drag_dropped = false;
        self.desk.hot = None;
        self.dirty = true;
    }
}
delegate_data_device!(Glove);

impl OutputHandler for Glove {}
delegate_output!(Glove);

// Bordas: o Glove prefere desenhar as próprias barras de título.
impl XdgDecorationHandler for Glove {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        // Tela de login: sem barras de título.
        let mode = if self.greeter { DecorationMode::ClientSide } else { DecorationMode::ServerSide };
        toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        let mode = if self.greeter { DecorationMode::ClientSide } else { mode };
        toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        let mode = if self.greeter { DecorationMode::ClientSide } else { DecorationMode::ServerSide };
        toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
}
delegate_xdg_decoration!(Glove);

// Escala fracionada: os apps desenham na escala exata da tela (ex.: 1.5).
impl FractionalScaleHandler for Glove {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let scale = self.scale();
        with_states(&surface, |states| {
            with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
        });
    }
}
delegate_fractional_scale!(Glove);
delegate_viewporter!(Glove);

impl Glove {
    /// Avisa todos os apps quando a escala da tela muda.
    pub fn update_client_scales(&mut self) {
        let scale = self.scale();
        let windows: Vec<_> = self.space.elements().cloned().chain(self.minimized.iter().chain(&self.hidden).map(|(w, _)| w.clone())).collect();
        for w in windows {
            w.with_surfaces(|_, states| {
                with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
            });
        }
    }
}
