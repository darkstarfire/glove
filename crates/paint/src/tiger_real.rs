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

// ---------------------------------------------------------------------------
// Segundo lote realista (9.12.2)
// ---------------------------------------------------------------------------

fn cubic(pts: &[(f32, f32)], u: U) -> Option<Path> {
    // Primeiro ponto e depois trios (controle, controle, fim).
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(pts[0].0), u.py(pts[0].1));
    for c in pts[1..].chunks(3) {
        if c.len() == 3 {
            pb.cubic_to(u.px(c[0].0), u.py(c[0].1), u.px(c[1].0), u.py(c[1].1), u.px(c[2].0), u.py(c[2].1));
        }
    }
    pb.finish()
}

/// Aço escovado escuro com faixas de reflexo na diagonal (engrenagens, metal de peças).
fn steel(u: U, x0: f32, y0: f32, x1: f32, y1: f32) -> Paint<'static> {
    linear(u.px(x0), u.py(y0), u.px(x1), u.py(y1), &[
        (0.0, rgba(214, 218, 226, 255)),
        (0.3, rgba(156, 160, 170, 255)),
        (0.5, rgba(112, 116, 126, 255)),
        (0.68, rgba(176, 180, 190, 255)),
        (1.0, rgba(78, 82, 92, 255)),
    ])
}

/// Preferências: placa de alumínio escovado com um poço redondo e, dentro,
/// uma engrenagem grande de cinco raios.
pub(crate) fn settings(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let tile = u.rr(0.07, 0.08, 0.86, 0.84, 0.1);
    soft_shadow(pm, tile.clone(), s, 1.0);
    pm.fill(tile.clone(), &vgrad(u.py(0.08), u.py(0.92), &[(0.0, rgba(222, 226, 232, 255)), (0.5, rgba(178, 182, 190, 255)), (1.0, rgba(120, 124, 132, 255))]));
    crate::tiger::brush_streaks(pm, tile.clone(), u, 13, false);
    pm.fill_clipped(u.rr(0.07, 0.08, 0.86, 0.012, 0.0), &solid(rgba(255, 255, 255, 170)), tile.clone());
    pm.stroke(tile, &solid(rgba(84, 88, 96, 220)), u.lw(0.005));
    // Poço: quina clara embaixo, sombra em cima, fundo escuro.
    let (cx, cy) = (0.5, 0.5);
    pm.fill(u.circ(cx, cy, 0.385), &vgrad(u.py(0.115), u.py(0.885), &[(0.0, rgba(80, 84, 92, 255)), (1.0, rgba(246, 247, 250, 255))]));
    let well = u.circ(cx, cy, 0.37);
    pm.fill(well.clone(), &radial(u.px(cx), u.py(cy + 0.04), u.d(0.4), &[(0.0, rgba(86, 90, 98, 255)), (0.75, rgba(56, 59, 66, 255)), (1.0, rgba(32, 34, 40, 255))]));
    pm.fill_clipped(well.clone(), &vgrad(u.py(0.13), u.py(0.3), &[(0.0, rgba(0, 0, 0, 120)), (1.0, rgba(0, 0, 0, 0))]), well.clone());
    // Engrenagem: dentes trapezoidais em volta de um aro; raios e cubo.
    let (ro, rr, rh) = (0.335, 0.285, 0.215);
    let teeth = if s >= 40.0 { 22 } else { 14 };
    let mut pb = PathBuilder::new();
    let step = std::f32::consts::TAU / teeth as f32;
    let at = |a: f32, r: f32| (u.px(cx) + a.cos() * u.d(r), u.py(cy) + a.sin() * u.d(r));
    for i in 0..teeth {
        let a0 = i as f32 * step;
        for (k, (da, r)) in [(0.0, rr), (0.14, ro), (0.42, ro), (0.56, rr)].iter().enumerate() {
            let p = at(a0 + da * step, *r);
            if i == 0 && k == 0 { pb.move_to(p.0, p.1) } else { pb.line_to(p.0, p.1) }
        }
    }
    pb.close();
    pb.push_circle(u.px(cx), u.py(cy), u.d(rh));
    let rim = pb.finish();
    let spokes = {
        let mut sp = PathBuilder::new();
        for k in 0..5 {
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 5.0;
            let (dx, dy) = (a.cos(), a.sin());
            let (nx, ny) = (-dy, dx);
            let w0 = 0.032;
            let w1 = 0.022;
            let p = |along: f32, side: f32| (u.px(cx + dx * along + nx * side), u.py(cy + dy * along + ny * side));
            let q = [p(0.05, -w0), p(rh + 0.02, -w1), p(rh + 0.02, w1), p(0.05, w0)];
            sp.move_to(q[0].0, q[0].1);
            for v in &q[1..] {
                sp.line_to(v.0, v.1);
            }
            sp.close();
        }
        sp.push_circle(u.px(cx), u.py(cy), u.d(0.08));
        sp.finish()
    };
    // Sombra da engrenagem no fundo do poço.
    for p in [&rim, &spokes] {
        pm.fill_rule(shift(p, u.d(0.008), u.d(0.025)), &solid(rgba(0, 0, 0, 110)), tiny_skia::FillRule::EvenOdd);
    }
    let face = steel(u, cx - ro, cy - ro, cx + ro, cy + ro);
    pm.fill_rule(spokes.clone(), &face, tiny_skia::FillRule::Winding);
    pm.fill_rule(rim.clone(), &face, tiny_skia::FillRule::EvenOdd);
    crate::tiger::spun_metal(pm, u.circ(cx, cy, ro), u, cx, cy, ro, 5);
    // Chanfros: claro em cima à esquerda, escuro embaixo à direita.
    let bevel = linear(u.px(cx - ro), u.py(cy - ro), u.px(cx + ro), u.py(cy + ro), &[(0.0, rgba(255, 255, 255, 200)), (0.5, rgba(255, 255, 255, 30)), (1.0, rgba(0, 0, 0, 120))]);
    pm.stroke(u.circ(cx, cy, rr - 0.02), &bevel, u.lw(0.008));
    pm.stroke(rim, &solid(rgba(30, 32, 38, 200)), u.lw(0.005));
    pm.stroke(spokes, &solid(rgba(30, 32, 38, 160)), u.lw(0.004));
    // Cubo abaulado com o furo do eixo.
    pm.fill(u.circ(cx, cy, 0.06), &radial(u.px(cx - 0.02), u.py(cy - 0.025), u.d(0.08), &[(0.0, rgba(250, 251, 253, 255)), (0.6, rgba(150, 154, 164, 255)), (1.0, rgba(90, 94, 104, 255))]));
    pm.fill(u.circ(cx, cy, 0.018), &solid(rgba(30, 32, 38, 255)));
}

