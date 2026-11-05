//! Visualizador do Glove: imagens (PNG e JPEG) e PDF, com barra de
//! ferramentas e miniaturas na lateral. As páginas de PDF são desenhadas pelo
//! pdftoppm (poppler) numa thread; as miniaturas também.

use std::path::{Path, PathBuf};

use glove_kit::{
    paint::{
        draw::*,
        tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform},
        Color,
    },
    run,
    ui::Key,
    App, Choice, FileBrowser, Glyph, Menu, MenuItem, Rect, Ui, WindowOptions, SEGMENT_H, SEGMENT_W,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Barra de ferramentas e lateral de miniaturas.
const TOOLBAR_H: f32 = 52.0;
const SIDEBAR_W: f32 = 132.0;
const THUMB: u32 = 96;

/// Lado máximo guardado na memória (fotos maiores são reduzidas ao abrir).
const MAX_SIDE: u32 = 4096;
const EXTS: [&str; 4] = glove_kit::image::EXTENSIONS;

fn is_pdf(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn is_image(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn decode(path: &Path) -> Result<Pixmap, String> {
    let img = glove_kit::image::decode(path)?;
    // Reduz fotos enormes logo na abertura.
    let (w, h) = (img.width(), img.height());
    if w.max(h) > MAX_SIDE {
        let k = MAX_SIDE as f32 / w.max(h) as f32;
        return Ok(scaled(&img, k));
    }
    Ok(img)
}

fn scaled(src: &Pixmap, k: f32) -> Pixmap {
    let (w, h) = (((src.width() as f32 * k).round() as u32).max(1), ((src.height() as f32 * k).round() as u32).max(1));
    let mut out = Pixmap::new(w, h).unwrap();
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    out.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_scale(w as f32 / src.width() as f32, h as f32 / src.height() as f32), None);
    out
}

/// Gira 90° (sentido horário se `cw`).
fn rotate(src: &Pixmap, cw: bool) -> Pixmap {
    let (w, h) = (src.width(), src.height());
    let mut out = Pixmap::new(h, w).unwrap();
    let s = src.data();
    let d = out.data_mut();
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = if cw { (h - 1 - y, x) } else { (y, w - 1 - x) };
            let si = ((y * w + x) * 4) as usize;
            let di = ((ny * h + nx) * 4) as usize;
            d[di..di + 4].copy_from_slice(&s[si..si + 4]);
        }
    }
    out
}

struct Image {
    /// Níveis: original, metade, um quarto… (para reduzir com qualidade).
    levels: Vec<Pixmap>,
}

impl Image {
    fn new(base: Pixmap) -> Self {
        let mut levels = vec![base];
        while levels.last().is_some_and(|p| p.width().min(p.height()) > 128) {
            let next = scaled(levels.last().unwrap(), 0.5);
            levels.push(next);
        }
        Self { levels }
    }
    fn size(&self) -> (f32, f32) {
        (self.levels[0].width() as f32, self.levels[0].height() as f32)
    }
    /// O menor nível que ainda tem pixels suficientes para a escala pedida.
    fn level_for(&self, k: f32) -> (&Pixmap, f32) {
        let mut idx = 0;
        while idx + 1 < self.levels.len() && k * (1 << (idx + 1)) as f32 <= 1.0 {
            idx += 1;
        }
        (&self.levels[idx], (1 << idx) as f32)
    }
}

/// O que está aberto: as imagens da pasta ou as páginas de um PDF.
enum Doc {
    Images(Vec<PathBuf>),
    Pdf { path: PathBuf, pages: usize, size_pt: (f32, f32) },
}

impl Doc {
    fn len(&self) -> usize {
        match self {
            Doc::Images(l) => l.len(),
            Doc::Pdf { pages, .. } => *pages,
        }
    }
}

/// Páginas e tamanho (pontos) do PDF pelo pdfinfo.
pub fn parse_pdfinfo(t: &str) -> (usize, (f32, f32)) {
    let pages = t.lines().find_map(|l| l.strip_prefix("Pages:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let size = t
        .lines()
        .find_map(|l| l.strip_prefix("Page size:"))
        .and_then(|v| {
            let mut it = v.split_whitespace();
            let w: f32 = it.next()?.parse().ok()?;
            it.next();
            let h: f32 = it.next()?.parse().ok()?;
            Some((w, h))
        })
        .unwrap_or((612.0, 792.0));
    (pages, size)
}

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).unwrap_or_else(std::env::temp_dir);
    base.join("glove/visualizador").join(std::process::id().to_string())
}

