//! Itens de Início (autostart do XDG): os .desktop de ~/.config/autostart e de
//! /etc/xdg/autostart que valem no Glove. O compositor abre os ativos ao
//! entrar; o painel Contas ▸ Itens de Início liga, desliga, põe e tira.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Nome do arquivo (ex.: "nm-applet.desktop").
    pub id: String,
    pub name: String,
    pub exec: String,
    pub enabled: bool,
    /// Vem do sistema (/etc/xdg/autostart): tirar só desliga.
    pub system: bool,
}

fn user_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))).unwrap_or_default();
    base.join("autostart")
}

fn system_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".into());
    dirs.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("autostart")).collect()
}

/// Campos do grupo [Desktop Entry].
fn fields(text: &str) -> Vec<(String, String)> {
    let mut in_entry = false;
    let mut out = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if let (true, Some((k, v))) = (in_entry, l.split_once('=')) {
            out.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

fn get<'a>(f: &'a [(String, String)], k: &str) -> Option<&'a str> {
    f.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

/// Vale no Glove? (OnlyShowIn precisa citar o Glove; NotShowIn não pode.)
fn for_glove(f: &[(String, String)]) -> bool {
    let has = |k: &str| get(f, k).map(|v| v.split(';').any(|d| d.eq_ignore_ascii_case("glove")));
    has("OnlyShowIn").unwrap_or(true) && !has("NotShowIn").unwrap_or(false)
}

/// Comando sem os códigos de campo (%f, %u…).
pub fn clean_exec(exec: &str) -> String {
    exec.split_whitespace().filter(|w| !(w.len() == 2 && w.starts_with('%'))).collect::<Vec<_>>().join(" ")
}

fn read_item(path: &Path, system: bool) -> Option<Item> {
    let text = std::fs::read_to_string(path).ok()?;
    let f = fields(&text);
    if get(&f, "Type").is_some_and(|t| t != "Application") || !for_glove(&f) {
        return None;
    }
    let name = get(&f, "Name[pt_BR]").or(get(&f, "Name")).unwrap_or("").to_string();
    let exec = get(&f, "Exec").map(clean_exec).unwrap_or_default();
    let enabled = get(&f, "Hidden") != Some("true") && get(&f, "X-GNOME-Autostart-enabled") != Some("false");
    Some(Item { id: path.file_name()?.to_string_lossy().into_owned(), name, exec, enabled, system })
}

fn desktop_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "desktop")).collect();
    v.sort();
    v
}

/// Todos os itens que valem no Glove (os do usuário sobrepõem os do sistema).
pub fn items() -> Vec<Item> {
    items_in(&user_dir(), &system_dirs())
}

fn items_in(user: &Path, system: &[PathBuf]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let user = user.to_path_buf();
    let user_ids: Vec<String> = desktop_files(&user).iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    for d in system {
        for p in desktop_files(d) {
            let id = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if out.iter().any(|i| i.id == id) {
                continue;
            }
            // Sobreposto pelo usuário: vale o dele, mas continua sendo "do sistema".
            let item = if user_ids.contains(&id) { read_item(&user.join(&id), true) } else { read_item(&p, true) };
            if let Some(i) = item {
                out.push(i);
            }
        }
    }
    for p in desktop_files(&user) {
        let id = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if !out.iter().any(|i| i.id == id) {
            if let Some(i) = read_item(&p, false) {
                out.push(i);
            }
        }
    }
    out
}

/// Comandos para abrir ao entrar (ativos e com o programa instalado, se pedir TryExec).
pub fn commands() -> Vec<String> {
    commands_in(&user_dir(), &system_dirs())
}

fn commands_in(user: &Path, system: &[PathBuf]) -> Vec<String> {
    items_in(user, system).into_iter().filter(|i| i.enabled && !i.exec.is_empty()).map(|i| i.exec).collect()
}

/// Liga ou desliga um item (no do sistema, grava uma cópia do usuário com Hidden).
pub fn set_enabled(id: &str, on: bool) -> std::io::Result<()> {
    set_enabled_in(&user_dir(), &system_dirs(), id, on)
}

fn set_enabled_in(user: &Path, system: &[PathBuf], id: &str, on: bool) -> std::io::Result<()> {
    let user = user.to_path_buf();
    let src = if user.join(id).is_file() { user.join(id) } else { system.iter().map(|d| d.join(id)).find(|p| p.is_file()).ok_or(std::io::ErrorKind::NotFound)? };
    let text = std::fs::read_to_string(&src)?;
    let mut out = String::new();
    let mut in_entry = false;
    let mut wrote = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            if in_entry && !wrote {
                out.push_str(&format!("Hidden={on_hidden}\n", on_hidden = !on));
                wrote = true;
            }
            in_entry = t == "[Desktop Entry]";
        } else if in_entry && (t.starts_with("Hidden=") || t.starts_with("X-GNOME-Autostart-enabled=")) {
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    if !wrote {
        out.push_str(&format!("Hidden={}\n", !on));
    }
    std::fs::create_dir_all(&user)?;
    std::fs::write(user.join(id), out)
}

