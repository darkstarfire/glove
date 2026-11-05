//! glove-kit: toolkit dos apps do Glove.
//!
//! Interface em modo imediato, desenhada por software com o visual do Glove,
//! nítida em qualquer escala e sem gastar CPU quando nada acontece.

pub mod clipfiles;
pub mod filebrowser;
pub mod image;
pub mod menu;
mod panels;
mod platform;
mod text;
pub mod ui;
pub use panels::{ColorPanel, FontPanel};
pub use text::{Align, TextBuffer};

pub use glove_paint as paint;
pub use platform::{app_icon, local_time, run, theme_from_config, use_own_frame, App, WindowOptions};
pub use menu::{Menu, MenuItem};
pub use filebrowser::{Choice, FileBrowser};
pub use ui::{disclosure_triangle, AlertKind, Column, Rect, TableResponse, TableRow, TableState, TableStyle, Ticks, Ui, SCROLLBAR_W, SEARCH_H, SEGMENT_H, SEGMENT_W, TABLE_HEADER_H};
pub use glove_paint::icons::Glyph;
