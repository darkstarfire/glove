//! Ícones na linguagem do Tiger (Fase 9.11), com arte própria: objetos com
//! volume, um pouco de perspectiva, materiais (metal escovado, vidro, couro,
//! papel), luz de cima e sombra no formato do objeto. Os do Mac servem só de
//! referência de estilo; nenhum desenho copia a composição deles.
//!
//! Coordenadas: cada ícone desenha no quadrado (x, y, s, s); `U` converte as
//! medidas de 0 a 1 do quadrado para pontos.

use tiny_skia::{Color, Path, PathBuilder, Transform};

use crate::draw::*;
use crate::font::Fonts;

/// Quadrado do ícone: converte medidas relativas (0 a 1) em pontos.
#[derive(Clone, Copy)]
pub(crate) struct U {
    pub x: f32,
    pub y: f32,
    pub s: f32,
}

impl U {
    pub fn px(&self, u: f32) -> f32 {
        self.x + u * self.s
    }
    pub fn py(&self, v: f32) -> f32 {
        self.y + v * self.s
    }
    pub fn d(&self, v: f32) -> f32 {
        v * self.s
    }
    /// Retângulo arredondado em medidas relativas.
    pub fn rr(&self, x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
        rrect(self.px(x), self.py(y), self.d(w), self.d(h), self.d(r))
    }
    pub fn circ(&self, cx: f32, cy: f32, r: f32) -> Option<Path> {
        circle(self.px(cx), self.py(cy), self.d(r))
    }
    pub fn ov(&self, x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
        oval(self.px(x), self.py(y), self.d(w), self.d(h))
    }
    /// Polígono pelos pontos relativos.
    pub fn poly(&self, pts: &[(f32, f32)]) -> Option<Path> {
        let mut pb = PathBuilder::new();
        for (i, &(a, b)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(self.px(a), self.py(b));
            } else {
                pb.line_to(self.px(a), self.py(b));
            }
        }
        pb.close();
        pb.finish()
    }
    /// Linha de espessura mínima legível.
    pub fn lw(&self, v: f32) -> f32 {
        self.d(v).max(0.8)
    }
}

/// Caminho transformado (inclinações e giros dos objetos em perspectiva).
pub(crate) fn tf(p: Option<Path>, t: Transform) -> Option<Path> {
    p?.transform(t)
}

/// Sombra suave no formato do objeto: o próprio contorno, deslocado para baixo
/// e "borrado" com traços cada vez mais largos e mais claros.
pub(crate) fn soft_shadow(pm: &mut Canvas, path: Option<Path>, s: f32, strength: f32) {
    let Some(p) = path else { return };
    let Some(moved) = p.clone().transform(Transform::from_translate(s * 0.008, s * 0.03)) else { return };
    let a = |v: f32| (v * strength).clamp(0.0, 255.0) as u8;
    // Camadas cada vez mais largas e claras: a borda some aos poucos (sem degrau).
    for k in (1..=6).rev() {
        pm.stroke(Some(moved.clone()), &solid(rgba(0, 0, 0, a(7.0))), s * 0.012 * k as f32);
    }
    pm.fill(Some(moved), &solid(rgba(0, 0, 0, a(30.0))));
}

/// Riscos finos do metal escovado dentro de `clip` (um caminho só para os claros
/// e um para os escuros: rápido mesmo com muitos riscos).
pub(crate) fn brush_streaks(pm: &mut Canvas, clip: Option<Path>, u: U, seed: u32, vertical: bool) {
    if u.s < 28.0 {
        return;
    }
    let n = (u.s * 0.7) as u32;
    let mut light = PathBuilder::new();
    let mut dark = PathBuilder::new();
    for i in 0..n {
        let h = hash(seed.wrapping_mul(977).wrapping_add(i));
        let t = (h & 0xffff) as f32 / 65535.0;
        let len = 0.25 + ((h >> 16) & 0xff) as f32 / 255.0 * 0.75;
        let off = ((h >> 24) & 0xff) as f32 / 255.0 * (1.0 - len);
        let pb = if h & 1 == 0 { &mut light } else { &mut dark };
        let th = u.s / 300.0;
        if vertical {
            if let Some(r) = tiny_skia::Rect::from_xywh(u.px(t), u.py(off), th.max(0.4), u.d(len)) {
                pb.push_rect(r);
            }
        } else if let Some(r) = tiny_skia::Rect::from_xywh(u.px(off), u.py(t), u.d(len), th.max(0.4)) {
            pb.push_rect(r);
        }
    }
    if let Some(p) = light.finish() {
        pm.fill_clipped(Some(p), &solid(rgba(255, 255, 255, 46)), clip.clone());
    }
    if let Some(p) = dark.finish() {
        pm.fill_clipped(Some(p), &solid(rgba(0, 0, 0, 26)), clip);
    }
}

/// Grão do couro: pontinhos claros e escuros espalhados dentro de `clip`.
pub(crate) fn leather_grain(pm: &mut Canvas, clip: Option<Path>, u: U, seed: u32) {
    if u.s < 32.0 {
        return;
    }
    let n = (u.s * u.s / 60.0) as u32;
    let mut light = PathBuilder::new();
    let mut dark = PathBuilder::new();
    let r = (u.s / 220.0).max(0.35);
    for i in 0..n {
        let h = hash(seed ^ i.wrapping_mul(2654435761));
        let (a, b) = ((h & 0xffff) as f32 / 65535.0, (h >> 16) as f32 / 65535.0);
        let pb = if i % 2 == 0 { &mut light } else { &mut dark };
        pb.push_circle(u.px(a), u.py(b), r);
    }
    if let Some(p) = light.finish() {
        pm.fill_clipped(Some(p), &solid(rgba(255, 255, 255, 22)), clip.clone());
    }
    if let Some(p) = dark.finish() {
        pm.fill_clipped(Some(p), &solid(rgba(0, 0, 0, 34)), clip);
    }
}

/// Brilho de vidro: faixa branca que some para baixo, só dentro de `clip`.
pub(crate) fn glass_glare(pm: &mut Canvas, clip: Option<Path>, x0: f32, y0: f32, x1: f32, y1: f32, a: u8) {
    if let Some(r) = tiny_skia::Rect::from_ltrb(x0, y0, x1, y1) {
        let p = PathBuilder::from_rect(r);
        pm.fill_clipped(Some(p), &vgrad(y0, y1, &[(0.0, rgba(255, 255, 255, a)), (1.0, rgba(255, 255, 255, 0))]), clip);
    }
}

/// Metal claro de cima (claro) para baixo (escuro), com o realce no meio.
/// Gradiente que acompanha um objeto girado.
pub(crate) fn tpaint(mut p: tiny_skia::Paint<'static>, t: Transform) -> tiny_skia::Paint<'static> {
    p.shader.transform(t);
    p
}

pub(crate) fn metal_paint(y0: f32, y1: f32) -> tiny_skia::Paint<'static> {
    vgrad(y0, y1, &[(0.0, rgba(246, 247, 249, 255)), (0.45, rgba(206, 208, 214, 255)), (0.55, rgba(190, 192, 199, 255)), (1.0, rgba(150, 153, 161, 255))])
}

// ---------------------------------------------------------------------------
// Pasta (a do Arquivos e as pastas comuns)
// ---------------------------------------------------------------------------

/// Pasta azul translúcida: fundo com a aba, frente com brilho e borda clara.
/// `base` muda a cor (as pastas especiais usam a mesma forma).
pub(crate) fn folder(pm: &mut Canvas, x: f32, y: f32, s: f32, base: Color) {
    // Uma margem em volta: sem ela a pasta encosta nos vizinhos no Dock.
    let u = U { x: x + s * 0.05, y: y + s * 0.05, s: s * 0.9 };
    let back = |u: &U| {
        let mut pb = PathBuilder::new();
        let r = u.d(0.035);
        pb.move_to(u.px(0.06) + r, u.py(0.14));
        pb.line_to(u.px(0.36), u.py(0.14));
        pb.quad_to(u.px(0.40), u.py(0.14), u.px(0.43), u.py(0.2));
        pb.line_to(u.px(0.94) - r, u.py(0.2));
        pb.quad_to(u.px(0.94), u.py(0.2), u.px(0.94), u.py(0.2) + r);
        pb.line_to(u.px(0.94), u.py(0.84));
        pb.line_to(u.px(0.06), u.py(0.84));
        pb.line_to(u.px(0.06), u.py(0.14) + r);
        pb.quad_to(u.px(0.06), u.py(0.14), u.px(0.06) + r, u.py(0.14));
        pb.close();
        pb.finish()
    };
    let front = u.poly(&[(0.04, 0.3), (0.96, 0.3), (0.94, 0.86), (0.06, 0.86)]);
    soft_shadow(pm, u.rr(0.06, 0.18, 0.88, 0.68, 0.04), s, 0.8);
    pm.fill(back(&u), &vgrad(u.py(0.14), u.py(0.84), &[(0.0, lighten(base, 0.15)), (1.0, darken(base, 0.25))]));
    pm.stroke(back(&u), &solid(with_alpha(darken(base, 0.45), 0.9)), u.lw(0.008));
    // Folhas dentro da pasta, aparecendo em cima.
    pm.fill(u.rr(0.12, 0.23, 0.76, 0.2, 0.01), &solid(rgba(250, 250, 252, 255)));
    pm.fill(u.rr(0.15, 0.25, 0.7, 0.2, 0.01), &solid(rgba(236, 238, 242, 255)));
    // Espessura da frente (a borda de baixo, mais escura).
    pm.fill(u.poly(&[(0.06, 0.86), (0.94, 0.86), (0.935, 0.885), (0.065, 0.885)]), &solid(darken(base, 0.45)));
    // Frente translúcida com brilho.
    pm.fill(front.clone(), &vgrad(u.py(0.3), u.py(0.86), &[(0.0, lighten(base, 0.5)), (0.5, lighten(base, 0.22)), (1.0, base)]));
    pm.fill_clipped(u.rr(0.04, 0.3, 0.92, 0.16, 0.0), &vgrad(u.py(0.3), u.py(0.46), &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(255, 255, 255, 0))]), front.clone());
    pm.stroke(front.clone(), &solid(with_alpha(darken(base, 0.4), 0.9)), u.lw(0.008));
    // Borda de cima da frente, clara (a luz bate ali).
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(0.05), u.py(0.305));
    pb.line_to(u.px(0.95), u.py(0.305));
    pm.stroke(pb.finish(), &solid(rgba(255, 255, 255, 220)), u.lw(0.012));
}

