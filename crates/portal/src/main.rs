//! glove-portal: o "escolher arquivo" do sistema (portal FileChooser) atendido
//! pela janela Abrir/Salvar do Glove. Navegadores e outros apps pedem ao
//! xdg-desktop-portal, que repassa para cá (glove-portals.conf).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zbus::zvariant::{ObjectPath, OwnedValue, Value};

struct FileChooser;

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

fn uri(p: &Path) -> String {
    glove_kit::clipfiles::uri(p)
}

fn string(o: &HashMap<String, OwnedValue>, k: &str) -> Option<String> {
    o.get(k).and_then(|v| String::try_from(v.try_clone().ok()?).ok())
}

fn boolean(o: &HashMap<String, OwnedValue>, k: &str) -> bool {
    o.get(k).and_then(|v| bool::try_from(v.try_clone().ok()?).ok()).unwrap_or(false)
}

/// Caminho em bytes terminado em zero (o formato "ay" do portal).
fn bytes_path(o: &HashMap<String, OwnedValue>, k: &str) -> Option<PathBuf> {
    let v: Vec<u8> = o.get(k).and_then(|v| Vec::<u8>::try_from(v.try_clone().ok()?).ok())?;
    let v: Vec<u8> = v.into_iter().take_while(|b| *b != 0).collect();
    use std::os::unix::ffi::OsStringExt;
    (!v.is_empty()).then(|| PathBuf::from(std::ffi::OsString::from_vec(v)))
}

/// Extensões dos filtros do app ("*.png" → "png"); vazio = tudo.
fn extensions(o: &HashMap<String, OwnedValue>) -> Vec<String> {
    let Some(v) = o.get("filters").and_then(|v| v.try_clone().ok()) else { return Vec::new() };
    let Ok(filters) = <Vec<(String, Vec<(u32, String)>)>>::try_from(v) else { return Vec::new() };
    let mut out = Vec::new();
    for (_, rules) in filters {
        for (kind, pat) in rules {
            if kind == 0 {
                if let Some(ext) = pat.strip_prefix("*.") {
                    if ext.chars().all(|c| c.is_alphanumeric()) {
                        out.push(ext.to_lowercase());
                    }
                } else if pat == "*" {
                    return Vec::new();
                }
            }
        }
    }
    out
}

/// Abre a janela do Glove e espera a escolha.
fn choose(mode: &str, title: &str, name: Option<String>, dir: Option<PathBuf>, types: &[String]) -> Option<PathBuf> {
    let mut cmd = std::process::Command::new(sibling("glove-escolher"));
    cmd.arg(mode).arg("--titulo").arg(title);
    if let Some(n) = name {
        cmd.arg("--nome").arg(n);
    }
    if let Some(d) = dir {
        cmd.arg("--em").arg(d);
    }
    if !types.is_empty() {
        cmd.arg("--tipos").arg(types.join(","));
    }
    let out = cmd.stderr(std::process::Stdio::inherit()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then(|| PathBuf::from(s))
}

/// Resposta do portal: 0 = escolheu, 1 = cancelou.
fn reply(paths: Option<Vec<PathBuf>>) -> (u32, HashMap<String, OwnedValue>) {
    let mut r = HashMap::new();
    match paths {
        Some(p) => {
            let uris: Vec<String> = p.iter().map(|p| uri(p)).collect();
            if let Ok(v) = OwnedValue::try_from(Value::from(uris)) {
                r.insert("uris".to_string(), v);
            }
            (0, r)
        }
        None => (1, r),
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[zbus(name = "OpenFile")]
    fn open_file(&self, _handle: ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let mode = if boolean(&options, "directory") { "pasta" } else { "abrir" };
        let dir = bytes_path(&options, "current_folder");
        reply(choose(mode, title, None, dir, &extensions(&options)).map(|p| vec![p]))
    }

    #[zbus(name = "SaveFile")]
    fn save_file(&self, _handle: ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let name = string(&options, "current_name");
        // Sem pasta pedida: Downloads (onde os navegadores costumam salvar).
        let dir = bytes_path(&options, "current_folder").or_else(|| bytes_path(&options, "current_file").and_then(|f| f.parent().map(Path::to_path_buf))).or_else(downloads);
        reply(choose("salvar", title, name, dir, &[]).map(|p| vec![p]))
    }

    /// Vários arquivos de uma vez: escolhe a pasta e devolve pasta/nome de cada um.
    #[zbus(name = "SaveFiles")]
    fn save_files(&self, _handle: ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let names: Vec<Vec<u8>> = options.get("files").and_then(|v| <Vec<Vec<u8>>>::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
        let dir = bytes_path(&options, "current_folder").or_else(downloads);
        reply(choose("pasta", title, None, dir, &[]).map(|d| {
            use std::os::unix::ffi::OsStringExt;
            names.into_iter().map(|n| d.join(std::ffi::OsString::from_vec(n.into_iter().take_while(|b| *b != 0).collect()))).collect()
        }))
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        4
    }
}

fn downloads() -> Option<PathBuf> {
    let h = PathBuf::from(std::env::var_os("HOME")?);
    let text = std::fs::read_to_string(h.join(".config/user-dirs.dirs")).unwrap_or_default();
    let p = text
        .lines()
        .find_map(|l| l.strip_prefix("XDG_DOWNLOAD_DIR="))
        .map(|v| v.trim().trim_matches('"').replace("$HOME", &h.to_string_lossy()))
        .map(PathBuf::from)
        .unwrap_or_else(|| h.join("Downloads"));
    p.is_dir().then_some(p)
}

fn main() -> zbus::Result<()> {
    let _conn = zbus::blocking::connection::Builder::session()?
        .name("org.freedesktop.impl.portal.desktop.glove")?
        .serve_at("/org/freedesktop/portal/desktop", FileChooser)?
        .build()?;
    // O D-Bus inicia este programa quando precisa; ele fica atendendo.
    loop {
        std::thread::park();
    }
}
