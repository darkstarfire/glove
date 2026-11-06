//! Ícones dos painéis das Preferências, dos periféricos e dos utilitários na
//! linguagem do Tiger (Fase 9.11.5), com arte própria. Usa os materiais de
//! `tiger` (metal escovado, vidro, gel, couro, papel).

use tiny_skia::{Color, Path, PathBuilder, Rect, Transform};

use crate::draw::*;
use crate::font::Fonts;
use crate::tiger::{brush_streaks, glass_glare, leather_grain, metal_paint, sheet, soft_shadow, spun_metal, tf, tpaint, U};

const PI: f32 = std::f32::consts::PI;

/// Gel brilhante (os botões do Aqua): degradê da cor, reflexo forte em cima,
/// brilho de luz atravessando embaixo e contorno escuro.
pub(crate) fn gel(pm: &mut Canvas, u: U, p: Option<Path>, base: Color, y0: f32, y1: f32) {
    pm.fill(p.clone(), &vgrad(u.py(y0), u.py(y1), &[(0.0, lighten(base, 0.35)), (0.5, base), (1.0, darken(base, 0.3))]));
    let h = y1 - y0;
    pm.fill_clipped(u.rr(0.0, y0 + h * 0.6, 1.0, h * 0.4, 0.0), &vgrad(u.py(y0 + h * 0.6), u.py(y1), &[(0.0, with_alpha(lighten(base, 0.6), 0.0)), (1.0, with_alpha(lighten(base, 0.6), 0.55))]), p.clone());
    pm.fill_clipped(u.rr(0.0, y0, 1.0, h * 0.48, 0.0), &vgrad(u.py(y0), u.py(y0 + h * 0.48), &[(0.0, rgba(255, 255, 255, 210)), (1.0, rgba(255, 255, 255, 25))]), p.clone());
    pm.stroke(p, &solid(darken(base, 0.55)), u.lw(0.008));
}

fn rect(u: U, x: f32, y: f32, w: f32, h: f32) -> Option<Rect> {
    Rect::from_xywh(u.px(x), u.py(y), u.d(w), u.d(h))
}

/// Placa de alumínio escovado com espessura (fundo de vários painéis).
fn alu_plate(pm: &mut Canvas, u: U, x: f32, y: f32, w: f32, h: f32, r: f32, seed: u32) -> Option<Path> {
    let p = u.rr(x, y, w, h, r);
    soft_shadow(pm, p.clone(), u.s, 1.0);
    pm.fill(u.rr(x, y + 0.025, w, h, r), &solid(rgba(110, 112, 120, 255)));
    pm.fill(p.clone(), &metal_paint(u.py(y), u.py(y + h)));
    brush_streaks(pm, p.clone(), u, seed, false);
    pm.stroke(p.clone(), &solid(rgba(96, 98, 106, 255)), u.lw(0.008));
    p
}

/// Mostrar Todos: placa de alumínio com nove ladrilhos de gel coloridos.
pub(crate) fn show_all(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    alu_plate(pm, u, 0.08, 0.08, 0.84, 0.82, 0.08, 3);
    let cols = [rgba(70, 140, 235, 255), rgba(110, 190, 80, 255), rgba(160, 110, 220, 255), rgba(230, 80, 80, 255), rgba(150, 154, 166, 255), rgba(70, 140, 235, 255), rgba(240, 170, 50, 255), rgba(60, 180, 170, 255), rgba(230, 110, 170, 255)];
    for (i, c) in cols.iter().enumerate() {
        let (cx, cy) = (0.17 + (i % 3) as f32 * 0.235, 0.16 + (i / 3) as f32 * 0.235);
        let t = u.rr(cx, cy, 0.19, 0.19, 0.04);
        gel(pm, u, t, *c, cy, cy + 0.19);
    }
}

/// Aparência: paleta de madeira envernizada com tintas de gel brilhantes e um pincel.
pub(crate) fn appearance(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(0.5), u.py(0.12));
    pb.cubic_to(u.px(0.86), u.py(0.1), u.px(0.98), u.py(0.42), u.px(0.86), u.py(0.62));
    pb.cubic_to(u.px(0.76), u.py(0.8), u.px(0.6), u.py(0.66), u.px(0.56), u.py(0.78));
    pb.cubic_to(u.px(0.52), u.py(0.92), u.px(0.2), u.py(0.92), u.px(0.1), u.py(0.66));
    pb.cubic_to(u.px(0.0), u.py(0.4), u.px(0.2), u.py(0.13), u.px(0.5), u.py(0.12));
    pb.close();
    pb.push_oval(Rect::from_xywh(u.px(0.6), u.py(0.48), u.d(0.12), u.d(0.09)).unwrap());
    let palette = pb.finish();
    soft_shadow(pm, palette.clone(), s, 1.0);
    if let Some(side) = palette.clone().and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.03)))) {
        pm.fill_rule(Some(side), &solid(rgba(120, 70, 30, 255)), tiny_skia::FillRule::EvenOdd);
    }
    pm.fill_rule(palette.clone(), &linear(u.px(0.1), u.py(0.1), u.px(0.9), u.py(0.9), &[(0.0, rgba(236, 196, 140, 255)), (0.5, rgba(206, 156, 96, 255)), (1.0, rgba(170, 116, 60, 255))]), tiny_skia::FillRule::EvenOdd);
    // Veios da madeira
    if s >= 32.0 {
        let mut grain = PathBuilder::new();
        for k in 0..7 {
            let yy = 0.2 + k as f32 * 0.1;
            grain.move_to(u.px(0.05), u.py(yy));
            grain.cubic_to(u.px(0.35), u.py(yy - 0.04), u.px(0.6), u.py(yy + 0.05), u.px(0.95), u.py(yy - 0.02));
        }
        let g = grain.finish().and_then(|g| g.stroke(&tiny_skia::Stroke { width: u.lw(0.006), ..Default::default() }, 1.0));
        pm.fill_clipped(g, &solid(rgba(140, 90, 40, 70)), palette.clone());
    }
    glass_glare(pm, palette.clone(), u.px(0.0), u.py(0.1), u.px(1.0), u.py(0.45), 90);
    pm.stroke(palette, &solid(rgba(110, 66, 26, 255)), u.lw(0.008));
    for (cx, cy, c) in [(0.3, 0.3, rgba(40, 110, 230, 255)), (0.52, 0.24, rgba(150, 154, 166, 255)), (0.72, 0.32, rgba(220, 40, 40, 255)), (0.24, 0.56, rgba(250, 190, 30, 255)), (0.4, 0.72, rgba(70, 180, 70, 255))] {
        let b = u.circ(cx, cy, 0.075);
        gel(pm, u, b, c, cy - 0.075, cy + 0.075);
    }
}

/// Dock: prateleira de vidro em perspectiva com ícones de gel e o reflexo deles.
pub(crate) fn dock(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let back = u.rr(0.05, 0.1, 0.9, 0.78, 0.06);
    soft_shadow(pm, back.clone(), s, 1.0);
    pm.fill(back.clone(), &linear(u.px(0.05), u.py(0.1), u.px(0.95), u.py(0.88), &[(0.0, rgba(90, 160, 240, 255)), (0.6, rgba(30, 80, 180, 255)), (1.0, rgba(14, 40, 110, 255))]));
    let shelf = u.poly(&[(0.16, 0.6), (0.84, 0.6), (0.95, 0.8), (0.05, 0.8)]);
    pm.fill_clipped(shelf.clone(), &vgrad(u.py(0.6), u.py(0.8), &[(0.0, rgba(255, 255, 255, 170)), (1.0, rgba(210, 225, 245, 120))]), back.clone());
    pm.stroke(shelf, &solid(rgba(255, 255, 255, 200)), u.lw(0.006));
    let cols = [rgba(70, 150, 240, 255), rgba(110, 200, 80, 255), rgba(250, 170, 40, 255), rgba(160, 164, 176, 255)];
    for (i, c) in cols.iter().enumerate() {
        let ix = 0.18 + i as f32 * 0.165;
        let t = u.rr(ix, 0.44, 0.14, 0.14, 0.03);
        soft_shadow(pm, t.clone(), s * 0.5, 0.8);
        gel(pm, u, t, *c, 0.44, 0.58);
        // Reflexo no vidro
        pm.fill(u.rr(ix, 0.62, 0.14, 0.06, 0.02), &vgrad(u.py(0.62), u.py(0.68), &[(0.0, with_alpha(*c, 0.45)), (1.0, with_alpha(*c, 0.0))]));
    }
    glass_glare(pm, back.clone(), u.px(0.05), u.py(0.1), u.px(0.95), u.py(0.38), 120);
    pm.stroke(back, &solid(rgba(14, 36, 90, 255)), u.lw(0.008));
}

