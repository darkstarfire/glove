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
    // Área vazia (ex.: janela encolhendo numa animação): nada a desenhar.
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
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
    /// Onde se desenha: a tela toda ou, com recorte, só a área recortada.
    pub pm: Pixmap,
    pub scale: f32,
    /// Tamanho lógico.
    pub w: f32,
    pub h: f32,
    /// Com recorte (redesenho parcial): a imagem inteira guardada e a posição,
    /// em pixels, da área recortada que está em `pm`.
    full: Option<Pixmap>,
    off: (i32, i32),
}

/// a·b/255 arredondado, só com inteiros.
#[inline]
fn mul255(a: u32, b: u32) -> u32 {
    let x = a * b + 128;
    (x + (x >> 8)) >> 8
}

/// Compõe uma linha de pixels pré-multiplicados (`src` sobre `dst`).
#[inline]
fn blend_row(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        match s[3] {
            0 => {}
            255 => d.copy_from_slice(s),
            a => {
                let inv = 255 - a as u32;
                for k in 0..4 {
                    d[k] = (s[k] as u32 + mul255(d[k] as u32, inv)) as u8;
                }
            }
        }
    }
}

/// Cor como RGBA pré-multiplicado em bytes.
fn premul(c: Color) -> [u8; 4] {
    let c = c.premultiply().to_color_u8();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

/// Copia um retângulo de pixels entre pixmaps (`w`×`h` a partir das origens dadas).
fn copy_px(src: &Pixmap, sx: i32, sy: i32, dst: &mut Pixmap, dx: i32, dy: i32, w: i32, h: i32) {
    let (sw, dw) = (src.width() as usize, dst.width() as usize);
    let n = w as usize * 4;
    let s = src.data();
    let d = dst.data_mut();
    for y in 0..h as usize {
        let si = ((sy as usize + y) * sw + sx as usize) * 4;
        let di = ((dy as usize + y) * dw + dx as usize) * 4;
        d[di..di + n].copy_from_slice(&s[si..si + n]);
    }
}

type MetalKey = (i32, i32, i32, i32, i32, u32, u32, [(u32, u32, u32, u32); 2]);

thread_local! {
    /// Faixas de metal escovado já calculadas (ver `brushed_metal`).
    static METAL: std::cell::RefCell<Vec<(MetalKey, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Canvas {
    pub fn new(w: f32, h: f32, scale: f32) -> Option<Self> {
        let pw = (w * scale).ceil().max(1.0) as u32;
        let ph = (h * scale).ceil().max(1.0) as u32;
        Some(Self { pm: Pixmap::new(pw, ph)?, scale, w, h, full: None, off: (0, 0) })
    }

    /// Limita o desenho a um retângulo em pixels físicos (`None` = a tela toda).
    /// Enquanto houver recorte, desenha-se numa superfície do tamanho dele (com
    /// os pixels atuais) que volta para a imagem ao tirar o recorte: o resultado
    /// é idêntico ao de redesenhar tudo, gastando só com a área recortada.
    pub fn set_clip(&mut self, r: Option<[i32; 4]>) {
        if let Some(mut full) = self.full.take() {
            let (w, h) = (self.pm.width() as i32, self.pm.height() as i32);
            copy_px(&self.pm, 0, 0, &mut full, self.off.0, self.off.1, w, h);
            self.pm = full;
            self.off = (0, 0);
        }
        let Some([x0, y0, x1, y1]) = r else { return };
        let (pw, ph) = (self.pm.width() as i32, self.pm.height() as i32);
        let [x0, y0, x1, y1] = [x0.clamp(0, pw), y0.clamp(0, ph), x1.clamp(0, pw), y1.clamp(0, ph)];
        let Some(mut sub) = Pixmap::new((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32) else { return };
        if x1 > x0 && y1 > y0 {
            copy_px(&self.pm, x0, y0, &mut sub, 0, 0, x1 - x0, y1 - y0);
        }
        self.full = Some(std::mem::replace(&mut self.pm, sub));
        self.off = (x0, y0);
    }

    /// O recorte atual em pixels físicos, se houver.
    pub fn clip(&self) -> Option<[i32; 4]> {
        self.full.as_ref().map(|_| self.bounds())
    }

    /// Área em pixels (da imagem inteira) onde se pode desenhar.
    pub fn bounds(&self) -> [i32; 4] {
        let (ox, oy) = self.off;
        [ox, oy, ox + self.pm.width() as i32, oy + self.pm.height() as i32]
    }

    /// Posição, em pixels da imagem inteira, do pixel (0, 0) de `pm`.
    pub fn offset(&self) -> (i32, i32) {
        self.off
    }

    /// O retângulo lógico cai (ao menos em parte) dentro do recorte?
    pub fn visible(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if self.full.is_none() {
            return true;
        }
        let [x0, y0, x1, y1] = self.bounds();
        let s = self.scale;
        ((x * s).floor() as i32) < x1 && (((x + w) * s).ceil() as i32) > x0 && ((y * s).floor() as i32) < y1 && (((y + h) * s).ceil() as i32) > y0
    }

    fn path_visible(&self, p: &Path, pad: f32) -> bool {
        let b = p.bounds();
        self.visible(b.x() - pad, b.y() - pad, b.width() + pad * 2.0, b.height() + pad * 2.0)
    }

    /// Desenha um pixmap com uma transformação em pixels da imagem inteira.
    pub fn draw_pixmap_t(&mut self, img: tiny_skia::PixmapRef, paint: &PixmapPaint, t: Transform) {
        let t = t.post_translate(-self.off.0 as f32, -self.off.1 as f32);
        self.pm.draw_pixmap(0, 0, img, paint, t, None);
    }

    pub fn px_width(&self) -> u32 {
        self.full.as_ref().unwrap_or(&self.pm).width()
    }
    pub fn px_height(&self) -> u32 {
        self.full.as_ref().unwrap_or(&self.pm).height()
    }
    pub fn data(&self) -> &[u8] {
        self.full.as_ref().unwrap_or(&self.pm).data()
    }

    fn t(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale).post_translate(-self.off.0 as f32, -self.off.1 as f32)
    }

    /// Índice em `pm` do pixel (x, y) da imagem inteira (que precisa estar em `bounds`).
    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        (((y - self.off.1) * self.pm.width() as i32 + (x - self.off.0)) * 4) as usize
    }

    pub fn clear(&mut self, c: Color) {
        self.pm.fill(c);
    }

    pub fn fill(&mut self, path: Option<Path>, paint: &Paint) {
        self.fill_rule(path, paint, FillRule::Winding);
    }

    pub fn fill_rule(&mut self, path: Option<Path>, paint: &Paint, rule: FillRule) {
        if let Some(p) = path {
            if !self.path_visible(&p, 1.0) {
                return;
            }
            let t = self.t();
            self.pm.fill_path(&p, paint, rule, t, None);
        }
    }

    pub fn stroke(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            if !self.path_visible(&p, width + 1.0) {
                return;
            }
            let t = self.t();
            let s = Stroke { width, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    /// Retângulo de cor lisa alinhado aos pixels da tela, sem antisserrilhado: blocos
    /// vizinhos (células do terminal) se encostam sem fresta entre eles.
    pub fn fill_cell(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let s = self.scale;
        let (x0, y0) = ((x * s).round(), (y * s).round());
        let (x1, y1) = (((x + w) * s).round(), ((y + h) * s).round());
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (x, y, w, h) = (x0 / s, y0 / s, (x1 - x0) / s, (y1 - y0) / s);
        if !self.visible(x, y, w, h) {
            return;
        }
        let mut paint = solid(color);
        paint.anti_alias = false;
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let t = self.t();
            self.pm.fill_rect(r, &paint, t, None);
        }
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, paint: &Paint) {
        if !self.visible(x, y, w, h) {
            return;
        }
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
        let [bx0, by0, bx1, by1] = self.bounds();
        for yy in y.max(by0)..(y + h).min(by1) {
            for xx in x.max(bx0)..(x + w).min(bx1) {
                let i = self.idx(xx, yy);
                blend_px(self.pm.data_mut(), i, c.red(), c.green(), c.blue(), c.alpha());
            }
        }
    }

    fn px_range(&self, x: f32, y: f32, w: f32, h: f32) -> (i32, i32, i32, i32) {
        let s = self.scale;
        let [bx0, by0, bx1, by1] = self.bounds();
        let x0 = ((x * s).round() as i32).clamp(bx0, bx1);
        let y0 = ((y * s).round() as i32).clamp(by0, by1);
        let x1 = (((x + w) * s).round() as i32).clamp(bx0, bx1);
        let y1 = (((y + h) * s).round() as i32).clamp(by0, by1);
        (x0, y0, x1.max(x0), y1.max(y0))
    }

    /// Listras finas horizontais; `a` e `b` alternam a cada 2 linhas lógicas.
    pub fn pinstripes(&mut self, x: f32, y: f32, w: f32, h: f32, a: Color, b: Color) {
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        let s = self.scale;
        if x0 >= x1 {
            return;
        }
        // Linha pronta de cada cor: copiada (opaca) ou composta com inteiros.
        let n = ((x1 - x0) * 4) as usize;
        let rows = [a, b].map(|c| premul(c).repeat(n / 4));
        for yy in y0..y1 {
            let ly = ((yy as f32 + 0.5) / s - y).floor() as i32;
            let k = if (ly / 2) % 2 == 0 { 0 } else { 1 };
            let at = self.idx(x0, yy);
            let dst = &mut self.pm.data_mut()[at..at + n];
            match rows[k][3] {
                0 => {}
                255 => dst.copy_from_slice(&rows[k]),
                _ => blend_row(dst, &rows[k]),
            }
        }
    }

    /// Metal escovado: gradiente vertical + riscos horizontais aleatórios.
    /// O resultado de cada faixa fica guardado (é o mesmo a cada quadro): só a
    /// primeira vez faz a conta pixel a pixel.
    pub fn brushed_metal(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let s = self.scale;
        // O gradiente usa a altura pedida, não só a parte visível.
        let ys = (y * s).round() as i32;
        let span = (((y + h) * s).round() as i32 - ys).max(1) as f32;
        let opaque = top.alpha() >= 1.0 && bottom.alpha() >= 1.0;
        let key = (x0, y0, x1, y1, ys, span.to_bits(), s.to_bits(), [top, bottom].map(|c| (c.red().to_bits(), c.green().to_bits(), c.blue().to_bits(), c.alpha().to_bits())));
        if opaque {
            let hit = METAL.with(|m| {
                let m = m.borrow();
                let Some((_, px)) = m.iter().find(|(k, _)| *k == key) else { return false };
                let n = ((x1 - x0) * 4) as usize;
                for (k, yy) in (y0..y1).enumerate() {
                    let at = self.idx(x0, yy);
                    self.pm.data_mut()[at..at + n].copy_from_slice(&px[k * n..(k + 1) * n]);
                }
                true
            });
            if hit {
                return;
            }
        }
        for yy in y0..y1 {
            let base = mix(top, bottom, (yy - ys) as f32 / span);
            let row = hash(yy as u32 * 7919) as f32 / u32::MAX as f32;
            for xx in x0..x1 {
                // Riscos longos e fracos (o escovado do Tiger mal se vê).
                let seg = hash(((xx as f32 / (70.0 * s)) as u32) ^ (yy as u32).wrapping_mul(2_654_435_761));
                let n = (row - 0.5) * 0.028 + ((seg & 0xff) as f32 / 255.0 - 0.5) * 0.012;
                let c = |v: f32| (v + n).clamp(0.0, 1.0);
                let i = self.idx(xx, yy);
                blend_px(self.pm.data_mut(), i, c(base.red()), c(base.green()), c(base.blue()), base.alpha());
            }
        }
        if opaque {
            let n = ((x1 - x0) * 4) as usize;
            let mut px = Vec::with_capacity(n * (y1 - y0) as usize);
            for yy in y0..y1 {
                let at = self.idx(x0, yy);
                px.extend_from_slice(&self.pm.data()[at..at + n]);
            }
            METAL.with(|m| {
                let mut m = m.borrow_mut();
                // Poucas faixas por app (barra de cima, de baixo, margens).
                if m.len() >= 12 {
                    m.remove(0);
                }
                m.push((key, px));
            });
        }
    }

    /// Arredonda os cantos superiores apagando os pixels de fora.
    /// Brilho do metal escovado do Tiger: um pouco mais claro no meio da janela
    /// (`full_w` = largura dela, para as faixas separadas casarem).
    pub fn metal_sheen(&mut self, x: f32, y: f32, w: f32, h: f32, full_w: f32) {
        self.fill_rect(x, y, w, h, &linear(0.0, 0.0, full_w.max(1.0), 0.0, &[(0.0, rgba(255, 255, 255, 0)), (0.5, rgba(255, 255, 255, 34)), (1.0, rgba(255, 255, 255, 0))]));
    }

    /// Arredonda os cantos de baixo (janelas de metal do Tiger), apagando os pixels de fora.
    pub fn clip_bottom_corners(&mut self, r: f32) {
        let r = r * self.scale;
        let (w, h) = (self.px_width() as i32, self.px_height() as i32);
        let ri = r.ceil() as i32;
        let [bx0, by0, bx1, by1] = self.bounds();
        for dy in 0..ri {
            for dx in 0..ri {
                let (px, py) = (dx as f32 + 0.5, dy as f32 + 0.5);
                if px >= r || py >= r {
                    continue;
                }
                let d = ((r - px).powi(2) + (r - py).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0);
                if cov >= 1.0 {
                    continue;
                }
                let y = h - 1 - dy;
                for x in [dx, w - 1 - dx] {
                    if x < bx0 || x >= bx1 || y < by0 || y >= by1 {
                        continue;
                    }
                    let i = self.idx(x, y);
                    let data = self.pm.data_mut();
                    for k in 0..4 {
                        data[i + k] = (data[i + k] as f32 * cov) as u8;
                    }
                }
            }
        }
    }

    pub fn clip_top_corners(&mut self, r: f32) {
        let r = r * self.scale;
        let w = self.px_width() as i32;
        let ri = r.ceil() as i32;
        let [bx0, by0, bx1, by1] = self.bounds();
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
                    if x < bx0 || x >= bx1 || y < by0 || y >= by1 {
                        continue;
                    }
                    let i = self.idx(x, y);
                    let data = self.pm.data_mut();
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
        if !self.path_visible(&p, 1.0) {
            return;
        }
        let Some(mut mask) = tiny_skia::Mask::new(self.pm.width(), self.pm.height()) else { return };
        let t = self.t();
        mask.fill_path(&cl, FillRule::Winding, true, t);
        self.pm.fill_path(&p, paint, FillRule::Winding, t, Some(&mask));
    }

    /// Traço com pontas arredondadas.
    pub fn stroke_round(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            if !self.path_visible(&p, width + 1.0) {
                return;
            }
            let t = self.t();
            let s = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    /// Copia outro canvas (mesma escala) na posição lógica (x, y).
    pub fn draw_canvas(&mut self, other: &Canvas, x: f32, y: f32) {
        let s = self.scale;
        let (ox, oy) = ((x * s).round() as i32, (y * s).round() as i32);
        let (sw, sh) = (other.pm.width() as i32, other.pm.height() as i32);
        let [bx0, by0, bx1, by1] = self.bounds();
        let (x0, x1) = (ox.max(bx0), (ox + sw).min(bx1));
        let (y0, y1) = (oy.max(by0), (oy + sh).min(by1));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let n = (x1 - x0) as usize * 4;
        let src = other.pm.data();
        for yy in y0..y1 {
            let si = ((yy - oy) as usize * sw as usize + (x0 - ox) as usize) * 4;
            let di = self.idx(x0, yy);
            blend_row(&mut self.pm.data_mut()[di..di + n], &src[si..si + n]);
        }
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
    // Vidro polido da era Aqua, em camadas: sombrinha embaixo, borda escura fina,
    // corpo com a cor mais funda logo abaixo do reflexo e acesa embaixo (a luz que
    // atravessa o vidro), um brilho difuso no fundo e o reflexo forte em cima, com
    // a borda de baixo bem definida.
    let colored = (base.red() - base.blue()).abs() + (base.green() - base.blue()).abs() + (base.red() - base.green()).abs() > 0.15;
    c.fill(rrect(x, y + 1.0, w, h, r), &solid(rgba(0, 0, 0, if colored { 45 } else { 28 })));
    c.fill(rrect(x, y, w, h, r), &solid(darken(base, if colored { 0.22 } else { 0.4 })));
    let (ix, iy, iw, ih) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    let ir = (r - 1.0).max(0.0);
    let body = || rrect(ix, iy, iw, ih, ir);
    if colored {
        c.fill(body(), &vgrad(iy, iy + ih, &[(0.0, lighten(base, 0.42)), (0.48, lighten(base, 0.28)), (0.56, lighten(base, 0.34)), (1.0, lighten(base, 0.88))]));
        // Brilho que vem de dentro, no fundo do botão.
        if iw > 8.0 && ih > 6.0 {
            let (cx, cy) = (ix + iw / 2.0, iy + ih * 1.05);
            let k = iw / ih.max(1.0);
            let glow = radial(0.0, 0.0, ih * 0.8, &[(0.0, with_alpha(lighten(base, 0.9), 1.0)), (1.0, with_alpha(lighten(base, 0.6), 0.0))]);
            let mut g = glow;
            g.shader.transform(tiny_skia::Transform::from_row(k * 0.6, 0.0, 0.0, 1.0, cx, cy));
            c.fill(body(), &g);
        }
    } else {
        c.fill(body(), &vgrad(iy, iy + ih, &[(0.0, darken(base, 0.04)), (0.5, darken(base, 0.1)), (0.75, darken(base, 0.04)), (1.0, lighten(base, 0.85))]));
    }
    let hh = ih * 0.48;
    let rr = (ir - 1.0).max(0.0);
    // Muito estreito ou baixo para o reflexo de cima.
    if iw <= 4.0 || hh <= 0.0 {
        return;
    }
    let inset = if colored { 1.5 } else { 2.0 };
    c.fill(
        rrect4(ix + inset, iy + 0.5, iw - 2.0 * inset, hh, [rr, rr, (rr * 0.35).min(3.0), (rr * 0.35).min(3.0)]),
        &vgrad(iy, iy + hh, &[(0.0, rgba(255, 255, 255, if colored { 250 } else { 230 })), (0.6, rgba(255, 255, 255, if colored { 150 } else { 70 })), (1.0, rgba(255, 255, 255, if colored { 95 } else { 40 }))]),
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

    /// Cena de teste: fundo listrado, área recortada copiada e um botão de gel
    /// cuja cor depende de `t` (como o pulso do botão padrão).
    fn scene(c: &mut Canvas, t: f32) {
        let (a, b) = (rgba(236, 236, 236, 255), rgba(242, 242, 242, 255));
        c.clear(a);
        c.pinstripes(0.0, 0.0, c.w, c.h, a, b);
        let mut sc = Canvas::new(60.0, 30.0, c.scale).unwrap();
        sc.fill(rrect(2.0, 2.0, 56.0, 26.0, 6.0), &solid(rgba(30, 90, 200, 160)));
        c.draw_canvas(&sc, 5.0, 5.0);
        gel_rect(c, 40.0, 50.0, 70.0, 22.0, 11.0, mix(rgba(40, 110, 220, 255), rgba(150, 200, 250, 255), t));
        c.stroke(rrect(1.0, 1.0, c.w - 2.0, c.h - 2.0, 4.0), &solid(rgba(0, 0, 0, 90)), 1.0);
    }

    #[test]
    fn redesenho_parcial_igual_ao_inteiro() {
        for scale in [1.0, 1.5, 2.0] {
            let mut full = Canvas::new(120.0, 90.0, scale).unwrap();
            scene(&mut full, 0.8);
            let mut part = Canvas::new(120.0, 90.0, scale).unwrap();
            scene(&mut part, 0.1);
            // Só a área do botão (com folga) é refeita.
            let r = [((38.0 * scale) as i32), ((48.0 * scale) as i32), ((112.0 * scale).ceil() as i32), ((74.0 * scale).ceil() as i32)];
            part.set_clip(Some(r));
            scene(&mut part, 0.8);
            part.set_clip(None);
            let pw = full.px_width() as usize;
            // Os degradês podem arredondar 1 nível diferente com a origem deslocada
            // (invisível); mais que isso é erro.
            let far = |a: &[u8], b: &[u8]| a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > 1);
            let bad: Vec<_> = full.data().chunks(4).zip(part.data().chunks(4)).enumerate().filter(|(_, (a, b))| far(a, b)).map(|(i, (a, b))| (i % pw, i / pw, a.to_vec(), b.to_vec())).take(5).collect();
            assert!(bad.is_empty(), "escala {scale}: parcial difere do inteiro: {bad:?}");
        }
    }

    #[test]
    fn recorte_nao_toca_fora() {
        let mut c = Canvas::new(40.0, 40.0, 1.0).unwrap();
        c.clear(rgba(255, 0, 0, 255));
        let before = c.data().to_vec();
        c.set_clip(Some([10, 10, 20, 20]));
        c.clear(rgba(0, 0, 255, 255));
        c.pinstripes(0.0, 0.0, 40.0, 40.0, rgba(0, 255, 0, 255), rgba(0, 0, 0, 255));
        c.fill(rrect(0.0, 0.0, 40.0, 40.0, 5.0), &solid(rgba(0, 0, 0, 128)));
        c.set_clip(None);
        for y in 0..40 {
            for x in 0..40 {
                let i = (y * 40 + x) * 4;
                let inside = (10..20).contains(&x) && (10..20).contains(&y);
                assert_eq!(c.data()[i..i + 4] == before[i..i + 4], !inside, "pixel ({x}, {y})");
            }
        }
    }
}
