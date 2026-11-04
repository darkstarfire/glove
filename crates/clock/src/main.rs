//! Relógio do Glove: mostrador analógico + data por extenso.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Duration;

use glove_kit::{local_time, paint::draw::*, paint::tiny_skia::PathBuilder, run, App, Ui, WindowOptions};

const W: u32 = 260;
const H: u32 = 304;

const DIAS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

/// O mostrador sem ponteiros fica pronto em cache; a cada segundo só
/// copiamos o cache e desenhamos ponteiros e texto por cima.
#[derive(Default)]
struct Clock {
    cache: Option<(f32, u64, Canvas)>,
}

/// Ponteiro afinando na ponta, do centro até `len`, com um rabicho de `tail`.
fn hand(c: &mut Canvas, cx: f32, cy: f32, angle: f32, len: f32, tail: f32, width: f32, color: glove_kit::paint::Color) {
    let (dx, dy) = (angle.cos(), angle.sin());
    let (nx, ny) = (-dy, dx);
    let mut pb = PathBuilder::new();
    pb.move_to(cx - dx * tail + nx * width * 0.5, cy - dy * tail + ny * width * 0.5);
    pb.line_to(cx + dx * len + nx * width * 0.18, cy + dy * len + ny * width * 0.18);
    pb.line_to(cx + dx * len - nx * width * 0.18, cy + dy * len - ny * width * 0.18);
    pb.line_to(cx - dx * tail - nx * width * 0.5, cy - dy * tail - ny * width * 0.5);
    pb.close();
    // Sombra leve deslocada para baixo
    let mut sh = PathBuilder::new();
    sh.move_to(cx - dx * tail + nx * width * 0.5, cy + 2.0 - dy * tail + ny * width * 0.5);
    sh.line_to(cx + dx * len, cy + 2.0 + dy * len);
    sh.line_to(cx - dx * tail - nx * width * 0.5, cy + 2.0 - dy * tail - ny * width * 0.5);
    sh.close();
    c.fill(sh.finish(), &solid(rgba(0, 0, 0, 40)));
    c.fill(pb.finish(), &solid(color));
}

/// Parte fixa do mostrador: aro, fundo, marcas e números.
fn dial(c: &mut Canvas, fonts: &glove_kit::paint::Fonts, cx: f32, cy: f32, r: f32) {
    // Aro: metal escovado em anel + borda de gel.
    c.fill(circle(cx, cy + 3.0, r + 2.0), &radial(cx, cy + 3.0, r + 6.0, &[(0.85, rgba(0, 0, 0, 70)), (1.0, rgba(0, 0, 0, 0))]));
    c.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(245, 246, 250, 255)), (0.5, rgba(170, 174, 185, 255)), (1.0, rgba(235, 236, 240, 255))]));
    c.fill(circle(cx, cy, r * 0.93), &linear(cx + r, cy + r, cx - r, cy - r, &[(0.0, rgba(240, 241, 245, 255)), (1.0, rgba(140, 145, 158, 255))]));
    // Mostrador branco com leve sombra interna no topo.
    let fr = r * 0.88;
    c.fill(circle(cx, cy, fr), &radial(cx, cy + fr * 0.2, fr * 1.1, &[(0.0, rgba(255, 255, 255, 255)), (0.8, rgba(248, 248, 250, 255)), (1.0, rgba(215, 218, 226, 255))]));

    // Marcas de minuto e hora
    for i in 0..60 {
        let a = i as f32 / 60.0 * TAU - FRAC_PI_2;
        let hour = i % 5 == 0;
        let (r0, r1, w) = if hour { (fr * 0.80, fr * 0.94, (r / 45.0).max(1.2)) } else { (fr * 0.89, fr * 0.94, (r / 110.0).max(0.6)) };
        let mut pb = PathBuilder::new();
        pb.move_to(cx + a.cos() * r0, cy + a.sin() * r0);
        pb.line_to(cx + a.cos() * r1, cy + a.sin() * r1);
        c.stroke(pb.finish(), &solid(rgba(40, 42, 50, if hour { 255 } else { 170 })), w);
    }
    // Números (só no tamanho grande)
    if r > 40.0 {
        let px = r * 0.16;
        for n in 1..=12 {
            let a = n as f32 / 12.0 * TAU - FRAC_PI_2;
            let t = n.to_string();
            let tw = fonts.measure(&t, px, false);
            let (x, y) = (cx + a.cos() * fr * 0.66, cy + a.sin() * fr * 0.66);
            fonts.draw(c, &t, x - tw / 2.0, y + px * 0.36, px, false, rgba(40, 42, 50, 255));
        }
    }

}

