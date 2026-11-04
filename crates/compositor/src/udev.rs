//! Sessão própria: o Glove desenha direto na placa de vídeo (DRM/KMS) e lê teclado,
//! mouse e touchpad pelo libinput. É o que roda quando se escolhe "Glove" na tela de
//! login (ou ao iniciar de um console de texto).
//!
//! Uma tela por vez: a escolhida nas Preferências (Monitor), ou a externa, se houver,
//! ou a do notebook. Só desenha quando algo mudou, no ritmo do monitor (vblank).

use std::time::Duration;

use glove_config::{auto_scale, parse_mode, OutputInfo, OutputsInfo};
use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::GbmFramebufferExporter,
            DrmDevice, DrmDeviceFd, DrmEvent,
        },
        egl::{EGLContext, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::gles::GlesRenderer,
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{self, UdevBackend, UdevEvent},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{connector, crtc, Mode as DrmMode, ModeTypeFlags},
        input::{self as libinput, AccelProfile, ClickMethod, Libinput},
        rustix::fs::OFlags,
        wayland_server::{backend::GlobalId, DisplayHandle},
    },
    utils::{DeviceFd, Transform},
};

use crate::{render, Backend, CalloopData, Glove};

type GbmDrmCompositor = DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// O monitor em uso.
struct Head {
    compositor: GbmDrmCompositor,
    connector: connector::Handle,
    name: String,
    mode: DrmMode,
    output: Output,
    global: GlobalId,
}

pub struct Drm {
    pub session: LibSeatSession,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    pub renderer: GlesRenderer,
    head: Option<Head>,
    /// Um quadro foi entregue e espera o vblank: o próximo só depois dele.
    pending: bool,
    /// A sessão está ativa (não trocou para outro console).
    active: bool,
    /// Teclados, mouses e touchpads (para reaplicar as preferências).
    devices: Vec<libinput::Device>,
    cursor_size: (u32, u32),
}

/// Um conector com monitor ligado.
#[derive(Clone)]
struct Connected {
    handle: connector::Handle,
    name: String,
    size_mm: (u32, u32),
    modes: Vec<DrmMode>,
    crtc: Option<crtc::Handle>,
}

fn connector_name(info: &connector::Info) -> String {
    format!("{}-{}", info.interface().as_str(), info.interface_id())
}

/// Tela embutida (notebook): fica em segundo plano quando há uma externa.
fn is_internal(name: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|p| name.starts_with(p))
}

fn mode_string(m: &DrmMode) -> String {
    let (w, h) = m.size();
    format!("{w}x{h}@{}", m.vrefresh())
}

/// Qual monitor usar: o escolhido, senão o externo, senão o primeiro.
fn choose_connector<'a>(list: &'a [Connected], wanted: &str) -> Option<&'a Connected> {
    list.iter()
        .find(|c| !wanted.is_empty() && c.name == wanted)
        .or_else(|| list.iter().find(|c| !is_internal(&c.name)))
        .or_else(|| list.first())
}

/// Qual resolução usar: a escolhida (se o monitor tiver), senão a recomendada.
fn choose_mode(modes: &[DrmMode], wanted: &str) -> Option<DrmMode> {
    if let Some((w, h, hz)) = parse_mode(wanted) {
        let mut same: Vec<&DrmMode> = modes.iter().filter(|m| m.size() == (w as u16, h as u16)).collect();
        same.sort_by_key(|m| (m.vrefresh() as i64 - hz as i64).abs());
        if let Some(m) = same.first() {
            return Some(**m);
        }
    }
    modes.iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).or_else(|| modes.first()).copied()
}

/// Conectores com monitor ligado (funciona também sem ser o dono da placa).
fn connected_on(dev: &impl smithay::reexports::drm::control::Device) -> Vec<Connected> {
    let Ok(res) = dev.resource_handles() else { return Vec::new() };
    let mut out = Vec::new();
    for &h in res.connectors() {
        let Ok(info) = dev.get_connector(h, true) else { continue };
        if info.state() != connector::State::Connected || info.modes().is_empty() {
            continue;
        }
        // Um controlador (CRTC) que esse conector consegue usar.
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|e| dev.get_encoder(*e).ok())
            .flat_map(|e| res.filter_crtcs(e.possible_crtcs()))
            .next();
        let mut modes = info.modes().to_vec();
        modes.retain(|m| !m.flags().contains(smithay::reexports::drm::control::ModeFlags::INTERLACE));
        out.push(Connected { handle: h, name: connector_name(&info), size_mm: info.size().unwrap_or((0, 0)), modes, crtc });
    }
    out
}

