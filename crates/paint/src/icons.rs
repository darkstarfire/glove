//! Ícones vetoriais originais, desenhados em qualquer tamanho.

use crate::draw::*;
use crate::font::Fonts;
use tiny_skia::PathBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Terminal,
    Folder,
    Globe,
    TextEditor,
    Calculator,
    Settings,
    Window,
    Generic,
    /// Relógio mostrando (hora, minuto).
    Clock(u8, u8),
    Trash,
    Disk,
    /// Arquivo de texto.
    Document,
    /// Arquivo de imagem.
    Picture,
    /// Arquivo de tipo desconhecido.
    File,
    Notes,
    Search,
    Chars,
    Camera,
    Monitor,
    Chess,
    /// Tela plana (preferência de Monitor).
    Display,
    Mouse,
    Trackpad,
}

impl Icon {
    pub fn from_name(s: &str) -> Self {
        match s {
            "terminal" => Icon::Terminal,
            "folder" | "files" => Icon::Folder,
            "globe" | "browser" => Icon::Globe,
            "editor" | "text" => Icon::TextEditor,
            "calculator" => Icon::Calculator,
            "settings" => Icon::Settings,
            "window" => Icon::Window,
            "clock" => Icon::Clock(10, 10),
            "trash" => Icon::Trash,
            "disk" => Icon::Disk,
            "document" => Icon::Document,
            "picture" | "image" => Icon::Picture,
            "file" => Icon::File,
            "notes" => Icon::Notes,
            "search" => Icon::Search,
            "chars" => Icon::Chars,
            "camera" => Icon::Camera,
            "monitor" => Icon::Monitor,
            "display" => Icon::Display,
            "mouse" => Icon::Mouse,
            "trackpad" => Icon::Trackpad,
            "chess" => Icon::Chess,
            _ => Icon::Generic,
        }
    }
}

/// Desenha o ícone ocupando o quadrado (x, y, s, s).
pub fn draw(pm: &mut Canvas, fonts: &Fonts, icon: Icon, x: f32, y: f32, s: f32, letter: Option<char>) {
    // Sombra de contato suave sob o ícone (mesa, Arquivos).
    pm.fill(oval(x + s * 0.12, y + s * 0.86, s * 0.76, s * 0.12),
        &radial(x + s * 0.5, y + s * 0.92, s * 0.4, &[(0.0, rgba(0, 0, 0, 90)), (1.0, rgba(0, 0, 0, 0))]),
    );
    draw_bare(pm, fonts, icon, x, y, s, letter);
}

/// O ícone sem a sombra de contato (no Dock os ícones ficam direto no vidro).
pub fn draw_bare(pm: &mut Canvas, fonts: &Fonts, icon: Icon, x: f32, y: f32, s: f32, letter: Option<char>) {
    match icon {
        Icon::Terminal => terminal(pm, x, y, s),
        Icon::Folder => folder(pm, x, y, s),
        Icon::Globe => globe(pm, x, y, s),
        Icon::TextEditor => editor(pm, x, y, s),
        Icon::Calculator => calculator(pm, x, y, s),
        Icon::Settings => gear(pm, x, y, s),
        Icon::Window => window(pm, fonts, x, y, s, letter),
        Icon::Generic => generic(pm, fonts, x, y, s, letter),
        Icon::Clock(h, m) => clock(pm, x, y, s, h, m),
        Icon::Trash => trash(pm, x, y, s),
        Icon::Disk => disk(pm, x, y, s),
        Icon::Document => page(pm, x, y, s, true),
        Icon::File => page(pm, x, y, s, false),
        Icon::Picture => picture(pm, x, y, s),
        Icon::Notes => notes(pm, x, y, s),
        Icon::Search => search(pm, x, y, s),
        Icon::Chars => chars(pm, fonts, x, y, s),
        Icon::Camera => camera(pm, x, y, s),
        Icon::Monitor => monitor(pm, x, y, s),
        Icon::Chess => chess(pm, fonts, x, y, s),
        Icon::Display => display(pm, x, y, s),
        Icon::Mouse => mouse(pm, x, y, s),
        Icon::Trackpad => trackpad(pm, x, y, s),
    }
}

fn gloss(pm: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32) {
    pm.fill(rrect4(x, y, w, h, [r, r, 2.0, 2.0]),
        &vgrad(y, y + h, &[(0.0, rgba(255, 255, 255, 170)), (1.0, rgba(255, 255, 255, 10))]),
    );
}

