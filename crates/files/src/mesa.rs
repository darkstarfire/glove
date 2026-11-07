//! Ajudantes que a mesa (desenhada pelo compositor) chama no Arquivos:
//!
//! - `glove-files --abrir CAMINHO…`: abre como o clique duplo do Arquivos;
//! - `glove-files --transferir mover|copiar DESTINO ORIGEM…`: copia ou move com a
//!   janela de progresso e o alerta de nome repetido (só aparece se demorar);
//! - `glove-files --opcoes-mesa`: Opções de Visualização da mesa (Ctrl+J).

use glove_config::tr;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glove_kit::{run, ui::Ticks, App, Rect, Ui, WindowOptions};

use super::{kind_of, launch, open_file, ops, Entry};

/// Abre cada caminho: pasta numa janela do Arquivos, app pelo .desktop, o resto no app padrão.
pub fn open_paths(paths: &[PathBuf]) {
    for p in paths {
        let Ok(meta) = std::fs::metadata(p) else { continue };
        if meta.is_dir() {
            let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).unwrap_or_else(|| "glove-files".into());
            let _ = launch(std::process::Command::new(exe).arg(p));
            continue;
        }
        if p.extension().is_some_and(|e| e == "desktop") {
            let _ = launch(std::process::Command::new("gio").arg("launch").arg(p));
            continue;
        }
        let e = Entry {
            name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: p.clone(),
            kind: kind_of(p, false),
            size: meta.len(),
            modified: meta.modified().ok(),
            link: false,
            app_icon: None,
            label: glove_config::arquivos::label_of(p),
        };
        open_file(&e);
    }
    // As threads do `launch` recolhem os filhos; dá tempo a elas de iniciar.
    std::thread::sleep(Duration::from_millis(300));
}

/// Copiar ou mover para `dest`. Sem nomes repetidos, começa na hora e só abre a
/// janela de progresso se passar de meio segundo.
pub fn transfer(mv: bool, dest: PathBuf, srcs: Vec<PathBuf>) {
    use std::os::unix::fs::MetadataExt;
    let srcs: Vec<PathBuf> = srcs.into_iter().filter(|s| s.exists() && !dest.starts_with(s) && !(mv && s.parent() == Some(dest.as_path()))).collect();
    if srcs.is_empty() || !dest.is_dir() {
        return;
    }
    // Mover para outro disco vira copiar (como no Mac).
    let dev = |p: &Path| std::fs::metadata(p).map(|m| m.dev()).ok();
    let mv = mv && srcs.iter().all(|s| dev(s) == dev(&dest));
    let (items, clashes) = ops::plan(&srcs, &dest);
    let n = items.len();
    let mut app = Transfer { ask: None, job: None, n, mv };
    if clashes.is_empty() {
        let p = ops::start(items, mv);
        let t = Instant::now();
        while !p.finished.load(Ordering::Relaxed) && t.elapsed() < Duration::from_millis(500) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if p.finished.load(Ordering::Relaxed) && p.error.lock().unwrap().is_none() {
            return;
        }
        app.job = Some(p);
    } else {
        app.ask = Some((items, clashes));
    }
    let title = if mv { tr!("Mover") } else { tr!("Copiar") };
    let opts = WindowOptions { title: title.into(), app_id: "glove.Files".into(), size: (440, 130), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-files: {e}");
    }
}

struct Transfer {
    ask: Option<(Vec<ops::Item>, Vec<String>)>,
    job: Option<Arc<ops::Progress>>,
    n: usize,
    mv: bool,
}

impl App for Transfer {
    fn tick(&self) -> Option<Duration> {
        self.job.as_ref().map(|_| Duration::from_millis(100))
    }

