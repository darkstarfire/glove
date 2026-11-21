//! "Usar a aparência em todo o sistema" (Aparência): a abertura e o desligamento do
//! sistema (o tema do Plymouth) e a tela de login (/etc/glove/login) passam a usar a
//! aparência escolhida. A sessão (abertura, bloqueio e saída) o Glove já segue sozinho
//! pela configuração; estas duas são do sistema e pedem a senha de administrador.
//!
//! O trabalho de administrador é o próprio glove-prefs: `glove-prefs --tema-sistema TEMA`
//! (rodado pelo sudo) grava o tema da tela de login e troca o tema do Plymouth.

use glove_config::tr;
use glove_ui::Ui;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// A configuração da tela de login (a do `glove --greeter`).
const LOGIN: &str = "/etc/glove/login/config.toml";
/// Os temas do Plymouth do Glove.
const PLYMOUTH: &str = "/usr/share/plymouth/themes";

/// O tema do Plymouth de cada aparência (os clássicos usam o "glove").
pub fn plymouth_de(tema: &str) -> &'static str {
    match tema {
        "moderno" => "glove-moderno",
        "moderno-escuro" => "glove-moderno-escuro",
        _ => "glove",
    }
}

/// Troca a linha `theme = …` do começo de um config.toml (ou a põe no começo).
pub fn trocar_tema(texto: &str, tema: &str) -> String {
    let nova = format!("theme = \"{tema}\"");
    let mut feito = false;
    let mut out: Vec<String> = Vec::new();
    let mut secao = false;
    for l in texto.lines() {
        if l.trim_start().starts_with('[') {
            secao = true;
        }
        if !secao && !feito && l.trim_start().starts_with("theme") && l.contains('=') {
            out.push(nova.clone());
            feito = true;
        } else {
            out.push(l.to_string());
        }
    }
    if !feito {
        out.insert(0, nova);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Como administrador (`glove-prefs --tema-sistema TEMA`): a tela de login e o Plymouth.
pub fn aplicar_como_admin(tema: &str) -> Result<(), String> {
    let mut feito = Vec::new();
    if let Ok(t) = std::fs::read_to_string(LOGIN) {
        std::fs::write(LOGIN, trocar_tema(&t, tema)).map_err(|e| format!("{LOGIN}: {e}"))?;
        feito.push("login");
    }
    // O Plymouth só troca se o tema atual já é um do Glove (não mexe no de outro sistema).
    let nome = plymouth_de(tema);
    let arquivo = format!("{PLYMOUTH}/{nome}/{nome}.plymouth");
    if Path::new(&arquivo).exists() {
        let atual = plymouth_atual();
        if atual.starts_with("glove") && atual != nome {
            if Command::new("plymouth-set-default-theme").arg("--help").output().is_ok() {
                // Debian, Fedora e cia.: troca e refaz o initramfs (-R).
                let ok = Command::new("plymouth-set-default-theme").args(["-R", nome]).status().map_err(|e| e.to_string())?;
                if !ok.success() {
                    return Err(tr!("Não deu para trocar o tema de abertura do sistema.").into());
                }
            } else {
                let _ = Command::new("update-alternatives").args(["--set", "default.plymouth", &arquivo]).status();
                let _ = Command::new("update-initramfs").arg("-u").status();
            }
            feito.push("plymouth");
        }
    }
    eprintln!("glove-prefs: tema do sistema {tema}: {}", if feito.is_empty() { "nada a fazer".to_string() } else { feito.join(", ") });
    Ok(())
}

/// O tema do Plymouth em uso (o nome da pasta), ou vazio.
fn plymouth_atual() -> String {
    if let Ok(o) = Command::new("plymouth-set-default-theme").output() {
        let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !t.is_empty() {
            return t;
        }
    }
    std::fs::read_link(format!("{PLYMOUTH}/default.plymouth"))
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// O que há para mudar no sistema (sem nada disso, nem pede a senha).
pub fn ha_o_que_mudar() -> bool {
    Path::new(LOGIN).exists() || plymouth_atual().starts_with("glove")
}

/// O pedido de senha e o trabalho em andamento.
#[derive(Default)]
pub struct TemaSistema {
    pub prompt: Option<(Vec<String>, crate::admin::Prompt)>,
    job: Option<Receiver<Result<(), String>>>,
    pub msg: Option<String>,
}

impl TemaSistema {
    /// Leva a aparência `tema` para a tela de login e a abertura do sistema.
    pub fn aplicar(&mut self, tema: &str) {
        self.msg = None;
        if !ha_o_que_mudar() {
            return;
        }
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "glove-prefs".into());
        let args = vec![exe, "--tema-sistema".into(), tema.to_string()];
        match crate::admin::sudo_state() {
            crate::admin::Sudo::Free => self.job = Some(crate::admin::run(args, None)),
            crate::admin::Sudo::No => self.msg = Some(tr!("A tela de login e a abertura do sistema só mudam com uma conta de administrador.").into()),
            crate::admin::Sudo::Password => self.prompt = Some((args, crate::admin::Prompt::new(tr!("Usar a aparência na tela de login e na abertura do sistema.").into()))),
        }
    }

    pub fn ocupado(&self) -> bool {
        self.job.is_some()
    }

    /// A janela de senha (ou a folha) e o fim do trabalho.
    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        if let Some((args, mut p)) = self.prompt.take() {
            match p.sheet(ui) {
                crate::admin::Answer::Waiting => self.prompt = Some((args, p)),
                crate::admin::Answer::Cancel => {}
                crate::admin::Answer::Password(pw) => self.job = Some(crate::admin::run(args, Some(pw))),
            }
        }
    }

    /// Confere se o trabalho terminou (chamado pelo painel a cada quadro).
    pub fn andamento(&mut self, ui: &mut Ui) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(Ok(())) => {
                self.msg = Some(tr!("Pronto: a tela de login e a abertura do sistema seguem a aparência.").into());
                self.job = None;
            }
            Ok(Err(e)) => {
                self.msg = Some(tr!("Não deu certo: {}", e));
                self.job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(250)),
            Err(_) => self.job = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn troca_o_tema_do_login() {
        let t = "theme = \"claro\"\nwallpaper = \"x\"\n\n[appearance]\ntheme_extra = 1\n";
        assert_eq!(trocar_tema(t, "moderno"), "theme = \"moderno\"\nwallpaper = \"x\"\n\n[appearance]\ntheme_extra = 1\n");
        assert_eq!(trocar_tema("wallpaper = \"x\"\n", "moderno-escuro"), "theme = \"moderno-escuro\"\nwallpaper = \"x\"\n");
        assert_eq!(plymouth_de("moderno"), "glove-moderno");
        assert_eq!(plymouth_de("metal"), "glove");
    }
}
