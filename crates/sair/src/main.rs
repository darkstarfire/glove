//! Alertas do fim de sessão (abertos pelo Glove):
//!
//! - `glove-sair desligar|reiniciar|encerrar|sair`: confirma, com contagem de 120 s
//!   (sem resposta, a ação acontece sozinha), e pede ao Glove para executar.
//! - `glove-sair cancelado <ação> <app>`: um app não fechou e o encerramento parou.
//! - `glove-sair falhou <ação>`: o sistema recusou desligar ou reiniciar.

use std::time::{Duration, Instant};

use glove_kit::{run, ui::AlertKind, App, Ui, WindowOptions};

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

const KINDS: [&str; 4] = ["desligar", "reiniciar", "encerrar", "sair"];

/// (título da janela, pergunta, botão de confirmar).
fn confirm_texts(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "desligar" => ("Desligar", "Tem certeza de que deseja desligar o computador agora?", "Desligar"),
        "reiniciar" => ("Reiniciar", "Tem certeza de que deseja reiniciar o computador agora?", "Reiniciar"),
        "encerrar" => ("Encerrar Sessão", "Tem certeza de que deseja encerrar a sessão e sair de todos os aplicativos?", "Encerrar Sessão"),
        _ => ("Sair do Glove", "Tem certeza de que deseja sair do Glove?", "Sair"),
    }
}

/// Aviso da contagem: "…será desligado automaticamente em 2 minutos."
fn countdown_text(kind: &str, left: u64) -> String {
    let what = match kind {
        "desligar" => "o computador será desligado",
        "reiniciar" => "o computador será reiniciado",
        "encerrar" => "a sessão será encerrada",
        _ => "o Glove será fechado",
    };
    let when = match left {
        120 => "2 minutos".to_string(),
        1 => "1 segundo".to_string(),
        s => format!("{s} segundos"),
    };
    format!("Se você não fizer nada, {what} automaticamente em {when}.")
}

/// Nome da ação para frases: "o desligamento", "a reinicialização"…
fn action_noun(kind: &str) -> &'static str {
    match kind {
        "desligar" => "O desligamento",
        "reiniciar" => "A reinicialização",
        "encerrar" => "O encerramento da sessão",
        _ => "A saída do Glove",
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
                let choice = ui.alert_window(AlertKind::Note, q, &countdown_text(kind, left.max(1)), &["Cancelar", ok]);
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
                let title = format!("{} foi cancelado.", action_noun(kind));
                let text = format!("“{app}” não fechou. Ele pode estar esperando uma resposta sua, como salvar um documento. Resolva e tente de novo.");
                if ui.alert_window(AlertKind::Caution, &title, &text, &["OK"]).is_some() {
                    ui.close = true;
                }
            }
            Mode::Failed { kind } => {
                let title = match *kind {
                    "reiniciar" => "Não foi possível reiniciar o computador.",
                    _ => "Não foi possível desligar o computador.",
                };
                let text = "O sistema recusou o pedido. Talvez haja outra sessão aberta ou falte permissão.";
                if ui.alert_window(AlertKind::Stop, title, text, &["OK"]).is_some() {
                    ui.close = true;
                }
            }
        }
    }
}

fn kind_arg(s: Option<String>) -> Option<&'static str> {
    let s = s?;
    KINDS.into_iter().find(|k| *k == s)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let mode = match first.as_deref() {
        Some("cancelado") => kind_arg(args.next()).map(|kind| Mode::Cancelled { kind, app: args.next().unwrap_or_else(|| "Um aplicativo".into()) }),
        Some("falhou") => kind_arg(args.next()).map(|kind| Mode::Failed { kind }),
        _ => kind_arg(first).map(|kind| Mode::Confirm { kind, started: Instant::now() }),
    };
    let Some(mode) = mode else {
        eprintln!("uso: glove-sair desligar|reiniciar|encerrar|sair\n      glove-sair cancelado <ação> <app>\n      glove-sair falhou <ação>");
        std::process::exit(2);
    };
    let title = match &mode {
        Mode::Confirm { kind, .. } => confirm_texts(kind).0,
        Mode::Cancelled { kind, .. } | Mode::Failed { kind } => confirm_texts(kind).0,
    };
    let opts = WindowOptions { title: title.into(), app_id: "glove.Sair".into(), size: (440, 130), fixed: true, min_size: None };
    if let Err(e) = run(opts, Sair { mode }) {
        eprintln!("glove-sair: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contagem() {
        assert_eq!(countdown_text("desligar", 120), "Se você não fizer nada, o computador será desligado automaticamente em 2 minutos.");
        assert_eq!(countdown_text("encerrar", 1), "Se você não fizer nada, a sessão será encerrada automaticamente em 1 segundo.");
        assert!(countdown_text("reiniciar", 37).ends_with("em 37 segundos."));
    }

    #[test]
    fn argumentos() {
        assert_eq!(kind_arg(Some("sair".into())), Some("sair"));
        assert_eq!(kind_arg(Some("x".into())), None);
    }
}
