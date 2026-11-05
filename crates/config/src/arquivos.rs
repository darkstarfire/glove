//! O que o Arquivos e a mesa compartilham: as Preferências do Arquivos, os
//! rótulos de cor (guardados no próprio arquivo, num atributo estendido) e os
//! apps que abrem cada tipo de arquivo ("Abrir com").

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Preferências do Arquivos (Arquivos ▸ Preferências…) e as Opções de
/// Visualização das janelas (Ctrl+J).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Files {
    /// Onde abrem as janelas novas: "inicio", "mesa", "documentos" ou "computador".
    pub new_window: String,
    /// Nomes dos sete rótulos (na ordem das cores de `LABEL_COLORS`).
    pub labels: Vec<String>,
    /// Lugares escondidos da barra lateral ("disco", "mesa", "inicio", "aplicativos",
    /// "documentos", "downloads", "imagens", "musica", "videos", "buscas").
    pub sidebar_hidden: Vec<String>,
    /// Mostrar a extensão dos nomes (".txt").
    pub show_extensions: bool,
    /// Perguntar antes de esvaziar o Lixo.
    pub warn_empty_trash: bool,
    /// Vista de ícones: lado do ícone (32 a 128), tamanho do nome (10 a 16), nome à direita.
    pub icon_size: u32,
    pub text_size: u32,
    pub label_right: bool,
    /// Ordem dos ícones: "nome", "data", "tamanho" ou "tipo".
    pub arrange: String,
    /// Fundo da vista de ícones: "" (branco) ou "#rrggbb".
    pub background: String,
    /// Conectar ao Servidor: os favoritos e os últimos usados.
    pub servers: Vec<String>,
    pub recent_servers: Vec<String>,
    /// Servidores conectados aparecem na mesa.
    pub servers_on_desk: bool,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            new_window: "inicio".into(),
            labels: DEFAULT_LABELS.iter().map(|s| s.to_string()).collect(),
            sidebar_hidden: Vec::new(),
            show_extensions: true,
            warn_empty_trash: true,
            icon_size: 64,
            text_size: 12,
            label_right: false,
            arrange: "nome".into(),
            background: String::new(),
            servers: Vec::new(),
            recent_servers: Vec::new(),
            servers_on_desk: true,
        }
    }
}

impl Files {
    /// Nome do rótulo `n` (1 a 7).
    pub fn label_name(&self, n: u8) -> String {
        let i = (n as usize).saturating_sub(1);
        self.labels.get(i).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| DEFAULT_LABELS.get(i).copied().unwrap_or("").to_string())
    }

    /// Pasta das janelas novas.
    pub fn new_window_dir(&self) -> PathBuf {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
        match self.new_window.as_str() {
            "mesa" => crate::desktop_dir(),
            "documentos" => user_dir("DOCUMENTS").unwrap_or(home),
            "computador" => PathBuf::from("/"),
            _ => home,
        }
    }

    /// Cor de fundo da vista de ícones (None = branco).
    pub fn background_rgb(&self) -> Option<(u8, u8, u8)> {
        let h = self.background.strip_prefix('#')?;
        if h.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }
}

/// Pasta do usuário pelo ~/.config/user-dirs.dirs ("DOCUMENTS" → ~/Documentos).
pub fn user_dir(key: &str) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let text = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok()?;
    let line = text.lines().find(|l| l.starts_with(&format!("XDG_{key}_DIR=")))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(v),
    };
    (p != home && p.is_dir()).then_some(p)
}

// ---------------------------------------------------------------------------
// Rótulos de cor
// ---------------------------------------------------------------------------

pub const DEFAULT_LABELS: [&str; 7] = ["Vermelho", "Laranja", "Amarelo", "Verde", "Azul", "Roxo", "Cinza"];
/// Cores dos rótulos (como as do Tiger).
pub const LABEL_COLORS: [(u8, u8, u8); 7] = [(252, 86, 76), (250, 160, 54), (246, 216, 60), (148, 210, 70), (82, 156, 250), (184, 118, 220), (166, 166, 170)];
const LABEL_ATTR: &str = "user.glove.rotulo";

