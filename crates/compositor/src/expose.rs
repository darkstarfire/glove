//! Exposé (10.3): F9 espalha todas as janelas lado a lado, F10 só as do app da
//! frente, F11 afasta todas para as bordas e mostra a mesa. Clique traz a
//! janela à frente; Esc ou a mesma tecla voltam. Animado no compositor.
//! Os cantos da tela podem disparar os mesmos modos (config `hot_corners`).

use std::time::{Duration, Instant};

use glove_paint::chrome;
use smithay::{
    backend::renderer::element::solid::SolidColorBuffer,
    desktop::Window,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    shell::{app_display_name, deco_top, to_tex, window_info, Tex},
    state::Glove,
};

const ANIM: Duration = Duration::from_millis(280);
/// Quanto da janela fica à vista na borda, no modo mesa.
const PEEK: f64 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExposeMode {
    All,
    App,
    Desktop,
}

impl ExposeMode {
    pub fn from_word(s: &str) -> Option<Self> {
        Some(match s {
            "todas" => Self::All,
            "app" => Self::App,
            "mesa" => Self::Desktop,
            _ => return None,
        })
    }
}

pub struct ExposeItem {
    pub window: Window,
    /// Moldura (com a barra de título) antes e no Exposé.
    pub from: Rectangle<f64, Logical>,
    pub to: Rectangle<f64, Logical>,
}

pub struct Expose {
    pub mode: ExposeMode,
    pub items: Vec<ExposeItem>,
    start: Instant,
    /// Voltando: quando começou e a janela a trazer à frente.
    leaving: Option<(Instant, Option<Window>)>,
    pub hover: Option<usize>,
    pub dim: SolidColorBuffer,
    pub label: Option<(usize, Tex)>,
}

impl Expose {
    /// Progresso de 0 (lugar normal) a 1 (Exposé), já suavizado.
    pub fn progress(&self) -> f64 {
        let t = |since: Instant| (since.elapsed().as_secs_f64() / ANIM.as_secs_f64()).clamp(0.0, 1.0);
        let p = match &self.leaving {
            Some((since, _)) => 1.0 - t(*since),
            None => t(self.start),
        };
        // Começa rápido e freia no fim.
        1.0 - (1.0 - p).powi(3)
    }

    pub fn current(&self, i: usize) -> Rectangle<f64, Logical> {
        let it = &self.items[i];
        let p = self.progress();
        let lerp = |a: f64, b: f64| a + (b - a) * p;
        Rectangle::new((lerp(it.from.loc.x, it.to.loc.x), lerp(it.from.loc.y, it.to.loc.y)).into(), (lerp(it.from.size.w, it.to.size.w), lerp(it.from.size.h, it.to.size.h)).into())
    }

    pub fn item_of(&self, w: &Window) -> Option<usize> {
        self.items.iter().position(|it| &it.window == w)
    }
}

/// Grade que aproveita melhor a área: (colunas, linhas).
fn grid(sizes: &[(f64, f64)], area: (f64, f64)) -> (usize, usize) {
    let n = sizes.len().max(1);
    let mut best = (1, n, 0.0);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let (cw, ch) = (area.0 / cols as f64, area.1 / rows as f64);
        let total: f64 = sizes.iter().map(|(w, h)| {
            let s = (cw * 0.88 / w).min(ch * 0.88 / h).min(1.0);
            w * h * s * s
        }).sum();
        if total > best.2 {
            best = (cols, rows, total);
        }
    }
    (best.0, best.1)
}

impl Glove {
    fn frame_rect(&self, w: &Window) -> Option<Rectangle<f64, Logical>> {
        let loc = self.space.element_location(w)?;
        let size = w.geometry().size;
        let top = deco_top(w);
        Some(Rectangle::new((loc.x, loc.y - top).into(), (size.w, size.h + top).into()).to_f64())
    }