/// Teclado de alumínio em perspectiva com teclas brancas; `hot` acende uma tecla
/// azul e põe o raio (Atalhos).
pub(crate) fn keyboard(pm: &mut Canvas, x: f32, y: f32, s: f32, hot: bool) {
    let u = U { x, y, s };
    let body = u.poly(&[(0.14, 0.32), (0.86, 0.32), (0.97, 0.78), (0.03, 0.78)]);
    soft_shadow(pm, body.clone(), s, 1.0);
    pm.fill(u.poly(&[(0.03, 0.78), (0.97, 0.78), (0.97, 0.83), (0.03, 0.83)]), &vgrad(u.py(0.78), u.py(0.83), &[(0.0, rgba(150, 152, 160, 255)), (1.0, rgba(100, 102, 110, 255))]));
    pm.fill(body.clone(), &metal_paint(u.py(0.32), u.py(0.78)));
    brush_streaks(pm, body.clone(), u, 31, false);
    pm.stroke(body, &solid(rgba(96, 98, 106, 255)), u.lw(0.007));
    // Teclas: fileiras em perspectiva (mais largas embaixo).
    let rows = 4;
    for r in 0..rows {
        let t0 = r as f32 / rows as f32;
        let t1 = (r as f32 + 0.8) / rows as f32;
        let ya = 0.36 + t0 * 0.38;
        let yb = 0.36 + t1 * 0.38;
        let xl = |t: f32| 0.17 - t * 0.11;
        let xr = |t: f32| 0.83 + t * 0.11;
        let n = if r == 3 { 5 } else { 10 };
        for k in 0..n {
            let f0 = k as f32 / n as f32;
            let f1 = (k as f32 + 0.84) / n as f32;
            let at = |f: f32, t: f32, yy: f32| (xl(t) + (xr(t) - xl(t)) * f, yy);
            let wide = r == 3 && k == 2;
            let (fa, fb) = if wide { (0.3, 0.7) } else { (f0, f1) };
            if r == 3 && (k == 1 || k == 3) {
                continue;
            }
            let key = u.poly(&[at(fa, t0, ya), at(fb, t0, ya), at(fb, t1, yb), at(fa, t1, yb)]);
            let lit = hot && r == 1 && k == 3;
            let (top, bot) = if lit { (rgba(150, 210, 255, 255), rgba(30, 110, 230, 255)) } else { (rgba(255, 255, 255, 255), rgba(214, 216, 224, 255)) };
            pm.fill(key.clone().and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.012)))), &solid(rgba(110, 112, 122, 255)));
            pm.fill(key.clone(), &vgrad(u.py(ya), u.py(yb), &[(0.0, top), (1.0, bot)]));
            pm.stroke(key, &solid(rgba(140, 142, 152, 255)), u.lw(0.003));
        }
    }
    if hot {
        let bolt = u.poly(&[(0.7, 0.02), (0.56, 0.2), (0.66, 0.2), (0.58, 0.34), (0.8, 0.13), (0.69, 0.13), (0.78, 0.02)]);
        soft_shadow(pm, bolt.clone(), s, 0.7);
        gel(pm, u, bolt, rgba(250, 190, 20, 255), 0.02, 0.34);
    }
}

/// Mesa e Protetor de Tela: moldura prateada com uma onda azul do Aqua, em pé.
pub(crate) fn desktop(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    pm.fill(u.poly(&[(0.6, 0.6), (0.7, 0.6), (0.82, 0.92), (0.76, 0.92)]), &solid(rgba(120, 122, 130, 255)));
    let frame = u.rr(0.06, 0.12, 0.88, 0.7, 0.03);
    soft_shadow(pm, frame.clone(), s, 1.0);
    pm.fill(frame.clone(), &linear(u.px(0.06), u.py(0.12), u.px(0.94), u.py(0.82), &[(0.0, rgba(250, 251, 253, 255)), (0.45, rgba(170, 174, 184, 255)), (0.55, rgba(210, 214, 222, 255)), (1.0, rgba(130, 134, 144, 255))]));
    pm.stroke(frame, &solid(rgba(90, 92, 100, 255)), u.lw(0.008));
    let pic = u.rr(0.12, 0.18, 0.76, 0.58, 0.0);
    pm.fill(pic.clone(), &linear(u.px(0.12), u.py(0.18), u.px(0.88), u.py(0.76), &[(0.0, rgba(140, 200, 255, 255)), (0.5, rgba(40, 110, 220, 255)), (1.0, rgba(14, 50, 140, 255))]));
    for (k, a) in [(0.0, 90u8), (0.08, 60), (0.16, 40)] {
        let mut w = PathBuilder::new();
        w.move_to(u.px(0.12), u.py(0.56 + k));
        w.cubic_to(u.px(0.35), u.py(0.36 + k), u.px(0.6), u.py(0.72 + k), u.px(0.88), u.py(0.46 + k));
        w.line_to(u.px(0.88), u.py(0.76));
        w.line_to(u.px(0.12), u.py(0.76));
        w.close();
        pm.fill_clipped(w.finish(), &solid(rgba(255, 255, 255, a)), pic.clone());
    }
    glass_glare(pm, pic.clone(), u.px(0.12), u.py(0.18), u.px(0.88), u.py(0.4), 90);
    pm.stroke(pic, &solid(rgba(60, 62, 70, 255)), u.lw(0.006));
}

/// Som: caixa de som de alumínio com o cone cromado, o tweeter e as ondas.
pub(crate) fn sound(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    alu_plate(pm, u, 0.16, 0.06, 0.5, 0.86, 0.06, 41);
    for (cy, r) in [(0.24, 0.08), (0.6, 0.18)] {
        pm.fill(u.circ(0.41, cy, r + 0.025), &linear(u.px(0.41 - r), u.py(cy - r), u.px(0.41 + r), u.py(cy + r), &[(0.0, rgba(110, 112, 120, 255)), (1.0, rgba(250, 250, 252, 255))]));
        let cone = u.circ(0.41, cy, r);
        pm.fill(cone.clone(), &radial(u.px(0.41), u.py(cy), u.d(r), &[(0.0, rgba(30, 30, 36, 255)), (0.5, rgba(70, 72, 80, 255)), (1.0, rgba(24, 24, 28, 255))]));
        spun_metal(pm, cone.clone(), u, 0.41, cy, r, 7);
        pm.fill(u.circ(0.41, cy, r * 0.32), &radial(u.px(0.39), u.py(cy - r * 0.1), u.d(r * 0.36), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(100, 104, 114, 255))]));
    }
    for (k, a) in [(0.0, 230u8), (1.0, 170), (2.0, 110)] {
        let r = 0.14 + k * 0.08;
        let mut w = PathBuilder::new();
        w.move_to(u.px(0.66 + r * (-0.5f32).cos()), u.py(0.5 + r * (-0.5f32).sin()));
        for i in 1..=10 {
            let a2 = -0.5 + i as f32 / 10.0;
            w.line_to(u.px(0.66 + r * a2.cos()), u.py(0.5 + r * a2.sin()));
        }
        pm.stroke_round(w.finish(), &solid(rgba(50, 120, 230, a)), u.lw(0.028));
    }
}

