//! Minimizar com o efeito "Escala" (a janela encolhe até o lugar dela no Dock e
//! volta do mesmo jeito) e a miniatura viva da janela minimizada no Dock, com o
//! ícone do app no canto. As minimizadas recebem quadros no máximo 2×/s, e só
//! quando mudam (app parado não acorda nada).

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use smithay::{
    desktop::Window,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Rectangle},
};

use crate::{
    shell::{deco_top, to_tex, window_info, Tex},
    state::Glove,
};

const ANIM: Duration = Duration::from_millis(330);
/// Intervalo mínimo entre quadros para as janelas minimizadas.
pub const MINIMIZED_FRAME: Duration = Duration::from_millis(500);

pub struct MinAnim {
    pub window: Window,
    start: Instant,
    /// Voltando do Dock (senão, indo).
    pub restoring: bool,
    /// Moldura na tela (com a barra de título).
    pub frame: Rectangle<f64, Logical>,
}

impl MinAnim {
    /// 0 = na tela, 1 = no Dock (suavizado nos dois extremos).
    pub fn progress(&self) -> f64 {
        let t = (self.start.elapsed().as_secs_f64() / ANIM.as_secs_f64()).clamp(0.0, 1.0);
        let e = t * t * (3.0 - 2.0 * t);
        if self.restoring { 1.0 - e } else { e }
    }
    pub fn done(&self) -> bool {
        self.start.elapsed() >= ANIM
    }
}

fn lerp_rect(a: Rectangle<f64, Logical>, b: Rectangle<f64, Logical>, p: f64) -> Rectangle<f64, Logical> {
    let l = |x: f64, y: f64| x + (y - x) * p;
    Rectangle::new((l(a.loc.x, b.loc.x), l(a.loc.y, b.loc.y)).into(), (l(a.size.w, b.size.w), l(a.size.h, b.size.h)).into())
}

/// Moldura que cabe num quadrado (mantendo a proporção), centrada nele.
pub fn fit(frame: (f64, f64), slot: Rectangle<f64, Logical>) -> Rectangle<f64, Logical> {
    let s = (slot.size.w * 0.92 / frame.0).min(slot.size.h * 0.92 / frame.1);
    let (w, h) = (frame.0 * s, frame.1 * s);
    Rectangle::new((slot.loc.x + (slot.size.w - w) / 2.0, slot.loc.y + (slot.size.h - h) / 2.0).into(), (w, h).into())
}

impl Glove {
    fn frame_of(&self, w: &Window, loc: smithay::utils::Point<i32, Logical>) -> Rectangle<f64, Logical> {
        let size = w.geometry().size;
        let top = deco_top(w);
        Rectangle::new((loc.x, loc.y - top).into(), (size.w, size.h + top).into()).to_f64()
    }

    /// Quadrado do ícone da janela minimizada no Dock (coordenadas da tela).
    pub fn dock_slot(&self, w: &Window) -> Option<Rectangle<f64, Logical>> {
        let i = self.minimized.iter().position(|(m, _)| m == w)?;
        let idx = self.config.dock_apps.len() + i;
        let l = self.chrome.dock_layout.as_ref()?;
        let (x, y, s) = *l.items.get(idx)?;
        let d = self.chrome.dock_pos();
        Some(Rectangle::new((d.x as f64 + x as f64, d.y as f64 + y as f64).into(), (s as f64, s as f64).into()))
    }

    /// Onde a janela minimizada aparece no Dock (já com a proporção dela).
    pub fn thumbnail_rect(&self, w: &Window) -> Option<Rectangle<f64, Logical>> {
        let slot = self.dock_slot(w)?;
        let size = w.geometry().size;
        Some(fit((size.w as f64, (size.h + deco_top(w)) as f64), slot))
    }

    /// Começa a animação de minimizar (a janela já saiu da tela).
    pub fn animate_minimize(&mut self, w: &Window, loc: smithay::utils::Point<i32, Logical>) {
        let frame = self.frame_of(w, loc);
        self.min_anims.retain(|a| &a.window != w);
        self.min_anims.push(MinAnim { window: w.clone(), start: Instant::now(), restoring: false, frame });
        self.min_anim_timer();
    }