/// Põe um app nos itens de início. Devolve o id do arquivo criado.
pub fn add(name: &str, exec: &str) -> std::io::Result<String> {
    add_in(&user_dir(), name, exec)
}

fn add_in(user: &Path, name: &str, exec: &str) -> std::io::Result<String> {
    let user = user.to_path_buf();
    std::fs::create_dir_all(&user)?;
    let base: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let base = base.trim_matches('-').to_string();
    let mut id = format!("glove-{base}.desktop");
    let mut n = 2;
    while user.join(&id).exists() {
        id = format!("glove-{base}-{n}.desktop");
        n += 1;
    }
    std::fs::write(user.join(&id), format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nX-Glove-Inicio=true\n"))?;
    Ok(id)
}

/// Tira um item: os do usuário são apagados; os do sistema ficam desligados.
pub fn remove(item: &Item) -> std::io::Result<()> {
    remove_in(&user_dir(), &system_dirs(), item)
}

fn remove_in(user: &Path, system: &[PathBuf], item: &Item) -> std::io::Result<()> {
    if item.system {
        return set_enabled_in(user, system, &item.id, false);
    }
    std::fs::remove_file(user.join(&item.id))
}

/// Apps que dá para pôr no início: (nome, comando). Os do Glove primeiro.
pub fn installed_apps() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = crate::default_dock_apps().into_iter().filter(|a| a.app_id.starts_with("glove.")).map(|a| (a.name, a.command)).collect();
    let mut dirs = vec![std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/applications")).unwrap_or_default()];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    let mut more = Vec::new();
    for d in dirs {
        for p in desktop_files(&d) {
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let f = fields(&text);
            if get(&f, "Type") != Some("Application") || get(&f, "NoDisplay") == Some("true") || get(&f, "Terminal") == Some("true") {
                continue;
            }
            let (Some(name), Some(exec)) = (get(&f, "Name[pt_BR]").or(get(&f, "Name")), get(&f, "Exec")) else { continue };
            if !out.iter().chain(more.iter()).any(|(n, _): &(String, String)| n.eq_ignore_ascii_case(name)) {
                more.push((name.to_string(), clean_exec(exec)));
            }
        }
    }
    more.sort_by_key(|(n, _)| n.to_lowercase());
    out.extend(more);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regras_do_xdg() {
        let f = fields("[Desktop Entry]\nName=A\nOnlyShowIn=GNOME;\n");
        assert!(!for_glove(&f));
        let f = fields("[Desktop Entry]\nName=A\nOnlyShowIn=GNOME;Glove;\n");
        assert!(for_glove(&f));
        let f = fields("[Desktop Entry]\nName=A\nNotShowIn=Glove;\n");
        assert!(!for_glove(&f));
        assert_eq!(clean_exec("firefox %u --x"), "firefox --x");
    }

    #[test]
    fn por_tirar_e_desligar() {
        let root = std::env::temp_dir().join(format!("glove-inicio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sys/autostart")).unwrap();
        std::fs::write(root.join("sys/autostart/rede.desktop"), "[Desktop Entry]\nType=Application\nName=Rede\nExec=nm-applet\n").unwrap();
        std::fs::write(root.join("sys/autostart/gnome.desktop"), "[Desktop Entry]\nType=Application\nName=G\nExec=g\nOnlyShowIn=GNOME;\n").unwrap();
        let (user, sys) = (root.join("user/autostart"), vec![root.join("sys/autostart")]);
        assert_eq!(commands_in(&user, &sys), ["nm-applet"]);
        let id = add_in(&user, "Meu App", "meu-app --x").unwrap();
        assert_eq!(id, "glove-meu-app.desktop");
        assert_eq!(commands_in(&user, &sys), ["nm-applet", "meu-app --x"]);
        set_enabled_in(&user, &sys, "rede.desktop", false).unwrap();
        assert_eq!(commands_in(&user, &sys), ["meu-app --x"]);
        let it = items_in(&user, &sys);
        assert!(it.iter().any(|i| i.id == "rede.desktop" && i.system && !i.enabled));
        remove_in(&user, &sys, it.iter().find(|i| i.id == id).unwrap()).unwrap();
        assert!(commands_in(&user, &sys).is_empty());
        set_enabled_in(&user, &sys, "rede.desktop", true).unwrap();
        assert_eq!(commands_in(&user, &sys), ["nm-applet"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