/// Economizador de Energia: lâmpada de vidro com o filamento aceso e a rosca cromada.
pub(crate) fn energy(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    pm.fill(u.circ(0.5, 0.38, 0.44), &radial(u.px(0.5), u.py(0.38), u.d(0.44), &[(0.0, rgba(255, 240, 160, 150)), (1.0, rgba(255, 240, 160, 0))]));
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(0.38), u.py(0.68));
    pb.cubic_to(u.px(0.36), u.py(0.56), u.px(0.18), u.py(0.5), u.px(0.18), u.py(0.36));
    pb.cubic_to(u.px(0.18), u.py(0.18), u.px(0.32), u.py(0.06), u.px(0.5), u.py(0.06));
    pb.cubic_to(u.px(0.68), u.py(0.06), u.px(0.82), u.py(0.18), u.px(0.82), u.py(0.36));
    pb.cubic_to(u.px(0.82), u.py(0.5), u.px(0.64), u.py(0.56), u.px(0.62), u.py(0.68));
    pb.close();
    let bulb = pb.finish();
    soft_shadow(pm, bulb.clone(), s, 0.6);
    pm.fill(bulb.clone(), &radial(u.px(0.5), u.py(0.4), u.d(0.4), &[(0.0, rgba(255, 252, 220, 255)), (0.6, rgba(255, 236, 140, 240)), (1.0, rgba(240, 190, 60, 230))]));
    let mut fil = PathBuilder::new();
    fil.move_to(u.px(0.42), u.py(0.66));
    fil.line_to(u.px(0.42), u.py(0.42));
    fil.line_to(u.px(0.46), u.py(0.36));
    fil.line_to(u.px(0.5), u.py(0.42));
    fil.line_to(u.px(0.54), u.py(0.36));
    fil.line_to(u.px(0.58), u.py(0.42));
    fil.line_to(u.px(0.58), u.py(0.66));
    let f = fil.finish();
    pm.stroke(f.clone(), &solid(rgba(255, 140, 20, 120)), u.lw(0.04));
    pm.stroke(f, &solid(rgba(200, 90, 10, 255)), u.lw(0.012));
    pm.fill_clipped(u.ov(0.24, 0.1, 0.3, 0.3), &radial(u.px(0.36), u.py(0.22), u.d(0.16), &[(0.0, rgba(255, 255, 255, 230)), (1.0, rgba(255, 255, 255, 0))]), bulb.clone());
    pm.stroke(bulb, &solid(rgba(200, 150, 40, 255)), u.lw(0.007));
    for k in 0..4 {
        let yy = 0.68 + k as f32 * 0.055;
        let w = if k == 3 { 0.16 } else { 0.24 };
        let r = u.rr(0.5 - w / 2.0, yy, w, 0.05, 0.02);
        pm.fill(r.clone(), &linear(u.px(0.38), 0.0, u.px(0.62), 0.0, &[(0.0, rgba(120, 124, 134, 255)), (0.35, rgba(250, 250, 252, 255)), (1.0, rgba(110, 114, 124, 255))]));
        pm.stroke(r, &solid(rgba(80, 82, 92, 255)), u.lw(0.004));
    }
    pm.fill(u.ov(0.45, 0.9, 0.1, 0.05), &solid(rgba(40, 40, 46, 255)));
}

/// Globo de vidro azul em grade de meridianos (Rede, Utilitário de Rede, Servidor).
fn wire_globe(pm: &mut Canvas, u: U, cx: f32, cy: f32, r: f32) {
    let g = u.circ(cx, cy, r);
    soft_shadow(pm, g.clone(), u.s, 0.9);
    pm.fill(g.clone(), &radial(u.px(cx - r * 0.3), u.py(cy - r * 0.35), u.d(r * 1.5), &[(0.0, rgba(150, 215, 255, 255)), (0.5, rgba(40, 120, 225, 255)), (1.0, rgba(8, 40, 120, 255))]));
    if u.s >= 24.0 {
        let mut grid = PathBuilder::new();
        for k in [0.35, 0.7] {
            if let Some(rc) = Rect::from_xywh(u.px(cx - r * k), u.py(cy - r), u.d(r * 2.0 * k), u.d(r * 2.0)) {
                grid.push_oval(rc);
            }
        }
        grid.move_to(u.px(cx), u.py(cy - r));
        grid.line_to(u.px(cx), u.py(cy + r));
        for k in [-0.5f32, 0.0, 0.5] {
            let hw = (1.0 - k * k).sqrt() * r;
            grid.move_to(u.px(cx - hw), u.py(cy + k * r));
            grid.line_to(u.px(cx + hw), u.py(cy + k * r));
        }
        let gp = grid.finish().and_then(|p| p.stroke(&tiny_skia::Stroke { width: u.lw(0.01), ..Default::default() }, 1.0));
        pm.fill_clipped(gp, &solid(rgba(210, 236, 255, 170)), g.clone());
    }
    glass_glare(pm, g.clone(), u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy), 160);
    pm.stroke(g, &solid(rgba(8, 36, 100, 255)), u.lw(0.008));
}

/// Rede: globo em grade com o cabo de rede de plástico na frente.
pub(crate) fn network(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    wire_globe(pm, u, 0.48, 0.44, 0.38);
    rj45(pm, u, 0.68, 0.74, -30.0);
}

/// Conector de rede (RJ45) de plástico transparente com o cabo azul.
fn rj45(pm: &mut Canvas, u: U, cx: f32, cy: f32, ang: f32) {
    let t = Transform::from_rotate_at(ang, u.px(cx), u.py(cy));
    let mut cable = PathBuilder::new();
    cable.move_to(u.px(cx), u.py(cy + 0.08));
    cable.cubic_to(u.px(cx), u.py(cy + 0.2), u.px(cx + 0.2), u.py(cy + 0.14), u.px(cx + 0.24), u.py(cy + 0.26));
    pm.stroke_round(tf(cable.finish(), t), &solid(rgba(30, 80, 180, 255)), u.lw(0.06));
    let body = tf(u.rr(cx - 0.09, cy - 0.12, 0.18, 0.2, 0.02), t);
    soft_shadow(pm, body.clone(), u.s, 0.7);
    pm.fill(body.clone(), &tpaint(linear(u.px(cx - 0.09), 0.0, u.px(cx + 0.09), 0.0, &[(0.0, rgba(200, 220, 240, 230)), (0.5, rgba(250, 252, 255, 240)), (1.0, rgba(170, 190, 215, 230))]), t));
    // Contatos dourados
    let mut pins = PathBuilder::new();
    for k in 0..5 {
        if let Some(r) = rect(u, cx - 0.07 + k as f32 * 0.03, cy - 0.12, 0.015, 0.06) {
            pins.push_rect(r);
        }
    }
    pm.fill(tf(pins.finish(), t), &solid(rgba(220, 170, 50, 255)));
    pm.stroke(body, &solid(rgba(110, 130, 160, 255)), u.lw(0.006));
}

/// Data e Hora: folha da Agenda atrás e o relógio cromado na frente.
pub(crate) fn date_time(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let (m, d) = unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_mon as u32 + 1, tm.tm_mday as u32)
    };
    crate::tiger::calendar(pm, fonts, x, y, s * 0.72, m, d);
    crate::tiger::clock(pm, fonts, x + s * 0.36, y + s * 0.36, s * 0.64, 10, 9);
}