// ---------------------------------------------------------------------------
// Terminal
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Calculadora
// ---------------------------------------------------------------------------

/// Calculadora de alumínio com espessura, visor verde e teclas de gel cinza.
pub(crate) fn calculator(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let body = u.rr(0.17, 0.05, 0.64, 0.86, 0.06);
    soft_shadow(pm, body.clone(), s, 1.0);
    // Lateral e base (a espessura do aparelho).
    pm.fill(u.rr(0.19, 0.07, 0.64, 0.86, 0.06), &solid(rgba(96, 98, 106, 255)));
    pm.fill(body.clone(), &metal_paint(u.py(0.05), u.py(0.91)));
    brush_streaks(pm, body.clone(), u, 23, false);
    pm.stroke(body, &solid(rgba(90, 92, 100, 255)), u.lw(0.009));
    // Visor
    let lcd = u.rr(0.24, 0.11, 0.5, 0.15, 0.02);
    pm.fill(u.rr(0.235, 0.105, 0.51, 0.16, 0.025), &solid(rgba(80, 82, 88, 255)));
    pm.fill(lcd.clone(), &vgrad(u.py(0.11), u.py(0.26), &[(0.0, rgba(156, 172, 136, 255)), (1.0, rgba(206, 222, 180, 255))]));
    if s >= 40.0 {
        let t = "1.984,25";
        let px = u.d(0.085);
        let tw = fonts.measure(t, px, false);
        fonts.draw(pm, t, u.px(0.71) - tw, u.py(0.215), px, false, rgba(36, 44, 30, 255));
    }
    glass_glare(pm, lcd, u.px(0.24), u.py(0.11), u.px(0.74), u.py(0.18), 70);
    // Teclas: 4 colunas, 5 linhas; a última coluna um pouco mais escura.
    let (kx, ky, kw, kh, gx, gy) = (0.245, 0.31, 0.105, 0.09, 0.022, 0.024);
    let labels = ["C", "±", "÷", "×", "7", "8", "9", "−", "4", "5", "6", "+", "1", "2", "3", "=", "0", "", ",", ""];
    for row in 0..5 {
        for col in 0..4 {
            let i = row * 4 + col;
            // "=" ocupa duas linhas; o "0", duas colunas.
            if (row == 4 && col == 3) || (row == 4 && col == 1) {
                continue;
            }
            let w = if row == 4 && col == 0 { kw * 2.0 + gx } else { kw };
            let h = if row == 3 && col == 3 { kh * 2.0 + gy } else { kh };
            let (rx, ry) = (kx + col as f32 * (kw + gx), ky + row as f32 * (kh + gy));
            let op = col == 3;
            let (top, bot) = if op { (rgba(196, 198, 206, 255), rgba(128, 132, 142, 255)) } else { (rgba(250, 250, 252, 255), rgba(184, 186, 194, 255)) };
            let key = u.rr(rx, ry, w, h, 0.02);
            pm.fill(u.rr(rx, ry + 0.008, w, h, 0.02), &solid(rgba(0, 0, 0, 60)));
            pm.fill(key.clone(), &vgrad(u.py(ry), u.py(ry + h), &[(0.0, top), (1.0, bot)]));
            pm.fill_clipped(u.rr(rx, ry, w, h * 0.45, 0.0), &solid(rgba(255, 255, 255, 70)), key.clone());
            pm.stroke(key, &solid(rgba(90, 92, 100, 200)), u.lw(0.005));
            let l = labels[i];
            if s >= 56.0 && !l.is_empty() {
                let px = u.d(0.055);
                let tw = fonts.measure(l, px, true);
                fonts.draw(pm, l, u.px(rx + w / 2.0) - tw / 2.0, u.py(ry + h / 2.0) + px * 0.36, px, true, rgba(40, 42, 50, 255));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Relógio
// ---------------------------------------------------------------------------

/// Relógio de mesa cromado: aro com volume, mostrador branco, ponteiros e o
/// reflexo do vidro; dois pezinhos embaixo.
pub(crate) fn clock(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, h: u8, m: u8) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let u = U { x, y, s };
    let (cx, cy, r) = (u.px(0.5), u.py(0.46), u.d(0.4));
    // Pés
    for fx in [0.3, 0.62] {
        pm.fill(u.rr(fx, 0.8, 0.08, 0.1, 0.03), &metal_paint(u.py(0.8), u.py(0.9)));
    }
    soft_shadow(pm, circle(cx, cy, r), s, 1.0);
    // Aro: metal com reflexo forte em cima à esquerda.
    pm.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(255, 255, 255, 255)), (0.35, rgba(200, 204, 212, 255)), (0.6, rgba(120, 124, 134, 255)), (1.0, rgba(210, 214, 222, 255))]));
    pm.stroke(circle(cx, cy, r), &solid(rgba(80, 82, 92, 255)), u.lw(0.008));
    pm.fill(circle(cx, cy, r * 0.86), &linear(cx + r, cy + r, cx - r, cy - r, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(130, 134, 144, 255))]));
    // Mostrador
    let fr = r * 0.8;
    pm.fill(circle(cx, cy, fr), &radial(cx, cy - fr * 0.3, fr * 1.3, &[(0.0, rgba(255, 255, 255, 255)), (0.8, rgba(240, 240, 236, 255)), (1.0, rgba(200, 200, 196, 255))]));
    for i in 0..60 {
        let a = i as f32 / 60.0 * TAU;
        let big = i % 5 == 0;
        if !big && s < 48.0 {
            continue;
        }
        let (r0, w) = if big { (fr * 0.8, u.lw(0.02)) } else { (fr * 0.9, u.lw(0.006)) };
        let mut pb = PathBuilder::new();
        pb.move_to(cx + a.cos() * r0, cy + a.sin() * r0);
        pb.line_to(cx + a.cos() * fr * 0.95, cy + a.sin() * fr * 0.95);
        pm.stroke(pb.finish(), &solid(rgba(30, 30, 36, 255)), w);
    }
    if s >= 72.0 {
        for (n, a) in [("12", -FRAC_PI_2), ("3", 0.0), ("6", FRAC_PI_2), ("9", std::f32::consts::PI)] {
            let px = u.d(0.085);
            let tw = fonts.measure(n, px, true);
            fonts.draw(pm, n, cx + a.cos() * fr * 0.6 - tw / 2.0, cy + a.sin() * fr * 0.6 + px * 0.36, px, true, rgba(30, 30, 36, 255));
        }
    }
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * TAU - FRAC_PI_2;
    let ma = m as f32 / 60.0 * TAU - FRAC_PI_2;
    let hand = |pm: &mut Canvas, a: f32, len: f32, w: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(cx - a.cos() * fr * 0.1, cy - a.sin() * fr * 0.1);
        pb.line_to(cx + a.cos() * len, cy + a.sin() * len);
        pm.stroke_round(pb.finish(), &solid(rgba(20, 20, 26, 255)), w);
    };
    hand(pm, ha, fr * 0.5, u.lw(0.04));
    hand(pm, ma, fr * 0.78, u.lw(0.026));
    let sa = ma + 2.1;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - sa.cos() * fr * 0.15, cy - sa.sin() * fr * 0.15);
    pb.line_to(cx + sa.cos() * fr * 0.86, cy + sa.sin() * fr * 0.86);
    pm.stroke_round(pb.finish(), &solid(rgba(210, 40, 30, 255)), u.lw(0.01));
    pm.fill(circle(cx, cy, u.d(0.03)), &solid(rgba(210, 40, 30, 255)));
    // Vidro: brilho em arco em cima.
    pm.fill_clipped(oval(cx - fr * 0.85, cy - fr * 1.05, fr * 1.7, fr * 1.1), &vgrad(cy - fr, cy, &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(255, 255, 255, 0))]), circle(cx, cy, fr));
}

// ---------------------------------------------------------------------------
// Correio
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Agenda
// ---------------------------------------------------------------------------

const MESES: [&str; 12] = ["JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ"];

