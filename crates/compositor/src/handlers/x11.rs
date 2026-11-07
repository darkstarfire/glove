//! Apps X11 pelo Xwayland: o Glove é o gerenciador de janelas delas. As
//! janelas normais ganham a moldura do Glove; menus e dicas (override-redirect)
//! ficam por cima, onde o app pedir.

use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle, SERIAL_COUNTER},
    wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};

use crate::{state::Glove, CalloopData};

impl XWaylandShellHandler for Glove {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}
smithay::delegate_xwayland_shell!(Glove);

impl XWaylandShellHandler for CalloopData {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.state.xwayland_shell_state
    }
}

/// A janela do Glove que mostra essa superfície X11.
fn find(state: &Glove, x: &X11Surface) -> Option<Window> {
    state.space.elements().chain(state.stashed()).find(|w| w.x11_surface() == Some(x)).cloned()
}

impl Glove {
    /// Liga o Xwayland (se existir). Os apps abertos depois recebem o DISPLAY.
    pub fn start_xwayland(&mut self) {
        if self.greeter || std::env::var_os("GLOVE_SEM_X11").is_some() || !self.config.x11_apps {
            return;
        }
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| (),
        );
        let (xwayland, client) = match spawned {
            Ok(v) => v,
            Err(e) => {
                tracing::info!("sem apps X11 (Xwayland não abriu: {e})");
                return;
            }
        };
        let res = self.loop_handle.insert_source(xwayland, move |event, _, data: &mut CalloopData| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(data.state.loop_handle.clone(), x11_socket, client.clone()) {
                    Ok(wm) => {
                        data.state.xwm = Some(wm);
                        data.state.xdisplay = Some(display_number);
                        tracing::info!("apps X11 no display :{display_number}");
                        if !data.state.nested && !data.state.greeter {
                            crate::state::export_session_env(&data.state.socket_name, Some(display_number));
                        }
                    }
                    Err(e) => tracing::warn!("gerenciador de janelas X11 não iniciou: {e}"),
                }
            }
            XWaylandEvent::Error => tracing::warn!("o Xwayland saiu ao iniciar"),
        });
        if let Err(e) = res {
            tracing::warn!("Xwayland: {e}");
        }
    }

    /// Avisa o app X11 da posição e do tamanho que ele tem na tela.
    pub fn sync_x11(&self, w: &Window) {
        let Some(x) = w.x11_surface() else { return };
        if let Some(loc) = self.space.element_location(w) {
            let _ = x.configure(Rectangle::<i32, Logical>::new(loc, x.geometry().size));
        }
    }

    fn x11_overlay_index(&self, x: &X11Surface) -> Option<usize> {
        self.x11_overlays.iter().position(|w| w.x11_surface() == Some(x))
    }
}

