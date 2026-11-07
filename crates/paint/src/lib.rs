//! glove-paint: desenho por software do visual Glove (inspirado na era Aqua).
//!
//! Tudo é desenhado por código (gradientes, gel, listras, metal escovado),
//! sem nenhuma imagem externa. Os resultados são `Pixmap`s RGBA
//! pré-multiplicados, prontos para virar textura no compositor.

pub mod chrome;
pub mod draw;
pub mod font;
pub mod sysicons;
pub mod icons;
pub mod saver;
pub mod theme;
mod tiger;
mod tiger_prefs;
mod tiger_real;

/// Quantas bolas de papel enchem o ícone do Lixo.
pub use tiger::TRASH_FULL_AT;
pub mod wallpaper;
pub mod widgets;

pub use font::Fonts;
pub use theme::{Theme, Variant};
pub use tiny_skia;
pub use tiny_skia::{Color, Pixmap};