/// Itens de Início: folha com caixas marcadas e o botão de gel verde de tocar.
pub(crate) fn login_items(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    sheet(pm, U { x: x - s * 0.06, y, s });
    let cols = [rgba(70, 140, 235, 255), rgba(240, 170, 50, 255), rgba(110, 190, 80, 255), rgba(160, 110, 220, 255)];
    for (k, c) in cols.iter().enumerate() {
        let yy = 0.24 + k as f32 * 0.14;
        let b = u.rr(0.2, yy, 0.09, 0.09, 0.02);
        gel(pm, u, b, *c, yy, yy + 0.09);
        pm.fill(u.rr(0.34, yy + 0.03, 0.3, 0.03, 0.01), &solid(rgba(120, 124, 136, 220)));
    }
    let b = u.circ(0.7, 0.74, 0.17);
    soft_shadow(pm, b.clone(), s, 0.9);
    gel(pm, u, b, rgba(60, 170, 60, 255), 0.57, 0.91);
    pm.fill(u.poly(&[(0.65, 0.66), (0.79, 0.74), (0.65, 0.82)]), &solid(Color::WHITE));
}

/// Acesso Universal: botão de gel azul com a figura de braços abertos em relevo.
pub(crate) fn access(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let b = u.circ(0.5, 0.48, 0.4);
    soft_shadow(pm, b.clone(), s, 1.0);
    pm.fill(u.circ(0.5, 0.48, 0.44), &linear(u.px(0.1), u.py(0.1), u.px(0.9), u.py(0.9), &[(0.0, rgba(255, 255, 255, 255)), (0.5, rgba(150, 154, 166, 255)), (1.0, rgba(230, 232, 238, 255))]));
    gel(pm, u, b, rgba(30, 100, 220, 255), 0.08, 0.88);
    let figure = |pm: &mut Canvas, dy: f32, c: Color| {
        pm.fill(u.circ(0.5, 0.25 + dy, 0.06), &solid(c));
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(0.27), u.py(0.38 + dy));
        pb.line_to(u.px(0.73), u.py(0.38 + dy));
        pb.move_to(u.px(0.5), u.py(0.38 + dy));
        pb.line_to(u.px(0.5), u.py(0.56 + dy));
        pb.line_to(u.px(0.38), u.py(0.74 + dy));
        pb.move_to(u.px(0.5), u.py(0.56 + dy));
        pb.line_to(u.px(0.62), u.py(0.74 + dy));
        pm.stroke_round(pb.finish(), &solid(c), u.lw(0.06));
    };
    figure(pm, 0.012, rgba(0, 30, 100, 120));
    figure(pm, 0.0, Color::WHITE);
}

/// Pessoa de gel (Contas): cabeça e ombros arredondados, brilhantes.
fn person(pm: &mut Canvas, u: U, cx: f32, cy: f32, k: f32, base: Color) {
    let body = u.rr(cx - 0.2 * k, cy + 0.12 * k, 0.4 * k, 0.36 * k, 0.16 * k);
    soft_shadow(pm, body.clone(), u.s, 0.7);
    gel(pm, u, body, base, cy + 0.12 * k, cy + 0.48 * k);
    let head = u.circ(cx, cy, 0.12 * k);
    soft_shadow(pm, head.clone(), u.s, 0.6);
    pm.fill(head.clone(), &radial(u.px(cx - 0.04 * k), u.py(cy - 0.04 * k), u.d(0.16 * k), &[(0.0, rgba(255, 230, 205, 255)), (0.7, rgba(236, 180, 140, 255)), (1.0, rgba(190, 130, 96, 255))]));
    pm.stroke(head, &solid(rgba(150, 96, 66, 255)), u.lw(0.006));
}

/// Contas: duas pessoas de gel, a da frente azul e a de trás cinza.
pub(crate) fn accounts(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    person(pm, u, 0.64, 0.2, 0.9, rgba(130, 136, 150, 255));
    person(pm, u, 0.4, 0.34, 1.05, rgba(40, 110, 220, 255));
}

