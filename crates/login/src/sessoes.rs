//! Ambientes gráficos instalados (os .desktop de wayland-sessions e xsessions),
//! para escolher na tela de login. O Glove vem primeiro; a última escolha fica
//! guardada na pasta do usuário da tela de login.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// Nome do arquivo sem ".desktop" (ex.: "glove", "zorin-xorg").
    pub id: String,
    pub name: String,
    /// Comando inteiro (vai para o `sh -c`).
    pub exec: String,
    pub x11: bool,
    pub desktop_names: String,
}

impl Session {
    /// Comando e variáveis para o greetd iniciar esta sessão. Sessões X11 sobem
    /// pelo `startx` (o greetd só inicia programas; quem abre o X é ele).
    pub fn command(&self) -> (Vec<String>, Vec<String>) {
        let mut env = vec![format!("XDG_SESSION_DESKTOP={}", self.id), format!("XDG_SESSION_TYPE={}", if self.x11 { "x11" } else { "wayland" })];
        if !self.desktop_names.is_empty() {
            env.push(format!("XDG_CURRENT_DESKTOP={}", self.desktop_names.trim_end_matches(';').replace(';', ":")));
        }
        let cmd = if self.x11 {
            vec!["startx".into(), "/bin/sh".into(), "-c".into(), self.exec.clone(), "--".into(), "-keeptty".into()]
        } else {
            vec!["/bin/sh".into(), "-c".into(), format!("exec {}", self.exec)]
        };
        (cmd, env)
    }
}

/// Lê um .desktop de sessão (None se escondido ou se o programa não existe).
pub fn parse(id: &str, text: &str, x11: bool) -> Option<Session> {
    let (mut name, mut name_pt, mut exec, mut try_exec, mut names) = (None, None, None, None, String::new());
    let mut in_entry = false;
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            match k.trim() {
                "Name" => name = Some(v.trim().to_string()),
                "Name[pt_BR]" | "Name[pt]" => name_pt = name_pt.or(Some(v.trim().to_string())),
                "Exec" => exec = Some(v.trim().to_string()),
                "TryExec" => try_exec = Some(v.trim().to_string()),
                "DesktopNames" => names = v.trim().to_string(),
                "Hidden" | "NoDisplay" if v.trim() == "true" => return None,
                _ => {}
            }
        }
    }
    if let Some(t) = try_exec {
        if !in_path(&t) {
            return None;
        }
    }
    // Tira os códigos de campo (%f, %U…), que não fazem sentido aqui.
    let exec: String = exec?.split_whitespace().filter(|w| !(w.len() == 2 && w.starts_with('%'))).collect::<Vec<_>>().join(" ");
    let mut name = name_pt.or(name)?;
    if x11 && !name.to_lowercase().contains("x") {
        name.push_str(" (X11)");
    }
    Some(Session { id: id.to_string(), name, exec, x11, desktop_names: names })
}

fn in_path(p: &str) -> bool {
    if p.contains('/') {
        return Path::new(p).is_file();
    }
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(p).is_file()))
}

/// As sessões instaladas, o Glove primeiro (com o comando de `glove_cmd`).
pub fn list(glove_cmd: &[String]) -> Vec<Session> {
    let mut out = vec![Session { id: "glove".into(), name: "Glove".into(), exec: glove_cmd.join(" "), x11: false, desktop_names: "Glove".into() }];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let has_x = in_path("startx");
    for base in data.split(':').filter(|d| !d.is_empty()) {
        for (sub, x11) in [("wayland-sessions", false), ("xsessions", true)] {
            if x11 && !has_x {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(Path::new(base).join(sub)) else { continue };
            let mut found: Vec<Session> = rd
                .flatten()
                .filter_map(|e| {
                    let f = e.file_name().to_string_lossy().into_owned();
                    let id = f.strip_suffix(".desktop")?.to_string();
                    let text = std::fs::read_to_string(e.path()).ok()?;
                    parse(&id, &text, x11)
                })
                .filter(|s| s.id != "glove" && !s.name.eq_ignore_ascii_case("glove"))
                .collect();
            found.sort_by_key(|s| s.name.to_lowercase());
            for s in found {
                if !out.iter().any(|o| o.id == s.id && o.x11 == s.x11) {
                    out.push(s);
                }
            }
        }
    }
    out
}

fn memory() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join(".cache/glove-login-sessao"))
}

/// A sessão escolhida da última vez (id e se é X11).
pub fn last() -> Option<(String, bool)> {
    let t = std::fs::read_to_string(memory()?).ok()?;
    let (id, kind) = t.trim().split_once(' ').unwrap_or((t.trim(), "wayland"));
    Some((id.to_string(), kind == "x11"))
}

pub fn remember(s: &Session) {
    if let Some(p) = memory() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, format!("{} {}\n", s.id, if s.x11 { "x11" } else { "wayland" }));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn le_sessao() {
        let t = "[Desktop Entry]\nName=Zorin Desktop on Xorg\nExec=env A=b /usr/bin/gnome-session --session=zorin %U\nDesktopNames=zorin;GNOME;\n";
        let s = super::parse("zorin-xorg", t, true).unwrap();
        assert_eq!(s.exec, "env A=b /usr/bin/gnome-session --session=zorin");
        let (cmd, env) = s.command();
        assert_eq!(cmd[0], "startx");
        assert!(env.contains(&"XDG_CURRENT_DESKTOP=zorin:GNOME".to_string()));
        assert!(super::parse("x", "[Desktop Entry]\nName=X\nExec=x\nNoDisplay=true\n", false).is_none());
    }
}
