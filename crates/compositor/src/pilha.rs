//! A pilha de Downloads no Dock (como nos Macs mais novos): um clique no ícone abre a
//! pasta em grade, acima dele, com os itens mais novos primeiro. Clicar num item abre
//! com o app do tipo (e a pilha fecha); o botão direito traz Abrir, Mostrar no Arquivos,
//! Mover para o Lixo e Apagar Imediatamente, e a pilha continua aberta, reorganizada,
//! para cuidar de vários itens seguidos. Clicar fora ou Esc fecha.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use glove_config::tr;
use glove_paint::chrome::{self, MenuItem, StackEntry, StackPart};
use glove_paint::icons;
use glove_paint::Pixmap;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Logical, Point};

use crate::shell::{to_tex, MenuAction, MenuNode, Tex};
use crate::state::Glove;

/// Lado da miniatura das imagens (em pixels da imagem).
const THUMB: u32 = 160;

pub struct Pilha {
    pub dir: PathBuf,
    /// Os itens mostrados: (caminho, nome, é pasta), do mais novo para o mais velho.
    pub items: Vec<(PathBuf, String, bool)>,
    thumbs: HashMap<PathBuf, Pixmap>,
    rx: Option<mpsc::Receiver<(PathBuf, Option<Pixmap>)>>,
    pub hover: Option<StackPart>,
    /// O canto do painel (sem a sombra) e o tamanho dele.
    pub origin: Point<i32, Logical>,
    pub size: (i32, i32),
    arrow_x: f32,
    pub tex: Option<Tex>,
    key: u64,
    /// Quando a pasta mudou pela última vez (para ver itens novos ou apagados).
    dir_mtime: Option<SystemTime>,
    checked: Instant,
    /// A confirmação de Apagar Imediatamente está aberta (desde quando): clicar nela não
    /// fecha a pilha.
    pub asking: Option<Instant>,
}

/// A pasta Downloads (a do user-dirs; senão ~/Downloads).
pub fn downloads_dir() -> PathBuf {
    glove_config::arquivos::user_dir("DOWNLOAD").unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Downloads"))
}

/// Os itens da pasta, do mais novo para o mais velho (sem os ocultos e os downloads
/// ainda pela metade).
fn listar(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(SystemTime, PathBuf, String, bool)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || [".part", ".crdownload", ".download", ".partial"].iter().any(|x| name.ends_with(x)) {
                return None;
            }
            let md = e.metadata().ok()?;
            let t = md.modified().ok().max(md.created().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
            Some((t, e.path(), name, md.is_dir()))
        })
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().take(chrome::STACK_MAX).map(|(_, p, n, d)| (p, n, d)).collect()
}