/// Internacional: balão de fala de gel azul com letras de várias línguas.
pub(crate) fn international(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let mut pb = PathBuilder::new();
    let r = u.d(0.1);
    let (x0, y0, x1, y1) = (u.px(0.06), u.py(0.12), u.px(0.94), u.py(0.7));
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(u.px(0.42), y1);
    pb.line_to(u.px(0.24), u.py(0.9));
    pb.line_to(u.px(0.28), y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    let bubble = pb.finish();
    soft_shadow(pm, bubble.clone(), s, 1.0);
    gel(pm, u, bubble, rgba(40, 120, 225, 255), 0.12, 0.7);
    if s >= 20.0 {
        let px = u.d(0.24);
        let t = "aé ñ";
        let tw = fonts.measure(t, px, true);
        fonts.draw(pm, t, u.px(0.5) - tw / 2.0 + u.d(0.01), u.py(0.51) + u.d(0.01), px, true, rgba(0, 30, 90, 120));
        fonts.draw(pm, t, u.px(0.5) - tw / 2.0, u.py(0.51), px, true, Color::WHITE);
    }
}

/// CDs e DVDs: o disco com o botão de tocar de gel.
pub(crate) fn cds(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::optical(pm, x, y, s * 0.86);
    let b = u.circ(0.74, 0.72, 0.17);
    soft_shadow(pm, b.clone(), s, 0.9);
    gel(pm, u, b, rgba(40, 120, 225, 255), 0.55, 0.89);
    pm.fill(u.poly(&[(0.69, 0.64), (0.83, 0.72), (0.69, 0.8)]), &solid(Color::WHITE));
}

/// Sem Fio (Bluetooth e AirPort): fones de ouvido cromados com as ondas.
pub(crate) fn wireless(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let mut arc = PathBuilder::new();
    arc.move_to(u.px(0.18), u.py(0.6));
    arc.cubic_to(u.px(0.16), u.py(0.08), u.px(0.84), u.py(0.08), u.px(0.82), u.py(0.6));
    let a = arc.finish();
    pm.stroke_round(a.clone(), &solid(rgba(50, 52, 60, 255)), u.lw(0.07));
    pm.stroke_round(a, &linear(u.px(0.2), u.py(0.1), u.px(0.8), u.py(0.6), &[(0.0, rgba(250, 250, 252, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(220, 222, 228, 255))]), u.lw(0.045));
    for cx in [0.18, 0.82] {
        let cup = u.rr(cx - 0.1, 0.5, 0.2, 0.32, 0.09);
        soft_shadow(pm, cup.clone(), s, 0.9);
        pm.fill(cup.clone(), &linear(u.px(cx - 0.1), 0.0, u.px(cx + 0.1), 0.0, &[(0.0, rgba(60, 62, 70, 255)), (0.4, rgba(150, 154, 164, 255)), (1.0, rgba(40, 42, 50, 255))]));
        glass_glare(pm, cup.clone(), u.px(cx - 0.1), u.py(0.5), u.px(cx + 0.1), u.py(0.62), 120);
        pm.stroke(cup, &solid(rgba(20, 20, 26, 255)), u.lw(0.007));
    }
    // Ondas de rádio saindo para os dois lados, entre as conchas.
    for (k, al) in [(0.0, 230u8), (1.0, 150)] {
        let r = 0.07 + k * 0.07;
        for (a0, a1) in [(-0.7f32, 0.7f32), (PI - 0.7, PI + 0.7)] {
            let mut w = PathBuilder::new();
            for i in 0..=8 {
                let a2 = a0 + i as f32 / 8.0 * (a1 - a0);
                let (px, py) = (u.px(0.5 + r * a2.cos()), u.py(0.6 + r * a2.sin()));
                if i == 0 { w.move_to(px, py) } else { w.line_to(px, py) }
            }
            pm.stroke_round(w.finish(), &solid(rgba(40, 120, 230, al)), u.lw(0.025));
        }
    }
}

/// Compartilhamento: a pasta azul com a placa amarela de pessoas.
pub(crate) fn sharing(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::folder(pm, x, y, s * 0.9, rgba(88, 150, 226, 255));
    let sign = u.rr(0.5, 0.52, 0.44, 0.36, 0.05);
    soft_shadow(pm, sign.clone(), s, 0.9);
    gel(pm, u, sign, rgba(250, 196, 40, 255), 0.52, 0.88);
    for (cx, k) in [(0.64, 1.0), (0.8, 1.0)] {
        pm.fill(u.circ(cx, 0.63, 0.04 * k), &solid(rgba(110, 70, 0, 230)));
        pm.fill(u.rr(cx - 0.065, 0.69, 0.13, 0.12, 0.05), &solid(rgba(110, 70, 0, 230)));
    }
}

/// Janela de app no estilo Aqua: barra de metal com os botões e o conteúdo branco.
fn mini_window(pm: &mut Canvas, u: U, x: f32, y: f32, w: f32, h: f32, tint: Color) {
    let win = u.rr(x, y, w, h, 0.02);
    soft_shadow(pm, win.clone(), u.s * 0.7, 0.9);
    pm.fill(win.clone(), &solid(rgba(250, 250, 252, 255)));
    let bar = u.rr(x, y, w, h * 0.2, 0.02);
    pm.fill_clipped(bar, &vgrad(u.py(y), u.py(y + h * 0.2), &[(0.0, rgba(236, 236, 240, 255)), (1.0, rgba(180, 182, 190, 255))]), win.clone());
    pm.fill_clipped(u.rr(x, y + h * 0.2, w, h * 0.8, 0.0), &vgrad(u.py(y + h * 0.2), u.py(y + h), &[(0.0, with_alpha(tint, 0.25)), (1.0, with_alpha(tint, 0.05))]), win.clone());
    for (k, c) in [rgba(236, 80, 70, 255), rgba(240, 190, 40, 255), rgba(90, 190, 70, 255)].iter().enumerate() {
        let cx = x + 0.04 + k as f32 * h * 0.12;
        let b = u.circ(cx, y + h * 0.1, h * 0.045);
        pm.fill(b, &solid(*c));
    }
    pm.stroke(win, &solid(rgba(110, 112, 122, 255)), u.lw(0.006));
}

/// Apps Padrão: janelas empilhadas e o selo de gel verde com a marca de certo.
pub(crate) fn default_apps(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    mini_window(pm, u, 0.06, 0.06, 0.6, 0.5, rgba(160, 110, 220, 255));
    mini_window(pm, u, 0.18, 0.2, 0.6, 0.5, rgba(240, 170, 50, 255));
    mini_window(pm, u, 0.3, 0.34, 0.6, 0.5, rgba(70, 140, 235, 255));
    let b = u.circ(0.74, 0.76, 0.17);
    soft_shadow(pm, b.clone(), s, 0.9);
    gel(pm, u, b, rgba(60, 170, 60, 255), 0.59, 0.93);
    let mut ck = PathBuilder::new();
    ck.move_to(u.px(0.66), u.py(0.76));
    ck.line_to(u.px(0.72), u.py(0.83));
    ck.line_to(u.px(0.83), u.py(0.68));
    pm.stroke_round(ck.finish(), &solid(Color::WHITE), u.lw(0.035));
}

/// Impressão e Fax: impressora branca com a folha saindo e a luz verde.
pub(crate) fn printer(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    // Folha atrás
    let paper = u.rr(0.26, 0.06, 0.48, 0.36, 0.0);
    pm.fill(paper.clone(), &vgrad(u.py(0.06), u.py(0.42), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(236, 236, 240, 255))]));
    for k in 0..4 {
        pm.fill(u.rr(0.32, 0.12 + k as f32 * 0.05, if k == 3 { 0.2 } else { 0.36 }, 0.02, 0.0), &solid(rgba(130, 134, 146, 200)));
    }
    pm.stroke(paper, &solid(rgba(150, 152, 160, 255)), u.lw(0.005));
    let top = u.poly(&[(0.12, 0.42), (0.88, 0.42), (0.94, 0.5), (0.06, 0.5)]);
    let body = u.rr(0.06, 0.5, 0.88, 0.3, 0.05);
    soft_shadow(pm, u.poly(&[(0.12, 0.42), (0.88, 0.42), (0.94, 0.5), (0.94, 0.8), (0.06, 0.8), (0.06, 0.5)]), s, 1.0);
    pm.fill(top.clone(), &vgrad(u.py(0.42), u.py(0.5), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(214, 216, 222, 255))]));
    pm.stroke(top, &solid(rgba(130, 132, 140, 255)), u.lw(0.006));
    pm.fill(body.clone(), &vgrad(u.py(0.5), u.py(0.8), &[(0.0, rgba(246, 246, 248, 255)), (0.6, rgba(214, 216, 224, 255)), (1.0, rgba(160, 162, 172, 255))]));
    glass_glare(pm, body.clone(), u.px(0.06), u.py(0.5), u.px(0.94), u.py(0.6), 120);
    pm.stroke(body, &solid(rgba(120, 122, 132, 255)), u.lw(0.007));
    // Saída do papel e a bandeja
    pm.fill(u.rr(0.18, 0.62, 0.64, 0.06, 0.02), &vgrad(u.py(0.62), u.py(0.68), &[(0.0, rgba(40, 42, 50, 255)), (1.0, rgba(90, 92, 100, 255))]));
    let tray = u.poly(&[(0.2, 0.8), (0.8, 0.8), (0.86, 0.9), (0.14, 0.9)]);
    pm.fill(tray.clone(), &vgrad(u.py(0.8), u.py(0.9), &[(0.0, rgba(170, 172, 180, 255)), (1.0, rgba(220, 222, 228, 255))]));
    pm.stroke(tray, &solid(rgba(120, 122, 132, 255)), u.lw(0.005));
    pm.fill(u.circ(0.84, 0.56, 0.02), &radial(u.px(0.84), u.py(0.56), u.d(0.03), &[(0.0, rgba(190, 255, 190, 255)), (1.0, rgba(40, 170, 60, 255))]));
}

/// Caixa de papelão com fita (Atualização de Software, Utilitário de Arquivo).
fn cardboard_box(pm: &mut Canvas, u: U) {
    let front = u.poly(&[(0.1, 0.42), (0.62, 0.5), (0.62, 0.92), (0.1, 0.82)]);
    let side = u.poly(&[(0.62, 0.5), (0.92, 0.38), (0.92, 0.78), (0.62, 0.92)]);
    let top = u.poly(&[(0.1, 0.42), (0.42, 0.3), (0.92, 0.38), (0.62, 0.5)]);
    soft_shadow(pm, u.poly(&[(0.1, 0.42), (0.42, 0.3), (0.92, 0.38), (0.92, 0.78), (0.62, 0.92), (0.1, 0.82)]), u.s, 1.0);
    pm.fill(front.clone(), &vgrad(u.py(0.42), u.py(0.92), &[(0.0, rgba(214, 170, 110, 255)), (1.0, rgba(176, 128, 70, 255))]));
    pm.fill(side.clone(), &vgrad(u.py(0.38), u.py(0.92), &[(0.0, rgba(180, 136, 80, 255)), (1.0, rgba(140, 98, 50, 255))]));
    pm.fill(top.clone(), &vgrad(u.py(0.3), u.py(0.5), &[(0.0, rgba(236, 200, 140, 255)), (1.0, rgba(214, 172, 110, 255))]));
    for p in [front.clone(), side, top] {
        leather_grain(pm, p.clone(), u, 13);
        pm.stroke(p, &solid(rgba(120, 80, 40, 255)), u.lw(0.006));
    }
    // Fita adesiva no meio do topo, descendo pela frente
    let tape = u.poly(&[(0.31, 0.36), (0.38, 0.34), (0.84, 0.42), (0.77, 0.45)]);
    pm.fill(tape, &solid(rgba(240, 220, 170, 200)));
    let tape2 = u.poly(&[(0.33, 0.45), (0.4, 0.46), (0.4, 0.86), (0.33, 0.85)]);
    pm.fill_clipped(tape2, &solid(rgba(240, 220, 170, 200)), front);
}

