//! Backend "aninhado": o Glove roda numa janela dentro do desktop atual.

use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::DisplayHandle,
        winit::{dpi::LogicalSize, window::WindowAttributes},
    },
    utils::Transform,
};

use crate::{render, Backend, CalloopData, Glove};

pub fn init(
    event_loop: &mut EventLoop<CalloopData>,
    state: &mut Glove,
    dh: &DisplayHandle,
) -> Result<(WinitGraphicsBackend<GlesRenderer>, Output), Box<dyn std::error::Error>> {
    let attrs = WindowAttributes::default()
        .with_title("Glove")
        .with_inner_size(LogicalSize::new(1280.0, 800.0));
    let (mut backend, winit) = winit::init_from_attributes::<GlesRenderer>(attrs)?;
    crate::dmabuf::init(state, dh, backend.renderer(), None);

    let mode = Mode { size: backend.window_size(), refresh: 60_000 };
    let output = Output::new(
        "glove-0".to_string(),
        PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "Glove".into(), model: "Janela".into() },
    );
    let _global = output.create_global::<Glove>(dh);
    // Usa a mesma escala do desktop por baixo (ex.: 1.5 em telas HiDPI).
    let scale = backend.window().scale_factor();
    output.change_current_state(Some(mode), Some(Transform::Flipped180), Some(Scale::Fractional(scale)), Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));
    state.nested = true;

    event_loop.handle().insert_source(winit, move |event, _, data| match event {
        WinitEvent::Resized { size, scale_factor } => {
            if let Backend::Winit(w) = &data.backend {
                w.output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, Some(Scale::Fractional(scale_factor)), None);
                data.state.write_outputs_info_nested(&w.output);
            }
            data.state.update_client_scales();
            data.state.dirty = true;
        }
        WinitEvent::Input(event) => data.state.process_input_event(event),
        // Perdeu o foco no desktop por baixo: solta as teclas que ficaram presas
        // (o "soltar" foi para outra janela e nunca chegaria aqui).
        WinitEvent::Focus(false) => data.state.release_all_keys(),
        WinitEvent::Redraw => render::render_frame(data),
        WinitEvent::CloseRequested => data.state.loop_signal.stop(),
        _ => {}
    })?;

    Ok((backend, output))
}