/// Desenha a página `n` (1…) do PDF em `dpi` e devolve o PNG.
fn render_pdf_page(pdf: &Path, n: usize, dpi: u32, tag: &str) -> Result<Pixmap, String> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let prefix = dir.join(format!("{tag}-{n}"));
    let out = std::process::Command::new("pdftoppm")
        .args(["-png", "-singlefile", "-r", &dpi.to_string(), "-f", &n.to_string(), "-l", &n.to_string()])
        .arg(pdf)
        .arg(&prefix)
        .output()
        .map_err(|e| format!("Não foi possível usar o pdftoppm: {e}"))?;
    if !out.status.success() {
        return Err("O pdftoppm não conseguiu ler este PDF.".into());
    }
    let png = prefix.with_extension("png");
    let pm = Pixmap::load_png(&png).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&png);
    pm
}

/// Resultados das threads: páginas desenhadas e miniaturas.
#[derive(Default)]
struct Jobs {
    /// (página, dpi) → imagem.
    page: Option<(usize, u32, Result<Pixmap, String>)>,
    thumbs: HashMap<usize, Pixmap>,
    thumbs_done: bool,
}

struct Viewer {
    /// Imprimir (Ctrl+P): a folha do kit.
    print: Option<glove_kit::print::PrintSheet>,
    path: Option<PathBuf>,
    doc: Option<Doc>,
    index: usize,
    image: Option<Image>,
    error: Option<String>,
    /// None = ajustar à janela.
    zoom: Option<f32>,
    pan: (f32, f32),
    drag_from: Option<(f32, f32)>,
    last_title: String,
    /// Diálogo Abrir aberto.
    open_fb: Option<FileBrowser>,
    sidebar: bool,
    /// Giro (em quartos de volta) aplicado às páginas do PDF.
    turns: i32,
    jobs: Arc<Mutex<Jobs>>,
    /// Página de PDF sendo desenhada: (página, dpi).
    pending: Option<(usize, u32)>,
    shown_dpi: u32,
    thumbs_started: bool,
    side_scroll: f32,
    /// Menu de contexto aberto (botão direito na imagem): onde.
    ctx: Option<(f32, f32)>,
    /// Aviso curto na faixa de baixo (ex.: "Imagem copiada.").
    note: Option<(String, std::time::Instant)>,
}

/// Itens do menu de contexto: (ação, nome); ação vazia = divisória.
const CTX: [(&str, &str); 8] = [
    ("copy", "Copiar Imagem"),
    ("trash", "Mover para a Lixeira"),
    ("", ""),
    ("rot-left", "Girar à Esquerda"),
    ("rot-right", "Girar à Direita"),
    ("", ""),
    ("reveal", "Mostrar no Arquivos"),
    ("open", "Abrir Outra…"),
];

fn ctx_rect(w: f32, h: f32, (x, y): (f32, f32)) -> Rect {
    let mh = CTX.iter().map(|i| if i.0.is_empty() { 9.0 } else { 20.0 }).sum::<f32>() + 8.0;
    let mw = 210.0;
    Rect::new(x.min(w - mw - 4.0).max(0.0), if y + mh > h - 4.0 { (y - mh).max(0.0) } else { y }, mw, mh)
}

impl Viewer {
    fn new(path: Option<PathBuf>) -> Self {
        let mut v = Self {
            print: None,
            path: None,
            doc: None,
            index: 0,
            image: None,
            error: None,
            zoom: None,
            pan: (0.0, 0.0),
            drag_from: None,
            last_title: String::new(),
            open_fb: None,
            sidebar: false,
            turns: 0,
            jobs: Arc::default(),
            pending: None,
            shown_dpi: 0,
            thumbs_started: false,
            side_scroll: 0.0,
            ctx: None,
            note: None,
        };
        if let Some(p) = path {
            v.load(p);
        }
        v
    }

    fn load(&mut self, p: PathBuf) {
        self.zoom = None;
        self.pan = (0.0, 0.0);
        self.turns = 0;
        self.image = None;
        self.error = None;
        self.pending = None;
        self.shown_dpi = 0;
        self.thumbs_started = false;
        self.side_scroll = 0.0;
        *self.jobs.lock().unwrap() = Jobs::default();
        if is_pdf(&p) {
            let info = std::process::Command::new("pdfinfo").arg(&p).output();
            match info {
                Ok(o) if o.status.success() => {
                    let (pages, size_pt) = parse_pdfinfo(&String::from_utf8_lossy(&o.stdout));
                    self.doc = Some(Doc::Pdf { path: p.clone(), pages: pages.max(1), size_pt });
                    self.index = 0;
                    // Um PDF abre com as miniaturas à mostra (como no 10.4).
                    self.sidebar = pages > 1;
                }
                _ => {
                    self.doc = None;
                    self.error = Some("Não foi possível abrir este PDF (o pdfinfo do poppler está instalado?).".into());
                }
            }
            self.path = Some(p);
            return;
        }
        // Imagem: a lista é a das imagens da mesma pasta.
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut list: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|q| is_image(q)).collect();
        list.sort_by_key(|q| q.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if !list.contains(&p) {
            list.insert(0, p.clone());
        }
        self.index = list.iter().position(|q| *q == p).unwrap_or(0);
        self.doc = Some(Doc::Images(list));
        self.show_image(&p);
        self.path = Some(p);
    }

