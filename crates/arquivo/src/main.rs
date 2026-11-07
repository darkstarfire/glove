//! Utilitário de Arquivo (como o do 10.3): compacta e expande arquivos com uma
//! janelinha de progresso e o botão Parar. Aberto pelo Arquivos:
//!
//! - `glove-arquivo compactar <item>...`: cria "<item>.zip" (ou "Arquivo.zip"
//!   com vários itens) na pasta do primeiro item.
//! - `glove-arquivo expandir <arquivo>`: expande .zip/.tar.*/.gz/.xz/.bz2 ao
//!   lado dele. Com um só item dentro, ele fica direto na pasta; com vários,
//!   vão para uma pasta com o nome do arquivo.
//!
//! O trabalho pesado é do `zip`, `unzip`, `tar` e companhia; aqui só contamos
//! as linhas que eles escrevem para saber o andamento.

use glove_config::tr;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::paint::icons::Icon;
use glove_kit::{run, ui::AlertKind, App, Ui, WindowOptions};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Compress,
    Expand,
}

#[derive(Default)]
struct Progress {
    done: usize,
    total: usize,
    stop: bool,
    /// Fim: Ok(resultado) ou Err(mensagem). Parado pelo usuário = Ok(None).
    result: Option<Result<Option<PathBuf>, String>>,
}

struct Arquivo {
    op: Op,
    /// Nome mostrado ("Fotos", "3 itens", "fotos.zip").
    what: String,
    shared: Arc<Mutex<Progress>>,
}

/// Tipo de arquivo que sabemos expandir.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Zip,
    Tar,
    /// Um só arquivo comprimido (programa que descomprime).
    Single(&'static str),
}

fn format_of(p: &Path) -> Option<(Format, String)> {
    let name = p.file_name()?.to_string_lossy().into_owned();
    let low = name.to_lowercase();
    const TARS: [&str; 9] = [".tar", ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tar.zst", ".tzst"];
    if let Some(ext) = TARS.iter().find(|e| low.ends_with(*e)) {
        return Some((Format::Tar, name[..name.len() - ext.len()].to_string()));
    }
    for (ext, f) in [(".zip", Format::Zip), (".gz", Format::Single("gzip")), (".xz", Format::Single("xz")), (".bz2", Format::Single("bzip2")), (".zst", Format::Single("zstd"))] {
        if low.ends_with(ext) && low.len() > ext.len() {
            return Some((f, name[..name.len() - ext.len()].to_string()));
        }
    }
    None
}

/// "Fotos.zip" livre na pasta: "Fotos 2.zip", "Fotos 3.zip"…
fn free_name(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}{ext}"));
    let mut n = 2;
    while p.symlink_metadata().is_ok() {
        p = dir.join(format!("{stem} {n}{ext}"));
        n += 1;
    }
    p
}

/// Quantos itens (arquivos e pastas) há em `p`, contando ele.
fn count_items(p: &Path) -> usize {
    let mut n = 1;
    if p.symlink_metadata().is_ok_and(|m| m.is_dir()) {
        for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
            n += count_items(&e.path());
        }
    }
    n
}

