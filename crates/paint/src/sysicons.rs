//! Ícones dos temas do sistema (padrão freedesktop: /usr/share/icons, ~/.local/share/icons),
//! em PNG ou SVG, seguindo a herança dos temas (Inherits) até o hicolor. Usado para
//! os apps de fora (o ícone de verdade de cada um) e, com um tema escolhido nas
//! Preferências, também para os apps e as pastas do Glove.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tiny_skia::{IntSize, Pixmap};

/// Uma pasta de um tema: tamanho nominal, escala e se é vetorial.
struct Dir {
    path: String,
    size: u32,
    scale: u32,
    scalable: bool,
}

struct Theme {
    roots: Vec<PathBuf>,
    dirs: Vec<Dir>,
    inherits: Vec<String>,
}

fn base_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        v.push(h.join(".local/share/icons"));
        v.push(h.join(".icons"));
    }
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("icons")));
    v
}

fn load_theme(name: &str) -> Option<Theme> {
    let roots: Vec<PathBuf> = base_dirs().into_iter().map(|b| b.join(name)).filter(|p| p.is_dir()).collect();
    let index = roots.iter().find_map(|r| std::fs::read_to_string(r.join("index.theme")).ok())?;
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut cur = String::new();
    for l in index.lines() {
        let l = l.trim();
        if let Some(s) = l.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            cur = s.to_string();
        } else if let Some((k, v)) = l.split_once('=') {
            sections.entry(cur.clone()).or_default().insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let head = sections.get("Icon Theme")?;
    let inherits = head.get("Inherits").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let mut list: Vec<String> = head.get("Directories").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
    if let Some(more) = head.get("ScaledDirectories") {
        list.extend(more.split(',').map(|s| s.trim().to_string()));
    }
    let dirs = list
        .into_iter()
        .filter(|d| !d.is_empty())
        .map(|d| {
            let s = sections.get(&d);
            let num = |k: &str, def: u32| s.and_then(|s| s.get(k)).and_then(|v| v.parse().ok()).unwrap_or(def);
            let size = num("Size", 48);
            Dir { scalable: s.and_then(|s| s.get("Type")).is_some_and(|t| t == "Scalable"), size, scale: num("Scale", 1), path: d }
        })
        .collect();
    Some(Theme { roots, dirs, inherits })
}

/// Temas instalados com ícones (não os só de cursor): (pasta, nome para mostrar).
pub fn installed_themes() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for b in base_dirs() {
        let Ok(rd) = std::fs::read_dir(&b) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if out.iter().any(|(i, _)| *i == id) || id == "hicolor" || id == "default" || id == "locolor" {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path().join("index.theme")) else { continue };
            if !text.contains("Directories=") {
                continue;
            }
            let name = text.lines().find_map(|l| l.strip_prefix("Name=")).unwrap_or(&id).trim().to_string();
            out.push((id, name));
        }
    }
    out.sort_by_key(|(_, n)| n.to_lowercase());
    out
}

struct Lookup {
    themes: HashMap<String, Option<Theme>>,
    found: HashMap<(String, String, u32), Option<PathBuf>>,
}

fn lookup() -> &'static Mutex<Lookup> {
    static L: OnceLock<Mutex<Lookup>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(Lookup { themes: HashMap::new(), found: HashMap::new() }))
}