/// Utilitário de Disco: o HD com um estetoscópio em cima (a campânula no prato,
/// o tubo de borracha preta, a forquilha e as hastes de metal com as olivas
/// viradas uma para a outra).
pub(crate) fn disk_utility(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    disk(pm, x, y + s * 0.1, s * 0.86);
    let (hx, hy) = (0.378, 0.467);
    let stroke = |w: f32| tiny_skia::Stroke { width: u.lw(w), line_cap: tiny_skia::LineCap::Round, ..Default::default() };
    // Borracha preta: sombra, corpo e o brilho fino de cilindro.
    let rubber = |pm: &mut Canvas, p: Option<Path>, w: f32| {
        pm.fill(shift(&p.clone().and_then(|q| q.stroke(&stroke(w), 1.0)), u.d(0.008), u.d(0.02)), &solid(rgba(0, 0, 0, 50)));
        pm.fill(p.clone().and_then(|q| q.stroke(&stroke(w), 1.0)), &solid(rgba(20, 20, 22, 255)));
        pm.fill(shift(&p.and_then(|q| q.stroke(&stroke(w * 0.22), 1.0)), -u.d(w * 0.18), -u.d(w * 0.18)), &solid(rgba(140, 142, 150, 190)));
    };
    // Haste de metal fina: borda escura, corpo claro e o reflexo.
    let metal = |pm: &mut Canvas, p: Option<Path>| {
        pm.fill(p.clone().and_then(|q| q.stroke(&stroke(0.02), 1.0)), &solid(rgba(96, 100, 110, 255)));
        pm.fill(p.clone().and_then(|q| q.stroke(&stroke(0.013), 1.0)), &solid(rgba(214, 217, 224, 255)));
        pm.fill(shift(&p.and_then(|q| q.stroke(&stroke(0.004), 1.0)), -u.d(0.003), -u.d(0.002)), &solid(rgba(255, 255, 255, 240)));
    };
    let (yx, yy) = (0.8, 0.8);
    rubber(pm, cubic(&[(hx, hy - 0.1), (hx - 0.03, 0.12), (0.6, 0.06), (0.63, 0.34), (0.65, 0.6), (0.7, 0.8), (yx, yy)], u), 0.044);
    // Forquilha: dois tubos curtos de borracha que sobem até as hastes.
    let (la, ra) = ((0.67, 0.56), (0.93, 0.54));
    rubber(pm, cubic(&[(yx, yy), (0.74, 0.74), (0.68, 0.66), la], u), 0.032);
    rubber(pm, cubic(&[(yx, yy), (0.88, 0.74), (0.93, 0.66), ra], u), 0.032);
    pm.fill(u.circ(yx, yy, 0.026), &radial(u.px(yx - 0.008), u.py(yy - 0.01), u.d(0.035), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(100, 104, 114, 255))]));
    // Hastes: sobem quase retas e no alto curvam para dentro.
    let left = cubic(&[la, (0.64, 0.36), (0.6, 0.14), (0.66, 0.08)], u);
    let right = cubic(&[ra, (0.96, 0.34), (0.97, 0.14), (0.9, 0.08)], u);
    metal(pm, left);
    metal(pm, right);
    // Olivas: borracha preta alongada na ponta de cada haste, viradas para dentro.
    for (ex, ey, ang) in [(0.685, 0.075, 15.0f32), (0.875, 0.075, -15.0)] {
        let t = Transform::from_rotate_at(ang, u.px(ex), u.py(ey));
        let tip = crate::tiger::tf(u.ov(ex - 0.032, ey - 0.022, 0.064, 0.044), t);
        pm.fill(tip, &radial(u.px(ex - 0.01), u.py(ey - 0.012), u.d(0.045), &[(0.0, rgba(120, 122, 130, 255)), (0.45, rgba(34, 34, 38, 255)), (1.0, rgba(10, 10, 12, 255))]));
    }
    // Campânula deitada no prato: disco, haste e o topo com a membrana.
    let chrome = |x0: f32, x1: f32| linear(u.px(x0), 0.0, u.px(x1), 0.0, &[(0.0, rgba(110, 114, 124, 255)), (0.3, rgba(250, 251, 253, 255)), (0.6, rgba(170, 174, 184, 255)), (1.0, rgba(96, 100, 110, 255))]);
    let base = u.ov(hx - 0.13, hy - 0.05, 0.26, 0.1);
    soft_shadow(pm, base.clone(), s, 0.6);
    pm.fill(u.ov(hx - 0.13, hy - 0.035, 0.26, 0.1), &solid(rgba(90, 94, 104, 255)));
    pm.fill(base, &chrome(hx - 0.13, hx + 0.13));
    pm.fill(u.rr(hx - 0.035, hy - 0.1, 0.07, 0.1, 0.0), &chrome(hx - 0.035, hx + 0.035));
    let top = u.ov(hx - 0.075, hy - 0.13, 0.15, 0.06);
    pm.fill(top, &chrome(hx - 0.075, hx + 0.075));
    pm.fill(u.ov(hx - 0.055, hy - 0.122, 0.11, 0.044), &solid(rgba(36, 36, 40, 255)));
    pm.fill(u.ov(hx - 0.04, hy - 0.116, 0.08, 0.032), &vgrad(u.py(hy - 0.116), u.py(hy - 0.084), &[(0.0, rgba(200, 202, 210, 255)), (1.0, rgba(250, 250, 252, 255))]));
    pm.fill(u.circ(hx, hy - 0.1, 0.008), &solid(rgba(30, 30, 34, 255)));
}