    fn show_image(&mut self, p: &Path) {
        match decode(p) {
            Ok(img) => {
                self.image = Some(Image::new(img));
                self.error = None;
            }
            Err(e) => {
                self.image = None;
                self.error = Some(e);
            }
        }
    }

    /// Vai para a página/imagem `i` do documento.
    fn go(&mut self, i: usize) {
        let Some(doc) = &self.doc else { return };
        let n = doc.len();
        if n == 0 {
            return;
        }
        let i = i.min(n - 1);
        if i == self.index && self.image.is_some() {
            return;
        }
        self.index = i;
        self.zoom = None;
        self.pan = (0.0, 0.0);
        match doc {
            Doc::Images(list) => {
                let p = list[i].clone();
                self.show_image(&p);
                self.path = Some(p);
            }
            Doc::Pdf { .. } => {
                self.image = None;
                self.shown_dpi = 0;
            }
        }
    }

    /// O arquivo mostrado agora (a imagem da vez, ou o PDF).
    fn current_file(&self) -> Option<PathBuf> {
        match &self.doc {
            Some(Doc::Images(l)) => l.get(self.index).cloned(),
            Some(Doc::Pdf { path, .. }) => Some(path.clone()),
            None => None,
        }
    }

    /// Manda o arquivo para a Lixeira (padrão do sistema) e mostra o próximo da pasta.
    fn trash_current(&mut self) {
        let Some(p) = self.current_file() else { return };
        let ok = std::process::Command::new("gio").arg("trash").arg(&p).status().is_ok_and(|s| s.success());
        if !ok {
            self.note = Some(("Não foi possível mover para a Lixeira.".into(), std::time::Instant::now()));
            return;
        }
        let rest: Vec<PathBuf> = match &self.doc {
            Some(Doc::Images(l)) => l.iter().filter(|q| **q != p).cloned().collect(),
            _ => Vec::new(),
        };
        if rest.is_empty() {
            self.doc = None;
            self.image = None;
            self.path = None;
            self.note = Some(("Movida para a Lixeira.".into(), std::time::Instant::now()));
        } else {
            let next = rest[self.index.min(rest.len() - 1)].clone();
            self.load(next);
            self.note = Some(("Movida para a Lixeira.".into(), std::time::Instant::now()));
        }
    }

    fn step(&mut self, delta: isize) {
        let Some(doc) = &self.doc else { return };
        let n = doc.len() as isize;
        if n == 0 {
            return;
        }
        let next = match doc {
            // Imagens dão a volta; páginas param no começo e no fim.
            Doc::Images(_) => ((self.index as isize + delta) % n + n) % n,
            Doc::Pdf { .. } => (self.index as isize + delta).clamp(0, n - 1),
        };
        self.go(next as usize);
    }

    /// Pede a página do PDF no dpi que a tela precisa (numa thread).
    fn want_pdf_page(&mut self, area: Rect, ui_scale: f32) {
        let Some(Doc::Pdf { path, size_pt, .. }) = &self.doc else { return };
        // Ajustada à janela (ou no zoom), em pixels reais.
        let (pw, ph) = if self.turns % 2 == 0 { *size_pt } else { (size_pt.1, size_pt.0) };
        let fit = ((area.w - 24.0) / pw).min((area.h - 24.0) / ph);
        let k = self.zoom.unwrap_or(fit).max(0.1);
        let dpi = ((72.0 * k * ui_scale).round() as u32).clamp(24, 300);
        // Já tem algo próximo, ou já pediu.
        if self.image.is_some() && (self.shown_dpi as f32 - dpi as f32).abs() < dpi as f32 * 0.15 {
            return;
        }
        if self.pending == Some((self.index, dpi)) {
            return;
        }
        self.pending = Some((self.index, dpi));
        let (path, page, jobs) = (path.clone(), self.index, self.jobs.clone());
        std::thread::spawn(move || {
            let r = render_pdf_page(&path, page + 1, dpi, "pagina");
            jobs.lock().unwrap().page = Some((page, dpi, r));
        });
    }

    /// Miniaturas numa thread: as imagens reduzidas ou as páginas a 18 dpi.
    fn start_thumbs(&mut self) {
        if self.thumbs_started {
            return;
        }
        let Some(doc) = &self.doc else { return };
        self.thumbs_started = true;
        let jobs = self.jobs.clone();
        match doc {
            Doc::Images(list) => {
                let list = list.clone();
                std::thread::spawn(move || {
                    for (i, p) in list.iter().enumerate() {
                        if let Ok(img) = decode(p) {
                            let k = THUMB as f32 / img.width().max(img.height()) as f32;
                            let t = scaled(&img, k.min(1.0));
                            jobs.lock().unwrap().thumbs.insert(i, t);
                        }
                    }
                    jobs.lock().unwrap().thumbs_done = true;
                });
            }
            Doc::Pdf { path, pages, .. } => {
                let (path, pages) = (path.clone(), *pages);
                std::thread::spawn(move || {
                    for i in 0..pages.min(500) {
                        if let Ok(t) = render_pdf_page(&path, i + 1, 14, "mini") {
                            jobs.lock().unwrap().thumbs.insert(i, t);
                        }
                    }
                    jobs.lock().unwrap().thumbs_done = true;
                });
            }
        }
    }

