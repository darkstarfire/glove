//! Gestos do touchpad com vários dedos (só na sessão própria, pelo libinput):
//!
//! - 3 dedos para cima: Exposé (todas as janelas); para baixo: as janelas do app
//!   (com o Exposé aberto, qualquer um dos dois volta);
//! - 3 dedos para os lados: a mesa virtual vizinha (o conteúdo acompanha os dedos:
//!   para a esquerda vai à próxima);
//! - pinçar com 4 dedos: Todos os Apps; abrir os 4 dedos: mostrar a mesa.
//!
//! O movimento é somado e decidido quando os dedos saem (Preferências ▸ Trackpad
//! liga e desliga).

use crate::{expose::ExposeMode, state::Glove};

/// Quanto os dedos precisam andar (em pontos do libinput) para valer.
const SWIPE_MIN: f64 = 70.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Swipe,
    Pinch,
}

#[derive(Clone, Debug)]
pub struct Gesto {
    kind: Kind,
    fingers: u32,
    dx: f64,
    dy: f64,
    scale: f64,
}

/// O que um gesto terminado faz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Acao {
    ExposeAll,
    ExposeApp,
    Desk(i32),
    Grade,
    ShowDesktop,
}

/// Decide o gesto (None = não era nada).
pub fn decide(g: &Gesto) -> Option<Acao> {
    match (g.kind, g.fingers) {
        (Kind::Swipe, 3) => {
            if g.dx.abs().max(g.dy.abs()) < SWIPE_MIN {
                None
            } else if g.dy.abs() > g.dx.abs() {
                Some(if g.dy < 0.0 { Acao::ExposeAll } else { Acao::ExposeApp })
            } else {
                Some(Acao::Desk(if g.dx < 0.0 { 1 } else { -1 }))
            }
        }
        (Kind::Pinch, 4..) if g.scale < 0.75 => Some(Acao::Grade),
        (Kind::Pinch, 4..) if g.scale > 1.35 => Some(Acao::ShowDesktop),
        _ => None,
    }
}

impl Glove {
    pub fn gesture_begin(&mut self, kind: Kind, fingers: u32) {
        self.gesto = (self.config.touchpad.gestures && fingers >= 3 && !self.locked()).then_some(Gesto { kind, fingers, dx: 0.0, dy: 0.0, scale: 1.0 });
    }

    pub fn gesture_swipe(&mut self, dx: f64, dy: f64) {
        if let Some(g) = &mut self.gesto {
            g.dx += dx;
            g.dy += dy;
        }
    }

    pub fn gesture_pinch(&mut self, dx: f64, dy: f64, scale: f64) {
        if let Some(g) = &mut self.gesto {
            g.dx += dx;
            g.dy += dy;
            g.scale = scale;
        }
    }

    pub fn gesture_end(&mut self, cancelled: bool) {
        let Some(g) = self.gesto.take() else { return };
        if cancelled {
            return;
        }
        let Some(a) = decide(&g) else { return };
        match a {
            Acao::ExposeAll | Acao::ExposeApp if self.expose_active() => self.leave_expose(None),
            Acao::ExposeAll => self.toggle_expose(ExposeMode::All),
            Acao::ExposeApp => self.toggle_expose(ExposeMode::App),
            Acao::Desk(d) => {
                // Com o Exposé aberto, troca e continua nele.
                let in_expose = self.expose.is_some();
                self.expose = None;
                self.step_desk(d, false);
                if in_expose {
                    self.toggle_expose(ExposeMode::All);
                }
            }
            Acao::Grade => self.toggle_grade(),
            Acao::ShowDesktop => self.toggle_expose(ExposeMode::Desktop),
        }
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(kind: Kind, fingers: u32, dx: f64, dy: f64, scale: f64) -> Gesto {
        Gesto { kind, fingers, dx, dy, scale }
    }

    #[test]
    fn gestos() {
        assert_eq!(decide(&g(Kind::Swipe, 3, 5.0, -200.0, 1.0)), Some(Acao::ExposeAll));
        assert_eq!(decide(&g(Kind::Swipe, 3, 5.0, 200.0, 1.0)), Some(Acao::ExposeApp));
        assert_eq!(decide(&g(Kind::Swipe, 3, -150.0, 20.0, 1.0)), Some(Acao::Desk(1)));
        assert_eq!(decide(&g(Kind::Swipe, 3, 150.0, 20.0, 1.0)), Some(Acao::Desk(-1)));
        assert_eq!(decide(&g(Kind::Swipe, 3, 10.0, 10.0, 1.0)), None);
        assert_eq!(decide(&g(Kind::Swipe, 2, 0.0, -300.0, 1.0)), None);
        assert_eq!(decide(&g(Kind::Pinch, 4, 0.0, 0.0, 0.5)), Some(Acao::Grade));
        assert_eq!(decide(&g(Kind::Pinch, 4, 0.0, 0.0, 1.6)), Some(Acao::ShowDesktop));
        assert_eq!(decide(&g(Kind::Pinch, 4, 0.0, 0.0, 1.0)), None);
    }
}
