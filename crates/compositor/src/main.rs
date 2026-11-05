//! Glove: ambiente gráfico leve com visual da era "gel".

mod grabs;
mod handlers;
mod input;
mod appmenu;
mod desk;
mod dockmenu;
mod ending;
mod expose;
mod lock;
mod extras;
mod forcequit;
mod menus;
mod minimize;
mod recent;
mod render;
mod shell;
mod search;
mod region;
mod saver;
mod startup;
mod state;
mod switcher;
mod udev;
mod winit;

use std::time::Duration;

use smithay::{
    backend::{renderer::{damage::OutputDamageTracker, gles::GlesRenderer}, winit::WinitGraphicsBackend},
    output::Output,
    utils::IsAlive,
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        wayland_server::{Display, DisplayHandle},
    },
};

pub use state::Glove;

pub struct CalloopData {
    pub state: Glove,
    pub display_handle: DisplayHandle,
    pub backend: Backend,
}

/// Onde o Glove desenha: numa janela dentro de outro desktop, ou direto na tela.
pub enum Backend {
    Winit(WinitBackend),
    Drm(Box<udev::Drm>),
}

pub struct WinitBackend {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub output: Output,
    pub damage_tracker: OutputDamageTracker,
}

/// Sessão própria (direto na tela) ou janela dentro do desktop atual.
fn want_drm() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--janela" || a == "--winit") {
        return false;
    }
    if args.iter().any(|a| a == "--sessao" || a == "--drm") {
        return true;
    }
    match std::env::var("GLOVE_BACKEND").as_deref() {
        Ok("drm") => true,
        Ok("winit") => false,
        // Sem outro desktop por baixo: é a sessão própria.
        _ => std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none(),
    }
}

