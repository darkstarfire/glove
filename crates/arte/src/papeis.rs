//! Papéis de parede em gel do Glove OS (`glove-arte papeis <pasta> [logo.png]`): 20 imagens
//! 4K com a luva e o nome "Glove OS", em cinco cores e em quatro formatos:
//!
//! - `4k`: um monitor (3840×2160);
//! - `dupla`: dois monitores lado a lado (7680×2160, para o modo panorama);
//! - `empilhada`: dois monitores um em cima do outro (3840×4320, panorama);
//! - `ultrawide`: um monitor 21:9 (5120×2160).
//!
//! Tudo é desenhado aqui, como o resto do Glove: o fundo em degradê, faixas de gel com o
//! brilho na borda, gotas de gel e pontos de luz. Só a luva vem de um arquivo (o logo do
//! Glove OS, 1024 px). Em panorama, a luva fica sempre dentro de um dos monitores (nunca na
//! emenda) e o resto do desenho atravessa os dois.

use std::path::Path;

use glove_paint::{
    draw::{linear, radial, rgba, with_alpha, Canvas},
    tiny_skia::{BlendMode, Color, FilterQuality, LineCap, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform},
    Fonts,
};

struct Pal {
    name: &'static str,
    deep: [u8; 3],
    mid: [u8; 3],
    light: [u8; 3],
}

const PALS: [Pal; 5] = [
    Pal { name: "azul", deep: [12, 46, 132], mid: [44, 108, 204], light: [150, 202, 252] },
    Pal { name: "agua", deep: [4, 62, 104], mid: [20, 134, 182], light: [160, 232, 248] },
    Pal { name: "grafite", deep: [30, 34, 44], mid: [88, 96, 112], light: [204, 210, 222] },
    Pal { name: "noite", deep: [12, 12, 48], mid: [50, 46, 138], light: [166, 156, 244] },
    Pal { name: "menta", deep: [6, 66, 64], mid: [30, 144, 126], light: [172, 242, 218] },
];

#[derive(Clone, Copy)]
enum Txt {
    Below,
    Right,
}

/// Onde vai a luva: o monitor, a posição do grupo (luva, nome e reflexo) dentro da área
/// segura do monitor (0 = encostado à esquerda/em cima, 1 = à direita/embaixo), a altura
/// da luva em fração da altura do monitor (o máximo: diminui se não couber) e onde fica o nome.
type Layout = (usize, f32, f32, f32, Txt);

struct Fmt {
    id: &'static str,
    w: u32,
    h: u32,
    /// Os monitores, em frações da imagem (x, y, l, a).
    mons: &'static [[f32; 4]],
    /// As telas em que o papel tem de ficar bom (a mais estreita e a mais larga): no
    /// "Preencher a tela", uma tela mais estreita que a imagem corta os lados, e uma mais
    /// larga corta em cima e embaixo. A luva fica no que sobra nas duas.
    aspects: (f32, f32),
    layouts: [Layout; 5],
}

const FMTS: [Fmt; 4] = [
    Fmt {
        id: "4k",
        w: 3840,
        h: 2160,
        mons: &[[0.0, 0.0, 1.0, 1.0]],
        aspects: (4.0 / 3.0, 16.0 / 9.0),
        layouts: [
            (0, 0.50, 0.45, 0.40, Txt::Below),
            (0, 0.08, 0.50, 0.34, Txt::Below),
            (0, 0.95, 0.70, 0.24, Txt::Right),
            (0, 0.90, 0.10, 0.22, Txt::Below),
            (0, 0.10, 0.85, 0.28, Txt::Right),
        ],
    },
    Fmt {
        id: "dupla",
        w: 7680,
        h: 2160,
        mons: &[[0.0, 0.0, 0.5, 1.0], [0.5, 0.0, 0.5, 1.0]],
        aspects: (4.0 / 3.0, 16.0 / 9.0),
        layouts: [
            (0, 0.50, 0.45, 0.40, Txt::Below),
            (1, 0.50, 0.45, 0.40, Txt::Below),
            (0, 0.70, 0.60, 0.28, Txt::Right),
            (1, 0.30, 0.70, 0.26, Txt::Right),
            (0, 0.15, 0.15, 0.24, Txt::Below),
        ],
    },
    Fmt {
        id: "empilhada",
        w: 3840,
        h: 4320,
        mons: &[[0.0, 0.0, 1.0, 0.5], [0.0, 0.5, 1.0, 0.5]],
        aspects: (4.0 / 3.0, 16.0 / 9.0),
        layouts: [
            (0, 0.50, 0.50, 0.40, Txt::Below),
            (1, 0.50, 0.40, 0.40, Txt::Below),
            (0, 0.10, 0.70, 0.28, Txt::Right),
            (1, 0.90, 0.30, 0.26, Txt::Right),
            (0, 0.85, 0.15, 0.24, Txt::Below),
        ],
    },
    Fmt {
        id: "ultrawide",
        w: 5120,
        h: 2160,
        mons: &[[0.0, 0.0, 1.0, 1.0]],
        aspects: (16.0 / 9.0, 24.0 / 9.0),
        layouts: [
            (0, 0.50, 0.45, 0.42, Txt::Below),
            (0, 0.05, 0.55, 0.34, Txt::Below),
            (0, 0.90, 0.60, 0.28, Txt::Right),
            (0, 0.95, 0.10, 0.22, Txt::Below),
            (0, 0.15, 0.80, 0.30, Txt::Right),
        ],
    },
];

