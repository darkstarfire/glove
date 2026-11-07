//! Protetor de tela: começa depois de `saver.minutes` parado (antes de apagar
//! a tela), num canto ativo ou pelo botão Testar; qualquer entrada tira.
//! O desenho (do glove_paint) é feito em meia resolução, ~20 quadros por segundo.

use glove_config::tr;
use std::time::{Duration, Instant};

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{shell::Tex, state::Glove};

const FRAME: Duration = Duration::from_millis(50);

pub struct ScreenSaver {
    pub kind: String,
    since: Instant,
    pub tex: Option<Tex>,
}

impl Glove {
    /// Liga o protetor (`kind` None = o escolhido nas Preferências). `test`: pelo
    /// botão Testar (não bloqueia a tela).
    pub fn start_saver(&mut self, kind: Option<String>, test: bool) {
        if self.greeter || self.saver.is_some() || self.locked() && test {
            return;
        }
        if !test && self.config.energy.require_password {
            self.lock_screen();
        }
        self.close_menu();
        let kind = kind.unwrap_or_else(|| self.config.saver.kind.clone());
        tracing::info!("protetor de tela: {kind}{}", if test { " (teste)" } else { "" });
        self.saver = Some(ScreenSaver { kind, since: Instant::now(), tex: None });
        self.saver_frame();
        self.loop_handle
            .insert_source(Timer::from_duration(FRAME), |_, _, data| {
                if data.state.saver.is_none() {
                    return TimeoutAction::Drop;
                }
                data.state.saver_frame();
                TimeoutAction::ToDuration(FRAME)
            })
            .ok();
    }

    /// Tira o protetor. Devolve verdadeiro se ele estava na tela.
    pub fn stop_saver(&mut self) -> bool {
        if self.saver.take().is_some() {
            self.dirty = true;
            return true;
        }
        false
    }

    fn saver_frame(&mut self) {
        let (w, h) = self.output_size();
        let Some((kind, t)) = self.saver.as_ref().map(|s| (s.kind.clone(), s.since.elapsed().as_secs_f32())) else { return };
        let (w, h) = (w as f32, h as f32);
        // Meia resolução: o protetor é suave e não precisa de cada pixel.
        let Some(mut c) = glove_paint::draw::Canvas::new(w, h, 0.5) else { return };
        let (wd, d, mo, y, hh, mm, _) = crate::shell::local_now();
        let dias = [tr!("domingo"), tr!("segunda-feira"), tr!("terça-feira"), tr!("quarta-feira"), tr!("quinta-feira"), tr!("sexta-feira"), tr!("sábado")];
        let meses = [tr!("janeiro"), tr!("fevereiro"), tr!("março"), tr!("abril"), tr!("maio"), tr!("junho"), tr!("julho"), tr!("agosto"), tr!("setembro"), tr!("outubro"), tr!("novembro"), tr!("dezembro")];
        let time = format!("{hh:02}:{mm:02}");
        let date = tr!("{0}, {1} de {2} de {3}", dias[(wd % 7) as usize], d, meses[((mo + 11) % 12) as usize], y);
        glove_paint::saver::draw(&mut c, &self.fonts, &kind, w, h, t, (&time, &date));
        if let Some(s) = &mut self.saver {
            s.tex = Some(crate::shell::to_tex(c));
        }
        self.dirty = true;
    }
}
