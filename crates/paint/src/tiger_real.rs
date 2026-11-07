//! Ícones com acabamento mais realista (9.12): menos contorno escuro e mais
//! luz, chanfro e textura de material. Disco, monitores de tubo, bússola do
//! navegador e o rosto do Arquivos. Arte própria: as referências do Mac servem
//! de inspiração e homenagem, sem copiar formas, marcas ou composição.

use tiny_skia::{Color, Paint, Path, PathBuilder, Transform};

use crate::draw::*;
use crate::font::Fonts;
use crate::tiger::{soft_shadow, U};

fn shift(p: &Option<Path>, dx: f32, dy: f32) -> Option<Path> {
    p.clone()?.transform(Transform::from_translate(dx, dy))
}

/// Rebaixo na peça (luz de cima): parede de cima na sombra, a de baixo clara,
/// e o fundo com `floor`.
fn sunken(pm: &mut Canvas, p: Option<Path>, d: f32, floor: &Paint, shade: Color, light: Color) {
    pm.fill(p.clone(), &solid(light));
    let up = shift(&p, 0.0, -d);
    pm.fill_clipped(up.clone(), &solid(shade), p.clone());
    let inner = shift(&p, 0.0, -d * 0.35);
    pm.fill_clipped(inner, floor, up);
}

/// Relevo saliente: o contrário do rebaixo (claro em cima, sombra embaixo).
fn raised(pm: &mut Canvas, p: Option<Path>, d: f32, face: &Paint, shade: Color, light: Color) {
    pm.fill(shift(&p, 0.0, d), &solid(shade));
    pm.fill(shift(&p, 0.0, -d * 0.6), &solid(light));
    pm.fill(p, face);
}

/// Pontinhos quase invisíveis: tira o aspecto de "cor chapada" do plástico.
fn speckle(pm: &mut Canvas, clip: Option<Path>, u: U, seed: u32, a: u8) {
    if u.s < 40.0 {
        return;
    }
    let n = (u.s * u.s / 30.0) as u32;
    let (mut light, mut dark) = (PathBuilder::new(), PathBuilder::new());
    let r = (u.s / 300.0).max(0.3);
    for i in 0..n {
        let h = hash(seed ^ i.wrapping_mul(2654435761));
        let (a0, b0) = ((h & 0xffff) as f32 / 65535.0, (h >> 16) as f32 / 65535.0);
        if i % 2 == 0 { &mut light } else { &mut dark }.push_circle(u.px(a0), u.py(b0), r);
    }
    pm.fill_clipped(light.finish(), &solid(rgba(255, 255, 255, a)), clip.clone());
    pm.fill_clipped(dark.finish(), &solid(rgba(0, 0, 0, a)), clip);
}

// ---------------------------------------------------------------------------
// Disco rígido
// ---------------------------------------------------------------------------

/// Tampa do HD em perspectiva de verdade: o fundo (em cima) mais estreito e
/// mais curto que a frente. Converte a medida na tampa (0 a 1) em ponto.
struct Lid {
    u: U,
}

impl Lid {
    fn at(&self, px: f32, py: f32) -> (f32, f32) {
        let w = 0.8 + 0.12 * py;
        let sy = 0.06 + 0.66 * py * (0.86 + 0.14 * py);
        (self.u.px(0.5 + (px - 0.5) * w), self.u.py(sy))
    }
    fn path(&self, pts: &[(f32, f32)]) -> Option<Path> {
        let mut pb = PathBuilder::new();
        for (i, &(a, b)) in pts.iter().enumerate() {
            let (x, y) = self.at(a, b);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        pb.finish()
    }
    fn circle(&self, cx: f32, cy: f32, r: f32) -> Option<Path> {
        let pts: Vec<(f32, f32)> = (0..72).map(|i| {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            (cx + a.cos() * r, cy + a.sin() * r)
        }).collect();
        self.path(&pts)
    }
    fn rr(&self, x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
        let mut pts = Vec::new();
        let corners = [(x + w - r, y + r, -0.25), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 0.25), (x + r, y + r, 0.5)];
        for (cx, cy, start) in corners {
            for k in 0..=6 {
                let a = (start + k as f32 / 6.0 * 0.25) * std::f32::consts::TAU;
                pts.push((cx + a.cos() * r, cy + a.sin() * r));
            }
        }
        self.path(&pts)
    }
}