/// Ponteiros, pino central e o reflexo do vidro (a parte que muda).
fn hands(c: &mut Canvas, cx: f32, cy: f32, r: f32, h: u32, m: u32, s: u32, accent: glove_kit::paint::Color) {
    let fr = r * 0.88;
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * TAU - FRAC_PI_2;
    let ma = (m as f32 + s as f32 / 60.0) / 60.0 * TAU - FRAC_PI_2;
    hand(c, cx, cy, ha, fr * 0.52, fr * 0.1, r * 0.075, rgba(35, 38, 46, 255));
    hand(c, cx, cy, ma, fr * 0.78, fr * 0.12, r * 0.055, rgba(35, 38, 46, 255));
    if r > 40.0 {
        let sa = s as f32 / 60.0 * TAU - FRAC_PI_2;
        hand(c, cx, cy, sa, fr * 0.86, fr * 0.2, r * 0.018, rgba(215, 50, 40, 255));
    }
    gel_circle(c, cx, cy, (r * 0.06).max(2.0), accent);

    // Vidro: reflexo na metade de cima
    c.fill(oval(cx - fr * 0.75, cy - fr * 0.95, fr * 1.5, fr * 0.9), &vgrad(cy - fr, cy - fr * 0.1, &[(0.0, rgba(255, 255, 255, 120)), (1.0, rgba(255, 255, 255, 0))]));
}

impl App for Clock {
    fn ui(&mut self, ui: &mut Ui) {
        let (y, mo, d, wd, h, m, s) = local_time();
        let w = ui.c.w;
        let accent = ui.theme.accent;
        // Refaz o cache só se a escala ou o tema mudarem.
        let key = ui.c.scale;
        let theme_id = accent.red().to_bits() as u64 ^ ((accent.blue().to_bits() as u64) << 32) ^ ui.theme.body_a.red().to_bits() as u64;
        if !matches!(&self.cache, Some((k, t, _)) if *k == key && *t == theme_id) {
            if let Some(mut bg) = Canvas::new(ui.c.w, ui.c.h, key) {
                bg.clear(ui.theme.body_a);
                bg.pinstripes(0.0, 0.0, bg.w, bg.h, ui.theme.body_a, ui.theme.body_b);
                dial(&mut bg, ui.fonts, w / 2.0, 124.0, 108.0);
                self.cache = Some((key, theme_id, bg));
            }
        }
        if let Some((_, _, bg)) = &self.cache {
            ui.c.pm.data_mut().copy_from_slice(bg.pm.data());
        }
        hands(ui.c, w / 2.0, 124.0, 108.0, h, m, s, accent);

        let center = |ui: &mut Ui, t: &str, yy: f32, px: f32, bold: bool| {
            let tw = ui.text_width(t, px, bold);
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0, yy, px, bold, rgba(30, 30, 30, 255));
        };
        center(ui, &format!("{h:02}:{m:02}:{s:02}"), 268.0, 20.0, true);
        let date = format!("{}, {} de {} de {}", DIAS[wd as usize % 7], d, MESES[(mo as usize + 11) % 12], y);
        center(ui, &date, 290.0, 12.0, false);
    }

    fn paints_background(&self) -> bool {
        true
    }

    fn tick(&self) -> Option<Duration> {
        // Acorda exatamente na virada do próximo segundo.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        Some(Duration::from_millis(1000 - (now.subsec_millis() as u64)) + Duration::from_millis(5))
    }
}

fn main() {
    let opts = WindowOptions { title: "Relógio".into(), app_id: "glove.Clock".into(), size: (W, H), fixed: true };
    if let Err(e) = run(opts, Clock::default()) {
        eprintln!("glove-clock: {e}");
    }
}
