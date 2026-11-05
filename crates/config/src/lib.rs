//! Configuração do Glove (~/.config/glove/config.toml), compartilhada entre
//! o compositor e os apps (Preferências, etc.).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod inicio;
pub mod rede;
pub mod sons;

/// Relógio da barra de menu (painel Data e Hora, aba Relógio).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub show: bool,
    /// Mostrador de ponteiros em vez dos números.
    pub analog: bool,
    pub seconds: bool,
    pub weekday: bool,
    /// Data curta ("5 nov") antes da hora.
    pub date: bool,
    pub h24: bool,
    /// Os dois-pontos piscam a cada segundo.
    pub blink: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self { show: true, analog: false, seconds: false, weekday: true, date: false, h24: true, blink: false }
    }
}

impl Clock {
    /// Texto do relógio para (dia da semana 0=domingo, dia, mês 1-12, hora, minuto, segundo).
    pub fn text(&self, wday: u32, day: u32, month: u32, h: u32, m: u32, s: u32) -> String {
        const DIAS: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
        const MESES: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
        let colon = if self.blink && s % 2 == 1 { " " } else { ":" };
        let (hh, suffix) = if self.h24 {
            (format!("{h:02}"), "")
        } else {
            let h12 = if h % 12 == 0 { 12 } else { h % 12 };
            (h12.to_string(), if h < 12 { " AM" } else { " PM" })
        };
        let mut t = String::new();
        if self.weekday {
            t.push_str(DIAS[(wday % 7) as usize]);
            t.push(' ');
        }
        if self.date {
            t.push_str(&format!("{day} {} ", MESES[((month + 11) % 12) as usize]));
        }
        t.push_str(&format!("{hh}{colon}{m:02}"));
        if self.seconds {
            t.push_str(&format!("{colon}{s:02}"));
        }
        t.push_str(suffix);
        t
    }
}

/// Protetor de tela.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saver {
    /// Qual (ver `glove_paint::saver::SAVERS`).
    pub kind: String,
    /// Minutos parado até começar; 0 = nunca.
    pub minutes: u32,
}

impl Default for Saver {
    fn default() -> Self {
        Self { kind: "aurora".into(), minutes: 5 }
    }
}

/// Acesso Universal (feito pelo compositor).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Access {
    /// Zoom da tela inteira, seguindo o mouse.
    pub zoom: bool,
    /// Quanto o zoom aproxima (1,5 a 8).
    pub zoom_level: f32,
    /// Branco no preto.
    pub invert: bool,
    /// Escala de cinza.
    pub grayscale: bool,
    /// Contraste a mais, de 0 (normal) a 1 (máximo).
    pub contrast: f32,
    /// Tamanho do cursor (1 = normal, até 4).
    pub cursor_size: f32,
}

impl Default for Access {
    fn default() -> Self {
        Self { zoom: false, zoom_level: 2.0, invert: false, grayscale: false, contrast: 0.0, cursor_size: 1.0 }
    }
}

impl Access {
    /// Algum efeito de tela ligado (precisa do passo extra de desenho)?
    pub fn screen_effects(&self) -> bool {
        (self.zoom && self.zoom_level > 1.01) || self.invert || self.grayscale || self.contrast > 0.01
    }
}

/// Painel Som.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sound {
    /// Nome do alerta (ver `sons::ALERTS`).
    pub alert: String,
    /// Volume do alerta, de 0 a 1.
    pub alert_volume: f32,
    /// "Pop" ao mudar o volume pelas teclas.
    pub volume_feedback: bool,
    /// Volume na barra de menu.
    pub menu_extra: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self { alert: "Vidro".into(), alert_volume: 0.7, volume_feedback: true, menu_extra: true }
    }
}

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
    /// Ícone pula enquanto o app abre.
    pub animate_launch: bool,
    /// Efeito de minimizar: "escala" (o único por enquanto).
    pub minimize_effect: String,
}

impl Default for Dock {
    fn default() -> Self {
        Self { size: 56, magnification: true, max_size: 80, position: "embaixo".into(), autohide: false, animate_launch: true, minimize_effect: "escala".into() }
    }
}

