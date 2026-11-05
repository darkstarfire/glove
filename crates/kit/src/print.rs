//! Imprimir (Arquivo ▸ Imprimir…, Ctrl+P): a folha do Tiger com a impressora,
//! cópias e páginas, Salvar como PDF e Pré-visualizar. Imprime pelo CUPS (`lp`).
//! Texto vira PDF aqui mesmo (Helvetica, A4); imagens vão direto para o CUPS
//! (e viram PDF para salvar ou pré-visualizar).

use std::path::{Path, PathBuf};

use glove_paint::draw::*;

use crate::filebrowser::{Choice, FileBrowser};
use crate::ui::{Rect, Ui};

/// O que imprimir.
#[derive(Clone)]
pub enum PrintContent {
    Text(String),
    Image(PathBuf),
    /// Um PDF pronto (o Visualizador com um PDF aberto).
    Pdf(PathBuf),
}

pub struct PrintSheet {
    /// Nome do documento (título do trabalho e do PDF salvo).
    pub name: String,
    content: PrintContent,
    printers: Vec<String>,
    printer: usize,
    copies: String,
    all_pages: bool,
    from: String,
    to: String,
    save: Option<FileBrowser>,
    message: Option<String>,
    first: bool,
}

/// Impressoras do CUPS e a padrão primeiro.
fn printers() -> Vec<String> {
    let out = |args: &[&str]| std::process::Command::new("lpstat").args(args).env("LC_ALL", "C").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut v: Vec<String> = out(&["-a"]).lines().filter_map(|l| l.split_whitespace().next().map(str::to_string)).collect();
    if let Some(def) = out(&["-d"]).lines().find_map(|l| l.rsplit_once(": ").map(|x| x.1.trim().to_string())) {
        if let Some(i) = v.iter().position(|p| *p == def) {
            let d = v.remove(i);
            v.insert(0, d);
        }
    }
    v
}

impl PrintSheet {
    pub fn new(name: &str, content: PrintContent) -> Self {
        Self { name: name.to_string(), content, printers: printers(), printer: 0, copies: "1".into(), all_pages: true, from: "1".into(), to: "1".into(), save: None, message: None, first: true }
    }

    fn pdf(&self) -> Result<Vec<u8>, String> {
        match &self.content {
            PrintContent::Text(t) => Ok(text_pdf(&self.name, t)),
            PrintContent::Image(p) => image_pdf(p),
            PrintContent::Pdf(p) => std::fs::read(p).map_err(|e| e.to_string()),
        }
    }

    /// PDF temporário (para o CUPS e para a pré-visualização).
    fn temp_pdf(&self) -> Result<PathBuf, String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let safe: String = self.name.chars().map(|c| if c.is_alphanumeric() || " .-_".contains(c) { c } else { '_' }).collect();
        let p = dir.join(format!("glove-imprimir-{}-{safe}.pdf", std::process::id()));
        std::fs::write(&p, self.pdf()?).map_err(|e| e.to_string())?;
        Ok(p)
    }

    fn print(&self) -> Result<(), String> {
        let printer = self.printers.get(self.printer).ok_or("Nenhuma impressora configurada.")?;
        let file = match &self.content {
            PrintContent::Image(p) | PrintContent::Pdf(p) => p.clone(),
            PrintContent::Text(_) => self.temp_pdf()?,
        };
        let copies = self.copies.trim().parse::<u32>().unwrap_or(1).clamp(1, 999).to_string();
        let mut cmd = std::process::Command::new("lp");
        cmd.args(["-d", printer, "-n", &copies, "-t", &self.name]);
        if !self.all_pages {
            let (a, b) = (self.from.trim().parse::<u32>().unwrap_or(1).max(1), self.to.trim().parse::<u32>().unwrap_or(1).max(1));
            cmd.args(["-P", &format!("{}-{}", a.min(b), a.max(b))]);
        }
        let o = cmd.arg(&file).env("LC_ALL", "C").output().map_err(|e| format!("Não foi possível usar o lp (CUPS): {e}"))?;
        if o.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
        }
    }
}

