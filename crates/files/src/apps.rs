//! Pasta Aplicativos (como a do Mac): uma pasta "virtual" com os apps
//! instalados (os .desktop do XDG) e os do Glove; os de sistema ficam em
//! Aplicativos/Utilitários. Clique duplo abre o app.

use std::path::{Path, PathBuf};

use glove_kit::paint::icons::Icon;

/// Caminho da pasta virtual (não existe no disco).
const ROOT: &str = "/:Aplicativos";
const UTILS: &str = "Utilitários";
/// Apps do Glove (que não têm .desktop): /:glove/<app_id>.
const GLOVE: &str = "/:glove";

pub fn apps_dir() -> PathBuf {
    PathBuf::from(ROOT)
}

pub fn utils_dir() -> PathBuf {
    apps_dir().join(UTILS)
}

pub fn is_virtual(p: &Path) -> bool {
    p.starts_with(ROOT)
}

/// Nome para mostrar das pastas virtuais.
pub fn display_name(p: &Path) -> Option<&'static str> {
    if p == apps_dir() {
        Some("Aplicativos")
    } else if p == utils_dir() {
        Some("Utilitários")
    } else {
        None
    }
}

pub struct App {
    pub name: String,
    /// .desktop do app (ou /:glove/<app_id> para os do Glove).
    pub path: PathBuf,
    pub icon: Icon,
    pub utility: bool,
}

/// Lê um .desktop: (nome em português se houver, é utilitário). None se não aparece.
fn parse(text: &str) -> Option<(String, bool)> {
    let mut in_entry = false;
    let (mut name, mut name_pt, mut cats, mut ok) = (None, None, String::new(), false);
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        match l.split_once('=') {
            Some(("Type", v)) => ok = v == "Application",
            Some(("Name", v)) => {
                name.get_or_insert_with(|| v.to_string());
            }
            Some(("Name[pt_BR]", v)) | Some(("Name[pt]", v)) => {
                name_pt.get_or_insert_with(|| v.to_string());
            }
            Some(("Categories", v)) => cats = v.to_string(),
            Some(("NoDisplay", "true")) | Some(("Hidden", "true")) | Some(("Terminal", "true")) => return None,
            Some(("OnlyShowIn", v)) if !v.split(';').any(|d| d.eq_ignore_ascii_case("glove")) => return None,
            _ => {}
        }
    }
    let utility = cats.split(';').any(|c| matches!(c, "System" | "Utility" | "Settings" | "Monitor"));
    ok.then(|| (name_pt.or(name).unwrap_or_default(), utility)).filter(|(n, _)| !n.is_empty())
}

/// Todos os apps: os do Glove primeiro na lista de nomes, depois os instalados.
pub fn all() -> Vec<App> {
    let mut out: Vec<App> = glove_config::default_dock_apps()
        .into_iter()
        .chain(glove_config::extra_apps())
        .filter(|a| a.app_id.starts_with("glove."))
        .map(|a| App {
            utility: matches!(a.app_id.as_str(), "glove.Monitor" | "glove.Disks" | "glove.Terminal" | "glove.Chars" | "glove.Shot" | "glove.Find" | "glove.Registros" | "glove.Info"),
            name: a.name,
            path: PathBuf::from(GLOVE).join(&a.app_id),
            icon: Icon::from_name(&a.icon),
        })
        .collect();
    let mut dirs = vec![std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/applications")).unwrap_or_default()];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    let mut seen = std::collections::HashSet::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let id = e.file_name();
            if !id.to_string_lossy().ends_with(".desktop") || !seen.insert(id) {
                continue;
            }
            let Some((name, utility)) = std::fs::read_to_string(e.path()).ok().and_then(|t| parse(&t)) else { continue };
            if out.iter().any(|a| a.name.eq_ignore_ascii_case(&name)) {
                continue;
            }
            out.push(App { name, path: e.path(), icon: Icon::Window, utility });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Abre o app (o do Glove pelo comando dele; os outros pelo gio).
pub fn open(path: &Path) -> Result<(), String> {
    if let Ok(id) = path.strip_prefix(GLOVE) {
        let id = id.to_string_lossy();
        let app = glove_config::known_app(&[], &id).ok_or("app não encontrado")?;
        let first = app.command.split_whitespace().next().unwrap_or("");
        let cmd = app.command.replacen(first, &super::sibling(first).to_string_lossy(), 1);
        return super::launch(std::process::Command::new("sh").arg("-c").arg(cmd)).map_err(|e| e.to_string());
    }
    super::launch(std::process::Command::new("gio").arg("launch").arg(path)).map_err(|e| format!("Não foi possível abrir: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop() {
        assert_eq!(super::parse("[Desktop Entry]\nType=Application\nName=Files\nName[pt_BR]=Arquivos\nCategories=GTK;Utility;\n"), Some(("Arquivos".into(), true)));
        assert_eq!(super::parse("[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n"), None);
        assert!(super::is_virtual(&super::utils_dir()));
    }
}
