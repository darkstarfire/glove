//! Fim de sessão: pede para cada janela fechar (os apps podem perguntar se querem
//! salvar), espera, mostra a tela de encerramento e então desliga, reinicia ou sai.
//! Se um app não fecha, o encerramento é cancelado e o usuário é avisado.

use std::time::{Duration, Instant};

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{shell::Tex, state::Glove};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndKind {
    LogOut,
    Restart,
    ShutDown,
}

impl EndKind {
    fn from_word(s: &str) -> Option<Self> {
        Some(match s {
            "encerrar" | "sair" => Self::LogOut,
            "reiniciar" => Self::Restart,
            "desligar" => Self::ShutDown,
            _ => return None,
        })
    }
    fn word(self) -> &'static str {
        match self {
            Self::LogOut => "encerrar",
            Self::Restart => "reiniciar",
            Self::ShutDown => "desligar",
        }
    }
    pub fn label(self, nested: bool) -> &'static str {
        match self {
            Self::LogOut if nested => "Saindo do Glove…",
            Self::LogOut => "Encerrando a sessão…",
            Self::Restart => "Reiniciando…",
            Self::ShutDown => "Desligando…",
        }
    }
}

/// Tempo sem nenhuma janela fechar até desistir (o app pode estar perguntando algo).
const GIVE_UP: Duration = Duration::from_secs(30);
/// Janelas minimizadas que não fecharam logo voltam à tela (podem estar perguntando algo).
const RESTORE_AFTER: Duration = Duration::from_secs(2);
/// Quanto a tela de encerramento aparece antes de agir.
const SCREEN_HOLD: Duration = Duration::from_millis(800);
/// Desligando/reiniciando: se o sistema não nos encerrar nesse tempo, saímos.
const SYSTEM_WAIT: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_millis(100);

pub struct Ending {
    pub kind: EndKind,
    /// Janelas que ainda faltam fechar (para ver se algo mudou).
    remaining: usize,
    last_change: Instant,
    started: Instant,
    restored: bool,
    /// Momento em que a tela de encerramento apareceu (todas as janelas fecharam).
    pub screen_since: Option<Instant>,
    /// O pedido ao sistema já foi feito.
    acted: bool,
    pub bg: Option<Tex>,
    pub bg_key: Option<(i32, i32, u64)>,
    pub spinner: Option<Tex>,
    pub spinner_key: Option<(i32, u64)>,
}

impl Ending {
    /// Passo do giratório (12 por volta, uma volta por segundo).
    pub fn spinner_step(&self) -> i32 {
        self.screen_since.map(|t| (t.elapsed().as_millis() / 83) as i32 % 12).unwrap_or(0)
    }
}

impl Glove {
    fn open_windows(&self) -> usize {
        self.space.elements().count() + self.minimized.len() + self.hidden.len()
    }

    /// A tela de encerramento está à frente (nada mais responde).
    pub fn ending_screen_shown(&self) -> bool {
        self.ending.as_ref().is_some_and(|e| e.screen_since.is_some())
    }

    /// Começa o fim de sessão pedido pelo glove-sair ("encerrar", "reiniciar", "desligar").
    pub fn end_session(&mut self, word: &str) {
        let Some(kind) = EndKind::from_word(word) else { return };
        if self.ending.is_some() {
            return;
        }
        tracing::info!("fim de sessão: {word}");
        self.close_menu();
        // Escondidas voltam à tela: podem perguntar se querem salvar.
        self.unhide_where(|_| true);
        let windows: Vec<_> = self.space.elements().cloned().chain(self.minimized.iter().map(|(w, _)| w.clone())).collect();
        for w in &windows {
            self.close_window(w);
        }
        let now = Instant::now();
        self.ending = Some(Ending {
            kind,
            remaining: windows.len(),
            last_change: now,
            started: now,
            restored: false,
            screen_since: None,
            acted: false,
            bg: None,
            bg_key: None,
            spinner: None,
            spinner_key: None,
        });
        self.loop_handle
            .insert_source(Timer::from_duration(TICK), |_, _, data| {
                if data.state.tick_ending() { TimeoutAction::ToDuration(TICK) } else { TimeoutAction::Drop }
            })
            .ok();
    }

    /// Um passo do encerramento. Devolve falso quando não há mais o que acompanhar.
    fn tick_ending(&mut self) -> bool {
        let open = self.open_windows();
        let Some(e) = &mut self.ending else { return false };

        if e.screen_since.is_none() {
            if open == 0 {
                e.screen_since = Some(Instant::now());
                self.dirty = true;
                return true;
            }
            if open < e.remaining {
                e.remaining = open;
                e.last_change = Instant::now();
            }
            if !e.restored && e.started.elapsed() > RESTORE_AFTER {
                e.restored = true;
                while !self.minimized.is_empty() {
                    self.restore_minimized(0);
                }
                return true;
            }
            if e.last_change.elapsed() > GIVE_UP {
                // Um app não fechou (o usuário cancelou o "Salvar?" ou o app travou).
                let kind = e.kind;
                let app = self.space.elements().next().map(|w| crate::shell::app_display_name(w)).unwrap_or_default();
                self.ending = None;
                tracing::info!("fim de sessão cancelado: {app} não fechou");
                let word = if kind == EndKind::LogOut && self.nested { "sair" } else { kind.word() };
                self.spawn(&format!("glove-sair cancelado {word} {}", shell_quote(&app)));
                return false;
            }
            return true;
        }

        // Tela de encerramento: o giratório anda.
        self.dirty = true;
        let shown = e.screen_since.unwrap().elapsed();
        if shown < SCREEN_HOLD {
            return true;
        }
        let kind = e.kind;
        if kind == EndKind::LogOut {
            self.loop_signal.stop();
            return false;
        }
        if !e.acted {
            e.acted = true;
            let cmd = if kind == EndKind::Restart { "reboot" } else { "poweroff" };
            let ok = std::process::Command::new("systemctl").arg(cmd).status().map(|s| s.success()).unwrap_or(false);
            if !ok {
                tracing::warn!("systemctl {cmd} falhou");
                self.ending = None;
                self.dirty = true;
                self.spawn(&format!("glove-sair falhou {}", kind.word()));
                return false;
            }
        }
        if shown > SCREEN_HOLD + SYSTEM_WAIT {
            self.loop_signal.stop();
            return false;
        }
        true
    }
}

/// Aspas simples para o `sh -c` do spawn.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
