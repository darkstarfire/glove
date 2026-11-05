//! Tudo que o Glove desenha por conta própria (barra de menu, Dock, menus,
//! barras de título e sombras) e as ações de janela (foco, minimizar, zoom).
//!
//! Os desenhos ficam em cache e só são refeitos quando algo visível muda.

use std::cell::RefCell;

use glove_paint::{
    chrome::{self, DockItem, DockLayout, MenuItem, TitleBarState, TitleButton, MENU_SHADOW},
    icons::Icon,
    theme::{MENUBAR_H, TITLE_H},
    draw::Canvas,
    wallpaper, Theme, Variant,
};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::texture::TextureBuffer,
            gles::{GlesRenderer, GlesTexture},
        },
    },
    desktop::{Window, WindowSurfaceType},
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecoMode,
            shell::server::xdg_toplevel,
        },
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size, Transform, SERIAL_COUNTER},
    wayland::{compositor::with_states, shell::xdg::{SurfaceCachedState, XdgToplevelSurfaceData}},
};

use crate::{grabs::ResizeEdge, state::Glove};

/// Largura da borda invisível para redimensionar.
const RESIZE_BORDER: i32 = 6;

/// Uma peça desenhada: os pixels vão para a GPU no próximo quadro e a cópia
/// na memória é descartada logo em seguida (nada fica duplicado na RAM).
pub struct Tex {
    pending: RefCell<Option<Canvas>>,
    gpu: RefCell<Option<TextureBuffer<GlesTexture>>>,
    pub size: Size<i32, Logical>,
    /// Tamanho real em pixels.
    pub px: (i32, i32),
}

impl Tex {
    /// Envia para a GPU (se ainda não foi) e devolve a textura.
    pub fn upload(&self, renderer: &mut GlesRenderer) -> Option<TextureBuffer<GlesTexture>> {
        if let Some(c) = self.pending.borrow_mut().take() {
            match TextureBuffer::from_memory(renderer, c.data(), Fourcc::Abgr8888, self.px, false, 1, Transform::Normal, None) {
                Ok(t) => *self.gpu.borrow_mut() = Some(t),
                Err(e) => tracing::warn!("textura: {e:?}"),
            }
        }
        self.gpu.borrow().clone()
    }
}

/// Janela de tamanho fixo (mínimo = máximo): sem zoom.
pub fn fixed_size(w: &Window) -> bool {
    let Some(t) = w.toplevel() else { return false };
    let (min, max) = with_states(t.wl_surface(), |st| {
        let mut c = st.cached_state.get::<SurfaceCachedState>();
        let d = c.current();
        (d.min_size, d.max_size)
    });
    min.w > 0 && min == max
}

pub fn to_tex(c: Canvas) -> Tex {
    let size = (c.w.round() as i32, c.h.round() as i32).into();
    let px = (c.px_width() as i32, c.px_height() as i32);
    Tex { pending: RefCell::new(Some(c)), gpu: RefCell::new(None), size, px }
}

// ---------------------------------------------------------------------------
// Informações das janelas
// ---------------------------------------------------------------------------

pub fn window_surface(w: &Window) -> Option<&WlSurface> {
    w.toplevel().map(|t| t.wl_surface())
}

pub fn window_info(w: &Window) -> (String, String) {
    let Some(s) = window_surface(w) else { return Default::default() };
    let (title, id) = with_states(s, |st| {
        st.data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|d| d.lock().ok().map(|d| (d.title.clone().unwrap_or_default(), d.app_id.clone().unwrap_or_default())))
            .unwrap_or_default()
    });
    (title, canonical_app_id(id))
}

/// Os navegadores que o glove-navegador abre contam como o "Navegador" do Dock.
pub fn canonical_app_id(id: String) -> String {
    match id.to_ascii_lowercase().as_str() {
        "firefox" | "firefox_firefox" | "org.mozilla.firefox" | "netsurf" | "netsurf-gtk" | "netsurf-gtk3" => "glove.Navegador".into(),
        _ => id,
    }
}

/// A janela pediu (e recebeu) bordas desenhadas pelo Glove?
pub fn is_ssd(w: &Window) -> bool {
    w.toplevel().map(|t| t.current_state().decoration_mode == Some(DecoMode::ServerSide)).unwrap_or(false)
}

pub fn deco_top(w: &Window) -> i32 {
    if is_ssd(w) { TITLE_H } else { 0 }
}

/// A Lixeira ($XDG_DATA_HOME/Trash/files) tem algum item?
pub fn trash_has_items() -> bool {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")));
    base.and_then(|b| std::fs::read_dir(b.join("Trash/files")).ok()).is_some_and(|mut d| d.next().is_some())
}

/// Apps com janela "unificada" (título em metal emendado na barra de ferramentas).
pub const UNIFIED_APPS: [&str; 1] = ["glove.Files"];

/// Nome do app de uma janela, para mostrar ao usuário.
pub fn app_display_name(w: &Window) -> String {
    let (title, id) = window_info(w);
    if id.is_empty() { title } else { pretty_app_name(&id) }
}

/// Nome curto e bonito a partir do app_id ("org.gnome.TextEditor" → "TextEditor").
pub(crate) fn pretty_app_name(app_id: &str) -> String {
    // Apps do próprio Glove têm nomes em português.
    match app_id {
        "glove.Preferences" => return "Preferências".into(),
        "glove.Calculator" => return "Calculadora".into(),
        "glove.Clock" => return "Relógio".into(),
        "glove.TextEditor" => return "Editor de Texto".into(),
        "glove.Files" => return "Arquivos".into(),
        "glove.Viewer" => return "Visualizador".into(),
        "glove.Terminal" => return "Terminal".into(),
        "glove.Notes" => return "Lembretes".into(),
        "glove.Monitor" => return "Monitor".into(),
        "glove.Shot" => return "Captura".into(),
        "glove.Chars" => return "Caracteres".into(),
        "glove.Find" => return "Busca".into(),
        "glove.Disks" => return "Discos".into(),
        "glove.Chess" => return "Xadrez".into(),
        "glove.Arquivo" => return "Utilitário de Arquivo".into(),
        "glove.Navegador" => return "Navegador".into(),
        "glove.Musica" => return "Música".into(),
        id if id.starts_with("glove.") => return "Glove".into(),
        _ => {}
    }
    let last = app_id.rsplit('.').next().unwrap_or(app_id);
    let mut c = last.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Cache por janela das peças desenhadas.
#[derive(Default)]
pub struct Deco {
    title_key: Option<(i32, TitleBarState, String, u64)>,
    pub title: Option<Tex>,
    shadow_key: Option<(i32, i32, bool, u64)>,
    pub shadows: Vec<(i32, i32, Tex)>,
}

/// Posição e tamanho antes do zoom.
#[derive(Default)]
struct Restore(Option<Rectangle<i32, Logical>>);

/// Marca que a janela já foi posicionada na primeira exibição.
pub struct Placed;

// ---------------------------------------------------------------------------
// Peças globais (barra de menu, Dock, menu aberto, papel de parede)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    About,
    Preferences,
    /// Liga/desliga a ampliação do Dock.
    DockMagnification,
    DockAutohide,
    /// "esquerda", "embaixo" ou "direita".
    SetDockSide(String),
    DockPreferences,
    /// Abre um app dos Itens Recentes (app_id).
    RecentApp(String),
    /// Abre um documento dos Itens Recentes.
    RecentDocument(std::path::PathBuf),
    ClearRecent,
    ForceQuit,
    Lock,
    Sleep,
    Restart,
    ShutDown,
    LogOut,
    /// "Sobre <app>" genérico dos apps do Glove: (app_id, nome).
    AboutApp(String, String),
    /// Esconde as janelas do app em foco.
    HideApp(u32),
    HideOthers(u32),
    ShowAll,
    MinimizeWindow(Window),
    ZoomWindow(Window),
    BringAllToFront(u32),
    /// Janela escolhida na lista do menu Janela.
    FocusWindow(Window),
    /// Menu do Dock: fixar/tirar o app (app_id, pid de uma janela dele).
    ToggleKeepInDock(String, u32),
    /// Abre no Arquivos a pasta do programa (comando do app).
    ShowInFiles(String),
    DockRun(DockAction),
    /// Esvaziar a Lixeira (com confirmação, no Arquivos).
    EmptyTrash,
    /// Menu do monitor: (saída, "LxA@Hz").
    SetMode(String, String),
    ToggleBatteryPercent,
    /// Abre as Preferências num painel.
    OpenPrefs(String),
    /// Liga (true) ou desliga o Wi-Fi.
    SetWifi(bool),
    /// Relógio da barra com ponteiros (true) ou números.
    ClockAnalog(bool),
    /// Layout de teclado do Menu de Entrada ("us:intl").
    SetLayout(String),
    /// Entrar numa rede: (nome, pede senha).
    JoinWifi(String, bool),
    /// Fecha todas as janelas do app em foco.
    QuitApp(u32),
    /// Item de um menu do próprio app.
    Client(u32, String),
}