    fn ui(&mut self, ui: &mut Ui) {
        use glove_kit::AlertKind;
        if let Some((_, names)) = &self.ask {
            let title = if names.len() == 1 {
                tr!("Já existe um item chamado “{}” nesta pasta.", names[0])
            } else {
                tr!("Já existem {} itens com esses nomes nesta pasta.", names.len())
            };
            let text = tr!("Deseja substituir pelo que está sendo copiado? Substituir apaga o que está lá agora.");
            match ui.alert_window(AlertKind::Caution, &title, text, &[tr!("Parar"), tr!("Manter os dois"), tr!("Substituir")]) {
                Some(0) => ui.close = true,
                Some(c) => {
                    let (mut items, _) = self.ask.take().unwrap();
                    ops::resolve(&mut items, if c == 1 { ops::Conflict::KeepBoth } else { ops::Conflict::Replace });
                    self.job = Some(ops::start(items, self.mv));
                }
                None => {}
            }
            return;
        }
        let Some(p) = self.job.clone() else {
            ui.close = true;
            return;
        };
        if p.finished.load(Ordering::Relaxed) {
            let err = p.error.lock().unwrap().clone();
            match err {
                Some(e) => {
                    if ui.alert_window(AlertKind::Stop, tr!("Não foi possível terminar."), &e, &["OK"]).is_some() {
                        ui.close = true;
                    }
                }
                None => ui.close = true,
            }
            return;
        }
        let w = ui.c.w;
        let verb = if self.mv { tr!("Movendo") } else { tr!("Copiando") };
        let label = format!("{verb} {} {}…", self.n, if self.n == 1 { "item" } else { "itens" });
        ui.fonts.draw(ui.c, &label, 20.0, 30.0, 13.0, true, glove_kit::paint::draw::rgba(20, 20, 20, 255));
        let cur = p.current.lock().unwrap().clone();
        let cur = ui.fonts.ellipsize(&cur, 11.0, false, w - 40.0);
        ui.fonts.draw(ui.c, &cur, 20.0, 50.0, 11.0, false, glove_kit::paint::draw::rgba(70, 70, 70, 255));
        let total = p.total.load(Ordering::Relaxed);
        let frac = if total == 0 { None } else { Some((p.done.load(Ordering::Relaxed) as f32 / total as f32).min(1.0)) };
        ui.progress_in(Rect::new(20.0, 66.0, w - 40.0, 12.0), frac);
        ui.area(Rect::new(w - 110.0, 92.0, 90.0, 24.0), |ui| {
            if ui.button(tr!("Parar")) {
                p.cancel.store(true, Ordering::Relaxed);
            }
        });
    }
}

/// Duplicar: "foto cópia.png" ao lado de cada item (o mesmo nome do Arquivos).
pub fn duplicate(paths: Vec<PathBuf>) {
    let items: Vec<ops::Item> = paths
        .into_iter()
        .filter(|p| p.exists())
        .map(|p| {
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            let mut dest = p.with_file_name(tr!("{} cópia{}", stem, ext));
            if dest.exists() {
                dest = ops::free_name(&dest);
            }
            ops::Item { src: p, dest, replace: false }
        })
        .collect();
    if items.is_empty() {
        return;
    }
    let n = items.len();
    let p = ops::start(items, false);
    let t = Instant::now();
    while !p.finished.load(Ordering::Relaxed) && t.elapsed() < Duration::from_millis(500) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if p.finished.load(Ordering::Relaxed) && p.error.lock().unwrap().is_none() {
        return;
    }
    let opts = WindowOptions { title: tr!("Copiar").into(), app_id: "glove.Files".into(), size: (440, 130), fixed: true, min_size: None };
    if let Err(e) = run(opts, Transfer { ask: None, job: Some(p), n, mv: false }) {
        eprintln!("glove-files: {e}");
    }
}

/// Opções de Visualização da mesa.
pub fn view_options() {
    let opts = WindowOptions { title: tr!("Mesa").into(), app_id: "glove.Files".into(), size: (300, 362), fixed: true, min_size: None };
    let config = glove_config::Config::load();
    if let Err(e) = run(opts, ViewOptions { config, pending: false }) {
        eprintln!("glove-files: {e}");
    }
}

struct ViewOptions {
    config: glove_config::Config,
    /// Mudou algo durante um arraste do slider: grava ao soltar.
    pending: bool,
}

const ARRANGE: [(&str, &str); 4] = [("nome", "Nome"), ("data", "Data de Modificação"), ("tamanho", "Tamanho"), ("tipo", "Tipo")];

impl ViewOptions {
    fn save(&mut self, ui: &Ui) {
        if ui.mouse_down() {
            self.pending = true;
            return;
        }
        self.pending = false;
        let _ = self.config.save();
    }
}

impl App for ViewOptions {
    fn config_changed(&mut self) {
        self.config = glove_config::Config::load();
    }

