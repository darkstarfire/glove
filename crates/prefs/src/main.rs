//! Preferências do Glove. Cada mudança é salva no config.toml e o
//! compositor aplica na hora.

use glove_config::tr;
use std::time::{Duration, Instant};

use glove_config::KEYBOARD_LAYOUTS as LAYOUTS;

mod bluetooth;
mod compart;
mod contas;
mod datahora;
mod inicio;
mod internacional;
mod outros;
mod padrao;
mod rede;
mod som;

use glove_config::{Config, OutputsInfo};
use glove_kit::{
    paint::{
        draw::*,
        icons::{Icon, PrefKind},
        Color, Variant,
    },
    run, App, Glyph, Menu, MenuItem, Rect, Ticks, Ui, WindowOptions, SEGMENT_H, SEGMENT_W,
};

const W: u32 = 660;
const H: u32 = 480;
const TOOLBAR_H: f32 = 78.0;
/// Intervalo mínimo entre gravações da configuração enquanto se arrasta um slider.
const SAVE_EVERY: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pane {
    /// A grade com todos os painéis ("Mostrar Todos").
    Todos,
    Aparencia,
    Mesa,
    Monitor,
    Som,
    Energia,
    Rede,
    DataHora,
    Inicio,
    Acesso,
    Contas,
    Internacional,
    Compart,
    Midia,
    Padrao,
    Bluetooth,
    Atualizacao,
    Inicializacao,
    Fala,
    Impressao,
    Dock,
    Teclado,
    Mouse,
    Trackpad,
    Terminal,
    Seguranca,
    Busca,
    Expose,
}

/// Categorias da grade (como no 10.4). As vazias não aparecem.
const CATEGORIES: [&str; 4] = ["Pessoal", "Hardware", "Internet e Rede", "Sistema"];

struct PaneInfo {
    pane: Pane,
    /// Argumento do glove-prefs (e nome nos favoritos).
    id: &'static str,
    icon: Icon,
    name: &'static str,
    category: usize,
    /// Palavras que a busca também encontra.
    keywords: &'static str,
}

const PANES: [PaneInfo; 26] = [
    PaneInfo { pane: Pane::Mesa, id: "mesa", icon: Icon::Pref(PrefKind::Desktop), name: "Mesa e Protetor de Tela", category: 0, keywords: "papel de parede fundo imagem cor sólida foto protetor de tela cantos ativos aurora estrelas bolhas" },
    PaneInfo { pane: Pane::Aparencia, id: "aparencia", icon: Icon::Pref(PrefKind::Appearance), name: "Aparência", category: 0, keywords: "cor destaque tema seleção barra de rolagem grafite azul itens recentes minimizar clique duplo título" },
    PaneInfo { pane: Pane::Dock, id: "dock", icon: Icon::Pref(PrefKind::Dock), name: "Dock", category: 0, keywords: "tamanho ampliação ocultar automaticamente posição esquerda direita minimizar efeito" },
    PaneInfo { pane: Pane::Monitor, id: "monitor", icon: Icon::Display, name: "Monitores", category: 1, keywords: "tela resolução escala tamanho hz frequência monitores" },
    PaneInfo { pane: Pane::Som, id: "som", icon: Icon::Pref(PrefKind::Sound), name: "Som", category: 1, keywords: "volume mudo alto-falante fone saída entrada microfone alerta efeitos sonoros" },
    PaneInfo { pane: Pane::Energia, id: "energia", icon: Icon::Pref(PrefKind::Energy), name: "Economizador de Energia", category: 1, keywords: "repouso dormir suspender tela apagar bateria tomada adaptador brilho senha energia" },
    PaneInfo { pane: Pane::Teclado, id: "teclado", icon: Icon::Pref(PrefKind::Keyboard), name: "Teclado", category: 1, keywords: "layout idioma repetição de teclas abnt2 digitação atalhos de teclado combinação exposé captura bloquear" },
    PaneInfo { pane: Pane::Mouse, id: "mouse", icon: Icon::Mouse, name: "Mouse", category: 1, keywords: "velocidade rastreamento ponteiro rolagem clique duplo botão principal canhoto" },
    PaneInfo { pane: Pane::Trackpad, id: "trackpad", icon: Icon::Trackpad, name: "Trackpad", category: 1, keywords: "toque tocar para clicar rolagem natural dois dedos touchpad" },
    PaneInfo { pane: Pane::Rede, id: "rede", icon: Icon::Pref(PrefKind::Network), name: "Rede", category: 2, keywords: "wi-fi wifi internet sem fio ethernet cabo ip dhcp dns roteador senha conectar" },
    PaneInfo { pane: Pane::DataHora, id: "datahora", icon: Icon::Pref(PrefKind::DateTime), name: "Data e Hora", category: 3, keywords: "relógio hora fuso horário calendário ntp automático segundos 24 horas barra de menu" },
    PaneInfo { pane: Pane::Contas, id: "contas", icon: Icon::Pref(PrefKind::Accounts), name: "Contas", category: 3, keywords: "usuário conta senha nome imagem retrato foto itens de início abrir ao entrar autostart" },
    PaneInfo { pane: Pane::Internacional, id: "internacional", icon: Icon::Pref(PrefKind::International), name: "Internacional", category: 0, keywords: "idioma língua formatos data número moeda região menu de entrada layout teclado" },
    PaneInfo { pane: Pane::Acesso, id: "acesso", icon: Icon::Pref(PrefKind::Access), name: "Acesso Universal", category: 3, keywords: "zoom ampliar lupa inverter cores branco no preto cinza contraste cursor grande visão acessibilidade" },
    PaneInfo { pane: Pane::Midia, id: "midia", icon: Icon::Pref(PrefKind::Removable), name: "Mídia Removível", category: 1, keywords: "pendrive usb disco externo cartão cd dvd abrir automaticamente" },
    PaneInfo { pane: Pane::Bluetooth, id: "bluetooth", icon: Icon::Pref(PrefKind::Wireless), name: "Bluetooth", category: 2, keywords: "fone teclado mouse sem fio parear conectar aparelho" },
    PaneInfo { pane: Pane::Compart, id: "compartilhamento", icon: Icon::Pref(PrefKind::Sharing), name: "Compartilhamento", category: 2, keywords: "nome do computador ssh sessão remota samba arquivos impressoras rede" },
    PaneInfo { pane: Pane::Padrao, id: "padrao", icon: Icon::Pref(PrefKind::DefaultApps), name: "Aplicativos Padrão", category: 3, keywords: "navegador e-mail abrir com texto imagens pdf música vídeo padrão" },
    PaneInfo { pane: Pane::Impressao, id: "impressao", icon: Icon::Pref(PrefKind::Printer), name: "Impressão", category: 1, keywords: "impressora imprimir cups fila padrão" },
    PaneInfo { pane: Pane::Atualizacao, id: "atualizacao", icon: Icon::Pref(PrefKind::Update), name: "Atualização de Software", category: 3, keywords: "atualizar pacotes apt sistema versão upgrade" },
    PaneInfo { pane: Pane::Inicializacao, id: "inicializacao", icon: Icon::Pref(PrefKind::StartupDisk), name: "Disco de Inicialização", category: 3, keywords: "boot iniciar sistema operacional uefi reiniciar disco" },
    PaneInfo { pane: Pane::Fala, id: "fala", icon: Icon::Pref(PrefKind::Speech), name: "Fala", category: 3, keywords: "voz ler em voz alta falar sintetizador" },
    PaneInfo { pane: Pane::Terminal, id: "terminal", icon: Icon::Terminal, name: "Terminal", category: 3, keywords: "fonte shell linha de comando" },
    PaneInfo { pane: Pane::Seguranca, id: "seguranca", icon: Icon::Pref(PrefKind::Security), name: "Segurança", category: 0, keywords: "senha bloquear repouso protetor sair encerrar sessão inatividade firewall ufw proteção" },
    PaneInfo { pane: Pane::Busca, id: "busca", icon: Icon::Search, name: "Busca Rápida", category: 0, keywords: "spotlight buscar procurar categorias privacidade pastas excluir resultados" },
    PaneInfo { pane: Pane::Expose, id: "expose", icon: Icon::Pref(PrefKind::Expose), name: "Exposé e Painel", category: 0, keywords: "exposé expose cantos ativos janelas mesa painel de widgets dashboard f9 f10 f11 f12" },
];

fn info(p: Pane) -> Option<&'static PaneInfo> {
    PANES.iter().find(|i| i.pane == p)
}

