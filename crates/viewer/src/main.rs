//! Visualizador de imagens do Glove (PNG e JPEG).

use std::path::{Path, PathBuf};

use glove_kit::{
    paint::{
        draw::*,
        tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform},
        Color,
    },
    run,
    ui::Key,
    App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

/// Lado máximo guardado na memória (fotos maiores são reduzidas ao abrir).
const MAX_SIDE: u32 = 4096;
const EXTS: [&str; 3] = ["png", "jpg", "jpeg"];

fn is_image(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn decode(path: &Path) -> Result<Pixmap, String> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let img = if ext == "png" {
        Pixmap::load_png(path).map_err(|e| format!("PNG inválido: {e}"))?
    } else {
        use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), opts);
        let data = dec.decode().map_err(|e| format!("JPEG inválido: {e:?}"))?;
        let info = dec.info().ok_or("JPEG sem dimensões")?;
        let size = IntSize::from_wh(info.width as u32, info.height as u32).ok_or("tamanho inválido")?;
        Pixmap::from_vec(data, size).ok_or("dados inválidos")?
    };
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

struct Viewer {
    path: Option<PathBuf>,
    image: Option<Image>,
    error: Option<String>,
    /// None = ajustar à janela.
    zoom: Option<f32>,
    pan: (f32, f32),
    drag_from: Option<(f32, f32)>,
    last_title: String,
}

impl Viewer {
    fn new(path: Option<PathBuf>) -> Self {
        let mut v = Self { path: None, image: None, error: None, zoom: None, pan: (0.0, 0.0), drag_from: None, last_title: String::new() };
        if let Some(p) = path {
            v.load(p);
        }
        v
    }

    fn load(&mut self, p: PathBuf) {
        match decode(&p) {
            Ok(img) => {
                self.image = Some(Image::new(img));
                self.error = None;
            }
            Err(e) => {
                self.image = None;
                self.error = Some(e);
            }
        }
        self.path = Some(p);
        self.zoom = None;
        self.pan = (0.0, 0.0);
    }

    /// Imagem seguinte/anterior da mesma pasta.
    fn step(&mut self, delta: isize) {
        let Some(cur) = self.path.clone() else { return };
        let Some(dir) = cur.parent() else { return };
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut list: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| is_image(p)).collect();
        list.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if list.is_empty() {
            return;
        }
        let i = list.iter().position(|p| *p == cur).unwrap_or(0) as isize;
        let n = list.len() as isize;
        let next = list[((i + delta) % n + n) as usize % n as usize].clone();
        if next != cur {
            self.load(next);
        }
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
    }

    fn action(&mut self, a: &str, ui: &mut Ui, area: Rect) {
        let k = self.scale(area);
        match a {
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
            "close" => ui.close = true,
            "reveal" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()));
                let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-files"))).filter(|p| p.is_file()).unwrap_or_else(|| "glove-files".into());
                let _ = std::process::Command::new(exe).arg(dir).spawn();
            }
            _ => {}
        }
    }
}

impl App for Viewer {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(0.0, 0.0, w, h - 15.0);

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
                "Ctrl+O" => "reveal",
                "Right" => "next",
                "Left" => "prev",
                _ => match k.key {
                    Key::Right | Key::Down => "next",
                    Key::Left | Key::Up => "prev",
                    _ => "",
                },
            };
            if !a.is_empty() {
                acts.push(a.into());
            }
        }
        acts.extend(ui.menu_actions().iter().cloned());
        for a in acts {
            self.action(&a, ui, area);
        }

        // Fundo
        ui.c.fill_rect(area.x, area.y, area.w, area.h, &solid(rgba(108, 110, 116, 255)));

        let k = self.scale(area);
        let name = self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Visualizador".into());
        let title = if self.image.is_some() { format!("{name} ({:.0}%)", k * 100.0) } else { name };
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

            // Sombra suave sob a imagem
            ui.c.fill(rrect(x - 1.0, y + 2.0, dw + 2.0, dh + 2.0, 2.0), &solid(rgba(0, 0, 0, 70)));
            let s = ui.c.scale;
            let (lvl, f) = img.level_for(k * s);
            let paint = PixmapPaint { quality: if k * s >= 1.0 { FilterQuality::Nearest } else { FilterQuality::Bilinear }, ..Default::default() };
            let ks = k * s * f;
            ui.c.pm.draw_pixmap(0, 0, lvl.as_ref(), &paint, Transform::from_row(ks, 0.0, 0.0, ks, x * s, y * s), None);
        } else {
            let msg = self.error.clone().unwrap_or_else(|| "Abra uma imagem pelo Arquivos (clique duplo).".into());
            let tw = ui.text_width(&msg, 13.0, false);
            ui.fonts.draw(ui.c, &msg, (w - tw) / 2.0, h / 2.0, 13.0, false, Color::WHITE);
        }

        // Faixa inferior (puxador de redimensionar)
        ui.c.fill_rect(0.0, h - 15.0, w, 15.0, &vgrad(h - 15.0, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, h - 15.0, w, rgba(150, 150, 156, 255));
    }

    fn menus(&self) -> Vec<Menu> {
        let has = self.image.is_some();
        vec![
            Menu::new(
                "Arquivo",
                vec![MenuItem::new("reveal", "Mostrar no Arquivos", "Ctrl+O"), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")],
            ),
            Menu::new(
                "Visualizar",
                vec![
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
            Menu::new("Ir", vec![MenuItem::new("next", "Próxima imagem", "Right"), MenuItem::new("prev", "Imagem anterior", "Left")]),
        ]
    }
}

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let viewer = Viewer::new(path);
    // Janela do tamanho da imagem (dentro de limites razoáveis).
    let (w, h) = viewer.image.as_ref().map(|i| i.size()).unwrap_or((640.0, 480.0));
    let k = (900.0 / w).min(650.0 / h).min(1.0);
    let size = (((w * k) as u32 + 24).max(320), ((h * k) as u32 + 24 + 15).max(240));
    let opts = WindowOptions { title: "Visualizador".into(), app_id: "glove.Viewer".into(), size, fixed: false };
    if let Err(e) = run(opts, viewer) {
        eprintln!("glove-view: {e}");
    }
}
