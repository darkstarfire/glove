//! A pilha de Downloads no Dock (como nos Macs mais novos): um clique no ícone abre a
//! pasta acima dele, com os itens mais novos primeiro, em leque (um sobre o outro numa
//! curva leve, rolando com a roda ou o touchpad) ou em grade (um painel que rola, com um
//! campo de busca: o que se digita com a pilha aberta vai para ele). Clicar num item
//! abre com o app do tipo (e a pilha fecha); o botão direito traz Abrir, Mostrar no
//! Arquivos, Mover para o Lixo e Apagar Imediatamente, e a pilha continua aberta,
//! reorganizada, para cuidar de vários itens seguidos. Clicar fora ou Esc fecha.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use glove_config::tr;
use glove_paint::chrome::{self, MenuItem, StackEntry, StackGridState, StackPart};
use glove_paint::icons;
use glove_paint::Pixmap;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Logical, Point};

use crate::lock::LockKey;
use crate::shell::{to_tex, MenuAction, MenuNode, Tex};
use crate::state::Glove;

/// Lado da miniatura das imagens (em pixels da imagem).
const THUMB: u32 = 160;
/// Quantos itens a pilha lê da pasta (os mais novos).
const LIMITE: usize = 5000;
/// Miniaturas feitas de uma vez (as dos itens mais novos).
const MINIATURAS: usize = 300;
/// O leque abrindo a partir do ícone.
const ABRIR: Duration = Duration::from_millis(220);

pub struct Pilha {
    pub dir: PathBuf,
    /// Os itens da pasta: (caminho, nome, é pasta), do mais novo para o mais velho.
    pub items: Vec<(PathBuf, String, bool)>,
    /// Os mostrados (índices em `items`): todos, ou os que a busca acha.
    shown: Vec<usize>,
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
    /// Em leque (senão, em grade).
    pub fan: bool,
    /// O ícone da pilha no Dock (x, y, lado, na tela) e o monitor desse Dock (cada
    /// monitor tem o seu: a pilha abre no do clique).
    icon: (f32, f32, f32),
    area: smithay::utils::Rectangle<i32, Logical>,
    /// Leque: os lugares, o primeiro item à vista (com fração enquanto desliza) e para
    /// onde ele vai.
    slots: usize,
    bend: f32,
    off: f32,
    off_to: f32,
    opened: Instant,
    /// Grade: a busca e a rolagem (atual e para onde vai).
    pub query: String,
    scroll: f32,
    scroll_to: f32,
    stepped: Instant,
    /// Quantas janelas havia ao abrir: uma janela nova fecha a pilha (o teclado da busca
    /// não pode continuar preso nela).
    windows: usize,
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
    v.into_iter().take(LIMITE).map(|(_, p, n, d)| (p, n, d)).collect()
}