/// Minúsculas e sem acentos (a busca acha "aparencia" e "Aparência").
fn norm(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Painéis que combinam com a busca (todas as palavras, no nome ou nas palavras-chave).
fn matches(query: &str) -> Vec<Pane> {
    let words: Vec<String> = norm(query).split_whitespace().map(String::from).collect();
    if words.is_empty() {
        return Vec::new();
    }
    PANES
        .iter()
        .filter(|i| {
            // Busca nos dois idiomas: o nome e as palavras em português e traduzidos.
            let hay = norm(&format!("{} {} {} {}", i.name, i.keywords, tr(i.name), tr(i.keywords)));
            words.iter().all(|w| hay.split_whitespace().any(|h| h.starts_with(w.as_str())))
        })
        .map(|i| i.pane)
        .collect()
}

/// Grade: largura de cada célula e altura de cada faixa de categoria.
const CELL_W: f32 = 80.0;
const CELL_H: f32 = 78.0;
const BAND_LABEL: f32 = 22.0;

const SCALES: [f64; 8] = [0.0, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
/// Tempo para confirmar uma resolução nova antes de voltar à anterior.
const CONFIRM_SECS: u64 = 15;

fn scale_label(v: f64) -> String {
    format!("{}×", format!("{v}").replace('.', glove_config::virgula()))
}

/// "2256x1504@60" -> "2256 × 1504, 60 Hz".
fn mode_label(m: &str) -> String {
    match glove_config::parse_mode(m) {
        Some((w, h, hz)) if hz > 0 => format!("{w} × {h}, {hz} Hz"),
        Some((w, h, _)) => format!("{w} × {h}"),
        None => m.to_string(),
    }
}


const WALLPAPERS: [(&str, &str); 3] = [("ondas", "Ondas"), ("aurora", "Aurora"), ("liso", "Liso")];

/// Cores sólidas para a mesa (como a coleção do 10.4, em tons próprios).
const SOLID_COLORS: [(u8, u8, u8); 10] = [
    (38, 84, 160),
    (60, 120, 200),
    (110, 160, 210),
    (40, 110, 90),
    (110, 140, 70),
    (150, 60, 50),
    (200, 140, 60),
    (100, 80, 140),
    (90, 94, 102),
    (170, 172, 178),
];

/// Nome do papel de parede para mostrar ("Ondas", "Cor sólida", "foto.png").
fn wallpaper_name(style: &str) -> String {
    if let Some((_, n)) = WALLPAPERS.iter().find(|(i, _)| *i == style || (style.is_empty() && *i == "ondas")) {
        return tr(n).to_string();
    }
    if style.starts_with("cor:") {
        return tr!("Cor sólida").into();
    }
    std::path::Path::new(style).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| style.to_string())
}

struct Prefs {
    config: Config,
    pane: Pane,
    /// Imagens PNG encontradas em ~/Imagens (para usar de papel de parede).
    images: Vec<std::path::PathBuf>,
    /// Monitores conectados (escritos pelo compositor).
    outputs: Option<OutputsInfo>,
    /// Resolução nova esperando confirmação: (monitor e resolução anteriores, prazo).
    confirm: Option<(String, String, Instant)>,
    /// Organização dos monitores antes da mudança (volta junto com a resolução).
    confirm_arrangement: Option<String>,
    /// Ajustes dos outros monitores antes da mudança.
    confirm_others: Option<std::collections::BTreeMap<String, glove_config::OtherMonitor>>,
    /// Monitor escolhido em "Ajustar o monitor" (vazio = o principal).
    mon_target: String,
    /// Fontes de largura fixa instaladas (lidas ao abrir o painel Terminal).
    mono_fonts: Option<Vec<String>>,
    /// Temas de ícones instalados: (pasta, nome).
    icon_themes: Option<Vec<(String, String)>>,
    /// Aba aberta no painel Teclado.
    kbd_tab: usize,
    /// Grupos abertos no painel Atalhos.
    open_groups: [bool; 3],
    /// Aba do painel Som e os dispositivos (None = sem PipeWire).
    sound_tab: usize,
    /// Leitura dos dispositivos de som em andamento (numa thread).
    audio_rx: Option<std::sync::mpsc::Receiver<Option<(Vec<som::Device>, Vec<som::Device>)>>>,
    /// Gravação da configuração adiada (arrastando um slider) e a hora da última.
    save_pending: std::cell::Cell<bool>,
    last_save: std::cell::Cell<Instant>,
    /// Painel de energia: aba e "configurações para" (0 tomada, 1 bateria).
    energy_tab: usize,
    energy_source: usize,
    rede: rede::Rede,
    datahora: datahora::DataHora,
    inicio: inicio::Inicio,
    contas: contas::Contas,
    internacional: internacional::Internacional,
    compart: compart::Compart,
    padrao: padrao::Padrao,
    bluetooth: bluetooth::Bluetooth,
    atualizacao: outros::Atualizacao,
    inicializacao: outros::Inicializacao,
    fala: outros::Fala,
    impressao: outros::Impressao,
    audio: Option<(Vec<som::Device>, Vec<som::Device>)>,
    /// Aba do Acesso Universal.
    access_tab: usize,
    /// Aba do painel Mesa e Protetor de Tela.
    desk_tab: usize,
    /// Hora de abrir (para animar a prévia do protetor).
    started: Instant,
    /// Coleção escolhida no painel Mesa.
    desk_source: usize,
    /// Miniaturas de papel de parede já desenhadas.
    thumbs: std::collections::HashMap<String, Option<glove_kit::paint::draw::Canvas>>,
    /// Histórico das setas da barra.
    back: Vec<Pane>,
    forward: Vec<Pane>,
    /// Texto da busca da barra.
    search: String,
    /// Busca Rápida: pasta escolhida na lista da privacidade e a que vai entrar.
    busca_sel: Option<usize>,
    busca_nova: String,
    /// Menu de contexto de um painel da grade: (posição, painel).
    ctx: Option<((f32, f32), Pane)>,
}

/// Grupos de atalhos (como no painel de atalhos do 10.4).
const SHORTCUT_GROUPS: [(&str, &[&str]); 3] = [
    ("Janelas", &["close", "minimize", "zoom", "cycle", "expose_all", "expose_app", "expose_desktop", "dashboard", "cycle_windows"]),
    ("Aplicativos", &["terminal", "preferences", "search", "hide", "hide_others"]),
    ("Sistema", &["screenshot", "screenshot_area", "force_quit", "lock", "input_next", "zoom_toggle", "zoom_in", "zoom_out", "invert_colors", "quit"]),
];

impl Prefs {
    fn new() -> Self {
        let mut p = Self::build();
        if std::env::args().nth(1).as_deref() == Some("protetor") {
            p.desk_tab = 1;
        }
        if std::env::args().nth(1).as_deref() == Some("inicio") {
            p.contas.tab = 2;
        }
        if p.pane == Pane::Contas {
            p.contas.refresh();
            p.inicio.refresh();
        }
        if p.pane == Pane::Rede {
            p.rede.refresh();
        }
        if p.pane == Pane::DataHora {
            p.datahora.refresh();
        }
        if p.pane == Pane::Inicio {
            p.inicio.refresh();
        }
        // "glove-prefs rede --entrar REDE" (do menu Wi-Fi): já pede a senha dela.
        let args: Vec<String> = std::env::args().collect();
        if let Some(ssid) = args.iter().position(|a| a == "--entrar").and_then(|i| args.get(i + 1)) {
            p.rede.show = 1 + rede_wifi_index();
            p.rede.prompt = Some((ssid.clone(), String::new()));
        }
        p
    }

    fn build() -> Self {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let mut images = Vec::new();
        for d in [tr!("Imagens"), "Pictures"] {
            if let Ok(rd) = std::fs::read_dir(home.join(d)) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                        images.push(p);
                    }
                }
            }
        }
        images.sort();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let arg = args.first().cloned().unwrap_or_default();
        // "atalhos" é a segunda aba do Teclado (como no 10.4).
        let (pane, kbd_tab) = match arg.as_str() {
            "atalhos" => (Pane::Teclado, 1),
            "entrada" => (Pane::Teclado, 2),
            "protetor" => (Pane::Mesa, 0),
            "inicio" => (Pane::Contas, 0),
            a => (PANES.iter().find(|i| i.id == a).map_or(Pane::Todos, |i| i.pane), 0),
        };
        Self {
            config: Config::load(),
            pane,
            images,
            outputs: OutputsInfo::load(),
            confirm: None,
            confirm_arrangement: None,
            confirm_others: None,
            mon_target: String::new(),
            mono_fonts: None,
            icon_themes: None,
            kbd_tab,
            open_groups: [true, false, false],
            desk_source: 0,
            desk_tab: 0,
            started: Instant::now(),
            access_tab: 0,
            sound_tab: 0,
            audio_rx: (pane == Pane::Som).then(som::status_async),
            save_pending: std::cell::Cell::new(false),
            last_save: std::cell::Cell::new(Instant::now() - Duration::from_secs(1)),
            energy_tab: 0,
            energy_source: usize::from(on_battery()),
            rede: rede::Rede::default(),
            datahora: datahora::DataHora::default(),
            inicio: inicio::Inicio::default(),
            contas: contas::Contas::default(),
            internacional: internacional::Internacional::default(),
            compart: compart::Compart::default(),
            padrao: padrao::Padrao::default(),
            bluetooth: bluetooth::Bluetooth::default(),
            atualizacao: Default::default(),
            inicializacao: Default::default(),
            fala: Default::default(),
            impressao: Default::default(),
            audio: None,
            thumbs: std::collections::HashMap::new(),
            back: Vec::new(),
            forward: Vec::new(),
            search: String::new(),
            busca_sel: None,
            busca_nova: String::new(),
            ctx: None,
        }
    }

    /// Abre um painel, guardando o atual para a seta Voltar.
    fn go(&mut self, p: Pane) {
        if p != self.pane {
            self.back.push(self.pane);
            self.forward.clear();
            self.pane = p;
            self.ctx = None;
            if p == Pane::Som {
                self.audio_rx = Some(som::status_async());
            }
            if p == Pane::Rede {
                self.rede.refresh();
            }
            if p == Pane::DataHora {
                self.datahora.refresh();
            }
            if p == Pane::Inicio {
                self.inicio.refresh();
            }
            if p == Pane::Contas {
                self.contas.refresh();
                self.inicio.refresh();
            }
            if p == Pane::Compart {
                self.compart.refresh();
            }
            if p == Pane::Padrao {
                self.padrao.refresh();
            }
            if p == Pane::Bluetooth {
                self.bluetooth.refresh();
            }
        }
    }

    fn favorites(&self) -> Vec<&'static PaneInfo> {
        self.config.prefs_favorites.iter().filter_map(|f| PANES.iter().find(|i| i.id == f)).collect()
    }

    fn toggle_favorite(&mut self, p: Pane) {
        let Some(i) = info(p) else { return };
        let favs = &mut self.config.prefs_favorites;
        if let Some(at) = favs.iter().position(|f| f == i.id) {
            favs.remove(at);
        } else {
            favs.push(i.id.to_string());
        }
        self.save();
    }

    /// Grava a configuração (os outros apps e o Glove aplicam na hora). Arrastando
    /// um slider, grava no máximo a cada 150 ms; o resto fica para `flush_save`.
    fn save(&self) {
        if self.last_save.get().elapsed() < SAVE_EVERY {
            self.save_pending.set(true);
            return;
        }
        self.save_now();
    }

    fn save_now(&self) {
        self.save_pending.set(false);
        self.last_save.set(Instant::now());
        if let Err(e) = self.config.save() {
            eprintln!("glove-prefs: não consegui salvar: {e}");
        }
    }

    /// Grava o que ficou adiado (ao soltar o mouse ou passado o intervalo).
    fn flush_save(&self, ui: &mut Ui) {
        if !self.save_pending.get() {
            return;
        }
        if !ui.mouse_down() || self.last_save.get().elapsed() >= SAVE_EVERY {
            self.save_now();
        } else {
            ui.wake_after(SAVE_EVERY);
        }
    }

    /// Barra: setas, "Mostrar Todos", os favoritos e a busca (10.2 + 10.4).
    fn toolbar(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        // Faixa clara com brilho, separada do conteúdo por uma linha.
        ui.c.fill_rect(0.0, 0.0, w, TOOLBAR_H, &vgrad(0.0, TOOLBAR_H, &[(0.0, rgba(250, 250, 250, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.pinstripes(0.0, 0.0, w, TOOLBAR_H, rgba(255, 255, 255, 0), rgba(255, 255, 255, 70));
        ui.c.hline(0.0, TOOLBAR_H - 1.0, w, rgba(120, 120, 124, 255));
        let mid = 10.0 + 58.0 / 2.0;
        let nav = [!self.back.is_empty(), !self.forward.is_empty()];
        match ui.segmented_in(Rect::new(12.0, mid - SEGMENT_H / 2.0, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &nav) {
            Some(0) => {
                if let Some(p) = self.back.pop() {
                    self.forward.push(self.pane);
                    self.pane = p;
                }
            }
            Some(_) => {
                if let Some(p) = self.forward.pop() {
                    self.back.push(self.pane);
                    self.pane = p;
                }
            }
            None => {}
        }
        let favs = self.favorites();
        let mut chosen = None;
        ui.area(Rect::new(12.0 + 2.0 * SEGMENT_W + 10.0, 10.0, w - 340.0, TOOLBAR_H), |ui| {
            ui.spacing(4.0);
            ui.horizontal(|ui| {
                if ui.icon_tile(Icon::Pref(PrefKind::ShowAll), tr!("Mostrar Todos"), false) {
                    chosen = Some(Pane::Todos);
                }
                // Divisória entre "Mostrar Todos" e os favoritos.
                let r = ui.alloc(9.0, 58.0);
                ui.c.vline(r.x + 4.0, r.y + 8.0, 42.0, rgba(0, 0, 0, 40));
                ui.c.vline(r.x + 5.0, r.y + 8.0, 42.0, rgba(255, 255, 255, 160));
                for f in &favs {
                    // Os que não cabem antes da busca ficam de fora.
                    if ui.available_width() < (ui.text_width(tr(f.name), 11.0, false) + 12.0).max(64.0) {
                        break;
                    }
                    if ui.icon_tile(f.icon, tr(f.name), false) {
                        chosen = Some(f.pane);
                    }
                }
            });
        });
        if let Some(p) = chosen {
            self.search.clear();
            self.go(p);
            ui.redraw = true;
        }
        // Busca (como no 10.4): destaca os painéis que combinam.
        let before = self.search.clone();
        ui.search_field_in(Rect::new(w - 172.0, mid - glove_kit::SEARCH_H / 2.0, 160.0, glove_kit::SEARCH_H), &mut self.search, tr!("Buscar"));
        if self.search != before && !self.search.trim().is_empty() && self.pane != Pane::Todos {
            self.go(Pane::Todos);
        }
    }

    /// A grade "Mostrar Todos": uma faixa por categoria, alternando o fundo.
    fn show_all(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let found = matches(&self.search);
        let searching = !self.search.trim().is_empty();
        let per_row = ((w - 40.0) / CELL_W).floor().max(1.0) as usize;
        let mut y = TOOLBAR_H;
        let mut cells: Vec<(Rect, &'static PaneInfo)> = Vec::new();
        let mut band = 0;
        for (ci, cat) in CATEGORIES.iter().enumerate() {
            let cat: &str = tr(cat);
            let items: Vec<&'static PaneInfo> = PANES.iter().filter(|i| i.category == ci).collect();
            if items.is_empty() {
                continue;
            }
            let rows = items.len().div_ceil(per_row);
            let bh = BAND_LABEL + rows as f32 * CELL_H + 6.0;
            if band % 2 == 1 {
                ui.c.fill_rect(0.0, y, w, bh, &solid(rgba(236, 236, 238, 255)));
            } else {
                ui.c.fill_rect(0.0, y, w, bh, &solid(rgba(250, 250, 250, 255)));
            }
            ui.c.hline(0.0, y + bh - 1.0, w, rgba(205, 205, 208, 255));
            ui.fonts.draw(ui.c, cat, 14.0, y + 17.0, 12.0, true, rgba(60, 60, 64, 255));
            for (k, i) in items.into_iter().enumerate() {
                let (col, row) = ((k % per_row) as f32, (k / per_row) as f32);
                cells.push((Rect::new(20.0 + col * CELL_W, y + BAND_LABEL + row * CELL_H, CELL_W, CELL_H), i));
            }
            y += bh;
            band += 1;
        }
        let draw_cell = |ui: &mut Ui, r: &Rect, i: &PaneInfo, ink: Color| {
            glove_kit::paint::icons::draw(ui.c, ui.fonts, i.icon, r.x + (r.w - 32.0) / 2.0, r.y + 6.0, 32.0, None);
            // Nome em até duas linhas, centrado.
            for (n, l) in wrap2(ui, tr(i.name), r.w - 4.0).iter().enumerate() {
                // Palavra comprida demais ("Compartilhamento") encolhe um pouco para caber.
                let full = ui.text_width(l, 11.0, false);
                let px = if full > r.w - 4.0 { (11.0 * (r.w - 4.0) / full).max(8.5) } else { 11.0 };
                let tw = ui.text_width(l, px, false);
                ui.fonts.draw(ui.c, l, r.x + (r.w - tw) / 2.0, r.y + 52.0 + n as f32 * 13.0, px, false, ink);
            }
        };
        for (r, i) in &cells {
            draw_cell(ui, r, i, rgba(20, 20, 20, 255));
        }
        // Na busca, escurece tudo e acende os painéis encontrados.
        if searching {
            ui.c.fill_rect(0.0, TOOLBAR_H, w, y - TOOLBAR_H, &solid(rgba(0, 0, 0, 120)));
            for (r, i) in cells.iter().filter(|(_, i)| found.contains(&i.pane)) {
                let (cx, cy) = (r.x + r.w / 2.0, r.y + 30.0);
                ui.c.fill(circle(cx, cy, 44.0), &radial(cx, cy, 44.0, &[(0.0, rgba(255, 255, 255, 255)), (0.6, rgba(255, 255, 255, 220)), (1.0, rgba(255, 255, 255, 0))]));
                draw_cell(ui, r, i, rgba(20, 20, 20, 255));
            }
        }
        let mut open = None;
        // Com o menu de contexto aberto, os cliques são dele.
        let menu_open = self.ctx.is_some();
        for (r, i) in cells.iter().filter(|_| !menu_open) {
            if ui.hit(*r).clicked {
                open = Some(i.pane);
            }
            if ui.right_clicked(*r) {
                self.ctx = Some((ui.pointer(), i.pane));
            }
        }
        if searching && ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Enter) {
            open = found.first().copied();
        }
        if let Some(p) = open {
            self.search.clear();
            self.go(p);
            ui.redraw = true;
        }
        self.pane_menu(ui);
        // A janela termina junto com a última faixa.
        ui.alloc(w, y - TOOLBAR_H - 44.0);
    }

    /// Menu de contexto da grade: pôr ou tirar o painel da barra.
    fn pane_menu(&mut self, ui: &mut Ui) {
        let Some(((x, y), p)) = self.ctx else { return };
        let Some(i) = info(p) else { return };
        let fav = self.config.prefs_favorites.iter().any(|f| f == i.id);
        let items = [tr!("Abrir “{}”", tr(i.name)), if fav { tr!("Tirar da Barra").to_string() } else { tr!("Pôr na Barra").to_string() }];
        let (mw, row) = (190.0, 20.0);
        let mh = row * items.len() as f32 + 8.0;
        let x = x.min(ui.c.w - mw - 4.0);
        ui.c.fill_rect(x + 3.0, y + 4.0, mw, mh, &solid(rgba(0, 0, 0, 50)));
        ui.c.fill_rect(x, y, mw, mh, &solid(rgba(246, 247, 249, 252)));
        ui.c.stroke(rrect(x + 0.5, y + 0.5, mw - 1.0, mh - 1.0, 0.0), &solid(rgba(0, 0, 0, 80)), 1.0);
        let mut chosen = None;
        for (n, label) in items.iter().enumerate() {
            let r = Rect::new(x, y + 4.0 + n as f32 * row, mw, row);
            let h = ui.hit(r);
            if h.hover {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(ui.theme.menu_hot));
            }
            ui.fonts.draw(ui.c, label, x + 16.0, r.y + 14.5, 13.0, false, if h.hover { glove_kit::paint::Color::WHITE } else { rgba(20, 20, 20, 255) });
            if h.clicked {
                chosen = Some(n);
            }
        }
        match chosen {
            Some(0) => {
                self.ctx = None;
                self.go(p);
            }
            Some(_) => {
                self.ctx = None;
                self.toggle_favorite(p);
            }
            None if ui.mouse_pressed() || ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Escape) => {
                let (px, py) = ui.pointer();
                if !(px >= x && px < x + mw && py >= y && py < y + mh) {
                    self.ctx = None;
                }
            }
            None => {}
        }
    }

    fn aparencia(&mut self, ui: &mut Ui) {
        // Formulário da era: rótulos à direita, controles numa coluna comum.
        const LW: f32 = 210.0;
        ui.space(4.0);
        let names: Vec<String> = Variant::ALL.iter().map(|v| v.label().to_string()).collect();
        let mut v = Variant::ALL.iter().position(|v| v.id() == self.config.theme).unwrap_or(0);
        ui.form_row(tr!("Aparência"), LW, |ui| {
            ui.horizontal(|ui| {
                if ui.popup(&mut v, &names, 130.0) {
                    self.config.theme = Variant::ALL[v].id().into();
                    self.save();
                }
                ui.small(tr!("Para botões, menus e janelas"));
            });
        });
        ui.space(4.0);
        let names: Vec<String> = glove_config::HIGHLIGHTS.iter().map(|h| tr(h.1).to_string()).collect();
        let mut h = glove_config::HIGHLIGHTS.iter().position(|h| h.0 == self.config.appearance.highlight).unwrap_or(0);
        ui.form_row(tr!("Cor de destaque"), LW, |ui| {
            ui.horizontal(|ui| {
                if ui.popup(&mut h, &names, 130.0) {
                    self.config.appearance.highlight = glove_config::HIGHLIGHTS[h].0.into();
                    self.save();
                }
                ui.small(tr!("Para texto selecionado"));
            });
        });
        ui.space(4.0);
        // Ícones: os desenhados pelo Glove ou um tema do sistema (os apps de fora
        // usam sempre o ícone deles).
        if self.icon_themes.is_none() {
            self.icon_themes = Some(glove_kit::paint::sysicons::installed_themes());
        }
        let themes = self.icon_themes.clone().unwrap_or_default();
        let mut names: Vec<String> = vec![tr!("Glove (desenhados)").into()];
        names.extend(themes.iter().map(|t| t.1.clone()));
        let mut cur = themes.iter().position(|t| t.0 == self.config.appearance.icon_theme).map(|i| i + 1).unwrap_or(0);
        ui.form_row(tr!("Ícones"), LW, |ui| {
            if ui.popup(&mut cur, &names, 200.0) {
                self.config.appearance.icon_theme = if cur == 0 { String::new() } else { themes[cur - 1].0.clone() };
                glove_kit::paint::icons::set_theme(&self.config.appearance.icon_theme);
                self.save();
            }
        });
        // Prévia com alguns ícones no tema escolhido.
        ui.form_row("", LW, |ui| {
            use glove_kit::paint::icons::{FolderKind, Icon};
            let sample = [Icon::Folder, Icon::SpecialFolder(FolderKind::Documents), Icon::Terminal, Icon::TextEditor, Icon::Calculator, Icon::Settings, Icon::Trash];
            let (x, y) = ui.cursor();
            ui.alloc(7.0 * 40.0, 36.0);
            for (k, ic) in sample.iter().enumerate() {
                glove_kit::paint::icons::draw_bare(ui.c, ui.fonts, *ic, x + k as f32 * 40.0, y, 32.0, None);
            }
        });
        ui.space(10.0);
        ui.separator();
        ui.space(8.0);
        let mut jump = usize::from(self.config.appearance.scroll_jump);
        ui.form_row(tr!("Clicar na barra de rolagem para"), LW, |ui| {
            if ui.radio_group(&mut jump, &[tr!("Ir para a página seguinte"), tr!("Ir para o ponto clicado")]) {
                self.config.appearance.scroll_jump = jump == 1;
                self.save();
            }
        });
        ui.space(4.0);
        let mut dbl = self.config.appearance.double_click_minimizes;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut dbl, tr!("Minimizar ao clicar duas vezes na barra de título")) {
                self.config.appearance.double_click_minimizes = dbl;
                self.save();
            }
        });
        ui.space(10.0);
        ui.separator();
        ui.space(8.0);
        const COUNTS: [u32; 7] = [0, 5, 10, 15, 20, 30, 50];
        let names: Vec<String> = COUNTS.iter().map(|n| if *n == 0 { tr!("Nenhum").to_string() } else { n.to_string() }).collect();
        let mut apps = COUNTS.iter().position(|n| *n == self.config.appearance.recent_apps).unwrap_or(2);
        let mut docs = COUNTS.iter().position(|n| *n == self.config.appearance.recent_docs).unwrap_or(2);
        ui.form_row(tr!("Itens recentes"), LW, |ui| {
            ui.horizontal(|ui| {
                if ui.popup(&mut apps, &names, 90.0) {
                    self.config.appearance.recent_apps = COUNTS[apps];
                    self.save();
                }
                ui.small(tr!("Aplicativos"));
            });
            ui.horizontal(|ui| {
                if ui.popup(&mut docs, &names, 90.0) {
                    self.config.appearance.recent_docs = COUNTS[docs];
                    self.save();
                }
                ui.small(tr!("Documentos"));
            });
        });
    }

    /// Mesa: a imagem atual em destaque, as coleções à esquerda e as miniaturas à direita.
    fn mesa(&mut self, ui: &mut Ui) {
        let mut tab = self.desk_tab;
        let shown = tab;
        ui.space(4.0);
        ui.tab_box(&mut tab, &[tr!("Mesa"), tr!("Protetor de Tela")], if shown == 0 { 400.0 } else { 456.0 }, |ui| match shown {
            0 => self.mesa_tab(ui),
            _ => self.protetor_tab(ui),
        });
        self.desk_tab = tab;
    }

    /// Protetor de Tela: lista, prévia animada, tempo, Testar e cantos ativos.
    fn protetor_tab(&mut self, ui: &mut Ui) {
        use glove_kit::paint::saver::SAVERS;
        let (x0, y0) = ui.cursor();
        let w = ui.available_width();
        let rows: Vec<String> = SAVERS.iter().map(|(_, n)| n.to_string()).collect();
        let mut sel = SAVERS.iter().position(|(id, _)| *id == self.config.saver.kind).or(Some(0));
        ui.area(Rect::new(x0, y0, 150.0, 190.0), |ui| {
            ui.label(tr!("Protetores de tela"));
            if ui.list(&mut sel, &rows, 150.0, 166.0) {
                if let Some(i) = sel {
                    self.config.saver.kind = SAVERS[i].0.to_string();
                    self.save();
                }
            }
        });
        // Prévia (o mesmo desenho do compositor, pequeno).
        let (px, py) = (x0 + 170.0, y0 + 20.0);
        let pw = (w - 170.0).min(300.0);
        let ph = (pw * 0.6).round();
        ui.c.fill(rrect(px - 3.0, py - 3.0, pw + 6.0, ph + 6.0, 3.0), &solid(rgba(0, 0, 0, 60)));
        if let Some(mut c) = glove_kit::paint::draw::Canvas::new(pw, ph, ui.c.scale) {
            let (y, mo, d, _, h, m, _) = glove_kit::local_time();
            let time = format!("{h:02}:{m:02}");
            let date = format!("{d:02}/{mo:02}/{y}");
            glove_kit::paint::saver::draw(&mut c, ui.fonts, &self.config.saver.kind, pw, ph, self.started.elapsed().as_secs_f32(), (&time, &date));
            ui.c.draw_canvas(&c, px, py);
        }
        ui.fonts.draw(ui.c, tr!("Prévia"), px + pw / 2.0 - ui.text_width(tr!("Prévia"), 11.0, false) / 2.0, py + ph + 16.0, 11.0, false, rgba(90, 90, 96, 255));
        ui.area(Rect::new(px + pw - 80.0, py + ph + 6.0, 80.0, 30.0), |ui| {
            if ui.button(tr!("Testar")) {
                ui.request_screensaver(&self.config.saver.kind);
            }
        });
        ui.wake_after_in(Duration::from_millis(66), Rect::new(px, py, pw, ph));
        ui.alloc(w, (20.0 + ph + 40.0).max(196.0));
        if let Some(m) = Self::minutes_slider(ui, tr!("Iniciar o protetor de tela quando inativo por:"), self.config.saver.minutes) {
            self.config.saver.minutes = m;
            self.save();
        }
        ui.space(8.0);
        // Cantos ativos: no painel Exposé e Painel (como no Tiger, um botão leva até lá).
        if ui.button(tr!("Cantos Ativos…")) {
            self.go(Pane::Expose);
        }
    }

    /// Os quatro cantos da tela e o que cada um faz.
    fn hot_corners_ui(&mut self, ui: &mut Ui) {
        ui.label(tr!("Cantos ativos:"));
        let actions: [(&str, &str); 6] = [("", "—"), ("todas", tr!("Todas as janelas")), ("app", tr!("Janelas do app")), ("mesa", tr!("Mesa")), ("painel", tr!("Painel de Widgets")), ("protetor", tr!("Iniciar o protetor de tela"))];
        let names: Vec<String> = actions.iter().map(|a| a.1.to_string()).collect();
        let hc = self.config.hot_corners.clone();
        let mut corners = [hc.top_left, hc.top_right, hc.bottom_left, hc.bottom_right];
        let mut changed = false;
        for (row, pair) in [[0usize, 1], [2, 3]].iter().enumerate() {
            ui.horizontal(|ui| {
                ui.space(16.0);
                for &k in pair {
                    let mut cur = actions.iter().position(|a| a.0 == corners[k]).unwrap_or(0);
                    ui.push_id(&format!("canto{k}"), |ui| {
                        if ui.popup(&mut cur, &names, 200.0) {
                            corners[k] = actions[cur].0.to_string();
                            changed = true;
                        }
                    });
                    if k % 2 == 0 {
                        ui.space(40.0);
                    }
                }
            });
            if row == 0 {
                ui.space(4.0);
            }
        }
        if changed {
            let [a, b, c, d] = corners;
            self.config.hot_corners = glove_config::HotCorners { top_left: a, top_right: b, bottom_left: c, bottom_right: d };
            self.save();
        }
    }

    fn mesa_tab(&mut self, ui: &mut Ui) {
        let (x0, y0) = ui.cursor();
        let w = ui.available_width();
        // Poço com a imagem atual.
        let (pw, ph) = (128.0, 88.0);
        let cur = self.config.wallpaper.clone();
        ui.c.fill(rrect(x0 - 3.0, y0 - 3.0, pw + 6.0, ph + 6.0, 3.0), &solid(rgba(0, 0, 0, 30)));
        ui.c.stroke(rrect(x0 - 2.5, y0 - 2.5, pw + 5.0, ph + 5.0, 3.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        self.draw_thumb(ui, &cur, Rect::new(x0, y0, pw, ph));
        ui.fonts.draw(ui.c, &wallpaper_name(&cur), x0 + pw + 20.0, y0 + 40.0, 13.0, true, rgba(20, 20, 20, 255));
        ui.fonts.draw(ui.c, tr!("Clique numa imagem para usar na mesa."), x0 + pw + 20.0, y0 + 58.0, 11.0, false, rgba(90, 90, 96, 255));
        ui.alloc(w, ph + 16.0);
        // Coleções.
        let sources = [tr!("Imagens do Glove"), tr!("Cores Sólidas"), tr!("Pasta Imagens")];
        let mut sel = Some(self.desk_source);
        let rows: Vec<String> = sources.iter().map(|s| s.to_string()).collect();
        let (lx, ly) = ui.cursor();
        let list_h = 230.0;
        ui.area(Rect::new(lx, ly, 150.0, list_h), |ui| {
            if ui.list(&mut sel, &rows, 150.0, list_h) {
                if let Some(s) = sel {
                    self.desk_source = s;
                }
            }
        });
        // Miniaturas da coleção.
        let items: Vec<String> = match self.desk_source {
            0 => WALLPAPERS.iter().map(|(i, _)| i.to_string()).collect(),
            1 => SOLID_COLORS.iter().map(|(r, g, b)| format!("cor:{r},{g},{b}")).collect(),
            _ => self.images.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        };
        let gx = lx + 166.0;
        let gw = w - 166.0;
        ui.c.fill_rect(gx, ly, gw, list_h, &solid(rgba(255, 255, 255, 255)));
        ui.c.stroke(rrect(gx + 0.5, ly + 0.5, gw - 1.0, list_h - 1.0, 0.0), &solid(rgba(130, 130, 135, 255)), 1.0);
        if items.is_empty() {
            ui.fonts.draw(ui.c, tr!("Ponha imagens PNG na sua pasta Imagens."), gx + 14.0, ly + 24.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let (tw, th, gap) = (88.0, 62.0, 10.0);
        let per_row = ((gw - gap) / (tw + gap)).floor().max(1.0) as usize;
        let mut chosen = None;
        for (i, it) in items.iter().enumerate() {
            let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
            let r = Rect::new(gx + gap + col * (tw + gap), ly + gap + row * (th + gap), tw, th);
            if r.y + r.h > ly + list_h {
                break;
            }
            if *it == cur {
                ui.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 3.0), &solid(ui.theme.accent));
            }
            self.draw_thumb(ui, it, r);
            if ui.hit(r).clicked {
                chosen = Some(it.clone());
            }
        }
        if let Some(c) = chosen {
            self.config.wallpaper = c;
            self.save();
            ui.redraw = true;
        }
        ui.alloc(w, list_h);
    }

    /// Miniatura de um papel de parede (guardada; os PNG só são lidos uma vez).
    fn draw_thumb(&mut self, ui: &mut Ui, style: &str, r: Rect) {
        let key = format!("{style}@{}x{}", r.w, r.h);
        if !self.thumbs.contains_key(&key) {
            let c = glove_kit::paint::wallpaper::render_style(ui.theme, style, r.w, r.h, ui.c.scale);
            self.thumbs.insert(key.clone(), c);
        }
        if let Some(Some(c)) = self.thumbs.get(&key) {
            ui.c.draw_canvas(c, r.x, r.y);
        }
    }

    fn dock(&mut self, ui: &mut Ui) {
        // Formulário da era: rótulos à direita, controles numa coluna comum.
        const LW: f32 = 190.0;
        ui.space(8.0);
        let mut size = self.config.dock.size as f32;
        ui.form_row(tr!("Tamanho do Dock"), LW, |ui| {
            let t = Ticks { count: 2, labels: &[(0.0, tr!("Pequeno")), (1.0, tr!("Grande"))], snap: false };
            ui.horizontal(|ui| {
                if ui.slider_ticks(&mut size, 32.0, 72.0, 240.0, &t, true) {
                    self.config.dock.size = size.round() as u32;
                    self.config.dock.max_size = self.config.dock.max_size.max(self.config.dock.size);
                    self.save();
                }
                ui.spacing(6.0);
                ui.label(&tr!("{} px", self.config.dock.size));
            });
        });
        ui.space(6.0);
        let mut mag = self.config.dock.magnification;
        let mut max = self.config.dock.max_size as f32;
        let min = self.config.dock.size as f32;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut mag, tr!("Ampliação")) {
                self.config.dock.magnification = mag;
                self.save();
            }
            let t = Ticks { count: 2, labels: &[(0.0, tr!("Mínima")), (1.0, tr!("Máxima"))], snap: false };
            ui.horizontal(|ui| {
                if ui.slider_ticks(&mut max, min, 128.0, 240.0, &t, mag) {
                    self.config.dock.max_size = max.round() as u32;
                    self.save();
                }
                ui.spacing(6.0);
                if mag {
                    ui.label(&tr!("{} px", self.config.dock.max_size));
                }
            });
        });
        ui.space(8.0);
        let words = ["esquerda", "embaixo", "direita"];
        let mut pos = words.iter().position(|w| *w == self.config.dock.position).unwrap_or(1);
        ui.form_row(tr!("Posição na tela"), LW, |ui| {
            ui.horizontal(|ui| {
                ui.spacing(14.0);
                if ui.radio_group(&mut pos, &[tr!("Esquerda"), tr!("Embaixo"), tr!("Direita")]) {
                    self.config.dock.position = words[pos].to_string();
                    self.save();
                }
            });
        });
        ui.space(6.0);
        let effects: [(&str, &str); 2] = [("escala", tr!("Efeito Escala")), ("genio", tr!("Efeito Lâmpada Mágica"))];
        let mut effect = effects.iter().position(|e| e.0 == self.config.dock.minimize_effect).unwrap_or(0);
        ui.form_row(tr!("Minimizar usando"), LW, |ui| {
            if ui.popup(&mut effect, &effects.iter().map(|e| e.1.to_string()).collect::<Vec<_>>(), 200.0) {
                self.config.dock.minimize_effect = effects[effect].0.into();
                self.save();
            }
        });
        ui.space(6.0);
        let mut anim = self.config.dock.animate_launch;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut anim, tr!("Animar a abertura de aplicativos")) {
                self.config.dock.animate_launch = anim;
                self.save();
            }
        });
        let mut hide = self.config.dock.autohide;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut hide, tr!("Ocultar e mostrar o Dock automaticamente")) {
                self.config.dock.autohide = hide;
                self.save();
            }
        });
    }

    fn teclado(&mut self, ui: &mut Ui) {
        let mut tab = self.kbd_tab;
        let shown = tab;
        ui.space(4.0);
        // A aba de atalhos cresce com os grupos abertos.
        let open_rows: usize = SHORTCUT_GROUPS.iter().zip(self.open_groups).filter(|(_, o)| *o).map(|((_, a), _)| a.len()).sum();
        let height = match shown {
            1 => 120.0 + SHORTCUT_GROUPS.len() as f32 * 24.0 + open_rows as f32 * 30.0,
            2 => 380.0,
            _ => 240.0,
        };
        ui.tab_box(&mut tab, &[tr!("Teclado"), tr!("Atalhos de Teclado"), tr!("Menu de Entrada")], height, |ui| match shown {
            0 => {
                const LW: f32 = 170.0;
                let mut delay = self.config.keyboard.repeat_delay as f32;
                let mut rate = self.config.keyboard.repeat_rate as f32;
                ui.form_row(tr!("Velocidade de repetição"), LW, |ui| {
                    let t = Ticks { count: 8, labels: &[(0.0, tr!("Lenta")), (1.0, tr!("Rápida"))], snap: false };
                    if ui.slider_ticks(&mut rate, 2.0, 60.0, 220.0, &t, true) {
                        self.config.keyboard.repeat_rate = rate.round() as u32;
                        self.save();
                    }
                });
                ui.space(6.0);
                ui.form_row(tr!("Atraso até repetir"), LW, |ui| {
                    // Invertido: à direita = atraso menor.
                    let mut inv = 1000.0 - delay;
                    let t = Ticks { count: 6, labels: &[(0.0, tr!("Longo")), (1.0, tr!("Curto"))], snap: false };
                    if ui.slider_ticks(&mut inv, 0.0, 850.0, 220.0, &t, true) {
                        delay = 1000.0 - inv;
                        self.config.keyboard.repeat_delay = delay.round() as u32;
                        self.save();
                    }
                });
                ui.space(8.0);
                ui.form_row(tr!("Teste aqui"), LW, |ui| {
                    let mut test = TEST.with(|t| t.borrow().clone());
                    if ui.text_field(&mut test, 220.0) {
                        TEST.with(|t| *t.borrow_mut() = test);
                    }
                });
                ui.space(8.0);
                let names: Vec<String> = LAYOUTS.iter().map(|l| tr(l.2).to_string()).collect();
                let mut cur = LAYOUTS
                    .iter()
                    .position(|l| l.0 == self.config.keyboard.layout && l.1 == self.config.keyboard.variant)
                    .unwrap_or(0);
                ui.form_row(tr!("Layout do teclado"), LW, |ui| {
                    if ui.popup(&mut cur, &names, 240.0) {
                        self.config.keyboard.layout = LAYOUTS[cur].0.into();
                        self.config.keyboard.variant = LAYOUTS[cur].1.into();
                        self.save();
                    }
                });
                ui.form_note(LW, tr!("Vale na hora para todos os apps."));
            }
            1 => self.atalhos(ui),
            _ => self.menu_entrada(ui),
        });
        self.kbd_tab = tab;
    }

    fn nested(&self) -> bool {
        self.outputs.as_ref().is_none_or(|o| o.nested)
    }

    fn monitor(&mut self, ui: &mut Ui) {
        const LW: f32 = 150.0;
        let nested = self.nested();
        let outputs = self.outputs.clone().unwrap_or_default();
        // O monitor sendo ajustado: o principal (barra de menu) ou, estendendo a
        // mesa, outro escolhido em "Ajustar".
        let primary = outputs.outputs.iter().find(|o| o.primary).or_else(|| outputs.outputs.iter().find(|o| o.active)).cloned().unwrap_or_default();
        let extend = outputs.outputs.len() > 1 && self.config.monitor.arrangement == "estender";
        let active = outputs.outputs.iter().find(|o| extend && o.name == self.mon_target && o.active).cloned().unwrap_or_else(|| primary.clone());
        let is_primary = active.name == primary.name;
        let mut mode_cfg = if is_primary { self.config.monitor.mode.clone() } else { self.config.monitor.others.get(&active.name).map(|o| o.mode.clone()).unwrap_or_default() };
        let mut mode_changed = false;
        if nested {
            ui.small(tr!("O Glove está numa janela dentro de outro desktop: a resolução é a da janela."));
            ui.small(tr!("O que for escolhido aqui vale quando você entrar na sessão Glove."));
            ui.space(10.0);
        } else {
            if extend {
                let names: Vec<String> = outputs.outputs.iter().filter(|o| o.active).map(|o| if o.name == primary.name { tr!("{} (principal)", o.description) } else { o.description.clone() }).collect();
                let ids: Vec<String> = outputs.outputs.iter().filter(|o| o.active).map(|o| o.name.clone()).collect();
                let mut cur = ids.iter().position(|i| *i == active.name).unwrap_or(0);
                ui.form_row(tr!("Ajustar o monitor"), LW, |ui| {
                    if ui.popup(&mut cur, &names, 260.0) {
                        self.mon_target = ids[cur].clone();
                    }
                });
                ui.space(6.0);
            }
            // Resoluções (sem repetir as frequências) à esquerda; o resto à direita.
            let mut sizes: Vec<(u32, u32)> = Vec::new();
            for m in &active.modes {
                if let Some((w, h, _)) = glove_config::parse_mode(m) {
                    if !sizes.contains(&(w, h)) {
                        sizes.push((w, h));
                    }
                }
            }
            let current = glove_config::parse_mode(if mode_cfg.is_empty() { &active.current } else { &mode_cfg });
            let mut rows = vec![tr!("Recomendada ({})", mode_label(&active.preferred).split(',').next().unwrap_or(""))];
            rows.extend(sizes.iter().map(|(w, h)| format!("{w} × {h}")));
            let mut sel = if mode_cfg.is_empty() { Some(0) } else { current.and_then(|(w, h, _)| sizes.iter().position(|s| *s == (w, h))).map(|i| i + 1) };
            ui.horizontal(|ui| {
                ui.spacing(18.0);
                ui.column(220.0, |ui| {
                    ui.label(tr!("Resoluções:"));
                    if ui.list(&mut sel, &rows, 220.0, 180.0) {
                        self.ask_confirm();
                        mode_changed = true;
                        mode_cfg = match sel {
                            Some(i) if i > 0 => {
                                // A frequência mais alta da resolução escolhida.
                                let (w, h) = sizes[i - 1];
                                active.modes.iter().filter(|m| glove_config::parse_mode(m).is_some_and(|(a, b, _)| (a, b) == (w, h))).max_by_key(|m| glove_config::parse_mode(m).map(|x| x.2)).cloned().unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                    }
                });
                ui.column(ui.available_width(), |ui| {
                    ui.space(20.0);
                    if outputs.outputs.len() > 1 && is_primary {
                        let mut names = vec![tr!("Automático").to_string()];
                        let mut ids = vec![String::new()];
                        for o in &outputs.outputs {
                            names.push(o.description.clone());
                            ids.push(o.name.clone());
                        }
                        let mut cur = ids.iter().position(|i| *i == self.config.monitor.output).unwrap_or(0);
                        ui.label(tr!("Monitor principal (barra de menu):"));
                        if ui.popup(&mut cur, &names, 220.0) {
                            self.ask_confirm();
                            self.config.monitor.output = ids[cur].clone();
                            self.config.monitor.mode.clear();
                            mode_cfg.clear();
                            self.mon_target.clear();
                            self.save();
                        }
                        ui.space(6.0);
                    }
                    // Frequências da resolução em uso.
                    if let (Some((w, h, _)), false) = (current, mode_cfg.is_empty()) {
                        let modes: Vec<&String> = active.modes.iter().filter(|m| glove_config::parse_mode(m).is_some_and(|(a, b, _)| (a, b) == (w, h))).collect();
                        let names: Vec<String> = modes.iter().map(|m| glove_config::parse_mode(m).map(|x| tr!("{} Hz", x.2)).unwrap_or_default()).collect();
                        let mut cur = modes.iter().position(|m| **m == mode_cfg).unwrap_or(0);
                        ui.label(tr!("Taxa de atualização:"));
                        if ui.popup(&mut cur, &names, 120.0) {
                            self.ask_confirm();
                            mode_cfg = modes[cur].clone();
                            mode_changed = true;
                        }
                        ui.space(6.0);
                    }
                    if !active.current.is_empty() {
                        let t = tr!("Em uso: {}", mode_label(&active.current));
                        ui.small(&t);
                    }
                });
            });
            if mode_changed {
                if is_primary {
                    self.config.monitor.mode = mode_cfg.clone();
                } else {
                    self.config.monitor.others.entry(active.name.clone()).or_default().mode = mode_cfg.clone();
                }
                self.save();
            }
            ui.space(10.0);
            // Organização: mais de um monitor ligado ao computador.
            if outputs.outputs.len() > 1 {
                let arr: [(&str, &str); 3] = [("estender", tr!("Estender a mesa")), ("espelhar", tr!("Espelhar (a mesma imagem)")), ("um", tr!("Usar só o principal"))];
                let sides: [(&str, &str); 3] = [("direita", tr!("À direita do principal")), ("esquerda", tr!("À esquerda do principal")), ("abaixo", tr!("Abaixo do principal"))];
                let names: Vec<String> = arr.iter().map(|a| a.1.to_string()).collect();
                let mut cur = arr.iter().position(|a| a.0 == self.config.monitor.arrangement).unwrap_or(0);
                ui.form_row(tr!("Com mais monitores"), LW, |ui| {
                    if ui.popup(&mut cur, &names, 220.0) {
                        self.ask_confirm();
                        self.config.monitor.arrangement = arr[cur].0.into();
                        self.save();
                    }
                });
                if self.config.monitor.arrangement == "estender" {
                    let names: Vec<String> = sides.iter().map(|a| a.1.to_string()).collect();
                    let mut cur = sides.iter().position(|a| a.0 == self.config.monitor.side).unwrap_or(0);
                    ui.form_row(tr!("Os outros ficam"), LW, |ui| {
                        if ui.popup(&mut cur, &names, 220.0) {
                            self.config.monitor.side = sides[cur].0.into();
                            self.save();
                        }
                    });
                    ui.form_note(LW, tr!("A barra de menu e o Dock ficam no principal; arraste janelas para os outros."));
                } else if self.config.monitor.arrangement == "espelhar" {
                    ui.form_note(LW, tr!("Os monitores usam a maior resolução que todos aceitam."));
                }
                ui.space(10.0);
            }
        }
        let auto = active.auto_scale.max(1.0);
        let names: Vec<String> = SCALES.iter().map(|v| if *v == 0.0 { tr!("Automático ({})", scale_label(auto)) } else { scale_label(*v) }).collect();
        let scale_now = if is_primary { self.config.monitor.scale } else { self.config.monitor.others.get(&active.name).map_or(0.0, |o| o.scale) };
        let mut cur = SCALES.iter().position(|v| *v == scale_now).unwrap_or(0);
        ui.form_row(tr!("Tamanho da interface"), LW, |ui| {
            if ui.popup(&mut cur, &names, 180.0) {
                if is_primary {
                    self.config.monitor.scale = SCALES[cur];
                } else {
                    self.config.monitor.others.entry(active.name.clone()).or_default().scale = SCALES[cur];
                }
                self.save();
            }
        });
        ui.form_note(LW, tr!("Maior = textos e janelas maiores. Automático segue a densidade da tela."));
        ui.space(6.0);
        let mut extra = self.config.monitor.menu_extra;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut extra, tr!("Mostrar monitores na barra de menu")) {
                self.config.monitor.menu_extra = extra;
                self.save();
            }
        });

        if let Some((_, _, until)) = &self.confirm {
            let left = until.saturating_duration_since(Instant::now()).as_secs() + 1;
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            let t = tr!("Manter esta resolução? Volta à anterior em {} s.", left);
            ui.label(&t);
            let (mut keep, mut back) = (false, false);
            ui.horizontal(|ui| {
                keep = ui.default_button(tr!("Manter"));
                back = ui.button(tr!("Voltar"));
            });
            if keep {
                self.confirm = None;
                self.confirm_arrangement = None;
                self.confirm_others = None;
            } else if back {
                self.revert();
            }
        }
    }

    /// Guarda o monitor e a resolução atuais para poder voltar.
    fn ask_confirm(&mut self) {
        if self.confirm.is_none() {
            self.confirm_arrangement = Some(self.config.monitor.arrangement.clone());
            self.confirm_others = Some(self.config.monitor.others.clone());
            let m = &self.config.monitor;
            self.confirm = Some((m.output.clone(), m.mode.clone(), Instant::now() + Duration::from_secs(CONFIRM_SECS)));
        } else if let Some(c) = self.confirm.as_mut() {
            c.2 = Instant::now() + Duration::from_secs(CONFIRM_SECS);
        }
    }

    fn revert(&mut self) {
        if let Some((output, mode, _)) = self.confirm.take() {
            self.config.monitor.output = output;
            self.config.monitor.mode = mode;
            if let Some(a) = self.confirm_arrangement.take() {
                self.config.monitor.arrangement = a;
            }
            if let Some(o) = self.confirm_others.take() {
                self.config.monitor.others = o;
            }
            self.save();
        }
    }

    /// Velocidade do clique duplo: slider de 0 (muito lenta, 900 ms) a 1 (rápida, 200 ms).
    fn double_click_row(&mut self, ui: &mut Ui, lw: f32) {
        let mut v = (900.0 - self.config.pointer.double_click_ms as f32) / 700.0;
        ui.form_row(tr!("Velocidade do clique duplo"), lw, |ui| {
            let t = Ticks { count: 8, labels: &[(0.0, tr!("Muito lenta")), (1.0, tr!("Rápida"))], snap: false };
            if ui.slider_ticks(&mut v, 0.0, 1.0, 240.0, &t, true) {
                self.config.pointer.double_click_ms = (900.0 - v * 700.0).round() as u32;
                self.save();
            }
        });
        // Área de teste: o clique duplo troca o texto.
        ui.form_row("", lw, |ui| {
            let r = ui.alloc(240.0, 30.0);
            let ok = TEST_DBL.with(|t| t.get());
            ui.c.fill(rrect(r.x, r.y, r.w, r.h, 4.0), &solid(if ok { rgba(220, 238, 214, 255) } else { rgba(255, 255, 255, 200) }));
            ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 4.0), &solid(rgba(0, 0, 0, 60)), 1.0);
            let t = if ok { tr!("Clique duplo reconhecido!") } else { tr!("Clique duas vezes aqui para testar") };
            let tw = ui.text_width(t, 11.0, false);
            ui.fonts.draw(ui.c, t, r.x + (r.w - tw) / 2.0, r.y + 19.0, 11.0, false, rgba(60, 60, 66, 255));
            let h = ui.hit(r);
            if h.double {
                TEST_DBL.with(|t| t.set(!t.get()));
            }
        });
    }

    fn mouse(&mut self, ui: &mut Ui) {
        const LW: f32 = 210.0;
        let p = self.config.pointer.clone();
        ui.space(6.0);
        let mut speed = p.speed as f32;
        ui.form_row(tr!("Velocidade de rastreamento"), LW, |ui| {
            let t = Ticks { count: 10, labels: &[(0.0, tr!("Lenta")), (1.0, tr!("Rápida"))], snap: false };
            if ui.slider_ticks(&mut speed, -1.0, 1.0, 240.0, &t, true) {
                self.config.pointer.speed = (speed as f64 * 20.0).round() / 20.0;
                self.save();
            }
        });
        ui.space(6.0);
        let mut scroll = p.scroll_speed.clamp(0.25, 3.0) as f32;
        ui.form_row(tr!("Velocidade de rolagem"), LW, |ui| {
            let t = Ticks { count: 8, labels: &[(0.0, tr!("Lenta")), (1.0, tr!("Rápida"))], snap: false };
            if ui.slider_ticks(&mut scroll, 0.25, 3.0, 240.0, &t, true) {
                self.config.pointer.scroll_speed = (scroll as f64 * 20.0).round() / 20.0;
                self.save();
            }
        });
        ui.space(6.0);
        self.double_click_row(ui, LW);
        ui.space(10.0);
        let mut primary = usize::from(p.left_handed);
        ui.form_row(tr!("Botão principal do mouse"), LW, |ui| {
            ui.horizontal(|ui| {
                ui.spacing(14.0);
                if ui.radio_group(&mut primary, &[tr!("Esquerdo"), tr!("Direito")]) {
                    self.config.pointer.left_handed = primary == 1;
                    self.save();
                }
            });
        });
        ui.space(4.0);
        let mut acc = p.acceleration;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut acc, tr!("Aceleração (anda mais quando o mouse se move rápido)")) {
                self.config.pointer.acceleration = acc;
                self.save();
            }
            let mut nat = p.natural_scroll;
            if ui.checkbox(&mut nat, tr!("Rolagem natural (o conteúdo acompanha a roda)")) {
                self.config.pointer.natural_scroll = nat;
                self.save();
            }
        });
        if self.nested() {
            ui.space(8.0);
            ui.separator();
            ui.space(4.0);
            ui.small(tr!("Dentro de outro desktop só a rolagem e o clique duplo valem agora; o resto vale na sessão Glove."));
        }
    }

    fn trackpad(&mut self, ui: &mut Ui) {
        const LW: f32 = 210.0;
        let t = self.config.touchpad.clone();
        ui.space(6.0);
        let mut speed = t.speed as f32;
        ui.form_row(tr!("Velocidade de rastreamento"), LW, |ui| {
            let ticks = Ticks { count: 10, labels: &[(0.0, tr!("Lenta")), (1.0, tr!("Rápida"))], snap: false };
            if ui.slider_ticks(&mut speed, -1.0, 1.0, 240.0, &ticks, true) {
                self.config.touchpad.speed = (speed as f64 * 20.0).round() / 20.0;
                self.save();
            }
        });
        ui.space(6.0);
        self.double_click_row(ui, LW);
        ui.space(10.0);
        ui.form_row(tr!("Gestos do trackpad"), LW, |ui| {
            let mut tap = t.tap_to_click;
            if ui.checkbox(&mut tap, tr!("Tocar para clicar")) {
                self.config.touchpad.tap_to_click = tap;
                self.save();
            }
            let mut drag = t.tap_drag;
            if ui.checkbox(&mut drag, tr!("Tocar duas vezes e arrastar")) {
                self.config.touchpad.tap_drag = drag;
                self.save();
            }
            let mut nat = t.natural_scroll;
            if ui.checkbox(&mut nat, tr!("Rolagem natural (o conteúdo acompanha os dedos)")) {
                self.config.touchpad.natural_scroll = nat;
                self.save();
            }
        });
        ui.space(6.0);
        let mut v = if t.two_finger_right_click { 0 } else { 1 };
        ui.form_row(tr!("Clique secundário"), LW, |ui| {
            if ui.radio_group(&mut v, &[tr!("Com dois dedos"), tr!("No canto inferior direito")]) {
                self.config.touchpad.two_finger_right_click = v == 0;
                self.save();
            }
        });
        ui.space(6.0);
        let mut dwt = t.disable_while_typing;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut dwt, tr!("Ignorar o trackpad enquanto digito")) {
                self.config.touchpad.disable_while_typing = dwt;
                self.save();
            }
        });
        if self.nested() {
            ui.space(8.0);
            ui.separator();
            ui.space(4.0);
            ui.small(tr!("Dentro de outro desktop o trackpad segue o desktop de lá; isto vale na sessão Glove."));
        }
    }

    /// Menu de Entrada: os layouts que a barra e a tecla de troca oferecem.
    fn menu_entrada(&mut self, ui: &mut Ui) {
        ui.small(tr!("Marque os layouts que você usa. A barra de menu e Super+Alt+espaço passam por eles."));
        ui.space(6.0);
        for (l, v, name, badge) in LAYOUTS {
            let id = glove_config::layout_id(l, v);
            let mut on = self.config.keyboard.input_menu.contains(&id);
            let label = format!("{}  ({badge})", tr(name));
            ui.push_id(&id, |ui| {
                if ui.checkbox(&mut on, &label) {
                    let list = &mut self.config.keyboard.input_menu;
                    if on {
                        // Mantém a ordem da tabela.
                        list.push(id.clone());
                        list.sort_by_key(|x| LAYOUTS.iter().position(|t| glove_config::layout_id(t.0, t.1) == *x).unwrap_or(99));
                    } else {
                        list.retain(|x| *x != id);
                    }
                    self.save();
                }
            });
        }
        ui.space(8.0);
        let mut show = self.config.keyboard.show_input_menu;
        if ui.checkbox(&mut show, tr!("Mostrar o Menu de Entrada na barra de menu")) {
            self.config.keyboard.show_input_menu = show;
            self.save();
        }
    }

    fn atalhos(&mut self, ui: &mut Ui) {
        ui.small(tr!("Clique num atalho e pressione a nova combinação (Esc cancela)."));
        ui.space(4.0);
        for (gi, (group, actions)) in SHORTCUT_GROUPS.iter().enumerate() {
            ui.push_id(group, |ui| {
                ui.disclosure(&mut self.open_groups[gi], tr(group));
            });
            if !self.open_groups[gi] {
                continue;
            }
            for action in actions.iter() {
                let label = glove_config::action_label(action);
                let mut keys = self.config.shortcut(action).unwrap_or("").to_string();
                let mut changed = false;
                ui.push_id(action, |ui| {
                    ui.horizontal(|ui| {
                        ui.label_right(label, 200.0);
                        changed = ui.shortcut_field(&mut keys, 220.0);
                    });
                });
                if changed {
                    match self.config.shortcuts.iter_mut().find(|s| s.action == *action) {
                        Some(s) => s.keys = keys,
                        None => self.config.shortcuts.push(glove_config::Shortcut { action: action.to_string(), keys }),
                    }
                    self.save();
                }
            }
            ui.space(2.0);
        }
        ui.space(6.0);
        if ui.button(tr!("Restaurar padrões")) {
            self.config.shortcuts = glove_config::default_shortcuts();
            self.save();
        }
    }
}

