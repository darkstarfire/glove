//! Sessão própria: o Glove desenha direto na placa de vídeo (DRM/KMS) e lê teclado,
//! mouse e touchpad pelo libinput. É o que roda quando se escolhe "Glove" na tela de
//! login (ou ao iniciar de um console de texto).
//!
//! Vários monitores: o principal (o escolhido nas Preferências, ou o externo, ou o do
//! notebook) tem a barra de menu e o Dock; os outros estendem a mesa ao lado dele,
//! espelham a mesma imagem ou ficam apagados ("um"). Cada tela desenha só quando algo
//! mudou, no ritmo dela (vblank).

use glove_config::tr;
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

/// Um monitor ligado.
struct Head {
    compositor: GbmDrmCompositor,
    connector: connector::Handle,
    crtc: crtc::Handle,
    name: String,
    mode: DrmMode,
    output: Output,
    global: GlobalId,
    /// Um quadro foi entregue e espera o vblank desta tela.
    pending: bool,
    /// Algo mudou desde o último quadro desta tela.
    needs_frame: bool,
}

pub struct Drm {
    pub session: LibSeatSession,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    pub renderer: GlesRenderer,
    /// Monitores ligados; o primeiro é o principal (barra de menu e Dock).
    heads: Vec<Head>,
    /// Os outros monitores mostram a mesma imagem do principal.
    mirror: bool,
    /// A sessão está ativa (não trocou para outro console).
    active: bool,
    /// Teclados, mouses e touchpads (para reaplicar as preferências).
    devices: Vec<libinput::Device>,
    cursor_size: (u32, u32),
    /// Tela apagada (DPMS): o próximo quadro acende de novo.
    blanked: bool,
}

/// Um conector com monitor ligado.
#[derive(Clone)]
struct Connected {
    handle: connector::Handle,
    name: String,
    size_mm: (u32, u32),
    modes: Vec<DrmMode>,
    /// Controladores (CRTC) que esse conector consegue usar.
    crtcs: Vec<crtc::Handle>,
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
        // Controladores (CRTC) que esse conector consegue usar.
        let mut crtcs: Vec<crtc::Handle> = Vec::new();
        for c in info.encoders().iter().filter_map(|e| dev.get_encoder(*e).ok()).flat_map(|e| res.filter_crtcs(e.possible_crtcs())) {
            if !crtcs.contains(&c) {
                crtcs.push(c);
            }
        }
        let mut modes = info.modes().to_vec();
        modes.retain(|m| !m.flags().contains(smithay::reexports::drm::control::ModeFlags::INTERLACE));
        out.push(Connected { handle: h, name: connector_name(&info), size_mm: info.size().unwrap_or((0, 0)), modes, crtcs });
    }
    out
}

/// A maior resolução que todos os monitores têm (para espelhar).
pub fn common_size(lists: &[Vec<(u16, u16)>]) -> Option<(u16, u16)> {
    let first = lists.first()?;
    let mut ok: Vec<(u16, u16)> = first.iter().copied().filter(|s| lists.iter().all(|l| l.contains(s))).collect();
    ok.sort_by_key(|(w, h)| std::cmp::Reverse(*w as u32 * *h as u32));
    ok.first().copied()
}

/// Posição (lógica) dos monitores extras, ao lado do principal (que fica em 0,0).
pub fn place_extra(primary: (i32, i32), extras: &[(i32, i32)], side: &str) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let (mut x, mut y) = match side {
        "esquerda" => (0, 0),
        "abaixo" => (0, primary.1),
        _ => (primary.0, 0),
    };
    for (w, h) in extras {
        match side {
            "esquerda" => {
                x -= w;
                out.push((x, 0));
            }
            "abaixo" => {
                out.push((0, y));
                y += h;
            }
            _ => {
                out.push((x, 0));
                x += w;
            }
        }
    }
    out
}

/// Um monitor a ligar: conector, controlador, resolução, escala e posição.
struct Plan {
    c: Connected,
    crtc: crtc::Handle,
    mode: DrmMode,
    scale: f64,
    pos: (i32, i32),
}

