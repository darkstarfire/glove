//! Peças da interface desenhadas pelo compositor: barra de título,
//! barra de menu, menus suspensos e o Dock. Tudo em coordenadas lógicas,
//! gerado na escala da tela.

use crate::draw::*;
use crate::font::Fonts;
use crate::icons::{self, Icon};
use crate::theme::{MENUBAR_H, TITLE_H, Theme, Variant};
use tiny_skia::Color;

// ---------------------------------------------------------------------------
// Barra de título
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TitleButton {
    Close,
    Minimize,
    Zoom,
}

pub const BUTTON_R: f32 = 6.5;
const BUTTONS: [(TitleButton, f32); 3] = [(TitleButton::Close, 14.0), (TitleButton::Minimize, 34.0), (TitleButton::Zoom, 54.0)];

/// Qual botão está sob (x, y), em coordenadas da barra de título.
pub fn title_button_at(x: f32, y: f32) -> Option<TitleButton> {
    let cy = TITLE_H as f32 / 2.0;
    BUTTONS
        .iter()
        .find(|(_, cx)| (x - cx).powi(2) + (y - cy).powi(2) <= (BUTTON_R + 2.0).powi(2))
        .map(|(b, _)| *b)
}

/// A área dos três botões (para mostrar os símbolos ao passar o mouse).
pub fn over_button_group(x: f32, y: f32) -> bool {
    (4.0..66.0).contains(&x) && (2.0..(TITLE_H as f32 - 2.0)).contains(&y)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TitleBarState {
    pub active: bool,
    pub hover: bool,
    pub pressed: Option<TitleButton>,
    /// Zoom disponível (janelas de tamanho fixo mostram o botão desbotado).
    pub zoom_enabled: bool,
}

pub fn titlebar(fonts: &Fonts, theme: &Theme, w: f32, title: &str, st: &TitleBarState, scale: f32) -> Option<Canvas> {
    let h = TITLE_H as f32;
    let mut c = Canvas::new(w, h, scale)?;

    match theme.variant {
        Variant::Metal => {
            let (top, bot) = if st.active {
                (theme.metal_top, theme.metal_bottom)
            } else {
                (lighten(theme.metal_top, 0.3), lighten(theme.metal_bottom, 0.4))
            };
            c.brushed_metal(0.0, 0.0, w, h, top, bot);
            c.metal_sheen(0.0, 0.0, w, h, w);
        }
        Variant::Claro => {
            // Degradê cinza claro e liso, sem listras (10.4). A inativa fica mais clara.
            let (top, bot) = if st.active {
                (rgba(244, 244, 244, 255), rgba(232, 232, 232, 255))
            } else {
                (rgba(248, 248, 248, 255), rgba(240, 240, 240, 255))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
        }
        Variant::Listras => {
            // Listras claras, quase sem degradê (10.2). A inativa fica translúcida.
            let (top, bot) = if st.active {
                (rgba(246, 246, 246, 255), rgba(226, 226, 226, 255))
            } else {
                (rgba(250, 250, 250, 200), rgba(238, 238, 238, 200))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
            let a = if st.active { 0.6 } else { 0.35 };
            c.pinstripes(0.0, 0.0, w, h, rgba(255, 255, 255, 0), with_alpha(Color::WHITE, a));
        }
        _ => {
            let (top, bot) = if st.active {
                (rgba(248, 248, 248, 255), rgba(206, 206, 208, 255))
            } else {
                (rgba(250, 250, 250, 255), rgba(232, 232, 234, 255))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
            let a = if st.active { 0.35 } else { 0.18 };
            c.pinstripes(0.0, 0.0, w, h, rgba(255, 255, 255, 0), with_alpha(Color::WHITE, a));
        }
    }
    c.hline(0.0, 0.0, w, rgba(255, 255, 255, 200));
    let edge = if theme.variant == Variant::Claro { rgba(150, 150, 152, 255) } else { rgba(120, 120, 124, 255) };
    // Metal: título e barra de ferramentas são uma peça só (sem a linha entre eles).
    if theme.variant != Variant::Metal {
        c.hline(0.0, h - 1.0 / scale, w, edge);
    }
    c.clip_top_corners(6.0);

    let cy = h / 2.0;
    for (b, cx) in BUTTONS {
        let mut base = if st.active {
            match b {
                TitleButton::Close => theme.close,
                TitleButton::Minimize => theme.minimize,
                TitleButton::Zoom => theme.zoom,
            }
        } else {
            theme.inactive_button
        };
        if st.pressed == Some(b) {
            base = darken(base, 0.3);
        }
        let enabled = b != TitleButton::Zoom || st.zoom_enabled;
        if !st.active && theme.variant == Variant::Listras {
            // Inativa no 10.2: só o contorno do botão, com um brilho por dentro.
            c.fill(circle(cx, cy, BUTTON_R), &vgrad(cy - BUTTON_R, cy + BUTTON_R, &[(0.0, rgba(255, 255, 255, 120)), (1.0, rgba(225, 225, 228, 120))]));
            c.stroke(circle(cx, cy, BUTTON_R - 0.5), &solid(rgba(168, 168, 168, 255)), 1.0);
            c.fill(oval(cx - BUTTON_R * 0.55, cy - BUTTON_R * 0.85, BUTTON_R * 1.1, BUTTON_R * 0.7), &solid(rgba(255, 255, 255, 170)));
            continue;
        }
        if !enabled {
            // Indisponível: o mesmo gel, desbotado.
            gel_circle(&mut c, cx, cy, BUTTON_R, with_alpha(lighten(base, 0.35), 0.4));
            continue;
        }
        gel_circle(&mut c, cx, cy, BUTTON_R, base);
        if st.hover {
            let ink = solid(with_alpha(darken(base, 0.7), 0.85));
            let (l, lw) = (2.6, 1.4);
            match b {
                TitleButton::Close => {
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(cx - l, cy - l);
                    pb.line_to(cx + l, cy + l);
                    pb.move_to(cx + l, cy - l);
                    pb.line_to(cx - l, cy + l);
                    c.stroke(pb.finish(), &ink, lw);
                }
                TitleButton::Minimize => c.fill_rect(cx - l - 0.5, cy - lw / 2.0, 2.0 * l + 1.0, lw, &ink),
                TitleButton::Zoom => {
                    c.fill_rect(cx - l - 0.5, cy - lw / 2.0, 2.0 * l + 1.0, lw, &ink);
                    c.fill_rect(cx - lw / 2.0, cy - l - 0.5, lw, 2.0 * l + 1.0, &ink);
                }
            }
        }
    }

    let px = 13.0;
    let avail = w - 2.0 * 72.0;
    if avail > 20.0 {
        let t = fonts.ellipsize(title, px, false, avail);
        let tw = fonts.measure(&t, px, false);
        let x = ((w - tw) / 2.0).max(72.0);
        let color = if st.active { theme.title_text } else { theme.title_text_inactive };
        fonts.draw_embossed(&mut c, &t, x, 15.5, px, false, color, rgba(255, 255, 255, 180));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Sombras de janela (4 faixas em volta, recalculadas só ao redimensionar)
// ---------------------------------------------------------------------------

pub const SHADOW_M: i32 = 22;

pub fn shadow_spec(active: bool) -> ShadowSpec {
    ShadowSpec {
        blur: if active { 20.0 } else { 14.0 },
        offset_y: if active { 5.0 } else { 3.0 },
        radius: 6.0,
        alpha: if active { 0.5 } else { 0.3 },
    }
}

/// Faixas [cima, baixo, esquerda, direita] e suas posições lógicas relativas
/// ao canto superior esquerdo da janela (incluindo a barra de título).
pub fn window_shadow(w: i32, h: i32, active: bool, scale: f32) -> Vec<(i32, i32, Canvas)> {
    let m = SHADOW_M;
    let s = shadow_spec(active);
    // A faixa de cima entra um pouco na janela: os cantos de cima são arredondados
    // e, sem sombra no pedaço recortado, o fundo aparecia ali e o canto parecia vivo.
    // (Fica atrás da barra de título, então só aparece nesses cantos.)
    let r = 7;
    // A de baixo também entra na janela: as de metal têm os cantos de baixo arredondados.
    let parts = [(-m, -m, w + 2 * m, m + r), (-m, h - r, w + 2 * m, m + r), (-m, r, m, h - 2 * r), (w, r, m, h - 2 * r)];
    parts
        .into_iter()
        .filter(|p| p.2 > 0 && p.3 > 0)
        .filter_map(|(x, y, pw, ph)| {
            Canvas::shadow(x as f32, y as f32, pw as f32, ph as f32, w as f32, h as f32, &s, scale).map(|c| (x, y, c))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Barra de menu
// ---------------------------------------------------------------------------

pub struct MenuBarLayout {
    /// Faixa horizontal do botão do logo.
    pub logo: (f32, f32),
    /// Faixa de cada título de menu (o primeiro é o nome do app).
    pub titles: Vec<(f32, f32)>,
    /// Faixa de cada extra (volume, bateria, monitor), à esquerda do relógio.
    pub extras: Vec<(f32, f32)>,
    /// Lupa da Busca Rápida (canto direito).
    pub search: (f32, f32),
}

pub const MENUBAR_PX: f32 = 14.0;

/// `titles[0]` é o nome do app (em negrito). `open`: título aberto (0 = logo, 1.. = títulos);
/// `open_extra`: extra aberto.
/// `search_open`: a Busca Rápida está aberta (lupa realçada).
#[allow(clippy::too_many_arguments)]
pub fn menubar(fonts: &Fonts, theme: &Theme, w: f32, titles: &[String], extras: &[Extra], clock: &str, open: Option<usize>, open_extra: Option<usize>, search_open: bool, scale: f32) -> Option<(Canvas, MenuBarLayout)> {
    let h = MENUBAR_H as f32;
    let mut c = Canvas::new(w, h, scale)?;
    let claro = theme.variant == Variant::Claro;
    if claro {
        // Degradê brilhante e liso: branco, um pouco mais escuro no meio, branco de novo.
        c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(255, 255, 255, 255)), (0.45, rgba(243, 243, 244, 255)), (0.9, rgba(255, 255, 255, 255))]));
        c.hline(0.0, h - 1.0 / scale, w, rgba(200, 200, 202, 255));
    } else {
        c.pinstripes(0.0, 0.0, w, h, theme.menu_bg_a, theme.menu_bg_b);
        c.hline(0.0, h - 1.0 / scale, w, rgba(0, 0, 0, 110));
    }
    let hot = |c: &mut Canvas, a: f32, b: f32| {
        if claro {
            c.fill_rect(a, 0.0, b - a, h - 1.0, &solid(theme.menu_hot));
        } else {
            c.fill_rect(a, 0.0, b - a, h - 1.0, &vgrad(0.0, h, &[(0.0, lighten(theme.accent, 0.15)), (1.0, darken(theme.accent, 0.15))]));
        }
    };

    let logo = (6.0, 34.0);
    if open == Some(0) {
        hot(&mut c, logo.0, logo.1);
    }
    icons::logo(&mut c, 13.0, 4.0, 14.0, if open == Some(0) { lighten(theme.accent, 0.5) } else { theme.accent });

    let mut x = 40.0;
    let mut ranges = Vec::with_capacity(titles.len());
    for (i, t) in titles.iter().enumerate() {
        let bold = i == 0;
        let tw = fonts.measure(t, MENUBAR_PX, bold);
        let (a, b) = (x, x + tw + 20.0);
        let is_open = open == Some(i + 1);
        if is_open {
            hot(&mut c, a, b);
        }
        fonts.draw(&mut c, t, a + 10.0, 16.0, MENUBAR_PX, bold, if is_open { Color::WHITE } else { theme.menu_text });
        ranges.push((a, b));
        x = b;
    }
    // Lupa da Busca Rápida no canto, num quadrado azul quando aberta (como no 10.4).
    let search = (w - 32.0, w);
    if search_open {
        c.fill_rect(search.0, 0.0, search.1 - search.0, h - 1.0, &vgrad(0.0, h, &[(0.0, lighten(theme.accent, 0.15)), (1.0, darken(theme.accent, 0.15))]));
    }
    magnifier_glyph(&mut c, search.0 + 9.0, 4.0, 14.0, if search_open { Color::WHITE } else { theme.menu_text });
    let right = search.0 - 4.0;
    // O relógio pode vir à parte (texto fixo) ou como o último extra (clicável).
    let extras_right = if clock.is_empty() {
        right
    } else {
        let cw = fonts.measure(clock, MENUBAR_PX, false);
        fonts.draw(&mut c, clock, right - cw - 8.0, 16.0, MENUBAR_PX, false, theme.menu_text);
        right - cw - 16.0
    };
    let extra_ranges = draw_extras(&mut c, fonts, theme, extras, extras_right, open_extra, &hot);
    Some((c, MenuBarLayout { logo, titles: ranges, extras: extra_ranges, search }))
}

// ---------------------------------------------------------------------------
// Menu suspenso
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub checked: bool,
    pub separator: bool,
    /// Atalho mostrado à direita (ex.: "Ctrl+S").
    pub shortcut: String,
    pub enabled: bool,
    /// Abre um submenu (mostra ▸ à direita).
    pub submenu: bool,
    /// ◆ na coluna da marca (janela minimizada no menu Janela).
    pub diamond: bool,
}

impl MenuItem {
    pub fn item(label: impl Into<String>) -> Self {
        Self { label: label.into(), checked: false, separator: false, shortcut: String::new(), enabled: true, submenu: false, diamond: false }
    }
    /// Item que abre um submenu.
    pub fn sub(label: impl Into<String>) -> Self {
        Self { submenu: true, ..Self::item(label) }
    }
    pub fn check(label: impl Into<String>, checked: bool) -> Self {
        Self { checked, ..Self::item(label) }
    }
    pub fn sep() -> Self {
        Self { separator: true, ..Self::item("") }
    }
    pub fn with_shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = s.into();
        self
    }
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Atalho em duas partes, como nos menus da era: modificadores em símbolos (na ordem
/// ⌃ ⌥ ⇧ ❖) e a tecla, que fica numa coluna própria. "Ctrl+Shift+S" → ("⌃⇧", "S").
fn shortcut_parts(s: &str) -> (String, String) {
    let parts: Vec<&str> = s.split('+').collect();
    let (mods, key) = parts.split_at(parts.len().saturating_sub(1));
    let mut m = String::new();
    for (name, glyph) in [("Ctrl", "⌃"), ("Alt", "⌥"), ("Shift", "⇧"), ("Super", "❖")] {
        if mods.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            m.push_str(glyph);
        }
    }
    let key = match key.first().copied().unwrap_or("") {
        "Return" => "↩",
        "BackSpace" => "⌫",
        "Delete" => "⌦",
        // A fonte não tem o ⎋.
        "Escape" => "Esc",
        "Tab" => "⇥",
        "Up" => "↑",
        "Down" => "↓",
        "Left" => "←",
        "Right" => "→",
        "Comma" | "comma" => ",",
        "period" => ".",
        "equal" => "=",
        "minus" => "-",
        "plus" => "+",
        "space" => "Espaço",
        other => other,
    };
    let key = if key.chars().count() == 1 { key.to_uppercase() } else { key.to_string() };
    (m, key)
}

pub const MENU_ITEM_H: f32 = 19.0;
/// Separador: um vão (no tema Listras sem linha, como no 10.2).
const SEP_H: f32 = 11.0;
const MENU_PAD: f32 = 4.0;
/// Margem de sombra em volta do menu (esquerda, direita e embaixo).
pub const MENU_SHADOW: i32 = 12;
const MENU_PX: f32 = 14.0;

/// Larguras das colunas de atalho (modificadores, tecla).
fn shortcut_columns(fonts: &Fonts, items: &[MenuItem]) -> (f32, f32) {
    items.iter().filter(|i| !i.shortcut.is_empty()).fold((0.0f32, 0.0f32), |(mw, kw), i| {
        let (m, k) = shortcut_parts(&i.shortcut);
        (mw.max(fonts.measure(&m, MENU_PX, false)), kw.max(fonts.measure(&k, MENU_PX, false)))
    })
}

pub fn menu_content_size(fonts: &Fonts, items: &[MenuItem]) -> (f32, f32) {
    let (mw, kw) = shortcut_columns(fonts, items);
    let mut shortcuts = if mw + kw > 0.0 { mw + kw + 30.0 } else { 0.0 };
    if items.iter().any(|i| i.submenu) {
        shortcuts = shortcuts.max(SUB_ARROW_W);
    }
    let w = items.iter().map(|i| fonts.measure(&i.label, MENU_PX, false)).fold(0.0, f32::max) + shortcuts + 44.0;
    let h = items.iter().map(|i| if i.separator { SEP_H } else { MENU_ITEM_H }).sum::<f32>() + MENU_PAD * 2.0;
    (w.max(180.0).ceil(), h.ceil())
}

/// Espaço à direita reservado para o ▸ dos submenus.
const SUB_ARROW_W: f32 = 24.0;

/// Topo do item `i` (relativo ao topo do conteúdo do menu).
pub fn menu_item_top(items: &[MenuItem], i: usize) -> f32 {
    MENU_PAD + items.iter().take(i).map(|it| if it.separator { SEP_H } else { MENU_ITEM_H }).sum::<f32>()
}

/// Item sob a coordenada `y` (relativa ao topo do conteúdo do menu).
pub fn menu_item_at(items: &[MenuItem], y: f32) -> Option<usize> {
    let mut cy = MENU_PAD;
    for (i, it) in items.iter().enumerate() {
        let h = if it.separator { SEP_H } else { MENU_ITEM_H };
        if y >= cy && y < cy + h {
            return (!it.separator).then_some(i);
        }
        cy += h;
    }
    None
}

/// Menu completo (com margem de sombra). O conteúdo começa em (MENU_SHADOW, 0).
pub fn menu(fonts: &Fonts, theme: &Theme, items: &[MenuItem], hover: Option<usize>, scale: f32) -> Option<Canvas> {
    let (w, h) = menu_content_size(fonts, items);
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;

    // Sombra forte para baixo e cantos retos, como nos menus da era.
    let s = ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    let claro = theme.variant == Variant::Claro;
    if claro {
        // 10.4: listras quase invisíveis.
        c.fill_rect(x0, 0.0, w, h, &solid(rgba(237, 240, 241, 250)));
        c.pinstripes(x0, 0.0, w, h, rgba(235, 237, 239, 250), rgba(237, 240, 241, 0));
    } else {
        c.fill_rect(x0, 0.0, w, h, &solid(rgba(246, 247, 249, 248)));
        c.pinstripes(x0, 0.0, w, h, rgba(246, 247, 249, 0), rgba(230, 232, 234, 235));
    }
    c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);

    let (mw, kw) = shortcut_columns(fonts, items);
    let key_x = x0 + w - 14.0 - kw;
    let mut cy = MENU_PAD;
    for (i, it) in items.iter().enumerate() {
        if it.separator {
            if theme.variant != Variant::Listras {
                let line = if claro { rgba(217, 218, 220, 255) } else { rgba(212, 212, 212, 255) };
                c.hline(x0 + 1.0, (cy + SEP_H / 2.0).floor(), w - 2.0, line);
            }
            cy += SEP_H;
            continue;
        }
        let hot = hover == Some(i) && it.enabled;
        if hot {
            if claro {
                c.fill_rect(x0, cy, w, MENU_ITEM_H, &solid(theme.menu_hot));
            } else {
                c.fill_rect(x0, cy, w, MENU_ITEM_H, &vgrad(cy, cy + MENU_ITEM_H, &[(0.0, lighten(theme.accent, 0.2)), (1.0, darken(theme.accent, 0.1))]));
            }
        }
        let color = if hot {
            Color::WHITE
        } else if it.enabled {
            theme.menu_text
        } else {
            rgba(160, 160, 160, 255)
        };
        let base = cy + 14.5;
        if it.checked {
            fonts.draw(&mut c, "✓", x0 + 7.0, base, MENU_PX, false, color);
        } else if it.diamond {
            fonts.draw(&mut c, "◆", x0 + 6.0, base, MENU_PX - 2.0, false, color);
        }
        fonts.draw(&mut c, &it.label, x0 + 22.0, base, MENU_PX, false, color);
        if !it.shortcut.is_empty() {
            let (m, k) = shortcut_parts(&it.shortcut);
            let msw = fonts.measure(&m, MENU_PX, false);
            fonts.draw(&mut c, &m, key_x - 2.0 - msw, base, MENU_PX, false, color);
            fonts.draw(&mut c, &k, key_x, base, MENU_PX, false, color);
        }
        if it.submenu {
            // Triângulo cheio apontando para a direita.
            let (tx, ty) = (x0 + w - 16.0, cy + MENU_ITEM_H / 2.0);
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(tx, ty - 4.5);
            pb.line_to(tx + 6.5, ty);
            pb.line_to(tx, ty + 4.5);
            pb.close();
            c.fill(pb.finish(), &solid(color));
        }
        cy += MENU_ITEM_H;
    }
    let _ = mw;
    Some(c)
}

// ---------------------------------------------------------------------------
// Dock
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct DockItem {
    pub icon: Icon,
    pub label: String,
    pub letter: Option<char>,
    pub running: bool,
    pub separator_before: bool,
    /// Altura do pulo agora (app abrindo), em pontos.
    pub bounce: f32,
    /// Janela minimizada: o compositor desenha a miniatura viva no lugar do ícone.
    pub thumbnail: bool,
}

pub const DOCK_BASE: f32 = 56.0;
pub const DOCK_MAX: f32 = 80.0;
const DOCK_GAP: f32 = 2.0;
const DOCK_PAD_X: f32 = 8.0;
const DOCK_PAD_Y: f32 = 4.0;
const DOCK_SEP: f32 = 14.0;
const DOCK_LABEL_H: f32 = 26.0;

/// Borda da tela em que o Dock fica.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DockSide {
    #[default]
    Bottom,
    Left,
    Right,
}

impl DockSide {
    pub fn from_config(s: &str) -> Self {
        match s {
            "esquerda" => Self::Left,
            "direita" => Self::Right,
            _ => Self::Bottom,
        }
    }
    pub fn vertical(self) -> bool {
        self != Self::Bottom
    }
}

/// Espaço ao lado dos ícones para o nome, com o Dock na lateral.
const DOCK_SIDE_LABEL_W: f32 = 180.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DockLayout {
    /// Tamanho lógico.
    pub width: u32,
    pub height: u32,
    /// (x, y, tamanho) de cada ícone, relativos ao Dock.
    pub items: Vec<(f32, f32, f32)>,
    /// Prateleira de vidro: (x, y, w, h).
    pub shelf: (f32, f32, f32, f32),
    pub side: DockSide,
}

impl DockLayout {
    /// Coordenada ao longo do Dock (x embaixo, y nas laterais).
    pub fn along(&self, x: f32, y: f32) -> f32 {
        if self.side.vertical() { y } else { x }
    }
}

/// Posição de cada ícone com o Dock em `side`. `hover` é a coordenada ao longo do Dock.
pub fn dock_layout_on(items: &[DockItem], hover: Option<f32>, zoom: f32, base: f32, max: f32, side: DockSide) -> DockLayout {
    if side == DockSide::Bottom {
        return dock_layout(items, hover, zoom, base, max);
    }
    // Mede como se estivesse embaixo (com espaço para o nome ao lado) e gira.
    let h = dock_layout_room(items, hover, zoom, base, max, DOCK_SIDE_LABEL_W);
    let thick = h.height as f32;
    let items = h
        .items
        .iter()
        .map(|&(x, y, s)| {
            // Distância da borda da tela até o ícone, igual à de baixo.
            let from_edge = thick - (y + s);
            let nx = if side == DockSide::Left { from_edge } else { thick - from_edge - s };
            (nx, x, s)
        })
        .collect();
    let (sx, _, sw, sh) = h.shelf;
    let shelf = if side == DockSide::Left { (0.0, sx, sh, sw) } else { (thick - sh, sx, sh, sw) };
    DockLayout { width: h.height, height: h.width, items, shelf, side }
}

/// Posição de cada ícone. `hover_x` é relativo ao Dock; `zoom` (0 a 1) é a força
/// da ampliação, animada ao entrar e sair do Dock para não pular de uma vez.
pub fn dock_layout(items: &[DockItem], hover_x: Option<f32>, zoom: f32, base: f32, max: f32) -> DockLayout {
    dock_layout_room(items, hover_x, zoom, base, max, DOCK_LABEL_H)
}

/// Comprimento do Dock (ao longo da borda) para esses ícones e tamanhos.
pub fn dock_length(items: &[DockItem], base: f32, max: f32) -> f32 {
    let n = items.len() as f32;
    let seps = items.iter().filter(|i| i.separator_before).count() as f32;
    n * (base + DOCK_GAP) - DOCK_GAP + seps * DOCK_SEP + (max - base).max(0.0) * 3.2 + 2.0 * DOCK_PAD_X + 4.0
}

/// Dock embaixo, com `label_room` de espaço acima dos ícones para o nome.
fn dock_layout_room(items: &[DockItem], hover_x: Option<f32>, zoom: f32, base: f32, max: f32, label_room: f32) -> DockLayout {
    let n = items.len() as f32;
    let seps = items.iter().filter(|i| i.separator_before).count() as f32;
    let base_w = n * (base + DOCK_GAP) - DOCK_GAP + seps * DOCK_SEP;
    let extra = (max - base).max(0.0) * 3.2;
    let width = (base_w + extra + 2.0 * DOCK_PAD_X).ceil() as u32 + 4;
    let height = (label_room + max.max(base + 6.0) + 2.0 * DOCK_PAD_Y + 6.0).ceil() as u32;
    let center = width as f32 / 2.0;

    let mut base_centers = Vec::with_capacity(items.len());
    let mut x = center - base_w / 2.0;
    for it in items {
        if it.separator_before {
            x += DOCK_SEP;
        }
        base_centers.push(x + base / 2.0);
        x += base + DOCK_GAP;
    }

    let range = base * 2.6;
    let sizes: Vec<f32> = base_centers
        .iter()
        .map(|c| match hover_x {
            Some(hx) if max > base => {
                let d = ((hx - c).abs() / range).min(1.0);
                base + (max - base) * zoom.clamp(0.0, 1.0) * 0.5 * (1.0 + (std::f32::consts::PI * d).cos())
            }
            _ => base,
        })
        .collect();
    let total: f32 = sizes.iter().sum::<f32>() + DOCK_GAP * (n - 1.0).max(0.0) + seps * DOCK_SEP;

    let bottom = height as f32 - DOCK_PAD_Y - 6.0;
    let mut out = Vec::with_capacity(items.len());
    let mut x = center - total / 2.0;
    for (it, s) in items.iter().zip(&sizes) {
        if it.separator_before {
            x += DOCK_SEP;
        }
        out.push((x, bottom - s, *s));
        x += s + DOCK_GAP;
    }
    // A prateleira envolve os ícones no tamanho normal; ampliados, eles saem por cima.
    // Folga em cima: o ícone não encosta na borda da prateleira (como no Tiger).
    let shelf_h = base + DOCK_PAD_Y + 11.0;
    let shelf = (center - total / 2.0 - DOCK_PAD_X, height as f32 - shelf_h, total + 2.0 * DOCK_PAD_X, shelf_h);
    DockLayout { width, height, items: out, shelf, side: DockSide::Bottom }
}

/// Item do Dock sob (x, y) relativos ao Dock.
pub fn dock_item_at(layout: &DockLayout, x: f32, y: f32) -> Option<usize> {
    if layout.side.vertical() {
        let (sx, _, sw, _) = layout.shelf;
        return layout.items.iter().position(|(ix, iy, s)| {
            let across = if layout.side == DockSide::Left { x >= 0.0 && x < (ix + s).max(sx + sw) } else { x >= ix.min(sx) && x < layout.width as f32 };
            across && y >= *iy - DOCK_GAP / 2.0 && y < iy + s + DOCK_GAP / 2.0
        });
    }
    let sy = layout.shelf.1;
    layout
        .items
        .iter()
        .position(|(ix, iy, s)| x >= *ix - DOCK_GAP / 2.0 && x < ix + s + DOCK_GAP / 2.0 && y >= iy.min(sy) && y < layout.height as f32)
}

/// O mouse está sobre o Dock (prateleira ou ícones)?
pub fn dock_contains(layout: &DockLayout, x: f32, y: f32) -> bool {
    if layout.side.vertical() {
        let (sx, sy, sw, sh) = layout.shelf;
        let (lo, hi) = if layout.side == DockSide::Left {
            (0.0, layout.items.iter().map(|i| i.0 + i.2).fold(sx + sw, f32::max))
        } else {
            (layout.items.iter().map(|i| i.0).fold(sx, f32::min), layout.width as f32)
        };
        return x >= lo && x < hi && y >= sy && y < sy + sh;
    }
    let (sx, sy, sw, _) = layout.shelf;
    let top = layout.items.iter().map(|i| i.1).fold(sy, f32::min);
    x >= sx && x < sx + sw && y >= top && y < layout.height as f32
}

/// Nome do app sobre o Dock, como na era: branco em negrito com contorno escuro
/// (cópias deslocadas em volta) e uma sombra leve, sem fundo.
fn outlined_label(fonts: &Fonts, c: &mut Canvas, text: &str, x: f32, y: f32, px: f32) {
    fonts.draw(c, text, x + 0.5, y + 1.6, px, true, rgba(0, 0, 0, 90));
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.8, -0.8), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8)] {
        fonts.draw(c, text, x + dx, y + dy, px, true, rgba(0, 0, 0, 150));
    }
    fonts.draw(c, text, x, y, px, true, Color::WHITE);
}

