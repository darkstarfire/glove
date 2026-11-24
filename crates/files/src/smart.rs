//! Pastas inteligentes (10.4): buscas por nome que aparecem como pastas.
//! A busca da barra mostra os resultados em /:Busca/<texto> (com "Salvar");
//! as salvas ficam em ~/.config/glove/searches/<nome>.search e aparecem na
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
    static MIGRATED: std::sync::Once = std::sync::Once::new();
    let config = glove_config::dir();
    // Every read and write of the saved searches comes through here: what an older version
    // stored moves before the first one.
    MIGRATED.call_once(|| migrate_old_names(&config));
    config.join("searches")
}

/// The keys an older version wrote in a saved search, and today's.
const OLD_KEYS: [(&str, &str); 2] = [("busca=", "query="), ("onde=", "where=")];

/// Today's content of a saved search an older version wrote: the keys `busca=` and `onde=`
/// become `query=` and `where=`; every other line stays as it is.
fn current_saved(stored: &str) -> String {
    stored
        .split_inclusive('\n')
        .map(|line| match OLD_KEYS.iter().find_map(|(old, new)| line.strip_prefix(old).map(|value| format!("{new}{value}"))) {
            Some(converted) => converted,
            None => line.to_string(),
        })
        .collect()
}

/// Moves the saved searches an older version kept under `config` (the Glove configuration
/// folder): the folder `buscas` becomes `searches` and, inside it, each `<name>.busca` becomes
/// `<name>.search` with today's keys. Runs once: afterwards nothing is left under the old names.
fn migrate_old_names(config: &Path) {
    let dir = config.join("searches");
    glove_config::move_stored(&config.join("buscas"), &dir);
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let file = e.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_suffix(".busca") else { continue };
        let (old, new) = (e.path(), dir.join(format!("{name}.search")));
        // A search saved under today's name wins; the old file only goes away once the new one is there.
        if !new.exists() {
            let Ok(stored) = std::fs::read_to_string(&old) else { continue };
            if std::fs::write(&new, current_saved(&stored)).is_err() {
                continue;
            }
        }
        let _ = std::fs::remove_file(&old);
    }
}

/// The text and the place of a saved search, from the content of its file.
fn parse_saved(t: &str) -> Option<(String, Option<PathBuf>)> {
    let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}=")).map(str::to_string));
    Some((get("query")?, get("where").map(PathBuf::from)))
}

/// Busca guardada: (texto, onde).
fn read_saved(name: &str) -> Option<(String, PathBuf)> {
    let (query, scope) = parse_saved(&std::fs::read_to_string(store().join(format!("{name}.search"))).ok()?)?;
    Some((query, scope.unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))))
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
    std::fs::write(dir.join(format!("{name}.search")), format!("query={query}\nwhere={}\n", scope.display())).map_err(|e| e.to_string())?;
    Ok(saved_path(&name))
}

pub fn delete(p: &Path) {
    if let Ok(n) = p.strip_prefix(SAVED) {
        let _ = std::fs::remove_file(store().join(format!("{}.search", n.to_string_lossy())));
    }
}

/// Nomes das buscas salvas, em ordem.
pub fn saved() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(store())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".search").map(str::to_string))
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
    use super::{current_saved, migrate_old_names, parse_saved};
    use std::path::{Path, PathBuf};

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

    /// An empty folder for one test, under the system's temporary folder.
    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("glove-smart-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The files of a folder with their content, in order.
    fn listing(dir: &Path) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(e.path()).unwrap()))
            .collect();
        v.sort();
        v
    }

    fn owned(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect()
    }

    #[test]
    fn old_keys_of_a_saved_search() {
        assert_eq!(current_saved("busca=relatorio anual\nonde=/home/u/Documentos\n"), "query=relatorio anual\nwhere=/home/u/Documentos\n");
        // Only a key at the start of a line changes; the text searched for is never touched.
        assert_eq!(current_saved("busca=onde=busca=\nonde=/a/busca=b"), "query=onde=busca=\nwhere=/a/busca=b");
        // Today's format, and anything that is not a key, stays as it is.
        let today = "query=relatorio\nwhere=/home/u\n";
        assert_eq!(current_saved(today), today);
        assert_eq!(current_saved(""), "");
        assert_eq!(current_saved("other=1\r\n\n"), "other=1\r\n\n");
        // What was converted is what today's reader understands.
        assert_eq!(parse_saved(&current_saved("busca=a b\nonde=/x y\n")), Some(("a b".to_string(), Some(PathBuf::from("/x y")))));
        assert_eq!(parse_saved(&current_saved("busca=a\n")), Some(("a".to_string(), None)));
        assert_eq!(parse_saved("busca=a\nonde=/x\n"), None);
    }

    #[test]
    fn old_searches_are_moved_once() {
        let config = temp("moved");
        let old = config.join("buscas");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("Relatórios.busca"), "busca=relatorio\nonde=/home/u/Documentos\n").unwrap();
        std::fs::write(old.join("v1.2 notas.busca"), "busca=notas\nonde=/home/u\n").unwrap();
        std::fs::write(old.join("leia-me.txt"), "busca=not a saved search").unwrap();
        migrate_old_names(&config);
        assert!(!old.exists());
        let expected = owned(&[
            ("Relatórios.search", "query=relatorio\nwhere=/home/u/Documentos\n"),
            ("leia-me.txt", "busca=not a saved search"),
            ("v1.2 notas.search", "query=notas\nwhere=/home/u\n"),
        ]);
        assert_eq!(listing(&config.join("searches")), expected);
        // A second run finds nothing to do.
        migrate_old_names(&config);
        assert!(!old.exists());
        assert_eq!(listing(&config.join("searches")), expected);
        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn new_searches_are_left_untouched() {
        let config = temp("untouched");
        let (old, new) = (config.join("buscas"), config.join("searches"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("a.busca"), "busca=old\n").unwrap();
        std::fs::write(new.join("a.search"), "query=new\nwhere=/home/u\n").unwrap();
        // A leftover of the old name beside today's file: today's wins and the leftover goes.
        std::fs::write(new.join("a.busca"), "busca=leftover\n").unwrap();
        // An old file alone in today's folder is converted.
        std::fs::write(new.join("b.busca"), "busca=b\nonde=/tmp\n").unwrap();
        migrate_old_names(&config);
        assert_eq!(listing(&new), owned(&[("a.search", "query=new\nwhere=/home/u\n"), ("b.search", "query=b\nwhere=/tmp\n")]));
        // The new folder is the one in use: the old one is not merged into it.
        assert_eq!(listing(&old), owned(&[("a.busca", "busca=old\n")]));
        // Nothing stored at all: nothing is created.
        let empty = temp("empty");
        migrate_old_names(&empty);
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&config);
        let _ = std::fs::remove_dir_all(&empty);
    }
}
