//! Texto com ab_glyph: a fonte é lida sob demanda (só os caracteres usados
//! viram bitmap), então carregar uma fonte custa só o tamanho do arquivo.

use std::cell::RefCell;
use std::collections::HashMap;

use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, point};
use tiny_skia::Color;

use crate::draw::Canvas;

const REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
];
const BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
];

fn load(paths: &[&str]) -> Option<FontVec> {
    paths.iter().find_map(|p| FontVec::try_from_vec(std::fs::read(p).ok()?).ok())
}

/// Bitmap de um caractere já rasterizado.
struct Glyph {
    left: i32,
    top: i32,
    w: usize,
    h: usize,
    cov: Vec<u8>,
}

type Key = (char, u32, bool);

pub struct Fonts {
    regular: FontVec,
    bold: Option<FontVec>,
    cache: RefCell<HashMap<Key, Glyph>>,
    /// Fontes reserva (ícones Nerd Font, símbolos), lidas só na primeira vez
    /// que aparece um caractere que a fonte principal não tem.
    fallbacks: std::cell::OnceCell<Vec<(String, std::cell::OnceCell<Option<FontVec>>)>>,
}

fn new_fonts(regular: FontVec, bold: Option<FontVec>) -> Fonts {
    Fonts { regular, bold, cache: RefCell::new(HashMap::new()), fallbacks: std::cell::OnceCell::new() }
}

/// Arquivo de uma família de fonte instalada (via fontconfig).
fn fc_match(pattern: &str) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", pattern]).output().ok()?;
    let p = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    p.is_file().then_some(p)
}

/// Fontes reserva (só os caminhos; cada uma é lida quando for necessária),
/// da mais específica para a mais genérica.
fn fallback_fonts() -> Vec<(String, std::cell::OnceCell<Option<FontVec>>)> {
    let mut files: Vec<String> = Vec::new();
    if let Ok(out) = std::process::Command::new("fc-list").args([":", "file", "family"]).output() {
        let list = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut nerd: Vec<(u8, String)> = list
            .lines()
            .filter(|l| l.contains("Nerd Font"))
            .filter_map(|l| {
                let file = l.split(':').next()?.trim().to_string();
                let rank = if l.contains("Symbols Nerd Font") { 0 } else if l.contains("Mono") && file.contains("Regular") { 1 } else if file.contains("Regular") { 2 } else { 3 };
                Some((rank, file))
            })
            .collect();
        nerd.sort();
        files.extend(nerd.into_iter().map(|(_, f)| f).take(1));
    }
    for name in ["Noto Sans Symbols2", "Noto Sans Symbols", "DejaVu Sans", "Noto Sans CJK SC"] {
        if let Some(p) = fc_match(name) {
            let p = p.to_string_lossy().into_owned();
            if !files.contains(&p) {
                files.push(p);
            }
        }
    }
    files.into_iter().map(|f| (f, std::cell::OnceCell::new())).collect()
}

const DOC_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
];
const DOC_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
    "/usr/share/fonts/TTF/LiberationSans-Bold.ttf",
];

const MONO_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
];
const MONO_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-B.ttf",
];

impl Fonts {
    /// Fonte de uma família instalada (ex.: "JetBrainsMono Nerd Font"); vazio = monoespaçada padrão.
    pub fn load_family(family: &str) -> Option<Self> {
        if family.trim().is_empty() {
            return Self::load_mono();
        }
        let reg = fc_match(family).and_then(|p| FontVec::try_from_vec(std::fs::read(p).ok()?).ok());
        match reg {
            Some(r) => {
                let bold = fc_match(&format!("{family}:bold")).and_then(|p| FontVec::try_from_vec(std::fs::read(p).ok()?).ok());
                Some(new_fonts(r, bold))
            }
            None => Self::load_mono(),
        }
    }

    /// Fonte de largura fixa (terminal).
    pub fn load_mono() -> Option<Self> {
        Some(new_fonts(load(MONO_REGULAR)?, load(MONO_BOLD)))
    }

    /// Métricas da fonte em `px`: (avanço de um caractere, subida acima da linha de base).
    pub fn mono_metrics(&self, px: f32) -> (f32, f32) {
        let f = &self.regular;
        let sf = f.as_scaled(self.scale(f, px));
        (sf.h_advance(f.glyph_id('M')), sf.ascent())
    }

    pub fn load_system() -> Option<Self> {
        Some(new_fonts(load(REGULAR)?, load(BOLD)))
    }

    /// Fonte para o texto dos documentos (sem serifa, mais estreita que a da interface).
    pub fn load_document() -> Option<Self> {
        match load(DOC_REGULAR) {
            Some(regular) => Some(new_fonts(regular, load(DOC_BOLD))),
            None => Self::load_system(),
        }
    }

    /// Fonte que tem o caractere: a principal ou a primeira reserva que tiver.
    fn font_for(&self, ch: char, bold: bool) -> &FontVec {
        let f = self.font(bold);
        if ch <= ' ' || f.glyph_id(ch).0 != 0 {
            return f;
        }
        for (path, cell) in self.fallbacks.get_or_init(fallback_fonts) {
            let font = cell.get_or_init(|| FontVec::try_from_vec(std::fs::read(path).ok()?).ok());
            if let Some(fb) = font {
                if fb.glyph_id(ch).0 != 0 {
                    return fb;
                }
            }
        }
        f
    }