/// Bloco de calendário de mesa: folhas empilhadas, faixa vermelha com o mês,
/// argolas de metal e o dia de hoje grande; a folha de cima com a ponta virada.
pub(crate) fn calendar(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, month: u32, day: u32) {
    let u = U { x, y, s };
    let block = u.rr(0.12, 0.12, 0.76, 0.78, 0.04);
    soft_shadow(pm, block.clone(), s, 1.0);
    // Folhas de baixo (a espessura do bloco).
    for k in (1..=3).rev() {
        let d = k as f32 * 0.012;
        pm.fill(u.rr(0.12 + d * 0.3, 0.12 + d, 0.76, 0.78, 0.04), &solid(if k % 2 == 0 { rgba(214, 212, 206, 255) } else { rgba(236, 234, 228, 255) }));
    }
    // Folha de cima com a ponta direita de baixo virada.
    let page = u.poly(&[(0.12, 0.16), (0.88, 0.16), (0.88, 0.74), (0.76, 0.86), (0.12, 0.86)]);
    pm.fill(page.clone(), &vgrad(u.py(0.16), u.py(0.86), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(236, 236, 232, 255))]));
    pm.stroke(page.clone(), &solid(rgba(150, 148, 142, 255)), u.lw(0.007));
    let curl = u.poly(&[(0.88, 0.74), (0.76, 0.86), (0.79, 0.76)]);
    pm.fill(curl.clone(), &linear(u.px(0.88), u.py(0.74), u.px(0.78), u.py(0.82), &[(0.0, rgba(250, 250, 248, 255)), (1.0, rgba(186, 184, 178, 255))]));
    pm.stroke(curl, &solid(rgba(140, 138, 132, 255)), u.lw(0.006));
    // Faixa vermelha com brilho e o mês.
    let band = u.rr(0.12, 0.12, 0.76, 0.24, 0.04);
    pm.fill(band.clone(), &vgrad(u.py(0.12), u.py(0.36), &[(0.0, rgba(236, 86, 70, 255)), (0.5, rgba(204, 36, 28, 255)), (1.0, rgba(164, 20, 14, 255))]));
    pm.fill_clipped(u.rr(0.12, 0.12, 0.76, 0.1, 0.0), &solid(rgba(255, 255, 255, 50)), band.clone());
    pm.fill_rect(u.px(0.12), u.py(0.3), u.d(0.76), u.d(0.06), &vgrad(u.py(0.3), u.py(0.36), &[(0.0, rgba(164, 20, 14, 255)), (1.0, rgba(140, 14, 10, 255))]));
    if s >= 32.0 {
        let t = glove_i18n::tr(MESES[(month.clamp(1, 12) - 1) as usize]);
        let px = u.d(0.13);
        let tw = fonts.measure(t, px, true);
        fonts.draw(pm, t, u.px(0.5) - tw / 2.0, u.py(0.31), px, true, rgba(255, 255, 255, 245));
    }
    // Argolas
    for rx in [0.3, 0.7] {
        pm.fill(u.rr(rx - 0.025, 0.06, 0.05, 0.12, 0.025), &linear(u.px(rx - 0.025), 0.0, u.px(rx + 0.025), 0.0, &[(0.0, rgba(120, 122, 130, 255)), (0.5, rgba(245, 246, 248, 255)), (1.0, rgba(110, 112, 120, 255))]));
        pm.fill(u.ov(rx - 0.03, 0.15, 0.06, 0.03), &solid(rgba(60, 20, 18, 160)));
    }
    // Dia
    let t = day.to_string();
    let px = u.d(0.4);
    let tw = fonts.measure(&t, px, true);
    fonts.draw(pm, &t, u.px(0.5) - tw / 2.0, u.py(0.76), px, true, rgba(34, 34, 40, 255));
}

// ---------------------------------------------------------------------------
// Contatos
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Lixo
// ---------------------------------------------------------------------------

/// Bolas de papel com que o Lixo fica cheio (um papel por item).
pub const TRASH_FULL_AT: u8 = 20;


// ---------------------------------------------------------------------------
// Segundo lote (9.11.3)
// ---------------------------------------------------------------------------

/// Papel amassado: contorno irregular dividido em facetas (triângulos a partir de
/// dois "nós" fora do centro), cada uma num tom conforme vira para a luz de cima
/// à esquerda, mais alguns vincos curtos.
pub(crate) fn paper_ball(pm: &mut Canvas, u: U, cx: f32, cy: f32, r: f32, seed: u32) {
    let rnd = |k: u32| (hash(seed.wrapping_mul(7919).wrapping_add(k)) & 0xffff) as f32 / 65535.0;
    let n = 14;
    let mut pts = Vec::with_capacity(n);
    for k in 0..n {
        let a = (k as f32 + (rnd(k as u32) - 0.5) * 0.5) / n as f32 * std::f32::consts::TAU;
        let j = 0.82 + rnd(100 + k as u32) * 0.22;
        pts.push((cx + a.cos() * r * j, cy + a.sin() * r * j * 0.86));
    }
    let ball = u.poly(&pts);
    pm.fill(ball.clone(), &solid(rgba(222, 223, 228, 255)));
    // Dois nós internos: as facetas se juntam neles (como as pontas das dobras).
    let knots = [(cx - r * (0.2 + rnd(200) * 0.2), cy - r * (0.1 + rnd(201) * 0.2)), (cx + r * (0.15 + rnd(202) * 0.2), cy + r * (0.1 + rnd(203) * 0.2))];
    let tone = |pm: &mut Canvas, tri: [(f32, f32); 3], k: u32| {
        let mx = (tri[0].0 + tri[1].0 + tri[2].0) / 3.0 - cx;
        let my = (tri[0].1 + tri[1].1 + tri[2].1) / 3.0 - cy;
        // Lado de cima à esquerda claro, de baixo à direita escuro, com variação.
        let lit = (-(mx + my) / r * 0.9 + (rnd(300 + k) - 0.5) * 0.9).clamp(-1.0, 1.0);
        let v = (226.0 + lit * 30.0).clamp(150.0, 255.0) as u8;
        pm.fill_clipped(u.poly(&tri), &solid(rgba(v, v, (v as f32 * 1.01).min(255.0) as u8, 255)), ball.clone());
    };
    for k in 0..n {
        let (p0, p1) = (pts[k], pts[(k + 1) % n]);
        let knot = if (p0.0 - cx) * 0.6 + (p0.1 - cy) < 0.0 { knots[0] } else { knots[1] };
        tone(pm, [p0, p1, knot], k as u32);
    }
    tone(pm, [knots[0], knots[1], pts[n / 4]], 90);
    tone(pm, [knots[0], knots[1], pts[3 * n / 4]], 91);
    if u.s >= 40.0 {
        // Vincos: arestas escuras finas e o fio claro ao lado.
        let mut dark = PathBuilder::new();
        let mut light = PathBuilder::new();
        for k in (0..n).step_by(3) {
            let knot = knots[k % 2];
            dark.move_to(u.px(knot.0), u.py(knot.1));
            dark.line_to(u.px(pts[k].0), u.py(pts[k].1));
            light.move_to(u.px(knot.0 + 0.004), u.py(knot.1 + 0.004));
            light.line_to(u.px(pts[k].0 + 0.004), u.py(pts[k].1 + 0.004));
        }
        dark.move_to(u.px(knots[0].0), u.py(knots[0].1));
        dark.line_to(u.px(knots[1].0), u.py(knots[1].1));
        let (d, l) = (dark.finish(), light.finish());
        pm.stroke(d, &solid(rgba(130, 132, 142, 140)), u.lw(0.004));
        pm.stroke(l, &solid(rgba(255, 255, 255, 170)), u.lw(0.003));
    }
    // Sombra no lado de baixo e contorno.
    pm.fill_clipped(ball.clone(), &radial(u.px(cx - r * 0.4), u.py(cy - r * 0.5), u.d(r * 2.0), &[(0.0, rgba(255, 255, 255, 60)), (0.55, rgba(255, 255, 255, 0)), (1.0, rgba(60, 64, 80, 90))]), ball.clone());
    pm.stroke(ball, &solid(rgba(132, 134, 144, 255)), u.lw(0.005));
}

/// Moldura de alumínio de monitor (Terminal, Monitor, Monitores): devolve a tela.
fn aluminum_display(pm: &mut Canvas, u: U, seed: u32) -> Option<Path> {
    pm.fill(u.ov(0.3, 0.86, 0.4, 0.08), &metal_paint(u.py(0.86), u.py(0.94)));
    pm.fill(u.poly(&[(0.43, 0.74), (0.57, 0.74), (0.6, 0.89), (0.4, 0.89)]), &vgrad(u.py(0.74), u.py(0.89), &[(0.0, rgba(150, 152, 160, 255)), (1.0, rgba(200, 202, 208, 255))]));
    let body = u.rr(0.05, 0.07, 0.9, 0.7, 0.07);
    soft_shadow(pm, body.clone(), u.s, 1.0);
    pm.fill(u.rr(0.05, 0.09, 0.9, 0.7, 0.07), &solid(rgba(110, 112, 120, 255)));
    pm.fill(body.clone(), &metal_paint(u.py(0.07), u.py(0.77)));
    brush_streaks(pm, body.clone(), u, seed, false);
    pm.stroke(body, &solid(rgba(96, 98, 106, 255)), u.lw(0.01));
    pm.fill(u.rr(0.115, 0.135, 0.77, 0.55, 0.03), &solid(rgba(70, 72, 80, 255)));
    u.rr(0.12, 0.14, 0.76, 0.54, 0.025)
}