fn terminal(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.12, s * 0.84, s * 0.72);
    // Moldura de plástico claro
    pm.fill(rrect(bx, by, bw, bh, s * 0.1), &vgrad(by, by + bh, &[(0.0, rgba(235, 235, 238, 255)), (1.0, rgba(160, 162, 170, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.1), &solid(rgba(90, 90, 100, 255)), (s / 64.0).max(1.0));
    // Tela
    let (sx, sy, sw, sh) = (bx + s * 0.07, by + s * 0.07, bw - s * 0.14, bh - s * 0.16);
    pm.fill(rrect(sx, sy, sw, sh, s * 0.05), &vgrad(sy, sy + sh, &[(0.0, rgba(30, 34, 40, 255)), (1.0, rgba(8, 10, 12, 255))]));
    // Prompt ">_"
    let green = rgba(90, 240, 120, 255);
    let lw = (s * 0.055).max(1.5);
    let mut pb = PathBuilder::new();
    pb.move_to(sx + sw * 0.16, sy + sh * 0.3);
    pb.line_to(sx + sw * 0.34, sy + sh * 0.5);
    pb.line_to(sx + sw * 0.16, sy + sh * 0.7);
    pm.stroke(pb.finish(), &solid(green), lw);
    pm.fill_rect(sx + sw * 0.42, sy + sh * 0.66, sw * 0.22, lw, &solid(green));
    gloss(pm, sx + 1.0, sy + 1.0, sw - 2.0, sh * 0.45, s * 0.05);
}

fn folder(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let base = rgba(70, 150, 240, 255);
    let (fx, fy, fw, fh) = (x + s * 0.08, y + s * 0.2, s * 0.84, s * 0.64);
    // Aba
    pm.fill(rrect4(fx, fy - s * 0.07, fw * 0.42, s * 0.16, [s * 0.05, s * 0.05, 0.0, 0.0]), &solid(darken(base, 0.2)));
    // Fundo
    pm.fill(rrect(fx, fy, fw, fh, s * 0.06), &solid(darken(base, 0.25)));
    // Frente
    let (ffy, ffh) = (fy + fh * 0.18, fh * 0.82);
    pm.fill(rrect(fx, ffy, fw, ffh, s * 0.06), &vgrad(ffy, ffy + ffh, &[(0.0, lighten(base, 0.35)), (1.0, darken(base, 0.1))]));
    pm.stroke(rrect(fx, ffy, fw, ffh, s * 0.06), &solid(darken(base, 0.5)), (s / 64.0).max(1.0));
    gloss(pm, fx + 2.0, ffy + 1.5, fw - 4.0, ffh * 0.4, s * 0.05);
}

fn globe(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.48, s * 0.38);
    gel_circle(pm, cx, cy, r, rgba(40, 120, 220, 255));
    let p = solid(rgba(255, 255, 255, 120));
    let lw = (s / 48.0).max(1.0);
    pm.stroke(oval(cx - r * 0.45, cy - r + 1.0, r * 0.9, r * 2.0 - 2.0), &p, lw);
    pm.stroke(oval(cx - r + 1.0, cy - r * 0.35, r * 2.0 - 2.0, r * 0.7), &p, lw);
    pm.fill_rect(cx - lw / 2.0, cy - r + 1.0, lw, r * 2.0 - 2.0, &p);
    pm.fill_rect(cx - r + 1.0, cy - lw / 2.0, r * 2.0 - 2.0, lw, &p);
}

fn editor(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (px, py, pw, ph) = (x + s * 0.18, y + s * 0.08, s * 0.6, s * 0.78);
    let fold = s * 0.16;
    let mut pb = PathBuilder::new();
    pb.move_to(px, py);
    pb.line_to(px + pw - fold, py);
    pb.line_to(px + pw, py + fold);
    pb.line_to(px + pw, py + ph);
    pb.line_to(px, py + ph);
    pb.close();
    let page = pb.finish();
    pm.fill(page.clone(), &vgrad(py, py + ph, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(225, 228, 235, 255))]));
    pm.stroke(page, &solid(rgba(120, 120, 130, 255)), (s / 64.0).max(1.0));
    // Dobra
    let mut pb = PathBuilder::new();
    pb.move_to(px + pw - fold, py);
    pb.line_to(px + pw - fold, py + fold);
    pb.line_to(px + pw, py + fold);
    pb.close();
    pm.fill(pb.finish(), &solid(rgba(200, 204, 212, 255)));
    // Linhas de texto
    for i in 0..6 {
        let ly = py + ph * 0.28 + i as f32 * ph * 0.11;
        let lw = if i == 5 { pw * 0.4 } else { pw * 0.68 };
        pm.fill_rect(px + pw * 0.14, ly, lw, (s * 0.025).max(1.0), &solid(rgba(150, 160, 180, 255)));
    }
    // Lápis
    let mut pb = PathBuilder::new();
    let (ax, ay) = (x + s * 0.88, y + s * 0.30);
    let (bx, by) = (x + s * 0.48, y + s * 0.78);
    let t = s * 0.06;
    pb.move_to(ax - t, ay - t);
    pb.line_to(ax + t, ay + t);
    pb.line_to(bx + t * 0.3, by + t * 0.9);
    pb.line_to(bx - t * 0.9, by - t * 0.3);
    pb.close();
    pm.fill(pb.finish(), &linear(ax - t, ay, ax + t, ay, &[(0.0, rgba(250, 200, 60, 255)), (1.0, rgba(210, 150, 20, 255))]));
    pm.fill(circle(bx - t * 0.2, by + t * 0.2, t * 0.55), &solid(rgba(60, 50, 40, 255)));
}

