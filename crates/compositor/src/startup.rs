//! Painel "Iniciando sessão" ao entrar: passos com barra de progresso sobre o papel
//! de parede; os itens de início (comandos -c) abrem no último passo e, no fim, o
//! painel some suave e a mesa aparece.

use std::time::{Duration, Instant};

use glove_paint::chrome;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// (início do passo em segundos, texto).
const STEPS: [(f32, &str); 4] = [
    (0.0, "Preparando a área de trabalho…"),
    (0.35, "Iniciando a barra de menu…"),
    (0.7, "Iniciando o Dock…"),
    (1.05, "Abrindo os itens de início…"),
];
/// A barra chega ao fim aqui; depois o painel some.
const DONE: f32 = 1.4;
const FADE: f32 = 0.35;

pub struct Startup {
    since: Instant,
    /// Comandos a abrir no último passo.
    commands: Vec<String>,
    launched: bool,
    key: Option<(usize, u32, u64)>,
    pub tex: Option<Tex>,
}

impl Startup {
    fn t(&self) -> f32 {
        self.since.elapsed().as_secs_f32()
    }
    /// Opacidade do painel (1 até o fim da barra; some depois).
    pub fn alpha(&self) -> f32 {
        (1.0 - (self.t() - DONE) / FADE).clamp(0.0, 1.0)
    }
    /// A mesa ainda está escondida (painel sobre o papel de parede apenas).
    pub fn covering(&self) -> bool {
        self.t() < DONE
    }
}

impl Glove {
    /// Começa a abertura. `commands` abrem no passo "Abrindo os itens de início".
    pub fn start_session_panel(&mut self, commands: Vec<String>) {
        self.startup = Some(Startup { since: Instant::now(), commands, launched: false, key: None, tex: None });
        self.dirty = true;
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(33)), |_, _, data| {
                if data.state.startup_tick() { TimeoutAction::ToDuration(Duration::from_millis(33)) } else { TimeoutAction::Drop }
            })
            .ok();
    }

    fn startup_tick(&mut self) -> bool {
        let Some(s) = &mut self.startup else { return false };
        self.dirty = true;
        let t = s.t();
        if !s.launched && t >= STEPS[3].0 {
            s.launched = true;
            for c in std::mem::take(&mut s.commands) {
                self.spawn(&c);
            }
            // Itens de Início (autostart do XDG que vale no Glove). Dentro de outro
            // desktop, não: os de lá já estão rodando e viriam em dobro.
            if !self.greeter && !self.nested {
                for c in glove_config::inicio::commands() {
                    tracing::info!("item de início: {c}");
                    self.spawn(&c);
                }
            }
            // Primeira vez: o assistente de boas-vindas.
            if !self.config.welcome_done {
                self.spawn("glove-boasvindas");
            }
        }
        if t >= DONE + FADE {
            self.startup = None;
            return false;
        }
        true
    }

    /// Mouse e teclado esperam a mesa aparecer.
    pub fn startup_covering(&self) -> bool {
        self.startup.as_ref().is_some_and(|s| s.covering())
    }

    pub fn prepare_startup(&mut self) {
        let sc = self.scale() as f32;
        let tgen = self.theme_gen;
        let Some(s) = &mut self.startup else { return };
        let t = s.t();
        let step = STEPS.iter().rposition(|(start, _)| t >= *start).unwrap_or(0);
        // Progresso suave dentro de cada passo.
        let progress = (t / DONE).clamp(0.0, 1.0);
        let key = (step, (progress * 200.0) as u32, tgen);
        if s.key != Some(key) {
            s.tex = chrome::startup_panel(&self.fonts, &self.theme, &glove_config::tr_dyn(STEPS[step].1), progress, sc).map(to_tex);
            s.key = Some(key);
        }
    }

    /// Canto do painel (sem a sombra).
    pub fn startup_pos(&self) -> (i32, i32) {
        let (w, h) = self.output_size();
        (((w as f32 - chrome::STARTUP_W) / 2.0) as i32, ((h as f32 - chrome::STARTUP_H) / 2.0 - h as f32 * 0.06) as i32)
    }
}