impl Ui<'_> {
    /// Desenha a folha Imprimir. Devolve verdadeiro quando ela fecha.
    pub fn print_sheet(&mut self, p: &mut PrintSheet) -> bool {
        // Salvar como PDF: a folha de salvar do kit.
        if let Some(fb) = &mut p.save {
            match self.file_sheet(fb) {
                Some(Choice::Picked(path)) => {
                    p.save = None;
                    let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) { path } else { path.with_extension("pdf") };
                    match p.pdf().and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
                        Ok(()) => return true,
                        Err(e) => p.message = Some(format!("Não foi possível salvar: {e}")),
                    }
                }
                Some(Choice::Cancelled) => p.save = None,
                None => return false,
            }
            return false;
        }
        self.set_enabled(true);
        let (w, h) = (self.c.w, self.c.h);
        let ink = rgba(20, 20, 20, 255);
        let sw = (w - 24.0).clamp(300.0, 540.0);
        let sh = 214.0f32.min(h - 8.0);
        let sx = ((w - sw) / 2.0).round();
        self.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
        let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
        self.c.fill(shape(), &solid(self.theme.body_b));
        self.c.pinstripes(sx, 0.0, sw, sh - 6.0, self.theme.body_a, self.theme.body_b);
        self.c.stroke(shape(), &solid(rgba(0, 0, 0, 90)), 1.0 / self.c.scale);
        let lw = 100.0;
        let label = |ui: &mut Ui, t: &str, y: f32| {
            let tw = ui.text_width(t, 13.0, false);
            ui.fonts.draw(ui.c, t, sx + lw - tw, y + 15.0, 13.0, false, ink);
        };
        // Impressora
        let mut y = 18.0;
        label(self, "Impressora:", y);
        let names: Vec<String> = if p.printers.is_empty() { vec!["Nenhuma impressora".into()] } else { p.printers.clone() };
        let mut add_printer = false;
        self.area(Rect::new(sx + lw + 8.0, y, sw - lw - 26.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_enabled(!p.printers.is_empty());
                ui.popup(&mut p.printer, &names, 220.0);
                ui.set_enabled(true);
                if p.printers.is_empty() {
                    ui.spacing(8.0);
                    add_printer = ui.small_button("Adicionar…");
                }
            });
        });
        if add_printer {
            let _ = std::process::Command::new("glove-prefs").arg("impressao").spawn();
        }
        // Cópias
        y += 36.0;
        label(self, "Cópias:", y);
        self.text_field_in(Rect::new(sx + lw + 8.0, y, 50.0, 22.0), &mut p.copies, false);
        // Páginas
        y += 32.0;
        label(self, "Páginas:", y);
        let mut all = p.all_pages as usize;
        self.area(Rect::new(sx + lw + 8.0, y + 2.0, 80.0, 22.0), |ui| {
            if ui.radio(p.all_pages, "Todas") {
                all = 1;
            }
        });
        self.area(Rect::new(sx + lw + 8.0, y + 26.0, 40.0, 22.0), |ui| {
            if ui.radio(!p.all_pages, "De:") {
                all = 0;
            }
        });
        p.all_pages = all == 1;
        let (c1, _) = self.text_field_in(Rect::new(sx + lw + 56.0, y + 24.0, 40.0, 22.0), &mut p.from, false);
        self.fonts.draw(self.c, "até:", sx + lw + 104.0, y + 40.0, 13.0, false, ink);
        let (c2, _) = self.text_field_in(Rect::new(sx + lw + 136.0, y + 24.0, 40.0, 22.0), &mut p.to, false);
        if c1 || c2 {
            p.all_pages = false;
        }
        if let Some(m) = &p.message {
            let m = self.fonts.ellipsize(m, 11.0, false, sw - 36.0);
            self.fonts.draw(self.c, &m, sx + 18.0, sh - 50.0, 11.0, false, rgba(170, 30, 30, 255));
        }
        // Botões: PDF à esquerda; Cancelar e Imprimir à direita.
        let mut done = false;
        let by = sh - 38.0;
        let (mut save_pdf, mut preview, mut cancel, mut go) = (false, false, false, false);
        self.area(Rect::new(sx + 16.0, by, sw - 230.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                save_pdf = ui.button("Salvar PDF…");
                ui.spacing(8.0);
                preview = ui.button("Pré-visualizar");
            });
        });
        self.area(Rect::new(sx + sw - 196.0, by, 180.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button("Cancelar");
                ui.spacing(8.0);
                ui.set_enabled(!p.printers.is_empty());
                go = ui.default_button("Imprimir");
                ui.set_enabled(true);
            });
        });
        let esc = self.keys().iter().any(|k| k.key == crate::ui::Key::Escape);
        if cancel || esc {
            done = true;
        }
        if save_pdf {
            let docs = glove_config::arquivos::user_dir("DOCUMENTS");
            p.save = Some(FileBrowser::save(docs, &format!("{}.pdf", Path::new(&p.name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.name.clone()))));
        }
        if preview {
            match p.temp_pdf() {
                Ok(f) => {
                    let viewer = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("glove-view"))).filter(|v| v.is_file()).unwrap_or_else(|| "glove-view".into());
                    if let Ok(mut c) = std::process::Command::new(viewer).arg(f).spawn() {
                        std::thread::spawn(move || c.wait());
                    }
                }
                Err(e) => p.message = Some(e),
            }
        }
        if go && !p.printers.is_empty() {
            match p.print() {
                Ok(()) => done = true,
                Err(e) => p.message = Some(e),
            }
        }
        p.first = false;
        done
    }
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 56.0;
const FONT: f32 = 11.0;
const LEADING: f32 = 14.0;