fn calculator(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (bx, by, bw, bh) = (x + s * 0.2, y + s * 0.08, s * 0.6, s * 0.8);
    pm.fill(rrect(bx, by, bw, bh, s * 0.08), &vgrad(by, by + bh, &[(0.0, rgba(110, 112, 120, 255)), (1.0, rgba(50, 52, 58, 255))]));
    let (lx, ly, lw, lh) = (bx + bw * 0.1, by + bh * 0.08, bw * 0.8, bh * 0.2);
    pm.fill(rrect(lx, ly, lw, lh, s * 0.03), &vgrad(ly, ly + lh, &[(0.0, rgba(190, 220, 190, 255)), (1.0, rgba(150, 190, 150, 255))]));
    let k = bw * 0.8 / 4.0;
    for row in 0..4 {
        for col in 0..4 {
            let kx = lx + col as f32 * k + k * 0.12;
            let ky = by + bh * 0.36 + row as f32 * k + k * 0.12;
            let base = if col == 3 { rgba(240, 150, 40, 255) } else { rgba(210, 212, 220, 255) };
            pm.fill(rrect(kx, ky, k * 0.76, k * 0.76, k * 0.18), &vgrad(ky, ky + k * 0.76, &[(0.0, lighten(base, 0.3)), (1.0, base)]));
        }
    }
    gloss(pm, bx + 2.0, by + 1.5, bw - 4.0, bh * 0.25, s * 0.07);
}

fn gear(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy) = (x + s * 0.5, y + s * 0.48);
    let (ro, ri) = (s * 0.42, s * 0.32);
    let teeth = 10;
    let mut pb = PathBuilder::new();
    for i in 0..teeth * 4 {
        let a = i as f32 / (teeth * 4) as f32 * std::f32::consts::TAU;
        let r = if (i / 2) % 2 == 0 { ro } else { ri };
        let (px, py) = (cx + a.cos() * r, cy + a.sin() * r);
        if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
    }
    pb.close();
    let path = pb.finish();
    pm.fill(path.clone(), &linear(x, y, x + s, y + s, &[(0.0, rgba(225, 228, 235, 255)), (1.0, rgba(110, 115, 125, 255))]));
    pm.stroke(path, &solid(rgba(70, 72, 80, 255)), (s / 64.0).max(1.0));
    pm.fill(circle(cx, cy, s * 0.13), &radial(cx, cy - s * 0.04, s * 0.15, &[(0.0, rgba(60, 62, 70, 255)), (1.0, rgba(20, 22, 26, 255))]));
}

fn window(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let (wx, wy, ww, wh) = (x + s * 0.1, y + s * 0.16, s * 0.8, s * 0.66);
    pm.fill(rrect(wx, wy, ww, wh, s * 0.06), &solid(rgba(250, 250, 250, 255)));
    pm.pinstripes(wx, wy + s * 0.14, ww, wh - s * 0.14, rgba(232, 232, 232, 255), rgba(246, 246, 246, 255));
    pm.fill(rrect4(wx, wy, ww, s * 0.14, [s * 0.06, s * 0.06, 0.0, 0.0]), &vgrad(wy, wy + s * 0.14, &[(0.0, rgba(240, 240, 240, 255)), (1.0, rgba(195, 195, 200, 255))]));
    pm.stroke(rrect(wx, wy, ww, wh, s * 0.06), &solid(rgba(110, 110, 120, 255)), 1.0);
    for (i, c) in [rgba(230, 70, 60, 255), rgba(240, 180, 40, 255), rgba(80, 190, 70, 255)].into_iter().enumerate() {
        pm.fill(circle(wx + s * 0.08 + i as f32 * s * 0.08, wy + s * 0.07, s * 0.03), &solid(c));
    }
    if let Some(l) = letter {
        let t = l.to_uppercase().to_string();
        let px = s * 0.34;
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, wx + (ww - tw) / 2.0, wy + wh * 0.78, px, true, rgba(60, 70, 90, 255));
    }
}