thread_local! {
    /// Sombras dos ícones do Dock já feitas: (ícone, letra, tamanho em pixels) → sombra.
    static ICON_SHADOWS: std::cell::RefCell<Vec<((String, u32), std::rc::Rc<Canvas>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Sombra com a forma do ícone: o contorno dele, desfocado e escuro (como os
/// ícones do Dock do Tiger). Devolve a sombra e a margem em volta do ícone.
fn icon_shadow(fonts: &Fonts, icon: icons::Icon, letter: Option<char>, size: f32, scale: f32) -> Option<(std::rc::Rc<Canvas>, f32)> {
    // Tamanhos de 2 em 2 pixels: a ampliação não refaz a sombra a cada quadro.
    let px = ((size * scale / 2.0).round() * 2.0) as u32;
    let key = (format!("{icon:?}{letter:?}"), px);
    if let Some(c) = ICON_SHADOWS.with(|m| m.borrow().iter().find(|(k, _)| *k == key).map(|(_, c)| c.clone())) {
        return Some((c, (size * 0.08).max(3.0)));
    }
    let pad = (size * 0.08).max(3.0);
    let side = px as f32 / scale;
    let mut c = Canvas::new(side + 2.0 * pad, side + 2.0 * pad, scale)?;
    icons::draw_bare(&mut c, fonts, icon, pad, pad, side, letter);
    let (w, h) = (c.pm.width() as usize, c.pm.height() as usize);
    // Só o alfa, desfocado com duas passadas de caixa em cada direção.
    let mut a: Vec<f32> = c.pm.data().chunks_exact(4).map(|p| p[3] as f32).collect();
    let r = ((size * 0.035 * scale).round() as usize).max(1);
    let mut tmp = vec![0f32; a.len()];
    for _ in 0..2 {
        for y in 0..h {
            for x in 0..w {
                let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
                tmp[y * w + x] = a[y * w + lo..=y * w + hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
        for x in 0..w {
            for y in 0..h {
                let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
                a[y * w + x] = (lo..=hi).map(|yy| tmp[yy * w + x]).sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
    }
    for (p, v) in c.pm.data_mut().chunks_exact_mut(4).zip(&a) {
        let al = (v * 0.45) as u8;
        p.copy_from_slice(&[0, 0, 0, al]);
    }
    let c = std::rc::Rc::new(c);
    ICON_SHADOWS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 96 {
            m.remove(0);
        }
        m.push((key, c.clone()));
    });
    Some((c, pad))
}

pub fn dock(fonts: &Fonts, theme: &Theme, items: &[DockItem], layout: &DockLayout, hover: Option<usize>, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(layout.width as f32, layout.height as f32, scale)?;
    let (sx, sy, sw, sh) = layout.shelf;

    // Prateleira de vidro fosco: retângulo translúcido com bordas claras.
    // Vidro branco translúcido com listras finas, encostado na borda de baixo.
    let side = layout.side;
    // Cantos arredondados só do lado de dentro (longe da borda da tela).
    let shelf = match side {
        DockSide::Bottom => rrect4(sx, sy, sw, sh + 2.0, [2.0, 2.0, 0.0, 0.0]),
        DockSide::Left => rrect4(sx - 2.0, sy, sw + 2.0, sh, [0.0, 2.0, 2.0, 0.0]),
        DockSide::Right => rrect4(sx, sy, sw + 2.0, sh, [2.0, 0.0, 0.0, 2.0]),
    };
    if theme.variant == Variant::Claro {
        // 10.4: vidro fosco cinza-lilás, quase opaco (o tom não depende do papel
        // de parede), liso e um pouco mais claro em cima.
        c.fill(shelf.clone(), &vgrad(sy, sy + sh, &[(0.0, rgba(170, 178, 216, 222)), (1.0, rgba(146, 156, 202, 228))]));
    } else {
        c.fill(shelf.clone(), &vgrad(sy, sy + sh, &[(0.0, rgba(255, 255, 255, 120)), (0.5, rgba(240, 244, 250, 100)), (1.0, rgba(225, 232, 242, 112))]));
        c.pinstripes(sx + 1.0, sy + 1.0, sw - 2.0, sh - 1.0, rgba(255, 255, 255, 0), rgba(255, 255, 255, 34));
    }
    // Borda clara fina e suave (no 10.4 é só um fio de luz em volta do vidro).
    let claro = theme.variant == Variant::Claro;
    c.stroke(shelf, &solid(rgba(255, 255, 255, if claro { 80 } else { 170 })), 1.0 / scale);
    match side {
        DockSide::Bottom => {
            c.hline(sx, sy - 1.0, sw, rgba(0, 0, 0, if claro { 35 } else { 50 }));
            if !claro {
                c.hline(sx + 2.0, sy + 1.0, sw - 4.0, rgba(255, 255, 255, 210));
            }
        }
        DockSide::Left => c.vline(sx + sw, sy, sh, rgba(0, 0, 0, 50)),
        DockSide::Right => c.vline(sx - 1.0, sy, sh, rgba(0, 0, 0, 50)),
    }

    for (i, (it, (x, y, s))) in items.iter().zip(&layout.items).enumerate() {
        if it.separator_before {
            if side.vertical() {
                let ly = y - DOCK_GAP / 2.0 - DOCK_SEP / 2.0;
                c.hline(sx, ly, sw, rgba(0, 0, 0, 70));
                c.hline(sx, ly + 1.0, sw, rgba(255, 255, 255, 190));
            } else {
                let lx = x - DOCK_GAP / 2.0 - DOCK_SEP / 2.0;
                c.vline(lx, sy, sh, rgba(0, 0, 0, 70));
                c.vline(lx + 1.0, sy, sh, rgba(255, 255, 255, 190));
            }
        }
        // App abrindo: o ícone pula para longe da borda (até o fim do Dock, no máximo).
        let (x, y) = match side {
            DockSide::Bottom => (x, &(y - it.bounce.clamp(0.0, (y - 2.0).max(0.0)))),
            DockSide::Left => (&(x + it.bounce.clamp(0.0, (layout.width as f32 - x - s - 2.0).max(0.0))), y),
            DockSide::Right => (&(x - it.bounce.clamp(0.0, (x - 2.0).max(0.0))), y),
        };
        if !it.thumbnail {
            // O desenho dos ícones tem margem dentro do quadro; no Dock ele cresce
            // para encher a vaga (como os ícones do Tiger, quase encostados).
            // Cresce para o lado da borda da tela (a margem de baixo do desenho fica
            // sobre o espaço do indicador), para o ícone caber na prateleira.
            let k = 1.14;
            let (dx, dy) = match side {
                DockSide::Bottom => (s * (k - 1.0) / 2.0, s * (k - 1.0) * 0.1),
                DockSide::Left => (s * (k - 1.0) * 0.7, s * (k - 1.0) / 2.0),
                DockSide::Right => (s * (k - 1.0) * 0.3, s * (k - 1.0) / 2.0),
            };
            let (ix, iy, is) = (*x - dx, *y - dy, *s * k);
            if let Some((sh, pad)) = icon_shadow(fonts, it.icon, it.letter, is, scale) {
                c.draw_canvas(&sh, ix - pad, iy - pad + is * 0.035);
            }
            icons::draw_bare(&mut c, fonts, it.icon, ix, iy, is, it.letter);
        }
        if it.running {
            // Indicador de app aberto: pequeno triângulo escuro junto à borda da tela.
            let mut pb = tiny_skia::PathBuilder::new();
            match side {
                DockSide::Bottom => {
                    let (cx, by) = (x + s / 2.0, layout.height as f32 - 1.5);
                    pb.move_to(cx - 4.0, by);
                    pb.line_to(cx + 4.0, by);
                    pb.line_to(cx, by - 4.5);
                }
                DockSide::Left => {
                    let cy = y + s / 2.0;
                    pb.move_to(1.5, cy - 4.0);
                    pb.line_to(1.5, cy + 4.0);
                    pb.line_to(6.0, cy);
                }
                DockSide::Right => {
                    let (cy, bx) = (y + s / 2.0, layout.width as f32 - 1.5);
                    pb.move_to(bx, cy - 4.0);
                    pb.line_to(bx, cy + 4.0);
                    pb.line_to(bx - 4.5, cy);
                }
            }
            pb.close();
            c.fill(pb.finish(), &solid(rgba(15, 20, 30, 220)));
        }
        if hover == Some(i) {
            let px = 14.0;
            let tw = fonts.measure(&it.label, px, true);
            let (lx, ly) = match side {
                DockSide::Bottom => ((x + s / 2.0 - tw / 2.0).clamp(10.0, layout.width as f32 - tw - 10.0), (y - 8.0).max(16.0)),
                // Nas laterais, o nome fica ao lado do ícone.
                DockSide::Left => (x + s + 10.0, y + s / 2.0 + 5.0),
                DockSide::Right => ((x - 10.0 - tw).max(4.0), y + s / 2.0 + 5.0),
            };
            outlined_label(fonts, &mut c, &it.label, lx, ly, px);
        }
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Ícones da mesa
// ---------------------------------------------------------------------------

pub const DESK_ICON_W: f32 = 96.0;
pub const DESK_ICON_H: f32 = 84.0;

/// Ícone na mesa: desenho + nome em branco com sombra (destacado se selecionado).
pub fn desktop_icon(fonts: &Fonts, theme: &Theme, icon: Icon, label: &str, selected: bool, scale: f32) -> Option<Canvas> {
    let (w, h) = (DESK_ICON_W, DESK_ICON_H);
    let mut c = Canvas::new(w, h, scale)?;
    let s = 52.0;
    if selected {
        c.fill(rrect((w - s) / 2.0 - 4.0, 0.0, s + 8.0, s + 4.0, 6.0), &solid(rgba(0, 0, 0, 70)));
    }
    icons::draw(&mut c, fonts, icon, (w - s) / 2.0, 2.0, s, None);
    let px = 12.0;
    let t = fonts.ellipsize(label, px, true, w - 6.0);
    let tw = fonts.measure(&t, px, true);
    let (x, y) = ((w - tw) / 2.0, s + 20.0);
    if selected {
        c.fill(rrect(x - 6.0, y - 13.0, tw + 12.0, 17.0, 8.5), &solid(theme.accent));
    } else {
        fonts.draw(&mut c, &t, x + 1.0, y + 1.0, px, true, rgba(0, 0, 0, 180));
    }
    fonts.draw(&mut c, &t, x, y, px, true, Color::WHITE);
    Some(c)
}

/// Ponta do cursor (ponto quente), em pontos lógicos.
pub const CURSOR_HOTSPOT: (f32, f32) = (4.0, 3.0);

/// Cursor de seta da era: preto, contorno branco e sombra suave. Desenhado aqui
/// (a sessão própria não tem o cursor do desktop por baixo).
pub fn cursor_arrow(scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(24.0, 28.0, scale)?;
    let (ox, oy) = CURSOR_HOTSPOT;
    let pts: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 16.5), (4.0, 12.8), (6.8, 19.2), (9.6, 18.0), (6.9, 11.8), (12.2, 11.8)];
    let arrow = |dx: f32, dy: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(ox + dx + pts[0].0, oy + dy + pts[0].1);
        for p in &pts[1..] {
            pb.line_to(ox + dx + p.0, oy + dy + p.1);
        }
        pb.close();
        pb.finish()
    };
    // Sombra deslocada para baixo e para a direita.
    c.fill(arrow(1.0, 1.6), &solid(rgba(0, 0, 0, 60)));
    c.stroke(arrow(0.0, 0.0), &solid(rgba(255, 255, 255, 255)), 2.2);
    c.fill(arrow(0.0, 0.0), &solid(rgba(0, 0, 0, 255)));
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atalhos_em_duas_partes() {
        assert_eq!(shortcut_parts("Ctrl+Shift+S"), ("⌃⇧".into(), "S".into()));
        assert_eq!(shortcut_parts("Ctrl+o"), ("⌃".into(), "O".into()));
        assert_eq!(shortcut_parts("Super+Return"), ("❖".into(), "↩".into()));
        assert_eq!(shortcut_parts("F5"), ("".into(), "F5".into()));
    }
}

// ---------------------------------------------------------------------------
// Tela de encerramento (Desligando…, Reiniciando…, Encerrando sessão…)
// ---------------------------------------------------------------------------

/// Tamanho do giratório da tela de encerramento.
pub const ENDING_SPINNER: f32 = 32.0;

/// Centro do giratório na tela de encerramento de altura `h`.
pub fn ending_spinner_center(w: f32, h: f32) -> (f32, f32) {
    (w / 2.0, h / 2.0 + 70.0)
}

/// Fundo cinza-claro de inicialização, com o orbe e o texto (sem o giratório,
/// que é uma peça separada para não redesenhar a tela inteira a cada passo).
pub fn ending_screen(fonts: &Fonts, theme: &Theme, w: f32, h: f32, label: &str, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(214, 216, 219, 255)), (1.0, rgba(188, 191, 196, 255))]));
    let (cx, cy) = (w / 2.0, h / 2.0 - 30.0);
    gel_circle(&mut c, cx, cy, 44.0, theme.accent);
    let px = 15.0;
    let tw = fonts.measure(label, px, false);
    let (_, sy) = ending_spinner_center(w, h);
    fonts.draw(&mut c, label, (w - tw) / 2.0, sy + ENDING_SPINNER / 2.0 + 30.0, px, false, rgba(60, 62, 66, 255));
    Some(c)
}

/// Giratório da tela de encerramento (`phase` de 0 a 1 = uma volta), sobre o fundo dela.
pub fn ending_spinner(phase: f32, scale: f32) -> Option<Canvas> {
    let s = ENDING_SPINNER;
    let mut c = Canvas::new(s, s, scale)?;
    spinner(&mut c, s / 2.0, s / 2.0, s / 2.0, phase, rgba(70, 72, 76, 255));
    Some(c)
}

// ---------------------------------------------------------------------------
// Forçar Encerrar (janela desenhada pelo compositor)
// ---------------------------------------------------------------------------

/// Um app na lista do Forçar Encerrar.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceQuitRow {
    pub icon: Icon,
    pub name: String,
    /// Não respondeu ao último ping.
    pub hung: bool,
}

