//! Pastas inteligentes (10.4): buscas por nome que aparecem como pastas.
//! A busca da barra mostra os resultados em /:Busca/<texto> (com "Salvar");
//! as salvas ficam em ~/.config/glove/buscas/<nome>.busca e aparecem na
//! barra lateral como /:Buscas/<nome>.

use std::path::{Path, PathBuf};

const TEMP: &str = "/:Busca";
const SAVED: &str = "/:Buscas";
/// Busca no máximo isso de itens (pastas grandes não travam a janela).
const MAX_SCAN: usize = 60_000;
const MAX_RESULTS: usize = 500;

pub fn is_smart(p: &Path) -> bool {
    p.starts_with(TEMP) || p.starts_with(SAVED)
}

/// Pasta de resultados de uma busca ainda não salva.
pub fn temp_path(query: &str) -> PathBuf {
    PathBuf::from(TEMP).join(query.replace('/', " "))
}

pub fn saved_path(name: &str) -> PathBuf {
    PathBuf::from(SAVED).join(name)
}

fn store() -> PathBuf {
    glove_config::dir().join("buscas")
}

/// Busca guardada: (texto, onde).
fn read_saved(name: &str) -> Option<(String, PathBuf)> {
    let t = std::fs::read_to_string(store().join(format!("{name}.busca"))).ok()?;
    let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}=")).map(str::to_string));
    Some((get("busca")?, PathBuf::from(get("onde").unwrap_or_else(|| std::env::var("HOME").unwrap_or_default()))))
}

/// Texto e lugar de uma pasta de busca (temporária ou salva).
pub fn query_of(p: &Path, scope: &Path) -> Option<(String, PathBuf)> {
    if let Ok(q) = p.strip_prefix(TEMP) {
        return Some((q.to_string_lossy().into_owned(), scope.to_path_buf()));
    }
    read_saved(&p.strip_prefix(SAVED).ok()?.to_string_lossy())
}

pub fn save(name: &str, query: &str, scope: &Path) -> Result<PathBuf, String> {
    let dir = store();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = name.replace('/', " ");
    std::fs::write(dir.join(format!("{name}.busca")), format!("busca={query}\nonde={}\n", scope.display())).map_err(|e| e.to_string())?;
    Ok(saved_path(&name))
}

pub fn delete(p: &Path) {
    if let Ok(n) = p.strip_prefix(SAVED) {
        let _ = std::fs::remove_file(store().join(format!("{}.busca", n.to_string_lossy())));
    }
}

/// Nomes das buscas salvas, em ordem.
pub fn saved() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(store())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".busca").map(str::to_string))
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

fn norm(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Itens em `scope` (e abaixo) cujo nome tem todas as palavras de `query`.
pub fn search(query: &str, scope: &Path) -> Vec<PathBuf> {
    let words: Vec<String> = norm(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut stack = vec![(scope.to_path_buf(), 0usize)];
    let mut seen = 0;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > MAX_SCAN || out.len() >= MAX_RESULTS {
                return out;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let n = norm(&name);
            if words.iter().all(|w| n.contains(w.as_str())) {
                out.push(e.path());
            }
            if depth < 8 && e.file_type().is_ok_and(|t| t.is_dir()) && !matches!(name.as_str(), "node_modules" | "target") {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn busca_por_palavras() {
        let root = std::env::temp_dir().join(format!("glove-smart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        for f in ["Relatório Anual.txt", "a/relatorio mensal.pdf", "a/b/outro.txt", ".oculto relatorio"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        let mut r: Vec<String> = super::search("relatorio", &root).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        r.sort();
        assert_eq!(r, ["Relatório Anual.txt", "relatorio mensal.pdf"]);
        assert_eq!(super::search("anual relat", &root).len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
