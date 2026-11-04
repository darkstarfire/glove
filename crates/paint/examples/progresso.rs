//! Prévia da barra de progresso e do giratório:
//! `cargo run -p glove-paint --example progresso -- saida.png [escala]`
use glove_paint::draw::*;
use glove_paint::theme::{Theme, Variant};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or("progresso.png".into());
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let mut c = Canvas::new(420.0, 190.0, scale).unwrap();
    c.clear(rgba(240, 240, 240, 255));
    c.pinstripes(0.0, 0.0, 420.0, 190.0, rgba(240, 240, 240, 255), rgba(243, 243, 243, 255));
    for (i, v) in [Variant::Claro, Variant::Grafite].into_iter().enumerate() {
        let accent = Theme::new(v).accent;
        let x = 16.0 + i as f32 * 205.0;
        for (j, val) in [Some(0.0), Some(0.12), Some(0.6), Some(1.0)].into_iter().enumerate() {
            progress_bar(&mut c, x, 14.0 + j as f32 * 22.0, 190.0, 12.0, val, 0.0, accent);
        }
        for (j, ph) in [0.0, 0.33].into_iter().enumerate() {
            progress_bar(&mut c, x, 106.0 + j as f32 * 22.0, 190.0, 12.0, None, ph, accent);
        }
        for (j, ph) in [0.0, 0.25, 0.5].into_iter().enumerate() {
            spinner(&mut c, x + 14.0 + j as f32 * 34.0, 168.0, 8.0, ph, rgba(60, 60, 64, 255));
            spinner(&mut c, x + 130.0 + j as f32 * 0.0, 168.0, 14.0, ph, rgba(60, 60, 64, 255));
        }
    }
    c.pm.save_png(&out).unwrap();
    println!("salvo em {out}");
}