fn screen_glare(pm: &mut Canvas, u: U, screen: Option<Path>) {
    let glare = u.poly(&[(0.12, 0.14), (0.62, 0.14), (0.3, 0.68), (0.12, 0.68)]);
    pm.fill_clipped(glare, &linear(u.px(0.12), u.py(0.14), u.px(0.45), u.py(0.5), &[(0.0, rgba(255, 255, 255, 50)), (1.0, rgba(255, 255, 255, 0))]), screen);
}

/// Monitores (preferência): a moldura com uma mesa azul na tela.
pub(crate) fn display_pref(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let screen = aluminum_display(pm, u, 17);
    pm.fill(screen.clone(), &linear(u.px(0.12), u.py(0.14), u.px(0.88), u.py(0.68), &[(0.0, rgba(110, 170, 240, 255)), (0.6, rgba(40, 90, 190, 255)), (1.0, rgba(20, 50, 130, 255))]));
    let mut wave = PathBuilder::new();
    wave.move_to(u.px(0.12), u.py(0.52));
    wave.quad_to(u.px(0.4), u.py(0.36), u.px(0.88), u.py(0.48));
    wave.line_to(u.px(0.88), u.py(0.68));
    wave.line_to(u.px(0.12), u.py(0.68));
    wave.close();
    pm.fill_clipped(wave.finish(), &solid(rgba(255, 255, 255, 40)), screen.clone());
    screen_glare(pm, u, screen);
}


/// Lembretes: três notas adesivas coloridas, a de cima com rabisco e um alfinete.
pub(crate) fn notes(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let notes = [(14.0, rgba(170, 210, 250, 255), 0.0), (-10.0, rgba(250, 180, 200, 255), 0.0), (-3.0, rgba(255, 238, 120, 255), 1.0)];
    for (i, (ang, col, top)) in notes.iter().enumerate() {
        let t = Transform::from_rotate_at(*ang, u.px(0.5), u.py(0.5));
        let off = i as f32 * 0.03;
        let note = tf(u.rr(0.14 + off, 0.14 + off, 0.66, 0.66, 0.01), t);
        soft_shadow(pm, note.clone(), s, 0.6);
        pm.fill(note.clone(), &vgrad(u.py(0.14), u.py(0.8), &[(0.0, lighten(*col, 0.15)), (1.0, *col)]));
        // A ponta de baixo levanta um pouco (sombra na dobra).
        pm.fill_clipped(tf(u.poly(&[(0.6 + off, 0.8 + off), (0.8 + off, 0.6 + off), (0.8 + off, 0.8 + off)]), t), &linear(u.px(0.7), u.py(0.7), u.px(0.8), u.py(0.8), &[(0.0, rgba(0, 0, 0, 0)), (1.0, rgba(0, 0, 0, 40))]), note.clone());
        if *top > 0.0 && s >= 32.0 {
            let mut sc = PathBuilder::new();
            for (k, yy) in [0.34, 0.46, 0.58].iter().enumerate() {
                let x0 = 0.26 + off;
                sc.move_to(u.px(x0), u.py(yy + off));
                for j in 1..=5 {
                    let xx = x0 + j as f32 * (0.08 - k as f32 * 0.01);
                    sc.quad_to(u.px(xx - 0.04), u.py(yy + off + if j % 2 == 0 { 0.025 } else { -0.025 }), u.px(xx), u.py(yy + off));
                }
            }
            pm.stroke(tf(sc.finish(), t), &solid(rgba(40, 60, 140, 220)), u.lw(0.012));
            // Alfinete vermelho
            let pin = tf(u.circ(0.5 + off, 0.2 + off, 0.05), t);
            pm.fill(pin.clone(), &radial(u.px(0.48 + off), u.py(0.18 + off), u.d(0.07), &[(0.0, rgba(255, 160, 150, 255)), (1.0, rgba(180, 20, 20, 255))]));
            pm.stroke(pin, &solid(rgba(110, 10, 10, 255)), u.lw(0.006));
        }
    }
}

/// Busca: lupa cromada inclinada, com lente de vidro e cabo preto.
pub(crate) fn search(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let (cx, cy, r) = (0.42, 0.4, 0.28);
    // Cabo
    let t = Transform::from_rotate_at(45.0, u.px(0.72), u.py(0.72));
    let handle = tf(u.rr(0.52, 0.66, 0.42, 0.12, 0.06), t);
    soft_shadow(pm, handle.clone(), s, 0.8);
    soft_shadow(pm, u.circ(cx, cy, r + 0.04), s, 0.8);
    pm.fill(handle.clone(), &linear(u.px(0.6), u.py(0.6), u.px(0.84), u.py(0.84), &[(0.0, rgba(80, 80, 90, 255)), (0.5, rgba(20, 20, 26, 255)), (1.0, rgba(60, 60, 70, 255))]));
    pm.fill_clipped(tf(u.rr(0.52, 0.66, 0.42, 0.04, 0.02), t), &solid(rgba(255, 255, 255, 70)), handle);
    // Aro cromado e lente
    pm.fill(u.circ(cx, cy, r + 0.05), &linear(u.px(cx - r), u.py(cy - r), u.px(cx + r), u.py(cy + r), &[(0.0, rgba(255, 255, 255, 255)), (0.4, rgba(170, 174, 184, 255)), (0.7, rgba(100, 104, 114, 255)), (1.0, rgba(220, 224, 230, 255))]));
    let lens = u.circ(cx, cy, r);
    // Vidro: quase transparente no meio, azulado e mais escuro na borda (espessura).
    pm.fill(lens.clone(), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(236, 246, 255, 120)), (0.7, rgba(170, 205, 245, 170)), (0.92, rgba(90, 130, 200, 220)), (1.0, rgba(40, 70, 130, 240))]));
    // Luz que atravessa a lente e bate embaixo à direita (cáustica).
    pm.fill_clipped(u.circ(cx + r * 0.25, cy + r * 0.3, r * 0.6), &radial(u.px(cx + r * 0.3), u.py(cy + r * 0.4), u.d(r * 0.6), &[(0.0, rgba(255, 255, 255, 140)), (1.0, rgba(255, 255, 255, 0))]), lens.clone());
    // Reflexo em meia-lua em cima à esquerda (a lente menos um círculo deslocado).
    let mut moon = PathBuilder::new();
    moon.push_circle(u.px(cx), u.py(cy), u.d(r * 0.9));
    moon.push_circle(u.px(cx + r * 0.2), u.py(cy + r * 0.24), u.d(r * 0.86));
    if let Some(m) = moon.finish() {
        let paint = linear(u.px(cx - r), u.py(cy - r), u.px(cx + r * 0.3), u.py(cy + r * 0.3), &[(0.0, rgba(255, 255, 255, 250)), (1.0, rgba(255, 255, 255, 40))]);
        pm.fill_rule(Some(m), &paint, tiny_skia::FillRule::EvenOdd);
    }
    // Ponto de luz e o fio claro na borda de baixo.
    pm.fill(u.ov(cx - r * 0.55, cy - r * 0.6, r * 0.34, r * 0.22), &radial(u.px(cx - r * 0.38), u.py(cy - r * 0.49), u.d(r * 0.2), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(255, 255, 255, 0))]));
    let mut lip = PathBuilder::new();
    lip.move_to(u.px(cx - r * 0.5), u.py(cy + r * 0.8));
    lip.quad_to(u.px(cx + r * 0.4), u.py(cy + r * 0.95), u.px(cx + r * 0.82), u.py(cy + r * 0.3));
    pm.stroke_round(lip.finish(), &solid(rgba(255, 255, 255, 170)), u.lw(0.012));
    pm.stroke(lens, &solid(rgba(50, 54, 66, 255)), u.lw(0.008));
}

/// Teclas de gel brancas com volume (Caracteres).
pub(crate) fn chars(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let keys = [("A", 0.08, 0.1), ("Ω", 0.52, 0.1), ("é", 0.08, 0.52), ("★", 0.52, 0.52)];
    for (g, kx, ky) in keys {
        let k = u.rr(kx, ky, 0.4, 0.38, 0.06);
        soft_shadow(pm, k.clone(), s, 0.8);
        // Lateral (altura da tecla) e o topo côncavo.
        pm.fill(u.rr(kx, ky + 0.03, 0.4, 0.38, 0.06), &solid(rgba(150, 152, 162, 255)));
        pm.fill(k.clone(), &vgrad(u.py(ky), u.py(ky + 0.38), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(214, 216, 224, 255))]));
        pm.fill(u.rr(kx + 0.04, ky + 0.03, 0.32, 0.28, 0.05), &vgrad(u.py(ky), u.py(ky + 0.32), &[(0.0, rgba(236, 238, 244, 255)), (1.0, rgba(252, 252, 254, 255))]));
        pm.stroke(k, &solid(rgba(120, 122, 132, 255)), u.lw(0.007));
        if s >= 24.0 {
            let px = u.d(0.22);
            let tw = fonts.measure(g, px, true);
            fonts.draw(pm, g, u.px(kx + 0.2) - tw / 2.0, u.py(ky + 0.26), px, true, rgba(40, 44, 60, 255));
        }
    }
}