    /// F9/F10/F11 (ou canto): entra no modo; o mesmo modo de novo volta.
    pub fn toggle_expose(&mut self, mode: ExposeMode) {
        if let Some(e) = &self.expose {
            if e.leaving.is_none() && e.mode == mode {
                self.leave_expose(None);
                return;
            }
            // Outro modo: troca na hora.
            self.expose = None;
        }
        self.close_menu();
        self.close_search();
        let focused_pid = self.focused_pid();
        let windows: Vec<Window> = self
            .space
            .elements()
            .filter(|w| mode != ExposeMode::App || (focused_pid.is_some() && self.window_pid(w) == focused_pid))
            .cloned()
            .collect();
        if windows.is_empty() {
            return;
        }
        let mut items: Vec<ExposeItem> = windows.into_iter().filter_map(|w| self.frame_rect(&w).map(|r| ExposeItem { window: w, from: r, to: r })).collect();
        let area = self.work_area().to_f64();
        let (sw, sh) = self.output_size();
        match mode {
            ExposeMode::Desktop => {
                // Cada janela vai para a borda (esquerda ou direita) mais perto, quase toda fora.
                for it in &mut items {
                    let cx = it.from.loc.x + it.from.size.w / 2.0;
                    it.to.loc.x = if cx < sw as f64 / 2.0 { PEEK - it.from.size.w } else { sw as f64 - PEEK };
                }
            }
            _ => {
                // Ordem pela posição (de cima para baixo, da esquerda para a direita).
                items.sort_by(|a, b| (a.from.loc.y + a.from.size.h / 2.0).partial_cmp(&(b.from.loc.y + b.from.size.h / 2.0)).unwrap().then(a.from.loc.x.partial_cmp(&b.from.loc.x).unwrap()));
                let pad = 24.0;
                let (ax, ay, aw, ah) = (area.loc.x + pad, area.loc.y + pad, area.size.w - 2.0 * pad, area.size.h - 2.0 * pad);
                let sizes: Vec<(f64, f64)> = items.iter().map(|i| (i.from.size.w, i.from.size.h)).collect();
                let (cols, rows) = grid(&sizes, (aw, ah));
                let (cw, ch) = (aw / cols as f64, ah / rows as f64);
                let n = items.len();
                for (k, it) in items.iter_mut().enumerate() {
                    let (r, c) = (k / cols, k % cols);
                    // Última linha incompleta fica centralizada.
                    let in_row = if r == rows - 1 { n - r * cols } else { cols };
                    let x0 = ax + (aw - in_row as f64 * cw) / 2.0;
                    let s = (cw * 0.88 / it.from.size.w).min(ch * 0.88 / it.from.size.h).min(1.0);
                    let (w, h) = (it.from.size.w * s, it.from.size.h * s);
                    it.to = Rectangle::new((x0 + c as f64 * cw + (cw - w) / 2.0, ay + r as f64 * ch + (ch - h) / 2.0).into(), (w, h).into());
                }
            }
        }
        let _ = sh;
        self.expose = Some(Expose { mode, items, start: Instant::now(), leaving: None, hover: None, dim: SolidColorBuffer::new((sw, sh), [0.0, 0.0, 0.0, 0.45]), label: None });
        self.dirty = true;
        self.expose_timer();
    }

