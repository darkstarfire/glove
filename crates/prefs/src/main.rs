//! Preferências do Glove. Cada mudança é salva no config.toml e o
//! compositor aplica na hora.

use std::time::{Duration, Instant};

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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pane {
    /// A grade com todos os painéis ("Mostrar Todos").
    Todos,
    Aparencia,
    Monitor,
    Dock,
    Teclado,
    Mouse,
    Trackpad,
    Atalhos,
    Terminal,
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

const PANES: [PaneInfo; 8] = [
    PaneInfo { pane: Pane::Aparencia, id: "aparencia", icon: Icon::Pref(PrefKind::Appearance), name: "Aparência", category: 0, keywords: "cor destaque tema papel de parede fundo mesa imagem rolagem grafite azul" },
    PaneInfo { pane: Pane::Dock, id: "dock", icon: Icon::Pref(PrefKind::Dock), name: "Dock", category: 0, keywords: "tamanho ampliação ocultar automaticamente posição esquerda direita minimizar efeito" },
    PaneInfo { pane: Pane::Monitor, id: "monitor", icon: Icon::Display, name: "Monitor", category: 1, keywords: "tela resolução escala tamanho hz frequência monitores" },
    PaneInfo { pane: Pane::Teclado, id: "teclado", icon: Icon::Pref(PrefKind::Keyboard), name: "Teclado", category: 1, keywords: "layout idioma repetição de teclas abnt2 digitação" },
    PaneInfo { pane: Pane::Atalhos, id: "atalhos", icon: Icon::Pref(PrefKind::Shortcuts), name: "Atalhos de Teclado", category: 1, keywords: "teclas combinação exposé captura bloquear forçar encerrar" },
    PaneInfo { pane: Pane::Mouse, id: "mouse", icon: Icon::Mouse, name: "Mouse", category: 1, keywords: "velocidade ponteiro rolagem clique duplo" },
    PaneInfo { pane: Pane::Trackpad, id: "trackpad", icon: Icon::Trackpad, name: "Trackpad", category: 1, keywords: "toque tocar para clicar rolagem natural dois dedos touchpad" },
    PaneInfo { pane: Pane::Terminal, id: "terminal", icon: Icon::Terminal, name: "Terminal", category: 3, keywords: "fonte shell linha de comando" },
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
            let hay = norm(&format!("{} {}", i.name, i.keywords));
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
    format!("{}×", format!("{v}").replace('.', ","))
}

/// "2256x1504@60" -> "2256 × 1504, 60 Hz".
fn mode_label(m: &str) -> String {
    match glove_config::parse_mode(m) {
        Some((w, h, hz)) if hz > 0 => format!("{w} × {h}, {hz} Hz"),
        Some((w, h, _)) => format!("{w} × {h}"),
        None => m.to_string(),
    }
}

const LAYOUTS: [(&str, &str, &str); 10] = [
    ("br", "", "Português (Brasil, ABNT2)"),
    ("us", "", "Inglês (EUA)"),
    ("us", "intl", "Inglês (EUA, internacional)"),
    ("pt", "", "Português (Portugal)"),
    ("latam", "", "Espanhol (América Latina)"),
    ("es", "", "Espanhol (Espanha)"),
    ("gb", "", "Inglês (Reino Unido)"),
    ("fr", "", "Francês"),
    ("de", "", "Alemão"),
    ("it", "", "Italiano"),
];

const WALLPAPERS: [(&str, &str); 3] = [("ondas", "Ondas"), ("aurora", "Aurora"), ("liso", "Liso")];

struct Prefs {
    config: Config,
    pane: Pane,
    /// Imagens PNG encontradas em ~/Imagens (para usar de papel de parede).
    images: Vec<std::path::PathBuf>,
    /// Monitores conectados (escritos pelo compositor).
    outputs: Option<OutputsInfo>,
    /// Resolução nova esperando confirmação: (monitor e resolução anteriores, prazo).
    confirm: Option<(String, String, Instant)>,
    /// Aba aberta no painel Teclado.
    kbd_tab: usize,
    /// Grupos abertos no painel Atalhos.
    open_groups: [bool; 3],
    /// Histórico das setas da barra.
    back: Vec<Pane>,
    forward: Vec<Pane>,
    /// Texto da busca da barra.
    search: String,
    /// Menu de contexto de um painel da grade: (posição, painel).
    ctx: Option<((f32, f32), Pane)>,
}

/// Grupos de atalhos (como no painel de atalhos do 10.4).
const SHORTCUT_GROUPS: [(&str, &[&str]); 3] = [
    ("Janelas", &["close", "minimize", "zoom", "cycle", "expose_all", "expose_app", "expose_desktop"]),
    ("Aplicativos", &["terminal", "preferences", "search", "hide", "hide_others"]),
    ("Sistema", &["screenshot", "force_quit", "lock", "quit"]),
];

impl Prefs {
    fn new() -> Self {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let mut images = Vec::new();
        for d in ["Imagens", "Pictures"] {
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
        let arg = std::env::args().nth(1).unwrap_or_default();
        let pane = PANES.iter().find(|i| i.id == arg).map_or(Pane::Todos, |i| i.pane);
        Self {
            config: Config::load(),
            pane,
            images,
            outputs: OutputsInfo::load(),
            confirm: None,
            kbd_tab: 0,
            open_groups: [true, false, false],
            back: Vec::new(),
            forward: Vec::new(),
            search: String::new(),
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

    fn save(&self) {
        if let Err(e) = self.config.save() {
            eprintln!("glove-prefs: não consegui salvar: {e}");
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
                if ui.icon_tile(Icon::Pref(PrefKind::ShowAll), "Mostrar Todos", false) {
                    chosen = Some(Pane::Todos);
                }
                // Divisória entre "Mostrar Todos" e os favoritos.
                let r = ui.alloc(9.0, 58.0);
                ui.c.vline(r.x + 4.0, r.y + 8.0, 42.0, rgba(0, 0, 0, 40));
                ui.c.vline(r.x + 5.0, r.y + 8.0, 42.0, rgba(255, 255, 255, 160));
                for f in &favs {
                    // Os que não cabem antes da busca ficam de fora.
                    if ui.available_width() < (ui.text_width(f.name, 11.0, false) + 12.0).max(64.0) {
                        break;
                    }
                    if ui.icon_tile(f.icon, f.name, false) {
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
        ui.search_field_in(Rect::new(w - 172.0, mid - glove_kit::SEARCH_H / 2.0, 160.0, glove_kit::SEARCH_H), &mut self.search, "Buscar");
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
            for (n, l) in wrap2(ui, i.name, r.w - 6.0).iter().enumerate() {
                let tw = ui.text_width(l, 11.0, false);
                ui.fonts.draw(ui.c, l, r.x + (r.w - tw) / 2.0, r.y + 52.0 + n as f32 * 13.0, 11.0, false, ink);
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
        let items = [format!("Abrir “{}”", i.name), if fav { "Tirar da Barra".to_string() } else { "Pôr na Barra".to_string() }];
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
        ui.heading("Tema");
        let mut v = Variant::ALL.iter().position(|v| v.id() == self.config.theme).unwrap_or(0);
        let labels: Vec<&str> = Variant::ALL.iter().map(|v| v.label()).collect();
        ui.indent(16.0, |ui| {
            ui.spacing(4.0);
            ui.horizontal(|ui| {
                ui.spacing(18.0);
                if ui.radio_group(&mut v, &labels) {
                    self.config.theme = Variant::ALL[v].id().into();
                    self.save();
                }
            });
            ui.small("Claro: sóbrio, como o fim da era (padrão). Listras: gel azul e listras finas. Metal: barras de metal escovado. Grafite: tons de cinza.");
        });
        ui.space(10.0);
        ui.separator();
        ui.space(6.0);
        ui.heading("Papel de parede");
        let mut options: Vec<String> = WALLPAPERS.iter().map(|(_, l)| l.to_string()).collect();
        let mut ids: Vec<String> = WALLPAPERS.iter().map(|(i, _)| i.to_string()).collect();
        for p in &self.images {
            options.push(p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            ids.push(p.to_string_lossy().into_owned());
        }
        let mut cur = ids.iter().position(|i| *i == self.config.wallpaper).unwrap_or(0);
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.label_right("Imagem:", 70.0);
                if ui.popup(&mut cur, &options, 260.0) {
                    self.config.wallpaper = ids[cur].clone();
                    self.save();
                }
            });
            ui.small("Imagens PNG da sua pasta Imagens também aparecem na lista.");
        });
    }

    fn dock(&mut self, ui: &mut Ui) {
        // Formulário da era: rótulos à direita, controles numa coluna comum.
        const LW: f32 = 190.0;
        ui.space(8.0);
        let mut size = self.config.dock.size as f32;
        ui.form_row("Tamanho do Dock", LW, |ui| {
            let t = Ticks { count: 2, labels: &[(0.0, "Pequeno"), (1.0, "Grande")], snap: false };
            if ui.slider_ticks(&mut size, 32.0, 72.0, 240.0, &t, true) {
                self.config.dock.size = size.round() as u32;
                self.config.dock.max_size = self.config.dock.max_size.max(self.config.dock.size);
                self.save();
            }
        });
        ui.space(6.0);
        let mut mag = self.config.dock.magnification;
        let mut max = self.config.dock.max_size as f32;
        let min = self.config.dock.size as f32;
        ui.form_row("Ampliação", LW, |ui| {
            let t = Ticks { count: 2, labels: &[(0.0, "Mínima"), (1.0, "Máxima")], snap: false };
            if ui.slider_ticks(&mut max, min, 128.0, 240.0, &t, mag) {
                self.config.dock.max_size = max.round() as u32;
                self.save();
            }
        });
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut mag, "Ampliar ícones ao passar o mouse") {
                self.config.dock.magnification = mag;
                self.save();
            }
        });
        ui.space(8.0);
        let words = ["esquerda", "embaixo", "direita"];
        let mut pos = words.iter().position(|w| *w == self.config.dock.position).unwrap_or(1);
        ui.form_row("Posição na tela", LW, |ui| {
            if ui.radio_group(&mut pos, &["À esquerda", "Embaixo", "À direita"]) {
                self.config.dock.position = words[pos].to_string();
                self.save();
            }
        });
        ui.space(6.0);
        let mut hide = self.config.dock.autohide;
        ui.form_row("", LW, |ui| {
            if ui.checkbox(&mut hide, "Ocultar e mostrar o Dock automaticamente") {
                self.config.dock.autohide = hide;
                self.save();
            }
        });
    }

    fn teclado(&mut self, ui: &mut Ui) {
        let mut tab = self.kbd_tab;
        let shown = tab;
        ui.space(4.0);
        ui.tab_box(&mut tab, &["Repetição", "Layout"], 300.0, |ui| match shown {
            0 => {
                const LW: f32 = 170.0;
                let mut delay = self.config.keyboard.repeat_delay as f32;
                let mut rate = self.config.keyboard.repeat_rate as f32;
                ui.form_row("Velocidade de repetição", LW, |ui| {
                    let t = Ticks { count: 8, labels: &[(0.0, "Lenta"), (1.0, "Rápida")], snap: false };
                    if ui.slider_ticks(&mut rate, 2.0, 60.0, 220.0, &t, true) {
                        self.config.keyboard.repeat_rate = rate.round() as u32;
                        self.save();
                    }
                });
                ui.space(6.0);
                ui.form_row("Atraso até repetir", LW, |ui| {
                    // Invertido: à direita = atraso menor.
                    let mut inv = 1000.0 - delay;
                    let t = Ticks { count: 6, labels: &[(0.0, "Longo"), (1.0, "Curto")], snap: false };
                    if ui.slider_ticks(&mut inv, 0.0, 850.0, 220.0, &t, true) {
                        delay = 1000.0 - inv;
                        self.config.keyboard.repeat_delay = delay.round() as u32;
                        self.save();
                    }
                });
                ui.space(10.0);
                ui.form_row("Teste aqui", LW, |ui| {
                    let mut test = TEST.with(|t| t.borrow().clone());
                    if ui.text_field(&mut test, 220.0) {
                        TEST.with(|t| *t.borrow_mut() = test);
                    }
                });
            }
            _ => {
                let names: Vec<String> = LAYOUTS.iter().map(|(_, _, n)| n.to_string()).collect();
                let mut cur = LAYOUTS
                    .iter()
                    .position(|(l, v, _)| *l == self.config.keyboard.layout && *v == self.config.keyboard.variant)
                    .unwrap_or(0);
                ui.form_row("Layout do teclado", 150.0, |ui| {
                    if ui.popup(&mut cur, &names, 260.0) {
                        self.config.keyboard.layout = LAYOUTS[cur].0.into();
                        self.config.keyboard.variant = LAYOUTS[cur].1.into();
                        self.save();
                    }
                });
                ui.form_note(150.0, "Vale na hora para todos os apps.");
            }
        });
        self.kbd_tab = tab;
    }

    fn nested(&self) -> bool {
        self.outputs.as_ref().is_none_or(|o| o.nested)
    }

    fn monitor(&mut self, ui: &mut Ui) {
        let nested = self.nested();
        let outputs = self.outputs.clone().unwrap_or_default();
        if nested {
            ui.small("O Glove está numa janela dentro de outro desktop: a resolução é a da janela.");
            ui.small("O que for escolhido aqui vale quando você entrar na sessão Glove.");
            ui.space(8.0);
        } else {
            ui.heading("Monitor");
            let mut names = vec!["Automático (externo, se houver)".to_string()];
            let mut ids = vec![String::new()];
            for o in &outputs.outputs {
                names.push(o.description.clone());
                ids.push(o.name.clone());
            }
            let mut cur = ids.iter().position(|i| *i == self.config.monitor.output).unwrap_or(0);
            ui.indent(16.0, |ui| {
                if ui.popup(&mut cur, &names, 300.0) {
                    self.ask_confirm();
                    self.config.monitor.output = ids[cur].clone();
                    self.config.monitor.mode.clear();
                    self.save();
                }
            });
            ui.space(8.0);

            let active = outputs.outputs.iter().find(|o| o.active).cloned().unwrap_or_default();
            ui.heading("Resolução");
            let mut names = vec![format!("Recomendada ({})", mode_label(&active.preferred))];
            let mut ids = vec![String::new()];
            for m in &active.modes {
                names.push(mode_label(m));
                ids.push(m.clone());
            }
            let mut cur = ids.iter().position(|i| *i == self.config.monitor.mode).unwrap_or(0);
            ui.indent(16.0, |ui| {
                if ui.popup(&mut cur, &names, 300.0) {
                    self.ask_confirm();
                    self.config.monitor.mode = ids[cur].clone();
                    self.save();
                }
                if !active.current.is_empty() {
                    let t = format!("Em uso: {}", mode_label(&active.current));
                    ui.small(&t);
                }
            });
            ui.space(8.0);
        }

        ui.heading("Tamanho da interface");
        let auto = outputs.outputs.iter().find(|o| o.active).map(|o| o.auto_scale).unwrap_or(1.0);
        let names: Vec<String> = SCALES
            .iter()
            .map(|v| if *v == 0.0 { format!("Automático ({})", scale_label(auto)) } else { scale_label(*v) })
            .collect();
        let mut cur = SCALES.iter().position(|v| *v == self.config.monitor.scale).unwrap_or(0);
        ui.indent(16.0, |ui| {
            if ui.popup(&mut cur, &names, 200.0) {
                self.config.monitor.scale = SCALES[cur];
                self.save();
            }
            ui.small("Maior = textos e janelas maiores. Automático segue a densidade da tela.");
        });

        if let Some((_, _, until)) = &self.confirm {
            let left = until.saturating_duration_since(Instant::now()).as_secs() + 1;
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            let t = format!("Manter esta resolução? Volta à anterior em {left} s.");
            ui.label(&t);
            let (mut keep, mut back) = (false, false);
            ui.horizontal(|ui| {
                keep = ui.default_button("Manter");
                back = ui.button("Voltar");
            });
            if keep {
                self.confirm = None;
            } else if back {
                self.revert();
            }
        }
    }

    /// Guarda o monitor e a resolução atuais para poder voltar.
    fn ask_confirm(&mut self) {
        if self.confirm.is_none() {
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
            self.save();
        }
    }

    fn mouse(&mut self, ui: &mut Ui) {
        let p = self.config.pointer.clone();
        ui.heading("Velocidade do cursor");
        let mut speed = p.speed as f32;
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.small("Lenta");
                if ui.slider(&mut speed, -1.0, 1.0, 300.0) {
                    self.config.pointer.speed = (speed as f64 * 20.0).round() / 20.0;
                    self.save();
                }
                ui.small("Rápida");
            });
            let mut acc = p.acceleration;
            if ui.checkbox(&mut acc, "Aceleração (anda mais quando o mouse se move rápido)") {
                self.config.pointer.acceleration = acc;
                self.save();
            }
        });
        ui.space(8.0);
        ui.heading("Rolagem e botões");
        ui.indent(16.0, |ui| {
            let mut nat = p.natural_scroll;
            if ui.checkbox(&mut nat, "Rolagem natural (o conteúdo acompanha a roda)") {
                self.config.pointer.natural_scroll = nat;
                self.save();
            }
            let mut left = p.left_handed;
            if ui.checkbox(&mut left, "Trocar os botões (para canhotos)") {
                self.config.pointer.left_handed = left;
                self.save();
            }
        });
        if self.nested() {
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            ui.small("Dentro de outro desktop só a rolagem vale agora; o resto vale na sessão Glove.");
        }
    }

    fn trackpad(&mut self, ui: &mut Ui) {
        let t = self.config.touchpad.clone();
        ui.heading("Clique");
        ui.indent(16.0, |ui| {
            let mut tap = t.tap_to_click;
            if ui.checkbox(&mut tap, "Tocar para clicar") {
                self.config.touchpad.tap_to_click = tap;
                self.save();
            }
            ui.space(4.0);
            ui.small("Clique secundário:");
            let mut v = if t.two_finger_right_click { 0 } else { 1 };
            ui.horizontal(|ui| {
                ui.spacing(18.0);
                if ui.radio_group(&mut v, &["Com dois dedos", "No canto inferior direito"]) {
                    self.config.touchpad.two_finger_right_click = v == 0;
                    self.save();
                }
            });
        });
        ui.space(8.0);
        ui.heading("Movimento");
        ui.indent(16.0, |ui| {
            let mut speed = t.speed as f32;
            ui.horizontal(|ui| {
                ui.small("Lenta");
                if ui.slider(&mut speed, -1.0, 1.0, 300.0) {
                    self.config.touchpad.speed = (speed as f64 * 20.0).round() / 20.0;
                    self.save();
                }
                ui.small("Rápida");
            });
            let mut nat = t.natural_scroll;
            if ui.checkbox(&mut nat, "Rolagem natural (o conteúdo acompanha os dedos)") {
                self.config.touchpad.natural_scroll = nat;
                self.save();
            }
            let mut dwt = t.disable_while_typing;
            if ui.checkbox(&mut dwt, "Ignorar o trackpad enquanto digito") {
                self.config.touchpad.disable_while_typing = dwt;
                self.save();
            }
        });
        if self.nested() {
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            ui.small("Dentro de outro desktop o trackpad segue o desktop de lá; isto vale na sessão Glove.");
        }
    }

    fn atalhos(&mut self, ui: &mut Ui) {
        ui.heading("Atalhos de teclado");
        ui.small("Clique num atalho e pressione a nova combinação (Esc cancela).");
        ui.space(4.0);
        for (gi, (group, actions)) in SHORTCUT_GROUPS.iter().enumerate() {
            ui.push_id(group, |ui| {
                ui.disclosure(&mut self.open_groups[gi], group);
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
        if ui.button("Restaurar padrões") {
            self.config.shortcuts = glove_config::default_shortcuts();
            self.save();
        }
    }
}

impl Prefs {
    fn terminal(&mut self, ui: &mut Ui) {
        ui.heading("Fonte do Terminal");
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.label_right("Família:", 80.0);
                let mut fam = self.config.terminal_font.clone();
                if ui.text_field(&mut fam, 300.0) {
                    self.config.terminal_font = fam;
                    self.save();
                }
            });
            ui.small("Ex.: JetBrainsMono Nerd Font. Vazio = DejaVu Sans Mono.");
            ui.small("Ícones de Nerd Font aparecem mesmo com outra fonte, se houver uma instalada.");
            ui.space(8.0);
            ui.horizontal(|ui| {
                ui.label_right("Tamanho:", 80.0);
                let mut px = self.config.terminal_font_size.clamp(8.0, 24.0);
                if ui.slider(&mut px, 8.0, 24.0, 260.0) {
                    self.config.terminal_font_size = px.round();
                    self.save();
                }
                let t = format!("{} pt", self.config.terminal_font_size.round());
                ui.small(&t);
            });
            ui.space(6.0);
            ui.small("Vale para as próximas janelas do Terminal.");
        });
    }
}