/// Atualização de Software: caixa de papelão com as setas de gel verde girando.
pub(crate) fn update(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    cardboard_box(pm, u);
    let (cx, cy, r) = (0.5, 0.28, 0.2);
    for half in [0.0, PI] {
        let mut pb = PathBuilder::new();
        let n = 12;
        for i in 0..=n {
            let a = half + 0.3 + i as f32 / n as f32 * (PI - 0.9);
            let (px, py) = (u.px(cx + r * a.cos()), u.py(cy + r * 0.8 * a.sin()));
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        let arc = pb.finish();
        pm.stroke_round(arc.clone(), &solid(rgba(20, 90, 20, 255)), u.lw(0.07));
        pm.stroke_round(arc, &vgrad(u.py(cy - r), u.py(cy + r), &[(0.0, rgba(170, 240, 140, 255)), (1.0, rgba(50, 150, 50, 255))]), u.lw(0.05));
        let a = half + 0.3 + (PI - 0.9);
        let (ex, ey) = (cx + r * a.cos(), cy + r * 0.8 * a.sin());
        let (dx, dy) = (-a.sin(), a.cos() * 0.8);
        let head = u.poly(&[(ex + dx * 0.08, ey + dy * 0.08), (ex - dy * 0.07, ey + dx * 0.07), (ex + dy * 0.07, ey - dx * 0.07)]);
        pm.fill(head.clone(), &solid(rgba(60, 160, 60, 255)));
        pm.stroke(head, &solid(rgba(20, 90, 20, 255)), u.lw(0.006));
    }
}

/// Disco de Início: o HD com o botão de ligar de gel azul.
pub(crate) fn startup_disk(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::disk(pm, x, y, s);
    let b = u.circ(0.72, 0.3, 0.17);
    soft_shadow(pm, b.clone(), s, 0.9);
    gel(pm, u, b, rgba(40, 120, 225, 255), 0.13, 0.47);
    let mut pw = PathBuilder::new();
    let n = 12;
    for i in 0..=n {
        let a = -PI / 2.0 + 0.6 + i as f32 / n as f32 * (2.0 * PI - 1.2);
        let (px, py) = (u.px(0.72 + 0.08 * a.cos()), u.py(0.31 + 0.08 * a.sin()));
        if i == 0 { pw.move_to(px, py) } else { pw.line_to(px, py) }
    }
    pw.move_to(u.px(0.72), u.py(0.2));
    pw.line_to(u.px(0.72), u.py(0.3));
    pm.stroke_round(pw.finish(), &solid(Color::WHITE), u.lw(0.025));
}

/// Fala: microfone de estúdio cromado com a grade e o pé.
pub(crate) fn speech(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    pm.fill(u.ov(0.28, 0.86, 0.44, 0.08), &metal_paint(u.py(0.86), u.py(0.94)));
    pm.fill(u.rr(0.47, 0.64, 0.06, 0.24, 0.02), &linear(u.px(0.47), 0.0, u.px(0.53), 0.0, &[(0.0, rgba(110, 112, 120, 255)), (0.5, rgba(240, 242, 246, 255)), (1.0, rgba(100, 102, 110, 255))]));
    let mut yoke = PathBuilder::new();
    yoke.move_to(u.px(0.26), u.py(0.36));
    yoke.cubic_to(u.px(0.26), u.py(0.7), u.px(0.74), u.py(0.7), u.px(0.74), u.py(0.36));
    pm.stroke_round(yoke.finish(), &solid(rgba(90, 92, 100, 255)), u.lw(0.035));
    let head = u.rr(0.32, 0.06, 0.36, 0.56, 0.18);
    soft_shadow(pm, head.clone(), s, 1.0);
    pm.fill(head.clone(), &linear(u.px(0.32), 0.0, u.px(0.68), 0.0, &[(0.0, rgba(110, 114, 124, 255)), (0.3, rgba(250, 250, 252, 255)), (0.6, rgba(170, 174, 184, 255)), (1.0, rgba(90, 94, 104, 255))]));
    if s >= 28.0 {
        let mut mesh = PathBuilder::new();
        let mut yy = 0.1;
        while yy < 0.6 {
            mesh.move_to(u.px(0.3), u.py(yy));
            mesh.line_to(u.px(0.7), u.py(yy));
            yy += 0.035;
        }
        let mut xx = 0.34;
        while xx < 0.68 {
            mesh.move_to(u.px(xx), u.py(0.05));
            mesh.line_to(u.px(xx), u.py(0.62));
            xx += 0.035;
        }
        let m = mesh.finish().and_then(|p| p.stroke(&tiny_skia::Stroke { width: u.lw(0.006), ..Default::default() }, 1.0));
        pm.fill_clipped(m, &solid(rgba(40, 42, 50, 110)), head.clone());
    }
    pm.fill(u.rr(0.31, 0.38, 0.38, 0.05, 0.01), &linear(u.px(0.31), 0.0, u.px(0.69), 0.0, &[(0.0, rgba(90, 92, 100, 255)), (0.4, rgba(240, 242, 246, 255)), (1.0, rgba(80, 82, 90, 255))]));
    pm.stroke(head, &solid(rgba(70, 72, 82, 255)), u.lw(0.007));
}

/// Cadeado dourado com a alça cromada (Segurança).
pub(crate) fn security(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let mut sh = PathBuilder::new();
    sh.move_to(u.px(0.3), u.py(0.48));
    sh.line_to(u.px(0.3), u.py(0.3));
    sh.cubic_to(u.px(0.3), u.py(0.04), u.px(0.7), u.py(0.04), u.px(0.7), u.py(0.3));
    sh.line_to(u.px(0.7), u.py(0.48));
    let shackle = sh.finish();
    pm.stroke(shackle.clone(), &solid(rgba(70, 72, 82, 255)), u.lw(0.11));
    pm.stroke(shackle, &linear(u.px(0.3), u.py(0.1), u.px(0.7), u.py(0.48), &[(0.0, rgba(255, 255, 255, 255)), (0.4, rgba(160, 164, 174, 255)), (0.7, rgba(230, 232, 238, 255)), (1.0, rgba(120, 124, 134, 255))]), u.lw(0.085));
    let body = u.rr(0.16, 0.44, 0.68, 0.48, 0.07);
    soft_shadow(pm, body.clone(), s, 1.0);
    pm.fill(u.rr(0.16, 0.47, 0.68, 0.48, 0.07), &solid(rgba(140, 96, 20, 255)));
    pm.fill(body.clone(), &linear(u.px(0.16), u.py(0.44), u.px(0.84), u.py(0.92), &[(0.0, rgba(255, 236, 150, 255)), (0.35, rgba(232, 180, 60, 255)), (0.6, rgba(250, 214, 110, 255)), (1.0, rgba(180, 120, 30, 255))]));
    brush_streaks(pm, body.clone(), u, 17, false);
    glass_glare(pm, body.clone(), u.px(0.16), u.py(0.44), u.px(0.84), u.py(0.56), 140);
    pm.stroke(body, &solid(rgba(130, 86, 16, 255)), u.lw(0.008));
    pm.fill(u.circ(0.5, 0.62, 0.06), &solid(rgba(60, 40, 10, 255)));
    pm.fill(u.poly(&[(0.475, 0.64), (0.525, 0.64), (0.54, 0.8), (0.46, 0.8)]), &solid(rgba(60, 40, 10, 255)));
}

/// Exposé e Dashboard: painel de vidro azul com quatro janelas espalhadas.
pub(crate) fn expose(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let back = u.rr(0.05, 0.08, 0.9, 0.82, 0.06);
    soft_shadow(pm, back.clone(), s, 1.0);
    pm.fill(back.clone(), &linear(u.px(0.05), u.py(0.08), u.px(0.95), u.py(0.9), &[(0.0, rgba(90, 160, 240, 255)), (0.6, rgba(30, 80, 180, 255)), (1.0, rgba(14, 40, 110, 255))]));
    let tints = [rgba(70, 140, 235, 255), rgba(240, 170, 50, 255), rgba(110, 190, 80, 255), rgba(160, 110, 220, 255)];
    for (i, c) in tints.iter().enumerate() {
        let (wx, wy) = (0.11 + (i % 2) as f32 * 0.42, 0.15 + (i / 2) as f32 * 0.38);
        mini_window(pm, u, wx, wy, 0.36, 0.3, *c);
    }
    glass_glare(pm, back.clone(), u.px(0.05), u.py(0.08), u.px(0.95), u.py(0.3), 70);
    pm.stroke(back, &solid(rgba(14, 36, 90, 255)), u.lw(0.008));
}

/// Mouse de plástico branco translúcido de um botão, com o fio.
pub(crate) fn mouse(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let mut wire = PathBuilder::new();
    wire.move_to(u.px(0.5), u.py(0.2));
    wire.cubic_to(u.px(0.5), u.py(0.04), u.px(0.7), u.py(0.1), u.px(0.78), u.py(0.02));
    pm.stroke_round(wire.finish(), &solid(rgba(200, 202, 210, 255)), u.lw(0.03));
    let shell = u.rr(0.24, 0.16, 0.52, 0.76, 0.26);
    soft_shadow(pm, shell.clone(), s, 1.0);
    pm.fill(shell.clone(), &linear(u.px(0.24), 0.0, u.px(0.76), 0.0, &[(0.0, rgba(200, 204, 214, 240)), (0.4, rgba(252, 252, 254, 250)), (1.0, rgba(180, 184, 196, 240))]));
    // Miolo cinza visto pelo plástico
    pm.fill(u.rr(0.32, 0.36, 0.36, 0.48, 0.16), &vgrad(u.py(0.36), u.py(0.84), &[(0.0, rgba(140, 144, 156, 140)), (1.0, rgba(90, 94, 104, 160))]));
    pm.fill_clipped(u.rr(0.24, 0.16, 0.52, 0.26, 0.0), &vgrad(u.py(0.16), u.py(0.42), &[(0.0, rgba(255, 255, 255, 220)), (1.0, rgba(255, 255, 255, 30))]), shell.clone());
    pm.stroke(shell, &solid(rgba(130, 134, 146, 255)), u.lw(0.007));
}

/// Trackpad: placa de alumínio em perspectiva com a área lisa e o botão.
pub(crate) fn trackpad(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let slab = u.poly(&[(0.16, 0.16), (0.84, 0.16), (0.94, 0.84), (0.06, 0.84)]);
    soft_shadow(pm, slab.clone(), s, 1.0);
    pm.fill(u.poly(&[(0.06, 0.84), (0.94, 0.84), (0.94, 0.89), (0.06, 0.89)]), &solid(rgba(110, 112, 120, 255)));
    pm.fill(slab.clone(), &metal_paint(u.py(0.16), u.py(0.84)));
    brush_streaks(pm, slab.clone(), u, 59, false);
    pm.stroke(slab, &solid(rgba(96, 98, 106, 255)), u.lw(0.007));
    let pad = u.poly(&[(0.24, 0.22), (0.76, 0.22), (0.82, 0.62), (0.18, 0.62)]);
    pm.fill(pad.clone(), &vgrad(u.py(0.22), u.py(0.62), &[(0.0, rgba(200, 202, 210, 255)), (1.0, rgba(232, 234, 240, 255))]));
    pm.stroke(pad, &solid(rgba(120, 122, 132, 255)), u.lw(0.006));
    let btn = u.poly(&[(0.19, 0.67), (0.81, 0.67), (0.84, 0.78), (0.16, 0.78)]);
    pm.fill(btn.clone(), &vgrad(u.py(0.67), u.py(0.78), &[(0.0, rgba(236, 238, 244, 255)), (1.0, rgba(170, 174, 184, 255))]));
    pm.stroke(btn, &solid(rgba(110, 112, 122, 255)), u.lw(0.006));
}

/// Acesso às Chaves: argola cromada com duas chaves de latão.
pub(crate) fn keychain(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let ring = u.circ(0.34, 0.28, 0.17);
    soft_shadow(pm, ring.clone(), s, 0.6);
    pm.stroke(ring.clone(), &solid(rgba(70, 72, 82, 255)), u.lw(0.06));
    pm.stroke(ring, &linear(u.px(0.17), u.py(0.11), u.px(0.51), u.py(0.45), &[(0.0, rgba(255, 255, 255, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(230, 232, 238, 255))]), u.lw(0.04));
    let key = |pm: &mut Canvas, ang: f32, len: f32| {
        let t = Transform::from_rotate_at(ang, u.px(0.44), u.py(0.44));
        let brass = tpaint(vgrad(u.py(0.38), u.py(0.5), &[(0.0, rgba(255, 236, 150, 255)), (0.5, rgba(214, 160, 40, 255)), (1.0, rgba(160, 110, 20, 255))]), t);
        let mut pb = PathBuilder::new();
        pb.push_circle(u.px(0.44), u.py(0.44), u.d(0.11));
        if let Some(r) = rect(u, 0.52, 0.415, len, 0.05) {
            pb.push_rect(r);
        }
        for k in [0.6, 0.8] {
            if let Some(r) = rect(u, 0.52 + len * k, 0.46, 0.04, 0.06) {
                pb.push_rect(r);
            }
        }
        let shape = tf(pb.finish(), t);
        soft_shadow(pm, shape.clone(), s, 0.7);
        pm.fill(shape.clone(), &brass);
        pm.stroke(shape, &solid(rgba(120, 80, 10, 255)), u.lw(0.006));
        pm.fill(tf(u.circ(0.44, 0.44, 0.04), t), &solid(rgba(90, 60, 10, 255)));
    };
    key(pm, 30.0, 0.34);
    key(pm, 70.0, 0.28);
}

/// Clipe de papel de arame cromado.
pub(crate) fn clip(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let t = Transform::from_rotate_at(-30.0, u.px(0.5), u.py(0.5));
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(0.58), u.py(0.3));
    pb.line_to(u.px(0.58), u.py(0.72));
    pb.cubic_to(u.px(0.58), u.py(0.84), u.px(0.42), u.py(0.84), u.px(0.42), u.py(0.72));
    pb.line_to(u.px(0.42), u.py(0.18));
    pb.cubic_to(u.px(0.42), u.py(0.04), u.px(0.66), u.py(0.04), u.px(0.66), u.py(0.18));
    pb.line_to(u.px(0.66), u.py(0.8));
    pb.cubic_to(u.px(0.66), u.py(0.98), u.px(0.34), u.py(0.98), u.px(0.34), u.py(0.8));
    pb.line_to(u.px(0.34), u.py(0.3));
    let wire = tf(pb.finish(), t);
    pm.stroke_round(wire.clone(), &solid(rgba(60, 62, 72, 255)), u.lw(0.07));
    pm.stroke_round(wire.clone(), &linear(u.px(0.3), u.py(0.1), u.px(0.7), u.py(0.9), &[(0.0, rgba(250, 250, 252, 255)), (0.5, rgba(150, 154, 164, 255)), (1.0, rgba(230, 232, 238, 255))]), u.lw(0.05));
    pm.stroke_round(wire, &solid(rgba(255, 255, 255, 140)), u.lw(0.012));
}

/// Janela de app genérica (apps sem ícone próprio): a letra do app no conteúdo.
pub(crate) fn window(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let u = U { x, y, s };
    mini_window(pm, u, 0.08, 0.12, 0.84, 0.74, rgba(90, 150, 226, 255));
    if let Some(l) = letter.filter(|_| s >= 20.0) {
        let t = l.to_uppercase().to_string();
        let px = u.d(0.36);
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, u.px(0.5) - tw / 2.0, u.py(0.7), px, true, rgba(40, 80, 150, 230));
    }
}

