//! Confirmação de Desligar, Reiniciar e Encerrar Sessão (aberta pelo menu principal).
//! Escolhida a ação, pede ao Glove para executá-la e fecha.

use glove_kit::{run, ui::AlertKind, App, Ui, WindowOptions};

struct Sair {
    kind: &'static str,
}

/// (título da janela, pergunta, texto, botão de confirmar).
fn texts(kind: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    Some(match kind {
        "desligar" => ("Desligar", "Tem certeza de que deseja desligar o computador agora?", "Os aplicativos abertos serão encerrados.", "Desligar"),
        "reiniciar" => ("Reiniciar", "Tem certeza de que deseja reiniciar o computador agora?", "Os aplicativos abertos serão encerrados.", "Reiniciar"),
        "encerrar" => ("Encerrar Sessão", "Tem certeza de que deseja encerrar a sessão e sair de todos os aplicativos?", "", "Encerrar Sessão"),
        "sair" => ("Sair do Glove", "Tem certeza de que deseja sair do Glove?", "As janelas abertas no Glove serão fechadas.", "Sair"),
        _ => return None,
    })
}

impl App for Sair {
    fn ui(&mut self, ui: &mut Ui) {
        let Some((_, q, text, ok)) = texts(self.kind) else { return };
        match ui.alert_window(AlertKind::Note, q, text, &["Cancelar", ok]) {
            Some(1) => {
                ui.request_end_session(self.kind);
                ui.close = true;
            }
            Some(_) => ui.close = true,
            None => {}
        }
    }
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let Some(kind) = ["desligar", "reiniciar", "encerrar", "sair"].into_iter().find(|k| *k == arg) else {
        eprintln!("uso: glove-sair desligar|reiniciar|encerrar|sair");
        std::process::exit(2);
    };
    let (title, ..) = texts(kind).unwrap();
    let opts = WindowOptions { title: title.into(), app_id: "glove.Sair".into(), size: (420, 120), fixed: true };
    if let Err(e) = run(opts, Sair { kind }) {
        eprintln!("glove-sair: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn textos() {
        for k in ["desligar", "reiniciar", "encerrar", "sair"] {
            assert!(super::texts(k).is_some());
        }
        assert!(super::texts("x").is_none());
    }
}
