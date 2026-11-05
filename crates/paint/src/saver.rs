//! Protetores de tela originais, desenhados em função do tempo (sem estado):
//! o compositor chama `draw` a cada quadro e as Preferências usam o mesmo
//! desenho na prévia.

use crate::draw::*;
use crate::font::Fonts;
use tiny_skia::PathBuilder;

/// (id, nome) na ordem do painel.
pub const SAVERS: [(&str, &str); 5] = [("aurora", "Aurora"), ("estrelas", "Estrelas"), ("bolhas", "Bolhas"), ("ondas", "Ondas"), ("relogio", "Relógio")];

/// Número de 0 a 1 a partir de um inteiro (sempre o mesmo para o mesmo `i`).
fn rnd(i: u32, salt: u32) -> f32 {
    (hash(i.wrapping_mul(2654435761).wrapping_add(salt.wrapping_mul(97))) % 10_000) as f32 / 10_000.0
}

fn frac(x: f32) -> f32 {
    x - x.floor()
}

/// Matiz (0 a 360) → cor viva.
fn hue(h: f32, alpha: f32) -> tiny_skia::Color {
    let h = h.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    tiny_skia::Color::from_rgba(r * 0.85 + 0.1, g * 0.85 + 0.1, b * 0.85 + 0.1, alpha).unwrap_or(tiny_skia::Color::BLACK)
}

/// Desenha o protetor `kind` no instante `t` (segundos). `clock`: hora e data
/// para o protetor Relógio.
pub fn draw(c: &mut Canvas, fonts: &Fonts, kind: &str, w: f32, h: f32, t: f32, clock: (&str, &str)) {
    match kind {
        "estrelas" => stars(c, w, h, t),
        "bolhas" => bubbles(c, w, h, t),
        "ondas" => waves(c, w, h, t),
        "relogio" => clock_saver(c, fonts, w, h, t, clock),
        _ => aurora(c, w, h, t),
    }
}

/// Manchas de luz coloridas que passeiam devagar sobre o preto.
fn aurora(c: &mut Canvas, w: f32, h: f32, t: f32) {
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(4, 6, 14, 255)));
    let m = w.max(h);
    for i in 0..5u32 {
        let (a, b) = (0.05 + rnd(i, 1) * 0.08, 0.04 + rnd(i, 2) * 0.07);
        let x = w * (0.5 + 0.38 * (t * a + rnd(i, 3) * 6.3).sin());
        let y = h * (0.5 + 0.36 * (t * b + rnd(i, 4) * 6.3).cos());
        let r = m * (0.22 + 0.12 * rnd(i, 5));
        let col = hue(i as f32 * 72.0 + t * 8.0, 0.55);
        c.fill_rect(0.0, 0.0, w, h, &radial(x, y, r, &[(0.0, col), (1.0, with_alpha(col, 0.0))]));
    }
}

/// Viagem por um campo de estrelas.
fn stars(c: &mut Canvas, w: f32, h: f32, t: f32) {
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 4, 255)));
    let (cx, cy) = (w / 2.0, h / 2.0);
    let spread = w.max(h) * 0.5;
    for i in 0..260u32 {
        let ang = rnd(i, 1) * std::f32::consts::TAU;
        let base = 0.05 + rnd(i, 2) * 0.95;
        // z vai de 1 (longe) a 0 (perto) e recomeça.
        let z = 1.0 - frac(rnd(i, 3) + t * (0.06 + rnd(i, 4) * 0.05));
        let d = base * spread * (0.12 / z.max(0.02));
        let (x, y) = (cx + ang.cos() * d, cy + ang.sin() * d);
        if x < -4.0 || y < -4.0 || x > w + 4.0 || y > h + 4.0 {
            continue;
        }
        let near = (1.0 - z).clamp(0.0, 1.0);
        let r = 0.4 + near * 2.2;
        c.fill(circle(x, y, r), &solid(rgba(220, 230, 255, (60.0 + near * 195.0) as u8)));
    }
}