/// O que está embaixo do mouse no painel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FqHit {
    Title(Option<TitleButton>),
    Row(usize),
    /// Botão da direita (Forçar Encerrar).
    Confirm,
    /// Botão "Cancelar" (só na pergunta de confirmação).
    Cancel,
    Body,
}

pub const FQ_W: f32 = 380.0;
/// Margem de sombra em volta do painel.
pub const FQ_SHADOW: f32 = 24.0;
const FQ_PAD: f32 = 18.0;
const FQ_ROW_H: f32 = 22.0;
const FQ_ROWS_MIN: usize = 6;
const FQ_ROWS_MAX: usize = 10;
const FQ_TEXT_PX: f32 = 11.0;
const FQ_BTN_H: f32 = 22.0;
const FQ_INFO: &str = "Se um aplicativo não responder por algum tempo, selecione o nome dele e clique em Forçar Encerrar.";

fn fq_wrap(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let try_line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && fonts.measure(&try_line, px, bold) > max {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = try_line;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Partes do painel, em coordenadas do conteúdo (0,0 = canto da barra de título).
struct FqLayout {
    h: f32,
    text_y: f32,
    list: (f32, f32, f32, f32),
    confirm: (f32, f32, f32, f32),
    cancel: Option<(f32, f32, f32, f32)>,
}

fn fq_layout(fonts: &Fonts, rows: usize, question: Option<&str>) -> FqLayout {
    let top = TITLE_H as f32;
    let text_lines = match question {
        Some(q) => fq_wrap(fonts, q, 13.0, true, FQ_W - 2.0 * FQ_PAD).len() + 1,
        None => fq_wrap(fonts, FQ_INFO, FQ_TEXT_PX, false, FQ_W - 2.0 * FQ_PAD).len(),
    };
    let text_y = top + 14.0;
    let text_h = text_lines as f32 * 16.0;
    let list_y = text_y + text_h + 8.0;
    let list_h = rows.clamp(FQ_ROWS_MIN, FQ_ROWS_MAX) as f32 * FQ_ROW_H + 2.0;
    let btn_y = list_y + list_h + 16.0;
    let label_w = |t: &str| (fonts.measure(t, 13.0, false) + 36.0).max(90.0).ceil();
    let cw = label_w("Forçar Encerrar");
    let confirm = (FQ_W - FQ_PAD - cw, btn_y, cw, FQ_BTN_H);
    let cancel = question.map(|_| {
        let w = label_w("Cancelar");
        (confirm.0 - 12.0 - w, btn_y, w, FQ_BTN_H)
    });
    FqLayout { h: btn_y + FQ_BTN_H + FQ_PAD, text_y, list: (FQ_PAD, list_y, FQ_W - 2.0 * FQ_PAD, list_h), confirm, cancel }
}

/// Tamanho do conteúdo do painel (sem a sombra).
pub fn force_quit_size(fonts: &Fonts, rows: usize, question: Option<&str>) -> (f32, f32) {
    (FQ_W, fq_layout(fonts, rows, question).h.ceil())
}

fn inside(r: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

/// O que há em (x, y), relativo ao conteúdo do painel.
pub fn force_quit_hit(fonts: &Fonts, rows: usize, question: Option<&str>, x: f32, y: f32) -> FqHit {
    if y < TITLE_H as f32 {
        return FqHit::Title(title_button_at(x, y));
    }
    let l = fq_layout(fonts, rows, question);
    if inside(l.confirm, x, y) {
        return FqHit::Confirm;
    }
    if l.cancel.is_some_and(|c| inside(c, x, y)) {
        return FqHit::Cancel;
    }
    if question.is_none() && inside(l.list, x, y) {
        let i = ((y - l.list.1 - 1.0) / FQ_ROW_H).floor();
        if i >= 0.0 && (i as usize) < rows.min(FQ_ROWS_MAX) {
            return FqHit::Row(i as usize);
        }
    }
    FqHit::Body
}

/// Painel completo, com margem de sombra (o conteúdo começa em FQ_SHADOW, FQ_SHADOW).
/// `question`: pergunta de confirmação no lugar do texto ("Deseja forçar…?").
#[allow(clippy::too_many_arguments)]
pub fn force_quit_panel(fonts: &Fonts, theme: &Theme, rows: &[ForceQuitRow], selected: Option<usize>, question: Option<&str>, pressed: Option<FqHit>, title_hover: bool, scale: f32) -> Option<Canvas> {
    let l = fq_layout(fonts, rows.len(), question);
    let (w, h) = (FQ_W, l.h.ceil());
    let m = FQ_SHADOW;
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &shadow_spec(true), scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    c.fill_rect(x0, y0, w, h, &solid(theme.body_b));
    c.pinstripes(x0, y0, w, h, theme.body_a, theme.body_b);
    let st = TitleBarState { active: true, hover: title_hover, pressed: match pressed { Some(FqHit::Title(b)) => b, _ => None }, zoom_enabled: false };
    if let Some(t) = titlebar(fonts, theme, w, "Forçar Encerrar Aplicativos", &st, scale) {
        c.draw_canvas(&t, x0, y0);
    }
    let ink = rgba(20, 20, 20, 255);

    // Texto (ou a pergunta)
    let mut ty = y0 + l.text_y + 11.0;
    match question {
        Some(q) => {
            for line in fq_wrap(fonts, q, 13.0, true, w - 2.0 * FQ_PAD) {
                fonts.draw(&mut c, &line, x0 + FQ_PAD, ty, 13.0, true, ink);
                ty += 16.0;
            }
            fonts.draw(&mut c, "Alterações não salvas serão perdidas.", x0 + FQ_PAD, ty, FQ_TEXT_PX, false, rgba(40, 40, 40, 255));
        }
        None => {
            for line in fq_wrap(fonts, FQ_INFO, FQ_TEXT_PX, false, w - 2.0 * FQ_PAD) {
                fonts.draw(&mut c, &line, x0 + FQ_PAD, ty, FQ_TEXT_PX, false, ink);
                ty += 16.0;
            }
        }
    }

    // Lista
    let (lx, ly, lw, lh) = l.list;
    let (lx, ly) = (x0 + lx, y0 + ly);
    c.fill_rect(lx, ly, lw, lh, &solid(Color::WHITE));
    c.stroke(rrect(lx + 0.5, ly + 0.5, lw - 1.0, lh - 1.0, 0.0), &solid(rgba(140, 140, 140, 255)), 1.0);
    for (i, r) in rows.iter().take(FQ_ROWS_MAX).enumerate() {
        let ry = ly + 1.0 + i as f32 * FQ_ROW_H;
        let sel = selected == Some(i);
        if sel {
            let col = if question.is_some() { rgba(200, 200, 200, 255) } else { theme.menu_hot };
            c.fill_rect(lx + 1.0, ry, lw - 2.0, FQ_ROW_H, &solid(col));
        }
        icons::draw_bare(&mut c, fonts, r.icon, lx + 6.0, ry + 3.0, 16.0, None);
        let text_c = if sel && question.is_none() { Color::WHITE } else { ink };
        fonts.draw(&mut c, &r.name, lx + 28.0, ry + 15.5, 13.0, false, text_c);
        if r.hung {
            let nw = fonts.measure(&r.name, 13.0, false);
            let red = if sel && question.is_none() { rgba(255, 210, 210, 255) } else { rgba(200, 30, 30, 255) };
            fonts.draw(&mut c, "(não responde)", lx + 28.0 + nw + 6.0, ry + 15.5, 13.0, false, red);
        }
    }

    // Botões
    let button = |c: &mut Canvas, r: (f32, f32, f32, f32), label: &str, default: bool, down: bool| {
        let base = if default { theme.accent } else { rgba(238, 238, 240, 255) };
        let base = if down { darken(base, 0.2) } else { base };
        gel_rect(c, x0 + r.0, y0 + r.1, r.2, r.3, r.3 / 2.0, base);
        let tw = fonts.measure(label, 13.0, false);
        fonts.draw(c, label, x0 + r.0 + (r.2 - tw) / 2.0, y0 + r.1 + 15.5, 13.0, false, ink);
    };
    let can_quit = selected.is_some_and(|s| s < rows.len());
    if let Some(cr) = l.cancel {
        button(&mut c, cr, "Cancelar", false, pressed == Some(FqHit::Cancel));
    }
    if can_quit {
        button(&mut c, l.confirm, "Forçar Encerrar", true, pressed == Some(FqHit::Confirm));
    } else {
        let r = l.confirm;
        gel_rect(&mut c, x0 + r.0, y0 + r.1, r.2, r.3, r.3 / 2.0, rgba(238, 238, 240, 160));
        let tw = fonts.measure("Forçar Encerrar", 13.0, false);
        fonts.draw(&mut c, "Forçar Encerrar", x0 + r.0 + (r.2 - tw) / 2.0, y0 + r.1 + 15.5, 13.0, false, rgba(150, 150, 150, 255));
    }
    Some(c)
}

/// Cursor de espera (app sem responder): disco claro com o giratório. `step` de 0 a 11.
pub const WAIT_CURSOR_SIZE: f32 = 24.0;
pub fn cursor_wait(step: i32, scale: f32) -> Option<Canvas> {
    let s = WAIT_CURSOR_SIZE;
    let mut c = Canvas::new(s, s, scale)?;
    let r = s / 2.0 - 1.5;
    c.fill(circle(s / 2.0, s / 2.0, r), &solid(rgba(250, 250, 250, 240)));
    c.stroke(circle(s / 2.0, s / 2.0, r), &solid(rgba(0, 0, 0, 150)), 1.0);
    spinner(&mut c, s / 2.0, s / 2.0, r - 2.0, step as f32 / 12.0, rgba(30, 60, 140, 255));
    Some(c)
}

// ---------------------------------------------------------------------------
// Extras da barra de menu (volume, bateria, monitor), controle de volume e
// aviso na tela (OSD) das teclas de volume e brilho
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    /// Volume de 0 a 1.
    Volume { level: f32, muted: bool },
    Battery { percent: u8, charging: bool, show_percent: bool },
    Display,
    /// Wi-Fi: ligado e quantas ondas acesas (0 = sem rede).
    Wifi { on: bool, bars: u8 },
    /// Menu de Entrada: a sigla do layout ("BR").
    Input { badge: String },
    /// Relógio: o texto, ou (hora, minuto) no mostrador de ponteiros.
    Clock { text: String, analog: Option<(u8, u8)> },
    /// Troca rápida de usuário (o último à direita): o nome de quem está usando.
    User { name: String },
}

/// Largura do extra na barra.
fn extra_width(fonts: &Fonts, e: &Extra) -> f32 {
    match e {
        Extra::Volume { .. } => 30.0,
        Extra::Battery { percent, show_percent: true, .. } => 36.0 + fonts.measure(&format!("{percent}%"), MENUBAR_PX - 1.0, false),
        Extra::Battery { .. } => 36.0,
        Extra::Display => 30.0,
        Extra::Wifi { .. } => 30.0,
        Extra::Input { badge } => fonts.measure(badge, 10.0, true) + 22.0,
        Extra::Clock { analog: Some(_), .. } => 28.0,
        Extra::Clock { text, .. } => fonts.measure(text, MENUBAR_PX, false) + 16.0,
        Extra::User { name } => fonts.measure(name, MENUBAR_PX, true) + 16.0,
    }
}

/// Alto-falante com 0 a 3 ondas (sem som = sem ondas, como na era).
pub fn speaker_glyph(c: &mut Canvas, x: f32, y: f32, s: f32, waves: u8, color: Color) {
    // Desenhado numa caixa de s × s.
    let k = s / 16.0;
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x + 1.0 * k, y + 5.5 * k);
    pb.line_to(x + 4.0 * k, y + 5.5 * k);
    pb.line_to(x + 8.0 * k, y + 2.0 * k);
    pb.line_to(x + 8.0 * k, y + 14.0 * k);
    pb.line_to(x + 4.0 * k, y + 10.5 * k);
    pb.line_to(x + 1.0 * k, y + 10.5 * k);
    pb.close();
    c.fill(pb.finish(), &solid(color));
    for i in 0..waves.min(3) {
        let r = (3.0 + 2.6 * i as f32) * k;
        let (cx, cy) = (x + 8.5 * k, y + 8.0 * k);
        let mut pb = tiny_skia::PathBuilder::new();
        let a = 0.85f32;
        pb.move_to(cx + r * a.cos(), cy - r * a.sin());
        pb.quad_to(cx + r * 1.25, cy, cx + r * a.cos(), cy + r * a.sin());
        c.stroke_round(pb.finish(), &solid(color), 1.3 * k);
    }
}

/// Sol (brilho): disco com raios.
pub fn sun_glyph(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Color) {
    c.fill(circle(cx, cy, r * 0.45), &solid(color));
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let (s, co) = a.sin_cos();
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx + co * r * 0.65, cy + s * r * 0.65);
        pb.line_to(cx + co * r, cy + s * r);
        c.stroke_round(pb.finish(), &solid(color), r * 0.14);
    }
}

