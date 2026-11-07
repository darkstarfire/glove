//! De onde vêm os apps: o apt (pacotes do sistema) e o Flatpak (Flathub).
//! Tudo pelas ferramentas de linha de comando, em threads; a janela só lê.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origem {
    Apt,
    Flatpak,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pacote {
    pub origem: Origem,
    /// Nome do pacote (apt) ou id do app (Flatpak).
    pub id: String,
    pub nome: String,
    pub resumo: String,
    pub versao: String,
    pub instalado: bool,
    /// Remoto do Flatpak (flathub…).
    pub remoto: String,
}

pub fn has(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

fn output(args: &[&str]) -> String {
    Command::new(args[0]).args(&args[1..]).env("LC_ALL", "C").stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// Pacotes do apt já instalados.
pub fn apt_installed() -> HashSet<String> {
    output(&["dpkg-query", "-W", "-f=${Package}\t${Status}\n"]).lines().filter(|l| l.ends_with("installed") && !l.contains("not-installed") && !l.contains("config-files")).filter_map(|l| l.split('\t').next()).map(|p| p.split(':').next().unwrap_or(p).to_string()).collect()
}

pub fn apt_search(q: &str, installed: &HashSet<String>) -> Vec<Pacote> {
    let mut v: Vec<Pacote> = output(&["apt-cache", "search", "--", q])
        .lines()
        .filter_map(|l| l.split_once(" - "))
        .map(|(id, resumo)| Pacote { origem: Origem::Apt, id: id.to_string(), nome: id.to_string(), resumo: resumo.to_string(), versao: String::new(), instalado: installed.contains(id), remoto: String::new() })
        .collect();
    // O nome que começa com a busca primeiro; bibliotecas e símbolos de depuração por último.
    let ql = q.to_lowercase();
    let peso = |p: &Pacote| {
        let n = p.id.to_lowercase();
        let lixo = n.starts_with("lib") || n.ends_with("-dev") || n.ends_with("-dbg") || n.ends_with("-doc") || n.ends_with("-dbgsym");
        (lixo, !n.starts_with(&ql), !n.contains(&ql), n.len())
    };
    v.sort_by_key(|p| peso(p));
    v.truncate(150);
    v
}

pub fn flatpak_installed() -> Vec<Pacote> {
    output(&["flatpak", "list", "--app", "--columns=application,name,version,origin"])
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 2).then(|| Pacote { origem: Origem::Flatpak, id: c[0].into(), nome: c[1].into(), resumo: String::new(), versao: c.get(2).unwrap_or(&"").to_string(), instalado: true, remoto: c.get(3).unwrap_or(&"").to_string() })
        })
        .collect()
}

pub fn flatpak_search(q: &str, installed: &HashSet<String>) -> Vec<Pacote> {
    output(&["flatpak", "search", "--columns=application,name,description,version,remotes", q])
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 3 && c[0].contains('.')).then(|| Pacote {
                origem: Origem::Flatpak,
                id: c[0].into(),
                nome: c[1].into(),
                resumo: c[2].into(),
                versao: c.get(3).unwrap_or(&"").to_string(),
                instalado: installed.contains(c[0]),
                remoto: c.get(4).unwrap_or(&"flathub").split(',').next().unwrap_or("flathub").to_string(),
            })
        })
        .take(100)
        .collect()
}

/// Há algum remoto do Flatpak configurado (Flathub)?
pub fn flatpak_remotes() -> bool {
    !output(&["flatpak", "remotes", "--columns=name"]).trim().is_empty()
}

/// Apps com janela instalados pelo apt: os donos dos `.desktop` do sistema.
pub fn apt_apps() -> Vec<Pacote> {
    let dir = Path::new("/usr/share/applications");
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: HashMap<String, (String, String)> = HashMap::new();
    let mut files = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "desktop") {
            continue;
        }
        let t = std::fs::read_to_string(&p).unwrap_or_default();
        if t.lines().any(|l| l.trim() == "NoDisplay=true") {
            continue;
        }
        let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(k)).unwrap_or("").to_string();
        names.insert(p.to_string_lossy().into_owned(), (get("Name="), get("Comment=")));
        files.push(p.to_string_lossy().into_owned());
    }
    if files.is_empty() {
        return Vec::new();
    }
    let mut args = vec!["dpkg", "-S"];
    args.extend(files.iter().map(String::as_str));
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for l in output(&args).lines() {
        let Some((pkgs, path)) = l.split_once(": ") else { continue };
        let pkg = pkgs.split(',').next().unwrap_or(pkgs).split(':').next().unwrap_or(pkgs).trim().to_string();
        if !seen.insert(pkg.clone()) {
            continue;
        }
        let (nome, resumo) = names.get(path.trim()).cloned().unwrap_or_default();
        v.push(Pacote { origem: Origem::Apt, id: pkg.clone(), nome: if nome.is_empty() { pkg } else { nome }, resumo, versao: String::new(), instalado: true, remoto: String::new() });
    }
    v
}