/// Números pseudoaleatórios repetíveis (a mesma imagem a cada geração).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

fn col(c: [u8; 3], a: f32) -> Color {
    with_alpha(rgba(c[0], c[1], c[2], 255), a)
}

fn mixc(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Desfoque (três passadas de caixa, separáveis) num pixmap pré-multiplicado.
fn blur(pm: &mut Pixmap, r: usize) {
    if r == 0 {
        return;
    }
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let data = pm.data_mut();
    let mut tmp = vec![0u8; data.len()];
    for _ in 0..3 {
        pass(data, &mut tmp, w, h, r, true);
        pass(&tmp, data, w, h, r, false);
    }
}

/// Uma passada de caixa (horizontal ou vertical) de `src` para `dst`.
fn pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horiz: bool) {
    let (n, lines) = if horiz { (w, h) } else { (h, w) };
    let idx = |line: usize, i: usize| if horiz { (line * w + i) * 4 } else { (i * w + line) * 4 };
    let div = (2 * r + 1) as u32;
    for line in 0..lines {
        let mut acc = [0u32; 4];
        for k in 0..=2 * r {
            let i = (k as isize - r as isize).clamp(0, n as isize - 1) as usize;
            let p = idx(line, i);
            for ch in 0..4 {
                acc[ch] += src[p + ch] as u32;
            }
        }
        for i in 0..n {
            let p = idx(line, i);
            for ch in 0..4 {
                dst[p + ch] = (acc[ch] / div) as u8;
            }
            let out = idx(line, (i as isize - r as isize).clamp(0, n as isize - 1) as usize);
            let inn = idx(line, (i + r + 1).min(n - 1));
            for ch in 0..4 {
                acc[ch] = acc[ch] + src[inn + ch] as u32 - src[out + ch] as u32;
            }
        }
    }
}

/// Desenha `src` sobre `dst` com a transformação dada. Sempre pelo bilinear: o bicúbico
/// passa do ponto nos pixels quase transparentes (a sombra do logo) e deixa um contorno claro.
fn put(dst: &mut Pixmap, src: &Pixmap, t: Transform, opacity: f32, mode: BlendMode) {
    let paint = PixmapPaint { opacity, blend_mode: mode, quality: FilterQuality::Bilinear };
    dst.draw_pixmap(0, 0, src.as_ref(), &paint, t, None);
}

/// Uma camada que só guarda a forma (alfa) de `src`, pintada de `c` (para sombra e brilho).
fn tinted(src: &Pixmap, c: [u8; 3], a: f32) -> Pixmap {
    let mut out = src.clone();
    for px in out.data_mut().chunks_exact_mut(4) {
        let al = px[3] as f32 * a;
        px[0] = (c[0] as f32 * al / 255.0) as u8;
        px[1] = (c[1] as f32 * al / 255.0) as u8;
        px[2] = (c[2] as f32 * al / 255.0) as u8;
        px[3] = al as u8;
    }
    out
}