/// Miniatura de uma imagem (lida numa thread: fotos grandes não travam a tela).
fn miniatura(p: &Path) -> Option<Pixmap> {
    let img = glove_paint::image::decode(p).ok()?;
    let k = (THUMB as f32 / img.width().max(img.height()) as f32).min(1.0);
    let (w, h) = (((img.width() as f32 * k).round() as u32).max(1), ((img.height() as f32 * k).round() as u32).max(1));
    let mut out = Pixmap::new(w, h)?;
    let paint = glove_paint::tiny_skia::PixmapPaint { quality: glove_paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    out.draw_pixmap(0, 0, img.as_ref(), &paint, glove_paint::tiny_skia::Transform::from_scale(k, k), None);
    Some(out)
}

fn e_imagem(p: &Path) -> bool {
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tif" | "tiff")
}

impl Glove {
    pub fn pilha_open(&self) -> bool {
        self.pilha.is_some()
    }

    /// Clique no ícone da pilha (o item `i` do Dock): abre, ou fecha se já estava aberta.
    pub fn toggle_pilha(&mut self, i: usize) {
        if self.pilha.take().is_some() {
            self.dirty = true;
            return;
        }
        self.close_menu();
        self.close_search();
        let dir = downloads_dir();
        let Some(l) = &self.chrome.dock_layout else { return };
        let Some(&(x, y, s)) = l.items.get(i) else { return };
        let d = self.chrome.dock_pos();
        let (ix, iy) = (d.x as f32 + x, d.y as f32 + y);
        let mut p = Pilha { dir: dir.clone(), items: Vec::new(), thumbs: HashMap::new(), rx: None, hover: None, origin: (0, 0).into(), size: (0, 0), arrow_x: 0.0, tex: None, key: 0, dir_mtime: None, checked: Instant::now(), asking: None };
        self.pilha_reload(&mut p);
        // Acima do ícone, com o bico no meio dele (no Dock da lateral, ao lado dele).
        let (sw, sh) = self.output_size();
        let (pw, ph) = (p.size.0 as f32, p.size.1 as f32);
        let (ox, oy) = if self.chrome.dock_side.vertical() {
            let ox = if self.chrome.dock_side == chrome::DockSide::Left { ix + s + 14.0 } else { ix - 14.0 - pw };
            (ox, (iy + s / 2.0 - ph / 2.0).clamp(glove_paint::theme::MENUBAR_H as f32 + 6.0, (sh as f32 - ph - 6.0).max(0.0)))
        } else {
            let cx = ix + s / 2.0;
            ((cx - pw / 2.0).clamp(8.0, (sw as f32 - pw - 8.0).max(8.0)), (iy - ph - chrome::STACK_ARROW - 4.0).max(glove_paint::theme::MENUBAR_H as f32 + 6.0))
        };
        p.origin = (ox.round() as i32, oy.round() as i32).into();
        p.arrow_x = if self.chrome.dock_side.vertical() { -1000.0 } else { ix + s / 2.0 - ox.round() };
        self.pilha = Some(p);
        self.pilha_timer();
        self.dirty = true;
    }

    /// Relê a pasta (e começa as miniaturas que faltam).
    fn pilha_reload(&self, p: &mut Pilha) {
        p.items = listar(&p.dir);
        p.dir_mtime = std::fs::metadata(&p.dir).and_then(|m| m.modified()).ok();
        let (_, _, w, h) = chrome::stack_geom(p.items.len());
        p.size = (w.ceil() as i32, h.ceil() as i32);
        let falta: Vec<PathBuf> = p.items.iter().filter(|(path, _, dir)| !dir && e_imagem(path) && !p.thumbs.contains_key(path)).map(|(path, _, _)| path.clone()).collect();
        if !falta.is_empty() {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                for path in falta {
                    let t = miniatura(&path);
                    if tx.send((path, t)).is_err() {
                        break;
                    }
                }
            });
            p.rx = Some(rx);
        }
        p.hover = None;
    }

    /// Enquanto aberta: confere a pasta e as miniaturas de tempos em tempos.
    fn pilha_timer(&mut self) {
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(80)), |_, _, data| {
                let st = &mut data.state;
                if st.pilha.is_none() {
                    return TimeoutAction::Drop;
                }
                st.dirty = true;
                TimeoutAction::ToDuration(Duration::from_millis(if st.pilha.as_ref().is_some_and(|p| p.rx.is_some()) { 80 } else { 500 }))
            })
            .ok();
    }

    pub fn close_pilha(&mut self) {
        if self.pilha.take().is_some() {
            self.dirty = true;
        }
    }

    /// O painel sob o ponto: Some(parte ou nada).
    pub fn pilha_hit(&self, pos: Point<f64, Logical>) -> Option<Option<StackPart>> {
        let p = self.pilha.as_ref()?;
        let (x, y) = ((pos.x - p.origin.x as f64) as f32, (pos.y - p.origin.y as f64) as f32);
        if x < 0.0 || y < 0.0 || x >= p.size.0 as f32 || y >= p.size.1 as f32 + chrome::STACK_ARROW {
            return None;
        }
        Some(chrome::stack_hit(&self.fonts, p.items.len(), x, y))
    }

    pub fn pilha_motion(&mut self, part: Option<StackPart>) {
        if let Some(p) = &mut self.pilha {
            if p.hover != part {
                p.hover = part;
                self.dirty = true;
            }
        }
    }

    /// Clique na pilha: `right` = botão direito (o menu do item).
    pub fn pilha_press(&mut self, part: Option<StackPart>, right: bool, pos: Point<f64, Logical>) {
        let Some(p) = &self.pilha else { return };
        match part {
            Some(StackPart::Item(i)) => {
                let Some((path, _, _)) = p.items.get(i).cloned() else { return };
                if right {
                    let nodes = vec![
                        MenuNode::new(MenuItem::item(tr!("Abrir")), Some(MenuAction::StackOpen(path.clone()))),
                        MenuNode::new(MenuItem::item(tr!("Mostrar no Arquivos")), Some(MenuAction::StackReveal(path.clone()))),
                        MenuNode::sep(),
                        MenuNode::new(MenuItem::item(tr!("Mover para o Lixo")), Some(MenuAction::StackTrash(path.clone()))),
                        MenuNode::new(MenuItem::item(tr!("Apagar Imediatamente…")), Some(MenuAction::StackDelete(path))),
                    ];
                    self.show_context_menu(nodes, pos);
                } else {
                    self.pilha_action(&MenuAction::StackOpen(path));
                }
            }
            Some(StackPart::OpenFolder) if !right => {
                let dir = p.dir.clone();
                self.files_cmd(&[dir.as_os_str()]);
                self.close_pilha();
            }
            _ => {}
        }
    }

    /// Roda o Arquivos com estes argumentos.
    fn files_cmd(&self, args: &[&std::ffi::OsStr]) {
        let _ = std::process::Command::new(crate::shell::sibling_binary("glove-files")).args(args).env("WAYLAND_DISPLAY", &self.socket_name).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
    }

    /// As ações do menu de um item da pilha.
    pub fn pilha_action(&mut self, a: &MenuAction) {
        match a {
            MenuAction::StackOpen(p) => {
                self.files_cmd(&["--open".as_ref(), p.as_os_str()]);
                self.close_pilha();
            }
            MenuAction::StackReveal(p) => {
                self.files_cmd(&["--show".as_ref(), p.as_os_str()]);
                self.close_pilha();
            }
            // A pilha fica aberta: quando a pasta muda, ela se reorganiza.
            MenuAction::StackTrash(p) => self.files_cmd(&["--trash".as_ref(), p.as_os_str()]),
            MenuAction::StackDelete(p) => {
                self.files_cmd(&["--delete".as_ref(), p.as_os_str()]);
                if let Some(s) = &mut self.pilha {
                    s.asking = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    pub fn prepare_pilha(&mut self) {
        let sc = self.scale() as f32;
        let dark = self.theme.dark;
        let tgen = self.theme_gen;
        // A confirmação de apagar fechou (ou nem abriu): os cliques fora voltam a fechar a pilha.
        let pergunta = tr!("Apagar Imediatamente");
        let aberta = self.space.elements().any(|w| crate::shell::window_info(w).0 == pergunta);
        let Some(mut p) = self.pilha.take() else { return };
        if p.asking.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) && !aberta {
            p.asking = None;
        }
        if let Some(rx) = &p.rx {
            loop {
                match rx.try_recv() {
                    Ok((path, Some(t))) => {
                        p.thumbs.insert(path, t);
                    }
                    Ok((_, None)) => {}
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        p.rx = None;
                        break;
                    }
                }
            }
        }
        // A pasta mudou (um download chegou, um item foi para o Lixo): relê.
        if p.checked.elapsed() >= Duration::from_millis(400) {
            p.checked = Instant::now();
            let m = std::fs::metadata(&p.dir).and_then(|m| m.modified()).ok();
            if m != p.dir_mtime || p.items.iter().any(|(path, _, _)| path.symlink_metadata().is_err()) {
                let (old_w, old_h) = p.size;
                self.pilha_reload(&mut p);
                // Continua com o bico no mesmo ícone (o painel cresce para cima).
                let dx = (p.size.0 - old_w) / 2;
                p.origin.x -= dx;
                p.arrow_x += dx as f32;
                p.origin.y -= p.size.1 - old_h;
            }
        }
        use std::hash::{Hash, Hasher};
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (p.items.len(), p.items.first().map(|i| i.0.clone()), p.thumbs.len(), p.hover.map(|h| format!("{h:?}")), dark, tgen, sc.to_bits(), p.arrow_x.to_bits()).hash(&mut k);
        for (path, _, _) in &p.items {
            path.hash(&mut k);
        }
        let key = k.finish();
        if p.tex.is_none() || p.key != key {
            let entries: Vec<StackEntry> = p.items.iter().map(|(path, name, dir)| StackEntry { icon: icons::file_icon(path, *dir), name: name.clone(), thumb: p.thumbs.get(path) }).collect();
            let title = p.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| tr!("Downloads").to_string());
            p.tex = chrome::stack_grid(&self.fonts, &title, &entries, p.hover, p.arrow_x, dark, sc).map(to_tex);
            p.key = key;
        }
        self.pilha = Some(p);
    }

    /// Um clique nesta janela deixa a pilha aberta (a confirmação de apagar, do Arquivos).
    pub fn pilha_keeps_for(&self, w: &smithay::desktop::Window) -> bool {
        self.pilha.as_ref().is_some_and(|p| p.asking.is_some()) && crate::shell::window_info(w).1 == "glove.Files"
    }

    /// O canto da textura da pilha (com a sombra).
    pub fn pilha_tex_pos(&self) -> Option<(Point<i32, Logical>, &Tex)> {
        let p = self.pilha.as_ref()?;
        let m = chrome::STACK_SHADOW as i32;
        Some(((p.origin.x - m, p.origin.y - m).into(), p.tex.as_ref()?))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mais_novos_primeiro_sem_ocultos() {
        let d = std::env::temp_dir().join(format!("glove-pilha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // Na ordem em que "chegaram" (vale a data mais nova: a de chegada ou a de mudança).
        for n in ["velho.txt", "meio.pdf", "novo.png", ".oculto", "baixando.zip.part"] {
            std::fs::write(d.join(n), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let v: Vec<String> = super::listar(&d).into_iter().map(|i| i.1).collect();
        assert_eq!(v, ["novo.png", "meio.pdf", "velho.txt"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