/// Lixo: cesto de tela preta em losangos (vazado: vê-se a tela de trás), aro
/// de tubo cromado na boca e a cinta cromada no pé. `papers` bolas de papel
/// vão enchendo de baixo para cima; com `TRASH_FULL_AT` passam da borda.
pub(crate) fn trash(pm: &mut Canvas, x: f32, y: f32, s: f32, papers: u8) {
    use crate::tiger::{paper_ball, TRASH_FULL_AT};
    let u = U { x, y, s };
    let (top, bot) = (0.13f32, 0.8f32);
    let rx = |t: f32| 0.39 - 0.07 * t;
    let ry = |t: f32| 0.08 - 0.015 * t;
    let cy = |t: f32| top + (bot - top) * t;
    let at = |t: f32, a: f32| (u.px(0.5 + rx(t) * a.cos()), u.py(cy(t) + ry(t) * a.sin()));
    let base_h = 0.075;
    let (bx0, bx1) = (0.5 - rx(1.0), 0.5 + rx(1.0));
    // Sombra no chão e o interior: um véu claro (a tela é vazada) e o fundo escuro espelhado.
    soft_shadow(pm, u.ov(bx0, cy(1.0) + base_h - ry(1.0), rx(1.0) * 2.0, ry(1.0) * 2.0), s, 1.4);
    let silhouette = {
        let mut pb = PathBuilder::new();
        let n = 24;
        for k in 0..=n {
            let (px, py) = at(0.0, std::f32::consts::PI + k as f32 / n as f32 * std::f32::consts::PI);
            if k == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        for k in 0..=n {
            let (px, py) = at(1.0, k as f32 / n as f32 * std::f32::consts::PI);
            pb.line_to(px, py);
        }
        pb.close();
        pb.finish()
    };
    pm.fill(silhouette, &vgrad(u.py(top), u.py(bot), &[(0.0, rgba(250, 251, 253, 70)), (1.0, rgba(200, 204, 214, 40))]));
    pm.fill(u.ov(bx0, cy(1.0) - ry(1.0), rx(1.0) * 2.0, ry(1.0) * 2.0), &linear(u.px(bx0), u.py(cy(1.0) - ry(1.0)), u.px(bx1), u.py(cy(1.0) + ry(1.0)), &[(0.0, rgba(30, 30, 36, 255)), (0.45, rgba(110, 114, 130, 255)), (1.0, rgba(24, 24, 28, 255))]));
    // Tela: hélices nos dois sentidos (losangos). Separa a parte de trás e a da frente.
    let n = if s < 28.0 { 12 } else if s < 64.0 { 20 } else { 28 };
    let twist = 1.7f32;
    let (mut back, mut front) = (PathBuilder::new(), PathBuilder::new());
    let (mut last_b, mut last_f): (Option<(f32, f32)>, Option<(f32, f32)>) = (None, None);
    for sign in [1.0f32, -1.0] {
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let m = 28;
            let mut prev = at(0.0, a0);
            for j in 1..=m {
                let t = j as f32 / m as f32;
                let a = a0 + sign * twist * t;
                let cur = at(t, a);
                let mid = a - sign * twist * 0.5 / m as f32;
                let (pb, last) = if mid.sin() > 0.0 { (&mut front, &mut last_f) } else { (&mut back, &mut last_b) };
                if *last != Some(prev) {
                    pb.move_to(prev.0, prev.1);
                }
                pb.line_to(cur.0, cur.1);
                *last = Some(cur);
                prev = cur;
            }
            last_b = None;
            last_f = None;
        }
    }
    pm.stroke(back.finish(), &solid(rgba(44, 44, 50, 220)), u.lw(0.007));
    // Papéis: fileiras de baixo para cima, dentro do cesto.
    let caps = [3u8, 3, 4, 4, 3, 3];
    let mut left = papers.min(TRASH_FULL_AT);
    let r = 0.08;
    'rows: for (j, cap) in caps.iter().enumerate() {
        let yy = 0.73 - j as f32 * 0.1;
        let tt = ((yy + r * 0.6 - top) / (bot - top)).clamp(0.0, 1.0);
        let w = rx(tt) - r * 1.05;
        for k in 0..*cap {
            if left == 0 {
                break 'rows;
            }
            left -= 1;
            let order: &[f32] = if *cap == 3 { &[0.5, 0.0, 1.0] } else { &[0.33, 0.67, 0.0, 1.0] };
            let f = order[k as usize];
            let jit = (hash(j as u32 * 17 + k as u32) & 0xff) as f32 / 255.0 - 0.5;
            paper_ball(pm, u, 0.5 - w + f * w * 2.0 + jit * 0.03, yy + jit * 0.025, r, (j * 7 + k as usize) as u32 + 1);
        }
    }
    let full = papers >= TRASH_FULL_AT;
    if full {
        paper_ball(pm, u, 0.36, 0.13, 0.09, 51);
        paper_ball(pm, u, 0.63, 0.12, 0.095, 57);
        paper_ball(pm, u, 0.5, 0.085, 0.08, 63);
    }
    // Tela da frente: arame preto com um fio de brilho.
    let fr = front.finish();
    pm.stroke(fr.clone(), &solid(rgba(14, 14, 16, 255)), u.lw(0.012));
    if s >= 48.0 {
        pm.stroke(shift(&fr, -u.d(0.002), -u.d(0.003)), &solid(rgba(150, 152, 162, 150)), u.lw(0.003));
    }
    // Cinta cromada do pé (um cilindro curto).
    let band = {
        let mut pb = PathBuilder::new();
        let k = 24;
        for i in 0..=k {
            let (px, py) = at(1.0, std::f32::consts::PI + i as f32 / k as f32 * std::f32::consts::PI);
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        for i in 0..=k {
            let (px, py) = at(1.0, i as f32 / k as f32 * std::f32::consts::PI);
            pb.line_to(px, py + u.d(base_h) * if i == 0 || i == k { 1.0 } else { 1.0 });
        }
        pb.close();
        pb.finish()
    };
    let chrome_h = |x0: f32, x1: f32| linear(u.px(x0), 0.0, u.px(x1), 0.0, &[(0.0, rgba(86, 88, 96, 255)), (0.18, rgba(226, 228, 234, 255)), (0.32, rgba(255, 255, 255, 255)), (0.55, rgba(160, 164, 174, 255)), (0.8, rgba(224, 226, 232, 255)), (1.0, rgba(92, 94, 102, 255))]);
    pm.fill(band.clone(), &chrome_h(bx0, bx1));
    pm.fill_clipped(band.clone(), &vgrad(u.py(cy(1.0)), u.py(cy(1.0) + base_h + ry(1.0)), &[(0.0, rgba(255, 255, 255, 40)), (0.6, rgba(0, 0, 0, 0)), (1.0, rgba(0, 0, 0, 70))]), band.clone());
    pm.stroke(band, &solid(rgba(60, 62, 70, 200)), u.lw(0.004));
    // Aro de tubo cromado na boca.
    let ring = {
        let mut pb = PathBuilder::new();
        let k = 40;
        let span = if full { std::f32::consts::PI } else { std::f32::consts::TAU };
        for i in 0..=k {
            let (px, py) = at(0.0, i as f32 / k as f32 * span);
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        if !full {
            pb.close();
        }
        pb.finish()
    };
    pm.stroke(ring.clone(), &solid(rgba(56, 58, 66, 255)), u.lw(0.046));
    pm.stroke(ring.clone(), &chrome_h(0.5 - rx(0.0), 0.5 + rx(0.0)), u.lw(0.036));
    pm.stroke(shift(&ring, 0.0, -u.d(0.008)), &solid(rgba(255, 255, 255, 170)), u.lw(0.008));
}

/// Música: CD com o arco-íris e uma nota dupla de vidro verde na frente.
pub(crate) fn music(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::optical(pm, x - s * 0.06, y, s * 0.94);
    let (n1, n2) = ((0.56, 0.78), (0.86, 0.7));
    let note = {
        let mut pb = PathBuilder::new();
        for (cx, cy) in [n1, n2] {
            // Cabeça oval inclinada.
            let t = Transform::from_rotate_at(-22.0, u.px(cx), u.py(cy));
            if let Some(p) = u.ov(cx - 0.085, cy - 0.06, 0.17, 0.12).and_then(|p| p.transform(t)) {
                pb.push_path(&p);
            }
        }
        let (s1, s2) = (n1.0 + 0.065, n2.0 + 0.065);
        for (sx, y0, y1) in [(s1, 0.3, n1.1 - 0.02), (s2, 0.22, n2.1 - 0.02)] {
            if let Some(r) = tiny_skia::Rect::from_ltrb(u.px(sx - 0.028), u.py(y0), u.px(sx), u.py(y1)) {
                pb.push_rect(r);
            }
        }
        pb.move_to(u.px(s1 - 0.028), u.py(0.3));
        pb.line_to(u.px(s2), u.py(0.2));
        pb.line_to(u.px(s2), u.py(0.3));
        pb.line_to(u.px(s1 - 0.028), u.py(0.4));
        pb.close();
        pb.finish()
    };
    soft_shadow(pm, note.clone(), s, 0.9);
    // Gel: a borda escura só por fora (o traço largo por baixo, o recheio por
    // cima apaga as emendas entre as peças), a cor e a luz que atravessa.
    let st = tiny_skia::Stroke { width: u.lw(0.014), line_join: tiny_skia::LineJoin::Round, ..Default::default() };
    pm.fill(note.clone().and_then(|p| p.stroke(&st, 1.0)), &solid(rgba(12, 92, 26, 255)));
    pm.fill(note.clone(), &solid(rgba(12, 92, 26, 255)));
    pm.fill(note.clone(), &linear(u.px(0.5), u.py(0.18), u.px(0.86), u.py(0.86), &[(0.0, rgba(150, 236, 110, 255)), (0.45, rgba(64, 190, 56, 255)), (1.0, rgba(24, 132, 36, 255))]));
    // Luz que atravessa o gel: mais clara embaixo de cada cabeça.
    for (cx, cy) in [n1, n2] {
        pm.fill_clipped(u.circ(cx, cy + 0.02, 0.09), &radial(u.px(cx), u.py(cy + 0.05), u.d(0.09), &[(0.0, rgba(210, 255, 170, 170)), (1.0, rgba(210, 255, 170, 0))]), note.clone());
    }
    // Brilhos: um oval em cada cabeça, um fio em cada haste e a faixa na barra.
    for (cx, cy) in [n1, n2] {
        let t = Transform::from_rotate_at(-22.0, u.px(cx), u.py(cy));
        let g = crate::tiger::tf(u.ov(cx - 0.06, cy - 0.05, 0.09, 0.04), t);
        pm.fill_clipped(g, &vgrad(u.py(cy - 0.05), u.py(cy - 0.01), &[(0.0, rgba(255, 255, 255, 220)), (1.0, rgba(255, 255, 255, 30))]), note.clone());
    }
    let (s1, s2) = (n1.0 + 0.065, n2.0 + 0.065);
    for (sx, y0, y1) in [(s1, 0.37, n1.1 - 0.06), (s2, 0.28, n2.1 - 0.06)] {
        pm.fill(u.rr(sx - 0.022, y0, 0.008, y1 - y0, 0.004), &solid(rgba(255, 255, 255, 150)));
    }
    let beam_gloss = {
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(s1 - 0.02), u.py(0.31));
        pb.line_to(u.px(s2 - 0.01), u.py(0.215));
        pb.line_to(u.px(s2 - 0.01), u.py(0.245));
        pb.line_to(u.px(s1 - 0.02), u.py(0.34));
        pb.close();
        pb.finish()
    };
    pm.fill_clipped(beam_gloss, &solid(rgba(255, 255, 255, 160)), note);
}