/// Larguras da Helvetica (AFM padrão) de 32 a 126, em milésimos.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Caractere → byte da codificação WinAnsi (a das fontes padrão do PDF).
fn win_ansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '€' => 0x80,
        '‚' => 0x82,
        '„' => 0x84,
        '…' => 0x85,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '\t' => b' ',
        _ => b'?',
    }
}

fn char_w(c: char) -> f32 {
    let b = win_ansi(c);
    let w = if (32..=126).contains(&b) { HELV[(b - 32) as usize] } else { 556 };
    w as f32 * FONT / 1000.0
}

/// Quebra o texto nas palavras para caber na largura da página.
fn wrap(text: &str, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut lw = 0.0;
        for word in para.split_inclusive(' ') {
            let ww: f32 = word.chars().map(char_w).sum();
            if lw + ww > max && !line.is_empty() {
                out.push(std::mem::take(&mut line));
                lw = 0.0;
            }
            // Palavra maior que a linha: corta nas letras.
            if ww > max {
                for ch in word.chars() {
                    let cw = char_w(ch);
                    if lw + cw > max {
                        out.push(std::mem::take(&mut line));
                        lw = 0.0;
                    }
                    line.push(ch);
                    lw += cw;
                }
                continue;
            }
            line.push_str(word);
            lw += ww;
        }
        out.push(line);
    }
    out
}

/// Monta o PDF com os objetos dados (o 1 é o catálogo); calcula a tabela xref.
fn assemble(objects: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend_from_slice(data);
    v.extend_from_slice(b"\nendstream");
    v
}

