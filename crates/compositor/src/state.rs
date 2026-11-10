use glove_config::tr;
use std::{ffi::OsString, sync::Arc};

use glove_paint::{Fonts, Theme, Variant};
use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        fractional_scale::FractionalScaleManagerState,
        viewporter::ViewporterState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::{decoration::XdgDecorationState, XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use glove_config::Config;

use crate::{input::Binding, shell::Chrome, CalloopData};

pub struct Glove {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub space: Space<Window>,
    pub loop_signal: LoopSignal,
    pub loop_handle: LoopHandle<'static, CalloopData>,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub viewporter_state: ViewporterState,
    /// linux-dmabuf (anunciado pelo backend, que sabe qual é a placa).
    pub dmabuf_state: smithay::wayland::dmabuf::DmabufState,
    pub dmabuf_global: Option<smithay::wayland::dmabuf::DmabufGlobal>,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Glove>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,

    pub config: Config,
    /// Atalhos já interpretados a partir da configuração.
    pub bindings: Vec<Binding>,
    pub theme: Theme,
    /// Muda a cada troca de tema, invalidando os caches de desenho.
    pub theme_gen: u64,
    pub fonts: Fonts,
    pub chrome: Chrome,

    /// Janelas minimizadas e onde estavam.
    pub minimized: Vec<(Window, Point<i32, Logical>)>,
    /// Janelas escondidas por "Ocultar" (não aparecem no Dock) e onde estavam.
    pub hidden: Vec<(Window, Point<i32, Logical>)>,
    /// Último clique numa barra de título (para o clique duplo).
    pub title_click: Option<(Window, std::time::Instant)>,
    /// Apertou o ícone do documento no título: (arquivo, onde). Andando, vira arrastar.
    pub proxy_press: Option<(std::path::PathBuf, smithay::utils::Point<f64, smithay::utils::Logical>)>,
    /// Há algo novo para desenhar.
    pub dirty: bool,
    /// Rodando dentro de outro desktop (atalhos usam Alt em vez de Super).
    pub nested: bool,
    pub cascade: i32,
    pub suppressed_keys: Vec<u32>,
    pub pointer_pos: Point<f64, Logical>,
    /// Apps conectados para mostrar seus menus na barra.
    pub menu_clients: Vec<crate::menus::MenuClient>,
    pub menu_seq: u64,
    pub menu_socket: Option<std::path::PathBuf>,
    /// Itens Recentes do menu principal.
    pub recent: crate::recent::Recent,
    /// Pedido de captura de tela (salva no próximo quadro).
    pub screenshot_request: Option<std::path::PathBuf>,
    /// Abrir a captura no Visualizador depois de salvar.
    pub open_after_shot: bool,
    /// Cursor pedido pelo app sob o ponteiro (desenhado só na sessão própria).
    pub cursor_status: smithay::input::pointer::CursorImageStatus,
    /// Ícone do que está sendo arrastado entre apps (segue o mouse).
    pub dnd_icon: Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    /// A seta padrão, desenhada na escala atual.
    pub cursor_tex: Option<crate::shell::Tex>,
    pub cursor_tex_scale: f64,
    /// Setas de redimensionar: ↔, ↕, ⤡ e ⤢ (na mesma escala da seta).
    pub resize_cursors: [Option<crate::shell::Tex>; 4],
    /// O mouse está sobre uma borda de janela que redimensiona (quais lados).
    pub resize_hover: Option<crate::grabs::ResizeEdge>,
    /// Uma janela está sendo redimensionada (quais lados).
    pub resize_active: Option<crate::grabs::ResizeEdge>,
    /// Protetor de tela na tela.
    pub saver: Option<crate::saver::ScreenSaver>,
    /// Escolhendo a área de uma captura.
    pub region: Option<crate::region::RegionSel>,
    /// Área (lógica) da próxima captura; None = a tela inteira.
    pub screenshot_crop: Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>,
    /// Textura e programa dos efeitos do Acesso Universal.
    pub effects: std::cell::RefCell<crate::render::EffectCache>,
    /// Ctrl+Alt+F1…F12 na sessão própria: trocar para esse console.
    pub vt_request: Option<i32>,
    /// Fim de sessão em andamento.
    pub ending: Option<crate::ending::Ending>,
    /// Pings aos apps (detecta quem não responde).
    pub pinger: crate::forcequit::Pinger,
    /// Painel Forçar Encerrar aberto.
    pub force_quit: Option<crate::forcequit::ForceQuit>,
    pub wait_cursor: crate::forcequit::WaitCursor,
    /// Volume, bateria, controle de volume e aviso na tela.
    pub extras: crate::extras::Extras,
    /// Tela bloqueada (painel de senha).
    pub lock: Option<crate::lock::Lock>,
    /// Tempo parado e tela apagada.
    pub idle: crate::lock::Idle,
    /// O clique que acendeu a tela não vai para os apps (nem o soltar dele).
    pub swallow_wake_release: bool,
    /// Painel "Iniciando sessão" (só na entrada).
    pub startup: Option<crate::startup::Startup>,
    /// Busca Rápida.
    pub search: crate::search::Search,
    /// A mesa: itens da pasta Mesa e discos (ver desk.rs).
    pub desk: crate::desk::Desk,
    /// Painel de Widgets (F12).
    pub dashboard: crate::dashboard::Dashboard,
    pub grade: crate::grade::Grade,
    /// Tela cheia: o mouse encostou no alto e a barra de menu apareceu.
    pub bar_peek: bool,
    /// Gesto do touchpad em andamento.
    pub gesto: Option<crate::gestos::Gesto>,
    /// Avisos (notificações) e a central.
    pub avisos: crate::avisos::Avisos,
    /// Mesas virtuais.
    pub mesas: crate::mesas::Mesas,
    /// Exposé em andamento.
    pub expose: Option<crate::expose::Expose>,
    /// O mouse saiu dos cantos desde o último disparo.
    pub corner_armed: bool,
    /// Troca de apps (Super+Tab) aberta.
    pub switcher: Option<crate::switcher::Switcher>,
    /// Apps (pid) do usado mais recentemente para o mais antigo.
    pub app_mru: Vec<u32>,
    /// O Super foi solto com a troca aberta: trocar depois de entregar a tecla.
    pub switcher_commit: bool,
    /// Apps abrindo pelo Dock: (app_id, início, fim do último pulo).
    pub launching: Vec<(String, std::time::Instant, Option<std::time::Instant>)>,
    /// Janelas indo para o Dock ou voltando dele.
    pub min_anims: Vec<crate::minimize::MinAnim>,
    pub min_anim_timer_on: bool,
    /// Lugar no Dock de onde cada janela está voltando.
    pub restore_slots: Vec<(Window, smithay::utils::Rectangle<f64, Logical>)>,
    /// Ícones de app já desenhados para o canto das miniaturas.
    pub badges: Vec<((String, i32, u32), std::rc::Rc<crate::shell::Tex>)>,
    pub badge_places: Vec<(Window, (i32, i32), std::rc::Rc<crate::shell::Tex>)>,
    /// Último quadro entregue às minimizadas e se há um agendado.
    pub min_frame_last: std::time::Instant,
    pub min_frame_timer: bool,
    /// Uma minimizada mudou desde o último quadro entregue a elas.
    pub min_commit_pending: bool,
    /// Quando o clique no Dock começou (para o clique segurado abrir o menu).
    pub dock_press_at: Option<std::time::Instant>,
    /// Modo quiosque da tela de login (glove --greeter).
    pub greeter: bool,
    /// Processo do app de login: quando ele termina, o Glove termina.
    pub greeter_child: Option<u32>,
    /// Apps X11: o gerenciador de janelas do Xwayland e o número do display (:N).
    pub xwm: Option<smithay::xwayland::X11Wm>,
    pub xdisplay: Option<u32>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
    /// Janelas X11 que se posicionam sozinhas (menus, dicas): fora do espaço das janelas.
    pub x11_overlays: Vec<Window>,
    /// Nome do monitor principal (barra de menu e Dock); os outros estendem a mesa.
    pub primary_output: Option<String>,
    /// Tampa do notebook fechada (com monitor externo, a tela embutida desliga).
    pub lid_closed: bool,
    /// Os monitores precisam ser reorganizados (a tampa abriu ou fechou).
    pub reconfigure_outputs: bool,
    /// Depuração: capturar a imagem que um monitor extra mostraria (arquivo, área, escala).
    pub extra_shot: Option<(std::path::PathBuf, smithay::utils::Rectangle<i32, Logical>, f64)>,
}

impl Glove {
    pub fn new(event_loop: &mut EventLoop<'static, CalloopData>, display: Display<Self>) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let fractional_scale_state = FractionalScaleManagerState::new::<Self>(&dh);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let xwayland_shell_state = smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(&dh);

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(Default::default(), 300, 30).expect("teclado");
        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let config = Config::load();
        glove_paint::icons::set_theme(&config.appearance.icon_theme);
        let theme = Theme::new(Variant::from_config(&config.theme));
        let fonts = Fonts::load_system().expect("nenhuma fonte encontrada (instale fonts-dejavu)");

        Self {
            start_time: std::time::Instant::now(),
            socket_name,
            display_handle: dh,
            space: Space::default(),
            loop_signal: event_loop.get_signal(),
            loop_handle: event_loop.handle(),
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            fractional_scale_state,
            viewporter_state,
            dmabuf_state: smithay::wayland::dmabuf::DmabufState::new(),
            dmabuf_global: None,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups: PopupManager::default(),
            seat,
            bindings: crate::input::parse_bindings(&config),
            config,
            theme,
            theme_gen: 0,
            fonts,
            chrome: Chrome::default(),
            minimized: Vec::new(),
            hidden: Vec::new(),
            title_click: None,
            proxy_press: None,
            dirty: true,
            nested: false,
            cascade: 0,
            suppressed_keys: Vec::new(),
            pointer_pos: (0.0, 0.0).into(),
            screenshot_request: None,
            open_after_shot: false,
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            dnd_icon: None,
            cursor_tex: None,
            cursor_tex_scale: 0.0,
            resize_cursors: Default::default(),
            resize_hover: None,
            resize_active: None,
            effects: Default::default(),
            saver: None,
            region: None,
            screenshot_crop: None,
            ending: None,
            pinger: Default::default(),
            force_quit: None,
            wait_cursor: Default::default(),
            extras: Default::default(),
            lock: None,
            idle: Default::default(),
            swallow_wake_release: false,
            startup: None,
            search: Default::default(),
            desk: Default::default(),
            dashboard: Default::default(),
            grade: Default::default(),
            bar_peek: false,
            gesto: None,
            avisos: Default::default(),
            mesas: Default::default(),
            expose: None,
            corner_armed: true,
            switcher: None,
            app_mru: Vec::new(),
            switcher_commit: false,
            launching: Vec::new(),
            min_anims: Vec::new(),
            min_anim_timer_on: false,
            restore_slots: Vec::new(),
            badges: Vec::new(),
            badge_places: Vec::new(),
            min_frame_last: std::time::Instant::now(),
            min_frame_timer: false,
            min_commit_pending: false,
            dock_press_at: None,
            greeter: false,
            greeter_child: None,
            vt_request: None,
            menu_clients: Vec::new(),
            menu_seq: 0,
            menu_socket: None,
            recent: crate::recent::Recent::load(),
            xwm: None,
            xdisplay: None,
            xwayland_shell_state,
            x11_overlays: Vec::new(),
            primary_output: None,
            lid_closed: false,
            reconfigure_outputs: false,
            extra_shot: None,
        }
    }

    fn init_wayland_listener(display: Display<Glove>, event_loop: &mut EventLoop<CalloopData>) -> OsString {
        let listening_socket = ListeningSocketSource::new_auto().expect("socket wayland");
        let socket_name = listening_socket.socket_name().to_os_string();
        let handle = event_loop.handle();

        handle
            .insert_source(listening_socket, |stream, _, data| {
                if let Err(e) = data.state.display_handle.insert_client(stream, Arc::new(ClientState::default())) {
                    tracing::warn!("cliente recusado: {e}");
                }
            })
            .expect("fonte de eventos do socket");

        handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, data| {
                // Segurança: o Display nunca é descartado enquanto o loop roda.
                unsafe {
                    display.get_mut().dispatch_clients(&mut data.state).unwrap();
                }
                Ok(PostAction::Continue)
            })
            .expect("fonte de eventos do display");

        socket_name
    }

    /// Superfície de app sob o ponteiro (nunca as peças desenhadas pelo Glove).
    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        match self.hit_test(pos) {
            crate::shell::Hit::Client(window) => {
                // Menus X11 ficam fora do espaço das janelas: a posição é a deles.
                let loc = match self.space.element_location(&window) {
                    Some(l) => l,
                    None => window.x11_surface()?.geometry().loc,
                } - window.geometry().loc;
                window
                    .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + loc).to_f64()))
            }
            _ => None,
        }
    }

    /// Captura de tela para ~/Imagens (ou ~/Pictures).
    pub fn request_screenshot(&mut self) {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let dir = ["Imagens", "Pictures"].iter().map(|d| home.join(d)).find(|d| d.is_dir()).unwrap_or(home);
        let name = unsafe {
            let t = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&t, &mut tm);
            tr!("Captura de tela {} às {}.png", format!("{}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday), format!("{:02}.{:02}.{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec))
        };
        self.screenshot_request = Some(dir.join(name));
        self.dirty = true;
    }

    /// Relê a configuração (as Preferências mudaram algo) e aplica. Devolve se mudou.
    pub fn reload_config(&mut self) -> bool {
        let new = Config::load();
        if new == self.config {
            return false;
        }
        let old = std::mem::replace(&mut self.config, new);
        glove_paint::icons::set_theme(&self.config.appearance.icon_theme);
        if old.theme != self.config.theme || old.wallpaper != self.config.wallpaper || old.appearance.icon_theme != self.config.appearance.icon_theme {
            self.theme = Theme::new(Variant::from_config(&self.config.theme));
            self.theme_gen += 1;
        }
        if old.keyboard != self.config.keyboard {
            self.apply_keyboard();
        }
        self.bindings = crate::input::parse_bindings(&self.config);
        if old.desktops != self.config.desktops {
            self.fit_desks();
        }
        self.dirty = true;
        // Segundos ou dois-pontos piscando: um relógio de segundo em segundo já agora
        // (o de minuto só passaria a cada segundo na virada do próximo minuto).
        if Self::clock_every_second(&self.config) && !Self::clock_every_second(&old) {
            self.loop_handle
                .insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)), |_, _, data| {
                    data.state.dirty = true;
                    if Self::clock_every_second(&data.state.config) {
                        smithay::reexports::calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
                    } else {
                        smithay::reexports::calloop::timer::TimeoutAction::Drop
                    }
                })
                .ok();
        }
        tracing::info!("configuração recarregada");
        true
    }

    /// Muda o Acesso Universal pelas teclas: vale na hora e fica salvo.
    pub fn change_access(&mut self, f: impl FnOnce(&mut glove_config::Access)) {
        f(&mut self.config.access);
        let mut c = Config::load();
        c.access = self.config.access.clone();
        if let Err(e) = c.save() {
            tracing::warn!("não salvou o acesso universal: {e}");
        }
        self.dirty = true;
    }

    /// O relógio da barra precisa mudar a cada segundo?
    pub fn clock_every_second(c: &Config) -> bool {
        c.clock.show && !c.clock.analog && (c.clock.seconds || c.clock.blink)
    }

    /// Seta do cursor na escala da tela (refeita só quando a escala muda).
    pub fn prepare_cursor(&mut self) {
        // Escala da tela × tamanho do cursor (Acesso Universal).
        let k = self.config.access.cursor_size.clamp(1.0, 4.0) as f64;
        let key = self.scale() * k;
        if self.cursor_tex.is_none() || self.cursor_tex_scale != key {
            self.cursor_tex = glove_paint::chrome::cursor_arrow(key as f32).map(|c| {
                let mut t = crate::shell::to_tex(c);
                t.size = t.size.to_f64().upscale(k).to_i32_round();
                t
            });
            for (i, angle) in [0.0, 90.0, 45.0, -45.0].into_iter().enumerate() {
                self.resize_cursors[i] = glove_paint::chrome::cursor_resize(angle, key as f32).map(|c| {
                    let mut t = crate::shell::to_tex(c);
                    t.size = t.size.to_f64().upscale(k).to_i32_round();
                    t
                });
            }
            self.cursor_tex_scale = key;
        }
    }

    /// A seta de redimensionar a mostrar agora (índice em `resize_cursors`), se houver.
    pub fn resize_cursor(&self) -> Option<usize> {
        use crate::grabs::ResizeEdge as E;
        let e = self.resize_active.or(self.resize_hover)?;
        let (h, v) = (e.intersects(E::LEFT | E::RIGHT), e.intersects(E::TOP | E::BOTTOM));
        Some(match (h, v) {
            (true, false) => 0,
            (false, true) => 1,
            // ⤡: canto de cima à esquerda ou de baixo à direita.
            (true, true) if e.contains(E::TOP) == e.contains(E::LEFT) => 2,
            (true, true) => 3,
            _ => return None,
        })
    }

    /// Depois de trocar de resolução, o ponteiro volta para dentro da tela.
    pub fn keep_pointer_inside(&mut self) {
        self.pointer_pos = self.clamp_to_outputs(self.pointer_pos);
    }

    /// O monitor principal (barra de menu e Dock).
    pub fn primary(&self) -> Option<&smithay::output::Output> {
        let mut outs = self.space.outputs();
        match &self.primary_output {
            Some(n) => self.space.outputs().find(|o| o.name() == *n).or_else(|| outs.next()),
            None => outs.next(),
        }
    }

    /// Retângulo (lógico) do monitor em que está o ponto.
    pub fn output_rect_at(&self, p: Point<f64, Logical>) -> Option<smithay::utils::Rectangle<i32, Logical>> {
        self.space.outputs().filter_map(|o| self.space.output_geometry(o)).find(|g| g.to_f64().contains(p))
    }

    /// O ponto, trazido para dentro de algum monitor (o mais perto).
    pub fn clamp_to_outputs(&self, p: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.output_rect_at(p).is_some() {
            return p;
        }
        let rects: Vec<_> = self.space.outputs().filter_map(|o| self.space.output_geometry(o)).collect();
        if rects.is_empty() {
            let (w, h) = self.output_size();
            return (p.x.clamp(0.0, w as f64 - 1.0), p.y.clamp(0.0, h as f64 - 1.0)).into();
        }
        rects
            .iter()
            .map(|g| {
                let g = g.to_f64();
                let q: Point<f64, Logical> = (p.x.clamp(g.loc.x, g.loc.x + g.size.w - 1.0), p.y.clamp(g.loc.y, g.loc.y + g.size.h - 1.0)).into();
                let d = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
                (d, q)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, q)| q)
            .unwrap_or(p)
    }

    /// Janelas que ficaram fora de todos os monitores (um saiu do cabo) voltam ao principal.
    pub fn rescue_windows(&mut self) {
        let rects: Vec<_> = self.space.outputs().filter_map(|o| self.space.output_geometry(o)).collect();
        if rects.is_empty() {
            return;
        }
        let area = self.work_area();
        let lost: Vec<Window> = self
            .space
            .elements()
            .filter(|w| self.space.element_geometry(w).is_some_and(|g| !rects.iter().any(|r| r.overlaps(g))))
            .cloned()
            .collect();
        for (i, w) in lost.iter().enumerate() {
            let top = crate::shell::deco_top(w);
            let off = (i as i32 % 6) * 26;
            self.space.map_element(w.clone(), (area.loc.x + 40 + off, area.loc.y + top + 20 + off), false);
            self.sync_x11(w);
        }
        if !lost.is_empty() {
            self.dirty = true;
        }
    }

    /// Dentro de outro desktop: a "tela" é a janela do Glove.
    pub fn write_outputs_info_nested(&self, output: &smithay::output::Output) {
        let mode = output.current_mode().map(|m| format!("{}x{}@{}", m.size.w, m.size.h, (m.refresh + 500) / 1000)).unwrap_or_default();
        let scale = output.current_scale().fractional_scale();
        glove_config::OutputsInfo {
            nested: true,
            outputs: vec![glove_config::OutputInfo {
                name: output.name(),
                description: tr!("Janela do Glove").into(),
                size_mm: (0, 0),
                active: true,
                primary: true,
                current: mode.clone(),
                preferred: mode.clone(),
                modes: vec![mode],
                scale,
                auto_scale: scale,
            }],
        }
        .save();
    }

    /// Layout e repetição do teclado.
    pub fn apply_keyboard(&mut self) {
        let k = self.config.keyboard.clone();
        let keyboard = self.seat.get_keyboard().unwrap();
        let xkb = smithay::input::keyboard::XkbConfig { layout: &k.layout, variant: &k.variant, ..Default::default() };
        if let Err(e) = keyboard.set_xkb_config(self, xkb) {
            tracing::warn!("layout de teclado inválido ({}): {e:?}", k.layout);
        }
        keyboard.change_repeat_info(k.repeat_rate as i32, k.repeat_delay as i32);
    }

    /// Abre um programa conectado a este compositor.
    pub fn spawn(&self, command: &str) {
        // Programas do Glove instalados ao lado do executável principal.
        let first = command.split_whitespace().next().unwrap_or("");
        let command = if first.starts_with("glove-") {
            command.replacen(first, &crate::shell::sibling_binary(first), 1)
        } else {
            command.to_string()
        };
        let _ = self.run_command(&command, false);
    }

    /// Abre um programa só (sem `&&`) e devolve o pid dele (o `sh` vira o programa).
    pub fn spawn_child(&self, command: &str) -> Option<u32> {
        let first = command.split_whitespace().next().unwrap_or("");
        let command = if first.starts_with("glove-") {
            command.replacen(first, &crate::shell::sibling_binary(first), 1)
        } else {
            command.to_string()
        };
        self.run_command(&command, true)
    }

    fn run_command(&self, command: &str, exec: bool) -> Option<u32> {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c")
            .arg(if exec { format!("exec {command}") } else { command.to_string() })
            .env("WAYLAND_DISPLAY", &self.socket_name)
            // Wayland primeiro; sem suporte (ou sem o plugin do Qt), cai no Xwayland.
            .env("GDK_BACKEND", "wayland,x11")
            .env("QT_QPA_PLATFORM", "wayland;xcb")
            .env("SDL_VIDEODRIVER", "wayland,x11")
            .env("MOZ_ENABLE_WAYLAND", "1")
            .env("GLOVE_MENU_SOCKET", self.menu_socket.clone().unwrap_or_default());
        // Apps X11 vão para o Xwayland do Glove (nunca para o X de fora, na janela aninhada).
        match self.xdisplay {
            Some(n) => cmd.env("DISPLAY", format!(":{n}")),
            None => cmd.env_remove("DISPLAY"),
        };
        match cmd.spawn() {
            Ok(c) => Some(c.id()),
            Err(e) => {
                tracing::warn!("não consegui abrir '{command}': {e}");
                None
            }
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

/// Sessão própria: os serviços do usuário (portais, chaveiro…) que o systemd e o
/// D-Bus abrem sob demanda passam a conhecer esta tela. Sem isso o portal GTK sai
/// com "cannot open display" e cada app GTK/Electron espera ~25 s ao abrir.
pub fn export_session_env(wayland: &std::ffi::OsStr, xdisplay: Option<u32>) {
    let mut vars = vec![format!("WAYLAND_DISPLAY={}", wayland.to_string_lossy()), "XDG_CURRENT_DESKTOP=Glove".into(), "XDG_SESSION_TYPE=wayland".into(), "XDG_SESSION_DESKTOP=glove".into()];
    if let Some(n) = xdisplay {
        vars.push(format!("DISPLAY=:{n}"));
    }
    // Portais: os do sistema mais o do Glove (escolher arquivo com a janela do Glove).
    if let Some(dir) = setup_portal_dir() {
        vars.push(format!("XDG_DESKTOP_PORTAL_DIR={}", dir.display()));
    }
    let first = xdisplay.is_none();
    std::thread::spawn(move || {
        let ok = std::process::Command::new("dbus-update-activation-environment").arg("--systemd").args(&vars).status().is_ok_and(|s| s.success());
        if !ok {
            let names: Vec<&str> = vars.iter().filter_map(|v| v.split('=').next()).collect();
            let _ = std::process::Command::new("systemctl").args(["--user", "import-environment"]).args(&names).status();
        }
        if first {
            write_portals_conf();
            // Portais de uma sessão anterior (o systemd do usuário continua de pé
            // entre logins) guardam a tela velha: saem e voltam sob demanda.
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "stop", "xdg-desktop-portal.service", "xdg-desktop-portal-gtk.service", "xdg-desktop-portal-gnome.service"])
                .stderr(std::process::Stdio::null())
                .status();
            let _ = std::process::Command::new("systemctl").args(["--user", "reset-failed"]).stderr(std::process::Stdio::null()).status();
        }
    });
}