/// App genérico: ladrilho de gel azul-acinzentado com a letra em relevo.
pub(crate) fn generic(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let u = U { x, y, s };
    let tile = u.rr(0.1, 0.08, 0.8, 0.8, 0.18);
    soft_shadow(pm, tile.clone(), s, 1.0);
    pm.fill(u.rr(0.1, 0.11, 0.8, 0.8, 0.18), &solid(rgba(60, 66, 90, 255)));
    gel(pm, u, tile, rgba(110, 124, 160, 255), 0.08, 0.88);
    if let Some(l) = letter {
        let t = l.to_uppercase().to_string();
        let px = u.d(0.42);
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, u.px(0.5) - tw / 2.0, u.py(0.64) + u.d(0.012), px, true, rgba(20, 26, 50, 140));
        fonts.draw(pm, &t, u.px(0.5) - tw / 2.0, u.py(0.64), px, true, Color::WHITE);
    }
}

/// Utilitário de Rede: globo em grade com a tela de radar verde na frente.
pub(crate) fn net_utility(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    wire_globe(pm, u, 0.42, 0.38, 0.32);
    let (cx, cy, r) = (0.66, 0.66, 0.24);
    pm.fill(u.circ(cx, cy, r + 0.035), &linear(u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy + r), &[(0.0, rgba(255, 255, 255, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(220, 222, 228, 255))]));
    soft_shadow(pm, u.circ(cx, cy, r + 0.035), s, 0.8);
    pm.fill(u.circ(cx, cy, r + 0.035), &linear(u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy + r), &[(0.0, rgba(255, 255, 255, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(220, 222, 228, 255))]));
    let scr = u.circ(cx, cy, r);
    pm.fill(scr.clone(), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(20, 70, 30, 255)), (1.0, rgba(4, 20, 8, 255))]));
    // Varredura e anéis
    let sweep = u.poly(&[(cx, cy), (cx + r * 1.1, cy - r * 0.3), (cx + r * 0.8, cy - r * 0.9)]);
    pm.fill_clipped(sweep, &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(120, 255, 140, 40)), (1.0, rgba(120, 255, 140, 170))]), scr.clone());
    let mut rings = PathBuilder::new();
    for k in [0.35, 0.7] {
        rings.push_circle(u.px(cx), u.py(cy), u.d(r * k));
    }
    rings.move_to(u.px(cx - r), u.py(cy));
    rings.line_to(u.px(cx + r), u.py(cy));
    rings.move_to(u.px(cx), u.py(cy - r));
    rings.line_to(u.px(cx), u.py(cy + r));
    pm.stroke(rings.finish(), &solid(rgba(90, 220, 110, 150)), u.lw(0.006));
    pm.fill(u.circ(cx + r * 0.45, cy - r * 0.35, 0.025), &solid(rgba(200, 255, 200, 255)));
    glass_glare(pm, scr.clone(), u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy), 90);
    pm.stroke(scr, &solid(rgba(30, 30, 36, 255)), u.lw(0.007));
}