fn generic(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let (bx, by, bw) = (x + s * 0.12, y + s * 0.1, s * 0.76);
    gel_rect(pm, bx, by, bw, bw, s * 0.18, rgba(120, 130, 160, 255));
    if let Some(l) = letter {
        let t = l.to_uppercase().to_string();
        let px = s * 0.42;
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, bx + (bw - tw) / 2.0, by + bw * 0.68, px, true, rgba(255, 255, 255, 240));
    }
}

/// Marca do menu principal: uma gota de gel azul.
pub fn logo(pm: &mut Canvas, x: f32, y: f32, s: f32, accent: tiny_skia::Color) {
    gel_circle(pm, x + s / 2.0, y + s / 2.0, s / 2.0, accent);
}

fn clock(pm: &mut Canvas, x: f32, y: f32, s: f32, h: u8, m: u8) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.47, s * 0.4);
    // Aro de metal + mostrador branco
    pm.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(245, 246, 250, 255)), (0.5, rgba(160, 165, 176, 255)), (1.0, rgba(230, 232, 238, 255))]));
    let fr = r * 0.84;
    pm.fill(circle(cx, cy, fr), &radial(cx, cy + fr * 0.2, fr * 1.1, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(212, 215, 224, 255))]));
    for i in 0..12 {
        let a = i as f32 / 12.0 * TAU;
        let mut pb = PathBuilder::new();
        pb.move_to(cx + a.cos() * fr * 0.78, cy + a.sin() * fr * 0.78);
        pb.line_to(cx + a.cos() * fr * 0.92, cy + a.sin() * fr * 0.92);
        pm.stroke(pb.finish(), &solid(rgba(50, 52, 60, 255)), (s / 40.0).max(1.0));
    }
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * TAU - FRAC_PI_2;
    let ma = m as f32 / 60.0 * TAU - FRAC_PI_2;
    for (a, len, w) in [(ha, fr * 0.5, s / 16.0), (ma, fr * 0.78, s / 24.0)] {
        let mut pb = PathBuilder::new();
        pb.move_to(cx, cy);
        pb.line_to(cx + a.cos() * len, cy + a.sin() * len);
        pm.stroke(pb.finish(), &solid(rgba(35, 38, 46, 255)), w.max(1.0));
    }
    pm.fill(circle(cx, cy, (s * 0.035).max(1.2)), &solid(rgba(215, 50, 40, 255)));
    pm.fill(oval(cx - fr * 0.7, cy - fr * 0.92, fr * 1.4, fr * 0.8), &vgrad(cy - fr, cy - fr * 0.1, &[(0.0, rgba(255, 255, 255, 130)), (1.0, rgba(255, 255, 255, 0))]));
}

/// Lixeira de metal escovado com nervuras verticais e tampa com alça.
fn trash(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (top, bot) = (y + s * 0.24, y + s * 0.88);
    let (tl, tr) = (x + s * 0.2, x + s * 0.8);
    let (bl, br) = (x + s * 0.27, x + s * 0.73);
    let mut pb = PathBuilder::new();
    pb.move_to(tl, top);
    pb.line_to(tr, top);
    pb.line_to(br, bot);
    pb.line_to(bl, bot);
    pb.close();
    let body = pb.finish();
    pm.fill(body.clone(), &linear(tl, 0.0, tr, 0.0, &[(0.0, rgba(150, 155, 165, 255)), (0.35, rgba(235, 238, 244, 255)), (0.7, rgba(190, 195, 205, 255)), (1.0, rgba(130, 135, 145, 255))]));
    // Nervuras
    for i in 1..6 {
        let t = i as f32 / 6.0;
        let (xa, xb) = (tl + (tr - tl) * t, bl + (br - bl) * t);
        let mut r = PathBuilder::new();
        r.move_to(xa, top + s * 0.03);
        r.line_to(xb, bot - s * 0.03);
        pm.stroke(r.finish(), &solid(rgba(110, 115, 125, 150)), (s / 64.0).max(0.8));
    }
    pm.stroke(body, &solid(rgba(90, 95, 105, 255)), (s / 64.0).max(1.0));
    // Tampa + alça
    let lid_y = y + s * 0.16;
    pm.fill(rrect(x + s * 0.15, lid_y, s * 0.7, s * 0.09, s * 0.03), &vgrad(lid_y, lid_y + s * 0.09, &[(0.0, rgba(245, 246, 250, 255)), (1.0, rgba(150, 155, 165, 255))]));
    pm.stroke(rrect(x + s * 0.15, lid_y, s * 0.7, s * 0.09, s * 0.03), &solid(rgba(90, 95, 105, 255)), (s / 64.0).max(1.0));
    pm.fill(rrect(x + s * 0.4, lid_y - s * 0.05, s * 0.2, s * 0.06, s * 0.02), &solid(rgba(120, 125, 135, 255)));
}