    fn font(&self, bold: bool) -> &FontVec {
        if bold { self.bold.as_ref().unwrap_or(&self.regular) } else { &self.regular }
    }

    /// Escala em que 1 em = `px` pixels (o mesmo que "tamanho da fonte").
    fn scale(&self, f: &FontVec, px: f32) -> PxScale {
        // PxScale do ab_glyph é a altura da linha; convertemos a partir do "em".
        match f.units_per_em() {
            Some(upem) if upem > 0.0 => PxScale::from(px * f.height_unscaled() / upem),
            _ => PxScale::from(px),
        }
    }

    pub fn measure(&self, text: &str, px: f32, bold: bool) -> f32 {
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, px));
        let mut w = 0.0;
        let mut prev: Option<GlyphId> = None;
        for ch in text.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, px, bold) } else { sf.h_advance(id) };
            prev = Some(id);
        }
        w
    }

    /// Corta o texto com "…" para caber em `max_w`.
    pub fn ellipsize(&self, text: &str, px: f32, bold: bool, max_w: f32) -> String {
        if self.measure(text, px, bold) <= max_w {
            return text.to_string();
        }
        let mut s: String = text.to_string();
        while !s.is_empty() {
            s.pop();
            let t = format!("{s}…");
            if self.measure(&t, px, bold) <= max_w {
                return t;
            }
        }
        String::new()
    }

    fn rasterize(&self, ch: char, ppx: f32, bold: bool) -> Glyph {
        let f = self.font_for(ch, bold);
        let g = f.glyph_id(ch).with_scale_and_position(self.scale(f, ppx), point(0.0, 0.0));
        let Some(og) = f.outline_glyph(g) else { return Glyph { left: 0, top: 0, w: 0, h: 0, cov: Vec::new() } };
        let b = og.px_bounds();
        let (w, h) = (b.width() as usize, b.height() as usize);
        let mut cov = vec![0u8; w * h];
        og.draw(|x, y, c| {
            let i = y as usize * w + x as usize;
            if i < cov.len() {
                cov[i] = (c.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        Glyph { left: b.min.x as i32, top: b.min.y as i32, w, h, cov }
    }

    /// Desenha `text` (coordenadas lógicas) com a linha de base em `baseline`.
    pub fn draw(&self, c: &mut Canvas, text: &str, x: f32, baseline: f32, px: f32, bold: bool, color: Color) {
        let s = c.scale;
        let ppx = px * s;
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, ppx));
        let mut pen = x * s;
        let base = (baseline * s).round() as i32;
        let mut prev: Option<GlyphId> = None;
        let pw = c.pm.width() as i32;
        let [bx0, by0, bx1, by1] = c.bounds();
        let (ox, oy) = c.offset();
        let mut cache = self.cache.borrow_mut();
        let data = c.pm.data_mut();
        let (cr, cg, cb, ca) = (color.red(), color.green(), color.blue(), color.alpha());
        for ch in text.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                pen += sf.kern(p, id);
            }
            prev = Some(id);
            let g = cache.entry((ch, ppx.to_bits(), bold)).or_insert_with(|| self.rasterize(ch, ppx, bold));
            let gx = pen.round() as i32 + g.left;
            let gy = base + g.top;
            for row in 0..g.h as i32 {
                let y = gy + row;
                if y < by0 || y >= by1 {
                    continue;
                }
                for col in 0..g.w as i32 {
                    let x = gx + col;
                    if x < bx0 || x >= bx1 {
                        continue;
                    }
                    let cov = g.cov[row as usize * g.w + col as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = cov as f32 / 255.0 * ca;
                    let i = (((y - oy) * pw + (x - ox)) * 4) as usize;
                    let inv = 1.0 - a;
                    data[i] = (cr * a * 255.0 + data[i] as f32 * inv) as u8;
                    data[i + 1] = (cg * a * 255.0 + data[i + 1] as f32 * inv) as u8;
                    data[i + 2] = (cb * a * 255.0 + data[i + 2] as f32 * inv) as u8;
                    data[i + 3] = (a * 255.0 + data[i + 3] as f32 * inv) as u8;
                }
            }
            pen += if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, ppx, bold) } else { sf.h_advance(id) };
        }
    }

    fn fallback_advance(&self, ch: char, px: f32, bold: bool) -> f32 {
        let f = self.font_for(ch, bold);
        f.as_scaled(self.scale(f, px)).h_advance(f.glyph_id(ch))
    }

    /// Texto com um "relevo" claro 1px abaixo (efeito gravado).
    pub fn draw_embossed(&self, c: &mut Canvas, text: &str, x: f32, baseline: f32, px: f32, bold: bool, color: Color, light: Color) {
        self.draw(c, text, x, baseline + 1.0, px, bold, light);
        self.draw(c, text, x, baseline, px, bold, color);
    }
}