/// Editor: folha de carta com o texto escrito e uma caneta-tinteiro preta com
/// anel e pena prateados, na diagonal.
pub(crate) fn editor(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let t = Transform::from_rotate_at(-6.0, u.px(0.45), u.py(0.5));
    let sheet = crate::tiger::tf(u.rr(0.14, 0.07, 0.6, 0.82, 0.006), t);
    soft_shadow(pm, sheet.clone(), s, 0.9);
    pm.fill(crate::tiger::tf(u.rr(0.17, 0.09, 0.6, 0.82, 0.006), t), &solid(rgba(222, 222, 218, 255)));
    pm.fill(sheet.clone(), &linear(u.px(0.14), u.py(0.07), u.px(0.74), u.py(0.89), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(236, 236, 232, 255))]));
    if s >= 28.0 {
        // Saudação, parágrafos e a assinatura (linhas de texto em cinza-escuro).
        let mut txt = PathBuilder::new();
        let mut line = |x0: f32, w: f32, yy: f32| {
            if let Some(r) = tiny_skia::Rect::from_xywh(u.px(x0), u.py(yy), u.d(w), u.d(0.014)) {
                txt.push_rect(r);
            }
        };
        line(0.21, 0.18, 0.15);
        let mut yy = 0.22;
        for k in 0..9u32 {
            if k == 4 {
                yy += 0.03;
            }
            let w = if k == 3 || k == 8 { 0.22 } else { 0.4 + (hash(k + 3) & 0x3f) as f32 / 63.0 * 0.06 };
            line(if k == 0 || k == 4 { 0.25 } else { 0.21 }, w, yy);
            yy += 0.045;
        }
        pm.fill(crate::tiger::tf(txt.finish(), t), &solid(rgba(70, 70, 78, 220)));
        let sig = cubic(&[(0.42, 0.8), (0.46, 0.74), (0.5, 0.84), (0.54, 0.78), (0.57, 0.74), (0.6, 0.82), (0.66, 0.78)], u);
        pm.stroke(crate::tiger::tf(sig, t), &solid(rgba(20, 40, 120, 220)), u.lw(0.008));
    }
    pm.stroke(sheet, &solid(rgba(160, 160, 156, 255)), u.lw(0.004));
    // Caneta-tinteiro: corpo de laca preta, anel prateado, pena com fenda.
    let pt = Transform::from_rotate_at(-48.0, u.px(0.62), u.py(0.58));
    let body = crate::tiger::tf(u.rr(0.44, 0.545, 0.48, 0.075, 0.035), pt);
    pm.fill(crate::tiger::tf(u.rr(0.45, 0.57, 0.48, 0.075, 0.035), pt), &solid(rgba(0, 0, 0, 70)));
    let lacquer = crate::tiger::tpaint(vgrad(u.py(0.545), u.py(0.62), &[(0.0, rgba(110, 112, 122, 255)), (0.3, rgba(30, 30, 36, 255)), (0.7, rgba(10, 10, 12, 255)), (1.0, rgba(50, 50, 56, 255))]), pt);
    pm.fill(body, &lacquer);
    let silver = crate::tiger::tpaint(vgrad(u.py(0.545), u.py(0.62), &[(0.0, rgba(250, 251, 253, 255)), (0.5, rgba(150, 154, 164, 255)), (1.0, rgba(220, 222, 228, 255))]), pt);
    pm.fill(crate::tiger::tf(u.rr(0.6, 0.542, 0.035, 0.081, 0.004), pt), &silver);
    pm.fill(crate::tiger::tf(u.rr(0.82, 0.565, 0.1, 0.012, 0.004), pt), &silver);
    // Seção e pena
    pm.fill(crate::tiger::tf(u.poly(&[(0.44, 0.553), (0.44, 0.612), (0.38, 0.6), (0.38, 0.565)]), pt), &lacquer);
    let nib = crate::tiger::tf(u.poly(&[(0.38, 0.565), (0.38, 0.6), (0.27, 0.585), (0.26, 0.582)]), pt);
    pm.fill(nib.clone(), &silver);
    pm.stroke(crate::tiger::tf(u.poly(&[(0.37, 0.582), (0.3, 0.583)]), pt), &solid(rgba(40, 40, 46, 255)), u.lw(0.004));
    pm.stroke(nib, &solid(rgba(90, 94, 104, 255)), u.lw(0.003));
}

