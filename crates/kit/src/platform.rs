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
    Theme::new(Variant::from_config(&c.theme))
}

struct Runner<A: App> {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
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
    exit: bool,
    input: Input,
    mem: Memory,
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
    // Clique duplo/triplo
    last_click: (u32, (f32, f32), u8),
    seat: Option<wl_seat::WlSeat>,
    // Menus na barra global
    menu_stream: Option<std::os::unix::net::UnixStream>,
    menu_sent: String,
    menu_shortcuts: std::collections::HashMap<String, String>,
    /// Já há um timer de "acordar" pendente (ui.wake_after).
    wake_pending: bool,
    /// Mudança de tamanho pedida pelo app, animada: (de, para, início, duração).
    size_anim: Option<((u32, u32), (u32, u32), std::time::Instant, Duration)>,
    /// Algum quadro já foi para a tela (antes disso o tamanho muda sem animar).
    shown: bool,
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
    fn event(&self, st: &mut Runner<A>, _: &WpFractionalScaleV1, ev: wp_fractional_scale_v1::Event, _: &Connection, qh: &QueueHandle<Runner<A>>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = ev {
            let s = scale as f64 / 120.0;
            if (s - st.scale).abs() > 1e-6 {
                st.scale = s;
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

    let surface = compositor.create_surface(&qh);
    let (viewport, frac) = match (&frac_mgr, &viewporter) {
        (Some(m), Some(v)) => (Some(v.get_viewport(&surface, &qh, NoEvents)), Some(m.get_fractional_scale(&surface, &qh, FracData))),
        _ => (None, None),
    };
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_title(opts.title.clone());
    window.set_app_id(opts.app_id.clone());
    if opts.fixed {
        window.set_min_size(Some(opts.size));
        window.set_max_size(Some(opts.size));
    } else {
        window.set_min_size(Some((opts.size.0 / 2, opts.size.1 / 2)));
    }
    window.commit();

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
                    st.dirty = true;
                    st.draw_now();
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
                st.dirty = true;
                st.draw_now();
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
        buffers: Vec::new(),
        frame_no: 0,
        history: std::collections::VecDeque::new(),
        prev: Vec::new(),
        prev_key: (0, 0, 0),
        canvas: None,
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
        exit: false,
        input: Input::default(),
        mem: Memory::with_app_icon(app_icon(&opts.app_id)),
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
        last_click: (0, (0.0, 0.0), 0),
        seat: None,
        menu_stream: None,
        menu_sent: String::new(),
        menu_shortcuts: std::collections::HashMap::new(),
        wake_pending: false,
        size_anim: None,
        shown: false,
    };
    runner.connect_menus();
    if let Some(fd) = runner.app.watch_fd() {
        // Segurança: o app garante que o descritor vive enquanto a janela existir.
        let src = Generic::new(unsafe { smithay_client_toolkit::reexports::calloop::generic::FdWrapper::new(fd) }, Interest::READ, Mode::Level);
        event_loop
            .handle()
            .insert_source(src, |_, _, st: &mut Runner<A>| {
                let keep = st.app.fd_ready();
                st.dirty = true;
                st.draw_now();
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
    fn request_redraw(&mut self, _qh: &QueueHandle<Self>) {
        self.dirty = true;
        if !self.frame_pending {
            self.draw_now();
        }
    }

    fn draw_now(&mut self) {
        // Espera o compositor mostrar o quadro anterior: nunca desenha mais
        // rápido que a tela (importante quando chega muita saída de uma vez).
        if !self.configured || !(self.dirty || self.anim_dirty) || self.frame_pending {
            return;
        }
        self.step_size_anim();
        let t0 = std::time::Instant::now();
        // Redesenho parcial: se só as animações pediram este quadro (nenhum
        // evento, nada do app), refaz só a área delas; o resto do canvas já
        // está certo desde o quadro anterior.
        let anim_rects = std::mem::take(&mut self.mem.anim_rects);
        let mut partial = !self.dirty
            && self.anim_dirty
            && !self.mem.anim_full
            && !anim_rects.is_empty()
            && self.size_anim.is_none()
            && !self.input.has_events()
            && !self.input.moved;
        self.anim_dirty = false;
        // Até 3 passadas seguidas se a interface pedir (ex.: pop-up que acabou de abrir).
        for pass in 0..3 {
            self.dirty = false;
            self.mem.anim_rects.clear();
            self.mem.anim_full = false;
            let scale = if self.viewport.is_some() { self.scale } else { self.int_scale as f64 } as f32;
            let reuse = self.canvas.take().filter(|c| c.w == self.width as f32 && c.h == self.height as f32 && c.scale == scale);
            partial &= reuse.is_some();
            let Some(mut canvas) = reuse.or_else(|| Canvas::new(self.width as f32, self.height as f32, scale)) else { return };
            let region = partial.then(|| px_union(&anim_rects, scale));
            canvas.set_clip(region);
            if !self.app.paints_background() {
                canvas.clear(self.theme.body_a);
                canvas.pinstripes(0.0, 0.0, self.width as f32, self.height as f32, self.theme.body_a, self.theme.body_b);
            }
            let (redraw, close) = {
                let mut ui = Ui::new(&mut canvas, &self.theme, &self.fonts, &self.doc_fonts, !self.fixed, &self.input, &mut self.mem);
                self.app.ui(&mut ui);
                ui.finish()
            };
            // Um clique muda o estado no meio do quadro (ex.: o visor já foi
            // desenhado antes do botão). Mais uma passada, sem eventos, mostra
            // o resultado na hora. Só a última passada vai para a tela.
            canvas.set_clip(None);
            let resized = self.after_frame();
            let had_events = self.input.has_events();
            self.input.end_frame();
            if close {
                self.exit = true;
            }
            if (redraw || had_events || resized) && pass < 2 {
                // A próxima passada pode mudar qualquer coisa: refaz tudo.
                partial = false;
                self.canvas = Some(canvas);
                self.dirty = true;
                continue;
            }
            let t1 = std::time::Instant::now();
            let key = (canvas.px_width(), canvas.px_height(), canvas.scale.to_bits());
            if key != self.prev_key {
                self.prev.clear();
                self.prev_key = key;
            }
            let (pw, ph) = (canvas.px_width() as i32, canvas.px_height() as i32);
            let damage = frame_damage(&mut self.prev, canvas.data(), pw, ph, region.unwrap_or([0, 0, pw, ph]));
            self.present(&canvas, &damage);
            if std::mem::take(&mut self.need_commit) && damage.is_empty() {
                self.window.commit();
            }
            if std::env::var_os("GLOVE_PROF").is_some() {
                let px: i32 = damage.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
                eprintln!("PROF desenho={:?} envio={:?} passadas={} parcial={} mudou={}px em {} áreas", t1 - t0, t1.elapsed(), pass + 1, region.is_some(), px, damage.len());
            }
            self.canvas = Some(canvas);
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
        match self.menu_shortcuts.get(id).filter(|s| !s.is_empty()).cloned() {
            Some(combo) => self.input.keys.push(KeyPress { key: Key::Other, combo }),
            None => self.input.menu_actions.push(id.to_string()),
        }
        self.dirty = true;
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
        if (w, h) == (self.width, self.height) {
            return;
        }
        self.width = w;
        self.height = h;
        if self.fixed {
            self.window.set_min_size(Some((w, h)));
            self.window.set_max_size(Some((w, h)));
        }
    }

    /// Avança a animação de tamanho e pede o próximo quadro enquanto ela durar.
    fn step_size_anim(&mut self) {
        let Some((from, to, start, dur)) = self.size_anim else { return };
        let t = (start.elapsed().as_secs_f32() / dur.as_secs_f32()).min(1.0);
        // Começa rápido e freia no fim.
        let e = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: u32, b: u32| (a as f32 + (b as f32 - a as f32) * e).round() as u32;
        self.set_size(lerp(from.0, to.0), lerp(from.1, to.1));
        if t >= 1.0 {
            self.size_anim = None;
        } else {
            self.mem.wake_after = Some(self.mem.wake_after.map_or(Duration::from_millis(16), |w| w.min(Duration::from_millis(16))));
        }
    }

    /// Devolve verdadeiro se o tamanho mudou na hora (o quadro precisa ser refeito).
    fn after_frame(&mut self) -> bool {
        let mut resized = false;
        if let Some((w, h, animate)) = self.mem.size_req.take() {
            let target = self.size_anim.map(|a| a.1).unwrap_or((self.width, self.height));
            if (w, h) != target {
                if animate && self.shown {
                    let d = (w as f32 - self.width as f32).abs().max((h as f32 - self.height as f32).abs());
                    let dur = Duration::from_millis((d * 0.8).clamp(120.0, 300.0) as u64);
                    self.size_anim = Some(((self.width, self.height), (w, h), std::time::Instant::now(), dur));
                    self.step_size_anim();
                } else {
                    self.size_anim = None;
                    self.set_size(w, h);
                    resized = true;
                }
            }
        }
        self.send_menus();
        for cmd in std::mem::take(&mut self.mem.glove_cmds) {
            use std::io::Write;
            if let Some(s) = &mut self.menu_stream {
                let _ = s.write_all(format!("{cmd}\n").as_bytes());
            }
        }
        if std::mem::take(&mut self.mem.resize_req) {
            if let Some(seat) = &self.seat {
                self.window.resize(seat, self.last_serial, smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge::BottomRight);
            }
        }
        if let Some(d) = self.mem.wake_after.take() {
            if !self.wake_pending {
                self.wake_pending = true;
                let _ = self.loop_handle.insert_source(Timer::from_duration(d), |_, _, st: &mut Runner<A>| {
                    st.wake_pending = false;
                    // A animação de tamanho refaz tudo; as outras podem ser parciais.
                    if st.size_anim.is_some() {
                        st.dirty = true;
                    } else {
                        st.anim_dirty = true;
                    }
                    st.draw_now();
                    TimeoutAction::Drop
                });
            }
        }
        if std::mem::take(&mut self.mem.minimize_req) {
            self.window.set_minimized();
        }
        if let Some(t) = self.mem.title.take() {
            self.window.set_title(t);
        }
        if let Some(text) = self.mem.clip_out.take() {
            self.clip_text = text;
            if let (Some(m), Some(d)) = (&self.data_manager, &self.data_device) {
                let src = m.create_copy_paste_source(&self.qh, TEXT_MIMES);
                src.set_selection(d, self.last_serial);
                self.clip_source = Some(src);
            }
        }
        if let Some(id) = self.mem.paste_req.take() {
            self.request_paste(id);
        }
        resized
    }

    /// Lê a área de transferência sem travar (o texto chega por um pipe).
    fn request_paste(&mut self, id: crate::ui::Id) {
        // Se a seleção é nossa, nem precisa ir ao compositor.
        if self.clip_source.is_some() || self.data_device.is_none() {
            if !self.clip_text.is_empty() {
                self.mem.paste_in = Some((id, self.clip_text.clone()));
                self.dirty = true;
            }
            return;
        }
        let Some(offer) = self.data_device.as_ref().and_then(|d| d.data().selection_offer()) else { return };
        let Some(mime) = offer.with_mime_types(|m| TEXT_MIMES.iter().find(|t| m.iter().any(|x| x == *t)).map(|t| t.to_string())) else { return };
        let Ok(pipe) = offer.receive(mime) else { return };
        let mut data = Vec::new();
        let _ = self.loop_handle.insert_source(pipe, move |_, f, st: &mut Runner<A>| {
            use std::io::Read;
            // Segurança: o descritor pertence à fonte enquanto ela existir.
            let f: &mut std::fs::File = unsafe { f.get_mut() };
            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => {
                        st.mem.paste_in = Some((id, String::from_utf8_lossy(&data).into_owned()));
                        st.dirty = true;
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
        self.buffers.retain(|(b, _)| b.height() == ph && b.stride() == pw * 4);
        let free = self.buffers.iter().position(|(b, _)| self.pool.canvas(b).is_some());
        let idx = match free {
            Some(i) => i,
            None => {
                let Ok((b, _)) = self.pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
                if self.buffers.len() >= 3 {
                    self.buffers.remove(0);
                }
                self.buffers.push((b, 0));
                self.buffers.len() - 1
            }
        };
        self.frame_no += 1;
        let n = self.frame_no;
        // O que copiar: o que mudou desde o quadro que o buffer guarda.
        let held = self.buffers[idx].1;
        let mut copy: Vec<[i32; 4]> = damage.to_vec();
        let known = held > 0 && (held + 1 == n || self.history.front().is_some_and(|(f, _)| *f <= held + 1));
        if known {
            for (f, rects) in &self.history {
                if *f > held {
                    copy.extend_from_slice(rects);
                }
            }
        } else {
            copy = vec![[0, 0, pw, ph]];
        }
        let (buffer, _) = &self.buffers[idx];
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
        let surface = self.window.wl_surface();
        if let Some(v) = &self.viewport {
            v.set_destination(self.width as i32, self.height as i32);
        } else {
            surface.set_buffer_scale(self.int_scale);
        }
        for [x0, y0, x1, y1] in damage {
            surface.damage_buffer(*x0, *y0, x1 - x0, y1 - y0);
        }
        surface.frame(&self.qh, smithay_client_toolkit::compositor::FrameCallbackData(surface.clone()));
        self.frame_pending = true;
        if buffer.attach_to(surface).is_ok() {
            self.window.commit();
            self.shown = true;
        }
        self.buffers[idx].1 = n;
        self.history.push_back((n, damage.to_vec()));
        while self.history.len() > 4 {
            self.history.pop_front();
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
            let m = &self.input.mods;
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
        self.input.keys.push(KeyPress { key, combo });
        if let Some(t) = &ev.utf8 {
            if !self.input.mods.ctrl && !self.input.mods.logo {
                self.input.text.push_str(t);
            }
        }
        self.dirty = true;
        self.draw_now();
    }
}

impl<A: App> CompositorHandler for Runner<A> {
    fn scale_factor_changed(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, factor: i32) {
        if self.int_scale != factor {
            self.int_scale = factor;
            if self.viewport.is_none() {
                self.request_redraw(qh);
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.frame_pending = false;
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
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        if self.app.close_requested() {
            self.exit = true;
        } else {
            self.dirty = true;
            self.draw_now();
        }
    }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &Window, cfg: WindowConfigure, _: u32) {
        if !self.fixed {
            if let (Some(w), Some(h)) = cfg.new_size {
                self.width = w.get();
                self.height = h.get();
            }
        }
        self.configured = true;
        self.need_commit = true;
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
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.input.active = true;
        self.dirty = true;
        self.draw_now();
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {
        self.input.active = false;
        self.dirty = true;
        self.draw_now();
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, serial: u32, ev: KeyEvent) {
        self.last_serial = serial;
        self.key(&ev);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        self.key(&ev);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.input.mods = crate::ui::Mods { ctrl: m.ctrl, alt: m.alt, shift: m.shift, logo: m.logo };
    }
}

impl<A: App> PointerHandler for Runner<A> {
    fn pointer_frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for e in events {
            self.input.pos = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } => self.input.inside = true,
                PointerEventKind::Leave { .. } => self.input.inside = false,
                PointerEventKind::Motion { .. } => self.input.moved = true,
                PointerEventKind::Press { serial, time, .. } => {
                    self.last_serial = serial;
                    self.input.down = true;
                    self.input.pressed = true;
                    // Clique duplo/triplo: até 400 ms e perto do anterior.
                    let (t0, p0, n) = self.last_click;
                    let near = (p0.0 - self.input.pos.0).abs() < 4.0 && (p0.1 - self.input.pos.1).abs() < 4.0;
                    let count = if time.wrapping_sub(t0) < 400 && near { (n % 3) + 1 } else { 1 };
                    self.last_click = (time, self.input.pos, count);
                    self.input.clicks = count;
                }
                PointerEventKind::Release { .. } => {
                    self.input.down = false;
                    self.input.released = true;
                }
                PointerEventKind::Axis { vertical, .. } => self.input.scroll += vertical.absolute as f32,
            }
        }
        self.request_redraw(qh);
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