fn battery_glyph(c: &mut Canvas, x: f32, y: f32, percent: u8, charging: bool, color: Color) {
    let (w, h) = (22.0, 10.0);
    c.stroke(rrect(x + 0.5, y + 0.5, w, h, 2.0), &solid(color), 1.0);
    c.fill_rect(x + w + 0.5, y + 3.0, 2.0, 5.0, &solid(color));
    let fill = (w - 3.0) * (percent.min(100) as f32 / 100.0);
    let fc = if percent <= 10 && !charging { rgba(210, 40, 30, 255) } else { color };
    if fill > 0.0 {
        c.fill_rect(x + 2.0, y + 2.0, fill, h - 3.0, &solid(fc));
    }
    if charging {
        // Raio por cima, branco com contorno escuro.
        let (bx, by) = (x + w / 2.0, y + h / 2.0 + 0.5);
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(bx + 1.5, by - 6.5);
        pb.line_to(bx - 3.5, by + 0.8);
        pb.line_to(bx - 0.2, by + 0.8);
        pb.line_to(bx - 1.5, by + 6.5);
        pb.line_to(bx + 3.5, by - 0.8);
        pb.line_to(bx + 0.2, by - 0.8);
        pb.close();
        let p = pb.finish();
        c.stroke(p.clone(), &solid(rgba(40, 40, 40, 255)), 1.2);
        c.fill(p, &solid(Color::WHITE));
    }
}