/// Qual menu da barra está aberto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarTarget {
    Logo,
    /// 0 = menu com o nome do app; 1.. = menus do app.
    Title(usize),
    /// Extra da barra (volume, bateria, monitor), pela posição.
    Extra(usize),
    /// Menu de um ícone do Dock (pela posição no Dock).
    Dock(usize),
}

/// Item de menu com o que ele faz e, se for submenu, os filhos.
#[derive(Clone, Debug)]
pub struct MenuNode {
    pub item: MenuItem,
    pub action: Option<MenuAction>,
    pub sub: Vec<MenuNode>,
}

impl MenuNode {
    pub fn new(item: MenuItem, action: Option<MenuAction>) -> Self {
        Self { item, action, sub: Vec::new() }
    }
    pub fn sep() -> Self {
        Self::new(MenuItem::sep(), None)
    }
    pub fn submenu(label: impl Into<String>, sub: Vec<MenuNode>) -> Self {
        Self { item: MenuItem::sub(label), action: None, sub }
    }
    /// Pode ser escolhido (pelo mouse ou pelas setas).
    fn selectable(&self) -> bool {
        !self.item.separator && self.item.enabled
    }
}

/// Um painel do menu aberto: o menu da barra ou um submenu.
pub struct MenuLevel {
    pub nodes: Vec<MenuNode>,
    pub items: Vec<MenuItem>,
    /// Posição do conteúdo (sem a margem de sombra).
    pub origin: Point<i32, Logical>,
    pub size: Size<i32, Logical>,
    pub hover: Option<usize>,
    /// Item do nível anterior que abriu este submenu.
    pub parent: Option<usize>,
    key: Option<(Option<usize>, u64)>,
    pub buffer: Option<Tex>,
}

impl MenuLevel {
    pub fn buffer_pos(&self) -> Point<i32, Logical> {
        (self.origin.x - MENU_SHADOW, self.origin.y).into()
    }
    /// Próximo item escolhível a partir do atual, na direção `dir` (±1), dando a volta.
    fn step(&self, dir: i32) -> Option<usize> {
        let n = self.nodes.len() as i32;
        let start = self.hover.map(|h| h as i32).unwrap_or(if dir > 0 { -1 } else { n });
        (1..=n).map(|k| (start + dir * k).rem_euclid(n) as usize).find(|&i| self.nodes[i].selectable())
    }
}

/// Menu aberto: o painel da barra e os submenus abertos a partir dele.
pub struct OpenMenu {
    pub target: BarTarget,
    pub levels: Vec<MenuLevel>,
    /// Menus do Dock: posição do bico (relativa ao conteúdo).
    pub pointer_x: Option<f32>,
}

/// Teclas que os menus abertos tratam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuKey {
    Up,
    Down,
    Left,
    Right,
    Activate,
    Close,
    /// Qualquer outra tecla: engolida enquanto o menu está aberto.
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockAction {
    App(usize),
    /// App aberto que não está fixo no Dock (pelo app_id).
    Running(String),
    Restore(usize),
    Trash,
}

#[derive(Default)]
pub struct Chrome {
    pub out_size: (i32, i32),
    wallpaper_key: Option<(i32, i32, u64)>,
    pub wallpaper: Option<Tex>,
    menubar_key: Option<(i32, Vec<String>, Vec<chrome::Extra>, String, Option<BarTarget>, Option<usize>, bool, u64)>,
    pub menubar: Option<Tex>,
    pub logo_range: (f32, f32),
    /// Faixas dos títulos de menu (o primeiro é o nome do app).
    pub title_ranges: Vec<(f32, f32)>,
    /// Faixas dos extras da barra (volume, bateria, monitor).
    pub extra_ranges: Vec<(f32, f32)>,
    /// Faixa da lupa da Busca Rápida.
    pub search_range: (f32, f32),

    pub dock_items: Vec<DockItem>,
    pub dock_actions: Vec<DockAction>,
    pub dock_layout: Option<DockLayout>,
    pub dock_hover_x: Option<f32>,
    /// Última posição do mouse no Dock (a ampliação some a partir dela ao sair).
    pub dock_last_x: f32,
    /// Força da ampliação (0 a 1) e a animação em curso: (de, para, início).
    pub dock_zoom: f32,
    pub dock_zoom_anim: Option<(f32, f32, std::time::Instant)>,
    /// Borda em que o Dock fica.
    pub dock_side: chrome::DockSide,
    /// Ocultar automaticamente: 0 = à vista, 1 = escondido; e a animação (de, para, início).
    pub dock_hide: f32,
    pub dock_hide_anim: Option<(f32, f32, std::time::Instant)>,
    /// A Lixeira tem itens (e quando foi conferido).
    pub trash_full: (bool, Option<std::time::Instant>),
    pub dock_hover: Option<usize>,
    pub dock_pressed: Option<usize>,
    dock_key: Option<(Vec<DockItem>, Option<i32>, i32, Option<usize>, u64)>,
    pub dock: Option<Tex>,
    /// Escala usada no último desenho (mudou → refaz tudo).
    pub last_scale: f64,

    pub menu: Option<OpenMenu>,
    /// Ícone do disco na mesa.
    /// Ícones da mesa: o disco do sistema e os removíveis montados (pendrives).
    pub desk_items: Vec<(String, std::path::PathBuf)>,
    pub desk_texs: Vec<Tex>,
    desk_key: Option<(Vec<(String, std::path::PathBuf)>, Option<usize>, u64)>,
    pub desk_selected: Option<usize>,
    pub desk_last_click: Option<std::time::Instant>,
    /// Janela com o mouse sobre os botões (mostra os símbolos).
    pub hover_buttons: Option<Window>,
    pub pressed_button: Option<(Window, TitleButton)>,
    /// Engole o próximo "soltar" do mouse (o clique foi do Glove).
    pub swallow_release: bool,
}

/// Duração da entrada e da saída da ampliação do Dock.
pub const DOCK_ZOOM_SECS: f32 = 0.16;

impl Chrome {
    /// Começa a animar a ampliação até `to` (1 ao entrar no Dock, 0 ao sair).
    pub fn animate_dock_zoom(&mut self, to: f32) {
        let current_to = self.dock_zoom_anim.map(|a| a.1).unwrap_or(self.dock_zoom);
        if current_to != to {
            self.dock_zoom_anim = Some((self.dock_zoom, to, std::time::Instant::now()));
        }
    }

    pub fn dock_pos(&self) -> Point<i32, Logical> {
        let (w, h) = self.out_size;
        let Some(l) = &self.dock_layout else { return (0, h).into() };
        // Escondido: desliza para fora pela borda (sobra só a sombra da prateleira).
        let (lw, lh) = (l.width as i32, l.height as i32);
        match self.dock_side {
            chrome::DockSide::Bottom => {
                let out = (self.dock_hide * l.height as f32) as i32;
                ((w - lw) / 2, h - lh + out).into()
            }
            chrome::DockSide::Left => {
                let out = (self.dock_hide * l.width as f32) as i32;
                (-out, (h - lh) / 2).into()
            }
            chrome::DockSide::Right => {
                let out = (self.dock_hide * l.width as f32) as i32;
                (w - lw + out, (h - lh) / 2).into()
            }
        }
    }

    /// Começa a esconder (1) ou mostrar (0) o Dock.
    pub fn animate_dock_hide(&mut self, to: f32) {
        let current_to = self.dock_hide_anim.map(|a| a.1).unwrap_or(self.dock_hide);
        if current_to != to {
            self.dock_hide_anim = Some((self.dock_hide, to, std::time::Instant::now()));
        }
    }
}

// ---------------------------------------------------------------------------
// Teste de clique
// ---------------------------------------------------------------------------

pub enum Hit {
    /// Menu aberto: (nível, item).
    Menu(usize, Option<usize>),
    /// Painel Forçar Encerrar.
    ForceQuit(chrome::FqHit),
    /// Controle de volume (y relativo ao conteúdo).
    VolumeSlider(f32),
    /// Lupa da Busca Rápida na barra.
    SearchButton,
    /// Painel da Busca Rápida (linha sob o mouse).
    Search(Option<usize>),
    MenuBar { target: Option<BarTarget> },
    Dock(Option<usize>),
    Title { window: Window, button: Option<TitleButton>, group: bool },
    Resize(Window, ResizeEdge),
    Client(Window),
    /// Ícone na mesa (o disco ou um removível).
    DesktopIcon(usize),
    Desktop,
}

impl Glove {
    pub fn output_size(&self) -> (i32, i32) {
        self.space
            .outputs()
            .next()
            .and_then(|o| self.space.output_geometry(o))
            .map(|g| (g.size.w, g.size.h))
            .unwrap_or((1280, 800))
    }

