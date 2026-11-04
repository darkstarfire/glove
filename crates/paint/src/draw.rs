//! Primitivas de desenho e o `Canvas`, que desenha em coordenadas lógicas
//! e gera pixels na escala da tela (1×, 1.5×, 2×…), sempre nítido.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point,
    RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Mistura duas cores (t = 0 → a, t = 1 → b).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Color::from_rgba(l(a.red(), b.red()), l(a.green(), b.green()), l(a.blue(), b.blue()), l(a.alpha(), b.alpha()))
        .unwrap_or(a)
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    let mut c = c;
    c.set_alpha(a.clamp(0.0, 1.0));
    c
}

pub fn lighten(c: Color, t: f32) -> Color {
    mix(c, with_alpha(Color::WHITE, c.alpha()), t)
}

pub fn darken(c: Color, t: f32) -> Color {
    mix(c, with_alpha(Color::BLACK, c.alpha()), t)
}

pub fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stops(s: &[(f32, Color)]) -> Vec<GradientStop> {
    s.iter().map(|(p, c)| GradientStop::new(*p, *c)).collect()
}

/// Gradiente linear entre dois pontos.
pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, s: &[(f32, Color)]) -> Paint<'static> {
    let shader = LinearGradient::new(Point::from_xy(x0, y0), Point::from_xy(x1, y1), stops(s), SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(s[0].1));
    Paint { shader, anti_alias: true, ..Default::default() }
}

/// Gradiente vertical (o mais usado).
pub fn vgrad(y0: f32, y1: f32, s: &[(f32, Color)]) -> Paint<'static> {
    linear(0.0, y0, 0.0, y1, s)
}

/// Gradiente radial simples (centro → raio).
pub fn radial(cx: f32, cy: f32, r: f32, s: &[(f32, Color)]) -> Paint<'static> {
    let c = Point::from_xy(cx, cy);
    let shader = RadialGradient::new(c, 0.0, c, r, stops(s), SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(s[s.len() - 1].1));
    Paint { shader, anti_alias: true, ..Default::default() }
}

/// Retângulo com cantos arredondados independentes (tl, tr, br, bl).
pub fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Option<Path> {
    let max = w.min(h) / 2.0;
    let [tl, tr, br, bl] = r.map(|v| v.clamp(0.0, max));
    const K: f32 = 1.0 - 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    if tr > 0.0 {
        pb.cubic_to(x + w - tr * K, y, x + w, y + tr * K, x + w, y + tr);
    }
    pb.line_to(x + w, y + h - br);
    if br > 0.0 {
        pb.cubic_to(x + w, y + h - br * K, x + w - br * K, y + h, x + w - br, y + h);
    }
    pb.line_to(x + bl, y + h);
    if bl > 0.0 {
        pb.cubic_to(x + bl * K, y + h, x, y + h - bl * K, x, y + h - bl);
    }
    pb.line_to(x, y + tl);
    if tl > 0.0 {
        pb.cubic_to(x, y + tl * K, x + tl * K, y, x + tl, y);
    }
    pb.close();
    pb.finish()
}

pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    rrect4(x, y, w, h, [r; 4])
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r)
}

pub fn oval(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    Rect::from_xywh(x, y, w, h).and_then(PathBuilder::from_oval)
}

