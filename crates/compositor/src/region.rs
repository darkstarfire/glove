//! Captura de uma área: a tela escurece, o mouse arrasta um retângulo e só ele
//! vai para o PNG (como o Shift+Cmd+4 da era), em qualquer monitor. No modo janela,
//! a janela sob o mouse fica destacada e o clique a fotografa. Esc cancela.

use smithay::{
    backend::renderer::element::{solid::{SolidColorBuffer, SolidColorRenderElement}, Kind},
    utils::{Logical, Point, Rectangle},
};

use crate::{render::GloveElement, state::Glove};

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
        self.region = Some(RegionSel { start: None, cur: self.pointer_pos, open, window });
        self.dirty = true;
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
                // Na frente de tudo, para nada a cobrir na foto.
                self.space.raise_element(&w, true);
                self.screenshot_crop = Some(rect);
                self.open_after_shot = open;
                self.request_screenshot();
            }
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