/// Xadrez: torre de madeira dourada envernizada na casa da ponta de um
/// tabuleiro 2×2 cinza e branco. Peça torneada (pé, bojo, coluna, anel e a
/// coroa com as ameias) com as elipses na mesma inclinação do tabuleiro.
pub(crate) fn chess(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    // Tabuleiro: losango com 2×2 casas e a espessura. Achatamento: 0,42.
    let (t, r, b, l) = ((0.5, 0.52), (0.98, 0.72), (0.5, 0.92), (0.02, 0.72));
    let board = u.poly(&[t, r, b, l]);
    soft_shadow(pm, board.clone(), s, 0.9);
    pm.fill(u.poly(&[l, b, (0.5, 0.97), (0.02, 0.77)]), &vgrad(u.py(0.72), u.py(0.97), &[(0.0, rgba(170, 172, 178, 255)), (1.0, rgba(110, 112, 118, 255))]));
    pm.fill(u.poly(&[b, r, (0.98, 0.77), (0.5, 0.97)]), &vgrad(u.py(0.72), u.py(0.97), &[(0.0, rgba(130, 132, 138, 255)), (1.0, rgba(84, 86, 92, 255))]));
    let mid = |p: (f32, f32), q: (f32, f32)| ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
    let c = mid(t, b);
    let (tr, rb, bl, lt) = (mid(t, r), mid(r, b), mid(b, l), mid(l, t));
    let light = linear(u.px(0.2), u.py(0.52), u.px(0.8), u.py(0.92), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(222, 224, 228, 255))]);
    let dark = linear(u.px(0.2), u.py(0.52), u.px(0.8), u.py(0.92), &[(0.0, rgba(176, 178, 184, 255)), (1.0, rgba(130, 132, 140, 255))]);
    pm.fill(u.poly(&[t, tr, c, lt]), &light);
    pm.fill(u.poly(&[c, rb, b, bl]), &light);
    pm.fill(u.poly(&[tr, r, rb, c]), &dark);
    pm.fill(u.poly(&[lt, c, bl, l]), &dark);
    pm.stroke(board, &solid(rgba(110, 112, 120, 200)), u.lw(0.004));

    let k = 0.42; // achatamento das elipses (o mesmo do tabuleiro)
    let cx = 0.5;
    let foot_y = mid(c, b).1; // centro da casa da frente
    let gold = |rx: f32| linear(u.px(cx - rx), 0.0, u.px(cx + rx), 0.0, &[
        (0.0, rgba(86, 54, 22, 255)),
        (0.2, rgba(180, 132, 66, 255)),
        (0.36, rgba(246, 218, 156, 255)),
        (0.55, rgba(204, 156, 86, 255)),
        (0.82, rgba(126, 84, 36, 255)),
        (1.0, rgba(66, 40, 14, 255)),
    ]);
    let edge = rgba(54, 32, 12, 220);
    let ell = |yc: f32, rx: f32| u.ov(cx - rx, yc - rx * k, rx * 2.0, rx * k * 2.0);
    // Sólido de revolução entre duas alturas: lateral (curva) e a frente da elipse de baixo.
    let turned = |pm: &mut Canvas, y0: f32, r0: f32, y1: f32, r1: f32, bulge: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(cx - r0), u.py(y0));
        pb.quad_to(u.px(cx - r1.max(r0) - bulge), u.py((y0 + y1) / 2.0), u.px(cx - r1), u.py(y1));
        let n = 20;
        for i in 1..=n {
            let a = std::f32::consts::PI - i as f32 / n as f32 * std::f32::consts::PI;
            pb.line_to(u.px(cx + a.cos() * r1), u.py(y1 + a.sin() * r1 * k));
        }
        pb.quad_to(u.px(cx + r1.max(r0) + bulge), u.py((y0 + y1) / 2.0), u.px(cx + r0), u.py(y0));
        pb.close();
        let p = pb.finish();
        let rr = r0.max(r1) + bulge;
        pm.fill(p.clone(), &gold(rr));
        pm.fill_clipped(p.clone(), &vgrad(u.py(y0), u.py(y1 + r1 * k), &[(0.0, rgba(0, 0, 0, 40)), (0.5, rgba(0, 0, 0, 0)), (1.0, rgba(0, 0, 0, 80))]), p.clone());
        pm.stroke(p, &solid(edge), u.lw(0.004));
        // Tampo (a elipse de cima, mais clara).
        let top = ell(y0, r0);
        pm.fill(top.clone(), &linear(u.px(cx - r0), u.py(y0 - r0 * k), u.px(cx + r0), u.py(y0 + r0 * k), &[(0.0, rgba(250, 228, 180, 255)), (1.0, rgba(190, 142, 78, 255))]));
        pm.stroke(top, &solid(rgba(90, 56, 22, 160)), u.lw(0.003));
    };
    // Sombra no tabuleiro
    pm.fill(u.ov(cx - 0.2, foot_y - 0.05, 0.46, 0.13), &radial(u.px(cx + 0.03), u.py(foot_y + 0.01), u.d(0.24), &[(0.0, rgba(0, 0, 0, 110)), (1.0, rgba(0, 0, 0, 0))]));
    // Torre: pé, bojo, coluna que afina, anel, e a coroa com as ameias.
    turned(pm, foot_y - 0.03, 0.19, foot_y, 0.19, 0.0);
    turned(pm, foot_y - 0.1, 0.15, foot_y - 0.03, 0.175, 0.02);
    turned(pm, foot_y - 0.12, 0.155, foot_y - 0.1, 0.155, 0.0);
    let col_top = foot_y - 0.44;
    turned(pm, col_top, 0.105, foot_y - 0.12, 0.135, 0.0);
    turned(pm, col_top - 0.03, 0.14, col_top + 0.005, 0.12, 0.0);
    let crown_top = col_top - 0.17;
    let rc = 0.145;
    turned(pm, crown_top, rc, col_top - 0.03, rc, 0.0);
    // Boca da coroa: o vão escuro no meio e a borda clara em volta.
    let inner = 0.095;
    pm.fill(ell(crown_top + 0.004, inner), &vgrad(u.py(crown_top - inner * k), u.py(crown_top + inner * k), &[(0.0, rgba(40, 22, 8, 255)), (1.0, rgba(110, 72, 34, 255))]));
    // Ameias: quatro vãos na borda (dois na frente, um de cada lado atrás).
    let notch = |pm: &mut Canvas, a: f32| {
        let (ca, sa) = (a.cos(), a.sin());
        let (nx, ny) = (cx + ca * rc, crown_top + sa * rc * k);
        let wd = 0.035 * sa.abs().max(0.35);
        let depth = 0.05;
        let front = sa > 0.0;
        let gap = u.poly(&[(nx - wd, ny - 0.006), (nx + wd, ny - 0.006), (nx + wd, ny + if front { depth } else { 0.012 }), (nx - wd, ny + if front { depth } else { 0.012 })]);
        let col = if front { rgba(70, 40, 14, 255) } else { rgba(50, 28, 10, 255) };
        pm.fill(gap.clone(), &vgrad(u.py(ny), u.py(ny + depth), &[(0.0, col), (1.0, rgba(120, 80, 40, 255))]));
        // A parede de dentro do vão (a outra borda da coroa) aparece por ele.
        if front {
            pm.fill(u.rr(nx - wd, ny + depth - 0.012, wd * 2.0, 0.012, 0.0), &solid(rgba(236, 200, 140, 255)));
        }
    };
    for a in [-2.1f32, -1.05, 1.05, 2.1] {
        notch(pm, a);
    }
    // Verniz: fio de luz descendo pela coluna.
    pm.fill(u.rr(cx - 0.055, col_top + 0.02, 0.016, foot_y - 0.15 - col_top, 0.008), &solid(rgba(255, 245, 220, 110)));
}

/// Contatos: caderno de endereços de couro marrom, com a costura em volta da
/// capa e um "@" grande em relevo dourado; as folhas creme aparecem do lado.
pub(crate) fn contacts(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let t = Transform::from_rotate_at(5.0, u.px(0.5), u.py(0.5));
    let tfp = |p: Option<Path>| crate::tiger::tf(p, t);
    let cover = tfp(u.rr(0.17, 0.07, 0.64, 0.84, 0.035));
    soft_shadow(pm, cover.clone(), s, 1.0);
    pm.fill(tfp(u.rr(0.21, 0.1, 0.64, 0.83, 0.03)), &vgrad(u.py(0.1), u.py(0.93), &[(0.0, rgba(250, 244, 228, 255)), (1.0, rgba(222, 212, 190, 255))]));
    if s >= 40.0 {
        let mut pages = PathBuilder::new();
        for i in 1..5 {
            let xx = 0.81 + i as f32 * 0.008;
            pages.move_to(u.px(xx), u.py(0.13));
            pages.line_to(u.px(xx), u.py(0.9));
        }
        pm.stroke(tfp(pages.finish()), &solid(rgba(190, 178, 152, 200)), u.lw(0.003));
    }
    let leather = linear(u.px(0.17), u.py(0.07), u.px(0.81), u.py(0.91), &[(0.0, rgba(168, 112, 66, 255)), (0.55, rgba(124, 74, 38, 255)), (1.0, rgba(84, 46, 20, 255))]);
    pm.fill(cover.clone(), &leather);
    crate::tiger::leather_grain(pm, cover.clone(), u, 43);
    // Lombada mais escura com o vinco.
    pm.fill_clipped(tfp(u.rr(0.17, 0.07, 0.09, 0.84, 0.0)), &linear(u.px(0.17), 0.0, u.px(0.26), 0.0, &[(0.0, rgba(50, 26, 10, 255)), (1.0, rgba(100, 60, 30, 255))]), cover.clone());
    pm.fill_clipped(tfp(u.rr(0.26, 0.07, 0.008, 0.84, 0.0)), &solid(rgba(220, 170, 120, 90)), cover.clone());
    // Costura pontilhada em volta da capa.
    if s >= 40.0 {
        let st = tiny_skia::Stroke { width: u.lw(0.006), dash: tiny_skia::StrokeDash::new(vec![u.d(0.022), u.d(0.016)], 0.0), ..Default::default() };
        let seam = tfp(u.rr(0.3, 0.11, 0.47, 0.76, 0.02));
        pm.fill(seam.clone().and_then(|p| p.stroke(&st, 1.0)).and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.004)))), &solid(rgba(40, 20, 6, 160)));
        pm.fill(seam.and_then(|p| p.stroke(&st, 1.0)), &solid(rgba(236, 210, 170, 220)));
    }
    pm.stroke(cover, &solid(rgba(50, 26, 10, 255)), u.lw(0.006));
    // "@" dourado em relevo: sombra afundada, fio claro e o metal.
    if s >= 20.0 {
        let px = u.d(0.42);
        let tw = fonts.measure("@", px, true);
        let (ax, ay) = (u.px(0.54) - tw / 2.0, u.py(0.62));
        fonts.draw(pm, "@", ax + u.d(0.006), ay + u.d(0.01), px, true, rgba(40, 20, 6, 200));
        fonts.draw(pm, "@", ax - u.d(0.004), ay - u.d(0.005), px, true, rgba(255, 240, 200, 200));
        fonts.draw(pm, "@", ax, ay, px, true, rgba(222, 176, 86, 255));
    }
    // Fita marcadora saindo embaixo.
    pm.fill(tfp(u.poly(&[(0.62, 0.89), (0.67, 0.89), (0.67, 0.99), (0.645, 0.96), (0.62, 0.99)])), &solid(rgba(200, 160, 60, 255)));
}

