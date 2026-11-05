//! Abre imagens PNG, JPEG e WebP (o formato é visto pelo conteúdo, não pela
//! extensão: fotos baixadas da web muitas vezes são WebP com nome .jpg).

use std::path::Path;

use glove_paint::tiny_skia::{IntSize, Pixmap};

/// Formatos que o Glove abre, pelo começo do arquivo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
}

pub fn sniff(head: &[u8]) -> Option<Format> {
    if head.starts_with(b"\x89PNG") {
        Some(Format::Png)
    } else if head.starts_with(&[0xff, 0xd8]) {
        Some(Format::Jpeg)
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some(Format::WebP)
    } else {
        None
    }
}

/// O formato do arquivo (lê só os primeiros bytes).
pub fn format_of(path: &Path) -> Option<Format> {
    use std::io::Read;
    let mut head = [0u8; 12];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut head).ok()?;
    sniff(&head[..n])
}

/// Extensões que costumam ser imagens que o Glove abre.
pub const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// Abre a imagem inteira (cores pré-multiplicadas, prontas para desenhar).
pub fn decode(path: &Path) -> Result<Pixmap, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match sniff(&bytes) {
        Some(Format::Png) => Pixmap::decode_png(&bytes).map_err(|e| format!("PNG inválido: {e}")),
        Some(Format::Jpeg) => {
            use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
            let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
            let mut dec = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), opts);
            let data = dec.decode().map_err(|e| format!("JPEG inválido: {e:?}"))?;
            let info = dec.info().ok_or("JPEG sem dimensões")?;
            let size = IntSize::from_wh(info.width as u32, info.height as u32).ok_or("tamanho inválido")?;
            Pixmap::from_vec(data, size).ok_or_else(|| "dados inválidos".into())
        }
        Some(Format::WebP) => {
            let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| format!("WebP inválido: {e}"))?;
            let (w, h) = dec.dimensions();
            let alpha = dec.has_alpha();
            let mut buf = vec![0u8; dec.output_buffer_size().ok_or("WebP grande demais")?];
            dec.read_image(&mut buf).map_err(|e| format!("WebP inválido: {e}"))?;
            let rgba: Vec<u8> = if alpha {
                // Pré-multiplica pelo alfa (o formato que o desenho usa).
                for px in buf.chunks_exact_mut(4) {
                    let a = px[3] as u16;
                    for c in &mut px[..3] {
                        *c = ((*c as u16 * a + 127) / 255) as u8;
                    }
                }
                buf
            } else {
                buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()
            };
            let size = IntSize::from_wh(w, h).ok_or("tamanho inválido")?;
            Pixmap::from_vec(rgba, size).ok_or_else(|| "dados inválidos".into())
        }
        None => Err("formato de imagem não reconhecido".into()),
    }
}