/// `glove --monitores`: mostra o que a sessão escolheria, sem tocar na tela.
pub fn probe(config: &glove_config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = udev::primary_gpu("seat0")?.or_else(|| udev::all_gpus("seat0").ok()?.into_iter().next()).ok_or("nenhuma placa de vídeo encontrada")?;
    let file = std::fs::File::open(&path)?;
    let fd = DrmDeviceFd::new(DeviceFd::from(std::os::fd::OwnedFd::from(file)));
    println!("Placa de vídeo: {}", path.display());
    let list = connected_on(&fd);
    for c in &list {
        let pref = choose_mode(&c.modes, "").map(|m| mode_string(&m)).unwrap_or_default();
        let auto = choose_mode(&c.modes, "").map(|m| auto_scale((m.size().0 as u32, m.size().1 as u32), c.size_mm)).unwrap_or(1.0);
        println!("  {}  {} mm × {} mm  recomendada {pref}  escala automática {auto}  ({} resoluções)", describe(&c.name), c.size_mm.0, c.size_mm.1, c.modes.len());
    }
    let cfg = &config.monitor;
    match choose_connector(&list, &cfg.output) {
        Some(c) => {
            let m = choose_mode(&c.modes, &cfg.mode).ok_or("monitor sem resoluções")?;
            let (w, h) = m.size();
            let scale = if cfg.scale > 0.0 { cfg.scale } else { auto_scale((w as u32, h as u32), c.size_mm) };
            println!("A sessão usaria: {} em {} com escala {scale} (área de {} × {} pontos)", c.name, mode_string(&m), (w as f64 / scale).round(), (h as f64 / scale).round());
        }
        None => println!("Nenhum monitor conectado."),
    }
    Ok(())
}

impl Drm {
    fn connected(&self) -> Vec<Connected> {
        connected_on(&self.drm)
    }

    /// Escolhe monitor e resolução pela configuração e liga a tela. Se já for a
    /// mesma, só troca o que mudou (resolução ou escala).
    fn configure(&mut self, state: &mut Glove, dh: &DisplayHandle) {
        let list = self.connected();
        let cfg = state.config.monitor.clone();
        write_outputs_info(&list, self.head.as_ref());
        let Some(c) = choose_connector(&list, &cfg.output).cloned() else {
            tracing::warn!("nenhum monitor conectado");
            return;
        };
        let Some(mode) = choose_mode(&c.modes, &cfg.mode) else { return };
        let (pw, ph) = mode.size();
        let scale = if cfg.scale > 0.0 { cfg.scale } else { auto_scale((pw as u32, ph as u32), c.size_mm) };

        if let Some(head) = self.head.as_mut().filter(|h| h.connector == c.handle) {
            if head.mode != mode {
                match head.compositor.use_mode(mode) {
                    Ok(()) => {
                        head.mode = mode;
                        head.output.change_current_state(Some(OutputMode::from(mode)), None, None, None);
                        tracing::info!("{}: resolução {}", head.name, mode_string(&mode));
                    }
                    Err(e) => tracing::warn!("{}: não consegui usar {}: {e}", head.name, mode_string(&mode)),
                }
            }
            if head.output.current_scale().fractional_scale() != scale {
                head.output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
                state.update_client_scales();
            }
            state.dirty = true;
            write_outputs_info(&list, self.head.as_ref());
            return;
        }

        // Monitor novo: desliga o anterior e liga este.
        if let Some(old) = self.head.take() {
            state.space.unmap_output(&old.output);
            dh.remove_global::<Glove>(old.global);
        }
        let Some(crtc) = c.crtc else {
            tracing::warn!("{}: sem controlador de vídeo livre", c.name);
            return;
        };
        let surface = match self.drm.create_surface(crtc, mode, &[c.handle]) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("{}: {e}", c.name);
                return;
            }
        };
        let output = Output::new(
            c.name.clone(),
            PhysicalProperties { size: (c.size_mm.0 as i32, c.size_mm.1 as i32).into(), subpixel: Subpixel::Unknown, make: "Glove".into(), model: c.name.clone() },
        );
        let global = output.create_global::<Glove>(dh);
        let omode = OutputMode::from(mode);
        output.change_current_state(Some(omode), Some(Transform::Normal), Some(Scale::Fractional(scale)), Some((0, 0).into()));
        output.set_preferred(omode);

        let allocator = GbmAllocator::new(self.gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(self.gbm.clone(), None);
        let formats = self.renderer.egl_context().dmabuf_render_formats().clone();
        let compositor = match DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            [Fourcc::Xrgb8888, Fourcc::Argb8888],
            formats,
            self.cursor_size.into(),
            Some(self.gbm.clone()),
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("{}: não consegui ligar a tela: {e}", c.name);
                dh.remove_global::<Glove>(global);
                return;
            }
        };
        state.space.map_output(&output, (0, 0));
        tracing::info!("monitor {} em {} (escala {scale})", c.name, mode_string(&mode));
        self.head = Some(Head { compositor, connector: c.handle, name: c.name.clone(), mode, output, global });
        self.pending = false;
        state.update_client_scales();
        state.keep_pointer_inside();
        state.dirty = true;
        write_outputs_info(&list, self.head.as_ref());
    }

    pub fn output(&self) -> Option<&Output> {
        self.head.as_ref().map(|h| &h.output)
    }

    /// Reaplica velocidade, toque para clicar etc. (as Preferências mudaram).
    pub fn apply_input_config(&mut self, config: &glove_config::Config) {
        for d in &mut self.devices {
            configure_device(d, config);
        }
    }
}