/// Mostradorzinho de ponteiros (relógio analógico da barra).
fn clock_glyph(c: &mut Canvas, cx: f32, cy: f32, h: u8, m: u8, color: Color) {
    c.stroke(circle(cx, cy, 7.5), &solid(color), 1.2);
    for k in 0..12 {
        let a = k as f32 * std::f32::consts::TAU / 12.0;
        if k % 3 == 0 {
            c.fill(circle(cx + 5.8 * a.sin(), cy - 5.8 * a.cos(), 0.7), &solid(color));
        }
    }
    let hand = |c: &mut Canvas, a: f32, len: f32, w: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx, cy);
        pb.line_to(cx + len * a.sin(), cy - len * a.cos());
        c.stroke_round(pb.finish(), &solid(color), w);
    };
    let ma = m as f32 / 60.0 * std::f32::consts::TAU;
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * std::f32::consts::TAU;
    hand(c, ha, 3.8, 1.6);
    hand(c, ma, 5.6, 1.1);
}

/// Leque do Wi-Fi: três arcos sobre um ponto; os apagados ficam claros.
/// Desligado: só o contorno.
fn wifi_glyph(c: &mut Canvas, cx: f32, base: f32, on: bool, bars: u8, color: Color) {
    let dim = with_alpha(color, color.alpha() * 0.28);
    for k in 0..3u8 {
        let r = 4.5 + k as f32 * 4.0;
        let mut pb = tiny_skia::PathBuilder::new();
        let a0 = std::f32::consts::PI * 1.25;
        let steps = 12;
        for i in 0..=steps {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
            let (x, y) = (cx + r * a.cos(), base + r * a.sin());
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        let lit = on && k < bars;
        c.stroke(pb.finish(), &solid(if lit { color } else { dim }), if on { 2.0 } else { 1.0 });
    }
    c.fill(circle(cx, base - 1.0, 1.6), &solid(if on && bars > 0 { color } else { dim }));
}

fn display_glyph(c: &mut Canvas, x: f32, y: f32, color: Color) {
    c.stroke(rrect(x + 0.5, y + 0.5, 16.0, 11.0, 1.5), &solid(color), 1.2);
    c.fill_rect(x + 2.5, y + 2.5, 12.0, 7.0, &solid(with_alpha(color, color.alpha() * 0.35)));
    c.fill_rect(x + 7.0, y + 12.0, 3.0, 2.0, &solid(color));
    c.fill_rect(x + 4.5, y + 14.0, 8.0, 1.3, &solid(color));
}

/// Desenha os extras da direita para a esquerda, terminando em `right`. Devolve as faixas.
fn draw_extras(c: &mut Canvas, fonts: &Fonts, theme: &Theme, extras: &[Extra], right: f32, open: Option<usize>, hot: &dyn Fn(&mut Canvas, f32, f32)) -> Vec<(f32, f32)> {
    let mut ranges = vec![(0.0, 0.0); extras.len()];
    let mut x = right;
    for (i, e) in extras.iter().enumerate().rev() {
        let w = extra_width(fonts, e);
        let (a, b) = (x - w, x);
        let is_open = open == Some(i);
        if is_open {
            hot(c, a, b);
        }
        let col = if is_open { Color::WHITE } else { theme.menu_text };
        match e {
            Extra::Volume { level, muted } => {
                let waves = if *muted || *level <= 0.0 { 0 } else { (1.0 + level * 2.99) as u8 };
                speaker_glyph(c, a + 7.0, 3.0, 16.0, waves, col);
            }
            Extra::Battery { percent, charging, show_percent } => {
                let mut bx = a + 6.0;
                if *show_percent {
                    let t = format!("{percent}%");
                    fonts.draw(c, &t, bx, 16.0, MENUBAR_PX - 1.0, false, col);
                    bx += fonts.measure(&t, MENUBAR_PX - 1.0, false) + 5.0;
                }
                battery_glyph(c, bx, 6.0, *percent, *charging, col);
            }
            Extra::Display => display_glyph(c, a + 7.0, 3.0, col),
            Extra::Wifi { on, bars } => wifi_glyph(c, a + 15.0, 16.0, *on, *bars, col),
            Extra::Input { badge } => {
                // Sigla numa plaquinha (como a bandeirinha do 10.4, sem bandeira).
                let tw = fonts.measure(badge, 10.0, true);
                c.fill(rrect(a + 7.0, 4.0, tw + 8.0, 14.0, 3.0), &solid(col));
                let ink = if is_open { theme.accent } else { Color::WHITE };
                fonts.draw(c, badge, a + 11.0, 15.0, 10.0, true, ink);
            }
            Extra::Clock { analog: Some((h, m)), .. } => clock_glyph(c, a + 14.0, 11.0, *h, *m, col),
            Extra::Clock { text, .. } => fonts.draw(c, text, a + 8.0, 16.0, MENUBAR_PX, false, col),
            Extra::User { name } => fonts.draw(c, name, a + 8.0, 16.0, MENUBAR_PX, true, col),
        }
        ranges[i] = (a, b);
        x = a;
    }
    ranges
}

/// Controle de volume que desce do ícone: trilho vertical em gel.
pub const VOLUME_SLIDER_W: f32 = 30.0;
pub const VOLUME_SLIDER_H: f32 = 132.0;
const VS_TOP: f32 = 14.0;
const VS_BOTTOM: f32 = 118.0;

/// Nível (0 a 1) no ponto `y` do conteúdo do controle.
pub fn volume_slider_level(y: f32) -> f32 {
    ((VS_BOTTOM - y) / (VS_BOTTOM - VS_TOP)).clamp(0.0, 1.0)
}

/// Controle completo, com margem de sombra igual à dos menus (conteúdo em MENU_SHADOW, 0).
pub fn volume_slider(theme: &Theme, level: f32, muted: bool, scale: f32) -> Option<Canvas> {
    let (w, h) = (VOLUME_SLIDER_W, VOLUME_SLIDER_H);
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;
    let s = ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    c.fill_rect(x0, 0.0, w, h, &solid(rgba(246, 247, 249, 248)));
    c.pinstripes(x0, 0.0, w, h, rgba(246, 247, 249, 0), rgba(232, 234, 236, 235));
    c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    let cx = x0 + w / 2.0;
    // Trilho: cinza em cima, cor de destaque embaixo (até o polegar).
    let tw = 7.0;
    let ky = VS_BOTTOM - level.clamp(0.0, 1.0) * (VS_BOTTOM - VS_TOP);
    gel_rect(&mut c, cx - tw / 2.0, VS_TOP, tw, VS_BOTTOM - VS_TOP, tw / 2.0, rgba(200, 202, 206, 255));
    if !muted {
        gel_rect(&mut c, cx - tw / 2.0, ky, tw, VS_BOTTOM - ky, tw / 2.0, theme.accent);
    }
    gel_circle(&mut c, cx, ky, 7.5, if muted { rgba(225, 225, 228, 255) } else { lighten(theme.accent, 0.1) });
    Some(c)
}

/// Aviso na tela das teclas de volume e brilho: quadrado translúcido com o
/// símbolo e 16 quadradinhos de nível.
pub const OSD_SIZE: f32 = 170.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OsdKind {
    Volume,
    Brightness,
}

pub fn osd(kind: OsdKind, level: f32, muted: bool, alpha: f32, scale: f32) -> Option<Canvas> {
    let s = OSD_SIZE;
    let mut c = Canvas::new(s, s, scale)?;
    let a = |v: u8| ((v as f32) * alpha.clamp(0.0, 1.0)) as u8;
    c.fill(rrect(0.0, 0.0, s, s, 22.0), &solid(rgba(30, 32, 36, a(120))));
    let white = rgba(255, 255, 255, a(235));
    match kind {
        OsdKind::Volume => {
            let waves = if muted || level <= 0.0 { 0 } else { 3 };
            speaker_glyph(&mut c, s / 2.0 - 46.0, 26.0, 88.0, waves, white);
            if muted {
                // Traço de "sem som" sobre as ondas ausentes.
                let mut pb = tiny_skia::PathBuilder::new();
                pb.move_to(s / 2.0 + 8.0, 52.0);
                pb.line_to(s / 2.0 + 38.0, 88.0);
                pb.move_to(s / 2.0 + 38.0, 52.0);
                pb.line_to(s / 2.0 + 8.0, 88.0);
                c.stroke_round(pb.finish(), &solid(white), 6.0);
            }
        }
        OsdKind::Brightness => sun_glyph(&mut c, s / 2.0, 70.0, 42.0, white),
    }
    let n = 16;
    let (bx, by, bw, bh, gap) = (20.0, s - 34.0, (s - 40.0 - 15.0 * 2.0) / 16.0, 9.0, 2.0);
    let lit = if muted { 0 } else { (level.clamp(0.0, 1.0) * n as f32).round() as i32 };
    for i in 0..n {
        let x = bx + i as f32 * (bw + gap);
        let col = if i < lit { white } else { rgba(255, 255, 255, a(55)) };
        c.fill_rect(x, by, bw, bh, &solid(col));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Tela bloqueada
// ---------------------------------------------------------------------------

pub const LOCK_W: f32 = 380.0;
pub const LOCK_H: f32 = 196.0;
/// Margem de sombra em volta do painel de desbloqueio.
pub const LOCK_SHADOW: f32 = 24.0;
const LOCK_FIELD: (f32, f32, f32, f32) = (112.0, 104.0, 248.0, 24.0);
const LOCK_BUTTON_W: f32 = 116.0;

/// Retângulo do botão Desbloquear (relativo ao conteúdo do painel).
pub fn lock_button_rect() -> (f32, f32, f32, f32) {
    (LOCK_W - 20.0 - LOCK_BUTTON_W, LOCK_H - 20.0 - 22.0, LOCK_BUTTON_W, 22.0)
}

/// Retrato genérico de usuário: cabeça e ombros em gel sobre um fundo claro.
pub fn user_picture(c: &mut Canvas, x: f32, y: f32, s: f32, accent: Color) {
    c.fill(rrect(x, y, s, s, s * 0.12), &vgrad(y, y + s, &[(0.0, rgba(236, 240, 246, 255)), (1.0, rgba(200, 210, 226, 255))]));
    c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, s * 0.12), &solid(rgba(0, 0, 0, 90)), 1.0);
    let body = darken(accent, 0.1);
    // Ombros: meia elipse cortada pela borda de baixo.
    let sh = s * 0.36;
    c.fill(rrect4(x + s * 0.16, y + s - sh - 1.0, s * 0.68, sh, [s * 0.3, s * 0.3, 0.0, 0.0]), &vgrad(y + s - sh, y + s, &[(0.0, lighten(body, 0.35)), (1.0, body)]));
    gel_circle(c, x + s / 2.0, y + s * 0.4, s * 0.2, body);
}

/// Painel de desbloqueio, com sombra. `dots`: quantos caracteres digitados;
/// `message`: aviso em vermelho ("Senha incorreta.") ou None; `busy`: verificando.
#[allow(clippy::too_many_arguments)]
pub fn lock_panel(fonts: &Fonts, theme: &Theme, name: &str, dots: usize, message: Option<&str>, busy: bool, caret: bool, pressed: bool, spin: f32, scale: f32) -> Option<Canvas> {
    let (w, h, m) = (LOCK_W, LOCK_H, LOCK_SHADOW);
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &ShadowSpec { blur: 16.0, offset_y: 6.0, radius: 8.0, alpha: 0.5 }, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    let shape = || rrect(x0, y0, w, h, 8.0);
    c.fill(shape(), &solid(theme.body_b));
    // Listras longe dos cantos arredondados.
    c.pinstripes(x0, y0 + 8.0, w, h - 16.0, theme.body_a, theme.body_b);
    c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0 / scale);

    user_picture(&mut c, x0 + 20.0, y0 + 22.0, 76.0, theme.accent);
    let ink = rgba(20, 20, 20, 255);
    fonts.draw(&mut c, name, x0 + 112.0, y0 + 40.0, 15.0, true, ink);
    fonts.draw(&mut c, "Esta tela está bloqueada.", x0 + 112.0, y0 + 62.0, 11.0, false, rgba(50, 50, 50, 255));
    fonts.draw(&mut c, "Digite sua senha para desbloquear.", x0 + 112.0, y0 + 78.0, 11.0, false, rgba(50, 50, 50, 255));

    // Campo de senha
    let (fx, fy, fw, fh) = LOCK_FIELD;
    let (fx, fy) = (x0 + fx, y0 + fy);
    c.fill(rrect(fx, fy, fw, fh, 3.0), &solid(Color::WHITE));
    c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, 3.0), &solid(rgba(120, 120, 120, 255)), 1.0);
    c.hline(fx + 1.0, fy + 1.5, fw - 2.0, rgba(0, 0, 0, 40));
    let mut dx = fx + 10.0;
    for _ in 0..dots.min(28) {
        c.fill(circle(dx + 3.0, fy + fh / 2.0, 3.2), &solid(ink));
        dx += 8.5;
    }
    if caret && !busy {
        c.fill_rect(dx + 1.0, fy + 5.0, 1.0, fh - 10.0, &solid(ink));
    }
    if busy {
        spinner(&mut c, fx + fw - 13.0, fy + fh / 2.0, 7.5, spin, rgba(70, 70, 70, 255));
    }

    if let Some(msg) = message {
        fonts.draw(&mut c, msg, x0 + 112.0, y0 + 146.0, 11.0, true, rgba(200, 30, 30, 255));
    }

    let (bx, by, bw, bh) = lock_button_rect();
    let base = if busy { rgba(225, 225, 228, 255) } else { theme.accent };
    gel_rect(&mut c, x0 + bx, y0 + by, bw, bh, bh / 2.0, if pressed { darken(base, 0.2) } else { base });
    let label = "Desbloquear";
    let tw = fonts.measure(label, 13.0, false);
    fonts.draw(&mut c, label, x0 + bx + (bw - tw) / 2.0, y0 + by + 15.5, 13.0, false, if busy { rgba(140, 140, 140, 255) } else { ink });
    Some(c)
}

