//! Painel de Fontes e Painel de Cores (como os da era): painéis flutuantes no
//! canto da janela, que mudam a escolha na hora.

use glove_paint::{
    draw::*,
    tiny_skia::{IntSize, Pixmap, PixmapPaint, Transform},
    Color, Fonts,
};

use crate::ui::{Rect, Ui};

/// Moldura do painel: faixa de título fina com o botão de fechar. Devolve o corpo
/// e se o fechar foi clicado.
fn panel_frame(ui: &mut Ui, r: Rect, title: &str) -> (Rect, bool) {
    ui.c.fill(rrect(r.x + 2.0, r.y + 4.0, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, 60)));
    ui.c.fill(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(rgba(236, 236, 238, 250)));
    ui.c.fill(rrect4(r.x, r.y, r.w, 18.0, [5.0, 5.0, 0.0, 0.0]), &vgrad(r.y, r.y + 18.0, &[(0.0, rgba(246, 246, 248, 255)), (1.0, rgba(210, 210, 214, 255))]));
    ui.c.hline(r.x, r.y + 18.0, r.w, rgba(150, 150, 156, 255));
    ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 5.0), &solid(rgba(0, 0, 0, 90)), 1.0);
    let tw = ui.text_width(title, 11.0, false);
    ui.fonts.draw(ui.c, title, r.x + (r.w - tw) / 2.0, r.y + 13.0, 11.0, false, rgba(30, 30, 34, 255));
    // Botão de fechar pequeno (painéis usam o menor).
    let (cx, cy) = (r.x + 11.0, r.y + 9.0);
    gel_circle(ui.c, cx, cy, 5.0, rgba(230, 70, 60, 255));
    let close = ui.hit(Rect::new(cx - 7.0, cy - 7.0, 14.0, 14.0)).clicked;
    (Rect::new(r.x + 8.0, r.y + 24.0, r.w - 16.0, r.h - 30.0), close)
}

// ---------------------------------------------------------------------------
// Fontes
// ---------------------------------------------------------------------------

pub struct FontPanel {
    pub open: bool,
    pub family: String,
    pub size: f32,
    families: Option<Vec<String>>,
    filter: String,
    scroll: f32,
    preview: Option<(String, Fonts)>,
}

const SIZES: [f32; 12] = [9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 24.0, 36.0, 48.0, 64.0];