/// Avisa as Preferências sobre os monitores e resoluções disponíveis.
fn write_outputs_info(list: &[Connected], head: Option<&Head>) {
    let outputs = list
        .iter()
        .map(|c| {
            let active = head.is_some_and(|h| h.connector == c.handle);
            let mut modes: Vec<String> = Vec::new();
            for m in &c.modes {
                let s = mode_string(m);
                if !modes.contains(&s) {
                    modes.push(s);
                }
            }
            let preferred = choose_mode(&c.modes, "").map(|m| mode_string(&m)).unwrap_or_default();
            let auto = choose_mode(&c.modes, "").map(|m| auto_scale((m.size().0 as u32, m.size().1 as u32), c.size_mm)).unwrap_or(1.0);
            OutputInfo {
                name: c.name.clone(),
                description: describe(&c.name),
                size_mm: c.size_mm,
                active,
                current: head.filter(|_| active).map(|h| mode_string(&h.mode)).unwrap_or_default(),
                preferred,
                modes,
                scale: head.filter(|_| active).map(|h| h.output.current_scale().fractional_scale()).unwrap_or(auto),
                auto_scale: auto,
            }
        })
        .collect();
    OutputsInfo { nested: false, outputs }.save();
}

fn describe(name: &str) -> String {
    let kind = if is_internal(name) {
        "Tela embutida"
    } else if name.starts_with("HDMI") {
        "Monitor HDMI"
    } else if name.starts_with("DP") {
        "Monitor DisplayPort"
    } else if name.starts_with("DVI") {
        "Monitor DVI"
    } else if name.starts_with("VGA") {
        "Monitor VGA"
    } else {
        "Monitor"
    };
    format!("{kind} ({name})")
}

/// Preferências de mouse e touchpad num aparelho do libinput.
fn configure_device(d: &mut libinput::Device, config: &glove_config::Config) {
    let touchpad = d.config_tap_finger_count() > 0;
    if touchpad {
        let t = &config.touchpad;
        let _ = d.config_tap_set_enabled(t.tap_to_click);
        if d.config_accel_is_available() {
            let _ = d.config_accel_set_speed(t.speed.clamp(-1.0, 1.0));
        }
        if d.config_scroll_has_natural_scroll() {
            let _ = d.config_scroll_set_natural_scroll_enabled(t.natural_scroll);
        }
        if d.config_dwt_is_available() {
            let _ = d.config_dwt_set_enabled(t.disable_while_typing);
        }
        let method = if t.two_finger_right_click { ClickMethod::Clickfinger } else { ClickMethod::ButtonAreas };
        if d.config_click_methods().contains(&method) {
            let _ = d.config_click_set_method(method);
        }
    } else {
        let p = &config.pointer;
        if d.config_accel_is_available() {
            let _ = d.config_accel_set_speed(p.speed.clamp(-1.0, 1.0));
            let profile = if p.acceleration { AccelProfile::Adaptive } else { AccelProfile::Flat };
            if d.config_accel_profiles().contains(&profile) {
                let _ = d.config_accel_set_profile(profile);
            }
        }
        if d.config_scroll_has_natural_scroll() {
            let _ = d.config_scroll_set_natural_scroll_enabled(p.natural_scroll);
        }
    }
    if d.config_left_handed_is_available() {
        let _ = d.config_left_handed_set(config.pointer.left_handed);
    }
}

