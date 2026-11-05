//! Janela Wayland do kit: escala fracionada (nítido em 1.5×), mouse, teclado,
//! e redesenho só quando há evento (zero CPU parado).

use std::os::fd::AsRawFd;
use std::time::Duration;

use glove_paint::{draw::Canvas, Fonts, Theme, Variant};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    data_device_manager::{
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{CopyPasteSource, DataSourceHandler},
        DataDeviceManagerState, WritePipe,
    },
    delegate_registry,
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            generic::Generic,
            timer::{TimeoutAction, Timer},
            EventLoop, Interest, LoopHandle, Mode, PostAction,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::{Buffer, SlotPool}, Shm, ShmHandler},
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, wp_fractional_scale_v1::{self, WpFractionalScaleV1}},
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use crate::ui::{Input, Key, KeyPress, Memory, Ui};

/// Um app do Glove: descreve a interface a cada quadro.
pub trait App: 'static {
    fn ui(&mut self, ui: &mut Ui);
    /// Chamado quando a configuração do Glove muda (tema etc.).
    fn config_changed(&mut self) {}
    /// Se devolver um intervalo, a janela é redesenhada nesse ritmo (ex.: relógio).
    /// Sem isso, nada acontece enquanto não houver evento.
    fn tick(&self) -> Option<Duration> {
        None
    }
    /// Chamado a cada batida do timer de `tick`, antes de redesenhar.
    fn on_tick(&mut self) {}
    /// O usuário pediu para fechar a janela. Devolva `false` para impedir
    /// (ex.: perguntar se quer salvar) e feche depois com `ui.close = true`.
    fn close_requested(&mut self) -> bool {
        true
    }
    /// O botão de fechar da janela `id` (apps com várias janelas). Devolva
    /// verdadeiro para fechar. Padrão: o mesmo que `close_requested`.
    fn close_window(&mut self, id: u32) -> bool {
        let _ = id;
        self.close_requested()
    }
    /// A janela `id` foi fechada (pelo botão ou por `ui.close`).
    fn window_closed(&mut self, id: u32) {
        let _ = id;
    }
    /// Descritor que o kit deve observar (ex.: saída do shell no terminal).
    fn watch_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }
    /// O descritor de `watch_fd` tem dados. Devolva `false` para parar de observar.
    fn fd_ready(&mut self) -> bool {
        false
    }
    /// Menus do app para a barra de menu global (enviados só quando mudam).
    fn menus(&self) -> Vec<crate::menu::Menu> {
        Vec::new()
    }
    /// O app pinta o fundo inteiro sozinho (ex.: a partir de um cache) e o kit
    /// pode pular o fundo listrado padrão.
    fn paints_background(&self) -> bool {
        false
    }
}

/// Hora local: (ano, mês 1-12, dia, dia da semana 0=domingo, hora, minuto, segundo).
pub fn local_time() -> (i32, u32, u32, u32, u32, u32, u32) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_year + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32, tm.tm_wday as u32, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32)
    }
}

#[derive(Clone, Debug)]
pub struct WindowOptions {
    pub title: String,
    pub app_id: String,
    pub size: (u32, u32),
    /// Tamanho fixo (sem zoom nem redimensionar).
    pub fixed: bool,
}

/// Ícone do app pelo app_id (a mesma lista do Dock).
pub fn app_icon(app_id: &str) -> glove_paint::icons::Icon {
    glove_config::default_dock_apps()
        .into_iter()
        .find(|a| a.app_id == app_id)
        .map(|a| glove_paint::icons::Icon::from_name(&a.icon))
        .unwrap_or(glove_paint::icons::Icon::Window)
}

pub fn theme_from_config() -> Theme {
    let c = glove_config::Config::load();
    let mut t = Theme::new(Variant::from_config(&c.theme));
    let (r, g, b) = glove_config::highlight_rgb(&c.appearance.highlight);
    t.highlight = glove_paint::draw::rgba(r, g, b, 255);
    // Ajustes que o kit lê a cada clique (clique duplo, trilho da rolagem).
    DOUBLE_CLICK_MS.store(c.pointer.double_click_ms.clamp(150, 1500), std::sync::atomic::Ordering::Relaxed);
    SCROLL_JUMP.store(c.appearance.scroll_jump, std::sync::atomic::Ordering::Relaxed);
    t
}