/// Captura: câmera compacta prateada vista um pouco de cima (topo com espessura),
/// lente grande com reflexos, flash e botão.
pub(crate) fn camera(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let body = u.rr(0.06, 0.28, 0.88, 0.54, 0.07);
    soft_shadow(pm, body.clone(), s, 1.0);
    // Topo (espessura vista de cima) e botão
    pm.fill(u.rr(0.66, 0.2, 0.16, 0.08, 0.02), &metal_paint(u.py(0.2), u.py(0.28)));
    pm.fill(u.poly(&[(0.1, 0.28), (0.9, 0.28), (0.86, 0.23), (0.14, 0.23)]), &vgrad(u.py(0.23), u.py(0.28), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(200, 202, 208, 255))]));
    pm.fill(body.clone(), &metal_paint(u.py(0.28), u.py(0.82)));
    brush_streaks(pm, body.clone(), u, 29, false);
    // Faixa de couro preta
    pm.fill_clipped(u.rr(0.06, 0.4, 0.88, 0.3, 0.0), &vgrad(u.py(0.4), u.py(0.7), &[(0.0, rgba(50, 50, 56, 255)), (1.0, rgba(24, 24, 28, 255))]), body.clone());
    leather_grain(pm, u.rr(0.06, 0.4, 0.88, 0.3, 0.0), u, 5);
    pm.stroke(body, &solid(rgba(90, 92, 100, 255)), u.lw(0.008));
    // Lente
    let (cx, cy) = (0.5, 0.55);
    pm.fill(u.circ(cx, cy, 0.22), &linear(u.px(0.3), u.py(0.35), u.px(0.7), u.py(0.75), &[(0.0, rgba(250, 250, 252, 255)), (0.5, rgba(130, 134, 144, 255)), (1.0, rgba(220, 222, 228, 255))]));
    pm.fill(u.circ(cx, cy, 0.17), &solid(rgba(30, 32, 38, 255)));
    pm.fill(u.circ(cx, cy, 0.12), &radial(u.px(cx - 0.03), u.py(cy - 0.03), u.d(0.14), &[(0.0, rgba(90, 120, 200, 255)), (0.6, rgba(30, 40, 90, 255)), (1.0, rgba(10, 10, 20, 255))]));
    pm.fill(u.circ(cx - 0.05, cy - 0.05, 0.035), &solid(rgba(255, 255, 255, 200)));
    pm.fill(u.circ(cx + 0.05, cy + 0.04, 0.015), &solid(rgba(255, 255, 255, 120)));
    // Flash
    pm.fill(u.rr(0.14, 0.31, 0.16, 0.07, 0.01), &vgrad(u.py(0.31), u.py(0.38), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(200, 210, 220, 255))]));
    pm.stroke(u.rr(0.14, 0.31, 0.16, 0.07, 0.01), &solid(rgba(120, 122, 130, 255)), u.lw(0.005));
}


/// Vídeo: claquete de cinema com listras, dobradiça e o topo um pouco aberto.
pub(crate) fn video(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let board = u.rr(0.1, 0.36, 0.8, 0.54, 0.03);
    soft_shadow(pm, board.clone(), s, 1.0);
    pm.fill(board.clone(), &vgrad(u.py(0.36), u.py(0.9), &[(0.0, rgba(50, 52, 58, 255)), (1.0, rgba(16, 16, 20, 255))]));
    pm.fill_clipped(u.rr(0.1, 0.36, 0.8, 0.1, 0.0), &solid(rgba(255, 255, 255, 30)), board.clone());
    // Linhas de giz (os campos da claquete)
    if s >= 32.0 {
        let mut chalk = PathBuilder::new();
        chalk.move_to(u.px(0.14), u.py(0.56));
        chalk.line_to(u.px(0.86), u.py(0.56));
        chalk.move_to(u.px(0.14), u.py(0.72));
        chalk.line_to(u.px(0.86), u.py(0.72));
        chalk.move_to(u.px(0.5), u.py(0.56));
        chalk.line_to(u.px(0.5), u.py(0.86));
        pm.stroke(chalk.finish(), &solid(rgba(236, 236, 230, 180)), u.lw(0.008));
        // Anotações a giz nos campos (riscos curtos e irregulares).
        let mut notes = PathBuilder::new();
        for (x0, y0, w) in [(0.18, 0.64, 0.22), (0.56, 0.64, 0.18), (0.18, 0.8, 0.14), (0.56, 0.8, 0.24)] {
            notes.move_to(u.px(x0), u.py(y0));
            let n = 5;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                let j = ((hash((x0 * 100.0) as u32 + k) & 0xff) as f32 / 255.0 - 0.5) * 0.02;
                notes.line_to(u.px(x0 + w * t), u.py(y0 + j));
            }
        }
        pm.stroke(notes.finish(), &solid(rgba(230, 230, 222, 140)), u.lw(0.012));
    }
    // Lousa: reflexo largo e suave na diagonal.
    pm.fill_clipped(u.poly(&[(0.1, 0.36), (0.5, 0.36), (0.25, 0.9), (0.1, 0.9)]), &solid(rgba(255, 255, 255, 18)), board.clone());
    pm.stroke(board, &solid(rgba(10, 10, 12, 255)), u.lw(0.008));
    // Faixa listrada de baixo (fixa) e a de cima (aberta, girada na dobradiça).
    let stripes = |pm: &mut Canvas, t: Transform| {
        let bar = tf(u.rr(0.1, 0.26, 0.8, 0.1, 0.01), t);
        pm.fill(bar.clone(), &solid(rgba(24, 24, 28, 255)));
        let mut w = PathBuilder::new();
        for k in 0..5 {
            let x0 = 0.12 + k as f32 * 0.16;
            w.move_to(u.px(x0), u.py(0.36));
            w.line_to(u.px(x0 + 0.07), u.py(0.26));
            w.line_to(u.px(x0 + 0.14), u.py(0.26));
            w.line_to(u.px(x0 + 0.07), u.py(0.36));
            w.close();
        }
        pm.fill_clipped(tf(w.finish(), t), &tpaint(vgrad(u.py(0.26), u.py(0.36), &[(0.0, rgba(255, 255, 252, 255)), (1.0, rgba(214, 214, 208, 255))]), t), bar.clone());
        // Chanfro: fio claro em cima e sombra embaixo da ripa.
        pm.fill_clipped(tf(u.rr(0.1, 0.26, 0.8, 0.012, 0.0), t), &solid(rgba(255, 255, 255, 90)), bar.clone());
        pm.fill_clipped(tf(u.rr(0.1, 0.345, 0.8, 0.015, 0.0), t), &solid(rgba(0, 0, 0, 90)), bar.clone());
        pm.stroke(bar, &solid(rgba(10, 10, 12, 255)), u.lw(0.006));
    };
    stripes(pm, Transform::identity());
    let open_t = Transform::from_rotate_at(-18.0, u.px(0.12), u.py(0.27)).pre_translate(0.0, -u.d(0.11));
    // Sombra da ripa aberta sobre a de baixo.
    pm.fill_clipped(tf(u.rr(0.1, 0.26, 0.8, 0.1, 0.01), open_t.post_translate(u.d(0.01), u.d(0.04))), &solid(rgba(0, 0, 0, 60)), u.rr(0.1, 0.26, 0.8, 0.1, 0.01));
    stripes(pm, open_t);
    pm.fill(u.circ(0.13, 0.28, 0.025), &metal_paint(u.py(0.25), u.py(0.31)));
}

/// Registros: prancheta de metal com uma folha de registros coloridos.
pub(crate) fn logs(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let board = u.rr(0.14, 0.08, 0.72, 0.86, 0.04);
    soft_shadow(pm, board.clone(), s, 1.0);
    pm.fill(board.clone(), &linear(u.px(0.14), u.py(0.08), u.px(0.86), u.py(0.94), &[(0.0, rgba(190, 150, 100, 255)), (1.0, rgba(140, 100, 60, 255))]));
    leather_grain(pm, board.clone(), u, 61);
    pm.stroke(board, &solid(rgba(90, 60, 30, 255)), u.lw(0.008));
    let paper = u.rr(0.2, 0.18, 0.6, 0.7, 0.01);
    pm.fill(paper.clone(), &vgrad(u.py(0.18), u.py(0.88), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(238, 238, 234, 255))]));
    if s >= 28.0 {
        let cols = [rgba(80, 80, 90, 220), rgba(200, 60, 50, 220), rgba(80, 80, 90, 220), rgba(60, 130, 210, 220), rgba(80, 80, 90, 220), rgba(220, 160, 40, 220), rgba(80, 80, 90, 220)];
        for (k, c) in cols.iter().enumerate() {
            let w = 0.3 + (hash(k as u32 + 7) & 0xff) as f32 / 255.0 * 0.18;
            pm.fill(u.rr(0.25, 0.3 + k as f32 * 0.075, w, 0.025, 0.01), &solid(*c));
        }
    }
    pm.stroke(paper, &solid(rgba(160, 160, 156, 255)), u.lw(0.005));
    // Prendedor de metal
    let clip = u.rr(0.34, 0.05, 0.32, 0.14, 0.04);
    pm.fill(clip.clone(), &metal_paint(u.py(0.05), u.py(0.19)));
    pm.stroke(clip, &solid(rgba(90, 92, 100, 255)), u.lw(0.007));
    pm.fill(u.rr(0.44, 0.08, 0.12, 0.04, 0.02), &solid(rgba(60, 62, 70, 255)));
}

