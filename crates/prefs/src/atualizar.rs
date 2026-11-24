//! Atualizações do Glove pelos repositórios que a CI monta a cada versão (GitHub
//! Pages, assinados): o Glove anda no próprio ritmo, sem esperar a distribuição, que
//! continua vindo dela mesma. Conforme o pacote instalado (`pacote`):
//!
//! - Debian, Ubuntu e o Glove OS: o repositório apt (só o `glove` e o `glove-os`);
//! - Fedora: o repositório dnf (`/etc/yum.repos.d/glove.repo`, índice assinado);
//! - Arch: o repositório do pacman (pacote assinado; a chave entra no chaveiro dele);
//! - Slackware: não há repositório; o pacote da versão vem da release e é conferido
//!   pelo `SHA256SUMS` dela;
//! - instalado pelo código: o painel diz como atualizar.
//!
//!   glove-prefs --check-updates   fica de fundo na sessão: confere de tempos
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
/// A chave do repositório (data/glove-os/glove-os-repositorio.asc): a impressão digital
/// e o arquivo, que vem no pacote do Glove (o dnf e o pacman conferem por ele, sem baixar).
pub const CHAVE: &str = "5F1980D7EC30400DC3DA51AC1909D833C5219461";
const CHAVE_ARQUIVO: &str = "/usr/share/glove/glove-os-repositorio.asc";
const REPO_DNF: &str = "/etc/yum.repos.d/glove.repo";
const PACMAN_CONF: &str = "/etc/pacman.conf";
/// Os arquivos de cada versão (o pacote do Slackware vem daqui).
const RELEASES: &str = "https://github.com/fabioaacarneiro/glove/releases/download";

/// O endereço do repositório; GLOVE_REPO troca (testes, ou um espelho).
pub fn repo_url() -> String {
    std::env::var("GLOVE_REPO").unwrap_or_else(|_| REPO.into())
}

/// Com que pacote o Glove está instalado: de onde vem a versão instalada e como atualizar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pacote {
    Apt,
    Dnf,
    Pacman,
    Slackware,
    /// Instalado pelo código (`glove-setup-tool.sh install`), ou de um jeito que não se conhece.
    Nenhum,
}

/// A saída de uma consulta rápida ao gerenciador de pacotes (None se falhou ou demorou).
fn consulta(cmd: &str, args: &[&str]) -> Option<String> {
    let o = glove_config::comando::output_within(Command::new(cmd).args(args), Duration::from_secs(5))?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn pacote() -> Pacote {
    static P: std::sync::OnceLock<Pacote> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        if consulta("dpkg-query", &["-W", "-f=${Status}", "glove"]).is_some_and(|s| dpkg_instalado(&s)) {
            Pacote::Apt
        } else if consulta("rpm", &["-q", "glove"]).is_some() {
            Pacote::Dnf
        } else if consulta("pacman", &["-Q", "glove"]).is_some() {
            Pacote::Pacman
        } else if slackware_instalada().is_some() {
            Pacote::Slackware
        } else {
            Pacote::Nenhum
        }
    })
}

/// O estado do dpkg diz que o pacote está instalado? ("install ok installed", ou
/// "hold ok installed" se ele foi segurado). Removido com a configuração guardada
/// ("deinstall ok config-files") não conta: o dpkg ainda dá a versão dele, e o painel
/// mostrava essa versão velha como a instalada.
pub fn dpkg_instalado(status: &str) -> bool {
    status.split_whitespace().nth(2) == Some("installed")
}

/// A versão num nome de pacote do Slackware ("glove-0.12.74-x86_64-1"); None se o
/// pacote é outro ("glove-extras-1.0-x86_64-1").
pub fn versao_slackware(nome: &str) -> Option<String> {
    let mut partes: Vec<&str> = nome.split('-').collect();
    if partes.len() != 4 || partes[0] != "glove" {
        return None;
    }
    Some(partes.swap_remove(1).to_string())
}

/// A versão do pacote do Glove instalado num Slackware (a lista de pacotes do pkgtools).
fn slackware_instalada() -> Option<String> {
    ["/var/lib/pkgtools/packages", "/var/log/packages"].iter().find_map(|d| std::fs::read_dir(d).ok()?.flatten().find_map(|e| versao_slackware(&e.file_name().to_string_lossy())))
}