/// Tempo do clique duplo (Preferências ▸ Mouse).
pub(crate) static DOUBLE_CLICK_MS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(400);
/// Clicar no trilho da barra de rolagem vai até ali (Preferências ▸ Aparência).
pub(crate) static SCROLL_JUMP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Estado de uma janela. Um app pode ter várias (ex.: Lembretes); a que está
/// recebendo o evento ou sendo desenhada fica em `Runner::w` e as outras em
/// `Runner::others` (trocadas de lugar quando o evento é de outra).
struct Win {
    /// Número dado pelo app (0 = a primeira janela).
    id: u32,
    window: Window,
    viewport: Option<WpViewport>,
    _frac: Option<WpFractionalScaleV1>,
    /// Escala fracionada vinda do compositor (ou inteira, sem o protocolo).
    scale: f64,
    int_scale: i32,
    width: u32,
    height: u32,
    fixed: bool,
    configured: bool,
    dirty: bool,
    /// Só uma animação pediu o quadro (pode ser redesenho parcial).
    anim_dirty: bool,
    /// Um configure foi confirmado: o compositor só aplica o novo estado
    /// (bordas, tamanho) no próximo commit, mesmo que nenhum pixel mude.
    need_commit: bool,
    frame_pending: bool,
    input: Input,
    mem: Memory,
    /// Buffers reaproveitados entre quadros, cada um com o número do quadro que
    /// guarda (só cria outro se o compositor ainda estiver usando todos).
    buffers: Vec<(Buffer, u64)>,
    /// Número do último quadro enviado e as áreas que mudaram nos últimos quadros
    /// (um buffer antigo recebe só o que mudou desde o quadro que ele guarda).
    frame_no: u64,
    history: std::collections::VecDeque<(u64, Vec<[i32; 4]>)>,
    /// Cópia do último quadro enviado, para achar o que mudou.
    prev: Vec<u8>,
    prev_key: (u32, u32, u32),
    /// Canvas reaproveitado enquanto o tamanho/escala não mudam.
    canvas: Option<Canvas>,
    /// Já há um timer de "acordar" pendente (ui.wake_after).
    wake_pending: bool,
    /// Mudança de tamanho pedida pelo app, animada: (de, para, início, duração).
    size_anim: Option<((u32, u32), (u32, u32), std::time::Instant, Duration)>,
    /// Algum quadro já foi para a tela (antes disso o tamanho muda sem animar).
    shown: bool,
    // Clique duplo/triplo
    last_click: (u32, (f32, f32), u8),
}

/// Janelas sem a barra de título do Glove: o app desenha a própria moldura
/// (ex.: Lembretes) e usa `ui.start_move()` para arrastar.
static OWN_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Chame antes de `run` para o app desenhar a própria moldura.
pub fn use_own_frame() {
    OWN_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// O que é preciso para abrir janelas novas depois da primeira.
struct Shells {
    compositor: CompositorState,
    xdg: XdgShell,
    frac_mgr: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
}

impl Shells {
    fn open<A: App>(&self, qh: &QueueHandle<Runner<A>>, id: u32, opts: &WindowOptions) -> Win {
        let surface = self.compositor.create_surface(qh);
        let (viewport, frac) = match (&self.frac_mgr, &self.viewporter) {
            (Some(m), Some(v)) => (Some(v.get_viewport(&surface, qh, NoEvents)), Some(m.get_fractional_scale(&surface, qh, FracData))),
            _ => (None, None),
        };
        let deco = if OWN_FRAME.load(std::sync::atomic::Ordering::Relaxed) { WindowDecorations::RequestClient } else { WindowDecorations::RequestServer };
        let window = self.xdg.create_window(surface, deco, qh);
        window.set_title(opts.title.clone());
        window.set_app_id(opts.app_id.clone());
        if opts.fixed {
            window.set_min_size(Some(opts.size));
            window.set_max_size(Some(opts.size));
        } else {
            window.set_min_size(Some((opts.size.0 / 2, opts.size.1 / 2)));
        }
        window.commit();
        let mut mem = Memory::with_app_icon(app_icon(&opts.app_id));
        mem.window_id = id;
        Win {
            id,
            window,
            viewport,
            _frac: frac,
            scale: 1.0,
            int_scale: 1,
            width: opts.size.0,
            height: opts.size.1,
            fixed: opts.fixed,
            configured: false,
            dirty: true,
            anim_dirty: false,
            need_commit: false,
            frame_pending: false,
            input: Input::default(),
            mem,
            buffers: Vec::new(),
            frame_no: 0,
            history: std::collections::VecDeque::new(),
            prev: Vec::new(),
            prev_key: (0, 0, 0),
            canvas: None,
            wake_pending: false,
            size_anim: None,
            shown: false,
            last_click: (0, (0.0, 0.0), 0),
        }
    }
}

struct Runner<A: App> {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    /// A janela atual e as outras do app.
    w: Win,
    others: Vec<Win>,
    shells: Shells,
    /// Superfície com o foco do teclado.
    kbd_surface: Option<wl_surface::WlSurface>,
    exit: bool,
    fonts: Fonts,
    doc_fonts: Fonts,
    theme: Theme,
    app: A,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    loop_handle: LoopHandle<'static, Runner<A>>,
    qh: QueueHandle<Runner<A>>,
    // Área de transferência
    data_manager: Option<DataDeviceManagerState>,
    data_device: Option<DataDevice>,
    clip_source: Option<CopyPasteSource>,
    clip_text: String,
    /// Último "serial" de clique/tecla (o compositor exige para copiar).
    last_serial: u32,
    seat: Option<wl_seat::WlSeat>,
    // Menus na barra global
    menu_stream: Option<std::os::unix::net::UnixStream>,
    menu_sent: String,
    menu_shortcuts: std::collections::HashMap<String, String>,
}

const TEXT_MIMES: [&str; 5] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"];

/// Dados para objetos que não recebem eventos.
pub struct NoEvents;
/// Recebe a escala preferida do compositor.
pub struct FracData;

impl<A: App> Dispatch2<WpFractionalScaleManagerV1, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpFractionalScaleManagerV1, _: <WpFractionalScaleManagerV1 as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpViewporter, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpViewporter, _: <WpViewporter as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpViewport, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpViewport, _: <WpViewport as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpFractionalScaleV1, Runner<A>> for FracData {
    fn event(&self, st: &mut Runner<A>, proxy: &WpFractionalScaleV1, ev: wp_fractional_scale_v1::Event, _: &Connection, qh: &QueueHandle<Runner<A>>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = ev {
            if st.w._frac.as_ref() != Some(proxy) {
                match st.others.iter().position(|o| o._frac.as_ref() == Some(proxy)) {
                    Some(i) => std::mem::swap(&mut st.w, &mut st.others[i]),
                    None => return,
                }
            }
            let s = scale as f64 / 120.0;
            if (s - st.w.scale).abs() > 1e-6 {
                st.w.scale = s;
                st.request_redraw(qh);
            }
        }
    }
}

/// Abre a janela e roda até ela ser fechada.
pub fn run<A: App>(opts: WindowOptions, app: A) -> Result<(), Box<dyn std::error::Error>> {
    glove_config::tune_memory();
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<Runner<A>> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), queue).insert(event_loop.handle()).map_err(|e| e.to_string())?;

    let compositor = CompositorState::bind(&globals, &qh)?;
    let xdg = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let frac_mgr: Option<WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, NoEvents).ok();
    let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, NoEvents).ok();

    let shells = Shells { compositor, xdg, frac_mgr, viewporter };
    let first = shells.open(&qh, 0, &opts);

    // Recarrega o tema quando a configuração muda.
    if let Some(fd) = glove_config::watch_fd() {
        let raw = fd.as_raw_fd();
        let _ = raw;
        event_loop
            .handle()
            .insert_source(Generic::new(fd, Interest::READ, Mode::Level), |_, fd, st: &mut Runner<A>| {
                if glove_config::drain_watch(fd) {
                    st.theme = theme_from_config();
                    st.app.config_changed();
                    st.redraw_all();
                }
                Ok(PostAction::Continue)
            })
            .map_err(|e| e.to_string())?;
    }

    // Timer só para apps que pedem (ex.: relógio); acorda alinhado ao próximo segundo.
    if let Some(first) = app.tick() {
        event_loop
            .handle()
            .insert_source(Timer::from_duration(first), |_, _, st: &mut Runner<A>| {
                st.app.on_tick();
                st.redraw_all();
                match st.app.tick() {
                    Some(d) => TimeoutAction::ToDuration(d),
                    None => TimeoutAction::Drop,
                }
            })
            .map_err(|e| e.to_string())?;
    }

    let data_manager = DataDeviceManagerState::bind(&globals, &qh).ok();
    let pool = SlotPool::new((opts.size.0 * opts.size.1 * 4 * 4) as usize, &shm)?;
    let mut runner = Runner {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        w: first,
        others: Vec::new(),
        shells,
        kbd_surface: None,
        exit: false,
        fonts: Fonts::load_system().ok_or("nenhuma fonte encontrada")?,
        doc_fonts: Fonts::load_document().ok_or("nenhuma fonte encontrada")?,
        theme: theme_from_config(),
        app,
        keyboard: None,
        pointer: None,
        loop_handle: event_loop.handle(),
        qh: qh.clone(),
        data_manager,
        data_device: None,
        clip_source: None,
        clip_text: String::new(),
        last_serial: 0,
        seat: None,
        menu_stream: None,
        menu_sent: String::new(),
        menu_shortcuts: std::collections::HashMap::new(),
    };
    runner.connect_menus();
    if let Some(fd) = runner.app.watch_fd() {
        // Segurança: o app garante que o descritor vive enquanto a janela existir.
        let src = Generic::new(unsafe { smithay_client_toolkit::reexports::calloop::generic::FdWrapper::new(fd) }, Interest::READ, Mode::Level);
        event_loop
            .handle()
            .insert_source(src, |_, _, st: &mut Runner<A>| {
                let keep = st.app.fd_ready();
                st.redraw_all();
                Ok(if keep { PostAction::Continue } else { PostAction::Remove })
            })
            .map_err(|e| e.to_string())?;
    }

    while !runner.exit {
        event_loop.dispatch(None::<Duration>, &mut runner)?;
    }
    Ok(())
}

