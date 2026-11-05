use crate::{grabs::resize_grab, state::{ClientState, Glove}};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        protocol::{wl_buffer, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState,
        },
        shm::{ShmHandler, ShmState},
    },
};

use super::xdg_shell;

impl CompositorHandler for Glove {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        // O Xwayland é um cliente com os dados do próprio Smithay.
        if let Some(x) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &x.compositor_state;
        }
        &client.get_data::<ClientState>().expect("cliente sem dados").compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            use smithay::wayland::seat::WaylandFocus;
            let window = self.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == root)).cloned();
            if let Some(window) = window {
                window.on_commit();
                self.place_new_window(&window);
            } else if let Some(window) = self.minimized.iter().chain(&self.hidden).map(|(w, _)| w).find(|w| w.wl_surface().is_some_and(|s| *s == root)).cloned() {
                // Minimizada (miniatura viva no Dock) ou oculta: o conteúdo mudou.
                window.on_commit();
                if self.minimized.iter().any(|(w, _)| *w == window) {
                    self.min_commit_pending = true;
                }
            }
        };
        self.dirty = true;

        xdg_shell::handle_commit(&mut self.popups, &self.space, surface);
        resize_grab::handle_commit(&mut self.space, surface);
    }
}

impl BufferHandler for Glove {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Glove {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Glove);
delegate_shm!(Glove);
