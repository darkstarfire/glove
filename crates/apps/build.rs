//! Cria, ao lado do executável, os atalhos glove-files, glove-prefs… para o
//! glove-apps (assim `target/release/glove-files` continua funcionando).

const APPS: &[&str] = &["glove-about", "glove-agenda", "glove-ajuda", "glove-arquivo", "glove-boasvindas", "glove-calc", "glove-chars", "glove-chaves", "glove-chess", "glove-clock", "glove-contatos", "glove-mensagens", "glove-fotos", "glove-gravador", "glove-central", "glove-copias", "glove-correio", "glove-dicionario", "glove-disks", "glove-edit", "glove-files", "glove-find", "glove-fontes", "glove-info", "glove-login", "glove-monitor", "glove-musica", "glove-navegador", "glove-notes", "glove-prefs", "glove-registros", "glove-sair", "glove-shot", "glove-term", "glove-utilrede", "glove-video", "glove-view"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let Some(out) = std::env::var_os("OUT_DIR") else { return };
    // OUT_DIR = target/<perfil>/build/glove-apps-*/out
    let Some(dir) = std::path::Path::new(&out).ancestors().nth(3) else { return };
    for app in APPS {
        let link = dir.join(app);
        let is_link = std::fs::symlink_metadata(&link).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if !is_link {
            let _ = std::fs::remove_file(&link);
            let _ = std::os::unix::fs::symlink("glove-apps", &link);
        }
    }
}