    pub fn hit_test(&self, pos: Point<f64, Logical>) -> Hit {
        let (px, py) = (pos.x, pos.y);
        // Tela de login: só a janela do app de login responde.
        if self.greeter {
            return self.space.element_under(pos).map(|(w, _)| Hit::Client(w.clone())).unwrap_or(Hit::Desktop);
        }

        if let Some(m) = &self.chrome.menu {
            // O submenu mais interno fica por cima.
            for (l, lv) in m.levels.iter().enumerate().rev() {
                if Rectangle::new(lv.origin, lv.size).to_f64().contains(pos) {
                    return Hit::Menu(l, chrome::menu_item_at(&lv.items, (py - lv.origin.y as f64) as f32));
                }
            }
        }
        if let Some(y) = self.volume_slider_y(pos) {
            return Hit::VolumeSlider(y);
        }
        if let Some(r) = self.search_hit(pos) {
            return Hit::Search(r);
        }
        if let Some(h) = self.force_quit_hit(pos) {
            return Hit::ForceQuit(h);
        }
        if py < MENUBAR_H as f64 {
            let (a, b) = self.chrome.logo_range;
            if px >= a as f64 && px < b as f64 {
                return Hit::MenuBar { target: Some(BarTarget::Logo) };
            }
            let (sa, sb) = self.chrome.search_range;
            if px >= sa as f64 && px < sb as f64 {
                return Hit::SearchButton;
            }
            if let Some(k) = self.chrome.extra_ranges.iter().position(|(a, b)| px >= *a as f64 && px < *b as f64) {
                return Hit::MenuBar { target: Some(BarTarget::Extra(k)) };
            }
            let t = self.chrome.title_ranges.iter().position(|(a, b)| px >= *a as f64 && px < *b as f64);
            return Hit::MenuBar { target: t.map(BarTarget::Title) };
        }
        if let Some(l) = self.chrome.dock_layout.as_ref().filter(|_| self.chrome.dock_hide < 0.5) {
            let d = self.chrome.dock_pos();
            let (lx, ly) = ((px - d.x as f64) as f32, (py - d.y as f64) as f32);
            if chrome::dock_contains(l, lx, ly) {
                return Hit::Dock(chrome::dock_item_at(l, lx, ly));
            }
        }

        for w in self.space.elements().rev() {
            let Some(geo) = self.space.element_geometry(w) else { continue };
            let ssd = is_ssd(w);
            let top = if ssd { TITLE_H } else { 0 };
            if ssd {
                let title = Rectangle::new((geo.loc.x, geo.loc.y - top).into(), (geo.size.w, top).into()).to_f64();
                if title.contains(pos) {
                    let (tx, ty) = ((px - title.loc.x) as f32, (py - title.loc.y) as f32);
                    return Hit::Title {
                        window: w.clone(),
                        button: chrome::title_button_at(tx, ty),
                        group: chrome::over_button_group(tx, ty),
                    };
                }
            }
            let render_loc = geo.loc - w.geometry().loc;
            if w.surface_under(pos - render_loc.to_f64(), WindowSurfaceType::ALL).is_some() {
                return Hit::Client(w.clone());
            }
            if ssd {
                let frame = Rectangle::new((geo.loc.x, geo.loc.y - top).into(), (geo.size.w, geo.size.h + top).into());
                let b = RESIZE_BORDER as f64;
                let f = frame.to_f64();
                let outer = Rectangle::new((f.loc.x - b, f.loc.y - b).into(), (f.size.w + 2.0 * b, f.size.h + 2.0 * b).into());
                if outer.contains(pos) && !f.contains(pos) {
                    let mut e = ResizeEdge::empty();
                    if px < f.loc.x {
                        e |= ResizeEdge::LEFT;
                    }
                    if px >= f.loc.x + f.size.w {
                        e |= ResizeEdge::RIGHT;
                    }
                    if py < f.loc.y {
                        e |= ResizeEdge::TOP;
                    }
                    if py >= f.loc.y + f.size.h {
                        e |= ResizeEdge::BOTTOM;
                    }
                    return Hit::Resize(w.clone(), e);
                }
            }
        }
        if let Some(i) = (0..self.chrome.desk_items.len()).find(|i| self.desk_icon_rect(*i).to_f64().contains(pos)) {
            return Hit::DesktopIcon(i);
        }
        Hit::Desktop
    }

    /// Onde fica o ícone `i` da mesa: uma coluna no canto superior direito.
    pub fn desk_icon_rect(&self, i: usize) -> Rectangle<i32, Logical> {
        // Encostado à direita da área útil (o Dock à direita empurra para dentro).
        let a = self.work_area();
        let w = a.loc.x + a.size.w;
        let y = MENUBAR_H + 12 + i as i32 * (chrome::DESK_ICON_H as i32 + 10);
        Rectangle::new((w - chrome::DESK_ICON_W as i32 - 12, y).into(), (chrome::DESK_ICON_W as i32, chrome::DESK_ICON_H as i32).into())
    }

    /// Relê os discos removíveis montados (pendrives) e atualiza a mesa se mudou.
    pub fn refresh_desk_items(&mut self) {
        let mut items = vec![("Disco".to_string(), std::path::PathBuf::from("/"))];
        items.extend(removable_mounts());
        if items != self.chrome.desk_items {
            // Mídia Removível: um disco novo (não os que já estavam na partida) abre no Arquivos.
            if !self.chrome.desk_items.is_empty() && self.config.removable_action == "abrir" && !self.greeter {
                for (_, path) in items.iter().filter(|i| !self.chrome.desk_items.contains(i)) {
                    let q = path.display().to_string().replace('\'', "'\\''");
                    self.spawn(&format!("glove-files '{q}'"));
                }
            }
            self.chrome.desk_items = items;
            self.chrome.desk_selected = None;
            self.dirty = true;
        }
    }

    /// Clique num ícone da mesa: seleciona; clique duplo abre no Arquivos.
    pub fn click_desk_icon(&mut self, i: usize) {
        let now = std::time::Instant::now();
        let double = self.chrome.desk_selected == Some(i) && self.chrome.desk_last_click.is_some_and(|t| now.duration_since(t).as_millis() < self.config.pointer.double_click_ms as u128);
        self.chrome.desk_selected = Some(i);
        self.chrome.desk_last_click = Some(now);
        if double {
            self.chrome.desk_last_click = None;
            if let Some((_, p)) = self.chrome.desk_items.get(i) {
                let p = p.display().to_string().replace('\'', "'\\''");
                self.spawn(&format!("glove-files '{p}'"));
            }
        }
        self.dirty = true;
    }

    // -----------------------------------------------------------------------
    // Foco e ações de janela
    // -----------------------------------------------------------------------

