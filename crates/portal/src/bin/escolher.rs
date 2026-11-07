//! glove-escolher: a janela Abrir/Salvar do Glove sozinha, para outros apps (o
//! navegador, pelo portal). Escreve o caminho escolhido na saída e sai com 0;
//! cancelar ou fechar sai com 1.
//!   glove-escolher abrir|salvar|pasta [--titulo T] [--nome N] [--em PASTA] [--tipos png,jpg]

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
    let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mode = args.first().map(String::as_str).unwrap_or("abrir");
    let dir = val("--em").map(PathBuf::from);
    let types: Vec<String> = val("--tipos").map(|t| t.split(',').map(str::to_string).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let refs: Vec<&str> = types.iter().map(String::as_str).collect();
    let mut fb = match mode {
        "salvar" => FileBrowser::save(dir, &val("--nome").unwrap_or_default()),
        "pasta" => FileBrowser::folder(dir),
        _ => FileBrowser::open(dir, &refs),
    };
    fb.whole_window = true;
    let default_title = match mode {
        "salvar" => tr!("Salvar"),
        "pasta" => tr!("Escolher uma pasta"),
        _ => tr!("Abrir"),
    };
    let title = val("--titulo").filter(|t| !t.is_empty()).unwrap_or_else(|| default_title.into());
    let opts = WindowOptions { title, app_id: "glove.Escolher".into(), size: (640, 410), fixed: true, min_size: None };
    if let Err(e) = run(opts, Escolher { fb }) {
        eprintln!("glove-escolher: {e}");
    }
    std::process::exit(1);
}
