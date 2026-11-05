//! Configuração do Glove (~/.config/glove/config.toml), compartilhada entre
//! o compositor e os apps (Preferências, etc.).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockApp {
    pub name: String,
    pub icon: String,
    pub command: String,
    /// Usado para saber se o app já está aberto.
    #[serde(default)]
    pub app_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dock {
    pub size: u32,
    pub magnification: bool,
    pub max_size: u32,
    /// "embaixo", "esquerda" ou "direita".
    pub position: String,
    /// Some e reaparece quando o mouse encosta na borda.
    pub autohide: bool,
}

impl Default for Dock {
    fn default() -> Self {
        Self { size: 56, magnification: true, max_size: 80, position: "embaixo".into(), autohide: false }
    }
}

/// Cantos ativos: "todas", "app" ou "mesa" (Exposé); vazio = nada.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotCorners {
    pub top_left: String,
    pub top_right: String,
    pub bottom_left: String,
    pub bottom_right: String,
}

/// Economia de energia e bloqueio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Energy {
    /// Minutos parado até apagar a tela; 0 = nunca.
    pub screen_off_minutes: u32,
    /// Pedir a senha ao voltar (tela apagada, repouso ou tampa fechada).
    pub require_password: bool,
}

impl Default for Energy {
    fn default() -> Self {
        Self { screen_off_minutes: 10, require_password: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub layout: String,
    pub variant: String,
    /// Atraso antes de repetir, em ms.
    pub repeat_delay: u32,
    /// Repetições por segundo.
    pub repeat_rate: u32,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { layout: "br".into(), variant: String::new(), repeat_delay: 300, repeat_rate: 30 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pointer {
    /// Rolagem natural da roda do mouse.
    pub natural_scroll: bool,
    /// Velocidade do cursor, de -1 (lenta) a 1 (rápida).
    pub speed: f64,
    /// Aceleração: o cursor anda mais quando o mouse se move rápido.
    pub acceleration: bool,
    /// Botões trocados (canhoto).
    pub left_handed: bool,
}

impl Default for Pointer {
    fn default() -> Self {
        Self { natural_scroll: false, speed: 0.0, acceleration: true, left_handed: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Touchpad {
    pub tap_to_click: bool,
    pub natural_scroll: bool,
    /// De -1 (lenta) a 1 (rápida).
    pub speed: f64,
    /// Ignora o touchpad enquanto se digita.
    pub disable_while_typing: bool,
    /// Clique com dois dedos = botão direito (em vez de canto inferior direito).
    pub two_finger_right_click: bool,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self { tap_to_click: true, natural_scroll: true, speed: 0.0, disable_while_typing: true, two_finger_right_click: true }
    }
}

/// Monitor (só na sessão própria; dentro de outro desktop vale a janela).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Monitor {
    /// Conector preferido (ex.: "HDMI-A-1"); vazio = automático (externo, se houver).
    pub output: String,
    /// Resolução "LxA@Hz" (ex.: "1920x1080@60"); vazio = a recomendada pelo monitor.
    pub mode: String,
    /// Escala da interface; 0 = automática (pela densidade da tela).
    pub scale: f64,
}

impl Default for Monitor {
    fn default() -> Self {
        Self { output: String::new(), mode: String::new(), scale: 0.0 }
    }
}

/// Monitores conectados, escritos pelo compositor para as Preferências lerem
/// ($XDG_RUNTIME_DIR/glove-outputs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputsInfo {
    /// Rodando dentro de outro desktop.
    pub nested: bool,
    pub outputs: Vec<OutputInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    pub description: String,
    /// Tamanho físico em mm.
    pub size_mm: (u32, u32),
    pub active: bool,
    /// "LxA@Hz" em uso.
    pub current: String,
    pub preferred: String,
    pub modes: Vec<String>,
    pub scale: f64,
    pub auto_scale: f64,
}

impl OutputsInfo {
    pub fn path() -> Option<PathBuf> {
        std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("glove-outputs"))
    }

    pub fn load() -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(Self::path()?).ok()?).ok()
    }

    pub fn save(&self) {
        if let (Some(p), Ok(s)) = (Self::path(), toml::to_string(self)) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
}

/// Escala sugerida pela densidade da tela (pontos por polegada), em passos de 0,25.
pub fn auto_scale(px: (u32, u32), mm: (u32, u32)) -> f64 {
    if mm.0 < 50 || mm.1 < 50 {
        // Sem tamanho físico confiável (projetores, alguns adaptadores).
        return 1.0;
    }
    let dpi = px.0 as f64 / (mm.0 as f64 / 25.4);
    ((dpi / 135.0) * 4.0).round().clamp(4.0, 12.0) / 4.0
}

/// "1920x1080@60" -> (1920, 1080, 60).
pub fn parse_mode(s: &str) -> Option<(u32, u32, u32)> {
    let (wh, hz) = s.split_once('@').unwrap_or((s, "0"));
    let (w, h) = wh.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?, hz.trim().parse().unwrap_or(0)))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    pub action: String,
    /// Ex.: "Super+Return", "Ctrl+Alt+BackSpace".
    pub keys: String,
}