/// Informações do Sistema: chip prateado em perspectiva com pinos e o "i" azul.
pub(crate) fn info(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let chip = u.rr(0.18, 0.18, 0.64, 0.64, 0.05);
    soft_shadow(pm, chip.clone(), s, 1.0);
    // Pinos dourados nos quatro lados
    let mut pins = PathBuilder::new();
    for k in 0..6 {
        let f = 0.24 + k as f32 * 0.1;
        for r in [tiny_skia::Rect::from_xywh(u.px(f), u.py(0.1), u.d(0.05), u.d(0.1)), tiny_skia::Rect::from_xywh(u.px(f), u.py(0.8), u.d(0.05), u.d(0.1)), tiny_skia::Rect::from_xywh(u.px(0.1), u.py(f), u.d(0.1), u.d(0.05)), tiny_skia::Rect::from_xywh(u.px(0.8), u.py(f), u.d(0.1), u.d(0.05))].into_iter().flatten() {
            pins.push_rect(r);
        }
    }
    pm.fill(pins.finish(), &linear(u.px(0.1), u.py(0.1), u.px(0.9), u.py(0.9), &[(0.0, rgba(255, 230, 150, 255)), (0.5, rgba(190, 140, 40, 255)), (1.0, rgba(240, 200, 100, 255))]));
    pm.fill(u.rr(0.18, 0.2, 0.64, 0.64, 0.05), &solid(rgba(80, 82, 90, 255)));
    pm.fill(chip.clone(), &metal_paint(u.py(0.18), u.py(0.82)));
    brush_streaks(pm, chip.clone(), u, 71, false);
    pm.stroke(chip, &solid(rgba(80, 82, 90, 255)), u.lw(0.008));
    // Selo azul com o "i"
    let badge = u.circ(0.5, 0.5, 0.18);
    pm.fill(badge.clone(), &radial(u.px(0.45), u.py(0.42), u.d(0.24), &[(0.0, rgba(140, 200, 255, 255)), (0.7, rgba(30, 100, 210, 255)), (1.0, rgba(14, 60, 150, 255))]));
    glass_glare(pm, badge.clone(), u.px(0.32), u.py(0.32), u.px(0.68), u.py(0.5), 150);
    pm.stroke(badge, &solid(rgba(10, 40, 110, 255)), u.lw(0.008));
    if s >= 20.0 {
        let px = u.d(0.26);
        let tw = fonts.measure("i", px, true);
        fonts.draw(pm, "i", u.px(0.5) - tw / 2.0, u.py(0.59), px, true, Color::WHITE);
    }
}

/// Ajuda: bola de vidro azul com o "?" branco e reflexo forte.
pub(crate) fn help(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let ball = u.circ(0.5, 0.48, 0.38);
    soft_shadow(pm, ball.clone(), s, 1.0);
    pm.fill(ball.clone(), &radial(u.px(0.5), u.py(0.66), u.d(0.5), &[(0.0, rgba(120, 200, 255, 255)), (0.6, rgba(30, 110, 220, 255)), (1.0, rgba(10, 50, 140, 255))]));
    let px = u.d(0.5);
    let tw = fonts.measure("?", px, true);
    fonts.draw(pm, "?", u.px(0.5) - tw / 2.0 + u.d(0.01), u.py(0.67) + u.d(0.01), px, true, rgba(0, 30, 90, 120));
    fonts.draw(pm, "?", u.px(0.5) - tw / 2.0, u.py(0.67), px, true, Color::WHITE);
    pm.fill_clipped(u.ov(0.18, 0.11, 0.64, 0.4), &vgrad(u.py(0.11), u.py(0.5), &[(0.0, rgba(255, 255, 255, 200)), (1.0, rgba(255, 255, 255, 10))]), ball.clone());
    pm.stroke(ball, &solid(rgba(10, 40, 110, 255)), u.lw(0.008));
}

/// Livro de capa dura em pé, levemente virado: capa, lombada e as folhas.
fn book(pm: &mut Canvas, u: U, cover: Color, ang: f32) -> (Option<Path>, Transform) {
    let t = Transform::from_rotate_at(ang, u.px(0.5), u.py(0.5));
    let c = tf(u.rr(0.2, 0.08, 0.6, 0.84, 0.03), t);
    soft_shadow(pm, c.clone(), u.s, 1.0);
    pm.fill(tf(u.rr(0.23, 0.11, 0.6, 0.82, 0.02), t), &solid(rgba(246, 240, 226, 255)));
    pm.fill(c.clone(), &linear(u.px(0.2), u.py(0.08), u.px(0.8), u.py(0.92), &[(0.0, lighten(cover, 0.25)), (0.6, cover), (1.0, darken(cover, 0.3))]));
    leather_grain(pm, c.clone(), u, 91);
    pm.fill_clipped(tf(u.rr(0.2, 0.08, 0.08, 0.84, 0.0), t), &linear(u.px(0.2), 0.0, u.px(0.28), 0.0, &[(0.0, darken(cover, 0.5)), (1.0, darken(cover, 0.15))]), c.clone());
    pm.stroke(c.clone(), &solid(darken(cover, 0.6)), u.lw(0.008));
    (c, t)
}

/// Dicionário: livro de couro vinho com "Aa" dourado, moldura dupla e fita.
pub(crate) fn dictionary(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let (_, t) = book(pm, u, rgba(130, 22, 28, 255), -4.0);
    let gold = rgba(240, 200, 100, 255);
    pm.stroke(tf(u.rr(0.34, 0.2, 0.38, 0.3, 0.02), t), &solid(gold), u.lw(0.01));
    pm.stroke(tf(u.rr(0.32, 0.18, 0.42, 0.34, 0.025), t), &solid(gold), u.lw(0.004));
    pm.stroke(tf(u.rr(0.3, 0.6, 0.46, 0.012, 0.0), t), &solid(gold), u.lw(0.004));
    if s >= 24.0 {
        let px = u.d(0.2);
        let tw = fonts.measure("Aa", px, true);
        fonts.draw(pm, "Aa", u.px(0.53) - tw / 2.0, u.py(0.42), px, true, gold);
    }
    pm.fill(tf(u.poly(&[(0.6, 0.9), (0.65, 0.9), (0.65, 0.99), (0.625, 0.96), (0.6, 0.99)]), t), &solid(rgba(200, 160, 40, 255)));
}


/// Metal torneado (aço polido): anéis concêntricos finos claros e escuros dentro de `clip`.
pub(crate) fn spun_metal(pm: &mut Canvas, clip: Option<Path>, u: U, cx: f32, cy: f32, r: f32, seed: u32) {
    if u.s < 28.0 {
        return;
    }
    let mut light = PathBuilder::new();
    let mut dark = PathBuilder::new();
    let n = (u.d(r) / 1.3) as u32;
    for i in 1..n {
        let rr = r * i as f32 / n as f32;
        let pb = if hash(seed.wrapping_add(i * 131)) & 1 == 0 { &mut light } else { &mut dark };
        pb.push_circle(u.px(cx), u.py(cy), u.d(rr));
    }
    let w = (u.s / 400.0).max(0.4);
    if let Some(p) = light.finish() {
        let clipped = clip.clone();
        pm.fill_clipped(p.stroke(&tiny_skia::Stroke { width: w, ..Default::default() }, 1.0), &solid(rgba(255, 255, 255, 60)), clipped);
    }
    if let Some(p) = dark.finish() {
        pm.fill_clipped(p.stroke(&tiny_skia::Stroke { width: w, ..Default::default() }, 1.0), &solid(rgba(0, 0, 0, 34)), clip);
    }
}


// ---------------------------------------------------------------------------
// Terceiro lote (9.11.4): documentos, tipos de arquivo, discos e servidores
// ---------------------------------------------------------------------------

/// Folha de papel com o canto de cima à direita dobrado para a frente (com a
/// sombrinha embaixo da dobra). Devolve a folha para recortar o conteúdo.
pub(crate) fn sheet(pm: &mut Canvas, u: U) -> Option<Path> {
    let (x0, y0, x1, y1, f) = (0.17, 0.05, 0.83, 0.95, 0.2);
    let page = u.poly(&[(x0, y0), (x1 - f, y0), (x1, y0 + f), (x1, y1), (x0, y1)]);
    soft_shadow(pm, page.clone(), u.s, 0.8);
    pm.fill(page.clone(), &linear(u.px(x0), u.py(y0), u.px(x1), u.py(y1), &[(0.0, rgba(255, 255, 255, 255)), (0.7, rgba(246, 247, 250, 255)), (1.0, rgba(226, 228, 234, 255))]));
    // Sombra que a dobra joga na folha.
    pm.fill_clipped(u.poly(&[(x1 - f, y0), (x1, y0 + f), (x1 - f * 0.1, y0 + f * 1.25), (x1 - f * 1.15, y0 + f * 0.2)]), &linear(u.px(x1 - f), u.py(y0 + f), u.px(x1 - f * 0.6), u.py(y0 + f * 1.3), &[(0.0, rgba(0, 0, 0, 50)), (1.0, rgba(0, 0, 0, 0))]), page.clone());
    pm.stroke(page.clone(), &solid(rgba(150, 152, 160, 255)), u.lw(0.006));
    // A dobra: o verso do papel, mais escuro na ponta.
    let fold = u.poly(&[(x1 - f, y0), (x1, y0 + f), (x1 - f, y0 + f)]);
    pm.fill(fold.clone(), &linear(u.px(x1 - f), u.py(y0 + f), u.px(x1 - f * 0.45), u.py(y0 + f * 0.45), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(196, 200, 210, 255))]));
    pm.stroke(fold, &solid(rgba(150, 152, 160, 255)), u.lw(0.006));
    page
}

