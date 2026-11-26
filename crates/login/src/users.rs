//! Usuários que podem entrar: contas comuns do /etc/passwd (UID entre UID_MIN e
//! UID_MAX do /etc/login.defs) com um shell de verdade. A foto vem do
//! AccountsService (legível por todos), se houver.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub login: String,
    /// Nome completo (antes da 1ª vírgula do GECOS) ou o login.
    pub name: String,
    pub photo: Option<PathBuf>,
    pub uid: u32,
}

fn uid_range(defs: &str) -> (u32, u32) {
    let get = |key: &str, def: u32| {
        defs.lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .find_map(|l| {
                let mut it = l.split_whitespace();
                (it.next() == Some(key)).then(|| it.next().and_then(|v| v.parse().ok())).flatten()
            })
            .unwrap_or(def)
    };
    (get("UID_MIN", 1000), get("UID_MAX", 60000))
}

/// Usuários a partir do conteúdo de /etc/passwd e /etc/login.defs.
pub fn parse(passwd: &str, defs: &str) -> Vec<User> {
    let (min, max) = uid_range(defs);
    let mut out: Vec<User> = passwd
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            if f.len() < 7 {
                return None;
            }
            let uid: u32 = f[2].parse().ok()?;
            let shell = f[6];
            if uid < min || uid > max || shell.ends_with("nologin") || shell.ends_with("false") || shell.is_empty() {
                return None;
            }
            let full = f[4].split(',').next().unwrap_or("").trim();
            Some(User { login: f[0].to_string(), name: if full.is_empty() { f[0].to_string() } else { full.to_string() }, photo: None, uid })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// The file with the login of the last one who entered, in the cache folder of the login
/// screen's user (the background shows that one's wallpaper).
const LAST: &str = "glove-login-user";

pub fn last() -> Option<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    std::fs::read_to_string(home.join(".cache").join(LAST)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn remember(login: &str) {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return };
    let dir = home.join(".cache");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(LAST), format!("{login}\n"));
}

pub fn list() -> Vec<User> {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let mut users = parse(&read("/etc/passwd"), &read("/etc/login.defs"));
    for u in &mut users {
        let p = PathBuf::from("/var/lib/AccountsService/icons").join(&u.login);
        if p.is_file() {
            u.photo = Some(p);
        }
    }
    users
}

#[cfg(test)]
mod tests {
    #[test]
    fn contas() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nana:x:1000:1000:Ana Souza,,,:/home/ana:/bin/bash\nsvc:x:1001:1001::/:/usr/sbin/nologin\nbeto:x:1002:1002::/home/beto:/bin/zsh\nfora:x:70000:70000::/:/bin/sh\n";
        let u = super::parse(passwd, "UID_MIN 1000\nUID_MAX 60000\n");
        assert_eq!(u.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), ["Ana Souza", "beto"]);
    }
}
