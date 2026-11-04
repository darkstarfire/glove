mod compositor;
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
    type KeyboardFocus = WlSurface;
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

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
        self.dirty = true;
    }
}
delegate_seat!(Glove);

impl SelectionHandler for Glove {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Glove {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for Glove {}
impl ServerDndGrabHandler for Glove {}
delegate_data_device!(Glove);

impl OutputHandler for Glove {}
delegate_output!(Glove);

// Bordas: o Glove prefere desenhar as próprias barras de título.
impl XdgDecorationHandler for Glove {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|s| s.decoration_mode = Some(DecorationMode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|s| s.decoration_mode = Some(DecorationMode::ServerSide));
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
        let windows: Vec<_> = self.space.elements().cloned().chain(self.minimized.iter().map(|(w, _)| w.clone())).collect();
        for w in windows {
            w.with_surfaces(|_, states| {
                with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
            });
        }
    }
}