// ---------------------------------------------------------------------------
// Iniciando sessão
// ---------------------------------------------------------------------------

pub const STARTUP_W: f32 = 340.0;
pub const STARTUP_H: f32 = 190.0;
pub const STARTUP_SHADOW: f32 = 24.0;

/// Painel "Iniciando sessão": orbe, nome, o passo atual e a barra de progresso.
pub fn startup_panel(fonts: &Fonts, theme: &Theme, status: &str, progress: f32, scale: f32) -> Option<Canvas> {
    let (w, h, m) = (STARTUP_W, STARTUP_H, STARTUP_SHADOW);
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &ShadowSpec { blur: 16.0, offset_y: 6.0, radius: 8.0, alpha: 0.45 }, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    c.fill(rrect(x0, y0, w, h, 8.0), &solid(theme.body_b));
    c.pinstripes(x0, y0 + 8.0, w, h - 16.0, theme.body_a, theme.body_b);
    c.stroke(rrect(x0, y0, w, h, 8.0), &solid(rgba(0, 0, 0, 90)), 1.0 / scale);
    gel_circle(&mut c, x0 + w / 2.0, y0 + 46.0, 26.0, theme.accent);
    let center = |c: &mut Canvas, t: &str, y: f32, px: f32, bold: bool, col: Color| {
        let tw = fonts.measure(t, px, bold);
        fonts.draw(c, t, x0 + (w - tw) / 2.0, y, px, bold, col);
    };
    center(&mut c, "Glove", y0 + 104.0, 22.0, true, rgba(20, 20, 20, 255));
    center(&mut c, status, y0 + 132.0, 11.0, false, rgba(50, 50, 50, 255));
    progress_bar(&mut c, x0 + 40.0, y0 + 148.0, w - 80.0, 12.0, Some(progress.clamp(0.0, 1.0)), 0.0, theme.accent);
    Some(c)
}

