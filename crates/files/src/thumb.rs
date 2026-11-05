//! Prévia de imagens na coluna de detalhes: a foto é aberta numa thread (reduzida
//! a no máximo 2560 pixels de lado) e desenhada no tamanho que couber, com zoom.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use glove_kit::paint::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

/// Arquivos maiores que isto ficam só com o ícone (decodificar custaria muita memória).
const MAX_BYTES: u64 = 40 * 1024 * 1024;
/// Lado máximo guardado (a prévia raramente é maior; o zoom amplia a partir dela).
const MAX_SIDE: u32 = 2560;

/// O Arquivos sabe mostrar a prévia deste arquivo?
pub fn supported(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.len() <= MAX_BYTES) && glove_kit::image::format_of(p).is_some()
}

/// Reduz pela metade até caber em `side` (sem serrilhado) e o resto bilinear.
fn shrink(mut img: Pixmap, side: u32) -> Pixmap {
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    let step = |src: &Pixmap, w: u32, h: u32| {
        let mut out = Pixmap::new(w.max(1), h.max(1))?;
        out.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_scale(w.max(1) as f32 / src.width() as f32, h.max(1) as f32 / src.height() as f32), None);
        Some(out)
    };
    while img.width().max(img.height()) > side * 2 {
        match step(&img, img.width() / 2, img.height() / 2) {
            Some(n) => img = n,
            None => break,
        }
    }
    let k = (side as f32 / img.width().max(img.height()) as f32).min(1.0);
    if k < 1.0 {
        if let Some(n) = step(&img, (img.width() as f32 * k).round() as u32, (img.height() as f32 * k).round() as u32) {
            img = n;
        }
    }
    img
}

/// A imagem da prévia (uma só: a do item escolhido) e o tamanho original dela.
#[derive(Default)]
pub struct Thumbs {
    want: Option<PathBuf>,
    job: Option<Receiver<Option<(Pixmap, (u32, u32))>>>,
    ready: Option<(Pixmap, (u32, u32))>,
}

impl Thumbs {
    /// A imagem de `p` (e as dimensões originais). `None` enquanto abre (ou se
    /// falhar); o segundo valor diz se ainda está abrindo.
    pub fn get(&mut self, p: &Path) -> (Option<&(Pixmap, (u32, u32))>, bool) {
        if self.want.as_deref() != Some(p) {
            self.want = Some(p.to_path_buf());
            self.ready = None;
            let (tx, rx) = channel();
            let path = p.to_path_buf();
            std::thread::spawn(move || {
                let _ = tx.send(glove_kit::image::decode(&path).ok().map(|i| {
                    let size = (i.width(), i.height());
                    (shrink(i, MAX_SIDE), size)
                }));
            });
            self.job = Some(rx);
        }
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(img) => {
                    self.ready = img;
                    self.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return (None, true),
                Err(_) => self.job = None,
            }
        }
        (self.ready.as_ref(), false)
    }
}