    /// Escala do PDF que cabe na área, em relação ao papel (100% = 72 dpi).
    fn pdf_fit(&self, area: Rect) -> Option<f32> {
        let Some(Doc::Pdf { size_pt, .. }) = &self.doc else { return None };
        let (pw, ph) = if self.turns % 2 == 0 { *size_pt } else { (size_pt.1, size_pt.0) };
        Some(((area.w - 24.0) / pw).min((area.h - 24.0) / ph))
    }

    fn fit_scale(&self, area: Rect) -> f32 {
        let Some(img) = &self.image else { return 1.0 };
        let (w, h) = img.size();
        ((area.w - 24.0) / w).min((area.h - 24.0) / h).min(1.0)
    }

    fn scale(&self, area: Rect) -> f32 {
        self.zoom.unwrap_or_else(|| self.fit_scale(area))
    }

    fn set_zoom(&mut self, z: f32) {
        self.zoom = Some(z.clamp(0.02, 16.0));
    }

    fn rotate(&mut self, cw: bool) {
        if let Some(img) = &self.image {
            self.image = Some(Image::new(rotate(&img.levels[0], cw)));
            self.pan = (0.0, 0.0);
        }
        self.turns = (self.turns + if cw { 1 } else { 3 }) % 4;
    }

    fn action(&mut self, a: &str, ui: &mut Ui, area: Rect) {
        // No PDF o zoom é em relação ao papel; nas imagens, aos pixels.
        let k = match self.pdf_fit(area) {
            Some(fit) => self.zoom.unwrap_or(fit),
            None => self.scale(area),
        };
        match a {
            "print" => {
                if let Some(p) = self.path.clone() {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let content = if matches!(self.doc, Some(Doc::Pdf { .. })) { glove_kit::print::PrintContent::Pdf(p) } else { glove_kit::print::PrintContent::Image(p) };
                    self.print = Some(glove_kit::print::PrintSheet::new(&name, content));
                }
            }
            "open" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
                self.open_fb = Some(FileBrowser::open(dir, &["png", "jpg", "jpeg", "webp", "pdf"]));
            }
            "bigger" => self.set_zoom(k * 1.25),
            "smaller" => self.set_zoom(k / 1.25),
            "actual" => {
                self.zoom = Some(1.0);
                self.pan = (0.0, 0.0);
            }
            "fit" => {
                self.zoom = None;
                self.pan = (0.0, 0.0);
            }
            "rot-left" => self.rotate(false),
            "rot-right" => self.rotate(true),
            "next" => self.step(1),
            "prev" => self.step(-1),
            "first" => self.go(0),
            "last" => self.go(usize::MAX),
            "sidebar" => self.sidebar = !self.sidebar,
            "close" => ui.close = true,
            "copy" => {
                // O arquivo de verdade: colar num chat ou editor cola a imagem.
                if let Some(p) = self.current_file() {
                    ui.copy_files(&[p], false);
                    self.note = Some(("Imagem copiada.".into(), std::time::Instant::now()));
                }
            }
            "trash" => self.trash_current(),
            "reveal" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()));
                let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.is_file()).unwrap_or_else(|| "glove-files".into());
                if let Ok(mut c) = std::process::Command::new(exe).arg(dir).spawn() {
                    std::thread::spawn(move || c.wait());
                }
            }
            _ => {}
        }
    }

    /// Barra de ferramentas em metal claro, com rótulos embaixo (como no Preview da era).
    fn toolbar(&mut self, ui: &mut Ui) -> Option<&'static str> {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, TOOLBAR_H, &vgrad(0.0, TOOLBAR_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(204, 204, 208, 255))]));
        ui.c.hline(0.0, TOOLBAR_H - 1.0, w, rgba(140, 140, 146, 255));
        let has = self.image.is_some() || self.pending.is_some();
        let n = self.doc.as_ref().map(Doc::len).unwrap_or(0);
        let mut act = None;
        let y = 8.0;
        let label = |ui: &mut Ui, t: &str, cx: f32| {
            let tw = ui.text_width(t, 10.0, false);
            ui.fonts.draw(ui.c, t, cx - tw / 2.0, TOOLBAR_H - 7.0, 10.0, false, rgba(40, 40, 44, 255));
        };
        let mut x = 22.0;
        // Miniaturas
        if ui.segmented_in(Rect::new(x, y, SEGMENT_W, SEGMENT_H), &[Glyph::Sidebar], self.sidebar.then_some(0), &[n > 0]).is_some() {
            act = Some("sidebar");
        }
        label(ui, "Miniaturas", x + SEGMENT_W / 2.0);
        x += SEGMENT_W + 36.0;
        // Anterior / Próxima
        let nav = [n > 1 && (matches!(self.doc, Some(Doc::Images(_))) || self.index > 0), n > 1 && (matches!(self.doc, Some(Doc::Images(_))) || self.index + 1 < n)];
        match ui.segmented_in(Rect::new(x, y, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &nav) {
            Some(0) => act = Some("prev"),
            Some(_) => act = Some("next"),
            None => {}
        }
        let pg = if n > 0 { format!("{} de {n}", self.index + 1) } else { "Anterior/Próxima".into() };
        label(ui, &pg, x + SEGMENT_W);
        x += 2.0 * SEGMENT_W + 30.0;
        // Zoom
        match ui.segmented_in(Rect::new(x, y, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Minus, Glyph::Plus], None, &[has, has]) {
            Some(0) => act = Some("smaller"),
            Some(_) => act = Some("bigger"),
            None => {}
        }
        label(ui, "Zoom", x + SEGMENT_W);
        x += 2.0 * SEGMENT_W + 30.0;
        // Girar
        match ui.segmented_in(Rect::new(x, y, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::RotateLeft, Glyph::RotateRight], None, &[has, has]) {
            Some(0) => act = Some("rot-left"),
            Some(_) => act = Some("rot-right"),
            None => {}
        }
        label(ui, "Girar", x + SEGMENT_W);
        act
    }

    /// Lateral com as miniaturas; clique vai para a página.
    fn side(&mut self, ui: &mut Ui, top: f32, h: f32) {
        let Some(doc) = &self.doc else { return };
        let n = doc.len();
        let names: Vec<String> = match doc {
            Doc::Images(l) => l.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect(),
            Doc::Pdf { .. } => (1..=n).map(|i| i.to_string()).collect(),
        };
        self.start_thumbs();
        let r = Rect::new(0.0, top, SIDEBAR_W, h);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(222, 226, 232, 255)));
        ui.c.vline(r.w - 0.5, r.y, r.h, rgba(150, 150, 156, 255));
        let cell = THUMB as f32 + 30.0;
        // Barra de rolagem do kit (setas, trilho, rodinha) quando não cabem todas.
        let inner = ui.scroll_area(Rect::new(r.x, r.y, r.w - 1.0, r.h), &mut self.side_scroll, n as f32 * cell + 10.0, cell);
        let side_w = inner.w;
        // Recorte: as miniaturas roladas não passam por cima da barra de ferramentas.
        let s = ui.c.scale;
        let old = ui.c.clip();
        let mut clip = [0, (top * s).floor() as i32, (side_w * s).ceil() as i32, ((top + h) * s).ceil() as i32];
        if let Some(o) = old {
            clip = [clip[0].max(o[0]), clip[1].max(o[1]), clip[2].min(o[2]), clip[3].min(o[3])];
        }
        ui.c.set_clip(Some(clip));
        let jobs = self.jobs.lock().unwrap();
        let first = (self.side_scroll / cell) as usize;
        let mut go = None;
        for i in first..n.min(first + (h / cell) as usize + 2) {
            let y = top + 8.0 + i as f32 * cell - self.side_scroll;
            let box_r = Rect::new((side_w - THUMB as f32) / 2.0, y, THUMB as f32, THUMB as f32);
            if i == self.index {
                ui.c.fill(rrect(box_r.x - 4.0, box_r.y - 4.0, box_r.w + 8.0, box_r.h + 22.0, 5.0), &solid(rgba(160, 180, 210, 255)));
            }
            match jobs.thumbs.get(&i) {
                Some(t) => {
                    let (tw, th) = (t.width() as f32 / ui.c.scale.max(1.0), t.height() as f32 / ui.c.scale.max(1.0));
                    let k = (THUMB as f32 / t.width().max(t.height()) as f32).min(1.0);
                    let (dw, dh) = (t.width() as f32 * k, t.height() as f32 * k);
                    let _ = (tw, th);
                    let (dx, dy) = (box_r.x + (box_r.w - dw) / 2.0, box_r.y + (box_r.h - dh) / 2.0);
                    ui.c.fill_rect(dx + 1.0, dy + 2.0, dw, dh, &solid(rgba(0, 0, 0, 50)));
                    let s = ui.c.scale;
                    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
                    ui.c.draw_pixmap_t(t.as_ref(), &paint, Transform::from_row(k * s, 0.0, 0.0, k * s, dx * s, dy * s));
                }
                None => ui.c.stroke(rrect(box_r.x + 20.0, box_r.y + 10.0, box_r.w - 40.0, box_r.h - 20.0, 2.0), &solid(rgba(160, 160, 166, 255)), 1.0),
            }
            let name = ui.fonts.ellipsize(&names[i], 10.0, false, side_w - 10.0);
            let tw = ui.text_width(&name, 10.0, false);
            ui.fonts.draw(ui.c, &name, (side_w - tw) / 2.0, y + THUMB as f32 + 13.0, 10.0, false, rgba(40, 40, 44, 255));
            if ui.hit(Rect::new(0.0, y - 4.0, side_w, cell)).clicked && ui.pointer().1 >= top {
                go = Some(i);
            }
        }
        let busy = !jobs.thumbs_done;
        drop(jobs);
        ui.c.set_clip(old);
        if busy {
            ui.wake_after(Duration::from_millis(250));
        }
        if let Some(i) = go {
            self.go(i);
        }
    }
}