/// Passos dos sliders de energia, em minutos (0 = nunca, no fim).
const ENERGY_STEPS: [u32; 14] = [1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 0];

fn minutes_label(m: u32) -> String {
    match m {
        0 => tr!("Nunca").into(),
        m if m < 60 => tr!("{} min", m),
        60 => tr!("1 hora").into(),
        m if m % 60 == 0 => tr!("{} horas", m / 60),
        m => tr!("{} h {} min", m / 60, m % 60),
    }
}

/// Tem bateria? (como o compositor: a primeira presente em /sys/class/power_supply).
fn has_battery() -> bool {
    let root = std::env::var_os("GLOVE_POWER_SUPPLY").map(std::path::PathBuf::from).unwrap_or_else(|| "/sys/class/power_supply".into());
    std::fs::read_dir(root).into_iter().flatten().flatten().any(|e| std::fs::read_to_string(e.path().join("type")).is_ok_and(|t| t.trim() == "Battery"))
}

/// Está na bateria agora (há bateria e nenhum adaptador ligado)?
fn on_battery() -> bool {
    let root = std::env::var_os("GLOVE_POWER_SUPPLY").map(std::path::PathBuf::from).unwrap_or_else(|| "/sys/class/power_supply".into());
    let read = |p: std::path::PathBuf| std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()).collect();
    has_battery() && !entries.iter().any(|d| read(d.join("type")) == "Mains" && read(d.join("online")) == "1")
}

