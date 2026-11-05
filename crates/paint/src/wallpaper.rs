//! Papel de parede procedural: fundo profundo com faixas de luz suaves.

use crate::draw::*;
use crate::theme::Theme;
use tiny_skia::PathBuilder;

/// `w` e `h` são lógicos; o resultado tem a resolução física da escala.
pub fn render(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, light] = theme.wallpaper;

    // Base: gradiente diagonal.
    c.fill_rect(0.0, 0.0, w, h, &linear(0.0, 0.0, w * 0.3, h, &[(0.0, mid), (1.0, deep)]));

    // Brilho grande no alto, à esquerda.
    let r = w.max(h) * 0.75;
    c.fill_rect(0.0, 0.0, w, h, &radial(w * 0.28, h * 0.18, r, &[(0.0, with_alpha(light, 0.55)), (1.0, with_alpha(light, 0.0))]));

    // Faixas curvas de luz atravessando a tela.
    let bands: [(f32, f32, f32, f32); 4] = [(0.55, 0.30, 0.18, 0.20), (0.70, 0.12, 0.10, 0.14), (0.82, -0.05, 0.07, 0.10), (0.40, 0.45, 0.05, 0.08)];
    for (y, bend, thick, alpha) in bands {
        let y0 = h * y;
        let t = h * thick;
        let mut pb = PathBuilder::new();
        pb.move_to(-w * 0.1, y0);
        pb.cubic_to(w * 0.3, y0 - h * bend, w * 0.65, y0 + h * bend, w * 1.1, y0 - h * bend * 0.5);
        pb.line_to(w * 1.1, y0 - h * bend * 0.5 + t);
        pb.cubic_to(w * 0.65, y0 + h * bend + t * 1.6, w * 0.3, y0 - h * bend + t * 0.4, -w * 0.1, y0 + t);
        pb.close();
        c.fill(
            pb.finish(),
            // Degradê pela altura real da faixa (com curva negativa, os limites se
            // invertiam e o degradê virava uma emenda dura).
            &vgrad(
                y0 - h * bend.abs(),
                y0 + h * bend.abs() + t * 1.6,
                &[(0.0, with_alpha(light, 0.0)), (0.5, with_alpha(light, alpha)), (1.0, with_alpha(light, 0.0))],
            ),
        );
    }

    // Vinheta escura nas bordas.
    c.fill_rect(0.0, 0.0, w, h, &radial(w * 0.5, h * 0.45, w.max(h) * 0.8, &[(0.6, with_alpha(deep, 0.0)), (1.0, with_alpha(deep, 0.6))]));
    Some(c)
}

/// Papel de parede por estilo: "ondas", "aurora", "liso" ou caminho de um PNG.
pub fn render_style(theme: &Theme, style: &str, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    match style {
        "aurora" => aurora(theme, w, h, scale),
        "liso" => plain(theme, w, h, scale),
        "ondas" | "" => render(theme, w, h, scale),
        c if c.starts_with("cor:") => solid_color(c, w, h, scale).or_else(|| render(theme, w, h, scale)),
        path => image(path, w, h, scale).or_else(|| render(theme, w, h, scale)),
    }
}

/// Cor sólida: "cor:R,G,B".
pub fn parse_color(style: &str) -> Option<(u8, u8, u8)> {
    let mut it = style.strip_prefix("cor:")?.split(',').map(|v| v.trim().parse::<u8>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

fn solid_color(style: &str, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let (r, g, b) = parse_color(style)?;
    let mut c = Canvas::new(w, h, scale)?;
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(r, g, b, 255)));
    Some(c)
}

fn plain(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, _] = theme.wallpaper;
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, mid), (1.0, deep)]));
    Some(c)
}

fn aurora(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, light] = theme.wallpaper;
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, deep), (1.0, mid)]));
    let m = w.max(h);
    for (x, y, r, a) in [(0.2, 0.75, 0.55, 0.45), (0.75, 0.3, 0.45, 0.35), (0.5, 1.05, 0.6, 0.5), (0.9, 0.85, 0.3, 0.25)] {
        c.fill_rect(0.0, 0.0, w, h, &radial(w * x, h * y, m * r, &[(0.0, with_alpha(light, a)), (1.0, with_alpha(light, 0.0))]));
    }
    Some(c)
}

/// Imagem PNG cobrindo a tela inteira (corta o excesso, sem distorcer).
fn image(path: &str, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let img = tiny_skia::Pixmap::load_png(path).ok()?;
    let mut c = Canvas::new(w, h, scale)?;
    let (pw, ph) = (c.px_width() as f32, c.px_height() as f32);
    let k = (pw / img.width() as f32).max(ph / img.height() as f32);
    let (dx, dy) = ((pw - img.width() as f32 * k) / 2.0, (ph - img.height() as f32 * k) / 2.0);
    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
    c.pm.draw_pixmap(0, 0, img.as_ref(), &paint, tiny_skia::Transform::from_row(k, 0.0, 0.0, k, dx, dy), None);
    Some(c)
}