    fn ui(&mut self, ui: &mut Ui) {
        if self.pending && !ui.mouse_down() {
            self.pending = false;
            let _ = self.config.save();
        }
        ui.area(Rect::new(20.0, 16.0, ui.c.w - 40.0, ui.c.h - 30.0), |ui| {
            let d = self.config.desk.clone();
            ui.label(&format!("Tamanho dos ícones: {0} × {0} px", d.icon_size));
            ui.space(4.0);
            let mut size = d.icon_size as f32;
            let t = Ticks { count: 7, labels: &[(0.0, tr!("Pequeno")), (1.0, tr!("Grande"))], snap: false };
            if ui.slider_ticks(&mut size, 32.0, 128.0, 250.0, &t, true) {
                self.config.desk.icon_size = (size / 4.0).round() as u32 * 4;
                self.save(ui);
            }
            ui.space(10.0);
            ui.horizontal(|ui| {
                ui.label(tr!("Tamanho do texto:"));
                ui.spacing(8.0);
                let sizes: Vec<String> = (10..=16).map(|s| tr!("{} pt", s)).collect();
                let mut i = d.text_size.clamp(10, 16) as usize - 10;
                if ui.popup(&mut i, &sizes, 80.0) {
                    self.config.desk.text_size = 10 + i as u32;
                    self.save(ui);
                }
            });
            ui.space(10.0);
            ui.label(tr!("Posição do nome:"));
            ui.horizontal(|ui| {
                ui.spacing(14.0);
                let mut pos = d.label_right as usize;
                if ui.radio_group(&mut pos, &[tr!("Embaixo"), tr!("À direita")]) {
                    self.config.desk.label_right = pos == 1;
                    self.save(ui);
                }
            });
            ui.space(8.0);
            let mut keep = !d.arrange.is_empty();
            ui.horizontal(|ui| {
                if ui.checkbox(&mut keep, tr!("Manter organizado por")) {
                    self.config.desk.arrange = if keep { "nome".into() } else { String::new() };
                    self.save(ui);
                }
            });
            ui.indent(20.0, |ui| {
                ui.set_enabled(keep);
                let names: Vec<String> = ARRANGE.iter().map(|a| tr(a.1).to_string()).collect();
                let mut i = ARRANGE.iter().position(|a| a.0 == d.arrange).unwrap_or(0);
                if ui.popup(&mut i, &names, 180.0) && keep {
                    self.config.desk.arrange = ARRANGE[i].0.into();
                    self.save(ui);
                }
                ui.set_enabled(true);
            });
            ui.space(8.0);
            ui.separator();
            ui.space(6.0);
            ui.label(tr!("Mostrar na mesa:"));
            let mut disks = d.show_disks;
            if ui.checkbox(&mut disks, tr!("Disco do sistema")) {
                self.config.desk.show_disks = disks;
                self.save(ui);
            }
            let mut rem = d.show_removable;
            if ui.checkbox(&mut rem, tr!("Pendrives e discos externos")) {
                self.config.desk.show_removable = rem;
                self.save(ui);
            }
        });
    }
}

/// Ejetar o disco montado em `mount` (pela mesa, pela barra lateral ou pelo Lixo).
/// Só abre janela se não der certo.
pub fn eject(mount: PathBuf) {
    if let Err(e) = glove_config::discos::eject(&mount) {
        let opts = WindowOptions { title: tr!("Ejetar").into(), app_id: "glove.Files".into(), size: (440, 130), fixed: true, min_size: None };
        let _ = run(opts, Notice { kind: glove_kit::AlertKind::Stop, title: tr!("Não foi possível ejetar o disco.").into(), text: e });
    }
}

/// Aviso de disco arrancado sem ejetar (como o do Tiger).
pub fn eject_warning(name: String) {
    let opts = WindowOptions { title: tr!("Disco").into(), app_id: "glove.Files".into(), size: (460, 140), fixed: true, min_size: None };
    let _ = run(
        opts,
        Notice {
            kind: glove_kit::AlertKind::Caution,
            title: tr!("O disco “{}” não foi ejetado corretamente.", name),
            text: tr!("Se possível, ejete o disco antes de desconectá-lo ou desligá-lo. Para ejetar, arraste o disco para o Lixo ou clique em ⏏ ao lado dele no Arquivos.").into(),
        },
    );
}

struct Notice {
    kind: glove_kit::AlertKind,
    title: String,
    text: String,
}

impl App for Notice {
    fn ui(&mut self, ui: &mut Ui) {
        if ui.alert_window(self.kind, &self.title, &self.text, &["OK"]).is_some() {
            ui.close = true;
        }
    }
}
