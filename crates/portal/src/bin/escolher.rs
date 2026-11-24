//! glove-escolher: a janela Abrir/Salvar do Glove sozinha, para outros apps (o
//! navegador, pelo portal). Escreve o caminho escolhido na saída e sai com 0;
//! cancelar ou fechar sai com 1.
//!   glove-escolher open|save|folder [--title T] [--name N] [--in PASTA] [--types png,jpg]

use glove_config::tr;
use std::path::PathBuf;

use glove_ui::{run, App, Choice, FileBrowser, Ui, WindowOptions};

struct Escolher {
    fb: FileBrowser,
}

impl App for Escolher {
    fn ui(&mut self, ui: &mut Ui) {
        if let Some(c) = ui.file_sheet(&mut self.fb) {
            if let Choice::Picked(p) = c {
                println!("{}", p.display());
                std::process::exit(0);
            }
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // As opções e os modos são em inglês; os de antes, em português, continuam aceitos.
    let val = |k: &str, antigo: &str| args.iter().position(|a| a == k || a == antigo).and_then(|i| args.get(i + 1)).cloned();
    let mode = match args.first().map(String::as_str).unwrap_or("open") {
        "save" | "salvar" => "save",
        "folder" | "pasta" => "folder",
        _ => "open",
    };
    let dir = val("--in", "--em").map(PathBuf::from);
    let types: Vec<String> = val("--types", "--tipos").map(|t| t.split(',').map(str::to_string).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let refs: Vec<&str> = types.iter().map(String::as_str).collect();
    let mut fb = match mode {
        "save" => FileBrowser::save(dir, &val("--name", "--nome").unwrap_or_default()),
        "folder" => FileBrowser::folder(dir),
        _ => FileBrowser::open(dir, &refs),
    };
    fb.whole_window = true;
    let default_title = match mode {
        "save" => tr!("Salvar"),
        "folder" => tr!("Escolher uma pasta"),
        _ => tr!("Abrir"),
    };
    let title = val("--title", "--titulo").filter(|t| !t.is_empty()).unwrap_or_else(|| default_title.into());
    let opts = WindowOptions { title, app_id: "glove.Escolher".into(), size: (640, 410), fixed: false, min_size: Some((480, 300)) };
    if let Err(e) = run(opts, Escolher { fb }) {
        eprintln!("glove-escolher: {e}");
    }
    std::process::exit(1);
}