/// Correio: selo grande picotado e um pouco inclinado, com uma andorinha
/// voando no céu azul, o valor no canto e o carimbo ondulado por cima.
pub(crate) fn mail(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let t = Transform::from_rotate_at(-8.0, u.px(0.5), u.py(0.5));
    let tfp = |p: Option<Path>| crate::tiger::tf(p, t);
    let (x0, y0, x1, y1) = (0.12, 0.1, 0.88, 0.9);
    // Papel do selo com o picote: círculos tirados da borda (par/ímpar).
    let mut pb = PathBuilder::new();
    if let Some(r) = tiny_skia::Rect::from_ltrb(u.px(x0), u.py(y0), u.px(x1), u.py(y1)) {
        pb.push_rect(r);
    }
    let holes = if s >= 40.0 { 9 } else { 5 };
    let hr = (y1 - y0) / holes as f32 * 0.28;
    for i in 0..=holes {
        let f = i as f32 / holes as f32;
        for (hx, hy) in [(x0 + f * (x1 - x0), y0), (x0 + f * (x1 - x0), y1), (x0, y0 + f * (y1 - y0)), (x1, y0 + f * (y1 - y0))] {
            pb.push_circle(u.px(hx), u.py(hy), u.d(hr));
        }
    }
    let paper = tfp(pb.finish());
    let paper_fill = |pm: &mut Canvas, p: Option<Path>, paint: &Paint| {
        if let Some(p) = p {
            pm.fill_rule(Some(p), paint, tiny_skia::FillRule::EvenOdd);
        }
    };
    soft_shadow(pm, tfp(u.rr(x0, y0, x1 - x0, y1 - y0, 0.0)), s, 0.9);
    paper_fill(pm, paper.clone(), &solid(rgba(252, 250, 244, 255)));
    // Imagem: céu azul, morros ao longe e a andorinha.
    let (ix0, iy0, ix1, iy1) = (x0 + 0.07, y0 + 0.07, x1 - 0.07, y1 - 0.15);
    let img = tfp(u.rr(ix0, iy0, ix1 - ix0, iy1 - iy0, 0.0));
    pm.fill(img.clone(), &vgrad(u.py(iy0), u.py(iy1), &[(0.0, rgba(60, 120, 210, 255)), (0.7, rgba(150, 200, 245, 255)), (1.0, rgba(220, 236, 250, 255))]));
    let hills = tfp(cubic(&[(ix0, iy1 - 0.08), (ix0 + 0.2, iy1 - 0.2), (ix0 + 0.35, iy1 - 0.02), (ix1, iy1 - 0.12)], u).and_then(|p| {
        let mut pb = PathBuilder::new();
        pb.push_path(&p);
        pb.line_to(u.px(ix1), u.py(iy1));
        pb.line_to(u.px(ix0), u.py(iy1));
        pb.close();
        pb.finish()
    }));
    pm.fill_clipped(hills, &vgrad(u.py(iy1 - 0.2), u.py(iy1), &[(0.0, rgba(110, 150, 190, 255)), (1.0, rgba(70, 110, 160, 255))]), img.clone());
    // Andorinha: asas em arco, corpo e a cauda bifurcada.
    let bird = {
        let mut pb = PathBuilder::new();
        let p = |a: f32, b: f32| (u.px(a), u.py(b));
        let mv = |pb: &mut PathBuilder, a: (f32, f32)| { let q = p(a.0, a.1); pb.move_to(q.0, q.1); };
        let qd = |pb: &mut PathBuilder, c: (f32, f32), e: (f32, f32)| { let (c, e) = (p(c.0, c.1), p(e.0, e.1)); pb.quad_to(c.0, c.1, e.0, e.1); };
        let ln = |pb: &mut PathBuilder, a: (f32, f32)| { let q = p(a.0, a.1); pb.line_to(q.0, q.1); };
        // De lado, voando para a direita: asa de trás para cima, a da frente
        // para baixo, cauda bifurcada atrás, corpo e cabeça.
        mv(&mut pb, (0.48, 0.45));
        qd(&mut pb, (0.42, 0.3), (0.3, 0.2));
        qd(&mut pb, (0.47, 0.27), (0.57, 0.44));
        pb.close();
        mv(&mut pb, (0.5, 0.5));
        qd(&mut pb, (0.45, 0.64), (0.33, 0.73));
        qd(&mut pb, (0.51, 0.66), (0.58, 0.5));
        pb.close();
        mv(&mut pb, (0.41, 0.46));
        ln(&mut pb, (0.2, 0.4));
        ln(&mut pb, (0.33, 0.49));
        ln(&mut pb, (0.21, 0.58));
        ln(&mut pb, (0.41, 0.51));
        pb.close();
        if let Some(b) = u.ov(0.38, 0.44, 0.28, 0.08) {
            pb.push_path(&b);
        }
        pb.push_circle(u.px(0.66), u.py(0.465), u.d(0.038));
        mv(&mut pb, (0.69, 0.45));
        ln(&mut pb, (0.75, 0.47));
        ln(&mut pb, (0.69, 0.48));
        pb.close();
        pb.finish()
    };
    pm.fill_rule(tfp(bird.clone()), &linear(u.px(0.3), u.py(0.3), u.px(0.6), u.py(0.6), &[(0.0, rgba(30, 40, 70, 255)), (1.0, rgba(10, 16, 30, 255))]), tiny_skia::FillRule::Winding);
    glass_glare_soft(pm, img.clone(), u, iy0, iy1);
    pm.stroke(img, &solid(rgba(40, 70, 120, 200)), u.lw(0.004));
    // Valor no canto de baixo.
    if s >= 40.0 {
        let px = u.d(0.09);
        fonts.draw(pm, "1,20", u.px(x0 + 0.08), u.py(y1 - 0.04), px, true, rgba(40, 60, 110, 255));
    }
    // Carimbo: círculo e ondas cinza por cima (fora do selo também).
    let ink = solid(rgba(40, 44, 56, 150));
    let st = |w: f32| tiny_skia::Stroke { width: u.lw(w), ..Default::default() };
    pm.fill(tfp(u.circ(0.28, 0.24, 0.14)).and_then(|p| p.stroke(&st(0.012), 1.0)), &ink);
    for i in 0..4 {
        let yy = 0.16 + i as f32 * 0.05;
        let mut w = PathBuilder::new();
        w.move_to(u.px(0.44), u.py(yy));
        for j in 1..=6 {
            let xx = 0.44 + j as f32 * 0.08;
            w.quad_to(u.px(xx - 0.04), u.py(yy + if j % 2 == 0 { 0.025 } else { -0.025 }), u.px(xx), u.py(yy));
        }
        pm.fill(tfp(w.finish()).and_then(|p| p.stroke(&st(0.01), 1.0)), &ink);
    }
}