/// O retângulo do que não é transparente.
fn content_box(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (w, h) = (pm.width(), pm.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, px) in pm.data().chunks_exact(4).enumerate() {
        if px[3] > 8 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x1 <= x0 { (0, 0, w, h) } else { (x0, y0, x1 - x0, y1 - y0) }
}

/// Faixa de gel: a borda de cima numa curva de um lado ao outro, com a espessura variando.
struct Ribbon {
    y: [f32; 4],
    t0: f32,
    t1: f32,
    alpha: f32,
}

fn ribbon_point(r: &Ribbon, s: f32, off: f32) -> (f32, f32) {
    let x = -0.08 + 1.16 * s;
    let u = 1.0 - s;
    let y = u * u * u * r.y[0] + 3.0 * u * u * s * r.y[1] + 3.0 * u * s * s * r.y[2] + s * s * s * r.y[3];
    (x, y + off * (r.t0 + (r.t1 - r.t0) * s))
}

fn ribbon_path(r: &Ribbon, w: f32, h: f32, off: f32) -> PathBuilder {
    let mut pb = PathBuilder::new();
    for i in 0..=96 {
        let (x, y) = ribbon_point(r, i as f32 / 96.0, off);
        if i == 0 { pb.move_to(x * w, y * h) } else { pb.line_to(x * w, y * h) }
    }
    pb
}

/// O nome "Glove OS" em branco brilhante (o degradê desce para a cor clara do papel).
fn name_layer(fonts: &Fonts, px: f32, pal: &Pal) -> Option<(Pixmap, f32, f32)> {
    let text = "Glove OS";
    let tw = fonts.measure(text, px, false);
    let pad = px * 0.3;
    let (w, h) = (tw + pad * 2.0, px * 1.4 + pad);
    let mut c = Canvas::new(w.ceil(), h.ceil(), 1.0)?;
    let base = pad + px * 0.95;
    fonts.draw(&mut c, text, pad, base, px, false, rgba(255, 255, 255, 255));
    let (_, _, _, cap) = fonts.v_metrics(px);
    let top = base - cap;
    let low = mixc(pal.light, [255, 255, 255], 0.35);
    let pw = c.pm.width() as usize;
    for (i, p) in c.pm.data_mut().chunks_exact_mut(4).enumerate() {
        let y = (i / pw) as f32;
        let t = ((y - top) / cap.max(1.0)).clamp(0.0, 1.0);
        // Gel: a metade de cima clara, uma virada no meio, a de baixo na cor do papel.
        let k = if t < 0.5 { t * 0.25 } else { 0.35 + (t - 0.5) * 1.3 };
        let cc = mixc([255, 255, 255], low, k.min(1.0));
        let a = p[3] as f32 / 255.0;
        p[0] = (cc[0] as f32 * a) as u8;
        p[1] = (cc[1] as f32 * a) as u8;
        p[2] = (cc[2] as f32 * a) as u8;
    }
    Some((c.pm, pad, base))
}

/// A área segura de um monitor (x, y, l, a em pixels da imagem): o que aparece em todas
/// as telas de `fmt.aspects` no "Preencher a tela", menos a barra de menu em cima, o Dock
/// embaixo e a coluna dos ícones da mesa à direita (com folga).
fn safe_area(fmt: &Fmt, m: [f32; 4]) -> [f32; 4] {
    let (mx, my, mw, mh) = (m[0] * fmt.w as f32, m[1] * fmt.h as f32, m[2] * fmt.w as f32, m[3] * fmt.h as f32);
    let a = mw / mh;
    // Fração visível na tela mais estreita (largura) e na mais larga (altura).
    let vw = (fmt.aspects.0 / a).min(1.0);
    let vh = (a / fmt.aspects.1).min(1.0);
    let (x0, x1) = (mx + mw * (1.0 - vw) / 2.0, mx + mw * (1.0 + vw) / 2.0);
    let (y0, y1) = (my + mh * (1.0 - vh) / 2.0, my + mh * (1.0 + vh) / 2.0);
    let (vw, vh) = (x1 - x0, y1 - y0);
    let (l, r, t, b) = (x0 + vw * 0.06, x1 - vw * 0.13, y0 + vh * 0.08, y1 - vh * 0.17);
    [l, t, r - l, b - t]
}

/// Onde ficam a luva e o nome.
struct Geo {
    lx: f32,
    ly: f32,
    lw: f32,
    lh: f32,
    px: f32,
    cap: f32,
    tw: f32,
    gap: f32,
    /// O grupo inteiro (luva, nome e reflexo).
    group: [f32; 4],
}

fn geometry(fmt: &Fmt, k: usize, logo_box: (u32, u32, u32, u32), fonts: &Fonts) -> Geo {
    let (mi, gx, gy, size, txt) = fmt.layouts[k];
    let mh = fmt.mons[mi][3] * fmt.h as f32;
    let safe = safe_area(fmt, fmt.mons[mi]);
    let (_, _, bw, bh) = logo_box;
    let mut lh = size * mh;
    loop {
        let lw = bw as f32 * lh / bh as f32;
        let px = match txt {
            Txt::Below => lh * 0.27,
            Txt::Right => lh * 0.36,
        };
        let (_, _, _, cap) = fonts.v_metrics(px);
        let tw = fonts.measure("Glove OS", px, false);
        let gap = lh * 0.10;
        // O reflexo desce uns 70% da altura das letras abaixo da linha de base.
        let refl = cap * 0.75;
        let (gw, gh) = match txt {
            Txt::Below => (lw.max(tw), lh + gap + cap + refl),
            Txt::Right => (lw + gap * 1.4 + tw, lh.max(lh / 2.0 + cap / 2.0 + refl)),
        };
        if (gw > safe[2] || gh > safe[3]) && lh > mh * 0.08 {
            lh *= 0.95;
            continue;
        }
        let gx0 = safe[0] + gx * (safe[2] - gw).max(0.0);
        let gy0 = safe[1] + gy * (safe[3] - gh).max(0.0);
        let (lx, ly) = match txt {
            Txt::Below => (gx0 + gw / 2.0 - lw / 2.0, gy0),
            Txt::Right => (gx0, gy0),
        };
        return Geo { lx, ly, lw, lh, px, cap, tw, gap, group: [gx0, gy0, gw, gh] };
    }
}

fn render(fmt: &Fmt, k: usize, logo: &Pixmap, fonts: &Fonts) -> Option<Pixmap> {
    let (w, h) = (fmt.w as f32, fmt.h as f32);
    let pal = &PALS[(k + FMTS.iter().position(|f| f.id == fmt.id).unwrap_or(0)) % PALS.len()];
    let (mi, _, _, _, txt) = fmt.layouts[k];
    let mh = fmt.mons[mi][3] * h;
    let s = mh / 2160.0;
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((fmt.w as u64) << 20) ^ ((fmt.h as u64) << 4) ^ k as u64 * 0x5851_f42d);
    for _ in 0..8 {
        rng.next();
    }

    // Medidas do grupo (luva e nome), para o fundo já saber onde fica a luz.
    let (bx, by, bw, bh) = content_box(logo);
    let g = geometry(fmt, k, (bx, by, bw, bh), fonts);
    let (lx, ly, lw, lh, px, cap, tw, gap) = (g.lx, g.ly, g.lw, g.lh, g.px, g.cap, g.tw, g.gap);
    let lsc = lh / bh as f32;
    let (gw, gh) = (g.group[2], g.group[3]);
    let (cx, cy) = (g.group[0] + gw / 2.0, g.group[1] + gh / 2.0);
    let lcx = lx + lw / 2.0;
    let lcy = ly + lh / 2.0;

    let mut c = Canvas::new(w, h, 1.0)?;
    // Fundo: degradê na diagonal e a luz atrás da luva.
    let a = rng.range(0.0, 1.0);
    c.fill_rect(0.0, 0.0, w, h, &linear(w * a * 0.3, 0.0, w * (0.7 + a * 0.3), h, &[(0.0, col(pal.deep, 1.0)), (0.55, col(pal.mid, 1.0)), (1.0, col(mixc(pal.mid, pal.deep, 0.5), 1.0))]));
    c.fill_rect(0.0, 0.0, w, h, &radial(lcx, lcy, mh * 0.95, &[(0.0, col(pal.light, 0.55)), (0.45, col(pal.light, 0.18)), (1.0, col(pal.light, 0.0))]));

    // Camada do brilho (um quarto do tamanho, desfocada e somada no fim).
    const Q: f32 = 4.0;
    let mut glow = Pixmap::new((w / Q) as u32, (h / Q) as u32)?;
    let qs = Transform::from_scale(1.0 / Q, 1.0 / Q);

    // Faixas de gel.
    let n = 3 + (rng.next() * 3.0) as usize;
    for i in 0..n {
        let y0 = rng.range(-0.1, 1.0);
        let y3 = (y0 + rng.range(-0.9, 0.9)).clamp(-0.2, 1.2);
        let r = Ribbon {
            y: [y0, y0 + rng.range(-0.4, 0.4), y3 + rng.range(-0.4, 0.4), y3],
            t0: rng.range(0.03, 0.22),
            t1: rng.range(0.03, 0.22),
            alpha: rng.range(0.10, 0.22) * if i == 0 { 1.4 } else { 1.0 },
        };
        let mut band = ribbon_path(&r, w, h, 0.0);
        for j in (0..=96).rev() {
            let (x, y) = ribbon_point(&r, j as f32 / 96.0, 1.0);
            band.line_to(x * w, y * h);
        }
        band.close();
        let (ax, ay) = ribbon_point(&r, 0.5, 0.0);
        let (_, by1) = ribbon_point(&r, 0.5, 1.0);
        c.fill(
            band.finish(),
            &linear(ax * w, ay * h, ax * w, by1 * h, &[(0.0, col(pal.light, r.alpha)), (0.35, col(pal.light, r.alpha * 0.35)), (1.0, col(pal.light, 0.0))]),
        );
        // O brilho da borda: um fio nítido e o halo em volta.
        let edge = ribbon_path(&r, w, h, 0.0).finish();
        let soft = ribbon_path(&r, w, h, 1.0).finish();
        if let Some(p) = &edge {
            let st = Stroke { width: 3.0 * s, line_cap: LineCap::Round, ..Default::default() };
            c.pm.stroke_path(p, &glove_paint::draw::solid(with_alpha(rgba(255, 255, 255, 255), 0.75)), &st, Transform::identity(), None);
            let st = Stroke { width: 26.0 * s, line_cap: LineCap::Round, ..Default::default() };
            glow.stroke_path(p, &glove_paint::draw::solid(with_alpha(rgba(255, 255, 255, 255), 0.55)), &st, qs, None);
        }
        if let Some(p) = &soft {
            let st = Stroke { width: 2.0 * s, line_cap: LineCap::Round, ..Default::default() };
            c.pm.stroke_path(p, &glove_paint::draw::solid(col(pal.light, 0.30)), &st, Transform::identity(), None);
        }
    }

    // Gotas de gel, longe da luva e do nome.
    let avoid = |x: f32, y: f32, r: f32| {
        let dx = (x - cx).abs() - gw / 2.0 - r;
        let dy = (y - cy).abs() - gh / 2.0 - r - cap;
        dx < mh * 0.04 && dy < mh * 0.04
    };
    let drops = 2 + (rng.next() * 4.0) as usize;
    let mut placed: Vec<(f32, f32, f32)> = Vec::new();
    for _ in 0..80 {
        if placed.len() == drops {
            break;
        }
        let r = rng.range(0.025, 0.11) * mh;
        let (x, y) = (rng.range(0.0, w), rng.range(0.0, h));
        if avoid(x, y, r) || placed.iter().any(|&(px, py, pr)| ((px - x).powi(2) + (py - y).powi(2)).sqrt() < pr + r + mh * 0.02) {
            continue;
        }
        placed.push((x, y, r));
        // O corpo transparente, mais claro na borda.
        c.fill(glove_paint::draw::circle(x, y, r), &radial(x, y + r * 0.15, r, &[(0.0, col(pal.light, 0.06)), (0.75, col(pal.light, 0.14)), (1.0, col(pal.light, 0.38))]));
        // A luz que atravessa e se junta embaixo.
        c.fill(glove_paint::draw::oval(x - r * 0.62, y + r * 0.18, r * 1.24, r * 0.7), &radial(x, y + r * 0.62, r * 0.6, &[(0.0, col([255, 255, 255], 0.28)), (1.0, col([255, 255, 255], 0.0))]));
        // O reflexo em cima.
        c.fill(
            glove_paint::draw::oval(x - r * 0.62, y - r * 0.9, r * 1.24, r * 0.78),
            &linear(0.0, y - r * 0.9, 0.0, y - r * 0.12, &[(0.0, col([255, 255, 255], 0.75)), (1.0, col([255, 255, 255], 0.0))]),
        );
        glow.fill_path(
            &PathBuilder::from_circle(x, y, r * 1.05)?,
            &glove_paint::draw::solid(col(pal.light, 0.22)),
            glove_paint::tiny_skia::FillRule::Winding,
            qs,
            None,
        );
    }
    // Pontos de luz.
    for _ in 0..40 {
        let r = rng.range(4.0, 26.0) * s;
        let (x, y) = (rng.range(0.0, w), rng.range(0.0, h));
        if let Some(p) = PathBuilder::from_circle(x, y, r) {
            glow.fill_path(&p, &glove_paint::draw::solid(col([255, 255, 255], rng.range(0.15, 0.6))), glove_paint::tiny_skia::FillRule::Winding, qs, None);
        }
    }
    blur(&mut glow, (10.0 * s) as usize + 2);
    put(&mut c.pm, &glow, Transform::from_scale(Q, Q), 1.0, BlendMode::Screen);

    // Cantos mais escuros em cada monitor.
    for m in fmt.mons {
        let (x, y, ww, hh) = (m[0] * w, m[1] * h, m[2] * w, m[3] * h);
        let r = (ww * ww + hh * hh).sqrt() * 0.62;
        c.fill_rect(x, y, ww, hh, &radial(x + ww / 2.0, y + hh / 2.0, r, &[(0.0, col(pal.deep, 0.0)), (0.6, col(pal.deep, 0.0)), (1.0, col(pal.deep, 0.55))]));
    }

    // A luva: halo, sombra e ela.
    c.fill_rect(lcx - lh * 1.2, lcy - lh * 1.2, lh * 2.4, lh * 2.4, &radial(lcx, lcy, lh * 0.9, &[(0.0, col([255, 255, 255], 0.22)), (1.0, col([255, 255, 255], 0.0))]));
    let lt = Transform::from_translate(-(bx as f32), -(by as f32)).post_scale(lsc, lsc).post_translate(lx, ly);
    let mut sh = Pixmap::new((w / Q) as u32, (h / Q) as u32)?;
    put(&mut sh, &tinted(logo, pal.deep, 0.75), lt.post_translate(0.0, lh * 0.045).post_scale(1.0 / Q, 1.0 / Q), 1.0, BlendMode::SourceOver);
    blur(&mut sh, (lh * 0.012) as usize + 1);
    put(&mut c.pm, &sh, Transform::from_scale(Q, Q), 1.0, BlendMode::SourceOver);
    put(&mut c.pm, logo, lt, 1.0, BlendMode::SourceOver);

    // O nome, com sombra e reflexo.
    let (name, pad, base) = name_layer(fonts, px, pal)?;
    let (nx, nbase) = match txt {
        Txt::Below => (cx - tw / 2.0, ly + lh + gap + cap),
        Txt::Right => (lx + lw + gap * 1.4, lcy + cap / 2.0),
    };
    let nt = Transform::from_translate(nx - pad, nbase - base);
    let mut nsh = tinted(&name, pal.deep, 0.65);
    blur(&mut nsh, (px * 0.035) as usize + 1);
    put(&mut c.pm, &nsh, nt.post_translate(0.0, px * 0.035), 1.0, BlendMode::SourceOver);
    put(&mut c.pm, &name, nt, 1.0, BlendMode::SourceOver);
    // Reflexo: o nome de cabeça para baixo, sumindo.
    let mut refl = name.clone();
    let rw = refl.width() as usize;
    for (i, p) in refl.data_mut().chunks_exact_mut(4).enumerate() {
        let y = (i / rw) as f32;
        let d = (base - y) / cap.max(1.0);
        let f = if d < 0.0 { 0.0 } else { (0.32 * (1.0 - d / 0.7)).clamp(0.0, 0.32) };
        for v in p.iter_mut() {
            *v = (*v as f32 * f) as u8;
        }
    }
    let rt = Transform::from_row(1.0, 0.0, 0.0, -1.0, nx - pad, nbase + base + px * 0.06);
    put(&mut c.pm, &refl, rt, 1.0, BlendMode::SourceOver);
    Some(c.pm)
}