/// Layouts de teclado conhecidos: (layout, variante, nome, sigla da barra).
pub const KEYBOARD_LAYOUTS: [(&str, &str, &str, &str); 10] = [
    ("br", "", "Português (Brasil, ABNT2)", "BR"),
    ("us", "", "Inglês (EUA)", "US"),
    ("us", "intl", "Inglês (EUA, internacional)", "US+"),
    ("pt", "", "Português (Portugal)", "PT"),
    ("latam", "", "Espanhol (América Latina)", "LA"),
    ("es", "", "Espanhol (Espanha)", "ES"),
    ("gb", "", "Inglês (Reino Unido)", "GB"),
    ("fr", "", "Francês", "FR"),
    ("de", "", "Alemão", "DE"),
    ("it", "", "Italiano", "IT"),
];

/// "us:intl" → ("us", "intl").
pub fn split_layout(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or((id, ""))
}

/// ("us", "intl") → "us:intl".
pub fn layout_id(layout: &str, variant: &str) -> String {
    if variant.is_empty() { layout.to_string() } else { format!("{layout}:{variant}") }
}

/// Sigla curta para a barra ("BR", "US+").
pub fn layout_badge(layout: &str, variant: &str) -> String {
    KEYBOARD_LAYOUTS.iter().find(|l| l.0 == layout && l.1 == variant).map(|l| l.3.to_string()).unwrap_or_else(|| layout.to_uppercase())
}

/// O próximo layout do Menu de Entrada depois do atual.
pub fn next_layout(k: &Keyboard) -> Option<(String, String)> {
    if k.input_menu.len() < 2 {
        return None;
    }
    let cur = layout_id(&k.layout, &k.variant);
    let i = k.input_menu.iter().position(|l| *l == cur).map_or(0, |i| (i + 1) % k.input_menu.len());
    let (l, v) = split_layout(&k.input_menu[i]);
    Some((l.to_string(), v.to_string()))
}

/// Painel Aparência.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Cor do texto selecionado (ver `HIGHLIGHTS`).
    pub highlight: String,
    /// Clique duplo na barra de título minimiza a janela.
    pub double_click_minimizes: bool,
    /// Clique no trilho da barra de rolagem vai até ali (em vez de uma página).
    pub scroll_jump: bool,
    /// Quantos itens em Itens Recentes (apps e documentos).
    pub recent_apps: u32,
    pub recent_docs: u32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { highlight: "azul".into(), double_click_minimizes: false, scroll_jump: false, recent_apps: 10, recent_docs: 10 }
    }
}

/// Cores de destaque do texto selecionado: (id, nome, r, g, b).
pub const HIGHLIGHTS: [(&str, &str, u8, u8, u8); 8] = [
    ("azul", "Azul", 181, 213, 255),
    ("grafite", "Grafite", 199, 208, 219),
    ("prata", "Prata", 214, 214, 220),
    ("dourado", "Dourado", 252, 236, 152),
    ("laranja", "Laranja", 255, 210, 160),
    ("roxo", "Roxo", 226, 200, 245),
    ("vermelho", "Vermelho", 255, 190, 190),
    ("verde", "Verde", 196, 240, 180),
];

/// Cor de destaque pelo id (azul se não conhecer).
pub fn highlight_rgb(id: &str) -> (u8, u8, u8) {
    let h = HIGHLIGHTS.iter().find(|h| h.0 == id).unwrap_or(&HIGHLIGHTS[0]);
    (h.2, h.3, h.4)
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
    /// Na tomada (ou sem bateria): minutos parado até apagar a tela; 0 = nunca.
    pub screen_off_minutes: u32,
    /// Na tomada: minutos parado até o computador entrar em repouso; 0 = nunca.
    pub sleep_minutes: u32,
    /// Na bateria: o mesmo, valores próprios.
    pub battery_screen_off_minutes: u32,
    pub battery_sleep_minutes: u32,
    /// Pedir a senha ao voltar (tela apagada, repouso ou tampa fechada).
    pub require_password: bool,
    /// Diminuir um pouco o brilho ao passar para a bateria.
    pub dim_on_battery: bool,
    /// Bateria na barra de menu.
    pub battery_menu: bool,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            screen_off_minutes: 10,
            sleep_minutes: 0,
            battery_screen_off_minutes: 5,
            battery_sleep_minutes: 15,
            require_password: true,
            dim_on_battery: true,
            battery_menu: true,
        }
    }
}