/// Atualizações disponíveis (apt e Flatpak).
pub fn updates() -> Vec<Pacote> {
    let mut v: Vec<Pacote> = Vec::new();
    if has("apt") {
        for l in output(&["apt", "list", "--upgradable"]).lines() {
            if let Some((id, rest)) = l.split_once('/') {
                let versao = rest.split_whitespace().nth(1).unwrap_or("").to_string();
                v.push(Pacote { origem: Origem::Apt, id: id.into(), nome: id.into(), resumo: String::new(), versao, instalado: true, remoto: String::new() });
            }
        }
    }
    if has("flatpak") {
        for l in output(&["flatpak", "remote-ls", "--updates", "--app", "--columns=application,name,version"]).lines() {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() >= 2 {
                v.push(Pacote { origem: Origem::Flatpak, id: c[0].into(), nome: c[1].into(), resumo: String::new(), versao: c.get(2).unwrap_or(&"").to_string(), instalado: true, remoto: String::new() });
            }
        }
    }
    v
}

/// Trabalho em andamento: a saída das ferramentas e o resultado.
#[derive(Default)]
pub struct Trabalho {
    pub linhas: Mutex<Vec<String>>,
    pub fim: Mutex<Option<Result<(), String>>>,
}

impl Trabalho {
    fn push(&self, l: &str) {
        let mut v = self.linhas.lock().unwrap();
        v.push(l.trim_end().to_string());
        if v.len() > 300 {
            v.drain(..100);
        }
    }

    pub fn ultima(&self) -> String {
        self.linhas.lock().unwrap().iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_default()
    }
}

/// Um passo: o comando e se precisa de administrador (sudo).
pub struct Passo {
    pub args: Vec<String>,
    pub admin: bool,
}

pub fn passo(args: &[&str], admin: bool) -> Passo {
    Passo { args: args.iter().map(|s| s.to_string()).collect(), admin }
}

/// Confere a senha de administrador (sudo -v).
pub fn senha_ok(senha: &str) -> bool {
    let Ok(mut c) = Command::new("sudo").args(["-S", "-k", "-v", "-p", ""]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
    if let Some(mut i) = c.stdin.take() {
        let _ = writeln!(i, "{senha}");
    }
    c.wait().is_ok_and(|s| s.success())
}

/// Roda os passos numa thread. A senha (se houver) vai só pela entrada do sudo.
pub fn rodar(passos: Vec<Passo>, senha: Option<String>) -> Arc<Trabalho> {
    let t = Arc::new(Trabalho::default());
    let tt = t.clone();
    std::thread::spawn(move || {
        let mut r = Ok(());
        for p in passos {
            let mut cmd = if p.admin {
                let mut c = Command::new("sudo");
                c.args(["-S", "-p", ""]).args(&p.args);
                c
            } else {
                let mut c = Command::new(&p.args[0]);
                c.args(&p.args[1..]);
                c
            };
            cmd.env("DEBIAN_FRONTEND", "noninteractive").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let Ok(mut child) = cmd.spawn() else {
                r = Err(format!("{}: não encontrado", p.args[0]));
                break;
            };
            if let (true, Some(s), Some(mut i)) = (p.admin, &senha, child.stdin.take()) {
                let _ = writeln!(i, "{s}");
            }
            let err = child.stderr.take();
            let t2 = tt.clone();
            let errs = std::thread::spawn(move || {
                let mut last = String::new();
                if let Some(e) = err {
                    for l in BufReader::new(e).lines().map_while(Result::ok) {
                        t2.push(&l);
                        last = l;
                    }
                }
                last
            });
            if let Some(o) = child.stdout.take() {
                for l in BufReader::new(o).lines().map_while(Result::ok) {
                    tt.push(&l);
                }
            }
            let last_err = errs.join().unwrap_or_default();
            match child.wait() {
                Ok(s) if s.success() => {}
                _ => {
                    r = Err(if last_err.is_empty() { tt.ultima() } else { last_err });
                    break;
                }
            }
        }
        *tt.fim.lock().unwrap() = Some(r);
    });
    t
}

#[cfg(test)]
mod tests {
    #[test]
    fn caminho() {
        assert!(super::has("sh"));
        assert!(!super::has("nao-existe-esse-programa-glove"));
    }
}