/// A versão na resposta do `pacman -Q glove` ("glove 0.12.74-1").
pub fn versao_pacman(saida: &str) -> Option<String> {
    let v = saida.split_whitespace().nth(1)?;
    Some(v.rsplit_once('-').map_or(v, |(v, _)| v).to_string())
}

/// Só números e pontos: a versão vem da rede e vai para um comando de administrador.
pub fn versao_valida(v: &str) -> bool {
    !v.is_empty() && v.len() <= 24 && v.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// A versão nova publicada e o que ela traz.
#[derive(Clone, Debug, PartialEq)]
pub struct Nova {
    pub versao: String,
    pub notas: String,
    pub data: String,
}

/// Lê o `novidades.json` do repositório.
pub fn buscar() -> Result<Nova, String> {
    let repo = repo_url();
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

/// A versão do Glove instalada: a do pacote (logo depois de atualizar, este programa
/// ainda é o antigo); sem pacote, a deste programa.
pub fn instalada() -> String {
    let v = match pacote() {
        Pacote::Apt => consulta("dpkg-query", &["-W", "-f=${Version}", "glove"]),
        Pacote::Dnf => consulta("rpm", &["-q", "--qf", "%{VERSION}", "glove"]),
        Pacote::Pacman => consulta("pacman", &["-Q", "glove"]).and_then(|s| versao_pacman(&s)),
        Pacote::Slackware => slackware_instalada(),
        Pacote::Nenhum => None,
    };
    v.filter(|v| !v.is_empty()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// `a` é mais nova que `b`? (números separados por ponto: 0.12.50 > 0.12.9)
pub fn mais_nova(a: &str, b: &str) -> bool {
    let n = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    n(a) > n(b)
}

/// O repositório do Glove está configurado neste sistema? (No Slackware não há
/// repositório: o pacote vem da release, sem nada para configurar.)
/// A lista do que o `glove-setup-tool.sh instalar` copiou para /usr/local.
pub const MANIFESTO_LOCAL: &str = "/usr/local/share/glove/instalados.txt";

/// Há o pacote e também uma cópia antiga instalada pelo código em /usr/local: ela
/// vem antes (no PATH e na tela de login), e a sessão continua abrindo a antiga.
pub fn copia_local_sobrando() -> bool {
    pacote() != Pacote::Nenhum && std::path::Path::new(MANIFESTO_LOCAL).is_file()
}

pub fn configurado() -> bool {
    match pacote() {
        Pacote::Apt => std::path::Path::new(SOURCES).exists(),
        Pacote::Dnf => std::path::Path::new(REPO_DNF).exists(),
        Pacote::Pacman => std::fs::read_to_string(PACMAN_CONF).is_ok_and(|t| t.lines().any(|l| l.trim() == "[glove]")),
        Pacote::Slackware => true,
        Pacote::Nenhum => false,
    }
}

/// Acrescenta o repositório do Glove ao gerenciador de pacotes (como administrador).
pub fn comando_repositorio() -> Vec<String> {
    let repo = repo_url();
    let script = match pacote() {
        Pacote::Dnf => script_repo_dnf(&repo),
        Pacote::Pacman => script_repo_pacman(&repo),
        _ => format!("curl -fsSL {repo}/glove-os-repositorio.gpg -o {KEYRING} && printf 'Types: deb\\nURIs: {repo}\\nSuites: glove\\nComponents: main\\nSigned-By: {KEYRING}\\n' > {SOURCES}"),
    };
    vec!["sh".into(), "-c".into(), script]
}

/// Fedora: o arquivo do repositório. Os pacotes não são assinados um a um; o índice é
/// (`repo_gpgcheck`), e ele traz a soma de cada pacote. A chave é a que veio no pacote
/// do Glove; sem ela (instalação antiga), a do próprio repositório.
pub fn script_repo_dnf(repo: &str) -> String {
    format!(
        "set -e\nif [ -f {CHAVE_ARQUIVO} ]; then chave=file://{CHAVE_ARQUIVO}; else chave={repo}/glove-os-repositorio.asc; fi\n\
         printf '[glove]\\nname=Glove\\nbaseurl={repo}/fedora\\nenabled=1\\ngpgcheck=0\\nrepo_gpgcheck=1\\ngpgkey=%s\\nskip_if_unavailable=True\\nmetadata_expire=6h\\n' \"$chave\" > {REPO_DNF}\n"
    )
}

/// Arch: a chave entra no chaveiro do pacman, assinada localmente pela impressão
/// digital (só ela passa a valer), e o repositório vai para o fim do pacman.conf.
pub fn script_repo_pacman(repo: &str) -> String {
    format!(
        "set -e\nchave={CHAVE_ARQUIVO}\nif [ ! -f \"$chave\" ]; then chave=$(mktemp); curl -fsSL {repo}/glove-os-repositorio.asc -o \"$chave\"; fi\n\
         pacman-key --add \"$chave\"\npacman-key --lsign-key {CHAVE}\n\
         grep -q '^\\[glove\\]' {PACMAN_CONF} || printf '\\n[glove]\\nServer = {repo}/arch\\n' >> {PACMAN_CONF}\n"
    )
}

/// Atualiza o Glove para a versão `v` fora do apt (como administrador). None se a
/// versão não é só números e pontos, ou se não há o que rodar neste sistema.
/// - Fedora: o dnf, com os índices renovados (e a conferência de que a versão mudou).
/// - Arch: o pacote da versão direto do repositório do Glove, com a assinatura
///   conferida, sem renovar os índices do sistema (isso é do `pacman -Syu`).
/// - Slackware: o pacote da release, conferido pelo SHA256SUMS dela, e o upgradepkg.
pub fn script_atualizar(p: Pacote, v: &str, repo: &str) -> Option<String> {
    if !versao_valida(v) {
        return None;
    }
    match p {
        // O dnf sai com sucesso mesmo sem achar nada (repositório fora do ar é pulado):
        // confere se a versão instalada passou a ser a nova.
        Pacote::Dnf => Some(format!(
            "dnf -y upgrade --refresh glove && [ \"$(rpm -q --qf '%{{VERSION}}' glove)\" = {v} ] || {{ echo \"{}\" >&2; exit 1; }}",
            tr!("o repositório do Glove não trouxe a versão {}", v)
        )),
        Pacote::Pacman => Some(format!("pacman -U --noconfirm {repo}/arch/glove-{v}-1-$(uname -m).pkg.tar.zst")),
        Pacote::Slackware => Some(format!(
            "set -e\nf=glove-{v}-$(uname -m)-1.tgz\nd=$(mktemp -d)\ncd \"$d\"\n\
             baixar() {{ if command -v curl >/dev/null 2>&1 && curl -fsSL \"$1\" -o \"$2\"; then :; else wget -q -O \"$2\" \"$1\"; fi; }}\n\
             baixar {RELEASES}/v{v}/$f $f\nbaixar {RELEASES}/v{v}/SHA256SUMS SHA256SUMS\n\
             grep \" $f\\$\" SHA256SUMS | sha256sum -c -\n/sbin/upgradepkg --install-new $f\ncd /\nrm -rf \"$d\"\n"
        )),
        Pacote::Apt | Pacote::Nenhum => None,
    }
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
            let _ = Command::new(exe).arg("software-update").spawn();
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
    fn estado_do_dpkg() {
        assert!(dpkg_instalado("install ok installed"));
        assert!(dpkg_instalado("hold ok installed"));
        assert!(!dpkg_instalado("deinstall ok config-files"));
        assert!(!dpkg_instalado("unknown ok not-installed"));
        assert!(!dpkg_instalado("install ok half-configured"));
        assert!(!dpkg_instalado(""));
    }

    #[test]
    fn versao_do_pacote_instalado() {
        assert_eq!(versao_slackware("glove-0.12.74-x86_64-1").as_deref(), Some("0.12.74"));
        assert_eq!(versao_slackware("glove-extras-1.0-x86_64-1"), None);
        assert_eq!(versao_slackware("glibc-2.33-x86_64-9_slack15.0"), None);
        assert_eq!(versao_pacman("glove 0.12.74-1").as_deref(), Some("0.12.74"));
        assert_eq!(versao_pacman("glove"), None);
    }

    /// A versão vem da rede e vai para um comando de administrador: só números e pontos.
    #[test]
    fn versao_que_vai_para_o_comando() {
        assert!(versao_valida("0.12.75"));
        for v in ["", "0.12.75; rm -rf /", "0.12.$(id)", "0..1", "1.2-3", "v0.12.75", "0.12.75\n"] {
            assert!(!versao_valida(v), "{v:?}");
            assert_eq!(script_atualizar(Pacote::Slackware, v, REPO), None);
        }
    }

    #[test]
    fn comandos_por_distribuicao() {
        let dnf = script_atualizar(Pacote::Dnf, "0.12.75", REPO).unwrap();
        assert!(dnf.starts_with("dnf -y upgrade --refresh glove && [ \"$(rpm -q --qf '%{VERSION}' glove)\" = 0.12.75 ] || "), "{dnf}");
        let pac = script_atualizar(Pacote::Pacman, "0.12.75", REPO).unwrap();
        assert_eq!(pac, "pacman -U --noconfirm https://fabioaacarneiro.github.io/glove/arch/glove-0.12.75-1-$(uname -m).pkg.tar.zst");
        let sl = script_atualizar(Pacote::Slackware, "0.12.75", REPO).unwrap();
        assert!(sl.contains("https://github.com/fabioaacarneiro/glove/releases/download/v0.12.75/$f"), "{sl}");
        assert!(sl.contains("sha256sum -c -") && sl.contains("/sbin/upgradepkg --install-new $f"), "{sl}");
        // O apt tem o comando dele (com o andamento); pelo código não há o que rodar.
        assert_eq!(script_atualizar(Pacote::Apt, "0.12.75", REPO), None);
        assert_eq!(script_atualizar(Pacote::Nenhum, "0.12.75", REPO), None);
        let r = script_repo_dnf(REPO);
        assert!(r.contains("repo_gpgcheck=1") && r.contains("baseurl=https://fabioaacarneiro.github.io/glove/fedora") && r.contains("> /etc/yum.repos.d/glove.repo"), "{r}");
        let r = script_repo_pacman(REPO);
        assert!(r.contains(&format!("pacman-key --lsign-key {CHAVE}")) && r.contains("Server = https://fabioaacarneiro.github.io/glove/arch"), "{r}");
    }

    /// A impressão digital fixada é a da chave que vai no pacote e no repositório.
    #[test]
    fn impressao_da_chave() {
        let asc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/glove-os/glove-os-repositorio.asc");
        let Ok(o) = Command::new("gpg").args(["--show-keys", "--with-colons", asc]).output() else { return };
        if !o.status.success() {
            return;
        }
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(out.lines().any(|l| l.starts_with("fpr:") && l.contains(CHAVE)), "{out}");
    }

    /// Para os testes nos contêineres: imprime os comandos (com GLOVE_REPO, se houver).
    #[test]
    #[ignore]
    fn imprime_comandos() {
        let repo = repo_url();
        let v = std::env::var("GLOVE_TEST_VERSION").unwrap_or_else(|_| "0.12.74".into());
        println!("### repo-dnf\n{}### repo-pacman\n{}", script_repo_dnf(&repo), script_repo_pacman(&repo));
        for (n, p) in [("dnf", Pacote::Dnf), ("pacman", Pacote::Pacman), ("slackware", Pacote::Slackware)] {
            println!("### atualizar-{n}\n{}", script_atualizar(p, &v, &repo).unwrap());
        }
    }

    #[test]
    fn pendentes_do_apt() {
        let out = "Listing... Done\nlibc6/stable-security 2.41-12+deb13u1 amd64 [upgradable from: 2.41-12]\nglove/glove 0.12.62 amd64 [upgradable from: 0.12.61]\nfirefox-esr/stable-security 140.5 amd64 [upgradable from: 140.4]\n";
        assert_eq!(pendentes_de(out), vec!["firefox-esr".to_string(), "libc6".to_string()]);
        assert!(pendentes_de("Listing... Done\n").is_empty());
    }
}