/// Documento de texto: linhas de texto em cinza; Arquivo: folha em branco.
pub(crate) fn document(pm: &mut Canvas, x: f32, y: f32, s: f32, lines: bool) {
    let u = U { x, y, s };
    sheet(pm, u);
    if lines && s >= 20.0 {
        let mut pb = PathBuilder::new();
        for k in 0..11 {
            let yy = 0.32 + k as f32 * 0.052;
            let w = if k % 4 == 3 { 0.28 } else { 0.5 - (hash(k + 3) & 0x3f) as f32 / 63.0 * 0.06 };
            if let Some(r) = tiny_skia::Rect::from_xywh(u.px(0.26), u.py(yy), u.d(w), u.d(0.018).max(0.8)) {
                pb.push_rect(r);
            }
        }
        pm.fill(pb.finish(), &solid(rgba(120, 124, 136, 200)));
    }
}

/// Imagem: foto com borda branca, um pouco girada, sobre outra atrás.
pub(crate) fn picture(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    for (ang, front) in [(8.0, false), (-5.0, true)] {
        let t = Transform::from_rotate_at(ang, u.px(0.5), u.py(0.5));
        let card = tf(u.rr(0.08, 0.16, 0.84, 0.66, 0.015), t);
        soft_shadow(pm, card.clone(), s, 0.8);
        pm.fill(card.clone(), &vgrad(u.py(0.16), u.py(0.82), &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(232, 232, 236, 255))]));
        pm.stroke(card, &solid(rgba(150, 152, 160, 255)), u.lw(0.005));
        let img = tf(u.rr(0.13, 0.21, 0.74, 0.5, 0.0), t);
        if !front {
            pm.fill(img, &vgrad(u.py(0.21), u.py(0.71), &[(0.0, rgba(240, 190, 130, 255)), (1.0, rgba(190, 100, 70, 255))]));
            continue;
        }
        pm.fill(img.clone(), &vgrad(u.py(0.21), u.py(0.6), &[(0.0, rgba(60, 130, 220, 255)), (1.0, rgba(180, 220, 250, 255))]));
        pm.fill_clipped(tf(u.circ(0.7, 0.32, 0.06), t), &radial(u.px(0.7), u.py(0.32), u.d(0.09), &[(0.0, rgba(255, 250, 200, 255)), (0.6, rgba(255, 210, 80, 255)), (1.0, rgba(255, 210, 80, 0))]), img.clone());
        let hills = tf(u.poly(&[(0.13, 0.71), (0.13, 0.56), (0.32, 0.42), (0.5, 0.58), (0.64, 0.48), (0.87, 0.62), (0.87, 0.71)]), t);
        pm.fill_clipped(hills, &vgrad(u.py(0.42), u.py(0.71), &[(0.0, rgba(120, 190, 90, 255)), (1.0, rgba(40, 110, 50, 255))]), img.clone());
        glass_glare(pm, img.clone(), u.px(0.1), u.py(0.2), u.px(0.9), u.py(0.45), 50);
        pm.stroke(img, &solid(rgba(90, 92, 100, 160)), u.lw(0.004));
    }
}

/// Tipos de arquivo: a folha com um selo do tipo (1 áudio, 2 vídeo, 3 compactado,
/// 4 PDF, 5 código).
pub(crate) fn typed_document(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, kind: u8) {
    let u = U { x, y, s };
    let page = sheet(pm, u);
    match kind {
        1 => {
            // Nota musical de vidro roxo.
            let ink = |pm: &mut Canvas, p: Option<Path>| {
                pm.fill(p.clone(), &linear(u.px(0.3), u.py(0.3), u.px(0.7), u.py(0.85), &[(0.0, rgba(200, 150, 255, 255)), (0.5, rgba(130, 60, 200, 255)), (1.0, rgba(70, 20, 130, 255))]));
                pm.stroke(p, &solid(rgba(50, 10, 90, 255)), u.lw(0.006));
            };
            ink(pm, tf(u.ov(0.24, 0.7, 0.2, 0.13), Transform::from_rotate_at(-20.0, u.px(0.34), u.py(0.76))));
            ink(pm, tf(u.ov(0.5, 0.64, 0.2, 0.13), Transform::from_rotate_at(-20.0, u.px(0.6), u.py(0.7))));
            ink(pm, u.poly(&[(0.4, 0.38), (0.45, 0.38), (0.45, 0.76), (0.4, 0.76)]));
            ink(pm, u.poly(&[(0.66, 0.32), (0.71, 0.32), (0.71, 0.7), (0.66, 0.7)]));
            ink(pm, u.poly(&[(0.4, 0.38), (0.71, 0.3), (0.71, 0.4), (0.4, 0.48)]));
            pm.fill(u.rr(0.42, 0.36, 0.26, 0.04, 0.02), &solid(rgba(255, 255, 255, 110)));
        }
        2 => {
            // Fotograma de filme com o triângulo de tocar.
            let film = u.rr(0.24, 0.38, 0.52, 0.4, 0.02);
            pm.fill(film.clone(), &vgrad(u.py(0.38), u.py(0.78), &[(0.0, rgba(60, 62, 70, 255)), (1.0, rgba(18, 18, 22, 255))]));
            let mut holes = PathBuilder::new();
            for k in 0..5 {
                let xx = 0.27 + k as f32 * 0.1;
                for yy in [0.4, 0.73] {
                    if let Some(r) = tiny_skia::Rect::from_xywh(u.px(xx), u.py(yy), u.d(0.05), u.d(0.03)) {
                        holes.push_rect(r);
                    }
                }
            }
            pm.fill(holes.finish(), &solid(rgba(240, 240, 244, 255)));
            pm.fill(u.rr(0.28, 0.45, 0.44, 0.26, 0.01), &vgrad(u.py(0.45), u.py(0.71), &[(0.0, rgba(90, 150, 230, 255)), (1.0, rgba(30, 60, 140, 255))]));
            pm.fill(u.poly(&[(0.44, 0.5), (0.6, 0.58), (0.44, 0.66)]), &solid(rgba(255, 255, 255, 240)));
        }
        3 => {
            // Zíper de metal pela folha, com o puxador.
            let mut teeth = PathBuilder::new();
            for k in 0..12 {
                let yy = 0.08 + k as f32 * 0.05;
                let xx = if k % 2 == 0 { 0.44 } else { 0.5 };
                if let Some(r) = tiny_skia::Rect::from_xywh(u.px(xx), u.py(yy), u.d(0.06), u.d(0.032)) {
                    teeth.push_rect(r);
                }
            }
            pm.fill_clipped(teeth.finish(), &linear(u.px(0.44), 0.0, u.px(0.56), 0.0, &[(0.0, rgba(250, 250, 252, 255)), (0.5, rgba(150, 154, 164, 255)), (1.0, rgba(220, 222, 228, 255))]), page);
            let pull = u.rr(0.43, 0.68, 0.14, 0.2, 0.04);
            pm.fill(pull.clone(), &metal_paint(u.py(0.68), u.py(0.88)));
            pm.stroke(pull, &solid(rgba(80, 82, 92, 255)), u.lw(0.007));
            pm.fill(u.rr(0.47, 0.74, 0.06, 0.1, 0.02), &solid(rgba(60, 62, 70, 255)));
        }
        4 => {
            // Faixa vermelha brilhante com "PDF" e linhas de texto embaixo.
            let band = u.rr(0.22, 0.42, 0.56, 0.2, 0.03);
            pm.fill(band.clone(), &vgrad(u.py(0.42), u.py(0.62), &[(0.0, rgba(250, 90, 80, 255)), (1.0, rgba(170, 16, 16, 255))]));
            pm.fill_clipped(u.rr(0.22, 0.42, 0.56, 0.09, 0.0), &solid(rgba(255, 255, 255, 60)), band.clone());
            pm.stroke(band, &solid(rgba(110, 10, 10, 255)), u.lw(0.006));
            if s >= 40.0 {
                let px = u.d(0.14);
                let tw = fonts.measure("PDF", px, true);
                fonts.draw(pm, "PDF", u.px(0.5) - tw / 2.0, u.py(0.575), px, true, Color::WHITE);
            }
            let mut pb = PathBuilder::new();
            for k in 0..4 {
                if let Some(r) = tiny_skia::Rect::from_xywh(u.px(0.26), u.py(0.68 + k as f32 * 0.05), u.d(if k == 3 { 0.26 } else { 0.48 }), u.d(0.018).max(0.8)) {
                    pb.push_rect(r);
                }
            }
            pm.fill(pb.finish(), &solid(rgba(120, 124, 136, 200)));
        }
        _ => {
            // Código: linhas coloridas recuadas como num editor.
            let rows = [(0.0, 0.3, rgba(180, 60, 160, 230)), (0.06, 0.38, rgba(60, 110, 200, 230)), (0.12, 0.26, rgba(40, 150, 80, 230)), (0.12, 0.34, rgba(110, 110, 120, 230)), (0.06, 0.2, rgba(60, 110, 200, 230)), (0.0, 0.1, rgba(180, 60, 160, 230)), (0.0, 0.32, rgba(200, 110, 40, 230)), (0.06, 0.4, rgba(110, 110, 120, 230)), (0.0, 0.1, rgba(180, 60, 160, 230))];
            for (k, (ind, w, c)) in rows.iter().enumerate() {
                pm.fill(u.rr(0.26 + ind, 0.32 + k as f32 * 0.062, *w, 0.026, 0.012), &solid(*c));
            }
        }
    }
}