/// Texto em páginas A4 (Helvetica 11, com o nome do documento no rodapé).
pub fn text_pdf(title: &str, text: &str) -> Vec<u8> {
    let lines = wrap(text, PAGE_W - 2.0 * MARGIN);
    let per_page = ((PAGE_H - 2.0 * MARGIN) / LEADING) as usize;
    let pages: Vec<&[String]> = if lines.is_empty() { vec![&[][..]] } else { lines.chunks(per_page).collect() };
    let n = pages.len();
    // 1 catálogo, 2 páginas, 3 fonte; cada página: objeto da página + conteúdo.
    let mut objects: Vec<Vec<u8>> = vec![Vec::new(), Vec::new(), b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec()];
    let mut kids = Vec::new();
    for (pi, page) in pages.iter().enumerate() {
        let mut s: Vec<u8> = format!("BT /F1 {FONT} Tf {LEADING} TL {MARGIN} {} Td\n", PAGE_H - MARGIN - FONT).into_bytes();
        for l in page.iter() {
            s.push(b'(');
            for c in l.chars() {
                match win_ansi(c) {
                    b'(' => s.extend_from_slice(b"\\("),
                    b')' => s.extend_from_slice(b"\\)"),
                    b'\\' => s.extend_from_slice(b"\\\\"),
                    b => s.push(b),
                }
            }
            s.extend_from_slice(b") Tj T*\n");
        }
        s.extend_from_slice(b"ET\n");
        // Rodapé: nome e número da página.
        let foot = format!("{title} - {} de {n}", pi + 1);
        let fw: f32 = foot.chars().map(|c| char_w(c) * 0.8).sum();
        s.extend_from_slice(format!("BT /F1 {} Tf {} 30 Td (", FONT * 0.8, (PAGE_W - fw) / 2.0).as_bytes());
        for c in foot.chars() {
            let b = win_ansi(c);
            if b == b'(' || b == b')' || b == b'\\' {
                s.push(b'\\');
            }
            s.push(b);
        }
        s.extend_from_slice(b") Tj ET\n");
        let page_id = objects.len() + 1;
        kids.push(format!("{page_id} 0 R"));
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>", page_id + 1).into_bytes());
        objects.push(stream("", &s));
    }
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objects[1] = format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes();
    assemble(objects)
}

/// Uma imagem numa página A4, centrada e do maior tamanho que cabe nas margens.
pub fn image_pdf(path: &Path) -> Result<Vec<u8>, String> {
    let px = crate::image::decode(path)?;
    let (iw, ih) = (px.width(), px.height());
    // Sobre fundo branco (a imagem vem com transparência pré-multiplicada).
    let mut rgb = Vec::with_capacity((iw * ih * 3) as usize);
    for p in px.data().chunks_exact(4) {
        let a = 255 - p[3] as u32;
        for c in &p[..3] {
            rgb.push((*c as u32 + a).min(255) as u8);
        }
    }
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);
    let (bw, bh) = (PAGE_W - 2.0 * MARGIN, PAGE_H - 2.0 * MARGIN);
    let k = (bw / iw as f32).min(bh / ih as f32);
    let (dw, dh) = (iw as f32 * k, ih as f32 * k);
    let (dx, dy) = ((PAGE_W - dw) / 2.0, (PAGE_H - dh) / 2.0);
    let content = format!("q {dw:.2} 0 0 {dh:.2} {dx:.2} {dy:.2} cm /Im1 Do Q\n");
    let objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /XObject << /Im1 4 0 R >> >> /Contents 5 0 R >>").into_bytes(),
        stream(&format!("/Type /XObject /Subtype /Image /Width {iw} /Height {ih} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode"), &data),
        stream("", content.as_bytes()),
    ];
    Ok(assemble(objects))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_de_texto() {
        let long = "palavra ".repeat(3000);
        let pdf = text_pdf("Teste", &format!("Olá, (mundo) — ação\n{long}"));
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        let count: u32 = s.split("/Count ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap();
        assert!(count > 1);
        assert!(s.contains("\\(mundo\\)"));
        assert!(s.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn quebra_de_linha() {
        let l = wrap("um dois três quatro", 40.0);
        assert!(l.len() > 1);
        assert!(l.iter().all(|x| x.chars().map(char_w).sum::<f32>() <= 41.0 || !x.contains(' ')));
    }
}