impl Prefs {
    /// Mídia Removível: o que fazer quando um pendrive ou disco é conectado.
    fn midia(&mut self, ui: &mut Ui) {
        const LW: f32 = 230.0;
        ui.space(8.0);
        let opts = [tr!("Abrir no Arquivos").to_string(), tr!("Não fazer nada").to_string()];
        let ids = ["abrir", "nada"];
        let mut cur = ids.iter().position(|i| *i == self.config.removable_action).unwrap_or(0);
        ui.form_row(tr!("Ao conectar um pendrive ou disco"), LW, |ui| {
            if ui.popup(&mut cur, &opts, 200.0) {
                self.config.removable_action = ids[cur].to_string();
                self.save();
            }
        });
        ui.form_note(LW, tr!("Ele também aparece na mesa, ao lado do Disco."));
        ui.space(14.0);
        // CDs e DVDs (como o painel do Tiger).
        let data_opts = [tr!("Abrir no Arquivos").to_string(), tr!("Não fazer nada").to_string()];
        let data_ids = ["abrir", "nada"];
        let mut cur = data_ids.iter().position(|i| *i == self.config.media.data_disc).unwrap_or(0);
        ui.form_row(tr!("Ao inserir um CD ou DVD de dados"), LW, |ui| {
            if ui.popup(&mut cur, &data_opts, 200.0) {
                self.config.media.data_disc = data_ids[cur].to_string();
                self.save();
            }
        });
        ui.space(6.0);
        let dvd_opts = [tr!("Abrir no Vídeo").to_string(), tr!("Abrir no Arquivos").to_string(), tr!("Não fazer nada").to_string()];
        let dvd_ids = ["video", "abrir", "nada"];
        let mut cur = dvd_ids.iter().position(|i| *i == self.config.media.video_dvd).unwrap_or(0);
        ui.form_row(tr!("Ao inserir um DVD de vídeo"), LW, |ui| {
            if ui.popup(&mut cur, &dvd_opts, 200.0) {
                self.config.media.video_dvd = dvd_ids[cur].to_string();
                self.save();
            }
        });
    }