/// Discos: HD visto de cima e de frente. Tampa de alumínio com o rebaixo do
/// prato e da etiqueta, parafusos Torx; na frente, as fendas e a luz acesa.
pub(crate) fn disk(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let lid = Lid { u };
    let (fl, fr) = (lid.at(0.0, 1.0), lid.at(1.0, 1.0));
    let (front_top, front_bot) = (fl.1, u.py(0.9));
    let plate = lid.rr(0.0, 0.0, 1.0, 1.0, 0.035);
    let body = {
        let mut pb = PathBuilder::new();
        let (tl, tr) = (lid.at(0.0, 0.0), lid.at(1.0, 0.0));
        let r = u.d(0.03);
        pb.move_to(tl.0, tl.1);
        pb.line_to(tr.0, tr.1);
        pb.line_to(fr.0, front_bot - r);
        pb.quad_to(fr.0, front_bot, fr.0 - r, front_bot);
        pb.line_to(fl.0 + r, front_bot);
        pb.quad_to(fl.0, front_bot, fl.0, front_bot - r);
        pb.close();
        pb.finish()
    };
    soft_shadow(pm, body, s, 1.1);

    // Frente: metal mais escuro, com a sombra que a tampa faz logo abaixo dela.
    let front = rrect4(fl.0, front_top, fr.0 - fl.0, front_bot - front_top, [0.0, 0.0, u.d(0.03), u.d(0.03)]);
    pm.fill(front.clone(), &vgrad(front_top, front_bot, &[(0.0, rgba(120, 122, 128, 255)), (0.25, rgba(156, 158, 164, 255)), (0.7, rgba(128, 130, 137, 255)), (1.0, rgba(88, 90, 97, 255))]));
    pm.fill_clipped(rrect(fl.0, front_top, fr.0 - fl.0, u.d(0.03), 0.0), &vgrad(front_top, front_top + u.d(0.03), &[(0.0, rgba(0, 0, 0, 110)), (1.0, rgba(0, 0, 0, 0))]), front.clone());
    let fx = |t: f32| fl.0 + (fr.0 - fl.0) * t;
    let fy = |t: f32| front_top + (front_bot - front_top) * t;
    // Vão do conector à esquerda.
    let bay = rrect(fx(0.1), fy(0.3), fx(0.42) - fx(0.1), fy(0.88) - fy(0.3), u.d(0.012));
    pm.fill(bay.clone(), &vgrad(fy(0.3), fy(0.88), &[(0.0, rgba(70, 72, 78, 255)), (1.0, rgba(112, 114, 120, 255))]));
    pm.fill_clipped(shift(&bay, 0.0, u.d(0.008)), &solid(rgba(0, 0, 0, 60)), bay.clone());
    // Fendas de ventilação
    if s >= 24.0 {
        let n = 8;
        for k in 0..n {
            let t0 = 0.47 + k as f32 * 0.032;
            let slot = rrect(fx(t0), fy(0.3), fx(t0 + 0.016) - fx(t0), fy(0.86) - fy(0.3), u.d(0.004));
            pm.fill(slot.clone(), &vgrad(fy(0.3), fy(0.86), &[(0.0, rgba(14, 14, 16, 255)), (1.0, rgba(44, 44, 48, 255))]));
            pm.fill(rrect(fx(t0 + 0.016), fy(0.32), u.d(0.004).max(0.4), fy(0.86) - fy(0.32), 0.0), &solid(rgba(255, 255, 255, 50)));
        }
    }
    // Luz de atividade acesa (amarelo-esverdeada), com o brilho em volta.
    let (lx, ly) = (fx(0.88), fy(0.55));
    pm.fill(circle(lx, ly, u.d(0.05)), &radial(lx, ly, u.d(0.05), &[(0.0, rgba(230, 255, 80, 120)), (1.0, rgba(230, 255, 80, 0))]));
    pm.fill(circle(lx, ly + u.d(0.004), u.d(0.022)), &solid(rgba(40, 40, 30, 200)));
    pm.fill(circle(lx, ly, u.d(0.019)), &radial(lx - u.d(0.006), ly - u.d(0.006), u.d(0.025), &[(0.0, rgba(255, 255, 220, 255)), (0.4, rgba(236, 240, 40, 255)), (1.0, rgba(150, 160, 0, 255))]));

    // Tampa: alumínio liso, mais claro no fundo e à esquerda (a luz vem de cima).
    let top_light = lid.at(0.0, 0.0);
    let bot_dark = lid.at(1.0, 1.0);
    let metal = linear(top_light.0, top_light.1, bot_dark.0, bot_dark.1, &[(0.0, rgba(232, 233, 236, 255)), (0.45, rgba(214, 215, 219, 255)), (0.7, rgba(196, 198, 203, 255)), (1.0, rgba(170, 172, 178, 255))]);
    pm.fill(plate.clone(), &metal);
    // Reflexo largo e suave na diagonal.
    pm.fill_clipped(u.poly(&[(0.0, 0.3), (0.35, 0.0), (0.6, 0.0), (0.0, 0.62)]), &solid(rgba(255, 255, 255, 34)), plate.clone());
    // Borda da tampa: fio claro na frente (a quina pega luz) e contorno discreto.
    let mut lip = PathBuilder::new();
    lip.move_to(fl.0 + u.d(0.02), fl.1 - u.d(0.003));
    lip.line_to(fr.0 - u.d(0.02), fr.1 - u.d(0.003));
    pm.stroke(lip.finish(), &solid(rgba(255, 255, 255, 230)), u.lw(0.007));
    pm.stroke(plate.clone(), &solid(rgba(96, 98, 106, 200)), u.lw(0.004));

    let floor = |a: f32, b: f32| {
        let (p0, p1) = (lid.at(a, a), lid.at(b, b));
        linear(p0.0, p0.1, p1.0, p1.1, &[(0.0, rgba(216, 217, 221, 255)), (1.0, rgba(184, 186, 192, 255))])
    };
    let (shade, light) = (rgba(118, 120, 128, 255), rgba(255, 255, 255, 255));
    let d = u.d(0.018);
    // Rebaixo do prato (o círculo grande).
    sunken(pm, lid.circle(0.43, 0.59, 0.33), d, &floor(0.2, 0.9), shade, light);
    // Cubo do motor: um anel afundado com a parte do meio saliente.
    sunken(pm, lid.circle(0.43, 0.59, 0.1), d * 0.8, &floor(0.4, 0.7), shade, light);
    raised(pm, lid.circle(0.43, 0.59, 0.075), d * 0.5, &floor(0.3, 0.8), shade, light);
    // Bolso da etiqueta e a etiqueta escura (o texto vira riscos).
    sunken(pm, lid.rr(0.12, 0.06, 0.62, 0.17, 0.02), d, &floor(0.1, 0.3), shade, light);
    let label = lid.rr(0.15, 0.085, 0.56, 0.12, 0.008);
    pm.fill(label.clone(), &vgrad(lid.at(0.0, 0.085).1, lid.at(0.0, 0.2).1, &[(0.0, rgba(34, 34, 38, 255)), (1.0, rgba(14, 14, 16, 255))]));
    if s >= 40.0 {
        let line = |a: f32, b: f32, yy: f32, th: f32| lid.path(&[(a, yy), (b, yy), (b, yy + th), (a, yy + th)]);
        let ink = solid(rgba(236, 236, 238, 235));
        pm.fill(line(0.18, 0.42, 0.1, 0.018), &ink);
        pm.fill(line(0.18, 0.6, 0.126, 0.005), &ink);
        for (k, w) in [0.38, 0.34, 0.4].iter().enumerate() {
            pm.fill(line(0.18, 0.18 + w, 0.142 + k as f32 * 0.018, 0.008), &solid(rgba(200, 200, 204, 200)));
        }
        pm.stroke(lid.circle(0.655, 0.14, 0.03), &solid(rgba(236, 236, 238, 220)), u.lw(0.006));
    }
    // Parafusos Torx: cabeça escura abaulada com o brilho em cima.
    if s >= 24.0 {
        for (a, b) in [(0.045, 0.04), (0.955, 0.04), (0.04, 0.48), (0.96, 0.48), (0.04, 0.95), (0.96, 0.95)] {
            let (sx, sy) = lid.at(a, b);
            let r = u.d(0.022 + 0.006 * b);
            pm.fill(circle(sx, sy + r * 0.25, r * 1.15), &solid(rgba(255, 255, 255, 140)));
            pm.fill(circle(sx, sy, r), &radial(sx - r * 0.3, sy - r * 0.4, r * 1.3, &[(0.0, rgba(120, 122, 130, 255)), (0.6, rgba(46, 48, 54, 255)), (1.0, rgba(20, 20, 24, 255))]));
            if s >= 64.0 {
                let mut st = PathBuilder::new();
                for k in 0..3 {
                    let ang = k as f32 * std::f32::consts::PI / 3.0;
                    st.move_to(sx - ang.cos() * r * 0.5, sy - ang.sin() * r * 0.5 * 0.7);
                    st.line_to(sx + ang.cos() * r * 0.5, sy + ang.sin() * r * 0.5 * 0.7);
                }
                pm.stroke(st.finish(), &solid(rgba(8, 8, 10, 255)), u.lw(0.006));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Monitor de tubo bege (Terminal e Monitor de Atividade)
// ---------------------------------------------------------------------------

/// Tela do monitor (medidas relativas): x0, y0, x1, y1.
pub(crate) const CRT: (f32, f32, f32, f32) = (0.165, 0.135, 0.835, 0.615);

/// Monitor de tubo de plástico bege, de frente: gabinete com quinas
/// arredondadas, moldura funda em volta do vidro, o queixo com a plaqueta,
/// a tecla de ligar e a grade do alto-falante, sobre o pé giratório.
/// Devolve a tela; depois do conteúdo, chame `crt_glass`.
pub(crate) fn crt(pm: &mut Canvas, u: U) -> Option<Path> {
    let beige = |y0: f32, y1: f32| vgrad(u.py(y0), u.py(y1), &[(0.0, rgba(236, 231, 218, 255)), (0.5, rgba(222, 216, 201, 255)), (1.0, rgba(196, 189, 172, 255))]);
    // Pé: o pescoço e a base oval.
    let foot = u.rr(0.25, 0.85, 0.5, 0.075, 0.03);
    soft_shadow(pm, foot.clone(), u.s, 0.7);
    pm.fill(foot.clone(), &beige(0.85, 0.93));
    pm.fill_clipped(u.rr(0.25, 0.85, 0.5, 0.012, 0.0), &solid(rgba(255, 255, 255, 120)), foot);
    pm.fill(u.poly(&[(0.38, 0.79), (0.62, 0.79), (0.66, 0.86), (0.34, 0.86)]), &vgrad(u.py(0.79), u.py(0.86), &[(0.0, rgba(140, 134, 120, 255)), (0.4, rgba(186, 180, 164, 255)), (1.0, rgba(210, 204, 188, 255))]));
    // Gabinete
    let case = u.rr(0.06, 0.05, 0.88, 0.76, 0.045);
    soft_shadow(pm, case.clone(), u.s, 1.0);
    pm.fill(case.clone(), &beige(0.05, 0.81));
    // As laterais arredondam: escurecem um pouco nas pontas.
    pm.fill_clipped(case.clone(), &linear(u.px(0.06), 0.0, u.px(0.94), 0.0, &[(0.0, rgba(0, 0, 0, 34)), (0.06, rgba(0, 0, 0, 0)), (0.94, rgba(0, 0, 0, 0)), (1.0, rgba(0, 0, 0, 44))]), case.clone());
    speckle(pm, case.clone(), u, 71, 14);
    // Quina de cima pegando luz e a de baixo na sombra.
    pm.fill_clipped(u.rr(0.06, 0.05, 0.88, 0.012, 0.0), &solid(rgba(255, 255, 255, 150)), case.clone());
    pm.fill_clipped(u.rr(0.06, 0.795, 0.88, 0.015, 0.0), &solid(rgba(0, 0, 0, 40)), case.clone());
    pm.stroke(case, &solid(rgba(150, 143, 126, 255)), u.lw(0.005));

    // Moldura funda em volta do vidro: sombra em cima, luz embaixo.
    let (x0, y0, x1, y1) = CRT;
    let m = 0.035;
    let well = u.rr(x0 - m, y0 - m, x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m, 0.06);
    pm.fill(well.clone(), &vgrad(u.py(y0 - m), u.py(y1 + m), &[(0.0, rgba(140, 133, 116, 255)), (0.35, rgba(196, 190, 174, 255)), (0.8, rgba(232, 227, 214, 255)), (1.0, rgba(250, 247, 238, 255))]));
    pm.fill_clipped(well.clone(), &linear(u.px(x0 - m), 0.0, u.px(x1 + m), 0.0, &[(0.0, rgba(0, 0, 0, 30)), (0.12, rgba(0, 0, 0, 0)), (0.88, rgba(0, 0, 0, 0)), (1.0, rgba(255, 255, 255, 40))]), well.clone());
    pm.stroke(well, &solid(rgba(160, 153, 136, 180)), u.lw(0.004));
    // Junta do queixo
    pm.fill(u.rr(0.07, 0.705, 0.86, 0.006, 0.0), &solid(rgba(150, 143, 126, 150)));
    pm.fill(u.rr(0.07, 0.711, 0.86, 0.005, 0.0), &solid(rgba(255, 255, 255, 110)));
    if u.s >= 24.0 {
        let (shade, light) = (rgba(150, 143, 126, 255), rgba(250, 247, 238, 255));
        // Plaqueta à esquerda (bege mais escuro, com um ponto de luz).
        sunken(pm, u.rr(0.12, 0.73, 0.1, 0.045, 0.008), u.d(0.008), &beige(0.73, 0.775), shade, light);
        pm.fill(u.rr(0.135, 0.742, 0.07, 0.022, 0.004), &vgrad(u.py(0.742), u.py(0.764), &[(0.0, rgba(90, 88, 84, 255)), (1.0, rgba(60, 58, 56, 255))]));
        pm.fill(u.circ(0.15, 0.753, 0.005), &solid(rgba(210, 205, 190, 255)));
        // Tecla de ligar e a luz verde.
        sunken(pm, u.rr(0.6, 0.73, 0.1, 0.045, 0.008), u.d(0.008), &beige(0.73, 0.775), shade, light);
        raised(pm, u.rr(0.615, 0.738, 0.045, 0.03, 0.005), u.d(0.004), &beige(0.73, 0.78), shade, light);
        pm.fill(u.circ(0.68, 0.752, 0.008), &radial(u.px(0.68), u.py(0.752), u.d(0.012), &[(0.0, rgba(200, 255, 200, 255)), (1.0, rgba(40, 170, 60, 255))]));
        // Grade do alto-falante
        if u.s >= 40.0 {
            let mut dots = PathBuilder::new();
            for i in 0..4 {
                for j in 0..2 {
                    dots.push_circle(u.px(0.76 + i as f32 * 0.025), u.py(0.743 + j as f32 * 0.02), u.d(0.006));
                }
            }
            let d = dots.finish();
            pm.fill(shift(&d, 0.0, u.d(0.003)), &solid(rgba(255, 255, 255, 160)));
            pm.fill(d, &solid(rgba(90, 84, 72, 255)));
        }
    }
    u.rr(x0, y0, x1 - x0, y1 - y0, 0.045)
}

/// Vidro do tubo: cantos escurecidos (abaulado), um reflexo largo e suave na
/// diagonal e o fio de luz na borda de cima.
pub(crate) fn crt_glass(pm: &mut Canvas, u: U, screen: Option<Path>) {
    let (x0, y0, x1, y1) = CRT;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    pm.fill_clipped(screen.clone(), &radial(u.px(cx), u.py(cy), u.d((x1 - x0) * 0.66), &[(0.0, rgba(0, 0, 0, 0)), (0.7, rgba(0, 0, 0, 20)), (1.0, rgba(0, 0, 0, 150))]), screen.clone());
    // Reflexo da sala no vidro curvo: faixa clara que some para baixo e à direita.
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(x0), u.py(y0));
    pb.line_to(u.px(x1 - 0.1), u.py(y0));
    pb.quad_to(u.px(cx), u.py(cy - 0.02), u.px(x0), u.py(y1 - 0.08));
    pb.close();
    pm.fill_clipped(pb.finish(), &linear(u.px(x0), u.py(y0), u.px(cx), u.py(cy), &[(0.0, rgba(255, 255, 255, 70)), (1.0, rgba(255, 255, 255, 6))]), screen.clone());
    let mut lip = PathBuilder::new();
    lip.move_to(u.px(x0 + 0.05), u.py(y0 + 0.006));
    lip.line_to(u.px(x1 - 0.05), u.py(y0 + 0.006));
    pm.stroke(lip.finish(), &solid(rgba(255, 255, 255, 50)), u.lw(0.005));
    pm.stroke(screen, &solid(rgba(30, 30, 30, 220)), u.lw(0.006));
}

/// Terminal: o monitor bege com o prompt ">_" e linhas de texto em fósforo verde.
pub(crate) fn terminal(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let screen = crt(pm, u);
    let (x0, y0, x1, y1) = CRT;
    pm.fill(screen.clone(), &radial(u.px((x0 + x1) / 2.0), u.py((y0 + y1) / 2.0), u.d(0.4), &[(0.0, rgba(26, 40, 34, 255)), (1.0, rgba(8, 14, 12, 255))]));
    // Linhas de varredura bem leves.
    if s >= 48.0 {
        let mut sl = PathBuilder::new();
        let mut yy = y0;
        while yy < y1 {
            if let Some(r) = tiny_skia::Rect::from_xywh(u.px(x0), u.py(yy), u.d(x1 - x0), (u.s / 256.0).max(0.4)) {
                sl.push_rect(r);
            }
            yy += 0.012;
        }
        pm.fill_clipped(sl.finish(), &solid(rgba(0, 0, 0, 60)), screen.clone());
    }
    let ink = rgba(150, 255, 170, 255);
    let (px0, py0) = (x0 + 0.07, y0 + 0.09);
    if s >= 40.0 {
        let mut lines = PathBuilder::new();
        for (k, w) in [0.34, 0.24, 0.4].iter().enumerate() {
            if let Some(r) = tiny_skia::Rect::from_xywh(u.px(px0), u.py(py0 + k as f32 * 0.045), u.d(*w), u.d(0.016)) {
                lines.push_rect(r);
            }
        }
        let l = lines.finish();
        pm.fill(l.clone(), &solid(rgba(80, 255, 130, 40)));
        pm.fill(l, &solid(rgba(120, 230, 150, 150)));
    }
    let (qy0, qh) = if s >= 40.0 { (py0 + 0.15, 0.11) } else { (y0 + 0.1, 0.2) };
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(px0), u.py(qy0));
    pb.line_to(u.px(px0 + qh * 0.75), u.py(qy0 + qh / 2.0));
    pb.line_to(u.px(px0), u.py(qy0 + qh));
    let prompt = pb.finish();
    pm.stroke_round(prompt.clone(), &solid(rgba(80, 255, 130, 50)), u.lw(qh * 0.5));
    pm.stroke_round(prompt, &solid(ink), u.lw(qh * 0.24));
    let cur = u.rr(px0 + qh * 0.95, qy0 + qh - 0.025, qh * 1.0, 0.025, 0.003);
    pm.fill(cur, &solid(ink));
    crt_glass(pm, u, screen);
}

/// Monitor de Atividade: o mesmo monitor com o gráfico de carga em verde.
pub(crate) fn activity(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let screen = crt(pm, u);
    let (x0, y0, x1, y1) = CRT;
    pm.fill(screen.clone(), &solid(rgba(6, 12, 9, 255)));
    if s >= 32.0 {
        let mut grid = PathBuilder::new();
        for k in 1..6 {
            let xx = x0 + k as f32 * (x1 - x0) / 6.0;
            grid.move_to(u.px(xx), u.py(y0));
            grid.line_to(u.px(xx), u.py(y1));
        }
        for k in 1..4 {
            let yy = y0 + k as f32 * (y1 - y0) / 4.0;
            grid.move_to(u.px(x0), u.py(yy));
            grid.line_to(u.px(x1), u.py(yy));
        }
        pm.fill_clipped(grid.finish().and_then(|g| g.stroke(&tiny_skia::Stroke { width: u.lw(0.003), ..Default::default() }, 1.0)), &solid(rgba(40, 120, 60, 110)), screen.clone());
    }
    let vals = [0.72, 0.6, 0.68, 0.32, 0.45, 0.22, 0.4, 0.56, 0.36, 0.5];
    let mut pb = PathBuilder::new();
    let mut area = PathBuilder::new();
    area.move_to(u.px(x0), u.py(y1));
    for (i, v) in vals.iter().enumerate() {
        let xx = x0 + i as f32 * (x1 - x0) / (vals.len() - 1) as f32;
        let yy = y0 + v * (y1 - y0);
        if i == 0 {
            pb.move_to(u.px(xx), u.py(yy));
        } else {
            pb.line_to(u.px(xx), u.py(yy));
        }
        area.line_to(u.px(xx), u.py(yy));
    }
    area.line_to(u.px(x1), u.py(y1));
    area.close();
    pm.fill_clipped(area.finish(), &vgrad(u.py(y0), u.py(y1), &[(0.0, rgba(60, 230, 90, 110)), (1.0, rgba(20, 120, 40, 15))]), screen.clone());
    let line = pb.finish();
    pm.stroke(line.clone(), &solid(rgba(80, 255, 130, 50)), u.lw(0.045));
    pm.stroke(line, &solid(rgba(150, 255, 170, 255)), u.lw(0.014));
    crt_glass(pm, u, screen);
}

// ---------------------------------------------------------------------------
// Navegador: bússola
// ---------------------------------------------------------------------------

/// Navegador: bússola de bolso de aço cromado com coroa, mostrador azul com os
/// meridianos de um globo, a rosa de traços finos e a agulha vermelha e prata
/// sob a cúpula de vidro.
pub(crate) fn compass(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let (cx, cy, r) = (0.5, 0.53, 0.42);
    let chrome = |a: f32, b: f32, rr: f32, flip: bool| {
        let (p0, p1) = ((u.px(a - rr), u.py(b - rr)), (u.px(a + rr), u.py(b + rr)));
        let (p0, p1) = if flip { (p1, p0) } else { (p0, p1) };
        linear(p0.0, p0.1, p1.0, p1.1, &[
            (0.0, rgba(255, 255, 255, 255)),
            (0.18, rgba(206, 210, 218, 255)),
            (0.38, rgba(116, 120, 132, 255)),
            (0.5, rgba(222, 225, 232, 255)),
            (0.62, rgba(250, 251, 253, 255)),
            (0.82, rgba(132, 136, 148, 255)),
            (1.0, rgba(78, 82, 94, 255)),
        ])
    };
    // Coroa em cima (o botão da bússola de bolso).
    let crown = u.rr(0.455, 0.055, 0.09, 0.07, 0.018);
    pm.fill(crown.clone(), &linear(u.px(0.455), 0.0, u.px(0.545), 0.0, &[(0.0, rgba(110, 114, 126, 255)), (0.35, rgba(250, 251, 253, 255)), (0.6, rgba(180, 184, 194, 255)), (1.0, rgba(90, 94, 106, 255))]));
    if s >= 40.0 {
        let mut kn = PathBuilder::new();
        for k in 0..6 {
            let xx = 0.465 + k as f32 * 0.014;
            kn.move_to(u.px(xx), u.py(0.065));
            kn.line_to(u.px(xx), u.py(0.115));
        }
        pm.stroke(kn.finish(), &solid(rgba(60, 62, 72, 120)), u.lw(0.004));
    }
    pm.fill(u.rr(0.47, 0.1, 0.06, 0.035, 0.008), &linear(u.px(0.47), 0.0, u.px(0.53), 0.0, &[(0.0, rgba(100, 104, 116, 255)), (0.5, rgba(230, 232, 238, 255)), (1.0, rgba(90, 94, 106, 255))]));
    // Caixa cromada: aro externo, a quina torneada e o aro interno escuro.
    let outer = u.circ(cx, cy, r);
    soft_shadow(pm, outer.clone(), s, 1.0);
    pm.fill(outer.clone(), &chrome(cx, cy, r, false));
    pm.fill(u.circ(cx, cy, r * 0.9), &chrome(cx, cy, r * 0.9, true));
    pm.stroke(outer, &solid(rgba(70, 74, 86, 200)), u.lw(0.005));
    pm.fill(u.circ(cx, cy, r * 0.82), &solid(rgba(40, 46, 60, 255)));
    // Mostrador azul, mais claro no meio.
    let rd = r * 0.8;
    let dial = u.circ(cx, cy, rd);
    pm.fill(dial.clone(), &radial(u.px(cx), u.py(cy - 0.04), u.d(rd * 1.1), &[(0.0, rgba(170, 220, 255, 255)), (0.45, rgba(70, 150, 230, 255)), (1.0, rgba(18, 70, 160, 255))]));
    // Meridianos e paralelos de um globo, bem leves.
    if s >= 32.0 {
        let line = solid(rgba(255, 255, 255, 46));
        for k in [0.35, 0.7] {
            pm.stroke(u.ov(cx - rd * k, cy - rd, rd * 2.0 * k, rd * 2.0), &line, u.lw(0.004));
        }
        let mut lat = PathBuilder::new();
        lat.move_to(u.px(cx - rd), u.py(cy));
        lat.line_to(u.px(cx + rd), u.py(cy));
        for k in [-0.5f32, 0.5] {
            let yy = cy + rd * k;
            let half = rd * (1.0 - k * k).sqrt();
            lat.move_to(u.px(cx - half), u.py(yy));
            lat.line_to(u.px(cx + half), u.py(yy));
        }
        pm.stroke(lat.finish(), &line, u.lw(0.004));
    }
    // Traços na borda do mostrador (os principais mais longos).
    if s >= 24.0 {
        let mut ticks = PathBuilder::new();
        let mut major = PathBuilder::new();
        let n = if s >= 64.0 { 72 } else { 24 };
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let main = i % (n / 8) == 0;
            let r0 = rd * if main { 0.78 } else { 0.88 };
            let pb = if main { &mut major } else { &mut ticks };
            pb.move_to(u.px(cx + a.sin() * r0), u.py(cy - a.cos() * r0));
            pb.line_to(u.px(cx + a.sin() * rd * 0.96), u.py(cy - a.cos() * rd * 0.96));
        }
        pm.stroke(ticks.finish(), &solid(rgba(255, 255, 255, 170)), u.lw(0.004));
        pm.stroke(major.finish(), &solid(rgba(255, 255, 255, 235)), u.lw(0.008));
    }
    // Pontos cardeais
    if s >= 48.0 {
        let px = u.d(0.075);
        for (t, a) in [("N", 0.0f32), ("L", 90.0), ("S", 180.0), ("O", 270.0)] {
            let a = a.to_radians();
            let (tx, ty) = (u.px(cx + a.sin() * rd * 0.6), u.py(cy - a.cos() * rd * 0.6));
            let tw = fonts.measure(t, px, true);
            fonts.draw(pm, t, tx - tw / 2.0 + 0.6, ty + px * 0.36 + 0.8, px, true, rgba(10, 40, 100, 120));
            fonts.draw(pm, t, tx - tw / 2.0, ty + px * 0.36, px, true, rgba(255, 255, 255, 240));
        }
    }
    // Agulha: losango comprido, cada metade com um lado claro e outro escuro.
    let ang = 28f32.to_radians();
    let (dx, dy) = (ang.sin(), -ang.cos());
    let (nx, ny) = (-dy, dx);
    let (len, wid) = (rd * 0.86, rd * 0.11);
    let p = |along: f32, side: f32| (u.px(cx + dx * along + nx * side), u.py(cy + dy * along + ny * side));
    let tri = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        pb.line_to(c.0, c.1);
        pb.close();
        pb.finish()
    };
    let (tip, tail, l, rr) = (p(len, 0.0), p(-len, 0.0), p(0.0, -wid), p(0.0, wid));
    // Sombra da agulha no mostrador.
    let shadow_t = Transform::from_translate(u.d(0.012), u.d(0.02));
    pm.fill(tri(tip, l, tail).and_then(|t| t.transform(shadow_t)), &solid(rgba(0, 20, 60, 70)));
    pm.fill(tri(tip, rr, tail).and_then(|t| t.transform(shadow_t)), &solid(rgba(0, 20, 60, 70)));
    pm.fill(tri(tip, l, p(0.0, 0.0)), &solid(rgba(236, 60, 44, 255)));
    pm.fill(tri(tip, rr, p(0.0, 0.0)), &solid(rgba(160, 24, 20, 255)));
    pm.fill(tri(tail, l, p(0.0, 0.0)), &solid(rgba(248, 249, 252, 255)));
    pm.fill(tri(tail, rr, p(0.0, 0.0)), &solid(rgba(170, 176, 190, 255)));
    // Eixo
    pm.fill(u.circ(cx, cy, 0.035), &chrome(cx, cy, 0.035, false));
    pm.fill(u.circ(cx, cy, 0.014), &solid(rgba(70, 74, 86, 255)));
    // Cúpula de vidro: reflexo largo em cima e o brilho na borda de baixo.
    pm.fill_clipped(u.ov(cx - rd * 0.85, cy - rd * 0.98, rd * 1.7, rd * 1.05), &vgrad(u.py(cy - rd), u.py(cy + rd * 0.1), &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(255, 255, 255, 0))]), dial.clone());
    pm.fill_clipped(dial.clone(), &radial(u.px(cx), u.py(cy), u.d(rd), &[(0.0, rgba(0, 0, 0, 0)), (0.8, rgba(0, 0, 0, 0)), (1.0, rgba(0, 20, 60, 90))]), dial.clone());
    let mut rim = PathBuilder::new();
    let (a0, a1) = (0.2f32 * std::f32::consts::PI, 0.8f32 * std::f32::consts::PI);
    for k in 0..=20 {
        let a = a0 + (a1 - a0) * k as f32 / 20.0;
        let (xx, yy) = (u.px(cx + a.cos() * rd * 0.93), u.py(cy + a.sin() * rd * 0.93));
        if k == 0 { rim.move_to(xx, yy) } else { rim.line_to(xx, yy) }
    }
    pm.stroke_round(rim.finish(), &solid(rgba(255, 255, 255, 90)), u.lw(0.01));
}