/// União dos retângulos lógicos em pixels físicos, com folga para a
/// suavização das bordas.
fn px_union(rects: &[crate::ui::Rect], scale: f32) -> [i32; 4] {
    let mut u = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for r in rects {
        u[0] = u[0].min(((r.x - 2.0) * scale).floor() as i32);
        u[1] = u[1].min(((r.y - 2.0) * scale).floor() as i32);
        u[2] = u[2].max(((r.x + r.w + 2.0) * scale).ceil() as i32);
        u[3] = u[3].max(((r.y + r.h + 2.0) * scale).ceil() as i32);
    }
    u
}

/// Compara o quadro novo (`cur`, RGBA w×h) com o anterior (`prev`) dentro de
/// `region` e devolve as áreas que mudaram, já copiadas para `prev`. Linhas
/// vizinhas que mudaram viram um só retângulo (até 8 linhas de folga).
pub(crate) fn frame_damage(prev: &mut Vec<u8>, cur: &[u8], w: i32, h: i32, region: [i32; 4]) -> Vec<[i32; 4]> {
    if prev.len() != cur.len() {
        *prev = cur.to_vec();
        return vec![[0, 0, w, h]];
    }
    let [rx0, ry0, rx1, ry1] = [region[0].clamp(0, w), region[1].clamp(0, h), region[2].clamp(0, w), region[3].clamp(0, h)];
    if rx0 >= rx1 || ry0 >= ry1 {
        return Vec::new();
    }
    let row = |y: i32| ((y * w + rx0) * 4) as usize..((y * w + rx1) * 4) as usize;
    let mut out = Vec::new();
    let mut band: Option<[i32; 4]> = None;
    let mut gap = 0;
    for y in ry0..ry1 {
        let (a, b) = (&cur[row(y)], &prev[row(y)]);
        if a == b {
            if band.is_some() {
                gap += 1;
                if gap > 8 {
                    out.extend(band.take());
                }
            }
            continue;
        }
        gap = 0;
        let n = (rx1 - rx0) as usize;
        let first = a.chunks_exact(4).zip(b.chunks_exact(4)).position(|(p, q)| p != q).unwrap_or(0) as i32;
        let last = (n - a.chunks_exact(4).rev().zip(b.chunks_exact(4).rev()).position(|(p, q)| p != q).unwrap_or(0)) as i32;
        let (x0, x1) = (rx0 + first, rx0 + last);
        band = Some(match band {
            Some([bx0, by0, bx1, _]) => [bx0.min(x0), by0, bx1.max(x1), y + 1],
            None => [x0, y, x1, y + 1],
        });
    }
    out.extend(band);
    if out.len() > 16 {
        let u = out.iter().fold([w, h, 0, 0], |u, r| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]);
        out = vec![u];
    }
    for &[x0, y0, x1, y1] in &out {
        for y in y0..y1 {
            let r = ((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize;
            prev[r.clone()].copy_from_slice(&cur[r]);
        }
    }
    out
}

impl<A: App> Runner<A> {
    /// Traz para `self.w` a janela desta superfície. Devolve se achou.
    fn focus_surface(&mut self, s: &wl_surface::WlSurface) -> bool {
        if self.w.window.wl_surface() == s {
            return true;
        }
        match self.others.iter().position(|o| o.window.wl_surface() == s) {
            Some(i) => {
                std::mem::swap(&mut self.w, &mut self.others[i]);
                true
            }
            None => false,
        }
    }

    /// Traz para `self.w` a janela de número `id`. Devolve se achou.
    fn focus_id(&mut self, id: u32) -> bool {
        if self.w.id == id {
            return true;
        }
        match self.others.iter().position(|o| o.id == id) {
            Some(i) => {
                std::mem::swap(&mut self.w, &mut self.others[i]);
                true
            }
            None => false,
        }
    }

    /// Traz para `self.w` a janela com o foco do teclado (se houver).
    fn focus_keyboard(&mut self) {
        if let Some(s) = self.kbd_surface.clone() {
            self.focus_surface(&s);
        }
    }

    /// Redesenha todas as janelas (tema, relógio, saída do terminal…).
    fn redraw_all(&mut self) {
        let ids: Vec<u32> = std::iter::once(self.w.id).chain(self.others.iter().map(|o| o.id)).collect();
        for id in ids {
            if self.focus_id(id) {
                self.w.dirty = true;
                self.draw_now();
            }
        }
    }

    /// Fecha a janela atual; sem nenhuma, o app termina.
    fn close_current(&mut self) {
        let id = self.w.id;
        match self.others.pop() {
            Some(next) => {
                // Soltar a janela fecha o toplevel no compositor.
                drop(std::mem::replace(&mut self.w, next));
                self.app.window_closed(id);
            }
            None => {
                self.app.window_closed(id);
                self.exit = true;
            }
        }
    }

    fn request_redraw(&mut self, _qh: &QueueHandle<Self>) {
        self.w.dirty = true;
        if !self.w.frame_pending {
            self.draw_now();
        }
    }

    fn draw_now(&mut self) {
        // Espera o compositor mostrar o quadro anterior: nunca desenha mais
        // rápido que a tela (importante quando chega muita saída de uma vez).
        if !self.w.configured || !(self.w.dirty || self.w.anim_dirty) || self.w.frame_pending {
            return;
        }
        self.step_size_anim();
        let t0 = std::time::Instant::now();
        // Redesenho parcial: se só as animações pediram este quadro (nenhum
        // evento, nada do app), refaz só a área delas; o resto do canvas já
        // está certo desde o quadro anterior.
        let anim_rects = std::mem::take(&mut self.w.mem.anim_rects);
        let mut partial = !self.w.dirty
            && self.w.anim_dirty
            && !self.w.mem.anim_full
            && !anim_rects.is_empty()
            && self.w.size_anim.is_none()
            && !self.w.input.has_events()
            && !self.w.input.moved;
        self.w.anim_dirty = false;
        // Até 3 passadas seguidas se a interface pedir (ex.: pop-up que acabou de abrir).
        for pass in 0..3 {
            self.w.dirty = false;
            self.w.mem.anim_rects.clear();
            self.w.mem.anim_full = false;
            let scale = if self.w.viewport.is_some() { self.w.scale } else { self.w.int_scale as f64 } as f32;
            let reuse = self.w.canvas.take().filter(|c| c.w == self.w.width as f32 && c.h == self.w.height as f32 && c.scale == scale);
            partial &= reuse.is_some();
            let Some(mut canvas) = reuse.or_else(|| Canvas::new(self.w.width as f32, self.w.height as f32, scale)) else { return };
            let region = partial.then(|| px_union(&anim_rects, scale));
            canvas.set_clip(region);
            if !self.app.paints_background() {
                canvas.clear(self.theme.body_a);
                canvas.pinstripes(0.0, 0.0, self.w.width as f32, self.w.height as f32, self.theme.body_a, self.theme.body_b);
            }
            let (redraw, close) = {
                let mut ui = Ui::new(&mut canvas, &self.theme, &self.fonts, &self.doc_fonts, !self.w.fixed, &self.w.input, &mut self.w.mem);
                self.app.ui(&mut ui);
                ui.finish()
            };
            // Um clique muda o estado no meio do quadro (ex.: o visor já foi
            // desenhado antes do botão). Mais uma passada, sem eventos, mostra
            // o resultado na hora. Só a última passada vai para a tela.
            canvas.set_clip(None);
            let resized = self.after_frame();
            let had_events = self.w.input.has_events();
            self.w.input.end_frame();
            if close {
                // Fecha só esta janela (o app termina quando não sobra nenhuma).
                self.close_current();
                return;
            }
            if (redraw || had_events || resized) && pass < 2 {
                // A próxima passada pode mudar qualquer coisa: refaz tudo.
                partial = false;
                self.w.canvas = Some(canvas);
                self.w.dirty = true;
                continue;
            }
            let t1 = std::time::Instant::now();
            let key = (canvas.px_width(), canvas.px_height(), canvas.scale.to_bits());
            if key != self.w.prev_key {
                self.w.prev.clear();
                self.w.prev_key = key;
            }
            let (pw, ph) = (canvas.px_width() as i32, canvas.px_height() as i32);
            let damage = frame_damage(&mut self.w.prev, canvas.data(), pw, ph, region.unwrap_or([0, 0, pw, ph]));
            self.present(&canvas, &damage);
            if std::mem::take(&mut self.w.need_commit) && damage.is_empty() {
                self.w.window.commit();
            }
            if std::env::var_os("GLOVE_PROF").is_some() {
                let px: i32 = damage.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
                eprintln!("PROF desenho={:?} envio={:?} passadas={} parcial={} mudou={}px em {} áreas", t1 - t0, t1.elapsed(), pass + 1, region.is_some(), px, damage.len());
            }
            self.w.canvas = Some(canvas);
            break;
        }
    }

    /// Conecta ao Glove para mostrar os menus na barra global (se houver).
    fn connect_menus(&mut self) {
        use std::io::Write;
        let Some(path) = std::env::var_os("GLOVE_MENU_SOCKET").filter(|p| !p.is_empty()) else { return };
        let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) else { return };
        if stream.write_all(format!("HELLO\t{}\n", std::process::id()).as_bytes()).is_err() {
            return;
        }
        let Ok(reader) = stream.try_clone() else { return };
        let _ = reader.set_nonblocking(true);
        let mut buf = Vec::new();
        let _ = self.loop_handle.insert_source(Generic::new(reader, Interest::READ, Mode::Level), move |_, s, st: &mut Runner<A>| {
            use std::io::Read;
            let mut tmp = [0u8; 1024];
            loop {
                // Segurança: o socket pertence a esta fonte.
                match unsafe { s.get_mut() }.read(&mut tmp) {
                    Ok(0) => return Ok(PostAction::Remove),
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Ok(PostAction::Remove),
                }
            }
            while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                if let Some(id) = line.strip_prefix("ACT\t") {
                    st.menu_activated(id);
                }
            }
            Ok(PostAction::Continue)
        });
        self.menu_stream = Some(stream);
    }

    /// Item escolhido na barra de menu: com atalho, vira o atalho; sem, vai para o app.
    fn menu_activated(&mut self, id: &str) {
        self.focus_keyboard();
        match self.menu_shortcuts.get(id).filter(|s| !s.is_empty()).cloned() {
            Some(combo) => self.w.input.keys.push(KeyPress { key: Key::Other, combo }),
            None => self.w.input.menu_actions.push(id.to_string()),
        }
        self.w.dirty = true;
        self.draw_now();
    }

    fn send_menus(&mut self) {
        use std::io::Write;
        if self.menu_stream.is_none() {
            return;
        }
        let menus = self.app.menus();
        let text = crate::menu::serialize(&menus);
        if text == self.menu_sent {
            return;
        }
        self.menu_shortcuts.clear();
        for m in &menus {
            let mut all = Vec::new();
            for it in &m.items {
                it.walk(&mut all);
            }
            for it in all {
                if let crate::menu::MenuItem::Item { id, shortcut, .. } = it {
                    self.menu_shortcuts.insert(id.clone(), shortcut.clone());
                }
            }
        }
        if let Some(s) = &mut self.menu_stream {
            if s.write_all(text.as_bytes()).is_err() {
                self.menu_stream = None;
            }
        }
        self.menu_sent = text;
    }

    /// Pedidos que os widgets deixaram durante o quadro.
    /// Aplica o tamanho da janela (o canto de cima fica parado; o compositor ancora
    /// a janela pelo canto superior esquerdo).
    fn set_size(&mut self, w: u32, h: u32) {
        if (w, h) == (self.w.width, self.w.height) {
            return;
        }
        self.w.width = w;
        self.w.height = h;
        if self.w.fixed {
            self.w.window.set_min_size(Some((w, h)));
            self.w.window.set_max_size(Some((w, h)));
        }
    }

    /// Avança a animação de tamanho e pede o próximo quadro enquanto ela durar.
    fn step_size_anim(&mut self) {
        let Some((from, to, start, dur)) = self.w.size_anim else { return };
        let t = (start.elapsed().as_secs_f32() / dur.as_secs_f32()).min(1.0);
        // Começa rápido e freia no fim.
        let e = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: u32, b: u32| (a as f32 + (b as f32 - a as f32) * e).round() as u32;
        self.set_size(lerp(from.0, to.0), lerp(from.1, to.1));
        if t >= 1.0 {
            self.w.size_anim = None;
        } else {
            self.w.mem.wake_after = Some(self.w.mem.wake_after.map_or(Duration::from_millis(16), |w| w.min(Duration::from_millis(16))));
        }
    }

    /// Devolve verdadeiro se o tamanho mudou na hora (o quadro precisa ser refeito).
    fn after_frame(&mut self) -> bool {
        let mut resized = false;
        if let Some((w, h, animate)) = self.w.mem.size_req.take() {
            let target = self.w.size_anim.map(|a| a.1).unwrap_or((self.w.width, self.w.height));
            if (w, h) != target {
                if animate && self.w.shown {
                    let d = (w as f32 - self.w.width as f32).abs().max((h as f32 - self.w.height as f32).abs());
                    let dur = Duration::from_millis((d * 0.8).clamp(120.0, 300.0) as u64);
                    self.w.size_anim = Some(((self.w.width, self.w.height), (w, h), std::time::Instant::now(), dur));
                    self.step_size_anim();
                } else {
                    self.w.size_anim = None;
                    self.set_size(w, h);
                    resized = true;
                }
            }
        }
        self.send_menus();
        for cmd in std::mem::take(&mut self.w.mem.glove_cmds) {
            use std::io::Write;
            if let Some(s) = &mut self.menu_stream {
                let _ = s.write_all(format!("{cmd}\n").as_bytes());
            }
        }
        if std::mem::take(&mut self.w.mem.resize_req) {
            if let Some(seat) = &self.seat {
                self.w.window.resize(seat, self.last_serial, smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge::BottomRight);
            }
        }
        for (id, opts) in std::mem::take(&mut self.w.mem.open_windows) {
            if self.w.id != id && !self.others.iter().any(|o| o.id == id) {
                let win = self.shells.open(&self.qh, id, &opts);
                self.others.push(win);
            }
        }
        if let Some(d) = self.w.mem.wake_after.take() {
            if !self.w.wake_pending {
                self.w.wake_pending = true;
                let id = self.w.id;
                let _ = self.loop_handle.insert_source(Timer::from_duration(d), move |_, _, st: &mut Runner<A>| {
                    if !st.focus_id(id) {
                        return TimeoutAction::Drop;
                    }
                    st.w.wake_pending = false;
                    // A animação de tamanho refaz tudo; as outras podem ser parciais.
                    if st.w.size_anim.is_some() {
                        st.w.dirty = true;
                    } else {
                        st.w.anim_dirty = true;
                    }
                    st.draw_now();
                    TimeoutAction::Drop
                });
            }
        }
        if std::mem::take(&mut self.w.mem.move_req) {
            if let Some(seat) = &self.seat {
                self.w.window.move_(seat, self.last_serial);
            }
        }
        if std::mem::take(&mut self.w.mem.minimize_req) {
            self.w.window.set_minimized();
        }
        if let Some(t) = self.w.mem.title.take() {
            self.w.window.set_title(t);
        }
        if let Some(text) = self.w.mem.clip_out.take() {
            self.clip_text = text;
            if let (Some(m), Some(d)) = (&self.data_manager, &self.data_device) {
                let src = m.create_copy_paste_source(&self.qh, TEXT_MIMES);
                src.set_selection(d, self.last_serial);
                self.clip_source = Some(src);
            }
        }
        if let Some(id) = self.w.mem.paste_req.take() {
            self.request_paste(id);
        }
        resized
    }

    /// Lê a área de transferência sem travar (o texto chega por um pipe).
    fn request_paste(&mut self, id: crate::ui::Id) {
        // Se a seleção é nossa, nem precisa ir ao compositor.
        if self.clip_source.is_some() || self.data_device.is_none() {
            if !self.clip_text.is_empty() {
                self.w.mem.paste_in = Some((id, self.clip_text.clone()));
                self.w.dirty = true;
            }
            return;
        }
        let Some(offer) = self.data_device.as_ref().and_then(|d| d.data().selection_offer()) else { return };
        let Some(mime) = offer.with_mime_types(|m| TEXT_MIMES.iter().find(|t| m.iter().any(|x| x == *t)).map(|t| t.to_string())) else { return };
        let Ok(pipe) = offer.receive(mime) else { return };
        let mut data = Vec::new();
        let win = self.w.id;
        let _ = self.loop_handle.insert_source(pipe, move |_, f, st: &mut Runner<A>| {
            use std::io::Read;
            // Segurança: o descritor pertence à fonte enquanto ela existir.
            let f: &mut std::fs::File = unsafe { f.get_mut() };
            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => {
                        if !st.focus_id(win) {
                            return PostAction::Remove;
                        }
                        st.w.mem.paste_in = Some((id, String::from_utf8_lossy(&data).into_owned()));
                        st.w.dirty = true;
                        st.draw_now();
                        return PostAction::Remove;
                    }
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return PostAction::Continue,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return PostAction::Remove,
                }
            }
        });
    }

    /// Manda o quadro para a tela: só as áreas em `damage` são copiadas e
    /// anunciadas ao compositor. Sem mudança, nada é enviado.
    fn present(&mut self, c: &Canvas, damage: &[[i32; 4]]) {
        if damage.is_empty() {
            return;
        }
        let (pw, ph) = (c.px_width() as i32, c.px_height() as i32);
        // Um buffer livre do mesmo tamanho; senão, um novo (no máximo 3).
        self.w.buffers.retain(|(b, _)| b.height() == ph && b.stride() == pw * 4);
        let free = self.w.buffers.iter().position(|(b, _)| self.pool.canvas(b).is_some());
        let idx = match free {
            Some(i) => i,
            None => {
                let Ok((b, _)) = self.pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
                if self.w.buffers.len() >= 3 {
                    self.w.buffers.remove(0);
                }
                self.w.buffers.push((b, 0));
                self.w.buffers.len() - 1
            }
        };
        self.w.frame_no += 1;
        let n = self.w.frame_no;
        // O que copiar: o que mudou desde o quadro que o buffer guarda.
        let held = self.w.buffers[idx].1;
        let mut copy: Vec<[i32; 4]> = damage.to_vec();
        let known = held > 0 && (held + 1 == n || self.w.history.front().is_some_and(|(f, _)| *f <= held + 1));
        if known {
            for (f, rects) in &self.w.history {
                if *f > held {
                    copy.extend_from_slice(rects);
                }
            }
        } else {
            copy = vec![[0, 0, pw, ph]];
        }
        let (buffer, _) = &self.w.buffers[idx];
        let Some(dst) = self.pool.canvas(buffer) else { return };
        let src = c.data();
        // RGBA (tiny-skia) → BGRA (wl_shm Argb8888 em little-endian)
        for [x0, y0, x1, y1] in copy {
            for y in y0..y1 {
                let a = ((y * pw + x0) * 4) as usize;
                let b = ((y * pw + x1) * 4) as usize;
                for (d, s) in dst[a..b].chunks_exact_mut(4).zip(src[a..b].chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
            }
        }
        let surface = self.w.window.wl_surface();
        if let Some(v) = &self.w.viewport {
            v.set_destination(self.w.width as i32, self.w.height as i32);
        } else {
            surface.set_buffer_scale(self.w.int_scale);
        }
        for [x0, y0, x1, y1] in damage {
            surface.damage_buffer(*x0, *y0, x1 - x0, y1 - y0);
        }
        surface.frame(&self.qh, smithay_client_toolkit::compositor::FrameCallbackData(surface.clone()));
        self.w.frame_pending = true;
        if buffer.attach_to(surface).is_ok() {
            self.w.window.commit();
            self.w.shown = true;
        }
        self.w.buffers[idx].1 = n;
        self.w.history.push_back((n, damage.to_vec()));
        while self.w.history.len() > 4 {
            self.w.history.pop_front();
        }
    }

    fn key(&mut self, ev: &KeyEvent) {
        let key = match ev.keysym {
            Keysym::BackSpace => Key::Backspace,
            Keysym::Delete => Key::Delete,
            Keysym::Return | Keysym::KP_Enter => Key::Enter,
            Keysym::Escape => Key::Escape,
            Keysym::Tab | Keysym::ISO_Left_Tab => Key::Tab,
            Keysym::Left => Key::Left,
            Keysym::Right => Key::Right,
            Keysym::Up => Key::Up,
            Keysym::Down => Key::Down,
            Keysym::Home => Key::Home,
            Keysym::End => Key::End,
            Keysym::Page_Up => Key::PageUp,
            Keysym::Page_Down => Key::PageDown,
            _ => Key::Other,
        };
        // Combinação no formato dos atalhos ("Super+Shift+Q").
        let name = ev.keysym.name().map(|n| n.trim_start_matches("XK_").to_string()).unwrap_or_default();
        let is_mod = matches!(
            ev.keysym,
            Keysym::Shift_L | Keysym::Shift_R | Keysym::Control_L | Keysym::Control_R | Keysym::Alt_L | Keysym::Alt_R | Keysym::Super_L | Keysym::Super_R | Keysym::Meta_L | Keysym::Meta_R
        );
        let combo = if is_mod || name.is_empty() {
            String::new()
        } else {
            let m = &self.w.input.mods;
            let mut parts = Vec::new();
            if m.ctrl {
                parts.push("Ctrl".to_string());
            }
            if m.alt {
                parts.push("Alt".to_string());
            }
            if m.shift {
                parts.push("Shift".to_string());
            }
            if m.logo {
                parts.push("Super".to_string());
            }
            let n = if name.chars().count() == 1 { name.to_uppercase() } else { name };
            parts.push(n);
            parts.join("+")
        };
        self.w.input.keys.push(KeyPress { key, combo });
        if let Some(t) = &ev.utf8 {
            if !self.w.input.mods.ctrl && !self.w.input.mods.logo {
                self.w.input.text.push_str(t);
            }
        }
        self.w.dirty = true;
        self.draw_now();
    }
}