/// Ejetar: triângulo e barra em grafite polido, com chanfro e reflexo.
pub(crate) fn eject(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let tri = u.poly(&[(0.5, 0.12), (0.88, 0.58), (0.12, 0.58)]);
    let bar = u.rr(0.12, 0.68, 0.76, 0.16, 0.03);
    for p in [tri.clone(), bar.clone()] {
        soft_shadow(pm, p.clone(), s, 0.9);
        pm.fill(p.clone().and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.025)))), &solid(rgba(20, 20, 26, 255)));
        pm.fill(p.clone(), &vgrad(u.py(0.1), u.py(0.86), &[(0.0, rgba(140, 144, 156, 255)), (0.45, rgba(70, 74, 86, 255)), (1.0, rgba(36, 38, 46, 255))]));
        pm.stroke(p, &solid(rgba(16, 16, 22, 255)), u.lw(0.008));
    }
    pm.fill_clipped(u.ov(0.2, 0.06, 0.6, 0.36), &vgrad(u.py(0.12), u.py(0.42), &[(0.0, rgba(255, 255, 255, 170)), (1.0, rgba(255, 255, 255, 10))]), tri);
    pm.fill_clipped(u.rr(0.12, 0.68, 0.76, 0.07, 0.0), &solid(rgba(255, 255, 255, 70)), bar);
}

/// Pendrive: corpo de plástico branco brilhante com o conector de metal e a luz.
pub(crate) fn removable(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let t = Transform::from_rotate_at(-35.0, u.px(0.5), u.py(0.5));
    let body = tf(u.rr(0.14, 0.36, 0.56, 0.28, 0.08), t);
    let plug = tf(u.rr(0.68, 0.4, 0.2, 0.2, 0.01), t);
    soft_shadow(pm, body.clone(), s, 1.0);
    soft_shadow(pm, plug.clone(), s, 0.8);
    pm.fill(plug.clone(), &tpaint(metal_paint(u.py(0.4), u.py(0.6)), t));
    pm.stroke(plug, &solid(rgba(90, 92, 100, 255)), u.lw(0.007));
    for yy in [0.45, 0.51] {
        pm.fill(tf(u.rr(0.75, yy, 0.06, 0.04, 0.0), t), &solid(rgba(60, 62, 70, 255)));
    }
    pm.fill(tf(u.rr(0.14, 0.38, 0.56, 0.28, 0.08), t), &solid(rgba(150, 154, 166, 255)));
    pm.fill(body.clone(), &tpaint(vgrad(u.py(0.36), u.py(0.64), &[(0.0, rgba(255, 255, 255, 255)), (0.6, rgba(230, 232, 238, 255)), (1.0, rgba(190, 194, 204, 255))]), t));
    pm.fill_clipped(tf(u.rr(0.16, 0.37, 0.52, 0.1, 0.05), t), &solid(rgba(255, 255, 255, 200)), body.clone());
    pm.stroke(body, &solid(rgba(120, 124, 134, 255)), u.lw(0.007));
    pm.fill(tf(u.circ(0.24, 0.5, 0.03), t), &radial(u.px(0.3), u.py(0.55), u.d(0.05), &[(0.0, rgba(180, 230, 255, 255)), (1.0, rgba(30, 120, 220, 255))]));
}

/// CD/DVD: disco prateado com o furo, reflexos coloridos em leque.
pub(crate) fn optical(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let (cx, cy, r) = (0.5, 0.5, 0.42);
    let disc = u.circ(cx, cy, r);
    soft_shadow(pm, disc.clone(), s, 0.9);
    pm.fill(disc.clone(), &linear(u.px(0.1), u.py(0.1), u.px(0.9), u.py(0.9), &[(0.0, rgba(236, 238, 244, 255)), (0.5, rgba(196, 200, 210, 255)), (1.0, rgba(150, 154, 166, 255))]));
    // Leques de cor (arco-íris do disco): triângulos finos saindo do centro.
    let fans = [(-0.75, rgba(110, 190, 255, 200)), (-0.4, rgba(210, 130, 255, 170)), (2.4, rgba(255, 190, 100, 200)), (2.75, rgba(120, 255, 170, 170))];
    for (a, c) in fans {
        let p = |a: f32| (cx + a.cos() * r * 1.1, cy + a.sin() * r * 1.1);
        let tri = u.poly(&[(cx, cy), p(a), p(a + 0.45)]);
        pm.fill_clipped(tri, &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, with_alpha(c, 0.0)), (0.35, with_alpha(c, 0.3)), (0.75, c), (1.0, with_alpha(c, 0.4))]), disc.clone());
    }
    // Brilho branco em leque cruzando o disco.
    let p = |a: f32| (cx + a.cos() * r * 1.1, cy + a.sin() * r * 1.1);
    pm.fill_clipped(u.poly(&[(cx, cy), p(-2.3), p(-2.0)]), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(255, 255, 255, 0)), (0.6, rgba(255, 255, 255, 200)), (1.0, rgba(255, 255, 255, 60))]), disc.clone());
    pm.fill_clipped(u.poly(&[(cx, cy), p(0.85), p(1.15)]), &radial(u.px(cx), u.py(cy), u.d(r), &[(0.0, rgba(255, 255, 255, 0)), (0.6, rgba(255, 255, 255, 170)), (1.0, rgba(255, 255, 255, 40))]), disc.clone());
    if s >= 40.0 {
        let mut rings = PathBuilder::new();
        for rr in [0.2, 0.39] {
            rings.push_circle(u.px(cx), u.py(cy), u.d(rr));
        }
        pm.stroke(rings.finish(), &solid(rgba(255, 255, 255, 120)), u.lw(0.004));
    }
    pm.stroke(disc, &solid(rgba(110, 114, 124, 255)), u.lw(0.007));
    pm.fill(u.circ(cx, cy, 0.13), &solid(rgba(226, 230, 238, 230)));
    pm.stroke(u.circ(cx, cy, 0.13), &solid(rgba(150, 154, 166, 255)), u.lw(0.005));
    let hole = u.circ(cx, cy, 0.055);
    pm.fill(hole.clone(), &solid(rgba(250, 250, 252, 255)));
    pm.stroke(hole, &solid(rgba(110, 114, 124, 255)), u.lw(0.006));
}

/// Servidor (compartilhamento de rede): gabinete de alumínio em pé com o globo azul na frente.
pub(crate) fn server(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let front = u.rr(0.18, 0.12, 0.5, 0.8, 0.04);
    let side = u.poly(&[(0.66, 0.14), (0.84, 0.06), (0.84, 0.84), (0.66, 0.92)]);
    soft_shadow(pm, u.poly(&[(0.18, 0.12), (0.84, 0.06), (0.84, 0.84), (0.66, 0.92), (0.18, 0.92)]), s, 1.0);
    pm.fill(side.clone(), &vgrad(u.py(0.06), u.py(0.92), &[(0.0, rgba(190, 192, 200, 255)), (1.0, rgba(120, 124, 134, 255))]));
    pm.stroke(side, &solid(rgba(90, 92, 100, 255)), u.lw(0.007));
    pm.fill(u.poly(&[(0.2, 0.12), (0.38, 0.04), (0.84, 0.06), (0.66, 0.13)]), &vgrad(u.py(0.04), u.py(0.13), &[(0.0, rgba(252, 252, 254, 255)), (1.0, rgba(214, 216, 222, 255))]));
    pm.fill(front.clone(), &metal_paint(u.py(0.12), u.py(0.92)));
    brush_streaks(pm, front.clone(), u, 53, false);
    pm.stroke(front.clone(), &solid(rgba(90, 92, 100, 255)), u.lw(0.008));
    // Gavetas de disco
    for k in 0..3 {
        let yy = 0.2 + k as f32 * 0.12;
        pm.fill(u.rr(0.24, yy, 0.38, 0.08, 0.01), &vgrad(u.py(yy), u.py(yy + 0.08), &[(0.0, rgba(70, 72, 80, 255)), (1.0, rgba(140, 142, 150, 255))]));
        pm.fill(u.circ(0.57, yy + 0.04, 0.012), &solid(rgba(110, 255, 140, 255)));
    }
    // Globo azul de rede
    let g = u.circ(0.43, 0.72, 0.14);
    pm.fill(g.clone(), &radial(u.px(0.39), u.py(0.67), u.d(0.2), &[(0.0, rgba(150, 210, 255, 255)), (0.6, rgba(40, 120, 220, 255)), (1.0, rgba(10, 50, 140, 255))]));
    if s >= 28.0 {
        let mut grid = PathBuilder::new();
        grid.push_oval(tiny_skia::Rect::from_xywh(u.px(0.37), u.py(0.58), u.d(0.12), u.d(0.28)).unwrap());
        grid.move_to(u.px(0.29), u.py(0.72));
        grid.line_to(u.px(0.57), u.py(0.72));
        pm.stroke(grid.finish(), &solid(rgba(220, 240, 255, 160)), u.lw(0.006));
    }
    glass_glare(pm, g.clone(), u.px(0.29), u.py(0.58), u.px(0.57), u.py(0.7), 140);
    pm.stroke(g, &solid(rgba(10, 40, 110, 255)), u.lw(0.006));
}