/// O que ligar com esta configuração: o principal primeiro.
fn plan(list: &[Connected], cfg: &glove_config::Monitor, keep: &[(connector::Handle, crtc::Handle)]) -> (Vec<Plan>, bool) {
    let Some(primary) = choose_connector(list, &cfg.output).cloned() else { return (Vec::new(), false) };
    let mut order = vec![primary.clone()];
    if cfg.arrangement != "um" {
        order.extend(list.iter().filter(|c| c.handle != primary.handle).cloned());
    }
    // Controladores diferentes para cada tela (quem já tem um, fica com ele).
    let mut used: Vec<crtc::Handle> = Vec::new();
    let mut chosen: Vec<(Connected, crtc::Handle)> = Vec::new();
    for c in order {
        let kept = keep.iter().find(|(h, _)| *h == c.handle).map(|(_, k)| *k).filter(|k| c.crtcs.contains(k) && !used.contains(k));
        let Some(crtc) = kept.or_else(|| c.crtcs.iter().copied().find(|k| !used.contains(k) && !keep.iter().any(|(h, kk)| kk == k && *h != c.handle))).or_else(|| c.crtcs.iter().copied().find(|k| !used.contains(k))) else {
            tracing::warn!("{}: sem controlador de vídeo livre", c.name);
            continue;
        };
        used.push(crtc);
        chosen.push((c, crtc));
    }
    if chosen.is_empty() {
        return (Vec::new(), false);
    }
    // Espelhar: a mesma resolução (a maior que todos têm) e a mesma escala.
    if cfg.arrangement == "espelhar" && chosen.len() > 1 {
        let sizes: Vec<Vec<(u16, u16)>> = chosen.iter().map(|(c, _)| c.modes.iter().map(|m| m.size()).collect()).collect();
        if let Some((w, h)) = common_size(&sizes) {
            let p = chosen[0].0.clone();
            let pm = choose_mode(&p.modes, &format!("{w}x{h}@0")).unwrap_or(p.modes[0]);
            let scale = if cfg.scale > 0.0 { cfg.scale } else { auto_scale((w as u32, h as u32), p.size_mm) };
            let plans = chosen
                .into_iter()
                .filter_map(|(c, crtc)| {
                    let mode = c.modes.iter().filter(|m| m.size() == (w, h)).max_by_key(|m| m.vrefresh()).copied().or(if c.handle == p.handle { Some(pm) } else { None })?;
                    Some(Plan { c, crtc, mode, scale, pos: (0, 0) })
                })
                .collect();
            return (plans, true);
        }
        tracing::warn!("sem resolução em comum para espelhar: os monitores ficam lado a lado");
    }
    // Estender (ou um só): cada tela com a resolução e a escala dela.
    let mut plans: Vec<Plan> = Vec::new();
    for (i, (c, crtc)) in chosen.into_iter().enumerate() {
        // O principal usa `mode`/`scale`; os outros, o ajuste de cada um (`others`).
        let other = cfg.others.get(&c.name).cloned().unwrap_or_default();
        let (wanted, want_scale) = if i == 0 { (cfg.mode.as_str(), cfg.scale) } else { (other.mode.as_str(), other.scale) };
        let Some(mode) = choose_mode(&c.modes, wanted) else { continue };
        let (pw, ph) = mode.size();
        let scale = if want_scale > 0.0 { want_scale } else { auto_scale((pw as u32, ph as u32), c.size_mm) };
        plans.push(Plan { c, crtc, mode, scale, pos: (0, 0) });
    }
    if let Some(first) = plans.first() {
        let logical = |p: &Plan| ((p.mode.size().0 as f64 / p.scale).round() as i32, (p.mode.size().1 as f64 / p.scale).round() as i32);
        let primary = logical(first);
        let extras: Vec<(i32, i32)> = plans.iter().skip(1).map(logical).collect();
        let pos = place_extra(primary, &extras, &cfg.side);
        for (p, xy) in plans.iter_mut().skip(1).zip(pos) {
            p.pos = xy;
        }
    }
    (plans, false)
}

/// `glove --monitores`: mostra o que a sessão escolheria, sem tocar na tela.
pub fn probe(config: &glove_config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = udev::primary_gpu("seat0")?.or_else(|| udev::all_gpus("seat0").ok()?.into_iter().next()).ok_or("nenhuma placa de vídeo encontrada")?;
    let file = std::fs::File::open(&path)?;
    let fd = DrmDeviceFd::new(DeviceFd::from(std::os::fd::OwnedFd::from(file)));
    println!("{}", tr!("Placa de vídeo: {}", path.display()));
    let list = connected_on(&fd);
    for c in &list {
        let pref = choose_mode(&c.modes, "").map(|m| mode_string(&m)).unwrap_or_default();
        let auto = choose_mode(&c.modes, "").map(|m| auto_scale((m.size().0 as u32, m.size().1 as u32), c.size_mm)).unwrap_or(1.0);
        println!("  {}", tr!("{}  {} mm × {} mm  recomendada {}  escala automática {}  ({} resoluções)", describe(&c.name), c.size_mm.0, c.size_mm.1, pref, auto, c.modes.len()));
    }
    let (plans, mirror) = plan(&list, &config.monitor, &[]);
    if plans.is_empty() {
        println!("{}", tr!("Nenhum monitor conectado."));
    }
    for (i, p) in plans.iter().enumerate() {
        let (w, h) = p.mode.size();
        let papel = if i == 0 { tr!("principal") } else if mirror { tr!("espelho") } else { tr!("extra") };
        println!(
            "{}",
            tr!(
            "A sessão usaria ({}): {} em {} com escala {} (área de {} × {} pontos, em {}, {})",
            papel,
            p.c.name,
            mode_string(&p.mode),
            p.scale,
            (w as f64 / p.scale).round(),
            (h as f64 / p.scale).round(),
            p.pos.0,
            p.pos.1
            )
        );
    }
    Ok(())
}

