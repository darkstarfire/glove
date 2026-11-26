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
        if std::env::var_os("GLOVE_DEBUG_CURSOR").is_some() {
            if let smithay::input::pointer::CursorImageStatus::Named(i) = &image {
                tracing::info!("cursor pedido: {i:?}");
            }
        }
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
        // Fora da linha principal: ler a imagem e escrever no cano podem demorar.
        let paths = self.desk.clip.clone();
        std::thread::spawn(move || {
            use std::io::Write;
            let data = crate::desk::payload(&paths, &mime_type);
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
        self.shot_thumb_drag_ended();
        self.dirty = true;
    }
    fn finished(&mut self, _seat: Seat<Self>) {
        self.desk.drag = None;
        self.desk.drag_dropped = false;
        self.desk.hot = None;
        self.shot_thumb_drag_ended();
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

// O mesmo pelo protocolo da KDE (o GTK pede as bordas por ele): a janela que pede as do
// servidor ganha a barra do Glove; a que desenha as próprias (barra de título do app)
// fica sem.
impl smithay::wayland::shell::kde::decoration::KdeDecorationHandler for Glove {
    fn kde_decoration_state(&self) -> &smithay::wayland::shell::kde::decoration::KdeDecorationState {
        &self.kde_decoration_state
    }

    fn request_mode(
        &mut self,
        surface: &WlSurface,
        decoration: &smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration,
        mode: smithay::reexports::wayland_server::WEnum<smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration::Mode>,
    ) {
        use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration::Mode as Kde;
        let smithay::reexports::wayland_server::WEnum::Value(pedido) = mode else { return };
        // Tela de login: sem barras de título.
        let modo = if self.greeter || pedido != Kde::Server { Kde::Client } else { Kde::Server };
        decoration.mode(modo);
        let m = if modo == Kde::Server { DecorationMode::ServerSide } else { DecorationMode::ClientSide };
        // O GTK pede antes de criar a janela (o xdg_toplevel): o pedido fica guardado na
        // superfície e vale quando ela vira janela (`new_toplevel`).
        with_states(surface, |st| {
            st.data_map.insert_if_missing(|| BordasKde(std::cell::Cell::new(None)));
            if let Some(b) = st.data_map.get::<BordasKde>() {
                b.0.set(Some(m));
            }
        });
        if let Some(t) = self.xdg_shell_state.toplevel_surfaces().iter().find(|t| t.wl_surface() == surface).cloned() {
            t.with_pending_state(|s| s.decoration_mode = Some(m));
            if t.is_initial_configure_sent() {
                t.send_pending_configure();
            }
        }
    }
}

/// As bordas pedidas pelo protocolo da KDE para uma superfície (antes de ela virar janela).
pub struct BordasKde(pub std::cell::Cell<Option<DecorationMode>>);

/// As bordas que a superfície pediu pelo protocolo da KDE, se pediu.
pub fn bordas_kde(surface: &WlSurface) -> Option<DecorationMode> {
    with_states(surface, |st| st.data_map.get::<BordasKde>().and_then(|b| b.0.get()))
}
smithay::delegate_kde_decoration!(Glove);

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

// Formato do cursor pedido pelo app (mãozinha nos links, barra nos textos).
impl smithay::wayland::tablet_manager::TabletSeatHandler for Glove {}
smithay::delegate_cursor_shape!(Glove);
delegate_viewporter!(Glove);

impl Glove {
    /// Avisa todos os apps quando a escala da tela muda.
    pub fn update_client_scales(&mut self) {
        let scale = self.scale();
        let windows: Vec<_> = self.space.elements().cloned().chain(self.stashed().cloned()).collect();
        for w in windows {
            w.with_surfaces(|_, states| {
                with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
            });
        }
    }
}