/// JPEG com um pouco de ruído (sem faixas nos degradês).
fn save_jpeg(pm: &Pixmap, path: &Path) -> Result<(), String> {
    let mut rgb = Vec::with_capacity((pm.width() * pm.height() * 3) as usize);
    let mut seed = 0x2545_f491u32;
    for p in pm.pixels() {
        let d = p.demultiply();
        for v in [d.red(), d.green(), d.blue()] {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let n = (seed % 3) as i32 - 1;
            rgb.push((v as i32 + n).clamp(0, 255) as u8);
        }
    }
    let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 90);
    enc.encode(&rgb, pm.width(), pm.height(), image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())
}

/// Miniatura (para as Preferências não abrirem a imagem inteira): reduz pela metade até
/// chegar perto de `max_w` e faz o último passo direto (cada metade é uma média de 2×2).
fn mini(pm: &Pixmap, max_w: u32) -> Option<Pixmap> {
    let mut cur = pm.clone();
    while cur.width() / 2 >= max_w {
        let mut half = Pixmap::new(cur.width() / 2, cur.height() / 2)?;
        put(&mut half, &cur, Transform::from_scale(0.5, 0.5), 1.0, BlendMode::Source);
        cur = half;
    }
    let k = max_w as f32 / cur.width() as f32;
    let mut out = Pixmap::new(max_w, ((cur.height() as f32 * k).round() as u32).max(1))?;
    put(&mut out, &cur, Transform::from_scale(k, k), 1.0, BlendMode::Source);
    Some(out)
}