impl<A: App> CompositorHandler for Runner<A> {
    fn scale_factor_changed(&mut self, _: &Connection, qh: &QueueHandle<Self>, surface: &wl_surface::WlSurface, factor: i32) {
        if !self.focus_surface(surface) {
            return;
        }
        if self.w.int_scale != factor {
            self.w.int_scale = factor;
            if self.w.viewport.is_none() {
                self.request_redraw(qh);
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        if !self.focus_surface(surface) {
            return;
        }
        self.w.frame_pending = false;
        self.draw_now();
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl<A: App> OutputHandler for Runner<A> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl<A: App> WindowHandler for Runner<A> {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        if !self.focus_surface(window.wl_surface()) {
            return;
        }
        if self.app.close_window(self.w.id) {
            self.close_current();
        } else {
            self.w.dirty = true;
            self.draw_now();
        }
    }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, window: &Window, cfg: WindowConfigure, _: u32) {
        if !self.focus_surface(window.wl_surface()) {
            return;
        }
        if !self.w.fixed {
            if let (Some(w), Some(h)) = cfg.new_size {
                self.w.width = w.get();
                self.w.height = h.get();
            }
        }
        self.w.configured = true;
        self.w.need_commit = true;
        self.request_redraw(qh);
    }
}

impl<A: App> SeatHandler for Runner<A> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
        self.seat = Some(seat.clone());
        if self.data_device.is_none() {
            if let Some(m) = &self.data_manager {
                self.data_device = Some(m.get_data_device(qh, &seat));
            }
        }
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard_with_repeat(qh, &seat, None, self.loop_handle.clone(), Box::new(|st: &mut Self, _, ev| st.key(&ev)))
                .ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<A: App> KeyboardHandler for Runner<A> {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.kbd_surface = Some(surface.clone());
        if !self.focus_surface(surface) {
            return;
        }
        self.w.input.active = true;
        self.w.dirty = true;
        self.draw_now();
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        if self.kbd_surface.as_ref() == Some(surface) {
            self.kbd_surface = None;
        }
        if !self.focus_surface(surface) {
            return;
        }
        self.w.input.active = false;
        self.w.dirty = true;
        self.draw_now();
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, serial: u32, ev: KeyEvent) {
        self.focus_keyboard();
        self.last_serial = serial;
        self.key(&ev);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        self.focus_keyboard();
        self.key(&ev);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.focus_keyboard();
        self.w.input.mods = crate::ui::Mods { ctrl: m.ctrl, alt: m.alt, shift: m.shift, logo: m.logo };
    }
}

impl<A: App> PointerHandler for Runner<A> {
    fn pointer_frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        // Cada evento vai para a janela dele; depois, redesenha as que receberam.
        let mut touched: Vec<u32> = Vec::new();
        for e in events {
            if !self.focus_surface(&e.surface) {
                continue;
            }
            if !touched.contains(&self.w.id) {
                touched.push(self.w.id);
            }
            self.w.input.pos = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } => self.w.input.inside = true,
                PointerEventKind::Leave { .. } => self.w.input.inside = false,
                PointerEventKind::Motion { .. } => self.w.input.moved = true,
                // Botão direito: só para menus de contexto (não é um clique comum).
                PointerEventKind::Press { button: 0x111, serial, .. } => {
                    self.last_serial = serial;
                    self.w.input.right_pressed = true;
                }
                PointerEventKind::Release { button: 0x111, .. } => {}
                PointerEventKind::Press { serial, time, .. } => {
                    self.last_serial = serial;
                    self.w.input.down = true;
                    self.w.input.pressed = true;
                    // Clique duplo/triplo: dentro do tempo das Preferências e perto do anterior.
                    let (t0, p0, n) = self.w.last_click;
                    let near = (p0.0 - self.w.input.pos.0).abs() < 4.0 && (p0.1 - self.w.input.pos.1).abs() < 4.0;
                    let count = if time.wrapping_sub(t0) < DOUBLE_CLICK_MS.load(std::sync::atomic::Ordering::Relaxed) && near { (n % 3) + 1 } else { 1 };
                    self.w.last_click = (time, self.w.input.pos, count);
                    self.w.input.clicks = count;
                }
                PointerEventKind::Release { .. } => {
                    self.w.input.down = false;
                    self.w.input.released = true;
                }
                PointerEventKind::Axis { vertical, .. } => self.w.input.scroll += vertical.absolute as f32,
            }
        }
        for id in touched {
            if self.focus_id(id) {
                self.request_redraw(qh);
            }
        }
    }
}