impl Drm {
    fn connected(&self) -> Vec<Connected> {
        connected_on(&self.drm)
    }

    /// Liga, ajusta ou desliga os monitores conforme a configuração. Os que já
    /// estão ligados só mudam o que for preciso (resolução, escala, posição).
    fn configure(&mut self, state: &mut Glove, dh: &DisplayHandle) {
        let mut list = self.connected();
        write_outputs_info(&list, &self.heads);
        // Tampa fechada com monitor externo: a tela embutida fica de fora.
        if state.lid_closed && list.iter().any(|c| !is_internal(&c.name)) {
            list.retain(|c| !is_internal(&c.name));
        }
        let keep: Vec<(connector::Handle, crtc::Handle)> = self.heads.iter().map(|h| (h.connector, h.crtc)).collect();
        let (plans, mirror) = plan(&list, &state.config.monitor, &keep);
        if plans.is_empty() {
            tracing::warn!("nenhum monitor conectado");
            return;
        }
        self.mirror = mirror;

        // Desliga os que saíram do plano (ou trocaram de controlador).
        let mut i = 0;
        while i < self.heads.len() {
            let h = &self.heads[i];
            if plans.iter().any(|p| p.c.handle == h.connector && p.crtc == h.crtc) {
                i += 1;
                continue;
            }
            let mut old = self.heads.remove(i);
            let _ = old.compositor.clear();
            state.space.unmap_output(&old.output);
            dh.remove_global::<Glove>(old.global);
            tracing::info!("monitor {} desligado", old.name);
        }

        let mut ordered: Vec<Head> = Vec::new();
        for p in plans {
            let existing = self.heads.iter().position(|h| h.connector == p.c.handle).map(|i| self.heads.remove(i));
            let head = match existing {
                Some(mut head) => {
                    if head.mode != p.mode {
                        match head.compositor.use_mode(p.mode) {
                            Ok(()) => {
                                head.mode = p.mode;
                                head.output.change_current_state(Some(OutputMode::from(p.mode)), None, None, None);
                                tracing::info!("{}: resolução {}", head.name, mode_string(&p.mode));
                            }
                            Err(e) => tracing::warn!("{}: não consegui usar {}: {e}", head.name, mode_string(&p.mode)),
                        }
                    }
                    if head.output.current_scale().fractional_scale() != p.scale {
                        head.output.change_current_state(None, None, Some(Scale::Fractional(p.scale)), None);
                    }
                    if head.output.current_location() != p.pos.into() {
                        head.output.change_current_state(None, None, None, Some(p.pos.into()));
                    }
                    state.space.map_output(&head.output, p.pos);
                    head.needs_frame = true;
                    head
                }
                None => match self.start_head(&p, state, dh) {
                    Some(h) => h,
                    None => continue,
                },
            };
            ordered.push(head);
        }
        self.heads = ordered;
        state.primary_output = self.heads.first().map(|h| h.output.name());
        state.rescue_windows();
        state.update_client_scales();
        state.keep_pointer_inside();
        state.dirty = true;
        write_outputs_info(&list, &self.heads);
    }

