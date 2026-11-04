//! Temas. Cada variante lembra uma fase da era "gel" dos anos 2000,
//! com nomes e arte próprios.

use crate::draw::rgba;
use tiny_skia::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Listras finas e gel azul (lembra o começo da era).
    Listras,
    /// Metal escovado nas barras de título.
    Metal,
    /// Listras com acentos em grafite, mais sóbrio.
    Grafite,
}

impl Variant {
    pub const ALL: [Variant; 3] = [Variant::Listras, Variant::Metal, Variant::Grafite];

    pub fn id(self) -> &'static str {
        match self {
            Variant::Listras => "listras",
            Variant::Metal => "metal",
            Variant::Grafite => "grafite",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Variant::Listras => "Listras",
            Variant::Metal => "Metal",
            Variant::Grafite => "Grafite",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id().eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub variant: Variant,
    /// Cor de destaque (gel de botões, seleção de menu, Dock ativo).
    pub accent: Color,
    pub close: Color,
    pub minimize: Color,
    pub zoom: Color,
    /// Botões de janelas inativas.
    pub inactive_button: Color,
    pub title_text: Color,
    pub title_text_inactive: Color,
    pub stripe_a: Color,
    pub stripe_b: Color,
    pub metal_top: Color,
    pub metal_bottom: Color,
    pub menu_bg_a: Color,
    pub menu_bg_b: Color,
    pub menu_text: Color,
    pub wallpaper: [Color; 3],
}

pub const TITLE_H: i32 = 22;
pub const MENUBAR_H: i32 = 22;

impl Theme {
    pub fn new(variant: Variant) -> Self {
        let graphite = variant == Variant::Grafite;
        let accent = if graphite { rgba(120, 132, 150, 255) } else { rgba(45, 120, 230, 255) };
        let (close, minimize, zoom) = if graphite {
            (rgba(140, 150, 165, 255), rgba(140, 150, 165, 255), rgba(140, 150, 165, 255))
        } else {
            (rgba(230, 70, 60, 255), rgba(240, 180, 40, 255), rgba(80, 190, 70, 255))
        };
        let wallpaper = match variant {
            Variant::Listras => [rgba(10, 40, 110, 255), rgba(30, 95, 200, 255), rgba(140, 200, 255, 255)],
            Variant::Metal => [rgba(8, 28, 70, 255), rgba(40, 80, 160, 255), rgba(170, 200, 240, 255)],
            Variant::Grafite => [rgba(35, 40, 52, 255), rgba(85, 95, 115, 255), rgba(190, 200, 215, 255)],
        };
        Self {
            variant,
            accent,
            close,
            minimize,
            zoom,
            inactive_button: rgba(190, 190, 195, 255),
            title_text: rgba(20, 20, 20, 255),
            title_text_inactive: rgba(130, 130, 130, 255),
            stripe_a: rgba(236, 236, 236, 255),
            stripe_b: rgba(246, 246, 246, 255),
            metal_top: rgba(222, 222, 222, 255),
            metal_bottom: rgba(170, 170, 172, 255),
            menu_bg_a: rgba(246, 246, 246, 238),
            menu_bg_b: rgba(255, 255, 255, 238),
            menu_text: rgba(0, 0, 0, 255),
            wallpaper,
        }
    }
}
