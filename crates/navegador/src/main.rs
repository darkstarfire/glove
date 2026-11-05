//! glove-navegador: abre o navegador que cabe na máquina. Abaixo de 1 GB de
//! RAM, o NetSurf (leve); acima, o Firefox num perfil do Glove com ajustes de
//! memória. A escolha das Preferências (navegador = "...") vale antes de tudo.
//! O processo vira o navegador (exec), com os endereços passados adiante.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use glove_kit::{run, ui::AlertKind, App, Ui, WindowOptions};

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

/// O Firefox é um snap (Ubuntu)? O snap não enxerga pastas ocultas da casa.
fn firefox_is_snap() -> bool {
    std::path::Path::new("/snap/bin/firefox").exists() || which("firefox").and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| t.contains("/snap/"))
}

/// Perfil do Firefox do Glove, com ajustes para gastar menos memória.
fn firefox_profile(mem_mb: u64) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let dir = if firefox_is_snap() {
        home.join("snap/firefox/common/glove-perfil")
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".cache")).join("glove/firefox")
    };
    let _ = std::fs::create_dir_all(&dir);
    let processes = if mem_mb < 2048 { 2 } else { 4 };
    let prefs = format!(
        "// Escrito pelo glove-navegador a cada abertura: ajustes de memória do Glove.\n\
         user_pref(\"dom.ipc.processCount\", {processes});\n\
         user_pref(\"browser.sessionhistory.max_total_viewers\", 0);\n\
         user_pref(\"browser.cache.memory.capacity\", 32768);\n\
         user_pref(\"browser.tabs.unloadOnLowMemory\", true);\n\
         user_pref(\"browser.aboutwelcome.enabled\", false);\n\
         user_pref(\"browser.startup.homepage_override.mstone\", \"ignore\");\n\
         user_pref(\"intl.locale.requested\", \"pt-BR\");\n"
    );
    let _ = std::fs::write(dir.join("user.js"), prefs);
    dir
}

struct NoBrowser;

impl App for NoBrowser {
    fn ui(&mut self, ui: &mut Ui) {
        let text = "Instale o NetSurf (leve, bom para pouca memória) ou o Firefox pelo gerenciador de pacotes do sistema.";
        if ui.alert_window(AlertKind::Caution, "Nenhum navegador instalado.", text, &["OK"]).is_some() {
            ui.close = true;
        }
    }
}

fn main() {
    let urls: Vec<String> = std::env::args().skip(1).collect();
    let mem = mem_total_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
    let preferred = glove_config::Config::load().navegador;
    let Some((prog, glove_firefox)) = choose(mem, &preferred, &|p| which(p).is_some()) else {
        let opts = WindowOptions { title: "Navegador".into(), app_id: "glove.Navegador".into(), size: (440, 130), fixed: true };
        let _ = run(opts, NoBrowser);
        return;
    };
    let mut cmd = Command::new(&prog);
    if glove_firefox {
        // Perfil próprio (não mexe no Firefox de outro desktop) e sem conversar com ele.
        cmd.arg("--no-remote").arg("--profile").arg(firefox_profile(mem));
    }
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
