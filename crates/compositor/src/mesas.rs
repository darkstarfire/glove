//! Mesas virtuais: várias mesas, cada uma com as suas janelas.
//!
//! As janelas das outras mesas saem do espaço (como as ocultas) e ficam em
//! `away` com o número da mesa e o lugar onde estavam. Trocar de mesa guarda as
//! da mesa atual e põe as da nova, na mesma ordem de empilhamento, com as
//! janelas deslizando de lado e um aviso no meio da tela mostrando as mesas.
//!
//! Ctrl+Super+←/→ trocam de mesa; com Shift, levam a janela da frente junto.
//! No Exposé (F9), a faixa no alto mostra as mesas: clicar troca, arrastar uma
//! janela até uma delas leva a janela para lá. Dar foco a uma janela de outra
//! mesa (pelo Dock, pela troca de apps, pelo menu Janela) vai até a mesa dela.

use glove_config::tr;
use std::time::{Duration, Instant};

use glove_paint::draw::*;
use smithay::{
    desktop::Window,
    utils::IsAlive,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point},
};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// Duração do deslizar das janelas ao trocar de mesa.
const SLIDE: Duration = Duration::from_millis(300);
/// O aviso fica na tela esse tempo e some nesse outro.
const HUD_SHOW: Duration = Duration::from_millis(900);
const HUD_FADE: Duration = Duration::from_millis(250);

/// Janelas saindo da tela na troca: (janela, lugar).
pub struct Slide {
    pub dir: i32,
    start: Instant,
    pub out: Vec<(Window, Point<i32, Logical>)>,
}

#[derive(Default)]
pub struct Mesas {
    /// A mesa na tela (a partir de 0).
    pub cur: usize,
    /// Janelas das outras mesas: (janela, lugar, mesa).
    pub away: Vec<(Window, Point<i32, Logical>, usize)>,
    pub slide: Option<Slide>,
    hud_since: Option<Instant>,
    pub hud: Option<((usize, usize, u64), Tex)>,
    pub hud_alpha: f32,
}