/// Visualizador: duas fotos e a lupa de vidro em cima.
pub(crate) fn viewer(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::picture(pm, x - s * 0.04, y - s * 0.06, s * 0.9);
    let (cx, cy, r) = (0.66, 0.62, 0.17);
    let t = Transform::from_rotate_at(45.0, u.px(0.82), u.py(0.82));
    let handle = tf(u.rr(0.74, 0.78, 0.24, 0.08, 0.04), t);
    soft_shadow(pm, handle.clone(), s, 0.8);
    pm.fill(handle, &solid(rgba(30, 30, 36, 255)));
    pm.fill(u.circ(cx, cy, r + 0.035), &linear(u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy + r), &[(0.0, rgba(255, 255, 255, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(220, 222, 228, 255))]));
    let lens = u.circ(cx, cy, r);
    pm.fill(lens.clone(), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(236, 246, 255, 90)), (0.8, rgba(170, 205, 245, 150)), (1.0, rgba(60, 90, 150, 220))]));
    let mut moon = PathBuilder::new();
    moon.push_circle(u.px(cx), u.py(cy), u.d(r * 0.9));
    moon.push_circle(u.px(cx + r * 0.2), u.py(cy + r * 0.24), u.d(r * 0.86));
    pm.fill_rule(moon.finish(), &solid(rgba(255, 255, 255, 200)), tiny_skia::FillRule::EvenOdd);
    pm.stroke(lens, &solid(rgba(50, 54, 66, 255)), u.lw(0.007));
}

/// Utilitário de Disco: o HD com a chave inglesa cromada atravessada.
pub(crate) fn disk_utility(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::disk(pm, x, y, s);
    let t = Transform::from_rotate_at(-40.0, u.px(0.5), u.py(0.5));
    let mut pb = PathBuilder::new();
    if let Some(r) = rect(u, 0.22, 0.46, 0.52, 0.08) {
        pb.push_rect(r);
    }
    pb.push_circle(u.px(0.2), u.py(0.5), u.d(0.12));
    pb.push_circle(u.px(0.8), u.py(0.5), u.d(0.1));
    let body = tf(pb.finish(), t);
    soft_shadow(pm, body.clone(), s, 1.0);
    pm.fill(body.clone(), &tpaint(vgrad(u.py(0.38), u.py(0.62), &[(0.0, rgba(255, 255, 255, 255)), (0.45, rgba(170, 174, 184, 255)), (1.0, rgba(100, 104, 114, 255))]), t));
    pm.stroke(body, &solid(rgba(60, 62, 72, 255)), u.lw(0.007));
    // Bocas da chave (os cortes)
    for (cx, w) in [(0.12, 0.1), (0.88, 0.08)] {
        pm.fill(tf(u.rr(cx - w / 2.0, 0.47, w, 0.06, 0.0), t), &solid(rgba(236, 236, 236, 255)));
    }
}

/// Utilitário de Arquivo: caixa de papelão com o zíper de metal na tampa.
pub(crate) fn archiver(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    cardboard_box(pm, u);
    let mut teeth = PathBuilder::new();
    for k in 0..8 {
        let f = k as f32 / 8.0;
        let (px, py) = (0.36 + f * 0.42, 0.35 + f * 0.07);
        let off = if k % 2 == 0 { -0.012 } else { 0.012 };
        if let Some(r) = rect(u, px, py + off, 0.04, 0.025) {
            teeth.push_rect(r);
        }
    }
    pm.fill(teeth.finish(), &solid(rgba(150, 154, 164, 255)));
    let pull = u.rr(0.76, 0.38, 0.08, 0.14, 0.03);
    pm.fill(pull.clone(), &metal_paint(u.py(0.38), u.py(0.52)));
    pm.stroke(pull, &solid(rgba(70, 72, 82, 255)), u.lw(0.006));
}