    /// Começa a animação de voltar do Dock (a janela já voltou para a tela).
    pub fn animate_restore(&mut self, w: &Window, loc: smithay::utils::Point<i32, Logical>, from_slot: Option<Rectangle<f64, Logical>>) {
        let frame = self.frame_of(w, loc);
        self.min_anims.retain(|a| &a.window != w);
        if from_slot.is_some() {
            self.restore_slots.retain(|(x, _)| x != w);
            self.restore_slots.push((w.clone(), from_slot.unwrap()));
        }
        self.min_anims.push(MinAnim { window: w.clone(), start: Instant::now(), restoring: true, frame });
        self.min_anim_timer();
    }

    fn min_anim_timer(&mut self) {
        self.dirty = true;
        if self.min_anim_timer_on {
            return;
        }
        self.min_anim_timer_on = true;
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
                let st = &mut data.state;
                st.dirty = true;
                let done: Vec<Window> = st.min_anims.iter().filter(|a| a.done()).map(|a| a.window.clone()).collect();
                st.min_anims.retain(|a| !a.done());
                st.restore_slots.retain(|(w, _)| !done.contains(w));
                if st.min_anims.is_empty() {
                    st.min_anim_timer_on = false;
                    TimeoutAction::Drop
                } else {
                    TimeoutAction::ToDuration(Duration::from_millis(16))
                }
            })
            .ok();
    }

    /// Retângulo da janela agora, na animação (None se não está animando).
    pub fn min_anim_rect(&self, w: &Window) -> Option<Rectangle<f64, Logical>> {
        let a = self.min_anims.iter().find(|a| &a.window == w)?;
        let size = w.geometry().size;
        let frame_size = (size.w as f64, (size.h + deco_top(w)) as f64);
        // O lugar no Dock: o atual (indo) ou o que ela tinha (voltando).
        let slot = if a.restoring {
            self.restore_slots.iter().find(|(x, _)| x == w).map(|(_, r)| fit(frame_size, *r))?
        } else {
            self.thumbnail_rect(w)?
        };
        Some(lerp_rect(a.frame, slot, a.progress()))
    }

    pub fn min_animating(&self, w: &Window) -> bool {
        self.min_anims.iter().any(|a| &a.window == w)
    }

    /// Ícone do app no canto da miniatura.
    pub fn badge_tex(&mut self, w: &Window, size: f32) -> Option<Rc<Tex>> {
        let app_id = window_info(w).1;
        let icon = self.config.dock_apps.iter().find(|a| !a.app_id.is_empty() && a.app_id.eq_ignore_ascii_case(&app_id)).map(|a| glove_paint::icons::Icon::from_name(&a.icon))?;
        let sc = self.scale() as f32;
        let key = (format!("{icon:?}"), size.round() as i32, sc.to_bits());
        if let Some((_, t)) = self.badges.iter().find(|(k, _)| *k == key) {
            return Some(t.clone());
        }
        let mut c = glove_paint::draw::Canvas::new(size, size, sc)?;
        glove_paint::icons::draw(&mut c, &self.fonts, icon, 0.0, 0.0, size, None);
        let t = Rc::new(to_tex(c));
        self.badges.push((key, t.clone()));
        Some(t)
    }

    /// Selos dos apps das minimizadas (preparados antes de desenhar).
    pub fn prepare_minimized(&mut self) {
        let wins: Vec<Window> = self.minimized.iter().map(|(w, _)| w.clone()).collect();
        let mut out = Vec::new();
        for w in wins {
            let Some(slot) = self.dock_slot(&w) else { continue };
            let size = (slot.size.w * 0.42) as f32;
            if let Some(t) = self.badge_tex(&w, size) {
                let x = slot.loc.x + slot.size.w - size as f64;
                let y = slot.loc.y + slot.size.h - size as f64;
                out.push((w, (x.round() as i32, y.round() as i32), t));
            }
        }
        self.badge_places = out;
    }
}