    fn expose_timer(&mut self) {
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
                let st = &mut data.state;
                st.dirty = true;
                let Some(e) = &st.expose else { return TimeoutAction::Drop };
                match &e.leaving {
                    Some((since, _)) if since.elapsed() >= ANIM => {
                        let focus = e.leaving.as_ref().and_then(|l| l.1.clone());
                        st.expose = None;
                        if let Some(w) = focus {
                            st.focus_window(Some(w));
                        }
                        TimeoutAction::Drop
                    }
                    None if e.start.elapsed() >= ANIM => TimeoutAction::Drop,
                    _ => TimeoutAction::ToDuration(Duration::from_millis(16)),
                }
            })
            .ok();
    }

    /// Volta ao normal, trazendo `focus` à frente no fim.
    pub fn leave_expose(&mut self, focus: Option<Window>) {
        let Some(e) = &mut self.expose else { return };
        if e.leaving.is_some() {
            return;
        }
        // Se ainda estava entrando, volta do ponto em que está.
        let p = e.progress();
        let back = Instant::now() - ANIM.mul_f64(1.0 - p.clamp(0.0, 1.0));
        e.leaving = Some((back, focus.clone()));
        e.label = None;
        if let Some(w) = &focus {
            self.space.raise_element(w, true);
        }
        self.dirty = true;
        self.expose_timer();
    }

    pub fn expose_active(&self) -> bool {
        self.expose.is_some()
    }

    /// Janela do Exposé sob o ponto (a de cima primeiro).
    pub fn expose_hit(&self, pos: Point<f64, Logical>) -> Option<usize> {
        let e = self.expose.as_ref()?;
        for w in self.space.elements().rev() {
            if let Some(i) = e.item_of(w) {
                if e.current(i).contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn expose_motion(&mut self, pos: Point<f64, Logical>) {
        let hit = self.expose_hit(pos);
        if let Some(e) = &mut self.expose {
            if e.hover != hit && e.mode != ExposeMode::Desktop {
                e.hover = hit;
                self.dirty = true;
            }
        }
    }

    /// Clique: na janela traz ela; fora volta como estava.
    pub fn expose_click(&mut self, pos: Point<f64, Logical>) {
        let hit = self.expose_hit(pos);
        let w = hit.and_then(|i| self.expose.as_ref().map(|e| e.items[i].window.clone()));
        self.leave_expose(w);
    }

    pub fn prepare_expose(&mut self) {
        let sc = self.scale() as f32;
        let (sw, sh) = self.output_size();
        let Some(e) = &mut self.expose else { return };
        e.dim.resize((sw, sh));
        // A cor muda com a animação (muda também o "commit", senão a tela não é refeita).
        let a = (0.45 * e.progress()) as f32;
        e.dim.set_color([0.0, 0.0, 0.0, a]);
        match e.hover {
            Some(i) if e.leaving.is_none() => {
                if e.label.as_ref().map(|l| l.0) != Some(i) {
                    let w = &e.items[i].window;
                    let (title, _) = window_info(w);
                    let text = if title.is_empty() { app_display_name(w) } else { title };
                    e.label = chrome::expose_label(&self.fonts, &text, sc).map(|c| (i, to_tex(c)));
                }
            }
            _ => e.label = None,
        }
    }

    // -----------------------------------------------------------------------
    // Cantos ativos
    // -----------------------------------------------------------------------

    /// Mouse encostado num canto: dispara o modo configurado (uma vez por chegada).
    pub fn hot_corner(&mut self, pos: Point<f64, Logical>) {
        let (w, h) = self.output_size();
        let (w, h) = (w as f64 - 1.0, h as f64 - 1.0);
        let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
        let hc = &self.config.hot_corners;
        let corner = if near(pos.x, 0.0) && near(pos.y, 0.0) {
            Some(hc.top_left.clone())
        } else if near(pos.x, w) && near(pos.y, 0.0) {
            Some(hc.top_right.clone())
        } else if near(pos.x, 0.0) && near(pos.y, h) {
            Some(hc.bottom_left.clone())
        } else if near(pos.x, w) && near(pos.y, h) {
            Some(hc.bottom_right.clone())
        } else {
            None
        };
        let armed = corner.is_none();
        if let Some(action) = corner {
            if self.corner_armed {
                if let Some(mode) = ExposeMode::from_word(&action) {
                    self.toggle_expose(mode);
                }
            }
        }
        self.corner_armed = armed;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn grade() {
        // Quatro janelas iguais numa área larga: 2×2 ou 4×1, nunca 1×4.
        let s = vec![(800.0, 600.0); 4];
        let (c, r) = super::grid(&s, (1600.0, 900.0));
        assert!(c >= 2 && c * r >= 4);
        assert_eq!(super::grid(&[(400.0, 300.0)], (1600.0, 900.0)), (1, 1));
    }
}