    pub fn focused_window(&self) -> Option<Window> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        self.space.elements().find(|w| window_surface(w) == Some(&focus)).cloned()
    }

    pub fn focus_window(&mut self, window: Option<Window>) {
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        match window {
            Some(w) => {
                self.space.raise_element(&w, true);
                self.note_app_used(&w);
                keyboard.set_focus(self, window_surface(&w).cloned(), serial);
            }
            None => {
                for w in self.space.elements() {
                    w.set_activated(false);
                }
                keyboard.set_focus(self, Option::<WlSurface>::None, serial);
            }
        }
        for w in self.space.elements() {
            if let Some(t) = w.toplevel() {
                t.send_pending_configure();
            }
        }
        self.dirty = true;
    }

    /// Dá foco à janela mais alta que sobrou (depois de fechar/minimizar).
    pub fn focus_topmost(&mut self, except: Option<&WlSurface>) {
        let next = self.space.elements().rev().find(|w| window_surface(w) != except).cloned();
        self.focus_window(next);
    }

    pub fn close_window(&mut self, w: &Window) {
        if let Some(t) = w.toplevel() {
            t.send_close();
        }
    }

    pub fn minimize_window(&mut self, w: &Window) {
        let Some(loc) = self.space.element_location(w) else { return };
        self.space.unmap_elem(w);
        self.minimized.push((w.clone(), loc));
        self.animate_minimize(w, loc);
        self.focus_topmost(None);
    }

    pub fn restore_minimized(&mut self, idx: usize) {
        if idx >= self.minimized.len() {
            return;
        }
        let slot = self.dock_slot(&self.minimized[idx].0);
        let (w, loc) = self.minimized.remove(idx);
        self.space.map_element(w.clone(), loc, true);
        self.animate_restore(&w, loc, slot);
        self.focus_window(Some(w));
    }

    /// Área útil: abaixo da barra de menu e acima do Dock.
    pub fn work_area(&self) -> Rectangle<i32, Logical> {
        let (w, h) = self.output_size();
        // Tela de login: a tela inteira.
        if self.greeter {
            return Rectangle::new((0, 0).into(), (w, h).into());
        }
        // Espaço reservado para o Dock (tamanho dos ícones + prateleira); escondido, nada.
        let reserve = if self.config.dock.autohide { 0 } else { (self.config.dock.size.clamp(24, 128) as f32 * 0.62 + 30.0) as i32 };
        match chrome::DockSide::from_config(&self.config.dock.position) {
            chrome::DockSide::Bottom => Rectangle::new((0, MENUBAR_H).into(), (w, (h - MENUBAR_H - reserve).max(100)).into()),
            chrome::DockSide::Left => Rectangle::new((reserve, MENUBAR_H).into(), ((w - reserve).max(100), h - MENUBAR_H).into()),
            chrome::DockSide::Right => Rectangle::new((0, MENUBAR_H).into(), ((w - reserve).max(100), h - MENUBAR_H).into()),
        }
    }

    pub fn toggle_zoom(&mut self, w: &Window) {
        let Some(t) = w.toplevel().cloned() else { return };
        // Janelas de tamanho fixo não têm zoom.
        if fixed_size(w) {
            return;
        }
        let Some(geo) = self.space.element_geometry(w) else { return };
        w.user_data().insert_if_missing(|| RefCell::new(Restore::default()));
        let restore = w.user_data().get::<RefCell<Restore>>().unwrap();
        let prev = restore.borrow_mut().0.take();
        match prev {
            Some(r) => {
                t.with_pending_state(|s| {
                    s.states.unset(xdg_toplevel::State::Maximized);
                    s.size = Some(r.size);
                });
                self.space.map_element(w.clone(), r.loc, true);
            }
            None => {
                restore.borrow_mut().0 = Some(geo);
                let top = deco_top(w);
                let area = self.work_area();
                t.with_pending_state(|s| {
                    s.states.set(xdg_toplevel::State::Maximized);
                    s.size = Some((area.size.w, area.size.h - top).into());
                });
                self.space.map_element(w.clone(), (area.loc.x, area.loc.y + top), true);
            }
        }
        t.send_pending_configure();
        self.dirty = true;
    }

    /// Posiciona uma janela nova no centro da área útil (com cascata leve).
    pub fn place_new_window(&mut self, w: &Window) {
        if w.user_data().get::<Placed>().is_some() {
            return;
        }
        let size = w.geometry().size;
        if size.w <= 0 || size.h <= 0 {
            return;
        }
        w.user_data().insert_if_missing(|| Placed);
        // Itens Recentes: só apps que o Glove sabe abrir de novo.
        let app_id = window_info(w).1;
        // A janela apareceu: o ícone para de pular (no fim do pulo atual).
        self.stop_bounce(&app_id);
        if self.config.dock_apps.iter().any(|a| a.app_id == app_id) {
            self.recent.add_app(&app_id);
        }
        let area = self.work_area();
        let top = deco_top(w);
        let off = (self.cascade % 6) * 26;
        self.cascade += 1;
        let (x, y) = if self.greeter {
            // Tela de login: o painel no centro, um pouco acima do meio.
            (area.loc.x + (area.size.w - size.w) / 2, area.loc.y + ((area.size.h - size.h) as f32 * 0.42) as i32)
        } else {
            (area.loc.x + ((area.size.w - size.w) / 2).max(0) + off - 65, area.loc.y + top + ((area.size.h - size.h - top) / 3).max(8) + off)
        };
        // Só posiciona: não muda a ordem nem o foco.
        self.space.map_element(w.clone(), (x.max(0), y.max(MENUBAR_H + top)), false);
        if self.focused_window().as_ref() == Some(w) {
            self.space.raise_element(w, true);
        }
        self.dirty = true;
    }

    // -----------------------------------------------------------------------
    // Menu principal e Dock
    // -----------------------------------------------------------------------

    /// Processo dono da janela em foco (para achar os menus dele).
    pub fn focused_pid(&self) -> Option<u32> {
        let w = self.focused_window()?;
        let surface = window_surface(&w)?;
        use smithay::reexports::wayland_server::Resource;
        let client = self.display_handle.get_client(surface.id()).ok()?;
        client.get_credentials(&self.display_handle).ok().map(|c| c.pid as u32)
    }

    pub(crate) fn focused_app_name(&self) -> String {
        match self.focused_window() {
            Some(win) => {
                let (title, app_id) = window_info(&win);
                self.config
                    .dock_apps
                    .iter()
                    .find(|a| !a.app_id.is_empty() && a.app_id.eq_ignore_ascii_case(&app_id))
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| if app_id.is_empty() { title } else { pretty_app_name(&app_id) })
            }
            None => "Glove".to_string(),
        }
    }

    /// Menus enviados pelo app em foco.
    pub(crate) fn focused_menus(&self) -> Vec<crate::menus::AppMenu> {
        let Some(pid) = self.focused_pid() else { return Vec::new() };
        self.menu_clients.iter().rev().find(|c| c.pid == pid).map(|c| c.menus.clone()).unwrap_or_default()
    }

    /// Títulos da barra: nome do app + menus dele.
    pub fn bar_titles(&self) -> Vec<String> {
        let mut t = vec![self.focused_app_name()];
        t.extend(self.bar_menus().into_iter().map(|m| match m {
            crate::appmenu::BarMenu::Client(m) => m.title,
            crate::appmenu::BarMenu::Window(_) => crate::appmenu::WINDOW_MENU.to_string(),
        }));
        t
    }

    pub fn open_bar_menu(&mut self, target: BarTarget) {
        self.close_volume_slider();
        self.close_search();
        match target {
            BarTarget::Extra(k) => self.open_extra(k),
            BarTarget::Dock(i) => self.open_dock_menu(i),
            BarTarget::Logo => self.open_main_menu(),
            BarTarget::Title(0) => self.open_app_menu(),
            BarTarget::Title(i) => {
                let pid = self.focused_pid().unwrap_or(0);
                let nodes = match self.bar_menus().into_iter().nth(i - 1) {
                    Some(crate::appmenu::BarMenu::Client(m)) => client_nodes(pid, m.items),
                    Some(crate::appmenu::BarMenu::Window(extra)) => self.window_menu_nodes(extra),
                    None => return,
                };
                self.show_menu(target, nodes);
            }
        }
    }

    /// Menu de ícone do Dock: logo acima do ícone, com o bico em `center`.
    pub(crate) fn show_dock_menu(&mut self, i: usize, nodes: Vec<MenuNode>, center: f32, top: f32) {
        self.close_menu();
        let mut level = self.menu_level(nodes, (0, 0).into());
        let (sw, _) = self.output_size();
        let x = ((center - level.size.w as f32 / 2.0) as i32).clamp(4, (sw - level.size.w - 4).max(4));
        let y = (top as i32 - level.size.h - 12).max(MENUBAR_H + 4);
        level.origin = (x, y).into();
        self.chrome.menu = Some(OpenMenu { target: BarTarget::Dock(i), levels: vec![level], pointer_x: Some(center - x as f32) });
        self.dirty = true;
    }

    /// Menu de ícone com o Dock na lateral: ao lado do ícone (x, y, tamanho).
    pub(crate) fn show_dock_menu_side(&mut self, i: usize, nodes: Vec<MenuNode>, icon: (f32, f32, f32)) {
        self.close_menu();
        let mut level = self.menu_level(nodes, (0, 0).into());
        let (sw, sh) = self.output_size();
        let (ix, iy, s) = icon;
        let x = if self.chrome.dock_side == chrome::DockSide::Left { (ix + s + 12.0) as i32 } else { (ix - 12.0) as i32 - level.size.w };
        let y = ((iy + s / 2.0) as i32 - level.size.h / 2).clamp(MENUBAR_H + 4, (sh - level.size.h - 4).max(MENUBAR_H + 4));
        level.origin = (x.clamp(0, (sw - level.size.w).max(0)), y).into();
        self.chrome.menu = Some(OpenMenu { target: BarTarget::Dock(i), levels: vec![level], pointer_x: None });
        self.dirty = true;
    }

    pub(crate) fn show_menu(&mut self, target: BarTarget, nodes: Vec<MenuNode>) {
        let x = match target {
            BarTarget::Logo => self.chrome.logo_range.0,
            BarTarget::Title(i) => self.chrome.title_ranges.get(i).map(|r| r.0).unwrap_or(0.0),
            BarTarget::Extra(k) => self.chrome.extra_ranges.get(k).map(|r| r.0).unwrap_or(0.0),
            BarTarget::Dock(_) => 0.0,
        };
        let mut level = self.menu_level(nodes, (x as i32, MENUBAR_H).into());
        // Menus dos extras ficam encostados à direita se não couberem.
        let (sw, _) = self.output_size();
        level.origin.x = level.origin.x.min(sw - level.size.w).max(0);
        self.chrome.menu = Some(OpenMenu { target, levels: vec![level], pointer_x: None });
        self.dirty = true;
    }

    fn menu_level(&self, nodes: Vec<MenuNode>, origin: Point<i32, Logical>) -> MenuLevel {
        let items: Vec<MenuItem> = nodes.iter().map(|n| n.item.clone()).collect();
        let (w, h) = chrome::menu_content_size(&self.fonts, &items);
        MenuLevel { nodes, items, origin, size: (w as i32, h as i32).into(), hover: None, parent: None, key: None, buffer: None }
    }

    /// Menu principal, como o da era: Sobre, Preferências, Dock ▸, Itens Recentes ▸,
    /// Forçar Encerrar, Repouso, Reiniciar, Desligar e Encerrar Sessão.
    pub fn open_main_menu(&mut self) {
        let item = |label: &str, a: MenuAction| MenuNode::new(MenuItem::item(label), Some(a));
        let mag = if self.config.dock.magnification { "Desativar Ampliação" } else { "Ativar Ampliação" };
        let hide = if self.config.dock.autohide { "Desativar Ocultação" } else { "Ativar Ocultação" };
        let pos = |label: &str, word: &str| MenuNode::new(MenuItem::check(label, self.config.dock.position == word || (word == "embaixo" && !matches!(self.config.dock.position.as_str(), "esquerda" | "direita"))), Some(MenuAction::SetDockSide(word.to_string())));
        let dock = vec![
            item(hide, MenuAction::DockAutohide),
            item(mag, MenuAction::DockMagnification),
            MenuNode::sep(),
            pos("Posição à Esquerda", "esquerda"),
            pos("Posição Embaixo", "embaixo"),
            pos("Posição à Direita", "direita"),
            MenuNode::sep(),
            item("Preferências do Dock…", MenuAction::DockPreferences),
        ];
        let logout = if self.nested { "Sair do Glove…".to_string() } else { format!("Encerrar Sessão de {}…", user_full_name()) };
        let nodes = vec![
            item("Sobre o Glove", MenuAction::About),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Preferências…").with_shortcut(self.binding("preferences")), Some(MenuAction::Preferences)),
            MenuNode::submenu("Dock", dock),
            MenuNode::sep(),
            MenuNode::submenu("Itens Recentes", self.recent_nodes()),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Forçar Encerrar…").with_shortcut(self.binding("force_quit")), Some(MenuAction::ForceQuit)),
            MenuNode::sep(),
            MenuNode::new(MenuItem::item("Bloquear Tela").with_shortcut(self.binding("lock")), Some(MenuAction::Lock)),
            item("Repouso", MenuAction::Sleep),
            item("Reiniciar…", MenuAction::Restart),
            item("Desligar…", MenuAction::ShutDown),
            MenuNode::sep(),
            item(&logout, MenuAction::LogOut),
        ];
        self.show_menu(BarTarget::Logo, nodes);
    }

    /// Atalho configurado para uma ação (texto vazio se não houver).
    pub(crate) fn binding(&self, action: &str) -> String {
        self.config.shortcuts.iter().find(|s| s.action == action).map(|s| s.keys.clone()).unwrap_or_default()
    }

    /// Submenu Itens Recentes: títulos "Aplicativos" e "Documentos" (desativados,
    /// como na era), as listas e "Limpar Menu".
    fn recent_nodes(&self) -> Vec<MenuNode> {
        let header = |t: &str| MenuNode::new(MenuItem::item(t).disabled(), None);
        let none = || MenuNode::new(MenuItem::item("Nenhum").disabled(), None);
        let mut v = vec![header("Aplicativos")];
        let apps: Vec<MenuNode> = self
            .recent
            .apps
            .iter()
            .filter_map(|id| self.config.dock_apps.iter().find(|a| &a.app_id == id))
            .take(self.config.appearance.recent_apps as usize)
            .map(|a| MenuNode::new(MenuItem::item(format!("  {}", a.name)), Some(MenuAction::RecentApp(a.app_id.clone()))))
            .collect();
        let has_apps = !apps.is_empty();
        if has_apps { v.extend(apps) } else { v.push(none()) }
        v.push(MenuNode::sep());
        v.push(header("Documentos"));
        let docs: Vec<_> = self.recent.documents().into_iter().take(self.config.appearance.recent_docs as usize).collect();
        let has_docs = !docs.is_empty();
        for p in docs {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            v.push(MenuNode::new(MenuItem::item(format!("  {name}")), Some(MenuAction::RecentDocument(p))));
        }
        if !has_docs {
            v.push(none());
        }
        v.push(MenuNode::sep());
        let clear = MenuItem::item("Limpar Menu");
        v.push(MenuNode::new(if has_apps || has_docs { clear } else { clear.disabled() }, Some(MenuAction::ClearRecent)));
        v
    }

    pub fn close_menu(&mut self) {
        if self.chrome.menu.take().is_some() {
            self.dirty = true;
        }
    }

    /// Mouse (ou teclado) sobre o item `hover` do nível `level`: realça, fecha os
    /// submenus de outros itens e abre o submenu deste, se houver.
    pub fn menu_hover(&mut self, level: usize, hover: Option<usize>) {
        let Some(m) = &mut self.chrome.menu else { return };
        let Some(lv) = m.levels.get_mut(level) else { return };
        let hover = hover.filter(|&i| lv.nodes.get(i).is_some_and(MenuNode::selectable));
        if lv.hover != hover {
            lv.hover = hover;
            self.dirty = true;
        }
        if m.levels.len() > level + 1 {
            // Submenu já aberto deste mesmo item: fica.
            if hover.is_some() && m.levels[level + 1].parent == hover {
                return;
            }
            m.levels.truncate(level + 1);
            self.dirty = true;
        }
        self.open_submenu(level);
    }

    /// Abre o submenu do item realçado no nível `level` (se ele tiver um).
    fn open_submenu(&mut self, level: usize) -> bool {
        let Some(m) = &self.chrome.menu else { return false };
        if m.levels.len() > level + 1 {
            return false;
        }
        let lv = &m.levels[level];
        let Some(i) = lv.hover else { return false };
        let node = &lv.nodes[i];
        if node.sub.is_empty() || !node.selectable() {
            return false;
        }
        let top = chrome::menu_item_top(&lv.items, i) - chrome::menu_item_top(&lv.items, 0);
        let (px, py, pw) = (lv.origin.x, lv.origin.y + top as i32, lv.size.w);
        let mut sub = self.menu_level(node.sub.clone(), (0, 0).into());
        let (sw, sh) = self.output_size();
        // À direita do menu; sem espaço, à esquerda (como no Mac OS X).
        let mut x = px + pw - 1;
        if x + sub.size.w > sw {
            x = (px - sub.size.w + 1).max(0);
        }
        let y = py.min(sh - sub.size.h).max(MENUBAR_H);
        sub.origin = (x, y).into();
        sub.parent = Some(i);
        if let Some(m) = &mut self.chrome.menu {
            m.levels.push(sub);
        }
        self.dirty = true;
        true
    }

    /// Escolhe o item `i` do nível `level`: submenu abre, ação roda e o menu fecha.
    pub fn menu_activate(&mut self, level: usize, i: usize) {
        let Some(node) = self.chrome.menu.as_ref().and_then(|m| m.levels.get(level)).and_then(|lv| lv.nodes.get(i)).cloned() else { return };
        if !node.selectable() {
            return;
        }
        if !node.sub.is_empty() {
            // Submenu: só abre (o mouse em cima já abriu).
            self.menu_hover(level, Some(i));
            return;
        }
        self.close_menu();
        if let Some(a) = node.action {
            self.run_menu_action(a);
        }
    }

    /// Abre a barra de menu pelo teclado, com o primeiro item realçado.
    pub fn open_menubar_by_key(&mut self) {
        self.open_bar_menu(BarTarget::Logo);
        self.menu_key(MenuKey::Down);
    }

    /// Navegação pelo teclado no menu aberto.
    pub fn menu_key(&mut self, k: MenuKey) {
        let Some(m) = &self.chrome.menu else { return };
        let target = m.target;
        let depth = m.levels.len() - 1;
        let deepest = &m.levels[depth];
        match k {
            MenuKey::Up | MenuKey::Down => {
                let next = deepest.step(if k == MenuKey::Down { 1 } else { -1 });
                // Só realça: o submenu abre com → ou Enter.
                if let Some(m) = &mut self.chrome.menu {
                    m.levels[depth].hover = next;
                }
                self.dirty = true;
            }
            MenuKey::Right => {
                if self.open_submenu(depth) {
                    if let Some(m) = &mut self.chrome.menu {
                        let lv = m.levels.last_mut().unwrap();
                        lv.hover = lv.step(1);
                    }
                } else {
                    self.menu_neighbor(target, 1);
                }
            }
            MenuKey::Left => {
                if depth > 0 {
                    if let Some(m) = &mut self.chrome.menu {
                        m.levels.truncate(depth);
                    }
                    self.dirty = true;
                } else {
                    self.menu_neighbor(target, -1);
                }
            }
            MenuKey::Activate => match deepest.hover {
                Some(i) if !deepest.nodes[i].sub.is_empty() => self.menu_key(MenuKey::Right),
                Some(i) => self.menu_activate(depth, i),
                None => self.close_menu(),
            },
            MenuKey::Close => self.close_menu(),
            MenuKey::Other => {}
        }
    }

    /// Abre o menu vizinho na barra (dir = ±1), dando a volta, com o primeiro item realçado.
    fn menu_neighbor(&mut self, target: BarTarget, dir: i32) {
        let n = self.bar_titles().len() as i32 + 1; // logotipo + títulos
        let idx = match target {
            BarTarget::Logo | BarTarget::Extra(_) | BarTarget::Dock(_) => 0,
            BarTarget::Title(i) => i as i32 + 1,
        };
        let next = (idx + dir).rem_euclid(n);
        let t = if next == 0 { BarTarget::Logo } else { BarTarget::Title(next as usize - 1) };
        self.open_bar_menu(t);
        if let Some(m) = &mut self.chrome.menu {
            let lv = &mut m.levels[0];
            lv.hover = lv.step(1);
        }
    }

    pub fn run_menu_action(&mut self, a: MenuAction) {
        match a {
            MenuAction::About => self.spawn("glove-about"),
            MenuAction::Preferences => self.open_preferences(),
            MenuAction::DockMagnification => {
                self.config.dock.magnification = !self.config.dock.magnification;
                let _ = self.config.save();
                self.dirty = true;
            }
            MenuAction::DockPreferences => self.spawn("glove-prefs dock"),
            MenuAction::DockAutohide => {
                self.config.dock.autohide = !self.config.dock.autohide;
                let _ = self.config.save();
                self.dirty = true;
            }
            MenuAction::SetDockSide(side) => {
                self.config.dock.position = side;
                let _ = self.config.save();
                self.dirty = true;
            }
            MenuAction::RecentApp(id) => {
                if let Some(i) = self.config.dock_apps.iter().position(|a| a.app_id == id) {
                    self.run_dock_action(DockAction::App(i));
                }
            }
            MenuAction::RecentDocument(p) => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(p)
                    .env("WAYLAND_DISPLAY", &self.socket_name)
                    .spawn();
            }
            MenuAction::ClearRecent => self.recent.clear(),
            MenuAction::ForceQuit => self.open_force_quit(),
            MenuAction::Lock => self.lock_screen(),
            MenuAction::Sleep => self.sleep(),
            MenuAction::Restart => self.spawn("glove-sair reiniciar"),
            MenuAction::ShutDown => self.spawn("glove-sair desligar"),
            MenuAction::LogOut => self.spawn(if self.nested { "glove-sair sair" } else { "glove-sair encerrar" }),
            MenuAction::AboutApp(id, name) => self.spawn(&format!("glove-about --app {id} '{}'", name.replace('\'', ""))),
            MenuAction::HideApp(pid) => self.hide_pid(pid),
            MenuAction::HideOthers(pid) => self.hide_others(pid),
            MenuAction::ShowAll => {
                // Volta tudo, sem trocar o app da frente.
                let front = self.focused_window();
                self.unhide_where(|_| true);
                self.focus_window(front);
            }
            MenuAction::MinimizeWindow(w) => self.minimize_window(&w),
            MenuAction::ZoomWindow(w) => self.toggle_zoom(&w),
            MenuAction::BringAllToFront(pid) => self.bring_all_to_front(pid),
            MenuAction::FocusWindow(w) => self.bring_window(w),
            MenuAction::ToggleKeepInDock(id, pid) => self.toggle_keep_in_dock(&id, pid),
            MenuAction::ShowInFiles(cmd) => self.show_in_files(&cmd),
            MenuAction::DockRun(a) => self.run_dock_action(a),
            MenuAction::EmptyTrash => {
                self.spawn("glove-files --esvaziar-lixeira");
                self.chrome.trash_full.1 = None;
            }
            MenuAction::SetMode(output, mode) => {
                self.config.monitor.output = output;
                self.config.monitor.mode = mode;
                // Salvar faz o Glove recarregar a configuração e trocar a resolução.
                let _ = self.config.save();
            }
            MenuAction::ToggleBatteryPercent => {
                self.config.battery_percent = !self.config.battery_percent;
                let _ = self.config.save();
                self.dirty = true;
            }
            MenuAction::OpenPrefs(pane) => self.spawn(&format!("glove-prefs {pane}")),
            MenuAction::SetWifi(on) => self.set_wifi(on),
            MenuAction::SetLayout(id) => {
                let (l, v) = glove_config::split_layout(&id);
                let (l, v) = (l.to_string(), v.to_string());
                self.set_layout(&l, &v);
            }
            MenuAction::ClockAnalog(on) => {
                let mut c = self.config.clone();
                c.clock.analog = on;
                if let Err(e) = c.save() {
                    tracing::warn!("não salvou o relógio: {e}");
                }
                self.config.clock.analog = on;
                self.dirty = true;
            }
            MenuAction::JoinWifi(ssid, secure) => self.join_wifi(&ssid, secure),
            MenuAction::QuitApp(pid) => {
                for w in self.windows_of_pid(pid) {
                    self.close_window(&w);
                }
            }
            MenuAction::Client(pid, id) => {
                if let Some(c) = self.menu_clients.iter_mut().rev().find(|c| c.pid == pid) {
                    c.activate(&id);
                }
            }
        }
    }

    pub(crate) fn windows_of_pid(&self, pid: u32) -> Vec<Window> {
        use smithay::reexports::wayland_server::Resource;
        self.space
            .elements()
            .filter(|w| {
                window_surface(w)
                    .and_then(|s| self.display_handle.get_client(s.id()).ok())
                    .and_then(|c| c.get_credentials(&self.display_handle).ok())
                    .is_some_and(|c| c.pid as u32 == pid)
            })
            .cloned()
            .collect()
    }

    /// Abre as Preferências (ou traz para frente se já estiverem abertas).
    pub fn open_preferences(&mut self) {
        match self.windows_of_app("glove.Preferences").pop() {
            Some(w) => self.focus_window(Some(w)),
            None => self.spawn("glove-prefs"),
        }
    }

    pub fn set_theme(&mut self, v: Variant) {
        self.theme = Theme::new(v);
        self.theme_gen += 1;
        self.config.theme = v.id().to_string();
        let _ = self.config.save();
        self.dirty = true;
    }

    pub(crate) fn windows_of_app(&self, app_id: &str) -> Vec<Window> {
        if app_id.is_empty() {
            return Vec::new();
        }
        self.space.elements().filter(|w| window_info(w).1.eq_ignore_ascii_case(app_id)).cloned().collect()
    }

    pub fn run_dock_action(&mut self, a: DockAction) {
        match a {
            DockAction::App(i) => {
                let Some(app) = self.config.dock_apps.get(i).cloned() else { return };
                // Se já está aberto, traz para frente (ou mostra, se estava oculto); senão, abre.
                let id = app.app_id.clone();
                let was_hidden = !id.is_empty() && self.hidden.iter().any(|(w, _)| window_info(w).1.eq_ignore_ascii_case(&id));
                if was_hidden {
                    self.unhide_where(|w| window_info(w).1.eq_ignore_ascii_case(&id));
                } else if let Some(w) = self.windows_of_app(&app.app_id).pop() {
                    self.focus_window(Some(w));
                } else if let Some(idx) = self.minimized.iter().position(|(w, _)| window_info(w).1.eq_ignore_ascii_case(&app.app_id) && !app.app_id.is_empty()) {
                    self.restore_minimized(idx);
                } else {
                    self.spawn(&app.command);
                    self.start_bounce(&app.app_id);
                }
            }
            DockAction::Restore(i) => self.restore_minimized(i),
            DockAction::Running(id) => {
                let pid = self.every_window_pid(&id);
                if let Some(pid) = pid {
                    self.activate_app(pid);
                }
            }
            DockAction::Trash => {
                let c = self.config.trash_command.clone();
                self.spawn(&c);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Preparação dos desenhos (antes de cada quadro)
    // -----------------------------------------------------------------------

    /// Escala da tela (1.0, 1.5, 2.0…).
    pub fn scale(&self) -> f64 {
        self.space.outputs().next().map(|o| o.current_scale().fractional_scale()).unwrap_or(1.0)
    }

    pub fn prepare_chrome(&mut self) {
        let scale = self.scale();
        if scale != self.chrome.last_scale {
            self.chrome.last_scale = scale;
            self.theme_gen += 1;
        }
        let sc = scale as f32;
        let (w, h) = self.output_size();
        self.chrome.out_size = (w, h);
        let tgen = self.theme_gen;

        // Papel de parede
        let key = (w, h, tgen);
        if self.chrome.wallpaper_key != Some(key) {
            self.chrome.wallpaper = wallpaper::render_style(&self.theme, &self.config.wallpaper, w as f32, h as f32, sc).map(to_tex);
            self.chrome.wallpaper_key = Some(key);
        }

        // Barra de menu (nome do app + menus dele)
        let titles = self.bar_titles();
        // O relógio vai como o último extra (clicável); aqui fica vazio.
        let clock = String::new();
        let open = self.chrome.menu.as_ref().and_then(|m| match m.target {
            BarTarget::Logo => Some(0),
            BarTarget::Title(i) => Some(i + 1),
            BarTarget::Extra(_) | BarTarget::Dock(_) => None,
        });
        let open_extra = match self.chrome.menu.as_ref().map(|m| m.target) {
            Some(BarTarget::Extra(k)) => Some(k),
            _ => self.extras.slider.as_ref().and(self.volume_extra_index()),
        };
        let extras = self.extras_for_bar();
        let open_t = self.chrome.menu.as_ref().map(|m| m.target);
        let search_open = self.search.open;
        let key = (w, titles.clone(), extras.clone(), clock.clone(), open_t, open_extra, search_open, tgen);
        if self.chrome.menubar_key.as_ref() != Some(&key) {
            if let Some((pm, layout)) = chrome::menubar(&self.fonts, &self.theme, w as f32, &titles, &extras, &clock, open, open_extra, search_open, sc) {
                self.chrome.menubar = Some(to_tex(pm));
                self.chrome.logo_range = layout.logo;
                self.chrome.title_ranges = layout.titles;
                self.chrome.extra_ranges = layout.extras;
                self.chrome.search_range = layout.search;
            }
            self.chrome.menubar_key = Some(key);
        }

        // Menu aberto
        if let Some(m) = &mut self.chrome.menu {
            let pointer = m.pointer_x;
            for (l, lv) in m.levels.iter_mut().enumerate() {
                let key = (lv.hover, tgen);
                if lv.key != Some(key) {
                    lv.buffer = match pointer.filter(|_| l == 0) {
                        Some(px) => chrome::menu_with_pointer(&self.fonts, &self.theme, &lv.items, lv.hover, px, sc),
                        None => chrome::menu(&self.fonts, &self.theme, &lv.items, lv.hover, sc),
                    }
                    .map(to_tex);
                    lv.key = Some(key);
                }
            }
        }

        // Tela de encerramento
        let nested = self.nested;
        if let Some(e) = &mut self.ending {
            if e.screen_since.is_some() {
                let key = (w, h, tgen);
                if e.bg_key != Some(key) {
                    e.bg = chrome::ending_screen(&self.fonts, &self.theme, w as f32, h as f32, e.kind.label(nested), sc).map(to_tex);
                    e.bg_key = Some(key);
                }
                let key = (e.spinner_step(), tgen);
                if e.spinner_key != Some(key) {
                    e.spinner = chrome::ending_spinner(key.0 as f32 / 12.0, sc).map(to_tex);
                    e.spinner_key = Some(key);
                }
            }
        }

        // Ícones da mesa (disco e removíveis)
        let key = (self.chrome.desk_items.clone(), self.chrome.desk_selected, tgen);
        if self.chrome.desk_key.as_ref() != Some(&key) {
            self.chrome.desk_texs = self
                .chrome
                .desk_items
                .iter()
                .enumerate()
                .filter_map(|(i, (label, _))| chrome::desktop_icon(&self.fonts, &self.theme, Icon::Disk, label, self.chrome.desk_selected == Some(i), sc).map(to_tex))
                .collect();
            self.chrome.desk_key = Some(key);
        }

        self.prepare_force_quit();
        self.prepare_startup();
        self.prepare_search();
        self.prepare_expose();
        self.prepare_switcher();
        self.prepare_lock();
        self.prepare_extras();
        self.prepare_wait_cursor();
        self.prepare_dock();
        self.prepare_minimized();
        self.prepare_decorations();
    }

    fn dock_model(&self) -> (Vec<DockItem>, Vec<DockAction>) {
        let mut items = Vec::new();
        let mut actions = Vec::new();
        let running: Vec<String> = self
            .space
            .elements()
            .chain(self.minimized.iter().chain(&self.hidden).map(|(w, _)| w))
            .map(|w| window_info(w).1.to_lowercase())
            .collect();
        for (i, app) in self.config.dock_apps.iter().enumerate() {
            let mut icon = Icon::from_name(&app.icon);
            if let Icon::Clock(..) = icon {
                let (h, m) = local_hm();
                icon = Icon::Clock(h, m);
            }
            items.push(DockItem {
                icon,
                label: app.name.clone(),
                letter: app.name.chars().next(),
                running: !app.app_id.is_empty() && running.contains(&app.app_id.to_lowercase()),
                separator_before: false,
                bounce: self.dock_bounce(&app.app_id, self.config.dock.size.clamp(24, 128) as f32),
                thumbnail: false,
            });
            actions.push(DockAction::App(i));
        }
        // Apps abertos que não estão fixos: depois dos fixos.
        for (id, w) in self.running_unpinned() {
            let name = app_display_name(&w);
            items.push(DockItem { icon: Icon::Window, letter: name.chars().next(), label: name, running: true, separator_before: false, bounce: 0.0, thumbnail: false });
            actions.push(DockAction::Running(id));
        }
        for (i, (w, _)) in self.minimized.iter().enumerate() {
            let (title, app_id) = window_info(w);
            let label = if title.is_empty() { pretty_app_name(&app_id) } else { title };
            items.push(DockItem {
                icon: Icon::Window,
                letter: label.chars().next(),
                label,
                running: false,
                separator_before: i == 0,
                bounce: 0.0,
                thumbnail: true,
            });
            actions.push(DockAction::Restore(i));
        }
        // Lixeira sempre no fim, do outro lado do separador.
        let trash_icon = if self.chrome.trash_full.0 { Icon::TrashFull } else { Icon::Trash };
        items.push(DockItem { icon: trash_icon, label: "Lixeira".into(), letter: None, running: false, separator_before: self.minimized.is_empty(), bounce: 0.0, thumbnail: false });
        actions.push(DockAction::Trash);
        (items, actions)
    }

    pub fn prepare_dock(&mut self) {
        // Lixeira cheia ou vazia: confere no máximo uma vez por segundo.
        if self.chrome.trash_full.1.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(1)) {
            self.chrome.trash_full = (trash_has_items(), Some(std::time::Instant::now()));
        }
        let (items, actions) = self.dock_model();
        let d = &self.config.dock;
        let base = d.size.clamp(24, 128) as f32;
        let max = if d.magnification { (d.max_size as f32).max(base) } else { base };
        // Animação da ampliação: avança pelo relógio e pede outro quadro até acabar.
        if let Some((from, to, start)) = self.chrome.dock_zoom_anim {
            let t = (start.elapsed().as_secs_f32() / DOCK_ZOOM_SECS).min(1.0);
            // Começa e termina devagar.
            let e = t * t * (3.0 - 2.0 * t);
            self.chrome.dock_zoom = from + (to - from) * e;
            if t >= 1.0 {
                self.chrome.dock_zoom_anim = None;
            } else {
                self.dirty = true;
            }
        }
        // Ocultar automaticamente: desliza pelo relógio, como a ampliação.
        if let Some((from, to, start)) = self.chrome.dock_hide_anim {
            let t = (start.elapsed().as_secs_f32() / 0.22).min(1.0);
            let e = t * t * (3.0 - 2.0 * t);
            self.chrome.dock_hide = from + (to - from) * e;
            if t >= 1.0 {
                self.chrome.dock_hide_anim = None;
            } else {
                self.dirty = true;
            }
        }
        if !self.config.dock.autohide && self.chrome.dock_hide != 0.0 && self.chrome.dock_hide_anim.is_none() {
            self.chrome.dock_hide = 0.0;
        }
        self.chrome.dock_side = chrome::DockSide::from_config(&self.config.dock.position);
        // Muitos ícones: encolhe até caber na borda (como no Mac).
        let (sw, sh) = self.output_size();
        let room = if self.chrome.dock_side.vertical() { sh - MENUBAR_H - 8 } else { sw - 8 } as f32;
        let (mut base, mut max) = (base, max);
        while base > 16.0 && chrome::dock_length(&items, base, max) > room {
            let k = (base - 1.0) / base;
            base -= 1.0;
            max = (max * k).max(base);
        }
        let zoom = self.chrome.dock_zoom;
        let hover_x = self.chrome.dock_hover_x.or((zoom > 0.0).then_some(self.chrome.dock_last_x));
        let layout = chrome::dock_layout_on(&items, hover_x, zoom, base, max, self.chrome.dock_side);
        // Com o menu de um ícone aberto, o nome sob o mouse some (o menu já diz qual é).
        let hover = if self.dock_menu_open() { None } else { self.chrome.dock_hover };
        let key = (items.clone(), hover_x.map(|x| x.round() as i32), (zoom * 1000.0) as i32, hover, self.theme_gen ^ ((base as u64) << 32) ^ ((max as u64) << 44));
        if self.chrome.dock_key.as_ref() != Some(&key) {
            self.chrome.dock = chrome::dock(&self.fonts, &self.theme, &items, &layout, hover, self.scale() as f32).map(to_tex);
            self.chrome.dock_key = Some(key);
        }
        self.chrome.dock_items = items;
        self.chrome.dock_actions = actions;
        self.chrome.dock_layout = Some(layout);
    }

    fn prepare_decorations(&mut self) {
        let sc = self.scale() as f32;
        let focused = self.focused_window();
        let tgen = self.theme_gen;
        for w in self.space.elements() {
            w.user_data().insert_if_missing(|| RefCell::new(Deco::default()));
            let mut deco = w.user_data().get::<RefCell<Deco>>().unwrap().borrow_mut();
            if !is_ssd(w) {
                if deco.title.is_some() || !deco.shadows.is_empty() {
                    *deco = Deco::default();
                }
                continue;
            }
            let size = w.geometry().size;
            if size.w <= 0 || size.h <= 0 {
                continue;
            }
            let active = focused.as_ref() == Some(w);
            let st = TitleBarState {
                active,
                hover: self.chrome.hover_buttons.as_ref() == Some(w),
                pressed: self.chrome.pressed_button.as_ref().filter(|(pw, _)| pw == w).map(|(_, b)| *b),
                zoom_enabled: !fixed_size(w),
            };
            let (title, app_id) = window_info(w);
            let tkey = (size.w, st.clone(), title.clone(), tgen);
            if deco.title_key.as_ref() != Some(&tkey) {
                // Janelas "unificadas" do Tiger (o Arquivos): título em metal escovado,
                // emendado na barra de ferramentas do app, em qualquer tema.
                let metal;
                let theme = if UNIFIED_APPS.contains(&app_id.as_str()) {
                    metal = Theme::new(Variant::Metal);
                    &metal
                } else {
                    &self.theme
                };
                deco.title = chrome::titlebar(&self.fonts, theme, size.w as f32, &title, &st, sc).map(to_tex);
                deco.title_key = Some(tkey);
            }
            let skey = (size.w, size.h, active, tgen);
            if deco.shadow_key != Some(skey) {
                deco.shadows = chrome::window_shadow(size.w, size.h + TITLE_H, active, sc)
                    .into_iter()
                    .map(|(x, y, p)| (x, y, to_tex(p)))
                    .collect();
                deco.shadow_key = Some(skey);
            }
        }
    }
}

/// Caminho de um programa do Glove instalado ao lado do executável principal.
pub fn sibling_binary(name: &str) -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// (hora, minuto) locais.
fn local_hm() -> (u8, u8) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_hour as u8, tm.tm_min as u8)
    }
}