thread_local! {
    static TEST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

impl App for Prefs {
    fn ui(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        self.toolbar(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        ui.set_title(info(self.pane).map_or("Preferências", |i| i.name));
        let (_, used) = ui.area_size(Rect::new(28.0, TOOLBAR_H + 20.0, w - 56.0, h - TOOLBAR_H - 40.0), |ui| match self.pane {
            Pane::Todos => self.show_all(ui),
            Pane::Aparencia => self.aparencia(ui),
            Pane::Monitor => self.monitor(ui),
            Pane::Trackpad => self.trackpad(ui),
            Pane::Dock => self.dock(ui),
            Pane::Teclado => self.teclado(ui),
            Pane::Mouse => self.mouse(ui),
            Pane::Atalhos => self.atalhos(ui),
            Pane::Terminal => self.terminal(ui),
        });
        // A janela fica da altura do painel (cresce e encolhe suave, como na era).
        ui.set_window_size(W as f32, (TOOLBAR_H + 20.0 + used + 24.0).clamp(240.0, 720.0), true);
    }

    fn menus(&self) -> Vec<Menu> {
        let mut items = vec![MenuItem::new("todos", "Mostrar Todos", "Ctrl+L"), MenuItem::Separator];
        let mut sorted: Vec<&PaneInfo> = PANES.iter().collect();
        sorted.sort_by_key(|i| norm(i.name));
        items.extend(sorted.into_iter().map(|i| MenuItem::new(&format!("pane:{}", i.id), i.name, "").checked(self.pane == i.pane)));
        vec![Menu::new("Visualizar", items)]
    }

    fn config_changed(&mut self) {
        self.config = Config::load();
        self.outputs = OutputsInfo::load();
    }

    fn tick(&self) -> Option<Duration> {
        // Contagem da confirmação de resolução; e os monitores conectados (cabo).
        if self.confirm.is_some() {
            Some(Duration::from_millis(500))
        } else if self.pane == Pane::Monitor {
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
    vec![t.to_string()]
}

fn main() {
    let opts = WindowOptions { title: "Preferências".into(), app_id: "glove.Preferences".into(), size: (W, H), fixed: true };
    if let Err(e) = run(opts, Prefs::new()) {
        eprintln!("glove-prefs: {e}");
    }
}

#[cfg(test)]
mod tests {
    /// Todo atalho do Glove aparece em algum grupo do painel Atalhos.
    #[test]
    fn busca_de_paineis() {
        use super::{matches, Pane};
        assert_eq!(matches("resolu"), vec![Pane::Monitor]);
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