    /// Liga um monitor novo.
    fn start_head(&mut self, p: &Plan, state: &mut Glove, dh: &DisplayHandle) -> Option<Head> {
        let c = &p.c;
        let surface = match self.drm.create_surface(p.crtc, p.mode, &[c.handle]) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("{}: {e}", c.name);
                return None;
            }
        };
        let output = Output::new(
            c.name.clone(),
            PhysicalProperties { size: (c.size_mm.0 as i32, c.size_mm.1 as i32).into(), subpixel: Subpixel::Unknown, make: "Glove".into(), model: c.name.clone() },
        );
        let global = output.create_global::<Glove>(dh);
        let omode = OutputMode::from(p.mode);
        output.change_current_state(Some(omode), Some(Transform::Normal), Some(Scale::Fractional(p.scale)), Some(p.pos.into()));
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
                return None;
            }
        };
        state.space.map_output(&output, p.pos);
        tracing::info!("monitor {} em {} (escala {}, posição {},{})", c.name, mode_string(&p.mode), p.scale, p.pos.0, p.pos.1);
        Some(Head { compositor, connector: c.handle, crtc: p.crtc, name: c.name.clone(), mode: p.mode, output, global, pending: false, needs_frame: true })
    }

    pub fn output(&self) -> Option<&Output> {
        self.heads.first().map(|h| &h.output)
    }

    /// Reaplica velocidade, toque para clicar etc. (as Preferências mudaram).
    pub fn apply_input_config(&mut self, config: &glove_config::Config) {
        for d in &mut self.devices {
            configure_device(d, config);
        }
    }
}

/// Avisa as Preferências sobre os monitores e resoluções disponíveis.
fn write_outputs_info(list: &[Connected], heads: &[Head]) {
    let outputs = list
        .iter()
        .map(|c| {
            let head = heads.iter().find(|h| h.connector == c.handle);
            let active = head.is_some();
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
                primary: heads.first().is_some_and(|h| h.connector == c.handle),
                current: head.map(|h| mode_string(&h.mode)).unwrap_or_default(),
                preferred,
                modes,
                scale: head.map(|h| h.output.current_scale().fractional_scale()).unwrap_or(auto),
                auto_scale: auto,
            }
        })
        .collect();
    OutputsInfo { nested: false, outputs }.save();
}

fn describe(name: &str) -> String {
    let kind = if is_internal(name) {
        tr!("Tela embutida")
    } else if name.starts_with("HDMI") {
        tr!("Monitor HDMI")
    } else if name.starts_with("DP") {
        tr!("Monitor DisplayPort")
    } else if name.starts_with("DVI") {
        tr!("Monitor DVI")
    } else if name.starts_with("VGA") {
        tr!("Monitor VGA")
    } else {
        tr!("Monitor|tela")
    };
    format!("{kind} ({name})")
}