/// Hora local: (dia da semana, dia, mês 1-12, ano, hora, minuto, segundo).
pub fn local_now() -> (u32, u32, u32, i32, u32, u32, u32) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_wday as u32, tm.tm_mday as u32, tm.tm_mon as u32 + 1, tm.tm_year + 1900, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32)
    }
}

/// "Ter 19:47" no horário local, como a aba Relógio pede.
pub fn clock_text(c: &glove_config::Clock) -> String {
    let (wd, d, mo, _, h, m, s) = local_now();
    c.text(wd, d, mo, h, m, s)
}

/// Itens do menu de um app (com submenus) já ligados às ações.
pub(crate) fn client_nodes(pid: u32, entries: Vec<crate::menus::Entry>) -> Vec<MenuNode> {
    entries
        .into_iter()
        .map(|e| {
            if e.separator {
                return MenuNode::sep();
            }
            if e.is_sub {
                let mut n = MenuNode::submenu(e.label, client_nodes(pid, e.sub));
                if !e.enabled {
                    n.item = n.item.disabled();
                }
                return n;
            }
            let mut it = MenuItem::check(e.label, e.checked).with_shortcut(e.shortcut);
            if !e.enabled {
                it = it.disabled();
            }
            let action = e.enabled.then(|| MenuAction::Client(pid, e.id));
            MenuNode::new(it, action)
        })
        .collect()
}

