//! Copiar e mover arquivos numa thread, com progresso, cancelamento e o
//! tratamento de nomes repetidos decidido antes de começar.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// O que fazer com um item cujo nome já existe no destino.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Conflict {
    Replace,
    KeepBoth,
}

/// Um item a copiar ou mover para `dest` (caminho final, já com o nome).
#[derive(Clone, Debug)]
pub struct Item {
    pub src: PathBuf,
    pub dest: PathBuf,
    pub replace: bool,
}

#[derive(Default)]
pub struct Progress {
    pub total: AtomicU64,
    pub done: AtomicU64,
    pub cancel: AtomicBool,
    pub finished: AtomicBool,
    pub current: Mutex<String>,
    pub error: Mutex<Option<String>>,
}

/// "foto.png" → "foto 2.png" (o primeiro nome livre na pasta).
pub fn free_name(dest: &Path) -> PathBuf {
    let dir = dest.parent().unwrap_or(Path::new("."));
    let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem} {n}{ext}"))).find(|p| !p.exists()).unwrap()
}

/// Monta a lista de itens para colar `srcs` em `dir`. Devolve também os nomes
/// que já existem lá (para perguntar antes). Colar na própria pasta duplica.
pub fn plan(srcs: &[PathBuf], dir: &Path) -> (Vec<Item>, Vec<String>) {
    let mut items = Vec::new();
    let mut clashes = Vec::new();
    for s in srcs {
        let Some(name) = s.file_name() else { continue };
        let dest = dir.join(name);
        if dest == *s {
            items.push(Item { src: s.clone(), dest: free_name(&dest), replace: false });
            continue;
        }
        if dest.exists() {
            clashes.push(name.to_string_lossy().into_owned());
        }
        items.push(Item { src: s.clone(), dest, replace: false });
    }
    (items, clashes)
}

/// Aplica a decisão sobre os nomes repetidos.
pub fn resolve(items: &mut [Item], c: Conflict) {
    for it in items.iter_mut() {
        if it.dest.exists() && it.dest != it.src {
            match c {
                Conflict::Replace => it.replace = true,
                Conflict::KeepBoth => it.dest = free_name(&it.dest),
            }
        }
    }
}

fn tree_size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| tree_size(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// Uma pasta dentro dela mesma não dá (ex.: copiar "A" para "A/B").
fn inside(dest: &Path, src: &Path) -> bool {
    dest.starts_with(src)
}

fn copy_tree(src: &Path, dest: &Path, p: &Progress) -> Result<(), String> {
    if p.cancel.load(Ordering::Relaxed) {
        return Err("cancelado".into());
    }
    let meta = std::fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(src).map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(target, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    } else if meta.is_dir() {
        std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        for e in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
            copy_tree(&e.path(), &dest.join(e.file_name()), p)?;
        }
    } else {
        *p.current.lock().unwrap() = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Em pedaços, para o progresso andar e o Cancelar valer no meio de arquivos grandes.
        use std::io::{Read, Write};
        let mut r = std::fs::File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
        let mut w = std::fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            if p.cancel.load(Ordering::Relaxed) {
                drop(w);
                let _ = std::fs::remove_file(dest);
                return Err("cancelado".into());
            }
            let n = r.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", dest.display()))?;
            p.done.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = std::fs::set_permissions(dest, meta.permissions());
    }
    Ok(())
}

fn remove_tree(p: &Path) -> std::io::Result<()> {
    if std::fs::symlink_metadata(p)?.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }
}

/// Copia (ou move, com `mv`) os itens numa thread. O progresso fica em `Progress`.
pub fn start(items: Vec<Item>, mv: bool) -> Arc<Progress> {
    let p = Arc::new(Progress::default());
    let pr = p.clone();
    std::thread::spawn(move || {
        let r = (|| -> Result<(), String> {
            for it in &items {
                if inside(&it.dest, &it.src) && it.src.is_dir() {
                    return Err(format!("Não dá para pôr “{}” dentro dela mesma.", it.src.file_name().unwrap_or_default().to_string_lossy()));
                }
            }
            // Mover no mesmo disco é só renomear; o resto é copiado.
            let mut to_copy = Vec::new();
            for it in &items {
                if it.replace && it.dest.exists() {
                    remove_tree(&it.dest).map_err(|e| format!("{}: {e}", it.dest.display()))?;
                }
                if mv && std::fs::rename(&it.src, &it.dest).is_ok() {
                    continue;
                }
                to_copy.push(it.clone());
            }
            pr.total.store(to_copy.iter().map(|i| tree_size(&i.src)).sum(), Ordering::Relaxed);
            for it in &to_copy {
                copy_tree(&it.src, &it.dest, &pr)?;
                if mv {
                    remove_tree(&it.src).map_err(|e| format!("{}: {e}", it.src.display()))?;
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            if e != "cancelado" {
                *pr.error.lock().unwrap() = Some(e);
            }
        }
        pr.finished.store(true, Ordering::Relaxed);
    });
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(p: &Progress) {
        while !p.finished.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn copiar_mover_e_repetidos() {
        let root = std::env::temp_dir().join(format!("glove-ops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(a.join("sub")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("sub/x.txt"), "xyz").unwrap();
        std::fs::write(a.join("f.txt"), "1").unwrap();
        std::fs::write(b.join("f.txt"), "antigo").unwrap();

        // Copiar: "f.txt" já existe em b.
        let (mut items, clashes) = plan(&[a.join("sub"), a.join("f.txt")], &b);
        assert_eq!(clashes, ["f.txt"]);
        resolve(&mut items, Conflict::KeepBoth);
        let p = start(items, false);
        wait(&p);
        assert!(p.error.lock().unwrap().is_none());
        assert_eq!(std::fs::read_to_string(b.join("sub/x.txt")).unwrap(), "xyz");
        assert_eq!(std::fs::read_to_string(b.join("f.txt")).unwrap(), "antigo");
        assert_eq!(std::fs::read_to_string(b.join("f 2.txt")).unwrap(), "1");

        // Mover substituindo.
        let (mut items, _) = plan(&[a.join("f.txt")], &b);
        resolve(&mut items, Conflict::Replace);
        let p = start(items, true);
        wait(&p);
        assert_eq!(std::fs::read_to_string(b.join("f.txt")).unwrap(), "1");
        assert!(!a.join("f.txt").exists());

        // Colar na mesma pasta duplica.
        let (items, clashes) = plan(&[b.join("f.txt")], &b);
        assert!(clashes.is_empty() && items[0].dest == b.join("f 3.txt"));

        // Pasta dentro dela mesma: erro.
        let (items, _) = plan(&[a.clone()], &a.join("sub"));
        let p = start(items, false);
        wait(&p);
        assert!(p.error.lock().unwrap().is_some());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