fn cpath(p: &Path) -> Option<std::ffi::CString> {
    std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).ok()
}

/// Rótulo do arquivo (0 = nenhum, 1 a 7).
pub fn label_of(p: &Path) -> u8 {
    let (Some(path), Ok(attr)) = (cpath(p), std::ffi::CString::new(LABEL_ATTR)) else { return 0 };
    let mut buf = [0u8; 4];
    let n = unsafe { libc::getxattr(path.as_ptr(), attr.as_ptr(), buf.as_mut_ptr() as *mut _, buf.len()) };
    if n <= 0 {
        return 0;
    }
    std::str::from_utf8(&buf[..n as usize]).ok().and_then(|s| s.trim().parse::<u8>().ok()).filter(|v| *v <= 7).unwrap_or(0)
}

/// Põe (1 a 7) ou tira (0) o rótulo.
pub fn set_label(p: &Path, n: u8) -> Result<(), String> {
    let path = cpath(p).ok_or("caminho inválido")?;
    let attr = std::ffi::CString::new(LABEL_ATTR).unwrap();
    let r = unsafe {
        if n == 0 {
            let r = libc::removexattr(path.as_ptr(), attr.as_ptr());
            if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA) { 0 } else { r }
        } else {
            let v = n.to_string();
            libc::setxattr(path.as_ptr(), attr.as_ptr(), v.as_ptr() as *const _, v.len(), 0)
        }
    };
    if r == 0 { Ok(()) } else { Err(format!("Este disco não guarda rótulos ({})", std::io::Error::last_os_error())) }
}

// ---------------------------------------------------------------------------
// Tipos de arquivo e "Abrir com"
// ---------------------------------------------------------------------------

/// Extensão → tipo, lido uma vez do banco de tipos do sistema (globs2).
fn globs() -> &'static HashMap<String, String> {
    static G: OnceLock<HashMap<String, String>> = OnceLock::new();
    G.get_or_init(|| {
        let mut m = HashMap::new();
        let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
        let mut dirs: Vec<PathBuf> = std::env::var_os("HOME").map(|h| vec![PathBuf::from(h).join(".local/share")]).unwrap_or_default();
        dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
        for d in dirs {
            let Ok(text) = std::fs::read_to_string(d.join("mime/globs2")) else { continue };
            for l in text.lines().filter(|l| !l.starts_with('#')) {
                // peso:tipo:padrão
                let mut f = l.splitn(3, ':');
                let (_, Some(mime), Some(pat)) = (f.next(), f.next(), f.next()) else { continue };
                if let Some(ext) = pat.strip_prefix("*.").filter(|e| !e.contains(['*', '?', '['])) {
                    m.entry(ext.to_lowercase()).or_insert_with(|| mime.to_string());
                }
            }
        }
        m
    })
}

/// Tipo do arquivo pela extensão (rápido; sem abrir o arquivo).
pub fn mime_of(p: &Path) -> String {
    if p.is_dir() {
        return "inode/directory".into();
    }
    let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Extensões compostas primeiro (".tar.gz").
    for (i, _) in name.match_indices('.') {
        if let Some(m) = globs().get(&name[i + 1..]) {
            return m.clone();
        }
    }
    "application/octet-stream".into()
}

fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/applications")).unwrap_or_default()];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

