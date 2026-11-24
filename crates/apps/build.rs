//! Cria, ao lado do executável, os atalhos glove-files, glove-prefs… para o
//! glove-apps (assim `target/release/glove-files` continua funcionando).

const APPS: &[&str] = &["glove-about", "glove-calendar", "glove-help", "glove-archive", "glove-welcome", "glove-calc", "glove-chars", "glove-keychain", "glove-chess", "glove-clock", "glove-contacts", "glove-installer", "glove-messages", "glove-photos", "glove-voicerec", "glove-screenrec", "glove-appcenter", "glove-backup", "glove-mail", "glove-dictionary", "glove-disks", "glove-edit", "glove-files", "glove-find", "glove-fonts", "glove-info", "glove-login", "glove-monitor", "glove-music", "glove-browser", "glove-notes", "glove-prefs", "glove-console", "glove-logout", "glove-auth", "glove-shot", "glove-term", "glove-netutil", "glove-video", "glove-view"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let Some(out) = std::env::var_os("OUT_DIR") else { return };
    // OUT_DIR = target/<perfil>/build/glove-apps-*/out (ou build/glove-apps/*/out nos
    // Cargos mais novos): a pasta do perfil é a que contém a "build".
    let Some(dir) = std::path::Path::new(&out).ancestors().find(|a| a.file_name().is_some_and(|n| n == "build")).and_then(|b| b.parent()) else { return };
    for app in APPS {
        let link = dir.join(app);
        let is_link = std::fs::symlink_metadata(&link).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if !is_link {
            let _ = std::fs::remove_file(&link);
            let _ = std::os::unix::fs::symlink("glove-apps", &link);
        }
    }
}