/// Arquivo do ícone `name` no tema `theme` (ou nos que ele herda), perto de `px`.
pub fn find(theme: &str, name: &str, px: u32) -> Option<PathBuf> {
    if name.starts_with('/') {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    let mut l = lookup().lock().ok()?;
    let key = (theme.to_string(), name.to_string(), px);
    if let Some(f) = l.found.get(&key) {
        return f.clone();
    }
    // Cadeia de temas: o escolhido, os que ele herda, e o hicolor no fim.
    let mut chain: Vec<String> = vec![theme.to_string()];
    let mut i = 0;
    while i < chain.len() && chain.len() < 12 {
        let t = chain[i].clone();
        if !l.themes.contains_key(&t) {
            let loaded = load_theme(&t);
            l.themes.insert(t.clone(), loaded);
        }
        if let Some(Some(th)) = l.themes.get(&t) {
            for inh in &th.inherits {
                if !chain.contains(inh) {
                    chain.push(inh.clone());
                }
            }
        }
        i += 1;
    }
    if !chain.iter().any(|t| t == "hicolor") {
        chain.push("hicolor".into());
        if !l.themes.contains_key("hicolor") {
            let h = load_theme("hicolor");
            l.themes.insert("hicolor".into(), h);
        }
    }
    let mut result = None;
    'themes: for t in &chain {
        let Some(Some(th)) = l.themes.get(t) else { continue };
        // A pasta de tamanho mais próximo (vetorial conta como qualquer tamanho).
        let mut best: Option<(u32, PathBuf)> = None;
        for d in &th.dirs {
            for root in &th.roots {
                for ext in ["svg", "png"] {
                    let f = root.join(&d.path).join(format!("{name}.{ext}"));
                    if f.is_file() {
                        let eff = d.size * d.scale;
                        let dist = if d.scalable { 1 } else if eff >= px { eff - px } else { (px - eff) * 2 + 8 };
                        if best.as_ref().is_none_or(|(b, _)| dist < *b) {
                            best = Some((dist, f));
                        }
                    }
                }
            }
        }
        if let Some((_, f)) = best {
            result = Some(f);
            break 'themes;
        }
    }
    if result.is_none() {
        result = ["/usr/share/pixmaps"].iter().flat_map(|d| ["png", "svg", "xpm"].map(|e| Path::new(d).join(format!("{name}.{e}")))).find(|p| p.is_file() && p.extension().is_some_and(|e| e != "xpm"));
    }
    l.found.insert(key, result.clone());
    result
}

thread_local! {
    /// Imagens já prontas: (arquivo, lado em pixels) → imagem.
    static RASTER: RefCell<Vec<((PathBuf, u32), Option<std::rc::Rc<Pixmap>>)>> = const { RefCell::new(Vec::new()) };
}

fn rasterize(path: &Path, px: u32) -> Option<Pixmap> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "svg" {
        let data = std::fs::read(path).ok()?;
        let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size();
        let k = px as f32 / size.width().max(size.height()).max(1.0);
        let mut pm = resvg::tiny_skia::Pixmap::new(px, px)?;
        let (dw, dh) = (size.width() * k, size.height() * k);
        let t = resvg::tiny_skia::Transform::from_row(k, 0.0, 0.0, k, (px as f32 - dw) / 2.0, (px as f32 - dh) / 2.0);
        resvg::render(&tree, t, &mut pm.as_mut());
        // O resvg usa outra versão do tiny-skia: os bytes (RGBA pré-multiplicado) são os mesmos.
        return Pixmap::from_vec(pm.take(), IntSize::from_wh(px, px)?);
    }
    let img = Pixmap::load_png(path).ok()?;
    if img.width() == px && img.height() == px {
        return Some(img);
    }
    let mut out = Pixmap::new(px, px)?;
    let k = px as f32 / img.width().max(img.height()) as f32;
    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    let (dw, dh) = (img.width() as f32 * k, img.height() as f32 * k);
    out.draw_pixmap(0, 0, img.as_ref(), &paint, tiny_skia::Transform::from_row(k, 0.0, 0.0, k, (px as f32 - dw) / 2.0, (px as f32 - dh) / 2.0), None);
    Some(out)
}

/// O ícone `name` do tema, pronto para desenhar com `px` pixels de lado.
pub fn image(theme: &str, name: &str, px: u32) -> Option<std::rc::Rc<Pixmap>> {
    let px = px.clamp(8, 512);
    let path = find(theme, name, px)?;
    let key = (path.clone(), px);
    if let Some(hit) = RASTER.with(|r| r.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let img = rasterize(&path, px).map(std::rc::Rc::new);
    RASTER.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 160 {
            r.remove(0);
        }
        r.push((key, img.clone()));
    });
    img
}