/// Apps instalados que abrem `mime`: (id do .desktop, nome), em ordem alfabética.
pub fn apps_for_mime(mime: &str) -> Vec<(String, String)> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<(String, String)>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().ok().and_then(|c| c.get(mime).cloned()) {
        return v;
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for d in app_dirs() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id.ends_with(".desktop") || out.iter().any(|(i, _)| *i == id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            let (mut name, mut name_pt, mut mimes, mut hidden, mut in_entry) = (None, None, "", false, false);
            for l in text.lines() {
                if l.starts_with('[') {
                    in_entry = l == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = l.strip_prefix("Name=") {
                    name.get_or_insert(v);
                }
                if let Some(v) = l.strip_prefix("Name[pt_BR]=") {
                    name_pt = Some(v);
                }
                if let Some(v) = l.strip_prefix("MimeType=") {
                    mimes = v;
                }
                // NoDisplay só tira dos menus: o app ainda abre arquivos (ex.: os do Glove).
                if l == "Hidden=true" {
                    hidden = true;
                }
            }
            if !hidden && mimes.split(';').any(|m| m == mime) {
                // O mesmo app instalado de dois jeitos (pacote e Flatpak): aparece uma vez.
                if let Some(n) = name_pt.or(name).filter(|n| !out.iter().any(|(_, o)| o == n)) {
                    out.push((id, n.to_string()));
                }
            }
        }
    }
    out.sort_by_key(|a| a.1.to_lowercase());
    if let Ok(mut c) = cache.lock() {
        c.insert(mime.to_string(), out.clone());
    }
    out
}

/// App padrão do tipo (mimeapps.list do usuário e do sistema).
pub fn default_app(mime: &str) -> Option<String> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut files = vec![home.join(".config/mimeapps.list"), home.join(".local/share/applications/mimeapps.list")];
    files.extend(app_dirs().into_iter().flat_map(|d| [d.join("mimeapps.list"), d.join("defaults.list")]));
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let mut section = "";
        for l in text.lines() {
            if l.starts_with('[') {
                section = l;
                continue;
            }
            if section != "[Default Applications]" {
                continue;
            }
            if let Some(v) = l.strip_prefix(mime).and_then(|r| r.strip_prefix('=')) {
                if let Some(first) = v.split(';').find(|s| !s.is_empty()) {
                    return Some(first.to_string());
                }
            }
        }
    }
    None
}

/// "Abrir com": o app (id do .desktop) abre os arquivos. Devolve o comando para o shell.
pub fn open_with_command(app_id: &str, paths: &[PathBuf]) -> String {
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    let args: Vec<String> = paths.iter().map(|p| q(p)).collect();
    // Apps do Glove (glove-view.desktop…): o próprio binário, ao lado do glove-files.
    if let Some(bin) = app_id.strip_suffix(".desktop").filter(|b| b.starts_with("glove-")) {
        return format!("{bin} {}", args.join(" "));
    }
    let file = app_dirs().into_iter().map(|d| d.join(app_id)).find(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(app_id));
    format!("gio launch {} {}", q(&file), args.join(" "))
}

/// Nome sem a extensão (quando as Preferências escondem as extensões). Pastas ficam inteiras.
pub fn shown_name(name: &str, is_dir: bool, show_ext: bool) -> String {
    if show_ext || is_dir {
        return name.to_string();
    }
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_sem_extensao() {
        assert_eq!(shown_name("foto.png", false, false), "foto");
        assert_eq!(shown_name(".bashrc", false, false), ".bashrc");
        assert_eq!(shown_name("pasta.d", true, false), "pasta.d");
        assert_eq!(shown_name("foto.png", false, true), "foto.png");
    }

    #[test]
    fn rotulos_no_arquivo() {
        let p = std::env::temp_dir().join(format!("glove-rotulo-{}", std::process::id()));
        std::fs::write(&p, "x").unwrap();
        // Alguns discos (tmpfs antigos) não guardam atributos: aí só confere que não quebra.
        if set_label(&p, 3).is_ok() {
            assert_eq!(label_of(&p), 3);
            set_label(&p, 0).unwrap();
            assert_eq!(label_of(&p), 0);
        }
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn cor_de_fundo() {
        let f = Files { background: "#ff8000".into(), ..Default::default() };
        assert_eq!(f.background_rgb(), Some((255, 128, 0)));
        assert_eq!(Files::default().background_rgb(), None);
    }
}