    /// Segurança: senha ao voltar, sair por inatividade e o firewall (ufw).
    fn seguranca(&mut self, ui: &mut Ui) {
        ui.space(8.0);
        let mut pw = self.config.energy.require_password;
        if ui.checkbox(&mut pw, tr!("Pedir a senha ao sair do repouso ou do protetor de tela")) {
            self.config.energy.require_password = pw;
            self.save();
        }
        ui.space(10.0);
        let mut on = self.config.security.logout_minutes > 0;
        if ui.checkbox(&mut on, tr!("Encerrar a sessão depois de um tempo sem uso")) {
            self.config.security.logout_minutes = if on { 60 } else { 0 };
            self.save();
        }
        if on {
            ui.indent(20.0, |ui| {
                if let Some(m) = Self::minutes_slider(ui, tr!("Depois de:"), self.config.security.logout_minutes) {
                    self.config.security.logout_minutes = m;
                    self.save();
                }
                ui.small(tr!("Antes de sair, aparece o aviso com a contagem (mexer no mouse cancela)."));
            });
        }
        ui.space(14.0);
        ui.separator();
        ui.space(8.0);
        ui.heading("Firewall");
        ui.space(4.0);
        let installed = std::path::Path::new("/usr/sbin/ufw").exists() || std::path::Path::new("/sbin/ufw").exists();
        if !installed {
            ui.small(tr!("O firewall (ufw) não está instalado. Para instalar: sudo apt install ufw"));
            return;
        }
        let enabled = std::fs::read_to_string("/etc/ufw/ufw.conf").ok().is_some_and(|t| t.lines().any(|l| l.trim().eq_ignore_ascii_case("ENABLED=yes")));
        ui.label(if enabled { tr!("Ligado: as conexões que chegam de fora são bloqueadas (as que você abre passam).") } else { tr!("Desligado: o computador aceita conexões de fora.") });
        ui.space(6.0);
        if ui.button(if enabled { tr!("Desligar o firewall") } else { tr!("Ligar o firewall") }) {
            let arg = if enabled { "disable" } else { "enable" };
            // Pede a senha de administrador pelo polkit.
            std::thread::spawn(move || {
                let _ = std::process::Command::new("pkexec").args(["ufw", arg]).status();
            });
        }
        ui.wake_after(Duration::from_secs(2));
    }