// ---------------------------------------------------------------------------
// Arquivos: o rosto em dois tons
// ---------------------------------------------------------------------------

/// Arquivos: placa de vidro azul com um rosto sorridente em dois tons. Uma
/// curva de perfil divide a placa e forma o nariz (homenagem ao rosto do
/// Finder, com desenho próprio: o lado claro à esquerda, o perfil olhando
/// para a direita, olhos redondos e sem traço preto).
pub(crate) fn files_app(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x: x + s * 0.04, y: y + s * 0.04, s: s * 0.92 };
    let tile = u.rr(0.04, 0.06, 0.92, 0.86, 0.09);
    soft_shadow(pm, tile.clone(), s, 1.0);
    // Espessura da placa
    pm.fill(shift(&tile, 0.0, u.d(0.03)), &vgrad(u.py(0.5), u.py(0.95), &[(0.0, rgba(40, 70, 130, 255)), (1.0, rgba(14, 34, 80, 255))]));
    // Perfil: desce do alto, entra para a esquerda na testa, sai no nariz
    // (para a direita), volta no lábio e desce pelo queixo.
    let profile = |pb: &mut PathBuilder| {
        pb.line_to(u.px(0.53), u.py(0.06));
        pb.cubic_to(u.px(0.5), u.py(0.2), u.px(0.47), u.py(0.3), u.px(0.5), u.py(0.4));
        pb.cubic_to(u.px(0.53), u.py(0.47), u.px(0.6), u.py(0.5), u.px(0.6), u.py(0.55));
        pb.cubic_to(u.px(0.6), u.py(0.58), u.px(0.55), u.py(0.58), u.px(0.53), u.py(0.6));
        pb.cubic_to(u.px(0.5), u.py(0.66), u.px(0.56), u.py(0.72), u.px(0.53), u.py(0.8));
        pb.line_to(u.px(0.5), u.py(0.92));
    };
    let left = {
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(0.0), u.py(0.06));
        profile(&mut pb);
        pb.line_to(u.px(0.0), u.py(0.92));
        pb.close();
        pb.finish()
    };
    // Lado direito: azul profundo; esquerdo: azul-gelo claro.
    pm.fill(tile.clone(), &linear(u.px(0.5), u.py(0.06), u.px(0.96), u.py(0.92), &[(0.0, rgba(96, 156, 238, 255)), (0.5, rgba(44, 106, 214, 255)), (1.0, rgba(20, 64, 168, 255))]));
    pm.fill_clipped(left.clone(), &linear(u.px(0.04), u.py(0.06), u.px(0.5), u.py(0.92), &[(0.0, rgba(246, 250, 255, 255)), (0.6, rgba(206, 222, 244, 255)), (1.0, rgba(170, 192, 228, 255))]), tile.clone());
    // A emenda do perfil em relevo: sombra fina do lado azul, fio claro do lado claro.
    let seam = {
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(0.53), u.py(0.06));
        profile(&mut pb);
        pb.finish()
    };
    pm.fill_clipped(seam.clone().and_then(|p| p.stroke(&tiny_skia::Stroke { width: u.lw(0.018), ..Default::default() }, 1.0)), &solid(rgba(10, 40, 110, 110)), tile.clone());
    pm.fill_clipped(seam.and_then(|p| p.stroke(&tiny_skia::Stroke { width: u.lw(0.006), ..Default::default() }, 1.0)), &solid(rgba(255, 255, 255, 170)), left.clone());
    // Olhos redondos e o sorriso, afundados (fio claro embaixo de cada um).
    let face = |pm: &mut Canvas, dy: f32, l: Color, r: Color| {
        for (ex, c) in [(0.3, l), (0.72, r)] {
            pm.fill(u.ov(ex - 0.045, 0.3 + dy, 0.09, 0.13), &solid(c));
        }
        let mut smile = PathBuilder::new();
        smile.move_to(u.px(0.22), u.py(0.66 + dy));
        smile.cubic_to(u.px(0.36), u.py(0.8 + dy), u.px(0.64), u.py(0.8 + dy), u.px(0.8), u.py(0.64 + dy));
        let sm = smile.finish().and_then(|p| p.stroke(&tiny_skia::Stroke { width: u.lw(0.045), line_cap: tiny_skia::LineCap::Round, ..Default::default() }, 1.0));
        pm.fill_clipped(sm.clone(), &solid(l), left.clone());
        // Metade do sorriso no lado azul: tudo que não está à esquerda.
        let mut right_side = PathBuilder::new();
        right_side.move_to(u.px(0.53), u.py(0.06));
        profile(&mut right_side);
        right_side.line_to(u.px(1.0), u.py(0.92));
        right_side.line_to(u.px(1.0), u.py(0.06));
        right_side.close();
        pm.fill_clipped(sm, &solid(r), right_side.finish());
    };
    face(pm, 0.012, rgba(255, 255, 255, 200), rgba(150, 200, 255, 140));
    face(pm, 0.0, rgba(40, 70, 130, 255), rgba(8, 30, 90, 255));
    // Vidro: reflexo largo em cima, brilho que atravessa embaixo, borda fina.
    pm.fill_clipped(u.ov(-0.1, -0.2, 1.2, 0.62), &vgrad(u.py(0.06), u.py(0.42), &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(255, 255, 255, 8))]), tile.clone());
    pm.fill_clipped(u.ov(0.1, 0.62, 0.8, 0.4), &radial(u.px(0.5), u.py(0.95), u.d(0.4), &[(0.0, rgba(180, 230, 255, 90)), (1.0, rgba(180, 230, 255, 0))]), tile.clone());
    pm.stroke(tile, &solid(rgba(20, 50, 120, 200)), u.lw(0.006));
}