/// Disco: caixa de metal com frente escura e luz de atividade verde.
fn disk(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (bx, by, bw, bh) = (x + s * 0.1, y + s * 0.3, s * 0.8, s * 0.46);
    // Tampa em perspectiva
    let mut pb = PathBuilder::new();
    pb.move_to(bx + s * 0.06, by - s * 0.1);
    pb.line_to(bx + bw - s * 0.06, by - s * 0.1);
    pb.line_to(bx + bw, by);
    pb.line_to(bx, by);
    pb.close();
    pm.fill(pb.finish(), &vgrad(by - s * 0.1, by, &[(0.0, rgba(250, 251, 253, 255)), (1.0, rgba(205, 208, 216, 255))]));
    pm.fill(rrect(bx, by, bw, bh, s * 0.04), &vgrad(by, by + bh, &[(0.0, rgba(225, 228, 235, 255)), (1.0, rgba(150, 154, 164, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.04), &solid(rgba(95, 100, 110, 255)), (s / 64.0).max(1.0));
    // Frente com grade de ventilação
    let (fx, fy, fw, fh) = (bx + s * 0.06, by + bh * 0.52, bw - s * 0.12, bh * 0.3);
    pm.fill(rrect(fx, fy, fw, fh, s * 0.02), &solid(rgba(70, 74, 84, 255)));
    for i in 0..7 {
        pm.fill_rect(fx + s * 0.03 + i as f32 * s * 0.055, fy + fh * 0.25, s * 0.03, fh * 0.5, &solid(rgba(40, 42, 48, 255)));
    }
    let (lx, ly) = (bx + bw - s * 0.11, by + bh * 0.25);
    pm.fill(circle(lx, ly, s * 0.035), &radial(lx, ly, s * 0.05, &[(0.0, rgba(170, 255, 170, 255)), (1.0, rgba(40, 160, 60, 255))]));
}

/// Folha com canto dobrado; `lines` desenha linhas de texto.
fn page(pm: &mut Canvas, x: f32, y: f32, s: f32, lines: bool) {
    let (px, py, pw, ph) = (x + s * 0.2, y + s * 0.08, s * 0.6, s * 0.8);
    let fold = s * 0.18;
    let mut pb = PathBuilder::new();
    pb.move_to(px, py);
    pb.line_to(px + pw - fold, py);
    pb.line_to(px + pw, py + fold);
    pb.line_to(px + pw, py + ph);
    pb.line_to(px, py + ph);
    pb.close();
    let p = pb.finish();
    pm.fill(p.clone(), &vgrad(py, py + ph, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(228, 231, 238, 255))]));
    pm.stroke(p, &solid(rgba(125, 128, 138, 255)), (s / 64.0).max(1.0));
    let mut pb = PathBuilder::new();
    pb.move_to(px + pw - fold, py);
    pb.line_to(px + pw - fold, py + fold);
    pb.line_to(px + pw, py + fold);
    pb.close();
    pm.fill(pb.finish(), &vgrad(py, py + fold, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(196, 200, 210, 255))]));
    if lines {
        for i in 0..7 {
            let ly = py + ph * 0.3 + i as f32 * ph * 0.09;
            let lw = if i == 6 { pw * 0.35 } else { pw * 0.68 };
            pm.fill_rect(px + pw * 0.14, ly, lw, (s * 0.022).max(0.8), &solid(rgba(140, 150, 172, 255)));
        }
    }
}

/// Moldura com uma paisagem simples (céu, sol e montanhas).
fn picture(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (fx, fy, fw, fh) = (x + s * 0.1, y + s * 0.18, s * 0.8, s * 0.62);
    pm.fill(rrect(fx, fy, fw, fh, s * 0.03), &vgrad(fy, fy + fh, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(215, 218, 226, 255))]));
    pm.stroke(rrect(fx, fy, fw, fh, s * 0.03), &solid(rgba(120, 124, 134, 255)), (s / 64.0).max(1.0));
    let (ix, iy, iw, ih) = (fx + s * 0.06, fy + s * 0.06, fw - s * 0.12, fh - s * 0.12);
    pm.fill_rect(ix, iy, iw, ih, &vgrad(iy, iy + ih, &[(0.0, rgba(90, 160, 235, 255)), (1.0, rgba(190, 225, 250, 255))]));
    pm.fill(circle(ix + iw * 0.75, iy + ih * 0.3, s * 0.06), &solid(rgba(255, 215, 80, 255)));
    let mut pb = PathBuilder::new();
    pb.move_to(ix, iy + ih);
    pb.line_to(ix + iw * 0.3, iy + ih * 0.45);
    pb.line_to(ix + iw * 0.5, iy + ih * 0.75);
    pb.line_to(ix + iw * 0.68, iy + ih * 0.55);
    pb.line_to(ix + iw, iy + ih);
    pb.close();
    pm.fill(pb.finish(), &vgrad(iy + ih * 0.45, iy + ih, &[(0.0, rgba(90, 160, 80, 255)), (1.0, rgba(50, 110, 50, 255))]));
}