/// Bolhas de gel subindo num azul profundo.
fn bubbles(c: &mut Canvas, w: f32, h: f32, t: f32) {
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(8, 30, 80, 255)), (1.0, rgba(20, 80, 160, 255))]));
    let m = w.min(h);
    for i in 0..28u32 {
        let r = m * (0.02 + rnd(i, 1) * 0.07);
        let speed = 0.03 + rnd(i, 2) * 0.05;
        let x = rnd(i, 3) * w + (t * (0.4 + rnd(i, 4)) + i as f32).sin() * m * 0.03;
        let y = h + r - frac(rnd(i, 5) + t * speed) * (h + 2.0 * r);
        c.fill(circle(x, y, r), &radial(x - r * 0.3, y - r * 0.35, r * 1.4, &[(0.0, rgba(255, 255, 255, 70)), (0.7, rgba(150, 200, 255, 30)), (1.0, rgba(150, 200, 255, 90))]));
        c.stroke(circle(x, y, r), &solid(rgba(200, 230, 255, 120)), (r * 0.06).max(0.8));
        c.fill(oval(x - r * 0.55, y - r * 0.75, r * 0.6, r * 0.35), &solid(rgba(255, 255, 255, 150)));
    }
}

/// As faixas de luz do papel de parede, ondulando devagar.
fn waves(c: &mut Canvas, w: f32, h: f32, t: f32) {
    c.fill_rect(0.0, 0.0, w, h, &linear(0.0, 0.0, w * 0.3, h, &[(0.0, rgba(30, 95, 200, 255)), (1.0, rgba(10, 40, 110, 255))]));
    let light = rgba(140, 200, 255, 255);
    for k in 0..5u32 {
        let y0 = h * (0.3 + k as f32 * 0.14);
        let bend = 0.18 * (t * (0.12 + k as f32 * 0.03) + k as f32).sin();
        let th = h * (0.06 + 0.04 * rnd(k, 1));
        let mut pb = PathBuilder::new();
        pb.move_to(-w * 0.1, y0);
        pb.cubic_to(w * 0.3, y0 - h * bend, w * 0.65, y0 + h * bend, w * 1.1, y0 - h * bend * 0.5);
        pb.line_to(w * 1.1, y0 - h * bend * 0.5 + th);
        pb.cubic_to(w * 0.65, y0 + h * bend + th * 1.6, w * 0.3, y0 - h * bend + th * 0.4, -w * 0.1, y0 + th);
        pb.close();
        let a = 0.12 + 0.06 * (t * 0.5 + k as f32).sin().abs();
        c.fill(pb.finish(), &vgrad(y0 - h * 0.2, y0 + h * 0.25, &[(0.0, with_alpha(light, 0.0)), (0.5, with_alpha(light, a)), (1.0, with_alpha(light, 0.0))]));
    }
}

/// Onda triangular de 0 a 1 (para quicar nas bordas).
fn tri(x: f32) -> f32 {
    let f = frac(x);
    if f < 0.5 { f * 2.0 } else { 2.0 - f * 2.0 }
}

/// A hora em letras grandes, passeando pela tela (sem marcar sempre o mesmo lugar).
fn clock_saver(c: &mut Canvas, fonts: &Fonts, w: f32, h: f32, t: f32, (time, date): (&str, &str)) {
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 255)));
    let px = (h * 0.16).max(12.0);
    let tw = fonts.measure(time, px, true);
    let dw = fonts.measure(date, px * 0.28, false);
    let bw = tw.max(dw);
    let x = tri(t * 0.013) * (w - bw).max(0.0);
    let y = px + tri(t * 0.009 + 0.3) * (h - px * 1.6).max(0.0);
    fonts.draw(c, time, x + (bw - tw) / 2.0, y, px, true, rgba(150, 190, 240, 230));
    fonts.draw(c, date, x + (bw - dw) / 2.0, y + px * 0.45, px * 0.28, false, rgba(120, 140, 170, 220));
}