/// Espera a sessão ficar ativa (algumas telas de login entregam antes de ativar).
/// O estado só muda processando os avisos do libseat, por isso o laço próprio.
fn wait_active(
    session: &LibSeatSession,
    notifier: smithay::backend::session::libseat::LibSeatSessionNotifier,
) -> Result<smithay::backend::session::libseat::LibSeatSessionNotifier, Box<dyn std::error::Error>> {
    use smithay::reexports::calloop::Dispatcher;
    if session.is_active() {
        return Ok(notifier);
    }
    tracing::info!("esperando a sessão ficar ativa");
    let mut waiting: EventLoop<()> = EventLoop::try_new()?;
    let dispatcher = Dispatcher::new(notifier, |_, _, _: &mut ()| {});
    let token = waiting.handle().register_dispatcher(dispatcher.clone())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !session.is_active() && std::time::Instant::now() < deadline {
        waiting.dispatch(Some(Duration::from_millis(100)), &mut ())?;
    }
    waiting.handle().remove(token);
    drop(waiting);
    if !session.is_active() {
        return Err("a sessão não ficou ativa em 10 s (outro programa está com a tela?)".into());
    }
    Ok(dispatcher.into_source_inner())
}

pub fn init(event_loop: &mut EventLoop<CalloopData>, state: &mut Glove, dh: &DisplayHandle) -> Result<Box<Drm>, Box<dyn std::error::Error>> {
    let (mut session, notifier) = LibSeatSession::new().map_err(|e| format!("sem sessão (entre pela tela de login ou por um console de texto): {e}"))?;
    let seat = session.seat();
    let notifier = wait_active(&session, notifier)?;

    let path = udev::primary_gpu(&seat)?
        .or_else(|| udev::all_gpus(&seat).ok()?.into_iter().next())
        .ok_or("nenhuma placa de vídeo encontrada")?;
    tracing::info!("placa de vídeo: {}", path.display());
    // A tela de login pode levar um instante para soltar a placa: tenta por até 3 s.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let fd = loop {
        match session.open(&path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK) {
            Ok(fd) => break fd,
            Err(e) if std::time::Instant::now() < deadline => {
                tracing::debug!("placa ainda ocupada: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("não consegui abrir {}: {e}", path.display()).into()),
        }
    };
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd.clone())?;
    // SAFETY: o display EGL guarda uma cópia do GBM, que vive junto com ele.
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&egl)?;
    // SAFETY: contexto recém-criado, usado só por este renderizador.
    let renderer = unsafe { GlesRenderer::new(context)? };

    let cursor_size = {
        use smithay::reexports::drm::{Device as _, DriverCapability};
        let w = drm.get_driver_capability(DriverCapability::CursorWidth).unwrap_or(64) as u32;
        let h = drm.get_driver_capability(DriverCapability::CursorHeight).unwrap_or(64) as u32;
        (w.clamp(32, 512), h.clamp(32, 512))
    };

    let mut backend = Box::new(Drm { session: session.clone(), drm, gbm, renderer, head: None, pending: false, active: true, devices: Vec::new(), cursor_size });
    backend.configure(state, dh);
    if backend.head.is_none() {
        return Err("nenhum monitor pôde ser ligado".into());
    }

    // O ponteiro começa no meio da tela.
    let (w, h) = state.output_size();
    state.pointer_pos = (w as f64 / 2.0, h as f64 / 2.0).into();

    let handle = event_loop.handle();

    // Vblank: o quadro foi para a tela; desenha o próximo se algo mudou.
    handle.insert_source(drm_notifier, |event, _, data| {
        let Backend::Drm(drm) = &mut data.backend else { return };
        match event {
            DrmEvent::VBlank(_) => {
                if let Some(head) = drm.head.as_mut() {
                    if let Err(e) = head.compositor.frame_submitted() {
                        tracing::warn!("vblank: {e}");
                    }
                }
                drm.pending = false;
                if data.state.dirty {
                    render_drm(data);
                }
            }
            DrmEvent::Error(e) => tracing::warn!("erro do DRM: {e}"),
        }
    })?;

    // Rede de segurança: se um vblank se perder (suspensão, driver), a tela não congela.
    handle.insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, data| {
        if let Backend::Drm(drm) = &mut data.backend {
            if drm.pending && drm.active {
                drm.pending = false;
                if let Some(h) = drm.head.as_mut() {
                    let _ = h.compositor.reset_state();
                }
                data.state.dirty = true;
            }
        }
        TimeoutAction::ToDuration(Duration::from_secs(1))
    })?;

    // Teclado, mouse e touchpad.
    let mut li = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    li.udev_assign_seat(&seat).map_err(|_| "libinput: não consegui usar o assento")?;
    let input = LibinputInputBackend::new(li.clone());
    handle.insert_source(input, |mut event, _, data| {
        if let Backend::Drm(drm) = &mut data.backend {
            match &mut event {
                InputEvent::DeviceAdded { device } => {
                    configure_device(device, &data.state.config);
                    tracing::info!("aparelho de entrada: {}", device.name());
                    drm.devices.push(device.clone());
                }
                InputEvent::DeviceRemoved { device } => drm.devices.retain(|d| d != device),
                _ => {}
            }
        }
        data.state.process_input_event(event);
    })?;

    // Troca de console (Ctrl+Alt+F1…) e volta: larga e retoma a placa e a entrada.
    handle.insert_source(notifier, move |event, _, data| {
        let Backend::Drm(drm) = &mut data.backend else { return };
        match event {
            SessionEvent::PauseSession => {
                tracing::info!("sessão em segundo plano");
                li.suspend();
                drm.drm.pause();
                drm.active = false;
                data.state.release_all_keys();
            }
            SessionEvent::ActivateSession => {
                tracing::info!("sessão de volta");
                if li.resume().is_err() {
                    tracing::warn!("libinput não voltou");
                }
                if let Err(e) = drm.drm.activate(false) {
                    tracing::warn!("placa de vídeo não voltou: {e}");
                }
                drm.active = true;
                drm.pending = false;
                if let Some(h) = drm.head.as_mut() {
                    let _ = h.compositor.reset_state();
                }
                data.state.dirty = true;
                render_drm(data);
            }
        }
    })?;

    // Monitor ligado ou desligado do cabo.
    let udev = UdevBackend::new(&seat)?;
    handle.insert_source(udev, |event, _, data| {
        if let UdevEvent::Changed { .. } = event {
            let dh = data.display_handle.clone();
            if let Backend::Drm(drm) = &mut data.backend {
                // O monitor em uso saiu: escolhe outro.
                let list = drm.connected();
                let still = drm.head.as_ref().is_some_and(|h| list.iter().any(|c| c.handle == h.connector));
                if !still {
                    if let Some(old) = drm.head.take() {
                        data.state.space.unmap_output(&old.output);
                        dh.remove_global::<Glove>(old.global);
                    }
                }
                drm.configure(&mut data.state, &dh);
            }
            render_drm(data);
        }
    })?;

    Ok(backend)
}