/// Roda o comando contando as linhas da saída; para no pedido do usuário.
fn run_counting(mut cmd: Command, shared: &Arc<Mutex<Progress>>, counts: impl Fn(&str) -> bool) -> Result<bool, String> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| tr!("Não foi possível iniciar: {}", e))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let errs = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(err), &mut s);
        s
    });
    // A leitura não para sozinha se o usuário pedir; outra thread mata o processo.
    let pid = child.id();
    let watcher = {
        let shared = shared.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(100));
            let p = shared.lock().unwrap();
            if p.result.is_some() {
                return;
            }
            if p.stop {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                return;
            }
        })
    };
    for line in BufReader::new(out).lines().map_while(Result::ok) {
        if counts(&line) {
            let mut p = shared.lock().unwrap();
            p.done = (p.done + 1).min(p.total.max(1));
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let stopped = shared.lock().unwrap().stop;
    if stopped {
        drop(watcher);
        return Ok(false);
    }
    if !status.success() {
        let msg = errs.join().unwrap_or_default();
        let msg = msg.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(tr!("o programa falhou")).trim().to_string();
        return Err(msg);
    }
    Ok(true)
}

fn compress(items: &[PathBuf], shared: &Arc<Mutex<Progress>>) -> Result<Option<PathBuf>, String> {
    let dir = items[0].parent().unwrap_or(Path::new("/")).to_path_buf();
    let stem = if items.len() == 1 { items[0].file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { "Arquivo".to_string() };
    shared.lock().unwrap().total = items.iter().map(|p| count_items(p)).sum();
    // O zip escreve numa pasta escondida (o arquivo pela metade e os temporários
    // dele não aparecem no Arquivos) e só no fim o .zip vai para o lugar.
    let tmp = free_name(&dir, &format!(".glove-compactando-{}", std::process::id()), "");
    std::fs::create_dir(&tmp).map_err(|e| tr!("Não foi possível escrever em “{}”: {}", dir.display(), e))?;
    let part = tmp.join("a.zip");
    let mut cmd = Command::new("zip");
    // -r: pastas inteiras; -y: atalhos como atalhos; -b: temporários na pasta escondida.
    cmd.current_dir(&dir).arg("-r").arg("-y").arg("-b").arg(&tmp).arg(&part);
    for p in items {
        cmd.arg(p.file_name().unwrap_or_default());
    }
    let r = match run_counting(cmd, shared, |l| l.trim_start().starts_with("adding:")) {
        Ok(true) => {
            let dest = free_name(&dir, &stem, ".zip");
            std::fs::rename(&part, &dest).map(|_| Some(dest)).map_err(|e| e.to_string())
        }
        Ok(false) => Ok(None),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

/// Lista de itens de um .zip/.tar (para saber o total).
fn list_count(fmt: Format, archive: &Path) -> usize {
    let out = match fmt {
        Format::Zip => Command::new("unzip").arg("-Z1").arg(archive).output(),
        Format::Tar => Command::new("tar").arg("-tf").arg(archive).output(),
        Format::Single(_) => return 1,
    };
    out.map(|o| String::from_utf8_lossy(&o.stdout).lines().count()).unwrap_or(0)
}

fn expand(archive: &Path, shared: &Arc<Mutex<Progress>>) -> Result<Option<PathBuf>, String> {
    let (fmt, stem) = format_of(archive).ok_or(tr!("Este tipo de arquivo não pode ser expandido."))?;
    let dir = archive.parent().unwrap_or(Path::new("/")).to_path_buf();
    // Expande numa pasta escondida e só no fim põe no lugar (Parar não deixa restos).
    let tmp = free_name(&dir, &format!(".glove-expandindo-{}", std::process::id()), "");
    std::fs::create_dir(&tmp).map_err(|e| tr!("Não foi possível escrever em “{}”: {}", dir.display(), e))?;
    shared.lock().unwrap().total = list_count(fmt, archive).max(1);
    let r = match fmt {
        Format::Zip => {
            let mut c = Command::new("unzip");
            c.arg("-o").arg(archive).arg("-d").arg(&tmp);
            run_counting(c, shared, |l| {
                let l = l.trim_start();
                ["inflating:", "extracting:", "creating:", "linking:"].iter().any(|k| l.starts_with(k))
            })
        }
        Format::Tar => {
            let mut c = Command::new("tar");
            c.arg("-xvf").arg(archive).arg("-C").arg(&tmp);
            run_counting(c, shared, |_| true)
        }
        Format::Single(prog) => {
            let out = std::fs::File::create(tmp.join(&stem)).map_err(|e| e.to_string())?;
            let mut c = Command::new(prog);
            c.arg("-dc").arg(archive);
            let mut child = c.stdout(out).stderr(Stdio::piped()).spawn().map_err(|e| tr!("Não foi possível iniciar “{}”: {}", prog, e))?;
            let ok = child.wait().map_err(|e| e.to_string())?.success();
            shared.lock().unwrap().done = 1;
            if ok { Ok(true) } else { Err(tr!("“{}” não conseguiu descomprimir o arquivo", prog)) }
        }
    };
    let done = match r {
        Ok(true) => place(&tmp, &dir, &stem),
        Ok(false) => Ok(None),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_dir_all(&tmp);
    done
}

/// Põe o que foi expandido no lugar: um item sozinho vai direto para a pasta;
/// vários vão juntos numa pasta com o nome do arquivo.
fn place(tmp: &Path, dir: &Path, stem: &str) -> Result<Option<PathBuf>, String> {
    let inside: Vec<PathBuf> = std::fs::read_dir(tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    let (from, name) = match inside.as_slice() {
        [one] => (one.clone(), one.file_name().unwrap_or_default().to_string_lossy().into_owned()),
        _ => (tmp.to_path_buf(), stem.to_string()),
    };
    // Mantém a extensão no nome livre: "foto.png" → "foto 2.png".
    let p = Path::new(&name);
    let (s, e) = match (p.file_stem(), p.extension()) {
        (Some(s), Some(e)) if from != tmp && from.is_file() => (s.to_string_lossy().into_owned(), format!(".{}", e.to_string_lossy())),
        _ => (name.clone(), String::new()),
    };
    let dest = free_name(dir, &s, &e);
    std::fs::rename(&from, &dest).map_err(|e| e.to_string())?;
    Ok(Some(dest))
}

impl App for Arquivo {
    fn ui(&mut self, ui: &mut Ui) {
        let (done, total, result) = {
            let p = self.shared.lock().unwrap();
            (p.done, p.total, p.result.clone())
        };
        if let Some(Err(e)) = &result {
            let title = match self.op {
                Op::Compress => tr!("Não foi possível compactar “{}”.", self.what),
                Op::Expand => tr!("Não foi possível expandir “{}”.", self.what),
            };
            if ui.alert_window(AlertKind::Caution, &title, e, &["OK"]).is_some() {
                ui.close = true;
            }
            return;
        }
        if result.is_some() {
            ui.close = true;
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        let icon = if self.op == Op::Compress { Icon::ArchiveFile } else { Icon::Folder };
        glove_kit::paint::icons::draw(ui.c, ui.fonts, icon, 16.0, 16.0, 32.0, None);
        let verb = if self.op == Op::Compress { "Compactando" } else { "Expandindo" };
        // Nome comprido perde o fim ("Compactando “Relatório do ano pass…”").
        let mut name: Vec<char> = self.what.chars().collect();
        let mut shown = format!("{verb} “{}”", self.what);
        while ui.text_width(&shown, 13.0, true) > w - 80.0 && name.len() > 4 {
            name.pop();
            shown = format!("{verb} “{}…”", name.iter().collect::<String>().trim_end());
        }
        ui.fonts.draw(ui.c, &shown, 60.0, 26.0, 13.0, true, glove_kit::paint::draw::rgba(20, 20, 20, 255));
        let value = (total > 0).then(|| done as f32 / total as f32);
        ui.progress_in(glove_kit::Rect::new(60.0, 36.0, w - 140.0, 12.0), value);
        let detail = if total > 0 { tr!("{} de {} itens", done, total) } else { "Preparando…".into() };
        ui.fonts.draw(ui.c, &detail, 60.0, 66.0, 11.0, false, glove_kit::paint::draw::rgba(90, 90, 90, 255));
        ui.area(glove_kit::Rect::new(w - 72.0, 30.0, 60.0, h), |ui| {
            if ui.small_button(tr!("Parar")) {
                self.shared.lock().unwrap().stop = true;
            }
        });
        ui.wake_after(Duration::from_millis(120));
    }
}

pub fn main() {
    let mut args = std::env::args_os().skip(1);
    let op = match args.next().and_then(|a| a.into_string().ok()).as_deref() {
        Some("compactar") => Op::Compress,
        Some("expandir") => Op::Expand,
        _ => {
            eprintln!("uso: glove-arquivo compactar <item>...\n      glove-arquivo expandir <arquivo>");
            std::process::exit(2);
        }
    };
    let paths: Vec<PathBuf> = args.map(PathBuf::from).map(|p| std::path::absolute(&p).unwrap_or(p)).collect();
    if paths.is_empty() || (op == Op::Expand && paths.len() != 1) {
        eprintln!("glove-arquivo: diga o que {}", if op == Op::Compress { "compactar" } else { "expandir (um arquivo)" });
        std::process::exit(2);
    }
    let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    let what = if paths.len() == 1 { name(&paths[0]) } else { tr!("{} itens", paths.len()) };
    let shared = Arc::new(Mutex::new(Progress::default()));
    {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let r = match op {
                Op::Compress => compress(&paths, &shared),
                Op::Expand => expand(&paths[0], &shared),
            };
            shared.lock().unwrap().result = Some(r);
        });
    }
    let opts = WindowOptions { title: tr!("Utilitário de Arquivo").into(), app_id: "glove.Arquivo".into(), size: (420, 84), fixed: true, min_size: None };
    if let Err(e) = run(opts, Arquivo { op, what, shared }) {
        eprintln!("glove-arquivo: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        assert_eq!(format_of(Path::new("/a/fotos.zip")), Some((Format::Zip, "fotos".into())));
        assert_eq!(format_of(Path::new("x.tar.gz")), Some((Format::Tar, "x".into())));
        assert_eq!(format_of(Path::new("x.TGZ")), Some((Format::Tar, "x".into())));
        assert_eq!(format_of(Path::new("log.txt.gz")), Some((Format::Single("gzip"), "log.txt".into())));
        assert_eq!(format_of(Path::new("nada.txt")), None);
    }

    /// Compacta e expande de verdade (precisa do zip/unzip/tar instalados).
    #[test]
    fn ida_e_volta() {
        if Command::new("zip").arg("-v").output().is_err() || Command::new("unzip").arg("-v").output().is_err() {
            return;
        }
        let root = std::env::temp_dir().join(format!("glove-arquivo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Fotos/sub")).unwrap();
        std::fs::write(root.join("Fotos/a.txt"), "a").unwrap();
        std::fs::write(root.join("Fotos/sub/b.txt"), "b").unwrap();
        std::fs::write(root.join("solto.txt"), "c").unwrap();
        let sh = Arc::new(Mutex::new(Progress::default()));
        let zip = compress(&[root.join("Fotos")], &sh).unwrap().unwrap();
        assert_eq!(zip, root.join("Fotos.zip"));
        assert_eq!(sh.lock().unwrap().done, 4);
        // Expandir de novo não sobrescreve: vira "Fotos 2".
        let sh = Arc::new(Mutex::new(Progress::default()));
        let out = expand(&zip, &sh).unwrap().unwrap();
        assert_eq!(out, root.join("Fotos 2"));
        assert_eq!(std::fs::read_to_string(out.join("sub/b.txt")).unwrap(), "b");
        // Vários itens: Arquivo.zip, que expande numa pasta "Arquivo 2"… e nada sobra escondido.
        let sh = Arc::new(Mutex::new(Progress::default()));
        let zip = compress(&[root.join("Fotos"), root.join("solto.txt")], &sh).unwrap().unwrap();
        assert_eq!(zip, root.join("Arquivo.zip"));
        let out = expand(&zip, &Arc::new(Mutex::new(Progress::default()))).unwrap().unwrap();
        assert_eq!(out, root.join("Arquivo"));
        assert!(out.join("solto.txt").is_file());
        // Um arquivo sozinho expande ao lado, com nome livre.
        let zip = compress(&[root.join("solto.txt")], &Arc::new(Mutex::new(Progress::default()))).unwrap().unwrap();
        let out = expand(&zip, &Arc::new(Mutex::new(Progress::default()))).unwrap().unwrap();
        assert_eq!(out, root.join("solto 2.txt"));
        assert!(!std::fs::read_dir(&root).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with('.')));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
