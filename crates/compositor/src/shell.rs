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
    with_states(s, |st| {
        st.data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|d| d.lock().ok().map(|d| (d.title.clone().unwrap_or_default(), d.app_id.clone().unwrap_or_default())))
            .unwrap_or_default()
    })
}

/// A janela pediu (e recebeu) bordas desenhadas pelo Glove?
pub fn is_ssd(w: &Window) -> bool {
    w.toplevel().map(|t| t.current_state().decoration_mode == Some(DecoMode::ServerSide)).unwrap_or(false)
}

pub fn deco_top(w: &Window) -> i32 {
    if is_ssd(w) { TITLE_H } else { 0 }
}

/// Nome curto e bonito a partir do app_id ("org.gnome.TextEditor" → "TextEditor").
fn pretty_app_name(app_id: &str) -> String {
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
    DockPreferences,
    /// Abre um app dos Itens Recentes (app_id).
    RecentApp(String),
    /// Abre um documento dos Itens Recentes.
    RecentDocument(std::path::PathBuf),
    ClearRecent,
    ForceQuit,
    Sleep,
    Restart,
    ShutDown,
    LogOut,
    /// Minimiza todas as janelas do app em foco.
    HideApp(u32),
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
    Restore(usize),
    Trash,
}

#[derive(Default)]
pub struct Chrome {
    pub out_size: (i32, i32),
    wallpaper_key: Option<(i32, i32, u64)>,
    pub wallpaper: Option<Tex>,
    menubar_key: Option<(i32, Vec<String>, String, Option<BarTarget>, u64)>,
    pub menubar: Option<Tex>,
    pub logo_range: (f32, f32),
    /// Faixas dos títulos de menu (o primeiro é o nome do app).
    pub title_ranges: Vec<(f32, f32)>,

    pub dock_items: Vec<DockItem>,
    pub dock_actions: Vec<DockAction>,
    pub dock_layout: Option<DockLayout>,
    pub dock_hover_x: Option<f32>,
    /// Última posição do mouse no Dock (a ampliação some a partir dela ao sair).
    pub dock_last_x: f32,
    /// Força da ampliação (0 a 1) e a animação em curso: (de, para, início).
    pub dock_zoom: f32,
    pub dock_zoom_anim: Option<(f32, f32, std::time::Instant)>,
    pub dock_hover: Option<usize>,
    pub dock_pressed: Option<usize>,
    dock_key: Option<(Vec<DockItem>, Option<i32>, i32, Option<usize>, u64)>,
    pub dock: Option<Tex>,
    /// Escala usada no último desenho (mudou → refaz tudo).
    pub last_scale: f64,