/// Suave no começo e no fim.
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Glove {
    /// Quantas mesas há (da configuração).
    pub fn desk_count(&self) -> usize {
        self.config.desktops.clamp(1, glove_config::MAX_DESKTOPS) as usize
    }

    /// Janelas fora da tela: minimizadas, ocultas e as das outras mesas.
    pub fn stashed(&self) -> impl Iterator<Item = &Window> + '_ {
        self.minimized.iter().map(|(w, _)| w).chain(self.hidden.iter().map(|(w, _)| w)).chain(self.mesas.away.iter().map(|(w, _, _)| w))
    }

    /// Em que mesa a janela está (None = na tela, ou não é de mesa nenhuma).
    pub fn desk_of(&self, w: &Window) -> Option<usize> {
        self.mesas.away.iter().find(|(a, _, _)| a == w).map(|(_, _, d)| *d)
    }

    /// Vai para a mesa `j`. `carry`: a janela que vai junto (fica na frente, com o foco).
    pub fn switch_desk(&mut self, j: usize, carry: Option<Window>) {
        let cur = self.mesas.cur;
        if j >= self.desk_count() || j == cur {
            self.show_desk_hud();
            return;
        }
        self.close_menu();
        self.close_search();
        self.close_grade();
        self.close_dashboard();
        let dir = if j > cur { 1 } else { -1 };
        // Guarda as janelas desta mesa (de baixo para cima, para voltar na mesma ordem).
        let here: Vec<Window> = self.space.elements().cloned().collect();
        let mut out = Vec::new();
        for w in here {
            if Some(&w) == carry.as_ref() || self.x11_overlays.contains(&w) {
                continue;
            }
            if let Some(loc) = self.space.element_location(&w) {
                self.space.unmap_elem(&w);
                self.mesas.away.push((w.clone(), loc, cur));
                out.push((w, loc));
            }
        }
        // E põe as da outra.
        let (back, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.mesas.away).into_iter().partition(|(_, _, d)| *d == j);
        self.mesas.away = keep;
        for (w, loc, _) in back {
            self.space.map_element(w, loc, false);
        }
        self.mesas.cur = j;
        self.mesas.slide = Some(Slide { dir, start: Instant::now(), out });
        match carry {
            Some(w) => {
                self.space.raise_element(&w, true);
                self.focus_window(Some(w));
            }
            None => self.focus_topmost(None),
        }
        self.show_desk_hud();
        self.anim_timer();
        self.dirty = true;
    }

    /// Leva a janela para a mesa `j` sem sair desta (o Exposé usa).
    pub fn send_to_desk(&mut self, w: &Window, j: usize) {
        if j >= self.desk_count() || j == self.mesas.cur {
            return;
        }
        let Some(loc) = self.space.element_location(w) else { return };
        let was_focused = self.focused_window().as_ref() == Some(w);
        self.space.unmap_elem(w);
        self.mesas.away.push((w.clone(), loc, j));
        if was_focused {
            self.focus_topmost(None);
        }
        self.dirty = true;
    }

    /// Ctrl+Super+←/→ (`carry`: com a janela da frente).
    pub fn step_desk(&mut self, d: i32, carry: bool) {
        let j = self.mesas.cur as i32 + d;
        if j < 0 || j >= self.desk_count() as i32 {
            // Já na ponta: só o aviso.
            self.show_desk_hud();
            return;
        }
        let w = if carry { self.focused_window().filter(|w| self.space.element_location(w).is_some()) } else { None };
        self.switch_desk(j as usize, w);
    }

    /// Se a janela está em outra mesa, vai até lá (antes de dar o foco a ela).
    pub fn reveal_window(&mut self, w: &Window) {
        if let Some(d) = self.desk_of(w) {
            self.switch_desk(d, None);
        }
    }

    /// O app não tem janela nesta mesa mas tem em outra: vai até a mais recente.
    pub fn reveal_app(&mut self, which: impl Fn(&Window) -> bool) -> bool {
        if self.space.elements().any(&which) {
            return false;
        }
        let Some(d) = self.mesas.away.iter().rev().find(|(w, _, _)| which(w)).map(|(_, _, d)| *d) else { return false };
        self.switch_desk(d, None);
        true
    }

    /// Menos mesas na configuração: as janelas das que sumiram vão para a última.
    pub fn fit_desks(&mut self) {
        let n = self.desk_count();
        if self.mesas.cur >= n {
            self.switch_desk_now(n - 1);
        }
        for (_, _, d) in &mut self.mesas.away {
            *d = (*d).min(n - 1);
        }
        // A mesa atual pode ter ficado com janelas guardadas: põe na tela.
        let cur = self.mesas.cur;
        let (back, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.mesas.away).into_iter().partition(|(_, _, d)| *d == cur);
        self.mesas.away = keep;
        for (w, loc, _) in back {
            self.space.map_element(w, loc, false);
        }
    }

    /// Troca sem animação (configuração mudou).
    fn switch_desk_now(&mut self, j: usize) {
        self.switch_desk(j, None);
        self.mesas.slide = None;
    }

    /// Quanto as janelas que entram e as que saem estão deslocadas agora (em x ou y).
    pub fn desk_slide_offsets(&self) -> Option<(i32, i32)> {
        let s = self.mesas.slide.as_ref()?;
        let p = ease(s.start.elapsed().as_secs_f64() / SLIDE.as_secs_f64());
        let (w, _) = self.output_size();
        let w = w as f64;
        let d = s.dir as f64;
        Some(((d * w * (1.0 - p)).round() as i32, (-d * w * p).round() as i32))
    }

    pub fn hide_desk_hud(&mut self) {
        self.mesas.hud_since = None;
        self.mesas.hud = None;
    }

    fn show_desk_hud(&mut self) {
        if self.desk_count() < 2 {
            return;
        }
        self.mesas.hud_since = Some(Instant::now());
        self.anim_timer();
        self.dirty = true;
    }

    /// Redesenha enquanto há animação (deslizar ou aviso).
    fn anim_timer(&mut self) {
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
            let st = &mut data.state;
            st.dirty = true;
            let sliding = st.mesas.slide.as_ref().is_some_and(|s| s.start.elapsed() < SLIDE);
            let hud = st.mesas.hud_since.is_some_and(|t| t.elapsed() < HUD_SHOW + HUD_FADE);
            if sliding || hud {
                TimeoutAction::ToDuration(Duration::from_millis(16))
            } else {
                TimeoutAction::Drop
            }
        });
    }

    pub fn prepare_mesas(&mut self) {
        if self.mesas.slide.as_ref().is_some_and(|s| s.start.elapsed() >= SLIDE) {
            self.mesas.slide = None;
        }
        // Janelas que fecharam no meio do deslizar.
        if let Some(s) = &mut self.mesas.slide {
            s.out.retain(|(w, _)| w.alive());
        }
        let Some(since) = self.mesas.hud_since else {
            self.mesas.hud = None;
            return;
        };
        let t = since.elapsed();
        if t >= HUD_SHOW + HUD_FADE {
            self.mesas.hud_since = None;
            self.mesas.hud = None;
            return;
        }
        self.mesas.hud_alpha = if t <= HUD_SHOW { 1.0 } else { 1.0 - (t - HUD_SHOW).as_secs_f32() / HUD_FADE.as_secs_f32() };
        let sc = self.scale() as f32;
        let key = (self.mesas.cur, self.desk_count(), self.theme_gen ^ sc.to_bits() as u64);
        if self.mesas.hud.as_ref().map(|h| h.0) != Some(key) {
            let (sw, sh) = self.output_size();
            self.mesas.hud = draw_hud(&self.fonts, key.0, key.1, sw as f32 / sh.max(1) as f32, sc).map(|c| (key, to_tex(c)));
        }
    }

    /// Onde fica o aviso (no meio da tela, um pouco abaixo do centro).
    pub fn desk_hud_pos(&self) -> Option<Point<i32, Logical>> {
        let (_, t) = self.mesas.hud.as_ref()?;
        let (sw, sh) = self.output_size();
        Some(((sw - t.size.w) / 2, (sh * 3 / 5 - t.size.h / 2)).into())
    }
}