    /// Busca Rápida: o que aparece nos resultados e as pastas que ficam de fora.
    fn busca(&mut self, ui: &mut Ui) {
        ui.space(8.0);
        ui.label(tr!("Mostrar nos resultados:"));
        ui.space(4.0);
        let mut sp = self.config.search.clone();
        let mut changed = false;
        ui.indent(16.0, |ui| {
            changed |= ui.checkbox(&mut sp.apps, tr!("Aplicativos"));
            changed |= ui.checkbox(&mut sp.prefs, tr!("Preferências do Sistema"));
            changed |= ui.checkbox(&mut sp.files, tr!("Pastas e arquivos"));
        });
        ui.space(12.0);
        ui.label(tr!("Privacidade: a busca não olha estas pastas."));
        ui.space(4.0);
        let rows: Vec<String> = sp.excluded.clone();
        ui.indent(16.0, |ui| {
            ui.list(&mut self.busca_sel, &rows, 420.0, 110.0);
            ui.space(6.0);
            ui.horizontal(|ui| {
                ui.text_field(&mut self.busca_nova, 300.0);
                ui.spacing(8.0);
                if ui.button(tr!("Adicionar")) && !self.busca_nova.trim().is_empty() {
                    let t = self.busca_nova.trim();
                    let p = match t.strip_prefix("~/") {
                        Some(rest) => std::env::var("HOME").map(|h| format!("{h}/{rest}")).unwrap_or_else(|_| t.to_string()),
                        None => t.to_string(),
                    };
                    if !sp.excluded.contains(&p) {
                        sp.excluded.push(p);
                        changed = true;
                    }
                    self.busca_nova.clear();
                }
                ui.spacing(8.0);
                ui.set_enabled(self.busca_sel.is_some());
                if ui.button(tr!("Remover")) {
                    if let Some(i) = self.busca_sel.take() {
                        if i < sp.excluded.len() {
                            sp.excluded.remove(i);
                            changed = true;
                        }
                    }
                }
                ui.set_enabled(true);
            });
            ui.small("Escreva o caminho da pasta (ex.: ~/Particular) e clique em Adicionar.");
        });
        if changed {
            self.config.search = sp;
            self.save();
        }
    }

    /// Exposé e Painel: os cantos ativos e as teclas.
    fn expose(&mut self, ui: &mut Ui) {
        ui.space(8.0);
        self.hot_corners_ui(ui);
        ui.space(14.0);
        ui.separator();
        ui.space(8.0);
        ui.label(tr!("Atalhos de teclado:"));
        ui.space(4.0);
        for action in ["expose_all", "expose_app", "expose_desktop", "dashboard"] {
            let label = glove_config::action_label(action);
            let mut keys = self.config.shortcut(action).unwrap_or("").to_string();
            let mut changed = false;
            ui.push_id(action, |ui| {
                ui.horizontal(|ui| {
                    ui.label_right(label, 220.0);
                    changed = ui.shortcut_field(&mut keys, 200.0);
                });
            });
            if changed {
                match self.config.shortcuts.iter_mut().find(|s| s.action == action) {
                    Some(s) => s.keys = keys,
                    None => self.config.shortcuts.push(glove_config::Shortcut { action: action.to_string(), keys }),
                }
                self.save();
            }
        }
    }