fn glass_glare_soft(pm: &mut Canvas, clip: Option<Path>, u: U, y0: f32, y1: f32) {
    pm.fill_clipped(clip.clone(), &vgrad(u.py(y0), u.py(y1), &[(0.0, rgba(255, 255, 255, 50)), (0.5, rgba(255, 255, 255, 0))]), clip);
}

/// Fontes: um "F" serifado de metal escovado em relevo, com dois tipos móveis
/// (blocos com a letra) na frente.
pub(crate) fn font_book(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let f = [(0.14, 0.1), (0.74, 0.1), (0.74, 0.3), (0.7, 0.3), (0.66, 0.22), (0.36, 0.22), (0.36, 0.42), (0.54, 0.42), (0.57, 0.36), (0.61, 0.36), (0.61, 0.57), (0.57, 0.57), (0.54, 0.51), (0.36, 0.51), (0.36, 0.76), (0.46, 0.76), (0.46, 0.82), (0.1, 0.82), (0.1, 0.76), (0.2, 0.76), (0.2, 0.16), (0.14, 0.16)];
    let face = u.poly(&f);
    soft_shadow(pm, shift(&face, u.d(0.03), u.d(0.03)), s, 1.0);
    // Espessura: o F repetido para trás e para baixo, em metal escuro.
    for k in (1..=8).rev() {
        let d = k as f32 * 0.005;
        pm.fill(shift(&face, u.d(d), u.d(d)), &solid(rgba((70 + k * 4) as u8, (72 + k * 4) as u8, (80 + k * 4) as u8, 255)));
    }
    pm.fill(face.clone(), &linear(u.px(0.1), u.py(0.1), u.px(0.74), u.py(0.82), &[(0.0, rgba(250, 251, 253, 255)), (0.4, rgba(200, 203, 210, 255)), (0.6, rgba(170, 174, 182, 255)), (1.0, rgba(130, 134, 144, 255))]));
    crate::tiger::brush_streaks(pm, face.clone(), u, 37, true);
    pm.stroke(shift(&face, -u.d(0.003), -u.d(0.003)), &solid(rgba(255, 255, 255, 120)), u.lw(0.004));
    pm.stroke(face, &solid(rgba(80, 84, 94, 220)), u.lw(0.004));
    // Tipos móveis: bloco de madeira escura com a letra em cima, visto de lado.
    for (bx, by, w, letter) in [(0.56, 0.66, 0.18, "a"), (0.76, 0.7, 0.16, "g")] {
        let h = w * 0.9;
        let topf = u.poly(&[(bx, by), (bx + w, by), (bx + w + 0.05, by - 0.04), (bx + 0.05, by - 0.04)]);
        let front = u.rr(bx, by, w, h, 0.0);
        let side = u.poly(&[(bx + w, by), (bx + w + 0.05, by - 0.04), (bx + w + 0.05, by + h - 0.04), (bx + w, by + h)]);
        soft_shadow(pm, front.clone(), s, 0.6);
        pm.fill(front.clone(), &vgrad(u.py(by), u.py(by + h), &[(0.0, rgba(196, 160, 110, 255)), (1.0, rgba(140, 100, 60, 255))]));
        pm.fill(side, &solid(rgba(110, 76, 44, 255)));
        pm.fill(topf, &solid(rgba(226, 200, 156, 255)));
        pm.stroke(front.clone(), &solid(rgba(90, 60, 30, 200)), u.lw(0.004));
        if s >= 48.0 {
            let px = u.d(w * 0.8);
            let tw = fonts.measure(letter, px, true);
            fonts.draw(pm, letter, u.px(bx + w / 2.0) - tw / 2.0, u.py(by + h * 0.72), px, true, rgba(60, 36, 16, 230));
        }
    }
}

/// Visualizador: as fotos e uma lupa de vidro com aro cromado e cabo preto
/// em cima delas (o vidro aumenta o pedaço da foto que está embaixo).
pub(crate) fn viewer(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    crate::tiger::picture(pm, x - s * 0.05, y - s * 0.06, s * 0.92);
    let (cx, cy, r) = (0.6, 0.56, 0.2);
    // Cabo preto na diagonal.
    let t = Transform::from_rotate_at(45.0, u.px(cx), u.py(cy));
    let handle = crate::tiger::tf(u.rr(cx + r + 0.02, cy - 0.04, 0.24, 0.08, 0.035), t);
    soft_shadow(pm, handle.clone(), s, 0.8);
    pm.fill(handle, &crate::tiger::tpaint(vgrad(u.py(cy - 0.04), u.py(cy + 0.04), &[(0.0, rgba(110, 112, 122, 255)), (0.35, rgba(30, 30, 36, 255)), (1.0, rgba(8, 8, 10, 255))]), t));
    pm.fill(crate::tiger::tf(u.rr(cx + r - 0.01, cy - 0.03, 0.05, 0.06, 0.01), t), &crate::tiger::tpaint(vgrad(u.py(cy - 0.03), u.py(cy + 0.03), &[(0.0, rgba(250, 251, 253, 255)), (1.0, rgba(120, 124, 134, 255))]), t));
    // Aro cromado
    let ring = u.circ(cx, cy, r + 0.03);
    soft_shadow(pm, ring.clone(), s, 0.7);
    pm.fill(ring, &linear(u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy + r), &[(0.0, rgba(255, 255, 255, 255)), (0.45, rgba(150, 154, 164, 255)), (0.6, rgba(230, 232, 238, 255)), (1.0, rgba(100, 104, 114, 255))]));
    // Vidro: o céu e o morro da foto, maiores (o aumento), com o reflexo.
    let lens = u.circ(cx, cy, r);
    pm.fill(lens.clone(), &vgrad(u.py(cy - r), u.py(cy + r), &[(0.0, rgba(70, 140, 225, 255)), (1.0, rgba(190, 225, 250, 255))]));
    let hill = {
        let mut pb = PathBuilder::new();
        pb.move_to(u.px(cx - r), u.py(cy + 0.06));
        pb.quad_to(u.px(cx - 0.02), u.py(cy - 0.12), u.px(cx + r), u.py(cy + 0.08));
        pb.line_to(u.px(cx + r), u.py(cy + r));
        pb.line_to(u.px(cx - r), u.py(cy + r));
        pb.close();
        pb.finish()
    };
    pm.fill_clipped(hill, &vgrad(u.py(cy - 0.06), u.py(cy + r), &[(0.0, rgba(130, 200, 100, 255)), (1.0, rgba(40, 110, 50, 255))]), lens.clone());
    pm.fill_clipped(lens.clone(), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(0, 0, 0, 0)), (0.8, rgba(0, 0, 0, 0)), (1.0, rgba(0, 20, 50, 90))]), lens.clone());
    pm.fill_clipped(u.ov(cx - r * 0.85, cy - r * 0.95, r * 1.3, r * 0.8), &vgrad(u.py(cy - r), u.py(cy - r * 0.2), &[(0.0, rgba(255, 255, 255, 190)), (1.0, rgba(255, 255, 255, 0))]), lens.clone());
    pm.stroke(lens, &solid(rgba(60, 64, 76, 200)), u.lw(0.005));
}

/// Captura: câmera compacta prateada em cima de fotos impressas.
pub(crate) fn capture(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    crate::tiger::picture(pm, x - s * 0.02, y + s * 0.3, s * 0.72);
    crate::tiger::camera(pm, x + s * 0.12, y - s * 0.06, s * 0.86);
}