/// Segundos até a virada do próximo minuto (para o relógio).
fn secs_to_next_minute() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    60 - (now % 60)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter("glove=info,smithay=warn").init();
    }

    glove_config::tune_memory();

    if std::env::args().any(|a| a == "--monitores") {
        return udev::probe(&glove_config::Config::load());
    }


    // Tela de login (greetd): quiosque, com a configuração do sistema.
    let greeter = std::env::args().any(|a| a == "--greeter");
    if greeter {
        // Seguro: ainda não há outras threads.
        unsafe { std::env::set_var("GLOVE_CONFIG_DIR", std::env::var_os("GLOVE_CONFIG_DIR").unwrap_or_else(|| "/etc/glove/login".into())) };
    }

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;
    let display: Display<Glove> = Display::new()?;
    let display_handle = display.handle();
    let mut state = Glove::new(&mut event_loop, display);

    let backend = if want_drm() {
        Backend::Drm(udev::init(&mut event_loop, &mut state, &display_handle)?)
    } else {
        let (backend, output) = winit::init(&mut event_loop, &mut state, &display_handle)?;
        let damage_tracker = OutputDamageTracker::from_output(&output);
        state.write_outputs_info_nested(&output);
        Backend::Winit(WinitBackend { backend, output, damage_tracker })
    };
    // Os apps abertos pela sessão falam com este compositor.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    }

    tracing::info!("Glove rodando em WAYLAND_DISPLAY={}", state.socket_name.to_string_lossy());
    if !greeter && matches!(backend, Backend::Drm(_)) {
        state::export_session_env(&state.socket_name, None);
    }
    // Visualizador, Editor, Vídeo… em "Abrir com" (fora da tela de login).
    if !greeter {
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            glove_config::register_file_apps(&dir);
        }
    }
    // Apps X11 (Xwayland), se instalado e ligado na configuração.
    state.start_xwayland();

    state.refresh_extras();
    state.start_idle_timer();
    // Mesa: confere a cada segundo os discos montados (lê /proc/mounts) e se a
    // pasta Mesa mudou (só a data dela; a pasta só é relida quando muda).
    state.refresh_desk_items();
    event_loop.handle().insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, data| {
        data.state.refresh_desk_items();
        TimeoutAction::ToDuration(Duration::from_secs(1))
    })?;

    // Relógio: acorda só uma vez por minuto.
    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(secs_to_next_minute())), |_, _, data| {
            data.state.dirty = true;
            // Bateria, volume e Wi-Fi mudam devagar (ou por fora): relidos uma vez por minuto.
            let left = secs_to_next_minute();
            if left >= 59 || data.state.config.clock.analog || !(data.state.config.clock.seconds || data.state.config.clock.blink) {
                data.state.refresh_extras();
            }
            // Com segundos ou dois-pontos piscando, o relógio acorda a cada segundo.
            let c = &data.state.config.clock;
            let every_second = c.show && !c.analog && (c.seconds || c.blink);
            TimeoutAction::ToDuration(Duration::from_secs(if every_second { 1 } else { left }))
        })?;

    // Modo de depuração (GLOVE_DEBUG_CTL=1): lê comandos de $XDG_RUNTIME_DIR/glove-ctl.
    // Comandos: "shot CAMINHO", "move X Y", "click", "down", "up". Desligado por padrão.
    if std::env::var_os("GLOVE_DEBUG_CTL").is_some() {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            let ctl = std::path::PathBuf::from(dir).join("glove-ctl");
            event_loop.handle().insert_source(Timer::from_duration(Duration::from_millis(100)), move |_, _, data| {
                if let Ok(text) = std::fs::read_to_string(&ctl) {
                    let _ = std::fs::remove_file(&ctl);
                    for line in text.lines() {
                        data.state.debug_command(line.trim());
                    }
                }
                TimeoutAction::ToDuration(Duration::from_millis(100))
            })?;
        }
    }

    // Recarrega a configuração quando as Preferências salvam (inotify: zero CPU parado).
    if let Some(fd) = glove_config::watch_fd() {
        event_loop.handle().insert_source(
            smithay::reexports::calloop::generic::Generic::new(fd, smithay::reexports::calloop::Interest::READ, smithay::reexports::calloop::Mode::Level),
            |_, fd, data| {
                if glove_config::drain_watch(fd) {
                    if data.state.reload_config() {
                        udev::config_changed(data);
                    }
                }
                Ok(smithay::reexports::calloop::PostAction::Continue)
            },
        )?;
    }
    state.apply_keyboard();

    // Menus dos apps na barra global.
    if let Some(path) = menus::socket_path(&state.socket_name) {
        match menus::listen(&event_loop.handle(), &path) {
            Ok(()) => state.menu_socket = Some(path),
            Err(e) => tracing::warn!("socket de menus: {e}"),
        }
    }

    // Itens de início (-c): abrem no último passo do painel "Iniciando sessão".
    let mut args = std::env::args().skip(1);
    let mut commands = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "-c" || arg == "--command" {
            if let Some(cmd) = args.next() {
                commands.push(cmd);
            }
        }
    }
    if greeter {
        // Só o app de login (ou o comando dado), e o Glove termina junto com ele.
        state.greeter = true;
        let cmd = commands.first().cloned().unwrap_or_else(|| "glove-login".into());
        state.greeter_child = state.spawn_child(&cmd);
        if state.greeter_child.is_none() {
            tracing::error!("modo login: não consegui abrir '{cmd}'");
            return Ok(());
        }
    } else {
        state.start_session_panel(commands);
    }

    let mut data = CalloopData { state, display_handle, backend };
    if matches!(data.backend, Backend::Drm(_)) {
        udev::render_drm(&mut data);
    }

    event_loop.run(None, &mut data, |data| {
        // Recolhe apps que terminaram (sem zumbis). Não usamos SIGCHLD = SIG_IGN:
        // ele impede esperar por comandos (wpctl, systemctl) e passaria para os apps.
        loop {
            let pid = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            // Tela de login: o app de login terminou (sessão iniciada ou erro).
            if data.state.greeter_child == Some(pid as u32) {
                tracing::info!("app de login terminou");
                data.state.loop_signal.stop();
            }
        }
        data.state.space.refresh();
        data.state.popups.cleanup();
        data.state.minimized.retain(|(w, _)| w.alive());
        data.state.hidden.retain(|(w, _)| w.alive());
        let _ = data.display_handle.flush_clients();
        // Só redesenha quando algo mudou: sem trabalho, sem CPU.
        if let Some(vt) = data.state.vt_request.take() {
            if let Backend::Drm(drm) = &mut data.backend {
                use smithay::backend::session::Session;
                if let Err(e) = drm.session.change_vt(vt) {
                    tracing::warn!("troca para o console {vt}: {e}");
                }
            }
        }
        if data.state.dirty {
            match &mut data.backend {
                Backend::Winit(w) => w.backend.window().request_redraw(),
                Backend::Drm(_) => udev::render_drm(data),
            }
        }
    })?;

    Ok(())
}
