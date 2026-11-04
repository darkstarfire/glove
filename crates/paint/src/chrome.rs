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
    c.hline(0.0, h - 1.0 / scale, w, edge);
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
    let parts = [(-m, -m, w + 2 * m, m), (-m, h, w + 2 * m, m), (-m, 0, m, h), (w, 0, m, h)];
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
}

pub const MENUBAR_PX: f32 = 14.0;

/// `titles[0]` é o nome do app (em negrito). `open`: título aberto (0 = logo, 1.. = títulos).
pub fn menubar(fonts: &Fonts, theme: &Theme, w: f32, titles: &[String], clock: &str, open: Option<usize>, scale: f32) -> Option<(Canvas, MenuBarLayout)> {
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
    let cw = fonts.measure(clock, MENUBAR_PX, false);
    fonts.draw(&mut c, clock, w - cw - 14.0, 16.0, MENUBAR_PX, false, theme.menu_text);
    Some((c, MenuBarLayout { logo, titles: ranges }))
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
}

impl MenuItem {
    pub fn item(label: impl Into<String>) -> Self {
        Self { label: label.into(), checked: false, separator: false, shortcut: String::new(), enabled: true, submenu: false }
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
}

pub const DOCK_BASE: f32 = 56.0;
pub const DOCK_MAX: f32 = 80.0;
const DOCK_GAP: f32 = 6.0;
const DOCK_PAD_X: f32 = 10.0;
const DOCK_PAD_Y: f32 = 4.0;
const DOCK_SEP: f32 = 14.0;
const DOCK_LABEL_H: f32 = 26.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DockLayout {
    /// Tamanho lógico.
    pub width: u32,
    pub height: u32,
    /// (x, y, tamanho) de cada ícone, relativos ao Dock.
    pub items: Vec<(f32, f32, f32)>,
    /// Prateleira de vidro: (x, y, w, h).
    pub shelf: (f32, f32, f32, f32),
}

/// Posição de cada ícone. `hover_x` é relativo ao Dock; `zoom` (0 a 1) é a força
/// da ampliação, animada ao entrar e sair do Dock para não pular de uma vez.
pub fn dock_layout(items: &[DockItem], hover_x: Option<f32>, zoom: f32, base: f32, max: f32) -> DockLayout {
    let n = items.len() as f32;
    let seps = items.iter().filter(|i| i.separator_before).count() as f32;
    let base_w = n * (base + DOCK_GAP) - DOCK_GAP + seps * DOCK_SEP;
    let extra = (max - base).max(0.0) * 3.2;
    let width = (base_w + extra + 2.0 * DOCK_PAD_X).ceil() as u32 + 4;
    let height = (DOCK_LABEL_H + max.max(base) + 2.0 * DOCK_PAD_Y + 6.0).ceil() as u32;
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
    let shelf_h = base + DOCK_PAD_Y + 6.0;
    let shelf = (center - total / 2.0 - DOCK_PAD_X, height as f32 - shelf_h, total + 2.0 * DOCK_PAD_X, shelf_h);
    DockLayout { width, height, items: out, shelf }
}

/// Item do Dock sob (x, y) relativos ao Dock.
pub fn dock_item_at(layout: &DockLayout, x: f32, y: f32) -> Option<usize> {
    let sy = layout.shelf.1;
    layout
        .items
        .iter()
        .position(|(ix, iy, s)| x >= *ix - DOCK_GAP / 2.0 && x < ix + s + DOCK_GAP / 2.0 && y >= iy.min(sy) && y < layout.height as f32)
}

/// O mouse está sobre o Dock (prateleira ou ícones)?
pub fn dock_contains(layout: &DockLayout, x: f32, y: f32) -> bool {
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

pub fn dock(fonts: &Fonts, theme: &Theme, items: &[DockItem], layout: &DockLayout, hover: Option<usize>, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(layout.width as f32, layout.height as f32, scale)?;
    let (sx, sy, sw, sh) = layout.shelf;

    // Prateleira de vidro fosco: retângulo translúcido com bordas claras.
    // Vidro branco translúcido com listras finas, encostado na borda de baixo.
    let shelf = rrect4(sx, sy, sw, sh + 2.0, [2.0, 2.0, 0.0, 0.0]);
    if theme.variant == Variant::Claro {
        // 10.4: vidro branco liso, sem listras.
        c.fill(shelf.clone(), &solid(rgba(255, 255, 255, 115)));
    } else {
        c.fill(shelf.clone(), &vgrad(sy, sy + sh, &[(0.0, rgba(255, 255, 255, 120)), (0.5, rgba(240, 244, 250, 100)), (1.0, rgba(225, 232, 242, 112))]));
        c.pinstripes(sx + 1.0, sy + 1.0, sw - 2.0, sh - 1.0, rgba(255, 255, 255, 0), rgba(255, 255, 255, 34));
    }
    c.stroke(shelf, &solid(rgba(255, 255, 255, 170)), 1.0);
    c.hline(sx, sy - 1.0, sw, rgba(0, 0, 0, 50));
    c.hline(sx + 2.0, sy + 1.0, sw - 4.0, rgba(255, 255, 255, 210));

    for (i, (it, (x, y, s))) in items.iter().zip(&layout.items).enumerate() {
        if it.separator_before {
            let lx = x - DOCK_GAP / 2.0 - DOCK_SEP / 2.0;
            c.vline(lx, sy, sh, rgba(0, 0, 0, 70));
            c.vline(lx + 1.0, sy, sh, rgba(255, 255, 255, 190));
        }
        icons::draw_bare(&mut c, fonts, it.icon, *x, *y, *s, it.letter);
        if it.running {
            // Indicador de app aberto: pequeno triângulo escuro na base.
            let cx = x + s / 2.0;
            let by = layout.height as f32 - 1.5;
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(cx - 4.0, by);
            pb.line_to(cx + 4.0, by);
            pb.line_to(cx, by - 4.5);
            pb.close();
            c.fill(pb.finish(), &solid(rgba(15, 20, 30, 220)));
        }
        if hover == Some(i) {
            let px = 14.0;
            let tw = fonts.measure(&it.label, px, true);
            let lx = (x + s / 2.0 - tw / 2.0).clamp(10.0, layout.width as f32 - tw - 10.0);
            let ly = (y - 8.0).max(16.0);
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