/// Ruído determinístico barato.
#[inline]
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn blend_px(data: &mut [u8], i: usize, r: f32, g: f32, b: f32, a: f32) {
    let inv = 1.0 - a;
    data[i] = (r * a * 255.0 + data[i] as f32 * inv) as u8;
    data[i + 1] = (g * a * 255.0 + data[i + 1] as f32 * inv) as u8;
    data[i + 2] = (b * a * 255.0 + data[i + 2] as f32 * inv) as u8;
    data[i + 3] = (a * 255.0 + data[i + 3] as f32 * inv) as u8;
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

/// Superfície de desenho em coordenadas lógicas.
pub struct Canvas {
    pub pm: Pixmap,
    pub scale: f32,
    /// Tamanho lógico.
    pub w: f32,
    pub h: f32,
}

impl Canvas {
    pub fn new(w: f32, h: f32, scale: f32) -> Option<Self> {
        let pw = (w * scale).ceil().max(1.0) as u32;
        let ph = (h * scale).ceil().max(1.0) as u32;
        Some(Self { pm: Pixmap::new(pw, ph)?, scale, w, h })
    }

    pub fn px_width(&self) -> u32 {
        self.pm.width()
    }
    pub fn px_height(&self) -> u32 {
        self.pm.height()
    }
    pub fn data(&self) -> &[u8] {
        self.pm.data()
    }

    fn t(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    pub fn clear(&mut self, c: Color) {
        self.pm.fill(c);
    }

    pub fn fill(&mut self, path: Option<Path>, paint: &Paint) {
        if let Some(p) = path {
            let t = self.t();
            self.pm.fill_path(&p, paint, FillRule::Winding, t, None);
        }
    }

    pub fn stroke(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            let t = self.t();
            let s = Stroke { width, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, paint: &Paint) {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let t = self.t();
            self.pm.fill_rect(r, paint, t, None);
        }
    }

    /// Linha de 1 pixel físico (para bordas sempre finas e nítidas).
    pub fn hline(&mut self, x: f32, y: f32, w: f32, c: Color) {
        let s = self.scale;
        let (px, py, pw) = ((x * s).round() as i32, (y * s).floor() as i32, (w * s).round() as i32);
        self.px_rect(px, py, pw, 1, c);
    }

    pub fn vline(&mut self, x: f32, y: f32, h: f32, c: Color) {
        let s = self.scale;
        let (px, py, ph) = ((x * s).floor() as i32, (y * s).round() as i32, (h * s).round() as i32);
        self.px_rect(px, py, 1, ph, c);
    }

    fn px_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let pw = self.pm.width() as i32;
        let ph = self.pm.height() as i32;
        let data = self.pm.data_mut();
        for yy in y.max(0)..(y + h).min(ph) {
            for xx in x.max(0)..(x + w).min(pw) {
                blend_px(data, ((yy * pw + xx) * 4) as usize, c.red(), c.green(), c.blue(), c.alpha());
            }
        }
    }

    fn px_range(&self, x: f32, y: f32, w: f32, h: f32) -> (i32, i32, i32, i32) {
        let s = self.scale;
        let pw = self.pm.width() as i32;
        let ph = self.pm.height() as i32;
        let x0 = ((x * s).round() as i32).clamp(0, pw);
        let y0 = ((y * s).round() as i32).clamp(0, ph);
        let x1 = (((x + w) * s).round() as i32).clamp(0, pw);
        let y1 = (((y + h) * s).round() as i32).clamp(0, ph);
        (x0, y0, x1, y1)
    }

    /// Listras finas horizontais; `a` e `b` alternam a cada 2 linhas lógicas.
    pub fn pinstripes(&mut self, x: f32, y: f32, w: f32, h: f32, a: Color, b: Color) {
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        let s = self.scale;
        let pw = self.pm.width() as i32;
        let data = self.pm.data_mut();
        for yy in y0..y1 {
            let ly = ((yy as f32 + 0.5) / s - y).floor() as i32;
            let c = if (ly / 2) % 2 == 0 { a } else { b };
            if c.alpha() <= 0.0 {
                continue;
            }
            for xx in x0..x1 {
                blend_px(data, ((yy * pw + xx) * 4) as usize, c.red(), c.green(), c.blue(), c.alpha());
            }
        }
    }

    /// Metal escovado: gradiente vertical + riscos horizontais aleatórios.
    pub fn brushed_metal(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        let s = self.scale;
        let pw = self.pm.width() as i32;
        let data = self.pm.data_mut();
        let span = (y1 - y0).max(1) as f32;
        for yy in y0..y1 {
            let base = mix(top, bottom, (yy - y0) as f32 / span);
            let row = hash(yy as u32 * 7919) as f32 / u32::MAX as f32;
            for xx in x0..x1 {
                let seg = hash(((xx as f32 / (23.0 * s)) as u32) ^ (yy as u32).wrapping_mul(2_654_435_761));
                let n = (row - 0.5) * 0.07 + ((seg & 0xff) as f32 / 255.0 - 0.5) * 0.035;
                let c = |v: f32| (v + n).clamp(0.0, 1.0);
                blend_px(data, ((yy * pw + xx) * 4) as usize, c(base.red()), c(base.green()), c(base.blue()), base.alpha());
            }
        }
    }

    /// Arredonda os cantos superiores apagando os pixels de fora.
    pub fn clip_top_corners(&mut self, r: f32) {
        let r = r * self.scale;
        let w = self.pm.width() as i32;
        let ri = r.ceil() as i32;
        let data = self.pm.data_mut();
        for y in 0..ri {
            for dx in 0..ri {
                let (px, py) = (dx as f32 + 0.5, y as f32 + 0.5);
                if px >= r || py >= r {
                    continue;
                }
                let d = ((r - px).powi(2) + (r - py).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0);
                if cov >= 1.0 {
                    continue;
                }
                for x in [dx, w - 1 - dx] {
                    let i = ((y * w + x) * 4) as usize;
                    for k in 0..4 {
                        data[i + k] = (data[i + k] as f32 * cov) as u8;
                    }
                }
            }
        }
    }

    /// Preenche `path` só dentro de `clip` (recorte com borda suave).
    pub fn fill_clipped(&mut self, path: Option<Path>, paint: &Paint, clip: Option<Path>) {
        let (Some(p), Some(cl)) = (path, clip) else { return };
        let Some(mut mask) = tiny_skia::Mask::new(self.pm.width(), self.pm.height()) else { return };
        let t = self.t();
        mask.fill_path(&cl, FillRule::Winding, true, t);
        self.pm.fill_path(&p, paint, FillRule::Winding, t, Some(&mask));
    }

    /// Traço com pontas arredondadas.
    pub fn stroke_round(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            let t = self.t();
            let s = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    /// Copia outro canvas (mesma escala) na posição lógica (x, y).
    pub fn draw_canvas(&mut self, other: &Canvas, x: f32, y: f32) {
        let s = self.scale;
        self.pm.draw_pixmap(
            (x * s).round() as i32,
            (y * s).round() as i32,
            other.pm.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    /// Sombra suave de um retângulo arredondado (rw × rh) cobrindo a região
    /// lógica que começa em (ox, oy) relativa ao retângulo.
    pub fn shadow(ox: f32, oy: f32, w: f32, h: f32, rw: f32, rh: f32, spec: &ShadowSpec, scale: f32) -> Option<Canvas> {
        let mut c = Canvas::new(w, h, scale)?;
        let (pw, ph) = (c.pm.width(), c.pm.height());
        let data = c.pm.data_mut();
        let (x0, y0) = (2.0, spec.offset_y + 2.0);
        let (x1, y1) = (rw - 2.0, rh + spec.offset_y - 1.0);
        let r = spec.radius;
        for j in 0..ph {
            let py = oy + (j as f32 + 0.5) / scale;
            for i in 0..pw {
                let px = ox + (i as f32 + 0.5) / scale;
                let qx = (x0 + r - px).max(px - (x1 - r)).max(0.0);
                let qy = (y0 + r - py).max(py - (y1 - r)).max(0.0);
                let d = (qx * qx + qy * qy).sqrt() - r;
                let t = (d / spec.blur).clamp(0.0, 1.0);
                let a = spec.alpha * (1.0 - t).powi(3);
                if a > 0.002 {
                    data[((j * pw + i) * 4 + 3) as usize] = (a * 255.0) as u8;
                }
            }
        }
        Some(c)
    }
}

pub struct ShadowSpec {
    pub blur: f32,
    pub offset_y: f32,
    pub radius: f32,
    pub alpha: f32,
}

// ---------------------------------------------------------------------------
// Gel
// ---------------------------------------------------------------------------

/// Bolha de "gel" redonda: borda escura, corpo com brilho inferior
/// e reflexo branco na metade de cima.
pub fn gel_circle(c: &mut Canvas, cx: f32, cy: f32, r: f32, base: Color) {
    c.fill(circle(cx, cy, r), &solid(darken(base, 0.45)));
    let inner = r - 0.9;
    c.fill(
        circle(cx, cy, inner),
        &radial(cx, cy + inner * 0.55, inner * 1.25, &[(0.0, lighten(base, 0.55)), (0.55, base), (1.0, darken(base, 0.2))]),
    );
    let hw = inner * 1.3;
    let hh = inner * 0.95;
    c.fill(
        oval(cx - hw / 2.0, cy - inner + 0.6, hw, hh),
        &vgrad(cy - inner, cy - inner + hh, &[(0.0, rgba(255, 255, 255, 235)), (1.0, rgba(255, 255, 255, 20))]),
    );
}

/// Pílula/retângulo de gel (botões, destaque de menu, barras).
pub fn gel_rect(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, base: Color) {
    c.fill(rrect(x, y, w, h, r), &solid(darken(base, 0.4)));
    let (ix, iy, iw, ih) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    c.fill(
        rrect(ix, iy, iw, ih, (r - 1.0).max(0.0)),
        &vgrad(iy, iy + ih, &[(0.0, darken(base, 0.05)), (0.5, base), (1.0, lighten(base, 0.6))]),
    );
    let hh = ih * 0.5;
    let rr = (r - 2.0).max(0.0);
    c.fill(
        rrect4(ix + 2.0, iy + 1.0, iw - 4.0, hh, [rr, rr, 2.0, 2.0]),
        &vgrad(iy, iy + hh, &[(0.0, rgba(255, 255, 255, 230)), (1.0, rgba(255, 255, 255, 40))]),
    );
}

// ---------------------------------------------------------------------------
// Progresso
// ---------------------------------------------------------------------------

/// Barra de progresso da era: trilho em gel cinza claro e preenchimento em gel azul.
/// `value` de 0 a 1; `None` = indeterminada, com listras diagonais que andam conforme
/// `phase` (0 a 1 = um período).
pub fn progress_bar(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, value: Option<f32>, phase: f32, accent: Color) {
    let r = (h / 4.0).min(3.0);
    c.fill(rrect(x, y, w, h, r), &solid(rgba(150, 150, 156, 255)));
    let (ix, iy, iw, ih) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    let inner = || rrect(ix, iy, iw, ih, (r - 1.0).max(0.0));
    c.fill(
        inner(),
        &vgrad(iy, iy + ih, &[(0.0, rgba(233, 233, 233, 255)), (0.3, rgba(226, 226, 226, 255)), (0.55, rgba(236, 236, 236, 255)), (0.75, rgba(248, 248, 248, 255)), (1.0, rgba(228, 228, 228, 255))]),
    );
    // Gel azul do 10.4: claro em cima, faixa mais forte no meio, bem claro embaixo.
    let blue = vgrad(iy, iy + ih, &[(0.0, lighten(accent, 0.45)), (0.4, lighten(accent, 0.3)), (0.5, lighten(accent, 0.12)), (1.0, lighten(accent, 0.65))]);
    match value {
        Some(v) => {
            let fw = (iw * v.clamp(0.0, 1.0)).round();
            if fw >= 1.0 {
                let fill = rrect(ix, iy, fw, ih, (r - 1.0).max(0.0));
                c.fill_clipped(fill, &blue, inner());
                // Textura leve de faixas verticais, como no original.
                let mut sx = ix + 3.0;
                while sx < ix + fw - 1.0 {
                    c.vline(sx, iy, ih, rgba(255, 255, 255, 22));
                    sx += 6.0;
                }
            }
        }
        None => {
            // "Poste de barbeiro": listras diagonais azuis sobre azul bem claro.
            c.fill_clipped(inner(), &vgrad(iy, iy + ih, &[(0.0, lighten(accent, 0.9)), (1.0, lighten(accent, 0.78))]), inner());
            let period = (ih * 2.0).max(8.0);
            let stripe = period / 2.0;
            let shift = phase.fract() * period;
            let mut sx = ix - ih - period + shift;
            let mut pb = tiny_skia::PathBuilder::new();
            while sx < ix + iw + ih {
                pb.move_to(sx, iy + ih);
                pb.line_to(sx + ih, iy);
                pb.line_to(sx + ih + stripe, iy);
                pb.line_to(sx + stripe, iy + ih);
                pb.close();
                sx += period;
            }
            c.fill_clipped(pb.finish(), &blue, inner());
        }
    }
    // Brilho em cima (vale para trilho e preenchimento).
    c.fill_clipped(
        rrect(ix, iy, iw, ih * 0.45, 0.0),
        &vgrad(iy, iy + ih * 0.45, &[(0.0, rgba(255, 255, 255, 110)), (1.0, rgba(255, 255, 255, 20))]),
        inner(),
    );
}

/// Giratório de 12 raios (espera). `phase` de 0 a 1 = uma volta; o raio mais forte
/// é o da vez e os seguintes vão sumindo.
pub fn spinner(c: &mut Canvas, cx: f32, cy: f32, r: f32, phase: f32, color: Color) {
    let head = (phase.fract() * 12.0).floor() as i32;
    let width = (r * 0.2).max(1.5);
    for i in 0..12 {
        let k = (head - i).rem_euclid(12) as f32;
        let alpha = 1.0 - k / 12.0 * 0.85;
        let a = i as f32 / 12.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (s, co) = a.sin_cos();
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx + co * r * 0.5, cy + s * r * 0.5);
        pb.line_to(cx + co * (r - width / 2.0), cy + s * (r - width / 2.0));
        c.stroke_round(pb.finish(), &solid(with_alpha(color, color.alpha() * alpha)), width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(c: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * c.px_width() + x) * 4) as usize;
        let d = c.data();
        (d[i], d[i + 1], d[i + 2])
    }

    #[test]
    fn progresso_pela_metade() {
        let mut c = Canvas::new(100.0, 12.0, 1.0).unwrap();
        c.clear(Color::WHITE);
        progress_bar(&mut c, 0.0, 0.0, 100.0, 12.0, Some(0.5), 0.0, rgba(45, 120, 230, 255));
        let (r, _, b) = px(&c, 25, 7);
        assert!(b > r + 40, "metade cheia deve ser azul: {r} {b}");
        let (r, g, b) = px(&c, 75, 7);
        assert!(r.abs_diff(b) < 10 && g.abs_diff(b) < 10, "metade vazia deve ser cinza: {r} {g} {b}");
    }

    #[test]
    fn listras_andam_com_a_fase() {
        let accent = rgba(45, 120, 230, 255);
        let mut a = Canvas::new(60.0, 12.0, 1.0).unwrap();
        let mut b = Canvas::new(60.0, 12.0, 1.0).unwrap();
        progress_bar(&mut a, 0.0, 0.0, 60.0, 12.0, None, 0.0, accent);
        progress_bar(&mut b, 0.0, 0.0, 60.0, 12.0, None, 0.5, accent);
        assert_ne!(a.data(), b.data());
    }
}