/// O aviso da troca: as mesas lado a lado, a atual acesa, e o nome dela.
fn draw_hud(f: &glove_paint::font::Fonts, cur: usize, n: usize, ratio: f32, sc: f32) -> Option<Canvas> {
    let ch = 34.0;
    let cw = (ch * ratio).round();
    let gap = 10.0;
    let pad = 18.0;
    let w = pad * 2.0 + n as f32 * cw + (n as f32 - 1.0) * gap;
    let h = pad + ch + 34.0;
    let mut c = Canvas::new(w, h, sc)?;
    c.fill(rrect(0.0, 0.0, w, h, 16.0), &solid(rgba(20, 22, 28, 175)));
    for i in 0..n {
        let x = pad + i as f32 * (cw + gap);
        let y = pad;
        if i == cur {
            c.fill(rrect(x, y, cw, ch, 4.0), &vgrad(y, y + ch, &[(0.0, rgba(120, 175, 245, 255)), (1.0, rgba(40, 105, 215, 255))]));
            c.stroke(rrect(x + 0.5, y + 0.5, cw - 1.0, ch - 1.0, 4.0), &solid(rgba(255, 255, 255, 220)), 1.0);
        } else {
            c.fill(rrect(x, y, cw, ch, 4.0), &solid(rgba(255, 255, 255, 40)));
            c.stroke(rrect(x + 0.5, y + 0.5, cw - 1.0, ch - 1.0, 4.0), &solid(rgba(255, 255, 255, 90)), 1.0);
        }
        let num = format!("{}", i + 1);
        let nw = f.measure(&num, 13.0, true);
        f.draw(&mut c, &num, x + (cw - nw) / 2.0, y + ch / 2.0 + 5.0, 13.0, true, rgba(255, 255, 255, if i == cur { 255 } else { 150 }));
    }
    let label = tr!("Mesa {}", cur + 1);
    let lw = f.measure(&label, 13.0, true);
    f.draw(&mut c, &label, (w - lw) / 2.0, h - 13.0, 13.0, true, rgba(255, 255, 255, 235));
    Some(c)
}

#[cfg(test)]
mod tests {
    #[test]
    fn suave() {
        assert_eq!(super::ease(0.0), 0.0);
        assert_eq!(super::ease(1.0), 1.0);
        assert_eq!(super::ease(2.0), 1.0);
        assert!((super::ease(0.5) - 0.5).abs() < 1e-9);
        assert!(super::ease(0.25) < 0.25);
    }
}