/// Para a busca: minúsculas e sem acentos ("Relatório" acha com "relatorio").
fn dobrar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// Os índices dos itens cujo nome tem a busca (todos, sem busca).
fn filtrar(items: &[(PathBuf, String, bool)], query: &str) -> Vec<usize> {
    let q = dobrar(query.trim());
    items.iter().enumerate().filter(|(_, (_, n, _))| q.is_empty() || dobrar(n).contains(&q)).map(|(i, _)| i).collect()
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

impl Pilha {
    /// Algo ainda se mexe (o leque abrindo, a rolagem deslizando).
    fn animating(&self) -> bool {
        (self.fan && self.opened.elapsed() < ABRIR) || (self.off - self.off_to).abs() > 0.002 || (self.scroll - self.scroll_to).abs() > 0.3
    }

    /// O item mostrado na posição `i` (da lista mostrada).
    fn item(&self, i: usize) -> Option<&(PathBuf, String, bool)> {
        self.shown.get(i).and_then(|&j| self.items.get(j))
    }
}

impl Glove {
    pub fn pilha_open(&self) -> bool {
        self.pilha.is_some()
    }

    /// A pilha está em grade (e o teclado vai para a busca dela).
    pub fn pilha_grid(&self) -> bool {
        self.pilha.as_ref().is_some_and(|p| !p.fan)
    }

    /// Clique no ícone da pilha (o item `i` do Dock): abre, ou fecha se já estava aberta.
    pub fn toggle_pilha(&mut self, i: usize) {
        if self.pilha.take().is_some() {
            self.dirty = true;
            return;
        }
        self.close_menu();
        self.close_search();
        let Some(l) = &self.chrome.dock_layout else { return };
        let Some(&(x, y, s)) = l.items.get(i) else { return };
        // O Dock do clique: o do monitor com o mouse (o vivo), não o do principal.
        let principal = self.primary_rect().unwrap_or_else(|| smithay::utils::Rectangle::from_size(self.output_size().into()));
        let area = self.live_dock_area().unwrap_or(principal);
        let d = if area == principal { self.chrome.dock_pos() } else { self.extra_dock_origin(area).unwrap_or_else(|| self.chrome.dock_pos()) };
        let icon = (d.x as f32 + x, d.y as f32 + y, s);
        // No Dock da lateral o leque não cabe: grade, como no Mac.
        let fan = self.config.dock.stack_view != "grid" && !self.chrome.dock_side.vertical();
        let agora = Instant::now();
        let mut p = Pilha {
            dir: downloads_dir(),
            items: Vec::new(),
            shown: Vec::new(),
            thumbs: HashMap::new(),
            rx: None,
            hover: None,
            origin: (0, 0).into(),
            size: (0, 0),
            arrow_x: 0.0,
            tex: None,
            key: 0,
            dir_mtime: None,
            checked: agora,
            asking: None,
            fan,
            icon,
            area,
            slots: 0,
            bend: chrome::FAN_BEND,
            off: 0.0,
            off_to: 0.0,
            opened: agora,
            query: String::new(),
            scroll: 0.0,
            scroll_to: 0.0,
            stepped: agora,
            windows: self.space.elements().count(),
        };
        self.pilha_reload(&mut p);
        self.pilha = Some(p);
        self.pilha_timer();
        self.dirty = true;
    }

    /// Relê a pasta (e começa as miniaturas que faltam), e põe o painel no lugar.
    fn pilha_reload(&self, p: &mut Pilha) {
        p.items = listar(&p.dir);
        p.shown = filtrar(&p.items, &p.query);
        p.dir_mtime = std::fs::metadata(&p.dir).and_then(|m| m.modified()).ok();
        self.pilha_place(p);
        let falta: Vec<PathBuf> =
            p.items.iter().take(MINIATURAS).filter(|(path, _, dir)| !dir && e_imagem(path) && !p.thumbs.contains_key(path)).map(|(path, _, _)| path.clone()).collect();
        if !falta.is_empty() && p.rx.is_none() {
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
        self.pilha_clamp(p);
    }

    /// O tamanho e o lugar do painel: o leque cresce do ícone para cima; a grade fica
    /// acima do ícone, com o bico no meio dele (no Dock da lateral, ao lado dele).
    fn pilha_place(&self, p: &mut Pilha) {
        // Os limites: o monitor do Dock do clique.
        let a = p.area;
        let (ax, ay) = (a.loc.x as f32, a.loc.y as f32);
        let (sw, sh) = (a.loc.x + a.size.w, a.loc.y + a.size.h);
        let (ix, iy, s) = p.icon;
        let menubar = ay + glove_paint::theme::MENUBAR_H as f32 + 6.0;
        if p.fan {
            p.slots = chrome::fan_slots(p.items.len(), iy - menubar);
            // A curva vai para a direita: perto da borda da tela, ela se curva menos.
            p.bend = chrome::fan_bend_for(p.slots, sw as f32 - 4.0 - (ix + s / 2.0));
            let (w, h, ax) = chrome::fan_geom(p.slots, p.bend);
            p.size = (w.ceil() as i32, h.ceil() as i32);
            let ox = (ix + s / 2.0 - ax).min(sw as f32 - w - 4.0);
            p.origin = (ox.round() as i32, (iy - 4.0 - h).round() as i32).into();
            return;
        }
        let (_, _, w, h) = chrome::stack_geom(p.items.len());
        p.size = (w.ceil() as i32, h.ceil() as i32);
        let (pw, ph) = (w, h);
        let (ox, oy) = if self.chrome.dock_side.vertical() {
            let ox = if self.chrome.dock_side == chrome::DockSide::Left { ix + s + 14.0 } else { ix - 14.0 - pw };
            (ox, (iy + s / 2.0 - ph / 2.0).clamp(menubar, (sh as f32 - ph - 6.0).max(0.0)))
        } else {
            let cx = ix + s / 2.0;
            ((cx - pw / 2.0).clamp(ax + 8.0, (sw as f32 - pw - 8.0).max(ax + 8.0)), (iy - ph - chrome::STACK_ARROW - 4.0).max(menubar))
        };
        p.origin = (ox.round() as i32, oy.round() as i32).into();
        p.arrow_x = if self.chrome.dock_side.vertical() { -1000.0 } else { ix + s / 2.0 - ox.round() };
    }

    /// A rolagem dentro dos limites (a lista pode ter encolhido).
    fn pilha_clamp(&self, p: &mut Pilha) {
        let max_off = p.items.len().saturating_sub(p.slots) as f32;
        p.off_to = p.off_to.clamp(0.0, max_off);
        p.off = p.off.clamp(0.0, max_off);
        let max = chrome::stack_max_scroll(p.items.len(), p.shown.len());
        p.scroll_to = p.scroll_to.clamp(0.0, max);
        p.scroll = p.scroll.clamp(0.0, max);
    }

    /// Enquanto aberta: confere a pasta e as miniaturas de tempos em tempos (e a cada
    /// quadro enquanto algo se mexe).
    fn pilha_timer(&mut self) {
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
                let st = &mut data.state;
                let Some(p) = st.pilha.as_ref() else { return TimeoutAction::Drop };
                let ms = if p.animating() {
                    16
                } else if p.rx.is_some() {
                    80
                } else {
                    500
                };
                st.dirty = true;
                TimeoutAction::ToDuration(Duration::from_millis(ms))
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
        if p.fan {
            if x < 0.0 || y < 0.0 || x >= p.size.0 as f32 || y >= p.size.1 as f32 {
                return None;
            }
            let names: Vec<String> = p.items.iter().map(|i| i.1.clone()).collect();
            // Só os itens e as etiquetas contam: o vão entre eles deixa passar o clique.
            return chrome::fan_hit(&self.fonts, &names, p.slots, p.bend, p.off, x, y).map(Some);
        }
        if x < 0.0 || y < 0.0 || x >= p.size.0 as f32 || y >= p.size.1 as f32 + chrome::STACK_ARROW {
            return None;
        }
        Some(chrome::stack_hit(&self.fonts, p.items.len(), p.shown.len(), p.scroll, x, y))
    }

    /// O ponto está na área da pilha (no leque, também no vão entre os itens: a rolagem
    /// vale ali).
    pub fn pilha_over(&self, pos: Point<f64, Logical>) -> bool {
        let Some(p) = self.pilha.as_ref() else { return false };
        let (x, y) = (pos.x - p.origin.x as f64, pos.y - p.origin.y as f64);
        x >= 0.0 && y >= 0.0 && x < p.size.0 as f64 && y < p.size.1 as f64 + chrome::STACK_ARROW as f64
    }

    pub fn pilha_motion(&mut self, part: Option<StackPart>) {
        if let Some(p) = &mut self.pilha {
            if p.hover != part {
                p.hover = part;
                self.dirty = true;
            }
        }
    }

    /// A roda ou o touchpad sobre a pilha: o leque desliza os itens pela curva; a grade
    /// rola.
    pub fn pilha_scroll(&mut self, dy: f64) {
        let Some(mut p) = self.pilha.take() else { return };
        if p.fan {
            p.off_to += dy as f32 / 18.0;
        } else {
            p.scroll_to += dy as f32 * 2.5;
        }
        self.pilha_clamp(&mut p);
        // Durante a rolagem, o que fica sob o mouse muda.
        p.hover = None;
        self.pilha = Some(p);
        self.pilha_timer();
        self.dirty = true;
    }

    /// O teclado com a grade aberta: a busca (Enter abre o primeiro item achado; Esc
    /// limpa a busca e, vazia, fecha).
    pub fn pilha_key(&mut self, k: LockKey) {
        let Some(p) = &mut self.pilha else { return };
        match k {
            LockKey::Char(c) if !c.is_control() && p.query.chars().count() < 120 => p.query.push(c),
            LockKey::Backspace => {
                p.query.pop();
            }
            LockKey::Enter => {
                if let Some((path, _, _)) = p.item(0).cloned() {
                    self.pilha_action(&MenuAction::StackOpen(path));
                }
                return;
            }
            LockKey::Escape => {
                if p.query.is_empty() {
                    self.close_pilha();
                    return;
                }
                p.query.clear();
            }
            _ => return,
        }
        let Some(mut p) = self.pilha.take() else { return };
        p.shown = filtrar(&p.items, &p.query);
        p.scroll = 0.0;
        p.scroll_to = 0.0;
        p.hover = None;
        self.pilha = Some(p);
        self.dirty = true;
    }

    /// Clique na pilha: `right` = botão direito (o menu do item).
    pub fn pilha_press(&mut self, part: Option<StackPart>, right: bool, pos: Point<f64, Logical>) {
        let Some(p) = &self.pilha else { return };
        match part {
            Some(StackPart::Item(i)) => {
                let item = if p.fan { p.items.get(i) } else { p.item(i) };
                let Some((path, _, _)) = item.cloned() else { return };
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
            MenuAction::StackView(fan) => {
                self.config.dock.stack_view = if *fan { "fan" } else { "grid" }.into();
                let _ = self.config.save();
                self.close_pilha();
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
        let janelas = self.space.elements().count();
        let Some(mut p) = self.pilha.take() else { return };
        // Uma janela nova (que não a confirmação de apagar): a pilha fecha.
        if janelas > p.windows && !aberta && p.asking.is_none() {
            self.dirty = true;
            return;
        }
        p.windows = janelas;
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
            if m != p.dir_mtime || p.items.iter().take(200).any(|(path, _, _)| path.symlink_metadata().is_err()) {
                self.pilha_reload(&mut p);
            }
        }
        // A rolagem desliza (e o leque abre) com o tempo, não com os quadros.
        let dt = p.stepped.elapsed().as_secs_f32().min(0.1);
        p.stepped = Instant::now();
        let k = 1.0 - (-dt * 16.0).exp();
        p.off += (p.off_to - p.off) * k;
        p.scroll += (p.scroll_to - p.scroll) * k;
        if (p.off - p.off_to).abs() <= 0.002 {
            p.off = p.off_to;
        }
        if (p.scroll - p.scroll_to).abs() <= 0.3 {
            p.scroll = p.scroll_to;
        }
        let t = if p.fan { (p.opened.elapsed().as_secs_f32() / ABRIR.as_secs_f32()).min(1.0) } else { 1.0 };
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (p.items.len(), p.shown.len(), p.thumbs.len(), p.hover.map(|h| format!("{h:?}")), dark, tgen, sc.to_bits(), p.arrow_x.to_bits()).hash(&mut h);
        (p.off.to_bits(), p.scroll.to_bits(), t.to_bits(), &p.query, p.fan, p.slots, p.bend.to_bits()).hash(&mut h);
        for (path, _, _) in p.items.iter().take(200) {
            path.hash(&mut h);
        }
        let key = h.finish();
        if p.tex.is_none() || p.key != key {
            let title = p.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| tr!("Downloads").to_string());
            let entry = |(path, name, dir): &(PathBuf, String, bool)| StackEntry { icon: icons::file_icon(path, *dir), name: name.clone(), thumb: p.thumbs.get(path) };
            p.tex = if p.fan {
                let entries: Vec<StackEntry> = p.items.iter().map(entry).collect();
                chrome::stack_fan(&self.fonts, &entries, p.slots, p.bend, p.off, t, p.hover, sc).map(to_tex)
            } else {
                let entries: Vec<StackEntry> = p.shown.iter().filter_map(|&i| p.items.get(i)).map(entry).collect();
                let st = StackGridState { title: &title, total: p.items.len(), query: &p.query, scroll: p.scroll, hover: p.hover, arrow_x: p.arrow_x, dark };
                chrome::stack_grid(&self.fonts, &st, &entries, sc).map(to_tex)
            };
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

    #[test]
    fn busca_sem_acento_nem_maiusculas() {
        let items: Vec<(std::path::PathBuf, String, bool)> =
            ["Relatório Final.pdf", "foto.png", "RELATORIO-2.txt", "notas.md"].iter().map(|n| (std::path::PathBuf::from(n), n.to_string(), false)).collect();
        assert_eq!(super::filtrar(&items, "relatorio"), [0, 2]);
        assert_eq!(super::filtrar(&items, ""), [0, 1, 2, 3]);
        assert_eq!(super::filtrar(&items, "  FOTO "), [1]);
        assert!(super::filtrar(&items, "zzz").is_empty());
    }
}