impl App for Viewer {
    fn ui(&mut self, ui: &mut Ui) {
        ui.set_document(self.path.as_deref());
        let (w, h) = (ui.c.w, ui.c.h);
        let left = if self.sidebar && self.doc.is_some() { SIDEBAR_W } else { 0.0 };
        let area = Rect::new(left, TOOLBAR_H, w - left, h - 15.0 - TOOLBAR_H);

        // Menu de contexto aberto: o clique sobre ele não chega à imagem; fora, só fecha.
        if let Some(pos) = self.ctx {
            let r = ctx_rect(w, h, pos);
            ui.reserve(r);
            if ui.mouse_pressed() && !r.contains(ui.pointer()) {
                self.ctx = None;
                ui.reserve(Rect::new(0.0, 0.0, w, h));
            }
        }

        // Página de PDF pronta?
        let done = self.jobs.lock().unwrap().page.take();
        if let Some((page, dpi, r)) = done {
            if page == self.index {
                match r {
                    Ok(mut pm) => {
                        for _ in 0..self.turns {
                            pm = rotate(&pm, true);
                        }
                        // A imagem vem no dpi certo: mostra a 1:1 em pixels reais.
                        self.image = Some(Image::new(pm));
                        self.shown_dpi = dpi;
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            if self.pending.is_some_and(|p| p == (page, dpi)) {
                self.pending = None;
            }
        }
        if matches!(self.doc, Some(Doc::Pdf { .. })) {
            self.want_pdf_page(area, ui.c.scale);
        }
        if self.pending.is_some() {
            ui.wake_after(Duration::from_millis(120));
        }

        // Teclado e menus
        let mut acts: Vec<String> = Vec::new();
        for k in ui.keys() {
            let a = match k.combo.as_str() {
                "Ctrl+equal" | "Ctrl+plus" => "bigger",
                "Ctrl+minus" => "smaller",
                "Ctrl+0" => "actual",
                "Ctrl+9" => "fit",
                "Ctrl+L" => "rot-left",
                "Ctrl+R" => "rot-right",
                "Ctrl+W" => "close",
                "Ctrl+O" => "open",
                "Ctrl+P" => "print",
                "Ctrl+Shift+R" => "reveal",
                "Ctrl+Shift+M" => "sidebar",
                "Ctrl+C" => "copy",
                "Ctrl+BackSpace" | "Delete" => "trash",
                _ => match k.key {
                    Key::Right | Key::Down | Key::PageDown => "next",
                    Key::Left | Key::Up | Key::PageUp => "prev",
                    Key::Home => "first",
                    Key::End => "last",
                    _ => "",
                },
            };
            if !a.is_empty() {
                acts.push(a.into());
            }
        }
        acts.extend(ui.menu_actions().iter().cloned());
        // Com o diálogo Abrir na frente, as teclas são dele.
        if self.open_fb.is_some() || self.print.is_some() {
            acts.clear();
        }
        if let Some(a) = self.toolbar(ui) {
            acts.push(a.into());
        }
        for a in acts {
            self.action(&a, ui, area);
        }

        // Fundo
        ui.c.fill_rect(area.x, area.y, area.w, area.h, &solid(rgba(108, 110, 116, 255)));
        if left > 0.0 {
            self.side(ui, TOOLBAR_H, h - 15.0 - TOOLBAR_H);
        }

        // PDF: a imagem tem o dpi da tela; o zoom "real" é em relação ao papel.
        let pdf_k = match (&self.doc, self.shown_dpi) {
            (Some(Doc::Pdf { .. }), d) if d > 0 => Some(72.0 / d as f32),
            _ => None,
        };
        let k = match pdf_k {
            // Mostra a página do PDF do tamanho pedido (ajustada ou no zoom).
            Some(base) => {
                let fit = self.fit_scale(area);
                match self.zoom {
                    Some(z) => {
                        let Some(Doc::Pdf { size_pt, .. }) = &self.doc else { unreachable!() };
                        let iw = self.image.as_ref().map(|i| i.size().0).unwrap_or(1.0);
                        let pw = if self.turns % 2 == 0 { size_pt.0 } else { size_pt.1 };
                        let _ = base;
                        z * pw / iw
                    }
                    None => fit,
                }
            }
            None => self.scale(area),
        };
        let name = self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Visualizador".into());
        let title = match &self.doc {
            Some(Doc::Pdf { pages, .. }) => format!("{name} (página {} de {pages})", self.index + 1),
            _ if self.image.is_some() => format!("{name} ({:.0}%)", k * 100.0),
            _ => name,
        };
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }

        if let Some(img) = &self.image {
            let (iw, ih) = img.size();
            let (dw, dh) = (iw * k, ih * k);
            // Arrastar move a imagem quando ela é maior que a janela.
            let resp = ui.hit(area);
            let pos = ui.pointer();
            if resp.pressed {
                self.drag_from = Some(pos);
            }
            if resp.down {
                if let Some((fx, fy)) = self.drag_from {
                    self.pan.0 += pos.0 - fx;
                    self.pan.1 += pos.1 - fy;
                    self.drag_from = Some(pos);
                }
            } else {
                self.drag_from = None;
            }
            self.pan.1 -= ui.scroll_in(area);
            let lim = |d: f32, a: f32| ((d - a) / 2.0 + 12.0).max(0.0);
            self.pan.0 = self.pan.0.clamp(-lim(dw, area.w), lim(dw, area.w));
            self.pan.1 = self.pan.1.clamp(-lim(dh, area.h), lim(dh, area.h));
            let x = area.x + (area.w - dw) / 2.0 + self.pan.0;
            let y = area.y + (area.h - dh) / 2.0 + self.pan.1;

            // Só dentro da área (o zoom não passa por cima da barra nem da lateral).
            let s = ui.c.scale;
            let old = ui.c.clip();
            let mut clip = [(area.x * s).floor() as i32, (area.y * s).floor() as i32, ((area.x + area.w) * s).ceil() as i32, ((area.y + area.h) * s).ceil() as i32];
            if let Some(o) = old {
                clip = [clip[0].max(o[0]), clip[1].max(o[1]), clip[2].min(o[2]), clip[3].min(o[3])];
            }
            ui.c.set_clip(Some(clip));
            // Sombra suave sob a imagem
            ui.c.fill(rrect(x - 1.0, y + 2.0, dw + 2.0, dh + 2.0, 2.0), &solid(rgba(0, 0, 0, 70)));
            let (lvl, f) = img.level_for(k * s);
            let paint = PixmapPaint { quality: if k * s >= 1.0 { FilterQuality::Nearest } else { FilterQuality::Bilinear }, ..Default::default() };
            let ks = k * s * f;
            ui.c.draw_pixmap_t(lvl.as_ref(), &paint, Transform::from_row(ks, 0.0, 0.0, ks, x * s, y * s));
            ui.c.set_clip(old);
        } else if self.pending.is_some() {
            let (cx, cy) = (area.x + area.w / 2.0, area.y + area.h / 2.0);
            ui.area(Rect::new(cx - 12.0, cy - 12.0, 24.0, 24.0), |ui| ui.spinner(24.0));
        } else {
            let msg = self.error.clone().unwrap_or_else(|| "Abra uma imagem ou um PDF com Arquivo › Abrir… (Ctrl+O) ou pelo Arquivos.".into());
            let tw = ui.text_width(&msg, 13.0, false);
            ui.fonts.draw(ui.c, &msg, area.x + (area.w - tw) / 2.0, area.y + area.h / 2.0, 13.0, false, Color::WHITE);
        }

        // Faixa inferior (puxador de redimensionar)
        ui.c.fill_rect(0.0, h - 15.0, w, 15.0, &vgrad(h - 15.0, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, h - 15.0, w, rgba(150, 150, 156, 255));
        if let Some((t, at)) = &self.note {
            if at.elapsed() < Duration::from_secs(3) {
                ui.fonts.draw(ui.c, t, 8.0, h - 4.0, 10.0, false, rgba(60, 60, 66, 255));
                ui.wake_after(Duration::from_millis(500));
            } else {
                self.note = None;
            }
        }
        // Botão direito na imagem: menu com copiar, Lixeira, girar…
        if self.ctx.is_none() && self.open_fb.is_none() && self.doc.is_some() && ui.right_clicked(area) {
            self.ctx = Some(ui.pointer());
        }
        if let Some(pos) = self.ctx {
            let r = ctx_rect(w, h, pos);
            let has = self.doc.is_some();
            let chosen = ui.unreserved(|ui| {
                ui.c.fill_rect(r.x + 3.0, r.y + 4.0, r.w, r.h, &solid(rgba(0, 0, 0, 50)));
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(246, 247, 249, 252)));
                ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 0.0), &solid(rgba(0, 0, 0, 80)), 1.0);
                let mut y = r.y + 4.0;
                let mut chosen = None;
                for (id, label) in CTX {
                    if id.is_empty() {
                        ui.c.hline(r.x + 1.0, y + 4.0, r.w - 2.0, rgba(214, 214, 218, 255));
                        y += 9.0;
                        continue;
                    }
                    let on = has || id == "open";
                    let row = Rect::new(r.x, y, r.w, 20.0);
                    let hit = ui.hit(row);
                    if hit.hover && on {
                        ui.c.fill_rect(row.x, row.y, row.w, row.h, &solid(ui.theme.menu_hot));
                    }
                    let col = if !on { rgba(160, 160, 160, 255) } else if hit.hover { Color::WHITE } else { rgba(20, 20, 20, 255) };
                    ui.fonts.draw(ui.c, label, r.x + 16.0, y + 14.5, 13.0, false, col);
                    if hit.clicked && on {
                        chosen = Some(id);
                    }
                    y += 20.0;
                }
                chosen
            });
            if let Some(a) = chosen {
                self.ctx = None;
                self.action(a, ui, area);
                ui.redraw = true;
            } else if ui.keys().iter().any(|k| k.key == Key::Escape) {
                self.ctx = None;
            }
        }
        // Imprimir.
        if let Some(p) = &mut self.print {
            if ui.print_sheet(p) {
                self.print = None;
                ui.redraw = true;
            }
        }
        // Diálogo Abrir.
        if let Some(fb) = &mut self.open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.open_fb = None;
                if let Choice::Picked(p) = c {
                    self.load(p);
                }
                ui.redraw = true;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let has = self.image.is_some();
        let pdf = matches!(self.doc, Some(Doc::Pdf { .. }));
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("open", "Abrir…", "Ctrl+O"),
                    MenuItem::new("reveal", "Mostrar no Arquivos", "Ctrl+Shift+R"),
                    MenuItem::new("trash", "Mover para a Lixeira", "Ctrl+BackSpace").enabled(self.doc.is_some()),
                    MenuItem::Separator,
                    MenuItem::new("print", "Imprimir…", "Ctrl+P").enabled(self.path.is_some()),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                ],
            ),
            Menu::new("Editar", vec![MenuItem::new("copy", "Copiar Imagem", "Ctrl+C").enabled(self.doc.is_some())]),
            Menu::new(
                "Visualizar",
                vec![
                    MenuItem::new("sidebar", if self.sidebar { "Ocultar Miniaturas" } else { "Mostrar Miniaturas" }, "Ctrl+Shift+M").enabled(self.doc.is_some()),
                    MenuItem::Separator,
                    MenuItem::new("bigger", "Ampliar", "Ctrl+equal").enabled(has),
                    MenuItem::new("smaller", "Reduzir", "Ctrl+minus").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("actual", "Tamanho real", "Ctrl+0").enabled(has).checked(self.zoom == Some(1.0)),
                    MenuItem::new("fit", "Ajustar à janela", "Ctrl+9").enabled(has).checked(self.zoom.is_none()),
                ],
            ),
            Menu::new(
                "Imagem",
                vec![
                    MenuItem::new("rot-left", "Girar à esquerda", "Ctrl+L").enabled(has),
                    MenuItem::new("rot-right", "Girar à direita", "Ctrl+R").enabled(has),
                ],
            ),
            Menu::new(
                "Ir",
                vec![
                    MenuItem::new("next", if pdf { "Próxima página" } else { "Próxima imagem" }, "Right"),
                    MenuItem::new("prev", if pdf { "Página anterior" } else { "Imagem anterior" }, "Left"),
                    MenuItem::Separator,
                    MenuItem::new("first", "Primeira", "Home"),
                    MenuItem::new("last", "Última", "End"),
                ],
            ),
        ]
    }

    fn close_requested(&mut self) -> bool {
        let _ = std::fs::remove_dir_all(cache_dir());
        true
    }
}

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let viewer = Viewer::new(path);
    // Janela do tamanho da imagem (dentro de limites razoáveis); PDF em formato de página.
    let (w, h) = match &viewer.doc {
        Some(Doc::Pdf { size_pt, .. }) => (size_pt.0 * 1.2, size_pt.1 * 1.2),
        _ => viewer.image.as_ref().map(|i| i.size()).unwrap_or((640.0, 480.0)),
    };
    // Cabe com a barra de menu, a de ferramentas e o Dock numa tela pequena.
    let k = (900.0 / w).min(520.0 / h).min(1.0);
    let side = if viewer.sidebar { SIDEBAR_W as u32 } else { 0 };
    let size = (((w * k) as u32 + 24 + side).max(480), ((h * k) as u32 + 24 + 15 + TOOLBAR_H as u32).max(300));
    let opts = WindowOptions { title: "Visualizador".into(), app_id: "glove.Viewer".into(), size, fixed: false, min_size: None };
    if let Err(e) = run(opts, viewer) {
        eprintln!("glove-view: {e}");
    }
    let _ = std::fs::remove_dir_all(cache_dir());
}

#[cfg(test)]
mod tests {
    #[test]
    fn le_o_pdfinfo() {
        let (n, s) = super::parse_pdfinfo("Title: x\nPages:          12\nPage size:      595.276 x 841.89 pts (A4)\n");
        assert_eq!(n, 12);
        assert!((s.0 - 595.276).abs() < 0.01 && (s.1 - 841.89).abs() < 0.01);
    }
}