// ---------------------------------------------------------------------------
// Busca Rápida
// ---------------------------------------------------------------------------

/// Lupa: aro e cabo, numa caixa de s × s.
pub fn magnifier_glyph(c: &mut Canvas, x: f32, y: f32, s: f32, color: Color) {
    let r = s * 0.32;
    let (cx, cy) = (x + r + 1.0, y + r + 1.0);
    c.stroke(circle(cx, cy, r), &solid(color), s * 0.13);
    let mut pb = tiny_skia::PathBuilder::new();
    let k = std::f32::consts::FRAC_1_SQRT_2;
    pb.move_to(cx + r * k + 0.5, cy + r * k + 0.5);
    pb.line_to(x + s - 0.5, y + s - 0.5);
    c.stroke_round(pb.finish(), &solid(color), s * 0.18);
}

/// Uma linha de resultado.
#[derive(Clone, Debug, PartialEq)]
pub struct QsRow {
    /// Nome da seção (só na primeira linha de cada uma).
    pub section: Option<String>,
    pub icon: Icon,
    pub name: String,
}

pub const QS_W: f32 = 430.0;
const QS_HEADER: f32 = 38.0;
const QS_ROW_H: f32 = 21.0;
const QS_LABEL_W: f32 = 118.0;
const QS_PAD: f32 = 6.0;

/// Altura do conteúdo do painel.
pub fn quick_search_height(rows: usize, query_empty: bool, none: bool) -> f32 {
    if query_empty {
        QS_HEADER
    } else if none {
        QS_HEADER + 2.0 * QS_PAD + QS_ROW_H
    } else {
        QS_HEADER + 2.0 * QS_PAD + rows as f32 * QS_ROW_H
    }
}

/// Linha sob o y do conteúdo.
pub fn quick_search_row_at(rows: usize, y: f32) -> Option<usize> {
    let i = ((y - QS_HEADER - QS_PAD) / QS_ROW_H).floor();
    (i >= 0.0 && (i as usize) < rows).then_some(i as usize)
}

/// Painel da Busca Rápida com margem de sombra de menu (conteúdo em MENU_SHADOW, 0).
#[allow(clippy::too_many_arguments)]
pub fn quick_search(fonts: &Fonts, theme: &Theme, query: &str, caret: bool, rows: &[QsRow], selected: Option<usize>, searching: bool, scale: f32) -> Option<Canvas> {
    let none = rows.is_empty();
    let (w, h) = (QS_W, quick_search_height(rows.len(), query.is_empty(), none).ceil());
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;
    let s = ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    c.fill_rect(x0, 0.0, w, h, &solid(rgba(250, 250, 251, 250)));

    // Faixa de cima em azul, com o nome e o campo em cápsula.
    let a = theme.accent;
    c.fill_rect(x0, 0.0, w, QS_HEADER, &vgrad(0.0, QS_HEADER, &[(0.0, lighten(a, 0.35)), (1.0, lighten(a, 0.05))]));
    c.hline(x0, QS_HEADER - 1.0, w, darken(a, 0.25));
    fonts.draw(&mut c, "Busca Rápida", x0 + 12.0, 24.0, 13.0, true, Color::WHITE);
    let (fx, fy, fw, fh) = (x0 + QS_LABEL_W, 8.0, w - QS_LABEL_W - 10.0, 22.0);
    c.fill(rrect(fx, fy, fw, fh, fh / 2.0), &solid(Color::WHITE));
    c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, fh / 2.0), &solid(darken(a, 0.35)), 1.0);
    magnifier_glyph(&mut c, fx + 7.0, fy + 5.0, 12.0, rgba(110, 110, 110, 255));
    let tx = fx + 25.0;
    let ink = rgba(20, 20, 20, 255);
    // Texto longo: mostra o fim (onde se digita).
    let mut shown = query.to_string();
    while fonts.measure(&shown, 13.0, false) > fw - 40.0 && !shown.is_empty() {
        shown.remove(0);
    }
    fonts.draw(&mut c, &shown, tx, fy + 15.5, 13.0, false, ink);
    if caret {
        let cx = tx + fonts.measure(&shown, 13.0, false) + 1.0;
        c.fill_rect(cx, fy + 4.0, 1.0, fh - 8.0, &solid(ink));
    }
    if searching {
        spinner(&mut c, fx + fw - 12.0, fy + fh / 2.0, 6.5, 0.0, rgba(110, 110, 110, 255));
    }

    if !query.is_empty() {
        let y0 = QS_HEADER + QS_PAD;
        if none {
            fonts.draw(&mut c, "Nenhum resultado", x0 + QS_LABEL_W + 26.0, y0 + 15.0, 13.0, false, rgba(120, 120, 120, 255));
        }
        // Coluna das seções em cinza claro.
        c.fill_rect(x0, QS_HEADER, QS_LABEL_W - 8.0, h - QS_HEADER, &solid(rgba(238, 240, 243, 255)));
        for (i, r) in rows.iter().enumerate() {
            let y = y0 + i as f32 * QS_ROW_H;
            if let Some(sec) = &r.section {
                if i > 0 {
                    c.hline(x0, y - 0.5, w, rgba(222, 224, 228, 255));
                }
                let sw = fonts.measure(sec, 11.0, true);
                fonts.draw(&mut c, sec, x0 + QS_LABEL_W - 16.0 - sw, y + 14.5, 11.0, true, rgba(110, 112, 118, 255));
            }
            let sel = selected == Some(i);
            if sel {
                c.fill_rect(x0 + QS_LABEL_W - 8.0, y, w - QS_LABEL_W + 8.0, QS_ROW_H, &solid(theme.menu_hot));
            }
            icons::draw_bare(&mut c, fonts, r.icon, x0 + QS_LABEL_W, y + 2.5, 16.0, None);
            let mut name = r.name.clone();
            let max = w - QS_LABEL_W - 34.0;
            if fonts.measure(&name, 13.0, false) > max {
                while fonts.measure(&format!("{name}…"), 13.0, false) > max && !name.is_empty() {
                    name.pop();
                }
                name.push('…');
            }
            fonts.draw(&mut c, &name, x0 + QS_LABEL_W + 22.0, y + 15.0, 13.0, false, if sel { Color::WHITE } else { ink });
        }
    }
    c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    Some(c)
}