/// Famílias instaladas (fc-list), sem repetir e em ordem.
pub fn installed_families() -> Vec<String> {
    let out = std::process::Command::new("fc-list").args([":", "family"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut v: Vec<String> = out.lines().map(|l| l.split(',').next().unwrap_or("").trim().to_string()).filter(|f| !f.is_empty()).collect();
    v.sort_by_key(|f| f.to_lowercase());
    v.dedup();
    v
}

impl FontPanel {
    pub fn new(family: &str, size: f32) -> Self {
        Self { open: false, family: family.to_string(), size, families: None, filter: String::new(), scroll: 0.0, preview: None }
    }
}

impl Ui<'_> {
    /// Painel de Fontes no canto de cima à direita. Devolve verdadeiro quando a
    /// família ou o tamanho mudam (o app aplica na hora).
    pub fn font_panel(&mut self, p: &mut FontPanel) -> bool {
        if !p.open {
            return false;
        }
        let families = p.families.get_or_insert_with(installed_families).clone();
        let (pw, ph) = (320.0, 300.0);
        let r = Rect::new((self.c.w - pw - 10.0).max(0.0), 30.0, pw, ph);
        let (body, close) = panel_frame(self, r, "Fontes");
        if close {
            p.open = false;
            return false;
        }
        let mut changed = false;
        // Busca.
        self.search_field_in(Rect::new(body.x, body.y, body.w, crate::ui::SEARCH_H), &mut p.filter, "Buscar família");
        let f = p.filter.trim().to_lowercase();
        let list: Vec<&String> = families.iter().filter(|x| f.is_empty() || x.to_lowercase().contains(&f)).collect();
        // Famílias à esquerda, tamanhos à direita.
        let ly = body.y + crate::ui::SEARCH_H + 6.0;
        let lr = Rect::new(body.x, ly, body.w - 66.0, 150.0);
        self.c.fill_rect(lr.x, lr.y, lr.w, lr.h, &solid(Color::WHITE));
        self.c.stroke(rrect(lr.x + 0.5, lr.y + 0.5, lr.w - 1.0, lr.h - 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        let row = 17.0;
        p.scroll = (p.scroll - self.scroll_in(lr)).clamp(0.0, (list.len() as f32 * row - lr.h).max(0.0));
        let first = (p.scroll / row) as usize;
        for (i, fam) in list.iter().enumerate().skip(first).take((lr.h / row) as usize + 1) {
            let y = lr.y + i as f32 * row - p.scroll;
            if y + row > lr.y + lr.h + 1.0 {
                break;
            }
            let rr = Rect::new(lr.x + 1.0, y, lr.w - 2.0, row);
            let on = **fam == p.family;
            if on {
                self.c.fill_rect(rr.x, rr.y.max(lr.y), rr.w, row, &solid(self.theme.accent));
            }
            let t = self.fonts.ellipsize(fam, 11.0, false, rr.w - 8.0);
            self.fonts.draw(self.c, &t, rr.x + 4.0, y + 12.5, 11.0, false, if on { Color::WHITE } else { rgba(20, 20, 24, 255) });
            if self.hit(rr).clicked && !on {
                p.family = fam.to_string();
                changed = true;
            }
        }
        let sr = Rect::new(body.x + body.w - 60.0, ly, 60.0, 150.0);
        self.c.fill_rect(sr.x, sr.y, sr.w, sr.h, &solid(Color::WHITE));
        self.c.stroke(rrect(sr.x + 0.5, sr.y + 0.5, sr.w - 1.0, sr.h - 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        for (i, s) in SIZES.iter().enumerate() {
            let y = sr.y + 1.0 + i as f32 * 12.3;
            let rr = Rect::new(sr.x + 1.0, y, sr.w - 2.0, 12.3);
            let on = (*s - p.size).abs() < 0.1;
            if on {
                self.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(self.theme.accent));
            }
            let t = format!("{}", *s as u32);
            self.fonts.draw(self.c, &t, rr.x + 6.0, y + 10.0, 10.0, false, if on { Color::WHITE } else { rgba(20, 20, 24, 255) });
            if self.hit(rr).clicked && !on {
                p.size = *s;
                changed = true;
            }
        }
        // Prévia na própria fonte (carregada uma vez por família).
        let pr = Rect::new(body.x, ly + 158.0, body.w, body.y + body.h - ly - 158.0);
        self.c.fill(rrect(pr.x, pr.y, pr.w, pr.h, 3.0), &solid(Color::WHITE));
        self.c.stroke(rrect(pr.x + 0.5, pr.y + 0.5, pr.w - 1.0, pr.h - 1.0, 3.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        if p.preview.as_ref().is_none_or(|(f, _)| *f != p.family) {
            p.preview = Fonts::load_family(&p.family).map(|f| (p.family.clone(), f));
        }
        if let Some((_, fonts)) = &p.preview {
            let px = p.size.min(pr.h - 14.0).max(8.0);
            let sample = "Aa Bb Çç Áé 123";
            fonts.draw(self.c, sample, pr.x + 8.0, pr.y + pr.h / 2.0 + px * 0.35, px, false, rgba(20, 20, 24, 255));
        }
        changed
    }
}

// ---------------------------------------------------------------------------
// Cores
// ---------------------------------------------------------------------------

pub struct ColorPanel {
    pub open: bool,
    pub rgb: (u8, u8, u8),
    hsv: (f32, f32, f32),
    wheel: Option<(f32, Pixmap)>,
    dragging: bool,
}

/// HSV (h 0..360, s e v 0..1) → RGB.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let f = |u: f32| ((u + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

pub fn rgb_to_hsv((r, g, b): (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// Lápis de cor: as amostras de sempre.
const SWATCHES: [(u8, u8, u8); 16] = [
    (0, 0, 0), (90, 90, 90), (160, 160, 160), (255, 255, 255),
    (200, 40, 40), (240, 140, 30), (250, 220, 60), (120, 190, 60),
    (40, 150, 80), (60, 180, 200), (40, 100, 220), (110, 70, 200),
    (200, 80, 170), (255, 205, 225), (255, 244, 160), (190, 222, 255),
];

impl ColorPanel {
    pub fn new(rgb: (u8, u8, u8)) -> Self {
        Self { open: false, rgb, hsv: rgb_to_hsv(rgb), wheel: None, dragging: false }
    }

    pub fn set(&mut self, rgb: (u8, u8, u8)) {
        self.rgb = rgb;
        self.hsv = rgb_to_hsv(rgb);
    }
}

/// Roda de cores (matiz em volta, saturação para fora) em pixels.
fn wheel_pixmap(d: u32) -> Option<Pixmap> {
    let mut data = vec![0u8; (d * d * 4) as usize];
    let r = d as f32 / 2.0;
    for y in 0..d {
        for x in 0..d {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > r {
                continue;
            }
            let h = dy.atan2(dx).to_degrees();
            let (cr, cg, cb) = hsv_to_rgb(h, (dist / r).min(1.0), 1.0);
            // Borda suave (alfa pré-multiplicado).
            let a = ((r - dist).clamp(0.0, 1.0) * 255.0) as u32;
            let i = ((y * d + x) * 4) as usize;
            data[i] = (cr as u32 * a / 255) as u8;
            data[i + 1] = (cg as u32 * a / 255) as u8;
            data[i + 2] = (cb as u32 * a / 255) as u8;
            data[i + 3] = a as u8;
        }
    }
    Pixmap::from_vec(data, IntSize::from_wh(d, d)?)
}

impl Ui<'_> {
    /// Painel de Cores no canto de cima à direita. Devolve verdadeiro quando a cor muda.
    pub fn color_panel(&mut self, p: &mut ColorPanel) -> bool {
        if !p.open {
            return false;
        }
        let (pw, ph) = (250.0, 286.0);
        let r = Rect::new((self.c.w - pw - 10.0).max(0.0), 30.0, pw, ph);
        let (body, close) = panel_frame(self, r, "Cores");
        if close {
            p.open = false;
            return false;
        }
        let mut changed = false;
        // Poço com a cor atual.
        let (cr, cg, cb) = p.rgb;
        self.c.fill(rrect(body.x, body.y, body.w, 22.0, 3.0), &solid(rgba(cr, cg, cb, 255)));
        self.c.stroke(rrect(body.x + 0.5, body.y + 0.5, body.w - 1.0, 21.0, 3.0), &solid(rgba(0, 0, 0, 120)), 1.0);
        // Roda.
        let d = 140.0;
        let (wx, wy) = (body.x + (body.w - d) / 2.0, body.y + 30.0);
        let s = self.c.scale;
        let px = (d * s).round() as u32;
        if p.wheel.as_ref().is_none_or(|(sc, _)| *sc != s) {
            p.wheel = wheel_pixmap(px).map(|pm| (s, pm));
        }
        if let Some((_, pm)) = &p.wheel {
            self.c.draw_pixmap_t(pm.as_ref(), &PixmapPaint::default(), Transform::from_translate(wx * s, wy * s));
        }
        // Escurece a roda pelo brilho.
        let v = p.hsv.2;
        if v < 1.0 {
            self.c.fill(circle(wx + d / 2.0, wy + d / 2.0, d / 2.0), &solid(rgba(0, 0, 0, ((1.0 - v) * 255.0) as u8)));
        }
        // Marca do ponto escolhido.
        let ang = p.hsv.0.to_radians();
        let (mx, my) = (wx + d / 2.0 + ang.cos() * p.hsv.1 * d / 2.0, wy + d / 2.0 + ang.sin() * p.hsv.1 * d / 2.0);
        self.c.stroke(circle(mx, my, 4.0), &solid(Color::WHITE), 2.0);
        self.c.stroke(circle(mx, my, 5.0), &solid(rgba(0, 0, 0, 160)), 1.0);
        let wr = Rect::new(wx, wy, d, d);
        let h = self.hit(wr);
        if h.pressed {
            p.dragging = true;
        }
        if !self.mouse_down() {
            p.dragging = false;
        }
        if p.dragging {
            let (x, y) = self.pointer();
            let (dx, dy) = (x - (wx + d / 2.0), y - (wy + d / 2.0));
            p.hsv.0 = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            p.hsv.1 = ((dx * dx + dy * dy).sqrt() / (d / 2.0)).min(1.0);
            p.rgb = hsv_to_rgb(p.hsv.0, p.hsv.1, p.hsv.2);
            changed = true;
        }
        // Brilho.
        let mut bright = p.hsv.2;
        self.area(Rect::new(body.x + 10.0, wy + d + 8.0, body.w - 20.0, 24.0), |ui| {
            if ui.slider(&mut bright, 0.0, 1.0, body.w - 20.0) {
                p.hsv.2 = bright;
                p.rgb = hsv_to_rgb(p.hsv.0, p.hsv.1, p.hsv.2);
                changed = true;
            }
        });
        // Amostras.
        let sy = wy + d + 38.0;
        let sw = (body.w - 7.0 * 4.0) / 8.0;
        for (i, (r0, g0, b0)) in SWATCHES.iter().enumerate() {
            let (col, row) = ((i % 8) as f32, (i / 8) as f32);
            let rr = Rect::new(body.x + col * (sw + 4.0), sy + row * 18.0, sw, 14.0);
            self.c.fill(rrect(rr.x, rr.y, rr.w, rr.h, 2.0), &solid(rgba(*r0, *g0, *b0, 255)));
            self.c.stroke(rrect(rr.x + 0.5, rr.y + 0.5, rr.w - 1.0, rr.h - 1.0, 2.0), &solid(rgba(0, 0, 0, 90)), 1.0);
            if self.hit(rr).clicked {
                p.set((*r0, *g0, *b0));
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hsv_ida_e_volta() {
        for rgb in [(255, 0, 0), (40, 100, 220), (255, 244, 160), (90, 90, 90)] {
            let (h, s, v) = super::rgb_to_hsv(rgb);
            let back = super::hsv_to_rgb(h, s, v);
            assert!((back.0 as i32 - rgb.0 as i32).abs() <= 1 && (back.1 as i32 - rgb.1 as i32).abs() <= 1 && (back.2 as i32 - rgb.2 as i32).abs() <= 1, "{rgb:?} → {back:?}");
        }
    }
}