/// Ações que podem ter atalho, com o nome que aparece nas Preferências.
pub const ACTIONS: &[(&str, &str)] = &[
    ("terminal", "Abrir Terminal"),
    ("close", "Fechar janela"),
    ("minimize", "Minimizar janela"),
    ("hide", "Ocultar o app"),
    ("hide_others", "Ocultar os outros apps"),
    ("zoom", "Zoom da janela"),
    ("cycle", "Trocar de app"),
    ("expose_all", "Exposé: todas as janelas"),
    ("expose_app", "Exposé: janelas do app"),
    ("expose_desktop", "Exposé: mostrar a mesa"),
    ("preferences", "Abrir Preferências"),
    ("screenshot", "Captura de tela"),
    ("force_quit", "Forçar Encerrar"),
    ("lock", "Bloquear a tela"),
    ("search", "Busca Rápida"),
    ("quit", "Sair do Glove"),
];

pub fn action_label(id: &str) -> &str {
    ACTIONS.iter().find(|(a, _)| *a == id).map(|(_, l)| *l).unwrap_or(id)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    /// "ondas", "aurora", "liso" ou o caminho de uma imagem PNG.
    pub wallpaper: String,
    pub terminal: String,
    /// Família da fonte do Terminal (ex.: "JetBrainsMono Nerd Font"); vazio = padrão.
    pub terminal_font: String,
    pub terminal_font_size: f32,
    pub dock: Dock,
    pub keyboard: Keyboard,
    pub pointer: Pointer,
    pub touchpad: Touchpad,
    pub monitor: Monitor,
    pub shortcuts: Vec<Shortcut>,
    pub dock_apps: Vec<DockApp>,
    /// Comando para abrir a lixeira.
    pub trash_command: String,
    #[serde(default)]
    pub energy: Energy,
    #[serde(default)]
    pub hot_corners: HotCorners,
    /// Porcentagem ao lado do ícone da bateria na barra de menu.
    #[serde(default)]
    pub battery_percent: bool,
    /// Versão da lista padrão do Dock já aplicada (para acrescentar apps novos).
    #[serde(default)]
    pub dock_rev: u32,
}

/// Aumente quando a lista padrão do Dock ganhar apps novos.
pub const DOCK_REV: u32 = 2;

fn default_terminal() -> String {
    "glove-term".into()
}

fn app(name: &str, icon: &str, command: &str, app_id: &str) -> DockApp {
    DockApp { name: name.into(), icon: icon.into(), command: command.into(), app_id: app_id.into() }
}

pub fn default_shortcuts() -> Vec<Shortcut> {
    [
        ("terminal", "Super+Return"),
        ("close", "Super+Q"),
        ("minimize", "Super+M"),
        ("hide", "Super+H"),
        ("hide_others", "Super+Alt+H"),
        ("zoom", "Super+Up"),
        ("cycle", "Super+Tab"),
        ("expose_all", "F9"),
        ("expose_app", "F10"),
        ("expose_desktop", "F11"),
        ("preferences", "Super+Comma"),
        ("screenshot", "Print"),
        ("force_quit", "Ctrl+Alt+Escape"),
        ("lock", "Super+L"),
        ("search", "Super+space"),
        ("quit", "Ctrl+Alt+BackSpace"),
    ]
    .into_iter()
    .map(|(a, k)| Shortcut { action: a.into(), keys: k.into() })
    .collect()
}

pub fn default_dock_apps() -> Vec<DockApp> {
    vec![
        app("Arquivos", "folder", "glove-files", "glove.Files"),
        app("Terminal", "terminal", "glove-term", "glove.Terminal"),
        app("Navegador", "globe", "mkdir -p ~/.cache/glove/firefox && firefox --no-remote --profile ~/.cache/glove/firefox", "firefox"),
        app("Editor de Texto", "editor", "glove-edit", "glove.TextEditor"),
        app("Lembretes", "notes", "glove-notes", "glove.Notes"),
        app("Calculadora", "calculator", "glove-calc", "glove.Calculator"),
        app("Relógio", "clock", "glove-clock", "glove.Clock"),
        app("Visualizador", "picture", "glove-view", "glove.Viewer"),
        app("Busca", "search", "glove-find", "glove.Find"),
        app("Caracteres", "chars", "glove-chars", "glove.Chars"),
        app("Captura", "camera", "glove-shot", "glove.Shot"),
        app("Monitor", "monitor", "glove-monitor", "glove.Monitor"),
        app("Discos", "disk", "glove-disks", "glove.Disks"),
        app("Xadrez", "chess", "glove-chess", "glove.Chess"),
        app("Preferências", "settings", "glove-prefs", "glove.Preferences"),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "claro".into(),
            wallpaper: "ondas".into(),
            terminal: default_terminal(),
            terminal_font: String::new(),
            terminal_font_size: 12.0,
            dock: Dock::default(),
            keyboard: Keyboard::default(),
            pointer: Pointer::default(),
            touchpad: Touchpad::default(),
            monitor: Monitor::default(),
            shortcuts: default_shortcuts(),
            dock_apps: default_dock_apps(),
            trash_command: "glove-files ~/.local/share/Trash/files".into(),
            battery_percent: false,
            energy: Energy::default(),
            hot_corners: HotCorners::default(),
            dock_rev: DOCK_REV,
        }
    }
}

