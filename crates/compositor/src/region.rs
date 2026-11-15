//! Captura de uma área: a tela escurece, o mouse arrasta um retângulo e só ele
//! vai para o PNG (como o Shift+Cmd+4 da era), em qualquer monitor. No modo janela,
//! a janela sob o mouse fica destacada e o clique a fotografa. Esc cancela.

use smithay::{
    backend::renderer::element::{solid::{SolidColorBuffer, SolidColorRenderElement}, Kind},
    utils::{Logical, Point, Rectangle},
};

use crate::{render::GloveElement, state::Glove};

/// A tela congelada no começo da seleção: a imagem (para recortar) e a textura
/// (mostrada por baixo do retângulo, para escolher o momento certo).
pub struct Frozen {
    pub pix: glove_paint::Pixmap,
    pub tex: crate::shell::Tex,
    pub scale: f64,
}

pub struct RegionSel {
    pub start: Option<Point<f64, Logical>>,
    pub cur: Point<f64, Logical>,
    pub open: bool,
    /// Escolhendo uma janela (não um retângulo).
    pub window: bool,
}

impl RegionSel {
    /// Retângulo escolhido (lógico).
    pub fn rect(&self) -> Option<Rectangle<i32, Logical>> {
        let s = self.start?;
        let (x0, y0) = (s.x.min(self.cur.x), s.y.min(self.cur.y));
        let (x1, y1) = (s.x.max(self.cur.x), s.y.max(self.cur.y));
        Some(Rectangle::new((x0.round() as i32, y0.round() as i32).into(), (((x1 - x0).round() as i32).max(1), ((y1 - y0).round() as i32).max(1)).into()))
    }
}

impl Glove {
    /// Começa a escolher a área (pedido do app Captura ou do atalho).
    pub fn start_region_shot(&mut self, open: bool, window: bool) {
        if self.locked() {
            return;
        }
        self.close_menu();
        // O próximo quadro fotografa a tela como está (sem a película) e só então a
        // seleção começa, sobre a imagem parada.
        self.region_pending = Some((open, window));
        self.dirty = true;
    }

    /// Guarda a tela congelada e começa a seleção (chamado na hora de desenhar).
    pub fn begin_region(&mut self, frozen: Option<Frozen>) {
        let Some((open, window)) = self.region_pending.take() else { return };
        self.freeze = frozen;
        self.region = Some(RegionSel { start: None, cur: self.pointer_pos, open, window });
        self.dirty = true;
    }

    /// Recorta a área da tela congelada e salva (como a captura normal). Devolve
    /// false se não dá (área fora do monitor congelado): aí vale a captura ao vivo.
    fn shot_from_freeze(&mut self, rect: smithay::utils::Rectangle<i32, smithay::utils::Logical>, open: bool) -> bool {
        let Some(f) = self.freeze.take() else { return false };
        let (lw, lh) = self.output_size();
        if !smithay::utils::Rectangle::from_size((lw, lh).into()).contains_rect(rect) {
            return false;
        }
        let sc = f.scale;
        let (w, h) = (f.pix.width() as i32, f.pix.height() as i32);
        let x0 = ((rect.loc.x as f64 * sc).round() as i32).clamp(0, w - 1);
        let y0 = ((rect.loc.y as f64 * sc).round() as i32).clamp(0, h - 1);
        let x1 = (((rect.loc.x + rect.size.w) as f64 * sc).round() as i32).clamp(x0 + 1, w);
        let y1 = (((rect.loc.y + rect.size.h) as f64 * sc).round() as i32).clamp(y0 + 1, h);
        let Some(mut out) = glove_paint::Pixmap::new((x1 - x0) as u32, (y1 - y0) as u32) else { return false };
        let row = (x1 - x0) as usize * 4;
        let src = f.pix.data();
        let stride = w as usize * 4;
        for (i, y) in (y0..y1).enumerate() {
            let a = y as usize * stride + x0 as usize * 4;
            out.data_mut()[i * row..(i + 1) * row].copy_from_slice(&src[a..a + row]);
        }
        let path = self.screenshot_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match out.save_png(&path) {
            Ok(()) => {
                tracing::info!("captura salva em {}", path.display());
                self.copy_file_to_clipboard(&path);
                if open {
                    self.spawn(&format!("glove-view --desenhar '{}'", path.display().to_string().replace('\'', "'\\''")));
                }
            }
            Err(e) => tracing::warn!("captura falhou: {e}"),
        }
        true
    }

    /// Janela (com a barra de título) sob o ponto, para a captura de janela.
    fn window_frame_at(&self, pos: Point<f64, Logical>) -> Option<(smithay::desktop::Window, Rectangle<i32, Logical>)> {
        let w = self.space.element_under(pos).map(|(w, _)| w.clone()).or_else(|| {
            // Na barra de título (desenhada pelo Glove, fora da janela do app).
            self.space.elements().rev().find(|w| {
                self.space.element_geometry(w).is_some_and(|g| {
                    let top = crate::shell::deco_top(w);
                    Rectangle::new((g.loc.x, g.loc.y - top).into(), (g.size.w, g.size.h + top).into()).to_f64().contains(pos)
                })
            }).cloned()
        })?;
        let g = self.space.element_geometry(&w)?;
        let top = crate::shell::deco_top(&w);
        Some((w, Rectangle::new((g.loc.x, g.loc.y - top).into(), (g.size.w, g.size.h + top).into())))
    }

