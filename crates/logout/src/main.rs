//! Alertas do fim de sessão (abertos pelo Glove):
//!
//! - `glove-logout shutdown|restart|logout|quit`: confirma, com contagem de 120 s
//!   (sem resposta, a ação acontece sozinha), e pede ao Glove para executar.
//! - `glove-logout cancelled <ação> <app>`: um app não fechou e o encerramento parou.
//! - `glove-logout failed <ação>`: o sistema recusou desligar ou reiniciar.

use glove_config::tr;
use std::time::{Duration, Instant};

use glove_ui::{run, ui::AlertKind, App, Ui, WindowOptions};

/// Segundos até a ação acontecer sozinha.
const COUNTDOWN: u64 = 120;

enum Mode {
    Confirm { kind: &'static str, started: Instant },
    Cancelled { kind: &'static str, app: String },
    Failed { kind: &'static str },
}

struct Sair {
    mode: Mode,
}

const KINDS: [&str; 4] = ["shutdown", "restart", "logout", "quit"];

/// (título da janela, pergunta, botão de confirmar).
fn confirm_texts(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "shutdown" => (tr!("Desligar"), tr!("Tem certeza de que deseja desligar o computador agora?"), tr!("Desligar")),
        "restart" => (tr!("Reiniciar"), tr!("Tem certeza de que deseja reiniciar o computador agora?"), tr!("Reiniciar")),
        "logout" => (tr!("Encerrar Sessão"), tr!("Tem certeza de que deseja encerrar a sessão e sair de todos os aplicativos?"), tr!("Encerrar Sessão")),
        _ => (tr!("Sair do Glove"), tr!("Tem certeza de que deseja sair do Glove?"), tr!("Sair")),
    }
}

/// Aviso da contagem: "…será desligado automaticamente em 2 minutos."
fn countdown_text(kind: &str, left: u64) -> String {
    let what = match kind {
        "shutdown" => tr!("o computador será desligado"),
        "restart" => tr!("o computador será reiniciado"),
        "logout" => tr!("a sessão será encerrada"),
        _ => tr!("o Glove será fechado"),
    };
    let when = match left {
        120 => tr!("2 minutos").to_string(),
        1 => tr!("1 segundo").to_string(),
        s => tr!("{} segundos", s),
    };
    tr!("Se você não fizer nada, {} automaticamente em {}.", what, when)
}

/// Nome da ação para frases: "o desligamento", "a reinicialização"…
fn action_noun(kind: &str) -> &'static str {
    match kind {
        "shutdown" => tr!("O desligamento"),
        "restart" => tr!("A reinicialização"),
        "logout" => tr!("O encerramento da sessão"),
        _ => tr!("A saída do Glove"),
    }
}

impl App for Sair {
    fn ui(&mut self, ui: &mut Ui) {
        match &self.mode {
            Mode::Confirm { kind, started } => {
                let kind = *kind;
                let elapsed = started.elapsed().as_secs();
                let left = COUNTDOWN.saturating_sub(elapsed);
                let (_, q, ok) = confirm_texts(kind);
                let choice = ui.alert_window(AlertKind::Note, q, &countdown_text(kind, left.max(1)), &[tr!("Cancelar"), ok]);
                if left == 0 || choice == Some(1) {
                    ui.request_end_session(kind);
                    ui.close = true;
                } else if choice.is_some() {
                    ui.close = true;
                } else {
                    // Acorda na virada do próximo segundo.
                    let next = Duration::from_secs(elapsed + 1).saturating_sub(started.elapsed());
                    ui.wake_after(next.max(Duration::from_millis(20)));
                }
            }
            Mode::Cancelled { kind, app } => {
                let title = tr!("{} foi cancelado.", action_noun(kind));
                let text = tr!("“{}” não fechou. Ele pode estar esperando uma resposta sua, como salvar um documento. Resolva e tente de novo.", app);
                if ui.alert_window(AlertKind::Caution, &title, &text, &["OK"]).is_some() {
                    ui.close = true;
                }
            }
            Mode::Failed { kind } => {
                let title = match *kind {
                    "restart" => tr!("Não foi possível reiniciar o computador."),
                    _ => tr!("Não foi possível desligar o computador."),
                };
                let text = tr!("O sistema recusou o pedido. Talvez haja outra sessão aberta ou falte permissão.");
                if ui.alert_window(AlertKind::Stop, title, text, &["OK"]).is_some() {
                    ui.close = true;
                }
            }
        }
    }
}

/// A ação pedida na linha de comando (`shutdown`, `restart`, `logout` ou `quit`).
fn kind_arg(s: Option<String>) -> Option<&'static str> {
    let s = s?;
    KINDS.into_iter().find(|k| *k == s)
}

pub fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let mode = match first.as_deref() {
        Some("cancelled") => kind_arg(args.next()).map(|kind| Mode::Cancelled { kind, app: args.next().unwrap_or_else(|| tr!("Um aplicativo").into()) }),
        Some("failed") => kind_arg(args.next()).map(|kind| Mode::Failed { kind }),
        _ => kind_arg(first).map(|kind| Mode::Confirm { kind, started: Instant::now() }),
    };
    let Some(mode) = mode else {
        eprintln!("usage: glove-logout shutdown|restart|logout|quit\n       glove-logout cancelled <action> <app>\n       glove-logout failed <action>");
        std::process::exit(2);
    };
    let title = match &mode {
        Mode::Confirm { kind, .. } => confirm_texts(kind).0,
        Mode::Cancelled { kind, .. } | Mode::Failed { kind } => confirm_texts(kind).0,
    };
    let opts = WindowOptions { title: title.into(), app_id: "glove.Logout".into(), size: (440, 130), fixed: true, min_size: None };
    if let Err(e) = run(opts, Sair { mode }) {
        eprintln!("glove-logout: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acoes_da_linha_de_comando() {
        for k in KINDS {
            assert_eq!(kind_arg(Some(k.to_string())), Some(k));
        }
        // Os nomes de antes, em português, não valem mais.
        assert_eq!(kind_arg(Some("desligar".into())), None);
        assert_eq!(kind_arg(None), None);
    }

    #[test]
    fn contagem() {
        assert_eq!(countdown_text("shutdown", 120), tr!("Se você não fizer nada, {} automaticamente em {}.", tr!("o computador será desligado"), tr!("2 minutos")));
        assert_eq!(countdown_text("logout", 1), tr!("Se você não fizer nada, {} automaticamente em {}.", tr!("a sessão será encerrada"), tr!("1 segundo")));
        assert!(countdown_text("restart", 37).contains(&tr!("{} segundos", 37)));
    }

    #[test]
    fn argumentos() {
        assert_eq!(kind_arg(Some("quit".into())), Some("quit"));
        assert_eq!(kind_arg(Some("x".into())), None);
    }
}