/// A fonte do nome: Lato (a do pacote fonts-lato), ou a do sistema.
fn name_font() -> Option<Fonts> {
    for f in ["/usr/share/fonts/truetype/lato/Lato-Semibold.ttf", "/usr/share/fonts/truetype/lato/Lato-Medium.ttf"] {
        if let Some(fo) = Fonts::load_file(Path::new(f), 0) {
            return Some(fo);
        }
    }
    Fonts::load_system()
}

/// Gera os 20 papéis em `dir` (`só`: só os que têm esse pedaço no nome, para testar).
pub fn papeis(dir: &Path, logo: &Path, only: Option<&str>) -> Result<(), String> {
    std::fs::create_dir_all(dir.join("mini")).map_err(|e| e.to_string())?;
    let logo = glove_paint::image::decode(logo)?;
    let fonts = name_font().ok_or("sem fonte")?;
    for (fi, fmt) in FMTS.iter().enumerate() {
        for k in 0..5 {
            let name = format!("gel-{}-{}.jpg", PALS[(k + fi) % PALS.len()].name, fmt.id);
            if only.is_some_and(|o| !name.contains(o)) {
                continue;
            }
            let pm = render(fmt, k, &logo, &fonts).ok_or("desenho")?;
            save_jpeg(&pm, &dir.join(&name))?;
            save_jpeg(&mini(&pm, 480).ok_or("miniatura")?, &dir.join("mini").join(&name))?;
            println!("{name}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A luva, o nome e o reflexo cabem na área segura em todos os papéis, e a área segura
    /// fica dentro do monitor (nada cortado em telas 4:3 a 16:9, nem embaixo do Dock).
    #[test]
    fn grupo_na_area_segura() {
        let Some(fonts) = name_font() else { return };
        let logo_box = (114, 13, 795, 997);
        for fmt in &FMTS {
            for k in 0..5 {
                let g = geometry(fmt, k, logo_box, &fonts);
                let m = fmt.mons[fmt.layouts[k].0];
                let safe = safe_area(fmt, m);
                let [x, y, w, h] = g.group;
                let e = 0.5;
                assert!(x >= safe[0] - e && y >= safe[1] - e && x + w <= safe[0] + safe[2] + e && y + h <= safe[1] + safe[3] + e, "{} {k}: {:?} fora de {:?}", fmt.id, g.group, safe);
                let (mx, my, mw, mh) = (m[0] * fmt.w as f32, m[1] * fmt.h as f32, m[2] * fmt.w as f32, m[3] * fmt.h as f32);
                assert!(safe[0] >= mx && safe[1] >= my && safe[0] + safe[2] <= mx + mw && safe[1] + safe[3] <= my + mh);
                // Nem pequena demais.
                assert!(g.lh >= mh * 0.15, "{} {k}: luva pequena ({})", fmt.id, g.lh / mh);
            }
        }
    }
}
