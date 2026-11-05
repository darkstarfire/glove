//! Temas. Cada variante lembra uma fase da era "gel" dos anos 2000,
//! com nomes e arte próprios.

use crate::draw::rgba;
use tiny_skia::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Sóbrio e claro, o padrão (lembra o fim da era): barra de menu lisa, menus com
    /// separador em linha, Dock liso e janelas em cinza claro.
    Claro,
    /// Listras finas e gel azul (lembra o começo da era).
    Listras,
    /// Metal escovado nas barras de título.
    Metal,
    /// Listras com acentos em grafite, mais sóbrio.
    Grafite,
}

impl Variant {
    pub const ALL: [Variant; 4] = [Variant::Claro, Variant::Listras, Variant::Metal, Variant::Grafite];

    pub fn id(self) -> &'static str {
        match self {
            Variant::Claro => "claro",
            Variant::Listras => "listras",
            Variant::Metal => "metal",
            Variant::Grafite => "grafite",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Variant::Claro => "Claro",
            Variant::Listras => "Listras",
            Variant::Metal => "Metal",
            Variant::Grafite => "Grafite",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id().eq_ignore_ascii_case(s))
    }

    /// Tema da configuração, com o padrão (Claro) quando o nome é desconhecido.
    pub fn from_config(s: &str) -> Self {
        Self::from_id(s).unwrap_or(Variant::Claro)
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
    /// Item de menu sob o mouse (o Tiger usa um azul liso, sem degradê).
    pub menu_hot: Color,
    /// Fundo das janelas dos apps: listras alternadas.
    pub body_a: Color,
    pub body_b: Color,
    pub wallpaper: [Color; 3],
    /// Fundo do texto selecionado (cor de destaque do painel Aparência).
    pub highlight: Color,
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
            Variant::Claro | Variant::Listras => [rgba(10, 40, 110, 255), rgba(30, 95, 200, 255), rgba(140, 200, 255, 255)],
            Variant::Metal => [rgba(8, 28, 70, 255), rgba(40, 80, 160, 255), rgba(170, 200, 240, 255)],
            Variant::Grafite => [rgba(35, 40, 52, 255), rgba(85, 95, 115, 255), rgba(190, 200, 215, 255)],
        };
        let claro = variant == Variant::Claro;
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
            // Metal do Tiger: claro e suave (de ~230 a ~200), riscos quase invisíveis.
            metal_top: rgba(232, 232, 232, 255),
            metal_bottom: rgba(203, 203, 205, 255),
            menu_bg_a: rgba(246, 246, 246, 238),
            menu_bg_b: rgba(255, 255, 255, 238),
            menu_text: rgba(0, 0, 0, 255),
            menu_hot: if claro { rgba(66, 132, 214, 255) } else { accent },
            body_a: if claro { rgba(240, 240, 240, 255) } else { rgba(232, 232, 232, 255) },
            body_b: if claro { rgba(243, 243, 243, 255) } else { rgba(242, 242, 242, 255) },
            wallpaper,
            highlight: if graphite { rgba(199, 208, 219, 255) } else { rgba(181, 213, 255, 255) },
        }
    }
}