/// Pilha de lembretes coloridos com um alfinete.
fn notes(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    for (i, col) in [rgba(150, 200, 255, 255), rgba(255, 170, 200, 255), rgba(255, 235, 120, 255)].iter().enumerate() {
        let o = i as f32 * s * 0.06;
        let (nx, ny, nw) = (x + s * 0.14 + o, y + s * 0.12 + o, s * 0.66);
        pm.fill(rrect(nx, ny, nw, nw, s * 0.02), &vgrad(ny, ny + nw, &[(0.0, lighten(*col, 0.25)), (1.0, *col)]));
        pm.stroke(rrect(nx, ny, nw, nw, s * 0.02), &solid(darken(*col, 0.35)), lw);
        if i == 2 {
            for k in 0..4 {
                let ly = ny + nw * 0.32 + k as f32 * nw * 0.15;
                pm.fill_rect(nx + nw * 0.14, ly, nw * if k == 3 { 0.4 } else { 0.7 }, (s * 0.022).max(0.8), &solid(rgba(150, 120, 40, 200)));
            }
            gel_circle(pm, nx + nw * 0.5, ny + nw * 0.1, s * 0.06, rgba(220, 50, 40, 255));
        }
    }
}

/// Lupa de vidro com cabo escuro.
fn search(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy, r) = (x + s * 0.42, y + s * 0.4, s * 0.26);
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r * 0.7, cy + r * 0.7);
    pb.line_to(x + s * 0.84, y + s * 0.84);
    pm.stroke(pb.finish(), &solid(rgba(50, 40, 35, 255)), s * 0.11);
    pm.fill(circle(cx, cy, r + s * 0.05), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(240, 242, 248, 255)), (1.0, rgba(120, 125, 140, 255))]));
    pm.fill(circle(cx, cy, r), &radial(cx, cy + r * 0.3, r * 1.2, &[(0.0, rgba(225, 240, 255, 230)), (1.0, rgba(120, 170, 230, 230))]));
    pm.fill(oval(cx - r * 0.7, cy - r * 0.85, r * 1.2, r * 0.8), &vgrad(cy - r, cy, &[(0.0, rgba(255, 255, 255, 200)), (1.0, rgba(255, 255, 255, 0))]));
}

/// Teclas com letras de alfabetos diferentes.
fn chars(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let k = s * 0.36;
    for (i, ch) in ["A", "Ω", "é", "★"].iter().enumerate() {
        let (kx, ky) = (x + s * 0.1 + (i % 2) as f32 * (k + s * 0.08), y + s * 0.1 + (i / 2) as f32 * (k + s * 0.08));
        pm.fill(rrect(kx, ky, k, k, s * 0.06), &vgrad(ky, ky + k, &[(0.0, rgba(252, 252, 254, 255)), (1.0, rgba(200, 204, 214, 255))]));
        pm.stroke(rrect(kx, ky, k, k, s * 0.06), &solid(rgba(110, 115, 128, 255)), (s / 64.0).max(1.0));
        let px = k * 0.62;
        let tw = fonts.measure(ch, px, true);
        fonts.draw(pm, ch, kx + (k - tw) / 2.0, ky + k * 0.72, px, true, rgba(40, 50, 80, 255));
    }
}

/// Câmera fotográfica prateada com lente azulada.
fn camera(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.28, s * 0.84, s * 0.54);
    pm.fill(rrect(x + s * 0.3, y + s * 0.18, s * 0.3, s * 0.14, s * 0.03), &solid(rgba(90, 95, 105, 255)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.08), &vgrad(by, by + bh, &[(0.0, rgba(235, 237, 242, 255)), (1.0, rgba(150, 155, 166, 255))]));
    pm.fill_rect(bx, by + bh * 0.3, bw, bh * 0.4, &solid(rgba(60, 62, 70, 255)));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.08), &solid(rgba(80, 84, 94, 255)), (s / 64.0).max(1.0));
    let (cx, cy) = (x + s * 0.5, by + bh * 0.5);
    pm.fill(circle(cx, cy, s * 0.2), &linear(cx - s * 0.2, cy - s * 0.2, cx + s * 0.2, cy + s * 0.2, &[(0.0, rgba(240, 242, 246, 255)), (1.0, rgba(110, 115, 125, 255))]));
    pm.fill(circle(cx, cy, s * 0.14), &radial(cx, cy, s * 0.14, &[(0.0, rgba(90, 140, 220, 255)), (0.7, rgba(20, 30, 70, 255)), (1.0, rgba(5, 5, 15, 255))]));
    pm.fill(circle(cx - s * 0.05, cy - s * 0.05, s * 0.04), &solid(rgba(255, 255, 255, 180)));
    pm.fill(circle(bx + bw * 0.86, by + bh * 0.15, s * 0.035), &solid(rgba(255, 200, 60, 255)));
}

