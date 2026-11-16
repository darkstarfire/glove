//! Atualizações do Glove pelo repositório apt do Glove OS, que a CI monta a cada
//! versão (GitHub Pages, assinado): o Glove anda no próprio ritmo, sem esperar o
//! Debian. O Debian continua vindo do Debian; aqui só o `glove` e o `glove-os`.
//!
//!   glove-prefs --verificar-atualizacao   fica de fundo na sessão: confere de tempos
//!                                         em tempos o Glove e o resto do sistema e
//!                                         avisa (balão e central de avisos da barra
//!                                         de menu); "Ver" abre o painel

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
    // O GitHub Pages guarda as páginas por até 10 minutos: um número que muda a cada
    // consulta faz ele mandar a versão de agora (logo depois de sair uma versão nova).
    let fresh = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let o = Command::new("curl").args(["-fsSL", "--max-time", "20", "-H", "Cache-Control: no-cache", &format!("{repo}/novidades.json?t={fresh}")]).output().map_err(|e| format!("curl: {e}"))?;
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

fn sistema_avisado_path() -> std::path::PathBuf {
    avisada_path().with_file_name("sistema-avisado")
}

/// Os pacotes do sistema com versão nova, pela lista que o apt já tem (sem rede e sem
/// senha: o próprio sistema a renova, pelo apt-daily do Debian, que o glove-os liga).
/// O Glove e o glove-os ficam de fora: eles têm o aviso deles, com as novidades.
pub fn sistema_pendentes() -> Vec<String> {
    let Ok(o) = Command::new("apt").args(["list", "--upgradable"]).env("LC_ALL", "C").output() else { return Vec::new() };
    pendentes_de(&String::from_utf8_lossy(&o.stdout))
}

/// Os nomes na saída do `apt list --upgradable` ("nome/suite versão arq [upgradable from: …]").
fn pendentes_de(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter(|l| l.contains("[upgradable from"))
        .filter_map(|l| l.split('/').next())
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty() && n != "glove" && n != "glove-os")
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Um aviso com "Ver", que abre o painel Atualização de Software. O aviso continua na
/// central de avisos (o balãozinho da barra de menu) depois que o balão some, e clicar
/// nele lá também abre o painel; por isso a espera fica numa linha à parte.
fn avisar(titulo: String, texto: String) {
    std::thread::spawn(move || {
        let out = Command::new("notify-send")
            .args(["-a", tr!("Atualização de Software"), "-i", "system-software-update", "-A", &format!("default={}", tr!("Ver")), "-A", &format!("ver={}", tr!("Ver")), &titulo, &texto])
            .output();
        let escolha = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        if escolha == "ver" || escolha == "default" {
            let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-prefs"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-prefs".into());
            let _ = Command::new(exe).arg("atualizacao").spawn();
        }
    });
}

/// O verificador de fundo: 2 minutos depois de entrar e depois de hora em hora. O
/// Glove vem do repositório dele (a cada 6 horas, avisa uma vez por versão); o resto do
/// sistema, da lista do apt (avisa quando aparece algo que ainda não foi avisado).
pub fn verificar_em_segundo_plano() {
    std::thread::sleep(Duration::from_secs(120));
    let mut volta = 0u32;
    loop {
        if volta % 6 == 0 {
            if let Ok(n) = buscar() {
                let avisada = std::fs::read_to_string(avisada_path()).unwrap_or_default();
                if mais_nova(&n.versao, &instalada()) && avisada.trim() != n.versao {
                    if let Some(d) = avisada_path().parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::write(avisada_path(), &n.versao);
                    let resumo = n.notas.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                    avisar(tr!("Glove OS {} disponível", n.versao), resumo);
                }
            }
        }
        let pend = sistema_pendentes();
        let ja: Vec<String> = std::fs::read_to_string(sistema_avisado_path()).unwrap_or_default().lines().map(String::from).collect();
        if pend.iter().any(|p| !ja.contains(p)) {
            if let Some(d) = sistema_avisado_path().parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(sistema_avisado_path(), pend.join("\n"));
            let titulo = if pend.len() == 1 { tr!("1 atualização do sistema disponível").to_string() } else { tr!("{} atualizações do sistema disponíveis", pend.len()) };
            let mut texto = pend.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
            if pend.len() > 4 {
                texto.push_str(&tr!(" e mais {}", pend.len() - 4));
            }
            avisar(titulo, texto);
        }
        volta += 1;
        std::thread::sleep(Duration::from_secs(3600));
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

    #[test]
    fn pendentes_do_apt() {
        let out = "Listing... Done\nlibc6/stable-security 2.41-12+deb13u1 amd64 [upgradable from: 2.41-12]\nglove/glove 0.12.62 amd64 [upgradable from: 0.12.61]\nfirefox-esr/stable-security 140.5 amd64 [upgradable from: 140.4]\n";
        assert_eq!(pendentes_de(out), vec!["firefox-esr".to_string(), "libc6".to_string()]);
        assert!(pendentes_de("Listing... Done\n").is_empty());
    }
}