impl XwmHandler for Glove {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("X11Wm")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if window.set_mapped(true).is_err() {
            return;
        }
        let st = self;
        let w = Window::new_x11_window(window.clone());
        let area = st.work_area();
        st.space.map_element(w.clone(), (area.loc.x + 80, area.loc.y + 40), true);
        if window.is_maximized() && !crate::shell::fixed_size(&w) {
            st.mark_born_zoomed(&w);
        }
        // Diálogos e janelas pequenas também passam pelo posicionamento do Glove.
        st.place_new_window(&w);
        st.sync_x11(&w);
        st.focus_window(Some(w));
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let st = self;
        if st.x11_overlay_index(&window).is_none() {
            st.x11_overlays.push(Window::new_x11_window(window));
        }
        st.dirty = true;
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let st = self;
        if let Some(i) = st.x11_overlay_index(&window) {
            st.x11_overlays.remove(i);
        }
        if let Some(w) = find(st, &window) {
            let was_focused = st.focused_window().as_ref() == Some(&w);
            st.space.unmap_elem(&w);
            st.minimized.retain(|(m, _)| *m != w);
            st.hidden.retain(|(h, _)| *h != w);
            st.mesas.away.retain(|(h, _, _)| *h != w);
            if was_focused {
                st.focus_topmost(None);
            }
        }
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
        st.dirty = true;
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(i) = self.x11_overlay_index(&window) {
            self.x11_overlays.remove(i);
        }
        self.dirty = true;
    }

    fn configure_request(&mut self, _xwm: XwmId, window: X11Surface, x: Option<i32>, y: Option<i32>, w: Option<u32>, h: Option<u32>, _reorder: Option<Reorder>) {
        // Tamanho: o que o app pedir. Posição: só antes de aparecer (depois quem move é o usuário).
        let mut geo = window.geometry();
        let mapped = find(self, &window);
        if mapped.is_none() {
            if let Some(x) = x {
                geo.loc.x = x;
            }
            if let Some(y) = y {
                geo.loc.y = y;
            }
        } else if let Some(loc) = mapped.as_ref().and_then(|m| self.space.element_location(m)) {
            geo.loc = loc;
        }
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
        self.dirty = true;
    }

    fn configure_notify(&mut self, _xwm: XwmId, window: X11Surface, _geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        // Menus e dicas se posicionam sozinhos: só redesenha.
        if window.is_override_redirect() {
            self.dirty = true;
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = find(self, &window) {
            if !window.is_maximized() {
                self.toggle_zoom(&w);
            }
        }
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = find(self, &window) {
            if window.is_maximized() {
                self.toggle_zoom(&w);
            }
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = find(self, &window) {
            self.set_fullscreen(&w, true);
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(w) = find(self, &window) {
            self.set_fullscreen(&w, false);
        }
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let w = self.space.elements().find(|w| w.x11_surface() == Some(&window)).cloned();
        if let Some(w) = w {
            self.minimize_window(&w);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, edge: X11ResizeEdge) {
        let Some(w) = self.space.elements().find(|w| w.x11_surface() == Some(&window)).cloned() else { return };
        use crate::grabs::resize_grab::ResizeEdge;
        let e = match edge {
            X11ResizeEdge::Top => ResizeEdge::TOP,
            X11ResizeEdge::Bottom => ResizeEdge::BOTTOM,
            X11ResizeEdge::Left => ResizeEdge::LEFT,
            X11ResizeEdge::Right => ResizeEdge::RIGHT,
            X11ResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
            X11ResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
            X11ResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
            X11ResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
        };
        self.start_resize(w, e, SERIAL_COUNTER.next_serial());
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        let Some(w) = self.space.elements().find(|w| w.x11_surface() == Some(&window)).cloned() else { return };
        self.start_move(w, SERIAL_COUNTER.next_serial());
    }
}

/// O gerenciador de janelas X11 recebe os eventos com os dados do laço;
/// tudo vai para o `Glove`.
impl XwmHandler for CalloopData {
    fn xwm_state(&mut self, xwm: XwmId) -> &mut X11Wm {
        self.state.xwm_state(xwm)
    }
    fn new_window(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.new_window(xwm, window)
    }
    fn new_override_redirect_window(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.new_override_redirect_window(xwm, window)
    }
    fn map_window_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.map_window_request(xwm, window)
    }
    fn mapped_override_redirect_window(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.mapped_override_redirect_window(xwm, window)
    }
    fn unmapped_window(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.unmapped_window(xwm, window)
    }
    fn destroyed_window(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.destroyed_window(xwm, window)
    }
    fn configure_request(&mut self, xwm: XwmId, window: X11Surface, x: Option<i32>, y: Option<i32>, w: Option<u32>, h: Option<u32>, reorder: Option<Reorder>) {
        self.state.configure_request(xwm, window, x, y, w, h, reorder)
    }
    fn configure_notify(&mut self, xwm: XwmId, window: X11Surface, geometry: Rectangle<i32, Logical>, above: Option<u32>) {
        self.state.configure_notify(xwm, window, geometry, above)
    }
    fn maximize_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.maximize_request(xwm, window)
    }
    fn unmaximize_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.unmaximize_request(xwm, window)
    }
    fn minimize_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.minimize_request(xwm, window)
    }
    fn fullscreen_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.fullscreen_request(xwm, window)
    }
    fn unfullscreen_request(&mut self, xwm: XwmId, window: X11Surface) {
        self.state.unfullscreen_request(xwm, window)
    }
    fn resize_request(&mut self, xwm: XwmId, window: X11Surface, button: u32, edge: X11ResizeEdge) {
        self.state.resize_request(xwm, window, button, edge)
    }
    fn move_request(&mut self, xwm: XwmId, window: X11Surface, button: u32) {
        self.state.move_request(xwm, window, button)
    }
}
