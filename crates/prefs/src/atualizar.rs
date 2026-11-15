//! Atualizações do Glove pelo repositório apt do Glove OS, que a CI monta a cada
//! versão (GitHub Pages, assinado): o Glove anda no próprio ritmo, sem esperar o
//! Debian. O Debian continua vindo do Debian; aqui só o `glove` e o `glove-os`.
//!
//!   glove-prefs --verificar-atualizacao   fica de fundo na sessão: confere de tempos
//!                                         em tempos e avisa (balão e contador da
//!                                         barra de menu); "Ver" abre o painel

use glove_config::tr;
use std::process::Command;
use std::time::Duration;

/// O repositório (GitHub Pages do projeto).
pub const REPO: &str = "https://fabioaacarneiro.github.io/glove";
/// Onde o pacote glove-os põe o repositório (ou o painel, fora do Glove OS).
pub const SOURCES: &str = "/etc/apt/sources.list.d/glove-os.sources";
const KEYRING: &str = "/usr/share/keyrings/glove-os-repositorio.gpg";

/// A versão nova publicada e o que ela traz.
#[derive(Clone, Debug, PartialEq)]
pub struct Nova {
    pub versao: String,
    pub notas: String,
    pub data: String,
}

/// Lê o `novidades.json` do repositório.
pub fn buscar() -> Result<Nova, String> {
    // GLOVE_REPO troca o endereço (testes, ou um espelho).
    let repo = std::env::var("GLOVE_REPO").unwrap_or_else(|_| REPO.into());
    let o = Command::new("curl").args(["-fsSL", "--max-time", "20", &format!("{repo}/novidades.json")]).output().map_err(|e| format!("curl: {e}"))?;
    if !o.status.success() {
        return Err(tr!("Não deu para consultar o repositório do Glove (sem internet, ou o repositório ainda não foi publicado).").into());
    }
    parse(&String::from_utf8_lossy(&o.stdout)).ok_or_else(|| tr!("A resposta do repositório veio num formato inesperado.").into())
}

pub fn parse(json: &str) -> Option<Nova> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let versao = s("versao");
    (!versao.is_empty()).then(|| Nova { versao, notas: s("notas"), data: s("data") })
}

/// A versão do Glove instalada (a do pacote; sem pacote, a deste programa).
pub fn instalada() -> String {
    Command::new("dpkg-query")
        .args(["-W", "-f=${Version}", "glove"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// `a` é mais nova que `b`? (números separados por ponto: 0.12.50 > 0.12.9)
pub fn mais_nova(a: &str, b: &str) -> bool {
    let n = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    n(a) > n(b)
}

/// O repositório do Glove está configurado neste sistema?
pub fn configurado() -> bool {
    std::path::Path::new(SOURCES).exists()
}

/// Atualiza só o Glove: lê só o índice do repositório dele (rápido, e um
/// problema no Debian não atrapalha) e instala a versão nova.
pub fn comando_atualizar() -> Vec<String> {
    vec![
        "sh".into(),
        "-c".into(),
        format!("apt-get update -o Dir::Etc::sourcelist={SOURCES} -o Dir::Etc::sourceparts=- -o APT::Get::List-Cleanup=0 && DEBIAN_FRONTEND=noninteractive apt-get install -y --only-upgrade glove glove-os"),
    ]
}

/// Fora do Glove OS (Debian, Ubuntu…): acrescenta o repositório do Glove.
pub fn comando_repositorio() -> Vec<String> {
    vec![
        "sh".into(),
        "-c".into(),
        format!("curl -fsSL {REPO}/glove-os-repositorio.gpg -o {KEYRING} && printf 'Types: deb\\nURIs: {REPO}\\nSuites: glove\\nComponents: main\\nSigned-By: {KEYRING}\\n' > {SOURCES}"),
    ]
}

fn avisada_path() -> std::path::PathBuf {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    home.join(".cache/glove/atualizacao-avisada")
}

/// O verificador de fundo: 2 minutos depois de entrar e a cada 6 horas. Avisa uma
/// vez por versão; "Ver" abre o painel Atualização de Software.
pub fn verificar_em_segundo_plano() {
    std::thread::sleep(Duration::from_secs(120));
    loop {
        if let Ok(n) = buscar() {
            let avisada = std::fs::read_to_string(avisada_path()).unwrap_or_default();
            if mais_nova(&n.versao, &instalada()) && avisada.trim() != n.versao {
                if let Some(d) = avisada_path().parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(avisada_path(), &n.versao);
                let resumo = n.notas.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                let titulo = tr!("Glove OS {} disponível", n.versao);
                // O notify-send espera o clique e diz qual ação foi escolhida.
                let out = Command::new("notify-send")
                    .args(["-a", tr!("Atualização de Software"), "-i", "system-software-update", "-A", &format!("ver={}", tr!("Ver")), &titulo, &resumo])
                    .output();
                if out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "ver") {
                    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-prefs"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-prefs".into());
                    let _ = Command::new(exe).arg("atualizacao").spawn();
                }
            }
        }
        std::thread::sleep(Duration::from_secs(6 * 3600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versoes_e_novidades() {
        assert!(mais_nova("0.12.50", "0.12.9"));
        assert!(mais_nova("0.13.0", "0.12.99"));
        assert!(!mais_nova("0.12.49", "0.12.49"));
        assert!(!mais_nova("0.12.48", "0.12.49"));
        let n = parse(r#"{"versao":"0.12.50","data":"2026-11-15","notas":"Discos novo\n- formatar"}"#).unwrap();
        assert_eq!((n.versao.as_str(), n.notas.lines().count()), ("0.12.50", 2));
        assert_eq!(parse("{}"), None);
    }
}