impl Energy {
    /// (minutos até apagar a tela, minutos até o repouso) para a fonte de energia.
    pub fn timeouts(&self, on_battery: bool) -> (u32, u32) {
        if on_battery {
            (self.battery_screen_off_minutes, self.battery_sleep_minutes)
        } else {
            (self.screen_off_minutes, self.sleep_minutes)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub layout: String,
    pub variant: String,
    /// Layouts do Menu de Entrada ("br", "us:intl"…); a tecla de troca passa por eles.
    pub input_menu: Vec<String>,
    /// Menu de Entrada na barra de menu.
    pub show_input_menu: bool,
    /// Atraso antes de repetir, em ms.
    pub repeat_delay: u32,
    /// Repetições por segundo.
    pub repeat_rate: u32,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { layout: "br".into(), variant: String::new(), input_menu: vec!["br".into(), "us".into()], show_input_menu: false, repeat_delay: 300, repeat_rate: 30 }
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
    /// Multiplica a rolagem (0,25 a 4).
    pub scroll_speed: f64,
    /// Tempo máximo entre os cliques de um clique duplo, em ms (mouse e trackpad).
    pub double_click_ms: u32,
}

impl Default for Pointer {
    fn default() -> Self {
        Self { natural_scroll: false, speed: 0.0, acceleration: true, left_handed: false, scroll_speed: 1.0, double_click_ms: 400 }
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
    /// Tocar duas vezes e arrastar.
    pub tap_drag: bool,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self { tap_to_click: true, natural_scroll: true, speed: 0.0, disable_while_typing: true, two_finger_right_click: true, tap_drag: true }
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
    /// Menu de monitores (brilho, resolução) na barra de menu.
    pub menu_extra: bool,
}

impl Default for Monitor {
    fn default() -> Self {
        Self { output: String::new(), mode: String::new(), scale: 0.0, menu_extra: true }
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
    ("screenshot_area", "Captura de uma área"),
    ("force_quit", "Forçar Encerrar"),
    ("lock", "Bloquear a tela"),
    ("search", "Busca Rápida"),
    ("input_next", "Próximo layout de teclado"),
    ("zoom_toggle", "Zoom: ligar e desligar"),
    ("zoom_in", "Zoom: aproximar"),
    ("zoom_out", "Zoom: afastar"),
    ("invert_colors", "Inverter as cores"),
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
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub sound: Sound,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub saver: Saver,
    /// Navegador do glove-navegador ("" = pela memória: NetSurf ou Firefox).
    #[serde(default)]
    pub navegador: String,
    /// Ao conectar um pendrive ou disco: "abrir" (no Arquivos) ou "nada".
    #[serde(default = "default_removable")]
    pub removable_action: String,
    /// Menu Wi-Fi na barra de menu.
    #[serde(default = "yes")]
    pub wifi_menu: bool,
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
    /// O assistente de primeiro uso já foi visto. Configurações antigas (sem o
    /// campo) contam como vistas; só instalações novas abrem o assistente.
    #[serde(default = "yes")]
    pub welcome_done: bool,
    /// Porcentagem ao lado do ícone da bateria na barra de menu.
    #[serde(default)]
    pub battery_percent: bool,
    /// Versão da lista padrão do Dock já aplicada (para acrescentar apps novos).
    #[serde(default)]
    pub dock_rev: u32,
    /// Painéis na barra das Preferências (ao lado de "Mostrar Todos").
    #[serde(default = "default_prefs_favorites")]
    pub prefs_favorites: Vec<String>,
    /// Abrir apps X11 (liga o Xwayland, uns 20 MB a mais).
    #[serde(default = "yes")]
    pub x11_apps: bool,
}

pub fn default_prefs_favorites() -> Vec<String> {
    ["monitor", "dock", "teclado"].map(String::from).to_vec()
}

/// Aumente quando a lista padrão do Dock ganhar apps novos.
pub const DOCK_REV: u32 = 6;

fn default_removable() -> String {
    "abrir".into()
}

fn yes() -> bool {
    true
}

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
        ("screenshot_area", "Shift+Print"),
        ("force_quit", "Ctrl+Alt+Escape"),
        ("lock", "Super+L"),
        ("search", "Super+space"),
        ("input_next", "Super+Alt+space"),
        ("zoom_toggle", "Super+Alt+8"),
        ("zoom_in", "Super+Alt+equal"),
        ("zoom_out", "Super+Alt+minus"),
        ("invert_colors", "Ctrl+Super+Alt+8"),
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
        app("Navegador", "globe", "glove-navegador", "glove.Navegador"),
        app("Correio", "mail", "glove-correio", "glove.Correio"),
        app("Editor de Texto", "editor", "glove-edit", "glove.TextEditor"),
        app("Lembretes", "notes", "glove-notes", "glove.Notes"),
        app("Contatos", "contacts", "glove-contatos", "glove.Contatos"),
        app("Agenda", "calendar", "glove-agenda", "glove.Agenda"),
        app("Calculadora", "calculator", "glove-calc", "glove.Calculator"),
        app("Relógio", "clock", "glove-clock", "glove.Clock"),
        app("Visualizador", "picture", "glove-view", "glove.Viewer"),
        app("Música", "music", "glove-musica", "glove.Musica"),
        app("Busca", "search", "glove-find", "glove.Find"),
        app("Caracteres", "chars", "glove-chars", "glove.Chars"),
        app("Captura", "camera", "glove-shot", "glove.Shot"),
        app("Monitor", "monitor", "glove-monitor", "glove.Monitor"),
        app("Discos", "disk", "glove-disks", "glove.Disks"),
        app("Xadrez", "chess", "glove-chess", "glove.Chess"),
        app("Preferências", "settings", "glove-prefs", "glove.Preferences"),
    ]
}

/// Apps do Glove que não ficam no Dock de saída (acham-se na Busca e na pasta Aplicativos).
pub fn extra_apps() -> Vec<DockApp> {
    vec![
        app("Vídeo", "video", "glove-video", "glove.Video"),
        app("Registros", "logs", "glove-registros", "glove.Registros"),
        app("Informações do Sistema", "info", "glove-info", "glove.Info"),
        app("Ajuda", "help", "glove-ajuda", "glove.Ajuda"),
        app("Dicionário", "dictionary", "glove-dicionario", "glove.Dicionario"),
        app("Livro de Fontes", "fontbook", "glove-fontes", "glove.Fontes"),
    ]
}

/// O app com esse app_id: o do Dock do usuário, senão um dos apps do Glove.
pub fn known_app(dock: &[DockApp], app_id: &str) -> Option<DockApp> {
    if app_id.is_empty() {
        return None;
    }
    dock.iter()
        .find(|a| a.app_id.eq_ignore_ascii_case(app_id))
        .cloned()
        .or_else(|| default_dock_apps().into_iter().chain(extra_apps()).find(|a| a.app_id.eq_ignore_ascii_case(app_id)))
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
            welcome_done: false,
            appearance: Appearance::default(),
            sound: Sound::default(),
            wifi_menu: true,
            clock: Clock::default(),
            access: Access::default(),
            saver: Saver::default(),
            removable_action: default_removable(),
            navegador: String::new(),
            dock_rev: DOCK_REV,
            prefs_favorites: default_prefs_favorites(),
            x11_apps: true,
        }
    }
}

pub fn dir() -> PathBuf {
    // A tela de login (glove --greeter) usa a configuração do sistema.
    if let Some(d) = std::env::var_os("GLOVE_CONFIG_DIR") {
        return PathBuf::from(d);
    }
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
            // O Firefox direto (perfil do Glove) virou o glove-navegador (NetSurf ou Firefox).
            if a.app_id == "firefox" && a.command.contains(".cache/glove/firefox") {
                if let Some(d) = defaults.iter().find(|d| d.app_id == "glove.Navegador") {
                    *a = d.clone();
                }
            }
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