// ---------------------------------------------------------------------------
// Exposé
// ---------------------------------------------------------------------------

/// Título da janela sob o mouse no Exposé: cápsula escura translúcida, texto branco.
pub fn expose_label(fonts: &Fonts, text: &str, scale: f32) -> Option<Canvas> {
    let px = 13.0;
    let mut t = text.to_string();
    while fonts.measure(&t, px, true) > 420.0 && t.chars().count() > 1 {
        t.pop();
    }
    if t.len() < text.len() {
        t.push('…');
    }
    let (w, h) = ((fonts.measure(&t, px, true) + 28.0).ceil(), 26.0);
    let mut c = Canvas::new(w, h, scale)?;
    c.fill(rrect(0.0, 0.0, w, h, h / 2.0), &solid(rgba(20, 20, 24, 170)));
    fonts.draw(&mut c, &t, 14.0, 17.5, px, true, Color::WHITE);
    Some(c)
}

// ---------------------------------------------------------------------------
// Troca de apps (Super+Tab, como no 10.4)
// ---------------------------------------------------------------------------

const SW_ICON: f32 = 96.0;
const SW_CELL: f32 = 124.0;
const SW_PAD: f32 = 18.0;
const SW_LABEL_H: f32 = 34.0;

/// Tamanho da faixa para `n` apps (encolhe os ícones se não couber em `max_w`).
pub fn app_switcher_size(n: usize, max_w: f32) -> (f32, f32, f32) {
    let k = ((max_w - 2.0 * SW_PAD) / (n.max(1) as f32 * SW_CELL)).min(1.0);
    let cell = SW_CELL * k;
    (n.max(1) as f32 * cell + 2.0 * SW_PAD, cell + SW_LABEL_H + SW_PAD, k)
}

/// Faixa escura translúcida com os ícones; o escolhido num quadrado claro e o
/// nome dele embaixo.
pub fn app_switcher(fonts: &Fonts, apps: &[(Icon, String)], selected: usize, max_w: f32, scale: f32) -> Option<Canvas> {
    let (w, h, k) = app_switcher_size(apps.len(), max_w);
    let mut c = Canvas::new(w, h, scale)?;
    c.fill(rrect(0.0, 0.0, w, h, 20.0 * k.max(0.5)), &solid(rgba(24, 26, 30, 165)));
    let cell = SW_CELL * k;
    let icon = SW_ICON * k;
    for (i, (ic, _)) in apps.iter().enumerate() {
        let x = SW_PAD + i as f32 * cell;
        let y = SW_PAD * 0.8;
        if i == selected {
            c.fill(rrect(x + 2.0, y, cell - 4.0, cell - 4.0, 12.0 * k.max(0.5)), &solid(rgba(255, 255, 255, 70)));
        }
        icons::draw(&mut c, fonts, *ic, x + (cell - icon) / 2.0, y + (cell - 4.0 - icon) / 2.0, icon, None);
    }
    if let Some((_, name)) = apps.get(selected) {
        let px = 15.0;
        let tw = fonts.measure(name, px, true);
        fonts.draw(&mut c, name, ((w - tw) / 2.0).max(8.0), h - SW_LABEL_H / 2.0 + 2.0, px, true, Color::WHITE);
    }
    Some(c)
}

/// Menu com bico embaixo apontando para `pointer_x` (relativo ao conteúdo), para
/// os menus do Dock. O bico ocupa a margem de sombra de baixo.
pub fn menu_with_pointer(fonts: &Fonts, theme: &Theme, items: &[MenuItem], hover: Option<usize>, pointer_x: f32, scale: f32) -> Option<Canvas> {
    let mut c = menu(fonts, theme, items, hover, scale)?;
    let (w, h) = menu_content_size(fonts, items);
    let m = MENU_SHADOW as f32;
    let px = m + pointer_x.clamp(14.0, w - 14.0);
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(px - 9.0, h - 0.5);
    pb.line_to(px, h + 9.0);
    pb.line_to(px + 9.0, h - 0.5);
    pb.close();
    let bg = if theme.variant == Variant::Claro { rgba(237, 240, 241, 255) } else { rgba(244, 245, 247, 255) };
    c.fill(pb.finish(), &solid(bg));
    // Só as duas bordas do bico (a de cima se funde com o menu).
    let mut edge = tiny_skia::PathBuilder::new();
    edge.move_to(px - 9.0, h - 0.5);
    edge.line_to(px, h + 9.0);
    edge.line_to(px + 9.0, h - 0.5);
    c.stroke(edge.finish(), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    Some(c)
}

// ---------------------------------------------------------------------------
// Avatares originais (usuários sem foto)
// ---------------------------------------------------------------------------

/// Número estável a partir de um texto (FNV-1a), para escolher o avatar.
pub fn avatar_seed(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// Avatar desenhado: azulejo colorido com um desenho (gota, folha, bola,
/// estrela, flor ou nuvem), escolhido por `seed`.
pub fn avatar(c: &mut Canvas, x: f32, y: f32, s: f32, seed: u64) {
    const TILES: [(u8, u8, u8); 6] = [(86, 150, 222), (98, 178, 112), (232, 150, 64), (164, 110, 196), (226, 96, 110), (70, 172, 176)];
    let (r, g, b) = TILES[(seed % 6) as usize];
    let base = rgba(r, g, b, 255);
    let tile = || rrect(x, y, s, s, s * 0.14);
    c.fill(tile(), &vgrad(y, y + s, &[(0.0, lighten(base, 0.35)), (1.0, darken(base, 0.1))]));
    c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, s * 0.14), &solid(rgba(0, 0, 0, 80)), 1.0);
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    let white = rgba(255, 255, 255, 235);
    let k = s / 100.0;
    let mut pb = tiny_skia::PathBuilder::new();
    match (seed / 6) % 6 {
        0 => {
            // Gota em gel.
            pb.move_to(cx, cy - 34.0 * k);
            pb.cubic_to(cx + 8.0 * k, cy - 18.0 * k, cx + 26.0 * k, cy + 2.0 * k, cx + 24.0 * k, cy + 16.0 * k);
            pb.cubic_to(cx + 22.0 * k, cy + 34.0 * k, cx - 22.0 * k, cy + 34.0 * k, cx - 24.0 * k, cy + 16.0 * k);
            pb.cubic_to(cx - 26.0 * k, cy + 2.0 * k, cx - 8.0 * k, cy - 18.0 * k, cx, cy - 34.0 * k);
            pb.close();
            let p = pb.finish();
            c.fill(p.clone(), &vgrad(cy - 34.0 * k, cy + 30.0 * k, &[(0.0, rgba(190, 225, 255, 255)), (1.0, rgba(40, 110, 210, 255))]));
            c.stroke(p, &solid(rgba(20, 60, 140, 200)), 1.2 * k);
            c.fill(circle(cx - 9.0 * k, cy + 8.0 * k, 5.0 * k), &solid(rgba(255, 255, 255, 170)));
        }
        1 => {
            // Folha com nervura.
            pb.move_to(cx - 26.0 * k, cy + 26.0 * k);
            pb.cubic_to(cx - 30.0 * k, cy - 18.0 * k, cx + 6.0 * k, cy - 34.0 * k, cx + 30.0 * k, cy - 30.0 * k);
            pb.cubic_to(cx + 32.0 * k, cy - 2.0 * k, cx + 14.0 * k, cy + 30.0 * k, cx - 26.0 * k, cy + 26.0 * k);
            pb.close();
            c.fill(pb.finish(), &vgrad(cy - 30.0 * k, cy + 28.0 * k, &[(0.0, rgba(170, 230, 120, 255)), (1.0, rgba(50, 140, 60, 255))]));
            let mut v = tiny_skia::PathBuilder::new();
            v.move_to(cx - 26.0 * k, cy + 26.0 * k);
            v.quad_to(cx + 2.0 * k, cy - 2.0 * k, cx + 26.0 * k, cy - 26.0 * k);
            c.stroke_round(v.finish(), &solid(rgba(30, 90, 40, 220)), 2.2 * k);
        }
        2 => {
            // Bola de praia em gel.
            gel_circle(c, cx, cy, 28.0 * k, rgba(240, 240, 240, 255));
            for (i, col) in [(0, rgba(230, 60, 60, 230)), (2, rgba(50, 120, 220, 230)), (4, rgba(250, 200, 40, 230))] {
                let a0 = i as f32 * std::f32::consts::TAU / 6.0;
                let mut w = tiny_skia::PathBuilder::new();
                w.move_to(cx, cy);
                for j in 0..=8 {
                    let a = a0 + j as f32 / 8.0 * std::f32::consts::TAU / 6.0;
                    w.line_to(cx + a.cos() * 26.5 * k, cy + a.sin() * 26.5 * k);
                }
                w.close();
                c.fill(w.finish(), &solid(col));
            }
            c.fill(circle(cx - 9.0 * k, cy - 11.0 * k, 7.0 * k), &solid(rgba(255, 255, 255, 120)));
        }
        3 => {
            // Estrela de cinco pontas.
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rr = if i % 2 == 0 { 32.0 } else { 13.0 } * k;
                let (px, py) = (cx + a.cos() * rr, cy + 2.0 * k + a.sin() * rr);
                if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
            }
            pb.close();
            let p = pb.finish();
            c.fill(p.clone(), &vgrad(cy - 30.0 * k, cy + 30.0 * k, &[(0.0, rgba(255, 240, 150, 255)), (1.0, rgba(240, 170, 30, 255))]));
            c.stroke(p, &solid(rgba(150, 90, 10, 200)), 1.2 * k);
        }
        4 => {
            // Flor: cinco pétalas e o miolo.
            for i in 0..5 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                c.fill(circle(cx + a.cos() * 15.0 * k, cy + a.sin() * 15.0 * k, 12.0 * k), &solid(white));
            }
            gel_circle(c, cx, cy, 9.0 * k, rgba(250, 190, 40, 255));
        }
        _ => {
            // Nuvem.
            for (dx, dy, r) in [(-14.0, 6.0, 13.0), (0.0, -4.0, 17.0), (15.0, 5.0, 12.0)] {
                c.fill(circle(cx + dx * k, cy + dy * k, r * k), &solid(white));
            }
            c.fill(rrect(cx - 27.0 * k, cy + 4.0 * k, 54.0 * k, 15.0 * k, 7.5 * k), &solid(white));
        }
    }
}