// ---------------------------------------------------------------------------
// Lugares da barra lateral
// ---------------------------------------------------------------------------

/// Início: casinha de paredes claras vista um pouco de lado, telhado de
/// ardósia, chaminé, porta de madeira e a janela azul.
pub(crate) fn house(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let front = u.poly(&[(0.1, 0.5), (0.36, 0.26), (0.62, 0.5), (0.62, 0.9), (0.1, 0.9)]);
    let side = u.poly(&[(0.62, 0.5), (0.9, 0.44), (0.9, 0.84), (0.62, 0.9)]);
    soft_shadow(pm, u.poly(&[(0.1, 0.5), (0.36, 0.26), (0.9, 0.2), (0.9, 0.84), (0.62, 0.9), (0.1, 0.9)]), s, 1.0);
    pm.fill(front.clone(), &vgrad(u.py(0.26), u.py(0.9), &[(0.0, rgba(252, 250, 244, 255)), (1.0, rgba(220, 214, 200, 255))]));
    pm.fill(side.clone(), &vgrad(u.py(0.44), u.py(0.9), &[(0.0, rgba(206, 200, 186, 255)), (1.0, rgba(170, 164, 150, 255))]));
    // Chaminé
    pm.fill(u.poly(&[(0.7, 0.14), (0.78, 0.13), (0.78, 0.3), (0.7, 0.32)]), &linear(u.px(0.7), 0.0, u.px(0.78), 0.0, &[(0.0, rgba(170, 80, 60, 255)), (1.0, rgba(120, 50, 36, 255))]));
    // Telhado: a água de cima vai para trás (ardósia azul-acinzentada).
    let roof = u.poly(&[(0.05, 0.53), (0.36, 0.2), (0.66, 0.14), (0.95, 0.44), (0.66, 0.5)]);
    pm.fill(roof.clone(), &linear(u.px(0.3), u.py(0.2), u.px(0.8), u.py(0.5), &[(0.0, rgba(120, 132, 156, 255)), (1.0, rgba(60, 68, 88, 255))]));
    let gable = u.poly(&[(0.05, 0.53), (0.36, 0.2), (0.67, 0.53), (0.62, 0.55), (0.36, 0.3), (0.1, 0.55)]);
    pm.fill(gable, &vgrad(u.py(0.2), u.py(0.55), &[(0.0, rgba(90, 100, 124, 255)), (1.0, rgba(50, 56, 74, 255))]));
    if s >= 32.0 {
        let mut rows = PathBuilder::new();
        for k in 1..4 {
            let t = k as f32 / 4.0;
            rows.move_to(u.px(0.36 + 0.3 * t * 1.0 - 0.0), u.py(0.2 + 0.0));
            rows.move_to(u.px(0.36 + t * 0.29), u.py(0.2 + t * 0.32 - 0.06 * t));
            rows.line_to(u.px(0.66 + t * 0.29), u.py(0.14 + t * 0.3));
        }
        pm.stroke(rows.finish(), &solid(rgba(40, 44, 60, 120)), u.lw(0.006));
    }
    pm.fill_clipped(u.poly(&[(0.36, 0.2), (0.66, 0.14), (0.7, 0.2), (0.4, 0.26)]), &solid(rgba(255, 255, 255, 60)), roof);
    // Porta e janelas
    pm.fill(u.rr(0.28, 0.64, 0.14, 0.26, 0.01), &vgrad(u.py(0.64), u.py(0.9), &[(0.0, rgba(150, 90, 50, 255)), (1.0, rgba(100, 56, 28, 255))]));
    pm.fill(u.circ(0.39, 0.78, 0.01), &solid(rgba(240, 210, 120, 255)));
    for (wx, wy, ww, wh) in [(0.15, 0.6, 0.1, 0.1), (0.7, 0.56, 0.12, 0.12)] {
        pm.fill(u.rr(wx - 0.01, wy - 0.01, ww + 0.02, wh + 0.02, 0.0), &solid(rgba(250, 250, 248, 255)));
        pm.fill(u.rr(wx, wy, ww, wh, 0.0), &linear(u.px(wx), u.py(wy), u.px(wx + ww), u.py(wy + wh), &[(0.0, rgba(170, 210, 250, 255)), (1.0, rgba(50, 100, 180, 255))]));
    }
    pm.stroke(front, &solid(rgba(120, 112, 98, 200)), u.lw(0.004));
    pm.stroke(side, &solid(rgba(110, 104, 90, 200)), u.lw(0.004));
}

/// Aplicativos: compasso de desenho técnico de metal, aberto como um "A"
/// (as duas pernas, a articulação com o pino em cima e a barra de ajuste).
pub(crate) fn apps_place(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let leg = |x0: f32, x1: f32, tip: (f32, f32)| u.poly(&[(0.5 + x0, 0.2), (0.5 + x1, 0.22), (tip.0 + 0.04, tip.1 - 0.08), tip, (tip.0 - 0.04, tip.1 - 0.08)]);
    let left = leg(-0.08, 0.0, (0.2, 0.92));
    let right = leg(0.0, 0.08, (0.8, 0.92));
    let steel = |x0: f32, x1: f32| linear(u.px(x0), 0.0, u.px(x1), 0.0, &[(0.0, rgba(90, 94, 106, 255)), (0.4, rgba(250, 251, 253, 255)), (0.7, rgba(170, 174, 184, 255)), (1.0, rgba(100, 104, 116, 255))]);
    for p in [&left, &right] {
        soft_shadow(pm, p.clone(), s, 0.6);
    }
    pm.fill(left.clone(), &steel(0.2, 0.5));
    pm.fill(right.clone(), &steel(0.5, 0.8));
    pm.stroke(left, &solid(rgba(40, 42, 52, 240)), u.lw(0.008));
    pm.stroke(right, &solid(rgba(40, 42, 52, 240)), u.lw(0.008));
    // Ponta de grafite numa perna e a agulha na outra.
    pm.fill(u.poly(&[(0.76, 0.84), (0.82, 0.84), (0.8, 0.96)]), &solid(rgba(50, 50, 56, 255)));
    // Barra de ajuste (o traço do "A") com a roda serrilhada no meio.
    pm.fill(u.rr(0.29, 0.595, 0.42, 0.05, 0.025), &vgrad(u.py(0.6), u.py(0.64), &[(0.0, rgba(250, 251, 253, 255)), (1.0, rgba(120, 124, 134, 255))]));
    pm.fill(u.circ(0.5, 0.62, 0.06), &radial(u.px(0.48), u.py(0.6), u.d(0.08), &[(0.0, rgba(250, 251, 253, 255)), (1.0, rgba(110, 114, 124, 255))]));
    pm.stroke(u.circ(0.5, 0.62, 0.06), &solid(rgba(70, 72, 82, 220)), u.lw(0.006));
    // Articulação e o pino de segurar.
    pm.fill(u.rr(0.465, 0.04, 0.07, 0.12, 0.02), &linear(u.px(0.465), 0.0, u.px(0.535), 0.0, &[(0.0, rgba(30, 30, 34, 255)), (0.4, rgba(100, 100, 108, 255)), (1.0, rgba(20, 20, 24, 255))]));
    pm.fill(u.circ(0.5, 0.2, 0.07), &radial(u.px(0.48), u.py(0.18), u.d(0.09), &[(0.0, rgba(255, 255, 255, 255)), (0.6, rgba(170, 174, 184, 255)), (1.0, rgba(90, 94, 106, 255))]));
    pm.stroke(u.circ(0.5, 0.2, 0.07), &solid(rgba(60, 62, 72, 220)), u.lw(0.005));
}