pub fn dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("glove")
}

pub fn path() -> PathBuf {
    dir().join("config.toml")
}

impl Config {
    /// Lê o arquivo; se não existir ou estiver inválido, usa o padrão.
    /// Na primeira execução, grava o padrão para o usuário poder editar.
    pub fn load() -> Self {
        match std::fs::read_to_string(path()) {
            Ok(s) => {
                let mut c: Config = toml::from_str(&s).unwrap_or_default();
                if c.dock_apps.is_empty() {
                    c.dock_apps = default_dock_apps();
                }
                if c.shortcuts.is_empty() {
                    c.shortcuts = default_shortcuts();
                }
                let added = c.add_missing_shortcuts();
                if c.migrate() || added {
                    let _ = c.save();
                }
                c
            }
            Err(_) => {
                let c = Self::default();
                let _ = c.save();
                c
            }
        }
    }

    /// Grava de forma atômica (arquivo temporário + renomear), para quem
    /// estiver observando a pasta nunca ler um arquivo pela metade.
    pub fn save(&self) -> std::io::Result<()> {
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = p.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, p)
    }

    /// Ações novas ganham o atalho padrão em configurações antigas.
    pub fn add_missing_shortcuts(&mut self) -> bool {
        let mut added = false;
        for d in default_shortcuts() {
            if !self.shortcuts.iter().any(|s| s.action == d.action) {
                self.shortcuts.push(d);
                added = true;
            }
        }
        added
    }

    /// Atualiza configurações antigas: troca apps do GNOME pelos do Glove e
    /// acrescenta os apps novos do Dock. Devolve verdadeiro se mudou algo.
    pub fn migrate(&mut self) -> bool {
        if self.dock_rev >= DOCK_REV {
            return false;
        }
        let defaults = default_dock_apps();
        let replace = [
            ("org.gnome.Nautilus", "glove.Files"),
            ("org.gnome.Ptyxis", "glove.Terminal"),
            ("foot", "glove.Terminal"),
            ("org.gnome.TextEditor", "glove.TextEditor"),
            ("org.gnome.Calculator", "glove.Calculator"),
        ];
        for a in &mut self.dock_apps {
            if let Some((_, new_id)) = replace.iter().find(|(old, _)| a.app_id.eq_ignore_ascii_case(old)) {
                if let Some(d) = defaults.iter().find(|d| d.app_id == *new_id) {
                    *a = d.clone();
                }
            }
        }
        // Remove duplicados criados pela troca.
        let mut seen = Vec::new();
        self.dock_apps.retain(|a| {
            let dup = !a.app_id.is_empty() && seen.contains(&a.app_id);
            seen.push(a.app_id.clone());
            !dup
        });
        for d in defaults {
            if !self.dock_apps.iter().any(|a| a.app_id == d.app_id) {
                self.dock_apps.push(d);
            }
        }
        if self.terminal.contains("ptyxis") || self.terminal == "foot" {
            self.terminal = default_terminal();
        }
        if self.trash_command.contains("nautilus") || self.trash_command.is_empty() {
            self.trash_command = "glove-files ~/.local/share/Trash/files".into();
        }
        self.dock_rev = DOCK_REV;
        true
    }

    pub fn shortcut(&self, action: &str) -> Option<&str> {
        self.shortcuts.iter().find(|s| s.action == action).map(|s| s.keys.as_str())
    }
}

/// Observa a pasta de configuração (inotify) sem acordar a CPU à toa.
/// Retorna o descritor; ele fica legível quando o arquivo muda.
pub fn watch_fd() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let d = dir();
    std::fs::create_dir_all(&d).ok()?;
    let cpath = std::ffi::CString::new(d.to_string_lossy().as_bytes()).ok()?;
    unsafe {
        let fd = libc_inotify::init();
        if fd < 0 {
            return None;
        }
        if libc_inotify::add_watch(fd, cpath.as_ptr()) < 0 {
            libc_inotify::close(fd);
            return None;
        }
        Some(std::os::fd::OwnedFd::from_raw_fd(fd))
    }
}