/// Monitor de atividade: tela escura com gráfico verde.
fn monitor(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.12, s * 0.84, s * 0.62);
    pm.fill(rrect(bx, by, bw, bh, s * 0.06), &vgrad(by, by + bh, &[(0.0, rgba(225, 228, 235, 255)), (1.0, rgba(150, 154, 164, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.06), &solid(rgba(85, 90, 100, 255)), (s / 64.0).max(1.0));
    let (sx, sy, sw, sh) = (bx + s * 0.06, by + s * 0.06, bw - s * 0.12, bh - s * 0.12);
    pm.fill(rrect(sx, sy, sw, sh, s * 0.03), &solid(rgba(10, 20, 15, 255)));
    for i in 1..4 {
        pm.fill_rect(sx, sy + sh * i as f32 / 4.0, sw, (s / 128.0).max(0.5), &solid(rgba(40, 90, 50, 255)));
    }
    let pts = [0.7, 0.55, 0.62, 0.3, 0.45, 0.2, 0.35, 0.5, 0.25];
    let mut pb = PathBuilder::new();
    for (i, v) in pts.iter().enumerate() {
        let (px, py) = (sx + sw * i as f32 / (pts.len() - 1) as f32, sy + sh * v);
        if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
    }
    pm.stroke(pb.finish(), &solid(rgba(90, 255, 110, 255)), (s / 40.0).max(1.0));
    pm.fill(rrect(x + s * 0.38, by + bh, s * 0.24, s * 0.1, 0.0), &solid(rgba(140, 144, 154, 255)));
}

/// Cavalo de xadrez sobre uma casa do tabuleiro.
fn chess(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32) {
    let (bx, by, bw) = (x + s * 0.1, y + s * 0.5, s * 0.8);
    for i in 0..4 {
        let c = if i % 2 == 0 { rgba(235, 215, 175, 255) } else { rgba(150, 100, 60, 255) };
        pm.fill_rect(bx + i as f32 * bw / 4.0, by, bw / 4.0, s * 0.3, &vgrad(by, by + s * 0.3, &[(0.0, lighten(c, 0.15)), (1.0, c)]));
    }
    pm.stroke(rrect(bx, by, bw, s * 0.3, 0.0), &solid(rgba(80, 55, 30, 255)), (s / 64.0).max(1.0));
    let px = s * 0.72;
    let tw = fonts.measure("♞", px, false);
    fonts.draw(pm, "♞", x + (s - tw) / 2.0 + s * 0.02, y + s * 0.74 + s * 0.02, px, false, rgba(0, 0, 0, 90));
    fonts.draw(pm, "♞", x + (s - tw) / 2.0, y + s * 0.74, px, false, rgba(25, 25, 30, 255));
}

/// Tela plana sobre um pé de alumínio, com um céu azul na tela.
fn display(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    // Pé
    pm.fill(rrect(x + s * 0.4, y + s * 0.66, s * 0.2, s * 0.16, 0.0), &vgrad(y + s * 0.66, y + s * 0.82, &[(0.0, rgba(150, 152, 160, 255)), (1.0, rgba(205, 207, 212, 255))]));
    pm.fill(rrect(x + s * 0.26, y + s * 0.8, s * 0.48, s * 0.07, s * 0.035), &vgrad(y + s * 0.8, y + s * 0.87, &[(0.0, rgba(230, 231, 235, 255)), (1.0, rgba(140, 142, 150, 255))]));
    // Moldura clara
    let (bx, by, bw, bh) = (x + s * 0.06, y + s * 0.1, s * 0.88, s * 0.6);
    pm.fill(rrect(bx, by, bw, bh, s * 0.05), &vgrad(by, by + bh, &[(0.0, rgba(245, 246, 248, 255)), (1.0, rgba(190, 193, 200, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.05), &solid(rgba(110, 114, 124, 255)), lw);
    // Tela com céu
    let (sx, sy, sw, sh) = (bx + s * 0.05, by + s * 0.05, bw - s * 0.1, bh - s * 0.1);
    pm.fill(rrect(sx, sy, sw, sh, s * 0.02), &vgrad(sy, sy + sh, &[(0.0, rgba(40, 90, 200, 255)), (0.7, rgba(110, 170, 240, 255)), (1.0, rgba(190, 220, 250, 255))]));
    // Faixa de onda clara
    let mut pb = PathBuilder::new();
    pb.move_to(sx, sy + sh * 0.75);
    pb.quad_to(sx + sw * 0.35, sy + sh * 0.45, sx + sw, sy + sh * 0.62);
    pb.line_to(sx + sw, sy + sh * 0.78);
    pb.quad_to(sx + sw * 0.4, sy + sh * 0.6, sx, sy + sh * 0.9);
    pb.close();
    pm.fill(pb.finish(), &solid(rgba(255, 255, 255, 110)));
    gloss(pm, sx + 1.0, sy + 1.0, sw - 2.0, sh * 0.42, s * 0.02);
}

/// Mouse de um botão em plástico translúcido, com o fio saindo por cima.
fn mouse(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    // Fio
    let mut pb = PathBuilder::new();
    pb.move_to(x + s * 0.5, y + s * 0.2);
    pb.cubic_to(x + s * 0.5, y + s * 0.08, x + s * 0.72, y + s * 0.1, x + s * 0.78, y + s * 0.02);
    pm.stroke(pb.finish(), &solid(rgba(120, 124, 135, 255)), (s * 0.03).max(1.0));
    let (mx, my, mw, mh) = (x + s * 0.24, y + s * 0.18, s * 0.52, s * 0.74);
    // Sombra
    pm.fill(oval(mx + s * 0.02, my + s * 0.04, mw, mh), &solid(rgba(0, 0, 0, 50)));
    // Casca transparente
    pm.fill(oval(mx, my, mw, mh), &vgrad(my, my + mh, &[(0.0, rgba(250, 251, 253, 235)), (1.0, rgba(185, 190, 200, 235))]));
    // Miolo cinza visto pelo plástico
    let (ix, iy, iw, ih) = (mx + mw * 0.16, my + mh * 0.12, mw * 0.68, mh * 0.78);
    pm.fill(oval(ix, iy, iw, ih), &vgrad(iy, iy + ih, &[(0.0, rgba(120, 124, 136, 255)), (1.0, rgba(60, 62, 72, 255))]));
    // Botão (metade de cima) e sua linha
    pm.fill(rrect4(ix, iy, iw, ih * 0.36, [iw * 0.5, iw * 0.5, 0.0, 0.0]), &vgrad(iy, iy + ih * 0.36, &[(0.0, rgba(170, 174, 186, 255)), (1.0, rgba(110, 114, 126, 255))]));
    pm.fill_rect(ix, iy + ih * 0.36, iw, lw, &solid(rgba(40, 42, 50, 255)));
    pm.stroke(oval(mx, my, mw, mh), &solid(rgba(110, 114, 126, 255)), lw);
    // Brilho
    pm.fill(oval(mx + mw * 0.18, my + mh * 0.04, mw * 0.64, mh * 0.3), &vgrad(my, my + mh * 0.34, &[(0.0, rgba(255, 255, 255, 200)), (1.0, rgba(255, 255, 255, 0))]));
}

/// Superfície de toque com o botão largo embaixo.
fn trackpad(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.1, y + s * 0.1, s * 0.8, s * 0.8);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.08), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.08), &vgrad(by, by + bh, &[(0.0, rgba(225, 227, 232, 255)), (1.0, rgba(165, 168, 176, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.08), &solid(rgba(100, 104, 114, 255)), lw);
    // Área de toque
    let (px, py, pw, ph) = (bx + s * 0.08, by + s * 0.08, bw - s * 0.16, bh * 0.58);
    pm.fill(rrect(px, py, pw, ph, s * 0.04), &vgrad(py, py + ph, &[(0.0, rgba(150, 154, 164, 255)), (1.0, rgba(196, 199, 206, 255))]));
    pm.stroke(rrect(px, py, pw, ph, s * 0.04), &solid(rgba(110, 114, 124, 255)), lw);
    // Botão
    let (qy, qh) = (py + ph + s * 0.06, bh * 0.16);
    pm.fill(rrect(px, qy, pw, qh, s * 0.04), &vgrad(qy, qy + qh, &[(0.0, rgba(245, 246, 248, 255)), (1.0, rgba(175, 178, 186, 255))]));
    pm.stroke(rrect(px, qy, pw, qh, s * 0.04), &solid(rgba(110, 114, 124, 255)), lw);
    // Dedo (marca de toque)
    pm.fill(circle(px + pw * 0.62, py + ph * 0.45, s * 0.06), &solid(rgba(70, 130, 230, 150)));
}