/// Preferências de mouse e touchpad num aparelho do libinput.
fn configure_device(d: &mut libinput::Device, config: &glove_config::Config) {
    let touchpad = d.config_tap_finger_count() > 0;
    if touchpad {
        let t = &config.touchpad;
        let _ = d.config_tap_set_enabled(t.tap_to_click);
        let _ = d.config_tap_set_drag_enabled(t.tap_drag);
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
    crate::dmabuf::init(state, dh, &renderer, Some(&path));

    let cursor_size = {
        use smithay::reexports::drm::{Device as _, DriverCapability};
        let w = drm.get_driver_capability(DriverCapability::CursorWidth).unwrap_or(64) as u32;
        let h = drm.get_driver_capability(DriverCapability::CursorHeight).unwrap_or(64) as u32;
        (w.clamp(32, 512), h.clamp(32, 512))
    };

    let mut backend = Box::new(Drm { session: session.clone(), drm, gbm, renderer, heads: Vec::new(), mirror: false, active: true, devices: Vec::new(), cursor_size, blanked: false });
    backend.configure(state, dh);
    if backend.heads.is_empty() {
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
            DrmEvent::VBlank(crtc) => {
                if let Some(head) = drm.heads.iter_mut().find(|h| h.crtc == crtc) {
                    if let Err(e) = head.compositor.frame_submitted() {
                        tracing::warn!("vblank: {e}");
                    }
                    head.pending = false;
                    crate::prof::vblank(&head.name);
                }
                if data.state.dirty || drm.heads.iter().any(|h| h.needs_frame) {
                    render_drm(data);
                }
            }
            DrmEvent::Error(e) => tracing::warn!("erro do DRM: {e}"),
        }
    })?;

    // Rede de segurança: se um vblank se perder (suspensão, driver), a tela não congela.
    handle.insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, data| {
        if let Backend::Drm(drm) = &mut data.backend {
            if drm.active {
                // Só libera a tela se o quadro esperar de fato há 1 s (o flip normal leva ~200 ms).
                for h in drm.heads.iter_mut().filter(|h| h.pending && crate::prof::esperando(&h.name).is_none_or(|d| d >= std::time::Duration::from_secs(1))) {
                    if crate::prof::on() {
                        let espera = crate::prof::esperando(&h.name);
                        tracing::warn!("{}: sem vblank; quadro esperando há {:?}; liberando a tela à força", h.name, espera);
                    }
                    h.pending = false;
                    let _ = h.compositor.reset_state();
                    data.state.dirty = true;
                }
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
        // A tampa abriu ou fechou com monitor externo: reorganiza as telas.
        if std::mem::take(&mut data.state.reconfigure_outputs) {
            let dh = data.display_handle.clone();
            if let Backend::Drm(drm) = &mut data.backend {
                drm.configure(&mut data.state, &dh);
            }
        }
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
                for h in &mut drm.heads {
                    h.pending = false;
                    h.needs_frame = true;
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
                // Monitores que saíram do cabo: tira da mesa (as janelas voltam ao principal).
                let list = drm.connected();
                let mut i = 0;
                while i < drm.heads.len() {
                    if list.iter().any(|c| c.handle == drm.heads[i].connector) {
                        i += 1;
                        continue;
                    }
                    let old = drm.heads.remove(i);
                    data.state.space.unmap_output(&old.output);
                    dh.remove_global::<Glove>(old.global);
                    tracing::info!("monitor {} saiu", old.name);
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

/// Desenha um quadro em cada tela livre (sem quadro esperando o vblank) que precise.
pub fn render_drm(data: &mut CalloopData) {
    let CalloopData { state, backend, .. } = data;
    let Backend::Drm(drm) = backend else { return };
    if !drm.active || drm.heads.is_empty() {
        return;
    }
    // Parado: desliga as telas (DPMS). O próximo quadro de verdade as liga de novo.
    if state.idle.screen_off {
        if !drm.blanked {
            for h in &mut drm.heads {
                if let Err(e) = h.compositor.clear() {
                    tracing::warn!("apagar a tela {}: {e}", h.name);
                }
                h.pending = false;
                h.needs_frame = false;
            }
            drm.blanked = true;
        }
        state.dirty = false;
        return;
    }
    if state.dirty || drm.blanked {
        drm.blanked = false;
        state.dirty = false;
        let t0 = std::time::Instant::now();
        state.prepare_chrome();
        state.prepare_cursor();
        if crate::prof::on() {
            crate::prof::prep(t0.elapsed());
        }
        for h in &mut drm.heads {
            h.needs_frame = true;
        }
    }
    let mirror = drm.mirror;
    let Drm { heads, renderer, .. } = &mut **drm;
    for (i, head) in heads.iter_mut().enumerate() {
        if head.pending || !head.needs_frame {
            continue;
        }
        head.needs_frame = false;
        // O principal (e os espelhos) mostram a mesa inteira; os extras, a parte deles.
        let t1 = std::time::Instant::now();
        let elements = if i == 0 || mirror {
            state.build_elements(renderer, true)
        } else {
            let geo = state.space.output_geometry(&head.output).unwrap_or_default();
            state.build_extra_elements(renderer, geo, head.output.current_scale().fractional_scale())
        };
        let t2 = std::time::Instant::now();
        let rendered = head.compositor.render_frame(renderer, &elements, [0.0, 0.0, 0.0, 1.0], FrameFlags::DEFAULT);
        if crate::prof::on() {
            crate::prof::build(t2 - t1);
            crate::prof::render(t2.elapsed());
        }
        match rendered {
            Ok(r) => {
                if !r.is_empty {
                    match head.compositor.queue_frame(()) {
                        Ok(()) => {
                            head.pending = true;
                            crate::prof::queued(&head.name, elements.len());
                        }
                        Err(e) => tracing::warn!("{}: entrega do quadro: {e}", head.name),
                    }
                }
            }
            Err(e) => tracing::warn!("{}: desenho: {e}", head.name),
        }
        if i == 0 {
            let output = head.output.clone();
            render::after_frame(state, renderer, &output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_de_tela() {
        assert!(is_internal("eDP-1"));
        assert!(!is_internal("HDMI-A-1"));
        assert_eq!(describe("HDMI-A-1"), format!("{} (HDMI-A-1)", tr!("Monitor HDMI")));
    }

    #[test]
    fn resolucao_em_comum() {
        let a = vec![(1920, 1080), (1280, 720), (1024, 768)];
        let b = vec![(1024, 768), (1280, 720), (800, 600)];
        assert_eq!(common_size(&[a.clone(), b]), Some((1280, 720)));
        assert_eq!(common_size(&[a, vec![(640, 480)]]), None);
    }

    #[test]
    fn posicao_dos_extras() {
        assert_eq!(place_extra((1280, 800), &[(1920, 1080)], "direita"), vec![(1280, 0)]);
        assert_eq!(place_extra((1280, 800), &[(1920, 1080), (800, 600)], "esquerda"), vec![(-1920, 0), (-2720, 0)]);
        assert_eq!(place_extra((1280, 800), &[(1920, 1080)], "abaixo"), vec![(0, 800)]);
    }
}
