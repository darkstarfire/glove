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
    // GLOVE_SCALE=1.5: outra escala (testar telas HiDPI numa tela comum).
    let forcada = std::env::var("GLOVE_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= 1.0 && *v <= 4.0);
    let scale = forcada.unwrap_or_else(|| backend.window().scale_factor());
    output.change_current_state(Some(mode), Some(Transform::Flipped180), Some(Scale::Fractional(scale)), Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));
    state.nested = true;
    // Tests: GLOVE_FAKE_EXTRA=1920x1080 puts a monitor that does not exist to the right of
    // this window (nothing shows it; "shot2" of the debug control takes what it would show).
    if let Some((w, h)) = std::env::var("GLOVE_FAKE_EXTRA").ok().and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))) {
        let extra = Output::new("FAKE-1".to_string(), PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "Glove".into(), model: "Teste".into() });
        let m = Mode { size: (w, h).into(), refresh: 60_000 };
        extra.change_current_state(Some(m), None, Some(Scale::Fractional(1.0)), None);
        extra.set_preferred(m);
        let x = (backend.window_size().w as f64 / scale).round() as i32;
        state.space.map_output(&extra, (x, 0));
    }

    event_loop.handle().insert_source(winit, move |event, _, data| match event {
        WinitEvent::Resized { size, scale_factor } => {
            if let Backend::Winit(w) = &data.backend {
                w.output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, Some(Scale::Fractional(forcada.unwrap_or(scale_factor))), None);
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
