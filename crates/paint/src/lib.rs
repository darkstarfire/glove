//! glove-paint: desenho por software do visual Glove (inspirado na era Aqua).
//!
//! Tudo é desenhado por código (gradientes, gel, listras, metal escovado),
//! sem nenhuma imagem externa. Os resultados são `Pixmap`s RGBA
//! pré-multiplicados, prontos para virar textura no compositor.

pub mod chrome;
pub mod draw;
pub mod font;
pub mod icons;
pub mod theme;
pub mod wallpaper;

pub use font::Fonts;
pub use theme::{Theme, Variant};
pub use tiny_skia;
pub use tiny_skia::{Color, Pixmap};