    fn acesso(&mut self, ui: &mut Ui) {
        const LW: f32 = 150.0;
        let mut tab = self.access_tab;
        let shown = tab;
        ui.space(4.0);
        ui.tab_box(&mut tab, &[tr!("Visão"), tr!("Mouse")], 290.0, |ui| match shown {
            0 => {
                let mut a = self.config.access.clone();
                let mut zoom = usize::from(!a.zoom);
                ui.form_row(tr!("Zoom"), LW, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing(14.0);
                        if ui.radio_group(&mut zoom, &[tr!("Ligado"), tr!("Desligado")]) {
                            a.zoom = zoom == 0;
                        }
                    });
                });
                let mut lvl = a.zoom_level;
                ui.form_row(tr!("Ampliação"), LW, |ui| {
                    let t = Ticks { count: 8, labels: &[(0.0, tr!("1,5×")), (1.0, "8×")], snap: false };
                    if ui.slider_ticks(&mut lvl, 1.5, 8.0, 260.0, &t, a.zoom) {
                        a.zoom_level = (lvl * 4.0).round() / 4.0;
                    }
                });
                ui.form_note(LW, tr!("Super+Alt+8 liga e desliga; Super+Alt+= e Super+Alt+− aproximam e afastam."));
                ui.space(10.0);
                let mut inv = usize::from(a.invert);
                ui.form_row(tr!("Monitor"), LW, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing(14.0);
                        if ui.radio_group(&mut inv, &[tr!("Preto no branco"), tr!("Branco no preto")]) {
                            a.invert = inv == 1;
                        }
                    });
                    ui.checkbox(&mut a.grayscale, tr!("Usar escala de cinza"));
                });
                let mut ct = a.contrast;
                ui.form_row(tr!("Melhorar o contraste"), LW, |ui| {
                    let t = Ticks { count: 5, labels: &[(0.0, "Normal"), (1.0, tr!("Máximo"))], snap: false };
                    if ui.slider_ticks(&mut ct, 0.0, 1.0, 260.0, &t, true) {
                        a.contrast = (ct * 20.0).round() / 20.0;
                    }
                });
                if a != self.config.access {
                    self.config.access = a;
                    self.save();
                }
            }
            _ => {
                let mut k = self.config.access.cursor_size;
                ui.form_row(tr!("Tamanho do cursor"), LW, |ui| {
                    let t = Ticks { count: 7, labels: &[(0.0, "Normal"), (1.0, tr!("Grande"))], snap: false };
                    if ui.slider_ticks(&mut k, 1.0, 4.0, 260.0, &t, true) {
                        self.config.access.cursor_size = (k * 4.0).round() / 4.0;
                        self.save();
                    }
                });
                // Prévia: a seta no tamanho escolhido.
                ui.form_row("", LW, |ui| {
                    let s = 24.0 * self.config.access.cursor_size;
                    let r = ui.alloc(s.max(24.0), s.max(24.0));
                    if let Some(c) = glove_kit::paint::chrome::cursor_arrow(ui.c.scale * self.config.access.cursor_size) {
                        ui.c.draw_canvas(&c, r.x, r.y);
                    }
                });
                if self.nested() {
                    ui.form_note(LW, tr!("Dentro de outro desktop a seta é a dele; isto vale na sessão Glove."));
                }
            }
        });
        self.access_tab = tab;
    }

    /// Slider de minutos (com "Nunca" no fim); devolve o novo valor se mudou.
    fn minutes_slider(ui: &mut Ui, label: &str, minutes: u32) -> Option<u32> {
        let mut idx = ENERGY_STEPS.iter().position(|m| *m == minutes).unwrap_or_else(|| ENERGY_STEPS.iter().position(|m| *m >= minutes).unwrap_or(ENERGY_STEPS.len() - 1)) as f32;
        let mut out = None;
        ui.label(label);
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                let t = Ticks { count: ENERGY_STEPS.len(), labels: &[(0.0, "1 min"), (5.0 / 13.0, "15 min"), (9.0 / 13.0, "1 h"), (1.0, tr!("Nunca"))], snap: true };
                if ui.slider_ticks(&mut idx, 0.0, (ENERGY_STEPS.len() - 1) as f32, 380.0, &t, true) {
                    out = Some(ENERGY_STEPS[idx.round() as usize]);
                }
                let v = minutes_label(ENERGY_STEPS[idx.round() as usize]);
                ui.label(&v);
            });
        });
        out
    }

    fn energia(&mut self, ui: &mut Ui) {
        let battery = has_battery();
        if battery {
            ui.horizontal(|ui| {
                ui.label_right(tr!("Configurações para:"), 200.0);
                let names = vec![tr!("Adaptador de energia").to_string(), tr!("Bateria").to_string()];
                ui.popup(&mut self.energy_source, &names, 200.0);
            });
            ui.space(8.0);
        }
        let on_bat = battery && self.energy_source == 1;
        let mut tab = self.energy_tab;
        let shown = tab;
        ui.tab_box(&mut tab, &[tr!("Repouso"), tr!("Opções")], 200.0, |ui| match shown {
            0 => {
                let (screen, sleep) = self.config.energy.timeouts(on_bat);
                if let Some(m) = Self::minutes_slider(ui, tr!("Colocar o computador em repouso quando inativo por:"), sleep) {
                    if on_bat { self.config.energy.battery_sleep_minutes = m } else { self.config.energy.sleep_minutes = m }
                    self.save();
                }
                ui.space(10.0);
                if let Some(m) = Self::minutes_slider(ui, tr!("Apagar a tela quando inativo por:"), screen) {
                    if on_bat { self.config.energy.battery_screen_off_minutes = m } else { self.config.energy.screen_off_minutes = m }
                    self.save();
                }
                // Aviso da era: a tela não apaga depois do repouso.
                let (screen, sleep) = self.config.energy.timeouts(on_bat);
                if sleep > 0 && (screen == 0 || screen >= sleep) {
                    ui.space(6.0);
                    ui.small(tr!("A tela não vai apagar antes: o computador entra em repouso primeiro."));
                }
            }
            _ => {
                let e = self.config.energy.clone();
                let mut pw = e.require_password;
                if ui.checkbox(&mut pw, tr!("Pedir senha ao sair do repouso ou da tela apagada")) {
                    self.config.energy.require_password = pw;
                    self.save();
                }
                if battery {
                    let mut dim = e.dim_on_battery;
                    if ui.checkbox(&mut dim, tr!("Diminuir um pouco o brilho da tela ao usar a bateria")) {
                        self.config.energy.dim_on_battery = dim;
                        self.save();
                    }
                    let mut menu = e.battery_menu;
                    if ui.checkbox(&mut menu, tr!("Mostrar o estado da bateria na barra de menu")) {
                        self.config.energy.battery_menu = menu;
                        self.save();
                    }
                    let mut pct = self.config.battery_percent;
                    ui.indent(22.0, |ui| {
                        if ui.checkbox(&mut pct, tr!("Mostrar a porcentagem")) {
                            self.config.battery_percent = pct;
                            self.save();
                        }
                    });
                }
                ui.space(8.0);
                ui.small(tr!("O botão de ligar e a tampa seguem o sistema (logind); fechar a tampa apaga a tela."));
            }
        });
        self.energy_tab = tab;
        ui.space(8.0);
        ui.horizontal(|ui| {
            ui.space(ui.available_width() - 160.0);
            if ui.button(tr!("Restaurar Padrões")) {
                let d = glove_config::Energy::default();
                if on_bat {
                    self.config.energy.battery_screen_off_minutes = d.battery_screen_off_minutes;
                    self.config.energy.battery_sleep_minutes = d.battery_sleep_minutes;
                } else {
                    self.config.energy.screen_off_minutes = d.screen_off_minutes;
                    self.config.energy.sleep_minutes = d.sleep_minutes;
                }
                self.save();
            }
        });
    }

    fn som(&mut self, ui: &mut Ui) {
        const SINK: &str = "@DEFAULT_AUDIO_SINK@";
        const SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";
        let mut tab = self.sound_tab;
        let shown = tab;
        ui.space(4.0);
        ui.tab_box(&mut tab, &[tr!("Efeitos Sonoros"), tr!("Saída"), tr!("Entrada")], 280.0, |ui| match shown {
            0 => {
                ui.label(tr!("Escolher um alerta sonoro:"));
                let rows: Vec<String> = glove_config::sons::ALERTS.iter().map(|a| tr(a).to_string()).collect();
                let mut sel = glove_config::sons::ALERTS.iter().position(|a| *a == self.config.sound.alert);
                let (lx, ly) = ui.cursor();
                let list_r = Rect::new(lx, ly, ui.available_width(), 118.0);
                let again = ui.mouse_pressed() && list_r.contains(ui.pointer());
                if ui.list(&mut sel, &rows, ui.available_width(), 118.0) {
                    if let Some(i) = sel {
                        self.config.sound.alert = rows[i].clone();
                        self.save();
                        glove_config::sons::play(&rows[i], self.config.sound.alert_volume.max(0.3));
                    }
                } else if again {
                    // Clicar no alerta já escolhido toca de novo.
                    glove_config::sons::play(&self.config.sound.alert, self.config.sound.alert_volume.max(0.3));
                }
                ui.space(6.0);
                let mut v = self.config.sound.alert_volume;
                ui.form_row(tr!("Volume do alerta"), 150.0, |ui| {
                    let t = Ticks { count: 2, labels: &[(0.0, tr!("Baixo")), (1.0, "Alto")], snap: false };
                    if ui.slider_ticks(&mut v, 0.0, 1.0, 240.0, &t, true) {
                        self.config.sound.alert_volume = (v * 20.0).round() / 20.0;
                        self.save();
                    }
                });
                let mut fb = self.config.sound.volume_feedback;
                ui.form_row("", 150.0, |ui| {
                    if ui.checkbox(&mut fb, tr!("Tocar retorno ao mudar o volume")) {
                        self.config.sound.volume_feedback = fb;
                        self.save();
                    }
                });
            }
            n => {
                let Some((sinks, sources)) = self.audio.clone() else {
                    ui.small(tr!("O PipeWire não respondeu (o comando wpctl não está disponível)."));
                    return;
                };
                let (list, what) = if n == 1 { (&sinks, tr!("a saída")) } else { (&sources, tr!("a entrada")) };
                let t = tr!("Escolher um dispositivo para {} de som:", what);
                ui.label(&t);
                let rows: Vec<String> = list.iter().map(|d| d.name.clone()).collect();
                let mut sel = list.iter().position(|d| d.default);
                if ui.list(&mut sel, &rows, ui.available_width(), 120.0) {
                    if let Some(i) = sel {
                        som::set_default(list[i].id);
                        if let Some(a) = self.audio.as_mut() {
                            let l = if n == 1 { &mut a.0 } else { &mut a.1 };
                            for (k, d) in l.iter_mut().enumerate() {
                                d.default = k == i;
                            }
                        }
                    }
                }
                if rows.is_empty() {
                    ui.small(tr!("Nenhum dispositivo encontrado."));
                }
                if n == 2 {
                    ui.space(8.0);
                    let mut v = sources.iter().find(|d| d.default).map_or(0.0, |d| d.volume);
                    ui.form_row(tr!("Volume de entrada"), 150.0, |ui| {
                        let t = Ticks { count: 2, labels: &[(0.0, tr!("Baixo")), (1.0, "Alto")], snap: false };
                        if ui.slider_ticks(&mut v, 0.0, 1.0, 240.0, &t, !sources.is_empty()) {
                            som::set_volume(SOURCE, v);
                            if let Some(d) = self.audio.as_mut().and_then(|a| a.1.iter_mut().find(|d| d.default)) {
                                d.volume = v;
                            }
                        }
                    });
                }
            }
        });
        self.sound_tab = tab;
        ui.space(10.0);
        // Volume de saída e mudo, embaixo de todas as abas (como no 10.4).
        let out = self.audio.as_ref().and_then(|a| a.0.iter().find(|d| d.default).cloned());
        let has = out.is_some();
        let (mut vol, mut muted) = out.map_or((0.0, false), |d| (d.volume, d.muted));
        ui.form_row(tr!("Volume de saída"), 150.0, |ui| {
            ui.horizontal(|ui| {
                ui.spacing(14.0);
                if ui.slider_ticks(&mut vol, 0.0, 1.0, 240.0, &Ticks { count: 0, labels: &[], snap: false }, has) {
                    som::set_volume(SINK, vol);
                    if muted && vol > 0.0 {
                        som::set_mute(SINK, false);
                        muted = false;
                    }
                    if let Some(d) = self.audio.as_mut().and_then(|a| a.0.iter_mut().find(|d| d.default)) {
                        d.volume = vol;
                        d.muted = muted;
                    }
                }
                if ui.checkbox(&mut muted, tr!("Mudo")) && has {
                    som::set_mute(SINK, muted);
                    if let Some(d) = self.audio.as_mut().and_then(|a| a.0.iter_mut().find(|d| d.default)) {
                        d.muted = muted;
                    }
                }
            });
        });
        let mut extra = self.config.sound.menu_extra;
        ui.form_row("", 150.0, |ui| {
            if ui.checkbox(&mut extra, tr!("Mostrar volume na barra de menu")) {
                self.config.sound.menu_extra = extra;
                self.save();
            }
        });
    }

    /// Programas que o próprio Glove abre: o Terminal do atalho, o navegador do
    /// app Navegador, onde a Lixeira abre e os apps X11.
    fn padrao_glove(&mut self, ui: &mut Ui) {
        const LW: f32 = 120.0;
        let in_path = |p: &str| std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(p).is_file()));
        ui.space(10.0);
        ui.separator();
        ui.space(6.0);
        ui.label(tr!("Programas que o Glove abre:"));
        ui.space(4.0);
        // Terminal (atalho Abrir Terminal e Super+Enter).
        let mut terms: Vec<(String, String)> = vec![("glove-term".into(), tr!("Terminal (Glove)").into())];
        for (cmd, name) in [("gnome-terminal", tr!("Terminal do GNOME")), ("kitty", "kitty"), ("alacritty", "Alacritty"), ("foot", "foot"), ("wezterm", "WezTerm"), ("konsole", "Konsole"), ("xfce4-terminal", tr!("Terminal do Xfce")), ("tilix", "Tilix"), ("xterm", "XTerm")] {
            if in_path(cmd) {
                terms.push((cmd.into(), name.into()));
            }
        }
        if !terms.iter().any(|t| t.0 == self.config.terminal) && !self.config.terminal.is_empty() {
            terms.push((self.config.terminal.clone(), self.config.terminal.clone()));
        }
        let names: Vec<String> = terms.iter().map(|t| t.1.clone()).collect();
        let mut cur = terms.iter().position(|t| t.0 == self.config.terminal).unwrap_or(0);
        ui.form_row(tr!("Terminal"), LW, |ui| {
            if ui.popup(&mut cur, &names, 260.0) {
                self.config.terminal = terms[cur].0.clone();
                self.save();
            }
        });
        // Navegador do app Navegador (Automático = pela memória do computador).
        let mut browsers: Vec<(String, String)> = vec![(String::new(), tr!("Automático (pela memória)").into())];
        for (cmd, name) in [("firefox", "Firefox"), ("netsurf-gtk3", "NetSurf"), ("chromium", "Chromium"), ("chromium-browser", "Chromium"), ("google-chrome", "Google Chrome"), ("brave-browser", "Brave"), ("epiphany", tr!("Web (GNOME)")), ("falkon", "Falkon")] {
            if in_path(cmd) {
                browsers.push((cmd.into(), name.into()));
            }
        }
        let names: Vec<String> = browsers.iter().map(|t| t.1.clone()).collect();
        let mut cur = browsers.iter().position(|t| t.0 == self.config.navegador).unwrap_or(0);
        ui.form_row(tr!("Navegador"), LW, |ui| {
            if ui.popup(&mut cur, &names, 260.0) {
                self.config.navegador = browsers[cur].0.clone();
                self.save();
            }
        });
        // Lixeira do Dock: no Arquivos do Glove ou em outro gerenciador.
        let mut trash: Vec<(String, String)> = vec![("glove-files ~/.local/share/Trash/files".into(), tr!("Arquivos (Glove)").into())];
        for (cmd, name) in [("nautilus trash:///", "Nautilus"), ("thunar trash:///", "Thunar"), ("dolphin trash:/", "Dolphin"), ("nemo trash:///", "Nemo")] {
            if in_path(cmd.split_whitespace().next().unwrap_or("")) {
                trash.push((cmd.into(), name.into()));
            }
        }
        let names: Vec<String> = trash.iter().map(|t| t.1.clone()).collect();
        let mut cur = trash.iter().position(|t| t.0 == self.config.trash_command).unwrap_or(0);
        ui.form_row(tr!("Lixeira abre no"), LW, |ui| {
            if ui.popup(&mut cur, &names, 260.0) {
                self.config.trash_command = trash[cur].0.clone();
                self.save();
            }
        });
        let mut x11 = self.config.x11_apps;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut x11, tr!("Abrir apps X11 (Xwayland, uns 20 MB a mais)")) {
                self.config.x11_apps = x11;
                self.save();
            }
        });
        ui.form_note(LW, tr!("Os apps X11 ligam ou desligam na próxima sessão."));
    }

    fn terminal(&mut self, ui: &mut Ui) {
        const LW: f32 = 120.0;
        // Temas: amostras clicáveis com o fundo, o texto e algumas cores do tema.
        ui.label(tr!("Tema de cores:"));
        ui.space(4.0);
        let themes = glove_config::term_themes();
        let (sw, sh, gap) = (112.0, 62.0, 10.0);
        let per_row = ((ui.available_width() + gap) / (sw + gap)).floor().max(1.0) as usize;
        let rows = themes.len().div_ceil(per_row);
        let (x0, y0) = ui.cursor();
        let r = ui.alloc(ui.available_width(), rows as f32 * (sh + 20.0 + gap));
        for (i, t) in themes.iter().enumerate() {
            let (col, row) = (i % per_row, i / per_row);
            let bx = Rect::new(x0 + col as f32 * (sw + gap), y0 + row as f32 * (sh + 20.0 + gap), sw, sh);
            let on = self.config.terminal_theme == t.id || (i == 0 && !themes.iter().any(|x| x.id == self.config.terminal_theme));
            let c = |v: (u8, u8, u8)| rgba(v.0, v.1, v.2, 255);
            ui.c.fill(rrect(bx.x, bx.y, bx.w, bx.h, 4.0), &solid(c(t.bg)));
            let ink = c(t.fg);
            ui.fonts.draw(ui.c, "$ ls -la", bx.x + 8.0, bx.y + 18.0, 11.0, false, ink);
            ui.fonts.draw(ui.c, "drwxr-x  glove", bx.x + 8.0, bx.y + 33.0, 11.0, false, c(t.palette[4]));
            for k in 0..6 {
                ui.c.fill_rect(bx.x + 8.0 + k as f32 * 15.0, bx.y + 42.0, 12.0, 10.0, &solid(c(t.palette[k + 1])));
            }
            ui.c.fill_rect(bx.x + 8.0 + 92.0, bx.y + 42.0, 7.0, 10.0, &solid(c(t.cursor)));
            let stroke = if on { ui.theme.accent } else { rgba(120, 120, 126, 255) };
            ui.c.stroke(rrect(bx.x + 0.5, bx.y + 0.5, bx.w - 1.0, bx.h - 1.0, 4.0), &solid(stroke), if on { 3.0 } else { 1.0 });
            let nw = ui.text_width(t.name, 11.0, on);
            let name = ui.fonts.ellipsize(t.name, 11.0, on, sw + 6.0);
            ui.fonts.draw(ui.c, &name, bx.x + (sw - nw.min(sw)) / 2.0, bx.y + sh + 15.0, 11.0, on, rgba(20, 20, 20, 255));
            if ui.hit(Rect::new(bx.x, bx.y, bx.w, bx.h + 20.0)).clicked {
                self.config.terminal_theme = t.id.to_string();
                self.save();
            }
        }
        let _ = r;
        ui.form_note(0.0, tr!("As janelas do Terminal abertas mudam na hora."));
        ui.space(10.0);
        // Fonte: as famílias de largura fixa instaladas.
        if self.mono_fonts.is_none() {
            let out = std::process::Command::new("fc-list").args([":spacing=mono", "family"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            let mut v: Vec<String> = out.lines().filter_map(|l| l.split(',').next()).map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
            v.sort_by_key(|f| f.to_lowercase());
            v.dedup();
            v.insert(0, tr!("Padrão (DejaVu Sans Mono)").into());
            self.mono_fonts = Some(v);
        }
        let fonts = self.mono_fonts.clone().unwrap_or_default();
        let mut cur = fonts.iter().position(|f| *f == self.config.terminal_font).unwrap_or(0);
        ui.form_row(tr!("Fonte"), LW, |ui| {
            if ui.popup(&mut cur, &fonts, 280.0) {
                self.config.terminal_font = if cur == 0 { String::new() } else { fonts[cur].clone() };
                self.save();
            }
        });
        ui.form_note(LW, tr!("Ícones de Nerd Font aparecem mesmo com outra fonte, se houver uma instalada."));
        let mut px = self.config.terminal_font_size.clamp(8.0, 24.0);
        ui.form_row(tr!("Tamanho"), LW, |ui| {
            ui.horizontal(|ui| {
                if ui.slider(&mut px, 8.0, 24.0, 220.0) {
                    self.config.terminal_font_size = px.round();
                    self.save();
                }
                let t = tr!("{} pt", self.config.terminal_font_size.round());
                ui.small(&t);
            });
        });
        ui.form_note(LW, tr!("Fonte e tamanho valem para as próximas janelas do Terminal."));
    }
}

thread_local! {
    static TEST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static TEST_DBL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl App for Prefs {
    fn ui(&mut self, ui: &mut Ui) {
        // Dispositivos de som lidos na thread. Arrastando um slider, a leitura
        // (que pode ser de antes do arraste) espera, para o polegar não pular.
        if !ui.mouse_down() {
            if let Some(Ok(a)) = self.audio_rx.as_ref().map(|rx| rx.try_recv()) {
                self.audio = a;
                self.audio_rx = None;
            }
        }
        if self.audio_rx.is_some() {
            ui.wake_after(Duration::from_millis(80));
        }
        // Com a folha da senha aberta, só ela responde.
        let sheet = (self.pane == Pane::Rede && self.rede.prompt.is_some())
            || (matches!(self.pane, Pane::Inicio | Pane::Contas) && self.inicio.sheet_open())
            || (self.pane == Pane::Contas && self.contas.sheet_open());
        if sheet {
            ui.set_enabled(false);
        } else {
            self.keyboard(ui);
        }
        self.toolbar(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        ui.set_title(info(self.pane).map_or(tr!("Preferências"), |i| tr(i.name)));
        let (_, used) = ui.area_size(Rect::new(28.0, TOOLBAR_H + 20.0, w - 56.0, h - TOOLBAR_H - 40.0), |ui| match self.pane {
            Pane::Todos => self.show_all(ui),
            Pane::Aparencia => self.aparencia(ui),
            Pane::Mesa => self.mesa(ui),
            Pane::Som => self.som(ui),
            Pane::Energia => self.energia(ui),
            Pane::Rede => {
                let mut menu = self.config.wifi_menu;
                if self.rede.ui(ui, &mut menu) {
                    self.config.wifi_menu = menu;
                    self.save();
                }
            }
            Pane::Inicio => self.inicio.ui(ui),
            Pane::Contas => {
                self.contas.ui(ui, &mut self.inicio);
                ui.space(8.0);
                let mut on = self.config.user_menu;
                if ui.checkbox(&mut on, tr!("Ativar a troca rápida de usuário (nome na barra de menu)")) {
                    self.config.user_menu = on;
                    self.save();
                }
            }
            Pane::Compart => self.compart.ui(ui),
            Pane::Padrao => {
                self.padrao.ui(ui);
                self.padrao_glove(ui);
            }
            Pane::Bluetooth => self.bluetooth.ui(ui),
            Pane::Midia => self.midia(ui),
            Pane::Atualizacao => self.atualizacao.ui(ui),
            Pane::Inicializacao => self.inicializacao.ui(ui),
            Pane::Fala => self.fala.ui(ui),
            Pane::Impressao => self.impressao.ui(ui),
            Pane::Internacional => {
                let mut tab = self.internacional.tab;
                let shown = tab;
                ui.space(4.0);
                ui.tab_box(&mut tab, &[tr!("Idioma"), tr!("Formatos"), tr!("Menu de Entrada")], 380.0, |ui| match shown {
                    0 => self.internacional.idioma(ui),
                    1 => self.internacional.formatos(ui),
                    _ => self.menu_entrada(ui),
                });
                self.internacional.tab = tab;
            }
            Pane::Acesso => self.acesso(ui),
            Pane::DataHora => {
                let mut c = self.config.clock.clone();
                if self.datahora.ui(ui, &mut c) {
                    self.config.clock = c;
                    self.save();
                }
            }
            Pane::Monitor => self.monitor(ui),
            Pane::Trackpad => self.trackpad(ui),
            Pane::Dock => self.dock(ui),
            Pane::Teclado => self.teclado(ui),
            Pane::Mouse => self.mouse(ui),
            Pane::Terminal => self.terminal(ui),
            Pane::Seguranca => self.seguranca(ui),
            Pane::Busca => self.busca(ui),
            Pane::Expose => self.expose(ui),
        });
        if sheet {
            ui.set_enabled(true);
            if self.pane == Pane::Rede {
                self.rede.prompt_sheet(ui);
            } else if self.contas.sheet_open() {
                self.contas.sheets(ui);
            } else {
                self.inicio.picker_sheet(ui);
            }
        }
        // A janela fica da altura do painel (cresce e encolhe suave, como na era).
        ui.set_window_size(W as f32, (TOOLBAR_H + 20.0 + used + 24.0).clamp(240.0, 720.0), true);
        self.flush_save(ui);
    }

    fn menus(&self) -> Vec<Menu> {
        let mut items = vec![MenuItem::new("todos", tr!("Mostrar Todos"), "Ctrl+L"), MenuItem::Separator];
        let mut sorted: Vec<&PaneInfo> = PANES.iter().collect();
        sorted.sort_by_key(|i| norm(i.name));
        items.extend(sorted.into_iter().map(|i| MenuItem::new(&format!("pane:{}", i.id), i.name, "").checked(self.pane == i.pane)));
        vec![Menu::new(tr!("Visualizar"), items)]
    }

    fn config_changed(&mut self) {
        // Com uma gravação adiada, a memória está mais nova que o arquivo.
        if self.save_pending.get() {
            return;
        }
        self.config = Config::load();
        self.outputs = OutputsInfo::load();
    }

    fn tick(&self) -> Option<Duration> {
        // Contagem da confirmação de resolução; e os monitores conectados (cabo).
        if self.confirm.is_some() {
            Some(Duration::from_millis(500))
        } else if matches!(self.pane, Pane::Monitor | Pane::Som | Pane::Rede | Pane::Bluetooth) {
            Some(Duration::from_secs(2))
        } else {
            None
        }
    }

    fn on_tick(&mut self) {
        if self.confirm.as_ref().is_some_and(|c| Instant::now() >= c.2) {
            self.revert();
        }
        self.outputs = OutputsInfo::load();
        if self.pane == Pane::Som && self.audio_rx.is_none() {
            self.audio_rx = Some(som::status_async());
        }
        if self.pane == Pane::Rede {
            self.rede.refresh();
        }
        if self.pane == Pane::Bluetooth {
            self.bluetooth.refresh();
        }
    }

    fn close_requested(&mut self) -> bool {
        // Fechar sem confirmar = voltar à resolução anterior.
        self.revert();
        true
    }
}

impl Prefs {
    /// Atalhos e ações do menu Visualizar.
    fn keyboard(&mut self, ui: &mut Ui) {
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if ui.keys().iter().any(|k| k.combo == "Ctrl+L") {
            acts.push("todos".into());
        }
        if ui.keys().iter().any(|k| k.combo == "Ctrl+bracketleft") && !self.back.is_empty() {
            let p = self.back.pop().unwrap();
            self.forward.push(self.pane);
            self.pane = p;
        }
        for a in acts {
            if a == "todos" {
                self.go(Pane::Todos);
            } else if let Some(i) = a.strip_prefix("pane:").and_then(|id| PANES.iter().find(|i| i.id == id)) {
                self.go(i.pane);
            }
            ui.redraw = true;
        }
    }
}

/// Posição da interface Wi-Fi na lista do painel Rede (0 se não houver).
fn rede_wifi_index() -> usize {
    glove_config::rede::devices().and_then(|l| l.iter().position(|i| i.kind == "wifi")).unwrap_or(0)
}

/// Quebra o nome em até duas linhas que caibam em `max`.
fn wrap2(ui: &Ui, t: &str, max: f32) -> Vec<String> {
    if ui.text_width(t, 11.0, false) <= max {
        return vec![t.to_string()];
    }
    let words: Vec<&str> = t.split(' ').collect();
    for cut in (1..words.len()).rev() {
        let a = words[..cut].join(" ");
        if ui.text_width(&a, 11.0, false) <= max {
            return vec![a, words[cut..].join(" ")];
        }
    }
    // Nem a primeira palavra cabe: quebra nela mesmo assim (quem desenha encolhe).
    if words.len() > 1 {
        return vec![words[0].to_string(), words[1..].join(" ")];
    }
    vec![t.to_string()]
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Preferências").into(), app_id: "glove.Preferences".into(), size: (W, H), fixed: true, min_size: None };
    if let Err(e) = run(opts, Prefs::new()) {
        eprintln!("glove-prefs: {e}");
    }
}

#[cfg(test)]
mod tests {
    /// Todo atalho do Glove aparece em algum grupo do painel Atalhos.
    #[test]
    fn minutos() {
        use glove_config::tr;
        assert_eq!(super::minutes_label(0), tr!("Nunca"));
        assert_eq!(super::minutes_label(45), tr!("{} min", 45));
        assert_eq!(super::minutes_label(120), tr!("{} horas", 2));
        assert_eq!(super::minutes_label(90), tr!("{} h {} min", 1, 30));
    }

    #[test]
    fn busca_de_paineis() {
        use super::{matches, Pane};
        assert_eq!(matches("resolu"), vec![Pane::Monitor]);
        assert_eq!(matches("atalhos"), vec![Pane::Teclado]);
        assert_eq!(matches("papel parede"), vec![Pane::Mesa]);
        assert_eq!(matches("aparencia"), vec![Pane::Aparencia]);
        assert!(matches("rolagem").contains(&Pane::Mouse));
        assert!(matches("").is_empty());
    }

    #[test]
    fn grupos_cobrem_todos_os_atalhos() {
        for (action, _) in glove_config::ACTIONS {
            assert!(super::SHORTCUT_GROUPS.iter().any(|(_, a)| a.contains(action)), "{action} sem grupo");
        }
    }
}
