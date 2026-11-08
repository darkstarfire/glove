//! glove-navegador: abre o navegador que cabe na máquina. Abaixo de 1 GB de
//! RAM, o NetSurf (leve); acima, o Firefox (com o perfil de sempre do usuário).
//! A escolha das Preferências (navegador = "...") vale antes de tudo.
//! O processo vira o navegador (exec), com os endereços passados adiante.

use glove_config::tr;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use glove_ui::{run, ui::AlertKind, App, Ui, WindowOptions};

const NETSURF: [&str; 3] = ["netsurf-gtk3", "netsurf-gtk", "netsurf"];
const OTHERS: [&str; 4] = ["chromium", "chromium-browser", "google-chrome", "epiphany"];

fn which(prog: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| PathBuf::from(d).join(prog)).find(|p| p.is_file())
}

/// Memória total em MB (do /proc/meminfo).
pub fn mem_total_mb(meminfo: &str) -> u64 {
    meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

/// O que abrir: (programa, é o Firefox do Glove).
pub fn choose(mem_mb: u64, preferred: &str, has: &dyn Fn(&str) -> bool) -> Option<(String, bool)> {
    if !preferred.is_empty() && has(preferred) {
        return Some((preferred.to_string(), preferred == "firefox"));
    }
    let netsurf = NETSURF.iter().find(|n| has(n)).map(|n| (n.to_string(), false));
    let firefox = has("firefox").then(|| ("firefox".to_string(), true));
    let other = || OTHERS.iter().find(|n| has(n)).map(|n| (n.to_string(), false));
    if mem_mb > 0 && mem_mb < 1024 {
        netsurf.or(firefox).or_else(other)
    } else {
        firefox.or(netsurf).or_else(other)
    }
}

struct NoBrowser;

impl App for NoBrowser {
    fn ui(&mut self, ui: &mut Ui) {
        let text = tr!("Instale o NetSurf (leve, bom para pouca memória) ou o Firefox pelo gerenciador de pacotes do sistema.");
        if ui.alert_window(AlertKind::Caution, tr!("Nenhum navegador instalado."), text, &["OK"]).is_some() {
            ui.close = true;
        }
    }
}

pub fn main() {
    let urls: Vec<String> = std::env::args().skip(1).collect();
    let mem = mem_total_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
    let preferred = glove_config::Config::load().navegador;
    let Some((prog, glove_firefox)) = choose(mem, &preferred, &|p| which(p).is_some()) else {
        let opts = WindowOptions { title: tr!("Navegador").into(), app_id: "glove.Navegador".into(), size: (440, 130), fixed: true, min_size: None };
        let _ = run(opts, NoBrowser);
        return;
    };
    // O Firefox abre com o perfil de sempre do usuário (favoritos, senhas, abas), igual
    // a abrir pelo Todos os Apps. (Antes ele abria num perfil separado do Glove.)
    let _ = glove_firefox;
    let mut cmd = Command::new(&prog);
    cmd.args(&urls);
    let err = cmd.exec();
    eprintln!("glove-navegador: não foi possível abrir {prog}: {err}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escolhe_pela_memoria() {
        let all = |p: &str| matches!(p, "firefox" | "netsurf-gtk3" | "chromium");
        assert_eq!(choose(512, "", &all), Some(("netsurf-gtk3".into(), false)));
        assert_eq!(choose(4096, "", &all), Some(("firefox".into(), true)));
        assert_eq!(choose(512, "chromium", &all), Some(("chromium".into(), false)));
        let only_firefox = |p: &str| p == "firefox";
        assert_eq!(choose(512, "", &only_firefox), Some(("firefox".into(), true)));
        assert_eq!(choose(512, "", &|_| false), None);
        assert_eq!(mem_total_mb("MemTotal:        8034516 kB\nMemFree: 1 kB\n"), 7846);
    }
}