/// Esvazia os eventos pendentes do descritor de `watch_fd`.
/// Retorna verdadeiro se o config.toml foi tocado.
pub fn drain_watch(fd: &std::os::fd::OwnedFd) -> bool {
    use std::os::fd::AsRawFd;
    let mut buf = [0u8; 4096];
    let mut hit = false;
    loop {
        let n = unsafe { libc_inotify::read(fd.as_raw_fd(), buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        // Qualquer evento na pasta conta; o nome exato é verificado de forma simples.
        if buf[..n as usize].windows(11).any(|w| w == b"config.toml") {
            hit = true;
        }
    }
    hit
}

/// Chamadas mínimas ao inotify sem depender de outras bibliotecas.
mod libc_inotify {
    use std::os::raw::{c_char, c_int, c_void};
    const IN_NONBLOCK: c_int = 0o4000;
    const IN_CLOEXEC: c_int = 0o2000000;
    const IN_CLOSE_WRITE: u32 = 0x0000_0008;
    const IN_MOVED_TO: u32 = 0x0000_0080;
    const IN_CREATE: u32 = 0x0000_0100;
    unsafe extern "C" {
        fn inotify_init1(flags: c_int) -> c_int;
        fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;
        #[link_name = "read"]
        fn c_read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
        #[link_name = "close"]
        fn c_close(fd: c_int) -> c_int;
    }
    pub unsafe fn init() -> c_int {
        unsafe { inotify_init1(IN_NONBLOCK | IN_CLOEXEC) }
    }
    pub unsafe fn add_watch(fd: c_int, p: *const c_char) -> c_int {
        unsafe { inotify_add_watch(fd, p, IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE) }
    }
    pub unsafe fn read(fd: c_int, buf: *mut u8, n: usize) -> isize {
        unsafe { c_read(fd, buf as *mut c_void, n) }
    }
    pub unsafe fn close(fd: c_int) {
        unsafe {
            c_close(fd);
        }
    }
}

/// Faz o alocador devolver memória ao sistema assim que ela é liberada
/// (sem isso, o glibc guarda blocos grandes "para depois" e a RAM só cresce).
pub fn tune_memory() {
    unsafe {
        // Blocos acima de 64 KB vêm direto do sistema e voltam ao liberar.
        libc::mallopt(libc::M_MMAP_THRESHOLD, 64 * 1024);
        // Devolve sobras do topo do heap acima de 64 KB.
        libc::mallopt(libc::M_TRIM_THRESHOLD, 64 * 1024);
        // Uma arena só: programas do Glove são quase todos de uma thread.
        libc::mallopt(libc::M_ARENA_MAX, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migra_config_antiga() {
        let old = r#"
theme = "metal"
terminal = "ptyxis --standalone"
trash_command = "dbus-run-session -- nautilus --new-window trash:///"
[[dock_apps]]
name = "Arquivos"
icon = "folder"
command = "dbus-run-session -- nautilus --new-window"
app_id = "org.gnome.Nautilus"
[[dock_apps]]
name = "Meu app"
icon = "x"
command = "meuapp"
app_id = "meu.app"
"#;
        let mut c: Config = toml::from_str(old).unwrap();
        assert!(c.migrate());
        assert_eq!(c.theme, "metal");
        assert_eq!(c.terminal, "glove-term");
        assert_eq!(c.dock_apps[0].app_id, "glove.Files");
        assert!(c.dock_apps.iter().any(|a| a.app_id == "meu.app"));
        assert!(c.dock_apps.iter().any(|a| a.app_id == "glove.Chess"));
        assert!(!c.migrate());
    }
}

#[cfg(test)]
mod monitor_tests {
    use super::*;

    #[test]
    fn escala_automatica() {
        // Notebook 13,5" 2256x1504 (≈201 ppp) -> 1,5.
        assert_eq!(auto_scale((2256, 1504), (285, 190)), 1.5);
        // 15,6" Full HD (≈141 ppp) -> 1.
        assert_eq!(auto_scale((1920, 1080), (344, 194)), 1.0);
        // 27" 4K (≈163 ppp) -> 1,25.
        assert_eq!(auto_scale((3840, 2160), (597, 336)), 1.25);
        // Sem tamanho físico.
        assert_eq!(auto_scale((1920, 1080), (0, 0)), 1.0);
        assert_eq!(parse_mode("1920x1080@60"), Some((1920, 1080, 60)));
        assert_eq!(parse_mode("1280x800"), Some((1280, 800, 0)));
    }
}