impl<A: App> DataDeviceHandler for Runner<A> {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64, _: &wl_surface::WlSurface) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl<A: App> DataOfferHandler for Runner<A> {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl<A: App> DataSourceHandler for Runner<A> {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: String, fd: WritePipe) {
        use std::io::Write;
        let mut f = std::fs::File::from(std::os::fd::OwnedFd::from(fd));
        let _ = f.write_all(self.clip_text.as_bytes());
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        // Outro app copiou algo: a seleção deixou de ser nossa.
        if self.clip_source.as_ref().is_some_and(|s| s.inner() == source) {
            self.clip_source = None;
        }
        source.destroy();
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl<A: App> ShmHandler for Runner<A> {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(@<A: App + 'static> Runner<A>);

impl<A: App> ProvidesRegistryState for Runner<A> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState,];
}

smithay_client_toolkit::delegate_dispatch2!(@<A: App + 'static> Runner<A>);

#[cfg(test)]
mod tests {
    use super::frame_damage;

    #[test]
    fn so_o_que_mudou() {
        let (w, h) = (20, 16);
        let mut prev = Vec::new();
        let mut cur = vec![0u8; (w * h * 4) as usize];
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[0, 0, w, h]]);
        assert!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]).is_empty());
        let set = |cur: &mut Vec<u8>, x: i32, y: i32| cur[((y * w + x) * 4) as usize] = 255;
        set(&mut cur, 5, 3);
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[5, 3, 6, 4]]);
        assert_eq!(prev, cur);
        // Duas mudanças próximas viram uma área; uma distante, outra.
        set(&mut cur, 2, 0);
        set(&mut cur, 9, 2);
        set(&mut cur, 7, 15);
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[2, 0, 10, 3], [7, 15, 8, 16]]);
        // Fora da região comparada não conta.
        set(&mut cur, 0, 10);
        assert!(frame_damage(&mut prev, &cur, w, h, [5, 0, w, h]).is_empty());
    }
}
