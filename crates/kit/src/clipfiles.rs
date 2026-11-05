//! Arquivos na área de transferência e no arrastar: os formatos que os outros
//! apps entendem (lista de URIs, o formato dos gerenciadores de arquivos, texto e,
//! para uma imagem só, a própria imagem em PNG).

use std::path::{Path, PathBuf};

pub const URI_LIST: &str = "text/uri-list";
pub const GNOME_FILES: &str = "x-special/gnome-copied-files";

/// file:///caminho com os bytes fora do comum em %XX.
pub fn uri(p: &Path) -> String {
    let mut out = String::from("file://");
    for &b in p.as_os_str().as_encoded_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Caminhos de uma lista de URIs (aceita também caminhos soltos, um por linha).
pub fn parse(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "copy" && *l != "cut")
        .filter_map(|l| {
            if let Some(rest) = l.strip_prefix("file://") {
                // file://host/caminho: pula o nome do computador, se houver.
                let path = if rest.starts_with('/') { rest } else { &rest[rest.find('/')?..] };
                Some(PathBuf::from(unpercent(path)))
            } else if l.starts_with('/') {
                Some(PathBuf::from(l))
            } else {
                None
            }
        })
        .collect()
}

/// Uma imagem só: vai também como imagem (colar num chat cola a figura).
fn single_image(paths: &[PathBuf]) -> bool {
    paths.len() == 1 && crate::image::format_of(&paths[0]).is_some()
}

pub fn mimes(paths: &[PathBuf]) -> Vec<String> {
    let mut v = vec![URI_LIST.to_string(), GNOME_FILES.to_string()];
    if single_image(paths) {
        v.push("image/png".into());
    }
    v.extend(["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"].iter().map(|s| s.to_string()));
    v
}

/// O que mandar para `mime` (pode demorar: chame fora da linha principal).
pub fn payload(paths: &[PathBuf], cut: bool, mime: &str) -> Vec<u8> {
    match mime {
        URI_LIST => paths.iter().map(|p| uri(p) + "\r\n").collect::<String>().into_bytes(),
        GNOME_FILES => {
            let mut t = String::from(if cut { "cut" } else { "copy" });
            for p in paths {
                t.push('\n');
                t.push_str(&uri(p));
            }
            t.into_bytes()
        }
        "image/png" => {
            let Some(p) = paths.first() else { return Vec::new() };
            if crate::image::format_of(p) == Some(crate::image::Format::Png) {
                return std::fs::read(p).unwrap_or_default();
            }
            crate::image::decode(p).ok().and_then(|img| img.encode_png().ok()).unwrap_or_default()
        }
        _ => paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n").into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let p = PathBuf::from("/home/x/Captura de tela às 09.31.png");
        let u = uri(&p);
        assert_eq!(u, "file:///home/x/Captura%20de%20tela%20%C3%A0s%2009.31.png");
        assert_eq!(parse(&format!("copy\n{u}\r\n")), vec![p.clone()]);
        assert_eq!(parse("file://maquina/tmp/a%20b"), vec![PathBuf::from("/tmp/a b")]);
        assert_eq!(parse("/tmp/c"), vec![PathBuf::from("/tmp/c")]);
    }
}