/// As Preferências mudaram: monitor, mouse e touchpad.
pub fn config_changed(data: &mut CalloopData) {
    let dh = data.display_handle.clone();
    if let Backend::Drm(drm) = &mut data.backend {
        drm.apply_input_config(&data.state.config);
        drm.configure(&mut data.state, &dh);
    }
}

/// Desenha um quadro se a tela estiver livre (sem quadro esperando o vblank).
pub fn render_drm(data: &mut CalloopData) {
    let CalloopData { state, backend, .. } = data;
    let Backend::Drm(drm) = backend else { return };
    if !drm.active || drm.pending {
        return;
    }
    let Some(head) = drm.head.as_mut() else { return };
    state.dirty = false;
    state.prepare_chrome();
    state.prepare_cursor();
    let elements = state.build_elements(&mut drm.renderer, true);
    match head.compositor.render_frame(&mut drm.renderer, &elements, [0.0, 0.0, 0.0, 1.0], FrameFlags::DEFAULT) {
        Ok(r) => {
            if !r.is_empty {
                match head.compositor.queue_frame(()) {
                    Ok(()) => drm.pending = true,
                    Err(e) => tracing::warn!("entrega do quadro: {e}"),
                }
            }
        }
        Err(e) => tracing::warn!("desenho: {e}"),
    }
    let output = head.output.clone();
    render::after_frame(state, &mut drm.renderer, &output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_de_tela() {
        assert!(is_internal("eDP-1"));
        assert!(!is_internal("HDMI-A-1"));
        assert_eq!(describe("HDMI-A-1"), "Monitor HDMI (HDMI-A-1)");
    }
}