/// Nome completo do usuário (campo de comentário do /etc/passwd) ou o login.
pub(crate) fn user_full_name() -> String {
    let uid = unsafe { libc::getuid() };
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let entry = passwd.lines().map(|l| l.split(':').collect::<Vec<_>>()).find(|f| f.len() > 4 && f[2].parse() == Ok(uid));
    let login = std::env::var("USER").unwrap_or_default();
    match entry {
        Some(f) => {
            let full = f[4].split(',').next().unwrap_or("").trim();
            if full.is_empty() { f[0].to_string() } else { full.to_string() }
        }
        None => login,
    }
}

// ---------------------------------------------------------------------------
// Ícone que pula no Dock enquanto o app abre
// ---------------------------------------------------------------------------

/// Um pulo (subida e descida).
const HOP: f32 = 0.55;
/// Para de pular se a janela não aparecer nesse tempo.
const BOUNCE_MAX: std::time::Duration = std::time::Duration::from_secs(8);

impl Glove {
    pub fn start_bounce(&mut self, app_id: &str) {
        if app_id.is_empty() || !self.config.dock.animate_launch || self.launching.iter().any(|l| l.0.eq_ignore_ascii_case(app_id)) {
            return;
        }
        self.launching.push((app_id.to_string(), std::time::Instant::now(), None));
        if self.launching.len() == 1 {
            self.loop_handle
                .insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(16)), |_, _, data| {
                    let st = &mut data.state;
                    let now = std::time::Instant::now();
                    st.launching.retain(|(_, start, end)| end.map_or(start.elapsed() < BOUNCE_MAX, |e| now < e));
                    st.dirty = true;
                    if st.launching.is_empty() {
                        smithay::reexports::calloop::timer::TimeoutAction::Drop
                    } else {
                        smithay::reexports::calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_millis(16))
                    }
                })
                .ok();
        }
    }

    /// A janela do app apareceu: termina o pulo atual e para.
    pub fn stop_bounce(&mut self, app_id: &str) {
        for (id, start, end) in &mut self.launching {
            if end.is_none() && id.eq_ignore_ascii_case(app_id) {
                let t = start.elapsed().as_secs_f32();
                let left = HOP - t % HOP;
                *end = Some(std::time::Instant::now() + std::time::Duration::from_secs_f32(left));
            }
        }
    }

    /// Altura do pulo do ícone agora (parábola: sobe rápido, freia no alto, cai).
    pub fn dock_bounce(&self, app_id: &str, icon: f32) -> f32 {
        let Some((_, start, end)) = self.launching.iter().find(|l| !app_id.is_empty() && l.0.eq_ignore_ascii_case(app_id)) else { return 0.0 };
        if end.is_some_and(|e| std::time::Instant::now() >= e) {
            return 0.0;
        }
        let f = (start.elapsed().as_secs_f32() / HOP).fract();
        icon * 0.5 * 4.0 * f * (1.0 - f)
    }
}