/// Pasta de portais da sessão (~/.local/share/glove/portais): atalhos para os do
/// sistema e o do Glove (`glove.portal`), e o serviço do D-Bus que inicia o
/// `glove-portal` sob demanda. O xdg-desktop-portal lê essa pasta pela variável
/// XDG_DESKTOP_PORTAL_DIR (sem precisar instalar nada no sistema).
fn setup_portal_dir() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?.parent()?.join("glove-portal");
    if !exe.is_file() {
        return None;
    }
    let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("glove/portais");
    std::fs::create_dir_all(&dir).ok()?;
    let system = std::path::Path::new("/usr/share/xdg-desktop-portal/portals");
    if let Ok(rd) = std::fs::read_dir(system) {
        for e in rd.flatten() {
            let link = dir.join(e.file_name());
            if e.file_name() != "glove.portal" && std::fs::symlink_metadata(&link).is_err() {
                let _ = std::os::unix::fs::symlink(e.path(), link);
            }
        }
    }
    let _ = std::fs::write(dir.join("glove.portal"), "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.glove\nInterfaces=org.freedesktop.impl.portal.FileChooser;\nUseIn=Glove\n");
    let services = data.join("dbus-1/services");
    std::fs::create_dir_all(&services).ok()?;
    let _ = std::fs::write(
        services.join("org.freedesktop.impl.portal.desktop.glove.service"),
        format!("[D-BUS Service]\nName=org.freedesktop.impl.portal.desktop.glove\nExec={}\n", exe.display()),
    );
    Some(dir)
}

/// Quais portais o Glove usa (`glove-portals.conf`): o Glove para escolher arquivo,
/// o do GTK para o resto (escolher app…) e o chaveiro do GNOME para senhas.
fn write_portals_conf() {
    let Some(base) = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))) else { return };
    let dir = base.join("xdg-desktop-portal");
    let file = dir.join("glove-portals.conf");
    // O arquivo é do usuário se ele editou (sem a marca do Glove ou já com o portal novo).
    if let Ok(old) = std::fs::read_to_string(&file) {
        if !old.starts_with("# Gerado pelo Glove") || old.contains("FileChooser") {
            return;
        }
    }
    let portals = std::path::Path::new("/usr/share/xdg-desktop-portal/portals");
    let mut text = String::from("# Gerado pelo Glove. Pode editar: ele não grava por cima.\n[preferred]\n");
    text.push_str(if portals.join("gtk.portal").exists() { "default=gtk;\n" } else { "default=*;\n" });
    text.push_str("org.freedesktop.impl.portal.FileChooser=glove;gtk;\n");
    if portals.join("gnome-keyring.portal").exists() {
        text.push_str("org.freedesktop.impl.portal.Secret=gnome-keyring;\n");
    }
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(file, text);
    }
}
