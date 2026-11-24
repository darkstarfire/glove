//! glove-apps: abre o app do Glove pelo nome com que foi chamado.

/// Os apps (o nome do atalho de cada um). O build.rs cria os atalhos.
pub const APPS: &[&str] = &["glove-about", "glove-calendar", "glove-help", "glove-archive", "glove-welcome", "glove-calc", "glove-chars", "glove-keychain", "glove-chess", "glove-clock", "glove-contacts", "glove-installer", "glove-messages", "glove-photos", "glove-voicerec", "glove-screenrec", "glove-appcenter", "glove-backup", "glove-mail", "glove-dictionary", "glove-disks", "glove-edit", "glove-files", "glove-find", "glove-fonts", "glove-info", "glove-login", "glove-monitor", "glove-music", "glove-browser", "glove-notes", "glove-prefs", "glove-console", "glove-logout", "glove-auth", "glove-shot", "glove-term", "glove-netutil", "glove-video", "glove-view"];

fn main() {
    let arg0 = std::env::args_os().next().unwrap_or_default();
    let name = std::path::Path::new(&arg0).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    match name.as_str() {
        "glove-about" => glove_about::main(),
        "glove-calendar" => glove_calendar::main(),
        "glove-help" => glove_help::main(),
        "glove-archive" => glove_archive::main(),
        "glove-welcome" => glove_welcome::main(),
        "glove-calc" => glove_calc::main(),
        "glove-chars" => glove_chars::main(),
        "glove-keychain" => glove_keychain::main(),
        "glove-chess" => glove_chess::main(),
        "glove-clock" => glove_clock::main(),
        "glove-contacts" => glove_contacts::main(),
        "glove-installer" => glove_installer::main(),
        "glove-messages" => glove_messages::main(),
        "glove-photos" => glove_photos::main(),
        "glove-voicerec" => glove_voicerec::main(),
        "glove-screenrec" => glove_screenrec::main(),
        "glove-appcenter" => glove_appcenter::main(),
        "glove-backup" => glove_backup::main(),
        "glove-mail" => glove_mail::main(),
        "glove-dictionary" => glove_dictionary::main(),
        "glove-disks" => glove_disks::main(),
        "glove-edit" => glove_edit::main(),
        "glove-files" => glove_files::main(),
        "glove-find" => glove_find::main(),
        "glove-fonts" => glove_fonts::main(),
        "glove-info" => glove_info::main(),
        "glove-login" => glove_login::main(),
        "glove-monitor" => glove_monitor::main(),
        "glove-music" => glove_music::main(),
        "glove-browser" => glove_browser::main(),
        "glove-notes" => glove_notes::main(),
        "glove-prefs" => glove_prefs::main(),
        "glove-console" => glove_console::main(),
        "glove-logout" => glove_logout::main(),
        "glove-auth" => glove_auth::main(),
        "glove-shot" => glove_shot::main(),
        "glove-term" => glove_term::main(),
        "glove-netutil" => glove_netutil::main(),
        "glove-video" => glove_video::main(),
        "glove-view" => glove_view::main(),
        _ => {
            eprintln!("glove-apps: chame por um dos atalhos ({}).", APPS.join(", "));
            std::process::exit(2);
        }
    }
}
