//! glove-apps: abre o app do Glove pelo nome com que foi chamado.

/// Os apps (o nome do atalho de cada um). O build.rs cria os atalhos.
pub const APPS: &[&str] = &["glove-about", "glove-agenda", "glove-ajuda", "glove-arquivo", "glove-boasvindas", "glove-calc", "glove-chars", "glove-chaves", "glove-chess", "glove-clock", "glove-contatos", "glove-central", "glove-copias", "glove-correio", "glove-dicionario", "glove-disks", "glove-edit", "glove-files", "glove-find", "glove-fontes", "glove-info", "glove-login", "glove-monitor", "glove-musica", "glove-navegador", "glove-notes", "glove-prefs", "glove-registros", "glove-sair", "glove-shot", "glove-term", "glove-utilrede", "glove-video", "glove-view"];

fn main() {
    let arg0 = std::env::args_os().next().unwrap_or_default();
    let name = std::path::Path::new(&arg0).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    match name.as_str() {
        "glove-about" => glove_about::main(),
        "glove-agenda" => glove_agenda::main(),
        "glove-ajuda" => glove_ajuda::main(),
        "glove-arquivo" => glove_arquivo::main(),
        "glove-boasvindas" => glove_boasvindas::main(),
        "glove-calc" => glove_calc::main(),
        "glove-chars" => glove_chars::main(),
        "glove-chaves" => glove_chaves::main(),
        "glove-chess" => glove_chess::main(),
        "glove-clock" => glove_clock::main(),
        "glove-contatos" => glove_contatos::main(),
        "glove-central" => glove_central::main(),
        "glove-copias" => glove_copias::main(),
        "glove-correio" => glove_correio::main(),
        "glove-dicionario" => glove_dicionario::main(),
        "glove-disks" => glove_disks::main(),
        "glove-edit" => glove_edit::main(),
        "glove-files" => glove_files::main(),
        "glove-find" => glove_find::main(),
        "glove-fontes" => glove_fontes::main(),
        "glove-info" => glove_info::main(),
        "glove-login" => glove_login::main(),
        "glove-monitor" => glove_monitor::main(),
        "glove-musica" => glove_musica::main(),
        "glove-navegador" => glove_navegador::main(),
        "glove-notes" => glove_notes::main(),
        "glove-prefs" => glove_prefs::main(),
        "glove-registros" => glove_registros::main(),
        "glove-sair" => glove_sair::main(),
        "glove-shot" => glove_shot::main(),
        "glove-term" => glove_term::main(),
        "glove-utilrede" => glove_utilrede::main(),
        "glove-video" => glove_video::main(),
        "glove-view" => glove_view::main(),
        _ => {
            eprintln!("glove-apps: chame por um dos atalhos ({}).", APPS.join(", "));
            std::process::exit(2);
        }
    }
}