/// Discos removíveis montados (pendrives, cartões): /media/<usuário>/… e
/// /run/media/<usuário>/… em /proc/mounts, com o nome da pasta.
pub fn removable_mounts() -> Vec<(String, std::path::PathBuf)> {
    let user = std::env::var("USER").unwrap_or_default();
    parse_mounts(&std::fs::read_to_string("/proc/mounts").unwrap_or_default(), &user)
}

fn parse_mounts(text: &str, user: &str) -> Vec<(String, std::path::PathBuf)> {
    let prefixes = [format!("/media/{user}/"), format!("/run/media/{user}/")];
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(mp) = l.split_whitespace().nth(1) else { continue };
        // Espaços vêm como \040 no /proc/mounts.
        let mp = mp.replace("\\040", " ");
        if prefixes.iter().any(|p| mp.starts_with(p.as_str())) {
            let p = std::path::PathBuf::from(&mp);
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if !out.iter().any(|(_, q)| *q == p) {
                out.push((name, p));
            }
        }
    }
    out
}

#[cfg(test)]
mod mesa_tests {
    #[test]
    fn pendrives() {
        let m = "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /media/ana/MEU\\040PEN vfat rw 0 0\n/dev/sdc1 /run/media/ana/Fotos exfat rw 0 0\n/dev/sdd1 /media/beto/X vfat rw 0 0\n";
        let v = super::parse_mounts(m, "ana");
        assert_eq!(v.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["MEU PEN", "Fotos"]);
    }
}
