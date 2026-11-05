//! Captura de uma área: a tela escurece, o mouse arrasta um retângulo e só ele
//! vai para o PNG (como o Shift+Cmd+4 da era). Esc cancela.

use smithay::{
    backend::renderer::element::{solid::{SolidColorBuffer, SolidColorRenderElement}, Kind},
    utils::{Logical, Point, Rectangle},
};

use crate::{render::GloveElement, state::Glove};

pub struct RegionSel {
    pub start: Option<Point<f64, Logical>>,
    pub cur: Point<f64, Logical>,
    pub open: bool,
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
    pub fn start_region_shot(&mut self, open: bool) {
        if self.locked() {
            return;
        }
        self.close_menu();
        self.region = Some(RegionSel { start: None, cur: self.pointer_pos, open });
        self.dirty = true;
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
                self.screenshot_crop = Some(rect);
                self.open_after_shot = open;
                self.request_screenshot();
            }
            None => tracing::info!("captura de área cancelada"),
        }
    }

    pub fn cancel_region(&mut self) {
        if self.region.take().is_some() {
            self.dirty = true;
        }
    }

    /// Película escura em volta da área e o contorno dela (sobre tudo).
    pub fn region_elements(&self, sc: f64, out: &mut Vec<GloveElement>) {
        let Some(r) = &self.region else { return };
        let (w, h) = self.output_size();
        let sel = r.rect().unwrap_or_else(|| Rectangle::new((r.cur.x as i32, r.cur.y as i32).into(), (0, 0).into()));
        let (x0, y0, x1, y1) = (sel.loc.x, sel.loc.y, sel.loc.x + sel.size.w, sel.loc.y + sel.size.h);
        let dim = [0.0, 0.0, 0.0, 0.35];
        let line = [1.0, 1.0, 1.0, 0.9];
        let mut push = |i: usize, x: i32, y: i32, ww: i32, hh: i32| {
            if ww > 0 && hh > 0 {
                // Um buffer novo por pedaço (cada um com o seu id para o rastreador de danos).
                let b = SolidColorBuffer::new((ww, hh), if i == 4 { line } else { dim });
                out.push(GloveElement::Solid(SolidColorRenderElement::from_buffer(&b, Point::<i32, Logical>::from((x, y)).to_physical_precise_round(sc), sc, 1.0, Kind::Unspecified)));
            }
        };
        if r.start.is_some() {
            // Contorno branco de 1 ponto.
            push(4, x0, y0, x1 - x0, 1);
            push(4, x0, y1 - 1, x1 - x0, 1);
            push(4, x0, y0, 1, y1 - y0);
            push(4, x1 - 1, y0, 1, y1 - y0);
        }
        push(0, 0, 0, w, y0);
        push(1, 0, y1, w, h - y1);
        push(2, 0, y0, x0, y1 - y0);
        push(3, x1, y0, w - x1, y1 - y0);
    }
}