    pub menu: Option<OpenMenu>,
    /// Ícone do disco na mesa.
    pub desk_icon: Option<Tex>,
    desk_key: Option<(bool, u64)>,
    pub desk_selected: bool,
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
        match &self.dock_layout {
            Some(l) => ((w - l.width as i32) / 2, h - l.height as i32).into(),
            None => (0, h).into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Teste de clique
// ---------------------------------------------------------------------------

pub enum Hit {
    /// Menu aberto: (nível, item).
    Menu(usize, Option<usize>),
    MenuBar { target: Option<BarTarget> },
    Dock(Option<usize>),
    Title { window: Window, button: Option<TitleButton>, group: bool },
    Resize(Window, ResizeEdge),
    Client(Window),
    /// Ícone na mesa (o disco).
    DesktopIcon,
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

        if let Some(m) = &self.chrome.menu {
            // O submenu mais interno fica por cima.
            for (l, lv) in m.levels.iter().enumerate().rev() {
                if Rectangle::new(lv.origin, lv.size).to_f64().contains(pos) {
                    return Hit::Menu(l, chrome::menu_item_at(&lv.items, (py - lv.origin.y as f64) as f32));
                }
            }
        }
        if py < MENUBAR_H as f64 {
            let (a, b) = self.chrome.logo_range;
            if px >= a as f64 && px < b as f64 {
                return Hit::MenuBar { target: Some(BarTarget::Logo) };
            }
            let t = self.chrome.title_ranges.iter().position(|(a, b)| px >= *a as f64 && px < *b as f64);
            return Hit::MenuBar { target: t.map(BarTarget::Title) };
        }
        if let Some(l) = &self.chrome.dock_layout {
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
        if self.desk_icon_rect().to_f64().contains(pos) {
            return Hit::DesktopIcon;
        }
        Hit::Desktop
    }

    /// Onde fica o ícone do disco (canto superior direito, abaixo da barra de menu).
    pub fn desk_icon_rect(&self) -> Rectangle<i32, Logical> {
        let (w, _) = self.output_size();
        Rectangle::new((w - chrome::DESK_ICON_W as i32 - 12, MENUBAR_H + 12).into(), (chrome::DESK_ICON_W as i32, chrome::DESK_ICON_H as i32).into())
    }

    /// Clique no ícone do disco: seleciona; clique duplo abre o Arquivos.
    pub fn click_desk_icon(&mut self) {
        let now = std::time::Instant::now();
        let double = self.chrome.desk_last_click.is_some_and(|t| now.duration_since(t).as_millis() < 450);
        self.chrome.desk_selected = true;
        self.chrome.desk_last_click = Some(now);
        if double {
            self.chrome.desk_last_click = None;
            self.spawn("glove-files /");
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
        self.focus_topmost(None);
    }

    pub fn restore_minimized(&mut self, idx: usize) {
        if idx >= self.minimized.len() {
            return;
        }
        let (w, loc) = self.minimized.remove(idx);
        self.space.map_element(w.clone(), loc, true);
        self.focus_window(Some(w));
    }

    /// Área útil: abaixo da barra de menu e acima do Dock.
    pub fn work_area(&self) -> Rectangle<i32, Logical> {
        let (w, h) = self.output_size();
        // Espaço reservado embaixo para o Dock (tamanho dos ícones + prateleira).
        let reserve = (self.config.dock.size.clamp(24, 128) as f32 * 0.62 + 30.0) as i32;
        Rectangle::new((0, MENUBAR_H).into(), (w, (h - MENUBAR_H - reserve).max(100)).into())
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
        if self.config.dock_apps.iter().any(|a| a.app_id == app_id) {
            self.recent.add_app(&app_id);
        }
        let area = self.work_area();
        let top = deco_top(w);
        let off = (self.cascade % 6) * 26;
        self.cascade += 1;
        let x = area.loc.x + ((area.size.w - size.w) / 2).max(0) + off - 65;
        let y = area.loc.y + top + ((area.size.h - size.h - top) / 3).max(8) + off;
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

    fn focused_app_name(&self) -> String {
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
    fn focused_menus(&self) -> Vec<crate::menus::AppMenu> {
        let Some(pid) = self.focused_pid() else { return Vec::new() };
        self.menu_clients.iter().rev().find(|c| c.pid == pid).map(|c| c.menus.clone()).unwrap_or_default()
    }

    /// Títulos da barra: nome do app + menus dele.
    pub fn bar_titles(&self) -> Vec<String> {
        let mut t = vec![self.focused_app_name()];
        t.extend(self.focused_menus().into_iter().map(|m| m.title));
        t
    }

    pub fn open_bar_menu(&mut self, target: BarTarget) {
        match target {
            BarTarget::Logo => self.open_main_menu(),
            BarTarget::Title(0) => {
                let app = self.focused_app_name();
                let pid = self.focused_pid().unwrap_or(0);
                let has_window = self.focused_window().is_some();
                let item = |label: String| if has_window { MenuItem::item(label) } else { MenuItem::item(label).disabled() };
                let nodes = vec![
                    MenuNode::new(item(format!("Ocultar {app}")), Some(MenuAction::HideApp(pid))),
                    MenuNode::sep(),
                    MenuNode::new(item(format!("Encerrar {app}")), Some(MenuAction::QuitApp(pid))),
                ];
                self.show_menu(target, nodes);
            }
            BarTarget::Title(i) => {
                let pid = self.focused_pid().unwrap_or(0);
                let Some(menu) = self.focused_menus().into_iter().nth(i - 1) else { return };
                let nodes = client_nodes(pid, menu.items);
                self.show_menu(target, nodes);
            }
        }
    }

    fn show_menu(&mut self, target: BarTarget, nodes: Vec<MenuNode>) {
        let x = match target {
            BarTarget::Logo => self.chrome.logo_range.0,
            BarTarget::Title(i) => self.chrome.title_ranges.get(i).map(|r| r.0).unwrap_or(0.0),
        };
        let level = self.menu_level(nodes, (x as i32, MENUBAR_H).into());
        self.chrome.menu = Some(OpenMenu { target, levels: vec![level] });
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
        let dock = vec![item(mag, MenuAction::DockMagnification), MenuNode::sep(), item("Preferências do Dock…", MenuAction::DockPreferences)];
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
            item("Repouso", MenuAction::Sleep),
            item("Reiniciar…", MenuAction::Restart),
            item("Desligar…", MenuAction::ShutDown),
            MenuNode::sep(),
            item(&logout, MenuAction::LogOut),
        ];
        self.show_menu(BarTarget::Logo, nodes);
    }

    /// Atalho configurado para uma ação (texto vazio se não houver).
    fn binding(&self, action: &str) -> String {
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
            .map(|a| MenuNode::new(MenuItem::item(format!("  {}", a.name)), Some(MenuAction::RecentApp(a.app_id.clone()))))
            .collect();
        let has_apps = !apps.is_empty();
        if has_apps { v.extend(apps) } else { v.push(none()) }
        v.push(MenuNode::sep());
        v.push(header("Documentos"));
        let docs = self.recent.documents();
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

    /// Fim de sessão confirmado no glove-sair.
    pub fn end_session(&mut self, kind: &str) {
        let cmd = match kind {
            "desligar" => Some("poweroff"),
            "reiniciar" => Some("reboot"),
            "encerrar" | "sair" => None,
            _ => return,
        };
        tracing::info!("fim de sessão: {kind}");
        if let Some(c) = cmd {
            // Pede ao systemd antes de sair (o pedido é rápido; a máquina desliga depois).
            if let Err(e) = std::process::Command::new("systemctl").arg(c).status() {
                tracing::warn!("systemctl {c}: {e}");
                return;
            }
        }
        self.loop_signal.stop();
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
            BarTarget::Logo => 0,
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
            // Até a janela própria (etapa 3.5), o Monitor faz o papel de Forçar Encerrar.
            MenuAction::ForceQuit => self.spawn("glove-monitor"),
            MenuAction::Sleep => {
                let _ = std::process::Command::new("systemctl").arg("suspend").spawn();
            }
            MenuAction::Restart => self.spawn("glove-sair reiniciar"),
            MenuAction::ShutDown => self.spawn("glove-sair desligar"),
            MenuAction::LogOut => self.spawn(if self.nested { "glove-sair sair" } else { "glove-sair encerrar" }),
            MenuAction::HideApp(pid) => {
                for w in self.windows_of_pid(pid) {
                    self.minimize_window(&w);
                }
            }
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

    fn windows_of_pid(&self, pid: u32) -> Vec<Window> {
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

    fn windows_of_app(&self, app_id: &str) -> Vec<Window> {
        if app_id.is_empty() {
            return Vec::new();
        }
        self.space.elements().filter(|w| window_info(w).1.eq_ignore_ascii_case(app_id)).cloned().collect()
    }

    pub fn run_dock_action(&mut self, a: DockAction) {
        match a {
            DockAction::App(i) => {
                let Some(app) = self.config.dock_apps.get(i).cloned() else { return };
                // Se já está aberto, traz para frente; senão, abre.
                if let Some(w) = self.windows_of_app(&app.app_id).pop() {
                    self.focus_window(Some(w));
                } else if let Some(idx) = self.minimized.iter().position(|(w, _)| window_info(w).1.eq_ignore_ascii_case(&app.app_id) && !app.app_id.is_empty()) {
                    self.restore_minimized(idx);
                } else {
                    self.spawn(&app.command);
                }
            }
            DockAction::Restore(i) => self.restore_minimized(i),
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
        let clock = clock_text();
        let open = self.chrome.menu.as_ref().map(|m| match m.target {
            BarTarget::Logo => 0,
            BarTarget::Title(i) => i + 1,
        });
        let open_t = self.chrome.menu.as_ref().map(|m| m.target);
        let key = (w, titles.clone(), clock.clone(), open_t, tgen);
        if self.chrome.menubar_key.as_ref() != Some(&key) {
            if let Some((pm, layout)) = chrome::menubar(&self.fonts, &self.theme, w as f32, &titles, &clock, open, sc) {
                self.chrome.menubar = Some(to_tex(pm));
                self.chrome.logo_range = layout.logo;
                self.chrome.title_ranges = layout.titles;
            }
            self.chrome.menubar_key = Some(key);
        }

        // Menu aberto
        if let Some(m) = &mut self.chrome.menu {
            for lv in &mut m.levels {
                let key = (lv.hover, tgen);
                if lv.key != Some(key) {
                    lv.buffer = chrome::menu(&self.fonts, &self.theme, &lv.items, lv.hover, sc).map(to_tex);
                    lv.key = Some(key);
                }
            }
        }

        // Ícone do disco
        let key = (self.chrome.desk_selected, tgen);
        if self.chrome.desk_key != Some(key) {
            self.chrome.desk_icon = chrome::desktop_icon(&self.fonts, &self.theme, Icon::Disk, "Disco", self.chrome.desk_selected, sc).map(to_tex);
            self.chrome.desk_key = Some(key);
        }

        self.prepare_dock();
        self.prepare_decorations();
    }

    fn dock_model(&self) -> (Vec<DockItem>, Vec<DockAction>) {
        let mut items = Vec::new();
        let mut actions = Vec::new();
        let running: Vec<String> = self
            .space
            .elements()
            .chain(self.minimized.iter().map(|(w, _)| w))
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
            });
            actions.push(DockAction::App(i));
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
            });
            actions.push(DockAction::Restore(i));
        }
        // Lixeira sempre no fim, do outro lado do separador.
        items.push(DockItem { icon: Icon::Trash, label: "Lixeira".into(), letter: None, running: false, separator_before: self.minimized.is_empty() });
        actions.push(DockAction::Trash);
        (items, actions)
    }

    pub fn prepare_dock(&mut self) {
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
        let zoom = self.chrome.dock_zoom;
        let hover_x = self.chrome.dock_hover_x.or((zoom > 0.0).then_some(self.chrome.dock_last_x));
        let layout = chrome::dock_layout(&items, hover_x, zoom, base, max);
        let key = (items.clone(), hover_x.map(|x| x.round() as i32), (zoom * 1000.0) as i32, self.chrome.dock_hover, self.theme_gen ^ ((base as u64) << 32) ^ ((max as u64) << 44));
        if self.chrome.dock_key.as_ref() != Some(&key) {
            self.chrome.dock = chrome::dock(&self.fonts, &self.theme, &items, &layout, self.chrome.dock_hover, self.scale() as f32).map(to_tex);
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
            let (title, _) = window_info(w);
            let tkey = (size.w, st.clone(), title.clone(), tgen);
            if deco.title_key.as_ref() != Some(&tkey) {
                deco.title = chrome::titlebar(&self.fonts, &self.theme, size.w as f32, &title, &st, sc).map(to_tex);
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

/// "Ter 19:47" no horário local.
pub fn clock_text() -> String {
    const DIAS: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{} {:02}:{:02}", DIAS[(tm.tm_wday as usize) % 7], tm.tm_hour, tm.tm_min)
    }
}

/// Itens do menu de um app (com submenus) já ligados às ações.
fn client_nodes(pid: u32, entries: Vec<crate::menus::Entry>) -> Vec<MenuNode> {
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
fn user_full_name() -> String {
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