    pub fn region_motion(&mut self, pos: Point<f64, Logical>) {
        if let Some(r) = &mut self.region {
            r.cur = pos;
            self.dirty = true;
        }
    }

    /// Botão: aperta começa, solta captura (área muito pequena cancela).
    pub fn region_button(&mut self, pressed: bool) {
        let Some(r) = &mut self.region else { return };
        if r.window {
            if !pressed {
                return;
            }
            let (pos, open) = (r.cur, r.open);
            self.region = None;
            self.dirty = true;
            if let Some((w, rect)) = self.window_frame_at(pos) {
                // Da tela congelada, se der; senão, ao vivo, com a janela na frente de tudo.
                if !self.shot_from_freeze(rect, open) {
                    self.space.raise_element(&w, true);
                    self.screenshot_crop = Some(rect);
                    self.open_after_shot = open;
                    self.request_screenshot();
                }
            }
            self.freeze = None;
            return;
        }
        if pressed {
            r.start = Some(r.cur);
            self.dirty = true;
            return;
        }
        let rect = r.rect();
        let open = r.open;
        self.region = None;
        self.dirty = true;
        match rect.filter(|r| r.size.w >= 4 && r.size.h >= 4) {
            Some(rect) => {
                if !self.shot_from_freeze(rect, open) {
                    self.screenshot_crop = Some(rect);
                    self.open_after_shot = open;
                    self.request_screenshot();
                }
            }
            None => tracing::info!("captura de área cancelada"),
        }
        self.freeze = None;
    }

    pub fn cancel_region(&mut self) {
        self.freeze = None;
        self.region_pending = None;
        if self.region.take().is_some() {
            self.dirty = true;
        }
    }

    /// Película escura em volta da área e o contorno dela (sobre tudo), no monitor
    /// que começa em `origin` com o tamanho `size` (lógicos).
    pub fn region_elements_at(&self, sc: f64, origin: Point<i32, Logical>, size: (i32, i32), out: &mut Vec<GloveElement>) {
        let Some(r) = &self.region else { return };
        let (w, h) = size;
        let (ox, oy) = (origin.x, origin.y);
        // Modo janela: só a janela sob o mouse fica azulada.
        if r.window {
            if let Some((_, f)) = self.window_frame_at(r.cur) {
                let b = SolidColorBuffer::new((f.size.w, f.size.h), [0.07, 0.14, 0.28, 0.28]);
                out.push(GloveElement::Solid(SolidColorRenderElement::from_buffer(&b, Point::<i32, Logical>::from((f.loc.x - ox, f.loc.y - oy)).to_physical_precise_round(sc), sc, 1.0, Kind::Unspecified)));
            }
            return;
        }
        let sel = r.rect().unwrap_or_else(|| Rectangle::new((r.cur.x as i32, r.cur.y as i32).into(), (0, 0).into()));
        // Coordenadas deste monitor (o retângulo pode estar em outro: fica tudo escuro).
        let clampx = |v: i32| (v - ox).clamp(0, w);
        let clampy = |v: i32| (v - oy).clamp(0, h);
        let (x0, y0, x1, y1) = (clampx(sel.loc.x), clampy(sel.loc.y), clampx(sel.loc.x + sel.size.w), clampy(sel.loc.y + sel.size.h));
        let dim = [0.0, 0.0, 0.0, 0.35];
        let line = [1.0, 1.0, 1.0, 0.9];
        let mut push = |i: usize, x: i32, y: i32, ww: i32, hh: i32| {
            if ww > 0 && hh > 0 {
                // Um buffer novo por pedaço (cada um com o seu id para o rastreador de danos).
                let b = SolidColorBuffer::new((ww, hh), if i == 4 { line } else { dim });
                out.push(GloveElement::Solid(SolidColorRenderElement::from_buffer(&b, Point::<i32, Logical>::from((x, y)).to_physical_precise_round(sc), sc, 1.0, Kind::Unspecified)));
            }
        };
        if r.start.is_some() && x1 > x0 && y1 > y0 {
            // Contorno branco de 1 ponto.
            push(4, x0, y0, x1 - x0, 1);
            push(4, x0, y1 - 1, x1 - x0, 1);
            push(4, x0, y0, 1, y1 - y0);
            push(4, x1 - 1, y0, 1, y1 - y0);
        }
        if x1 > x0 && y1 > y0 {
            push(0, 0, 0, w, y0);
            push(1, 0, y1, w, h - y1);
            push(2, 0, y0, x0, y1 - y0);
            push(3, x1, y0, w - x1, y1 - y0);
        } else {
            push(0, 0, 0, w, h);
        }
    }

    pub fn region_elements(&self, sc: f64, out: &mut Vec<GloveElement>) {
        let size = self.output_size();
        self.region_elements_at(sc, Point::from((0, 0)), size, out);
    }
}
