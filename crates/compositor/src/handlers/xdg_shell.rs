use smithay::{
    delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKeyboardGrab, PopupKind, PopupManager, PopupPointerGrab, PopupUngrabStrategy, Space, Window},
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    handlers::focus::KbFocus,
    shell::deco_top,
    state::Glove,
};

impl XdgShellHandler for Glove {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn client_pong(&mut self, client: smithay::wayland::shell::xdg::ShellClient) {
        self.on_pong(client);
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // As bordas que o app pediu pelo protocolo da KDE (o GTK pede antes da janela existir).
        if let Some(m) = super::bordas_kde(surface.wl_surface()) {
            surface.with_pending_state(|s| s.decoration_mode = Some(m));
        }
        let window = Window::new_wayland_window(surface);
        // Posição provisória; a definitiva vem no primeiro quadro (já com tamanho).
        let area = self.work_area();
        self.space.map_element(window.clone(), (area.loc.x + 80, area.loc.y + 40), false);
        self.focus_window(Some(window));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let was_focused = self.focused_window().is_none_or(|w| w.toplevel() == Some(&surface));
        if was_focused {
            self.focus_topmost(Some(surface.wl_surface()));
        }
        self.dirty = true;
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for(&surface) {
            if !surface.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized)) {
                self.toggle_zoom(&w);
            }
        }
        if !surface.is_initial_configure_sent() {
            return;
        }
        surface.send_pending_configure();
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for(&surface) {
            if surface.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized)) {
                self.toggle_zoom(&w);
            }
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>) {
        match self.window_for(&surface) {
            Some(w) => self.set_fullscreen(&w, true),
            // Antes de aparecer: só marca (o primeiro tamanho já vem cheio na próxima).
            None => surface.with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Fullscreen);
            }),
        }
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for(&surface) {
            self.set_fullscreen(&w, false);
        }
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for(&surface) {
            self.minimize_window(&w);
        }
    }

    fn title_changed(&mut self, _surface: ToplevelSurface) {
        self.dirty = true;
    }

    fn app_id_changed(&mut self, _surface: ToplevelSurface) {
        self.dirty = true;
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == wl_surface))
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();

            let min_y = glove_paint::theme::MENUBAR_H + deco_top(&window);
            let grab = MoveSurfaceGrab::new(start_data, window, initial_window_location, min_y);

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == wl_surface))
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    /// O app pediu o grab do popup (lista suspensa, menu): como no anvil, o mouse
    /// fica com o app dono do popup (clicar fora dele, noutro app ou na mesa, fecha
    /// com `popup_done`) e o teclado vai para o popup até ele fechar.
    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let Some(seat) = Seat::<Glove>::from_resource(&seat) else { return };
        let kind = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&kind) else { return };
        // Só popups de janelas Wayland (a raiz precisa ser uma janela do espaço).
        if !self.space.elements().any(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root)) {
            return;
        }
        let mut grab = match self.popups.grab_popup(KbFocus::Wl(root), kind, &seat, serial) {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!("grab do popup recusado: {e:?}");
                return;
            }
        };
        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed() && !(keyboard.has_grab(serial) || keyboard.has_grab(grab.previous_serial().unwrap_or(serial))) {
                tracing::warn!("grab do popup recusado: o teclado já tem outro grab");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed() && !(pointer.has_grab(serial) || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial()))) {
                tracing::warn!("grab do popup recusado: o mouse já tem outro grab");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
        self.popup_grab = Some(grab);
        self.dirty = true;
    }
}

// Xdg Shell
delegate_xdg_shell!(Glove);

fn check_grab(
    seat: &Seat<Glove>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<Glove>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        if !initial_configure_sent {
            window.toplevel().unwrap().send_configure();
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    // NOTE: This should never fail as the initial configure is always
                    // allowed.
                    xdg.send_configure().expect("initial configure failed");
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl Glove {
    /// O app fechou o popup que tinha o grab (escolheu um item, apertou Esc): devolve
    /// o teclado à janela na hora. Sem isso o foco só voltaria no próximo evento, e a
    /// janela ficaria com a barra de título apagada até lá. Chamado a cada volta do laço.
    pub fn end_popup_grab(&mut self) {
        let Some(grab) = self.popup_grab.as_ref() else { return };
        if !grab.has_ended() {
            return;
        }
        let grab = self.popup_grab.take().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        // O grab do mouse também sai já. Deixado para o próximo movimento, ele se
        // desfaz sem repassar esse movimento (o do smithay), e o app recebe a entrada
        // na janela com a posição antiga do mouse: o clique seguinte cai noutro lugar.
        if let Some(pointer) = self.seat.get_pointer() {
            if pointer.is_grabbed() && (pointer.has_grab(grab.serial()) || grab.previous_serial().is_some_and(|s| pointer.has_grab(s))) {
                let time = self.start_time.elapsed().as_millis() as u32;
                pointer.unset_grab(self, serial, time);
            }
        }
        if let Some(keyboard) = self.seat.get_keyboard() {
            if keyboard.is_grabbed() && (keyboard.has_grab(grab.serial()) || grab.previous_serial().is_some_and(|s| keyboard.has_grab(s))) {
                keyboard.unset_grab(self);
            }
            // Só se o foco ainda está no popup que fechou (ou em nada): não rouba o
            // foco de uma janela que o usuário escolheu nesse meio-tempo.
            let stale = keyboard.current_focus().is_none_or(|f| !smithay::utils::IsAlive::alive(&f));
            if stale {
                if let Some(root) = grab.current_grab().filter(|r| smithay::utils::IsAlive::alive(r)) {
                    keyboard.set_focus(self, Some(root), serial);
                }
            }
        }
        // O mouse volta a apontar para o que está embaixo dele (a janela), na posição certa.
        self.refresh_pointer_focus();
        self.dirty = true;
    }

    /// Recalcula a superfície sob o mouse parado (um popup apareceu embaixo dele).
    pub fn refresh_pointer_focus(&mut self) {
        let Some(pointer) = self.seat.get_pointer() else { return };
        let pos = pointer.current_location();
        let under = self.surface_under(pos);
        if pointer.current_focus() == under.as_ref().map(|u| u.0.clone()) {
            return;
        }
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let time = self.start_time.elapsed().as_millis() as u32;
        pointer.motion(self, under, &smithay::input::pointer::MotionEvent { location: pos, serial, time });
        pointer.frame(self);
    }

    fn window_for(&self, surface: &ToplevelSurface) -> Option<Window> {
        self.space.elements().find(|w| w.toplevel() == Some(surface)).cloned()
    }

    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root))
        else {
            return;
        };

        let Some(window_geo) = self.space.element_geometry(window) else { return };
        // O monitor em que está a janela (pelo meio dela); senão, o principal.
        let center: smithay::utils::Point<f64, smithay::utils::Logical> = ((window_geo.loc.x + window_geo.size.w / 2) as f64, (window_geo.loc.y + window_geo.size.h / 2) as f64).into();
        let Some(output_geo) = self.output_rect_at(center).or_else(|| self.primary().and_then(|o| self.space.output_geometry(o))) else { return };

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        // Como no Mac: o popup não cobre a barra de menus nem o Dock (a área útil),
        // se o monitor for o da barra; nos outros, o monitor inteiro.
        let work = self.work_area_of(output_geo);
        let mut target = output_geo.intersection(work).unwrap_or(output_geo);
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
