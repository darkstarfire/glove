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
    /// A seta padrão, desenhada na escala atual.
    pub cursor_tex: Option<crate::shell::Tex>,
    pub cursor_tex_scale: f64,
    /// Ctrl+Alt+F1…F12 na sessão própria: trocar para esse console.
    pub vt_request: Option<i32>,
    /// Fim de sessão em andamento.
    pub ending: Option<crate::ending::Ending>,
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

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(Default::default(), 300, 30).expect("teclado");
        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let config = Config::load();
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
            dirty: true,
            nested: false,
            cascade: 0,
            suppressed_keys: Vec::new(),
            pointer_pos: (0.0, 0.0).into(),
            screenshot_request: None,
            open_after_shot: false,
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            cursor_tex: None,
            cursor_tex_scale: 0.0,
            ending: None,
            vt_request: None,
            menu_clients: Vec::new(),
            menu_seq: 0,
            menu_socket: None,
            recent: crate::recent::Recent::load(),
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
                let loc = self.space.element_location(&window)? - window.geometry().loc;
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
            format!("Captura de tela {}-{:02}-{:02} às {:02}.{:02}.{:02}.png", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
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
        if old.theme != self.config.theme || old.wallpaper != self.config.wallpaper {
            self.theme = Theme::new(Variant::from_config(&self.config.theme));
            self.theme_gen += 1;
        }
        if old.keyboard != self.config.keyboard {
            self.apply_keyboard();
        }
        self.bindings = crate::input::parse_bindings(&self.config);
        self.dirty = true;
        tracing::info!("configuração recarregada");
        true
    }

    /// Seta do cursor na escala da tela (refeita só quando a escala muda).
    pub fn prepare_cursor(&mut self) {
        let sc = self.scale();
        if self.cursor_tex.is_none() || self.cursor_tex_scale != sc {
            self.cursor_tex = glove_paint::chrome::cursor_arrow(sc as f32).map(crate::shell::to_tex);
            self.cursor_tex_scale = sc;
        }
    }

    /// Depois de trocar de resolução, o ponteiro volta para dentro da tela.
    pub fn keep_pointer_inside(&mut self) {
        let (w, h) = self.output_size();
        self.pointer_pos.x = self.pointer_pos.x.clamp(0.0, w as f64 - 1.0);
        self.pointer_pos.y = self.pointer_pos.y.clamp(0.0, h as f64 - 1.0);
    }

    /// Dentro de outro desktop: a "tela" é a janela do Glove.
    pub fn write_outputs_info_nested(&self, output: &smithay::output::Output) {
        let mode = output.current_mode().map(|m| format!("{}x{}@{}", m.size.w, m.size.h, (m.refresh + 500) / 1000)).unwrap_or_default();
        let scale = output.current_scale().fractional_scale();
        glove_config::OutputsInfo {
            nested: true,
            outputs: vec![glove_config::OutputInfo {
                name: output.name(),
                description: "Janela do Glove".into(),
                size_mm: (0, 0),
                active: true,
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
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&command)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("MOZ_ENABLE_WAYLAND", "1")
            .env("GLOVE_MENU_SOCKET", self.menu_socket.clone().unwrap_or_default())
            .env_remove("DISPLAY");
        if let Err(e) = cmd.spawn() {
            tracing::warn!("não consegui abrir '{command}': {e}");
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
