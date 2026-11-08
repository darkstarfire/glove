//! linux-dmabuf: os apps que desenham pela placa de vídeo (mpv, Firefox, VS Code,
//! jogos, o XWayland) entregam a imagem direto da memória da placa. Sem isto, o
//! Mesa não descobre a placa ("failed to get driver name for fd -1") e cai no
//! llvmpipe, o OpenGL feito no processador, ou nem abre.

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        drm::{DrmNode, NodeType},
        egl::EGLDevice,
        renderer::{gles::GlesRenderer, ImportDma},
    },
    delegate_dmabuf,
    reexports::wayland_server::DisplayHandle,
    wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};

use crate::state::Glove;

/// A placa de render: a do EGL, a dada (a placa aberta na sessão própria) ou a
/// primeira de /dev/dri (dentro de outro desktop).
fn render_node(renderer: &GlesRenderer, hint: Option<&std::path::Path>) -> Option<DrmNode> {
    let render = |n: DrmNode| n.node_with_type(NodeType::Render).and_then(Result::ok).or(Some(n));
    EGLDevice::device_for_display(renderer.egl_context().display())
        .ok()
        .and_then(|d| d.try_get_render_node().ok().flatten())
        .or_else(|| hint.and_then(|p| DrmNode::from_path(p).ok()).and_then(render))
        .or_else(|| {
            let mut v: Vec<_> = std::fs::read_dir("/dev/dri").ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("renderD"))).collect();
            v.sort();
            v.into_iter().find_map(|p| DrmNode::from_path(p).ok())
        })
}

/// Anuncia o dmabuf com os formatos que o renderizador importa e a placa principal
/// (versão 4: o Mesa escolhe a placa certa por ela).
pub fn init(state: &mut Glove, dh: &DisplayHandle, renderer: &GlesRenderer, hint: Option<&std::path::Path>) {
    let formats: Vec<_> = renderer.dmabuf_formats().into_iter().collect();
    if formats.is_empty() {
        tracing::warn!("dmabuf: o renderizador não importa nenhum formato; os apps usam memória comum");
        return;
    }
    let node = render_node(renderer, hint);
    let feedback = node.and_then(|n| DmabufFeedbackBuilder::new(n.dev_id(), formats.clone()).build().ok());
    let global = match feedback {
        Some(fb) => {
            tracing::info!("dmabuf v4 na placa {:?} ({} formatos)", node.map(|n| n.dev_path()), formats.len());
            state.dmabuf_state.create_global_with_default_feedback::<Glove>(dh, &fb)
        }
        None => {
            tracing::info!("dmabuf v3 ({} formatos; placa principal desconhecida)", formats.len());
            state.dmabuf_state.create_global::<Glove>(dh, formats)
        }
    };
    state.dmabuf_global = Some(global);
}

impl DmabufHandler for Glove {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, _dmabuf: Dmabuf, notifier: ImportNotifier) {
        // O renderizador fica no backend: ele importa o buffer na hora de desenhar.
        // Os formatos anunciados são os que ele importa, então aceita aqui.
        let _ = notifier.successful::<Glove>();
    }
}

delegate_dmabuf!(Glove);
