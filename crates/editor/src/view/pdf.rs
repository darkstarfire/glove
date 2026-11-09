//! PDF vetorial do documento: as mesmas folhas da tela (quebras, cabeçalho,
//! rodapé, notas, tabelas, listas), com as fontes embutidas. O texto fica nítido
//! em qualquer zoom e pode ser selecionado e copiado.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use ab_glyph::{Font, FontVec, GlyphId};

use super::{DocView, CELL_PAD, NOTE_SEP};
use crate::fontbook::FontBook;
use crate::layout::{note_label, Line};
use crate::rich::{CharStyle, Doc, Para, Role, LIST_INDENT};

/// Uma face usada no PDF: o arquivo, a fonte lida e os glifos usados (para as
/// larguras e para copiar o texto).
struct PdfFont {
    key: (String, bool, bool),
    font: FontVec,
    data: Vec<u8>,
    cff: bool,
    name: String,
    used: BTreeMap<u16, char>,
}

struct Writer<'a> {
    fb: &'a FontBook,
    fonts: Vec<PdfFont>,
}

fn num(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn rgb(c: [u8; 4]) -> String {
    format!("{} {} {}", num(c[0] as f32 / 255.0), num(c[1] as f32 / 255.0), num(c[2] as f32 / 255.0))
}

impl Writer<'_> {
    /// Índice da face do estilo (lê e guarda na primeira vez).
    fn font(&mut self, st: &CharStyle) -> Option<usize> {
        let key = (st.family.clone(), st.bold, st.italic);
        if let Some(i) = self.fonts.iter().position(|f| f.key == key) {
            return Some(i);
        }
        let (path, index) = self.fb.file(st).or_else(|| fallback_file(st))?;
        let mut data = std::fs::read(&path).ok()?;
        // Coleção (.ttc): o PDF não aceita; usa a Liberation equivalente.
        let (path, index) = if data.starts_with(b"ttcf") {
            let (p, i) = fallback_file(st)?;
            data = std::fs::read(&p).ok()?;
            (p, i)
        } else {
            (path, index)
        };
        let font = FontVec::try_from_vec_and_index(data.clone(), index).ok()?;
        let cff = data.starts_with(b"OTTO");
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Fonte".into());
        let name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        self.fonts.push(PdfFont { key, font, data, cff, name, used: BTreeMap::new() });
        Some(self.fonts.len() - 1)
    }

    /// Texto na posição (x, linha de base) em pontos do PDF.
    fn text(&mut self, out: &mut String, st: &CharStyle, x: f32, base: f32, s: &str) {
        if s.trim().is_empty() {
            return;
        }
        let Some(fi) = self.font(st) else { return };
        let size = FontBook::eff_size(st);
        let f = &mut self.fonts[fi];
        let upem = f.font.units_per_em().unwrap_or(1000.0);
        let _ = write!(out, "BT {} rg /F{} {} Tf {} {} Td [<", rgb(st.color), fi + 1, num(size), num(x), num(base));
        let mut prev: Option<GlyphId> = None;
        for c in s.chars() {
            let g = f.font.glyph_id(c);
            if let Some(p) = prev {
                let k = f.font.kern_unscaled(p, g);
                if k != 0.0 {
                    let _ = write!(out, "> {} <", num(-k * 1000.0 / upem));
                }
            }
            let _ = write!(out, "{:04X}", g.0);
            f.used.entry(g.0).or_insert(c);
            prev = Some(g);
        }
        out.push_str(">] TJ ET\n");
    }

    fn rect(out: &mut String, x: f32, y: f32, w: f32, h: f32, c: [u8; 4]) {
        let _ = writeln!(out, "{} rg {} {} {} {} re f", rgb(c), num(x), num(y), num(w), num(h));
    }
}

/// Fonte de reserva (Liberation) para quando a face não pode ser embutida.
fn fallback_file(st: &CharStyle) -> Option<(PathBuf, u32)> {
    let fam = st.family.to_ascii_lowercase();
    let base = if fam.contains("times") || fam.contains("serif") && !fam.contains("sans") { "LiberationSerif" } else if fam.contains("mono") || fam.contains("courier") { "LiberationMono" } else { "LiberationSans" };
    let style = match (st.bold, st.italic) {
        (true, true) => "BoldItalic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        _ => "Regular",
    };
    let p = PathBuf::from(format!("/usr/share/fonts/truetype/liberation/{base}-{style}.ttf"));
    p.is_file().then_some((p, 0))
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let z = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
    let mut v = format!("<< {dict} /Filter /FlateDecode /Length {} >>\nstream\n", z.len()).into_bytes();
    v.extend_from_slice(&z);
    v.extend_from_slice(b"\nendstream");
    v
}

fn assemble(objects: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = b"%PDF-1.6\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offs = Vec::with_capacity(objects.len());
    for (i, o) in objects.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

impl DocView {
    /// Desenha uma linha no PDF (`x0` = começo da linha, `top` = topo da linha;
    /// `y` converte a altura do documento para a do PDF).
    fn pdf_line(&self, w: &mut Writer, out: &mut String, p: &Para, ln: &Line, x0: f32, top: f32, y: &dyn Fn(f32) -> f32) {
        let base = top + ln.base;
        for s in &ln.segs {
            let st = &s.style;
            let x = x0 + s.x;
            if let Some(h) = st.highlight {
                let (asc, desc) = w.fb.metrics(st);
                Writer::rect(out, x, y(base + desc), s.w, asc + desc, h);
            }
            if s.tab {
                // Linha do sumário: pontilhado.
                if p.style.role == Role::Toc {
                    let mut dx = x + 4.0;
                    while dx < x + s.w - 6.0 {
                        Writer::rect(out, dx, y(base), 1.0, 1.0, [90, 90, 96, 255]);
                        dx += 4.0;
                    }
                }
                continue;
            }
            let label = note_label(st, &self.nums);
            let (st, text): (&CharStyle, &str) = match &label {
                Some((sup, t)) => (sup, t.as_str()),
                None => (st, &p.text[s.a..s.b]),
            };
            let shift = match st.script {
                1 => -st.size * 0.33,
                -1 => st.size * 0.15,
                _ => 0.0,
            };
            w.text(out, st, x, y(base + shift), text.trim_end_matches(' '));
            let th = (st.size / 14.0).max(0.5);
            let tw = w.fb.measure(st, text.trim_end_matches(' '));
            if st.underline {
                Writer::rect(out, x, y(base + shift + 1.5 + th), tw, th, st.color);
            }
            if st.strike {
                Writer::rect(out, x, y(base + shift - st.size * 0.3 + th), tw, th, st.color);
            }
        }
    }

    /// O documento todo em PDF (o modo página precisa estar diagramado).
    fn pdf_bytes(&self, doc: &Doc, fb: &FontBook) -> Vec<u8> {
        let mut w = Writer { fb, fonts: Vec::new() };
        let pg = &self.page;
        let (pw, ph) = (pg.width, pg.height);
        let mut contents: Vec<String> = Vec::new();
        for k in 0..self.pages {
            let mut out = String::new();
            let top = self.page_top(k);
            let px = self.page_x;
            let y = |yd: f32| ph - (yd - top);
            let on_page = |yd: f32| yd >= top - 0.5 && yd < top + ph;
            // Texto (parágrafos e células).
            for (pi, p) in doc.paras.iter().enumerate() {
                let l = self.lay(p);
                let x0 = self.col_x + self.para_x[pi] - px;
                for (li, ln) in l.lines.iter().enumerate() {
                    let ly = self.lys[pi][li];
                    if !on_page(ly) {
                        continue;
                    }
                    if li == 0 {
                        if let Some(Some(label)) = self.labels.get(pi) {
                            let st = &p.runs[0].style;
                            w.text(&mut out, st, x0 + p.style.left_indent - LIST_INDENT + 4.0, y(ly + ln.base), label);
                        }
                    }
                    self.pdf_line(&mut w, &mut out, p, ln, x0, ly, &y);
                }
            }
            // Bordas das tabelas.
            for t in self.tables.iter().filter(|t| t.borders) {
                for (rt, rh, cells) in &t.rows {
                    if !on_page(*rt) {
                        continue;
                    }
                    for (col, span) in cells {
                        let x0 = self.col_x + t.bounds[*col] - px;
                        let x1 = self.col_x + t.bounds[(col + span).min(t.bounds.len() - 1)] - px;
                        let _ = writeln!(out, "0.24 0.24 0.26 RG 0.5 w {} {} {} {} re S", num(x0), num(y(rt + rh)), num(x1 - x0), num(*rh));
                    }
                }
            }
            let _ = CELL_PAD;
            // Cabeçalho e rodapé.
            if !(k == 0 && self.title_page) {
                let st = CharStyle { size: 10.0, color: [80, 80, 86, 255], family: self.base_family.clone(), ..CharStyle::default() };
                let (asc, _) = fb.metrics(&st);
                let x0 = pg.margin_left;
                let cw = self.width;
                for (hf, base) in [(&self.header, pg.margin_top / 2.0 + asc / 2.0), (&self.footer, ph - pg.margin_bottom / 2.0 + asc / 2.0)] {
                    if hf.is_empty() {
                        continue;
                    }
                    let [l, m, r] = hf.fill(k + 1, self.pages, &self.title, &self.date);
                    let by = ph - base;
                    w.text(&mut out, &st, x0, by, &l);
                    w.text(&mut out, &st, x0 + (cw - fb.measure(&st, &m)) / 2.0, by, &m);
                    w.text(&mut out, &st, x0 + cw - fb.measure(&st, &r), by, &r);
                }
            }
            // Notas no pé da folha.
            if let Some(ids) = self.page_notes.get(k).filter(|v| !v.is_empty()) {
                let total: f32 = NOTE_SEP + ids.iter().map(|id| self.note_lays.get(id).map_or(0.0, |l| l.3.h)).sum::<f32>();
                let mut yd = top + ph - pg.margin_bottom - total;
                let x0 = pg.margin_left;
                Writer::rect(&mut out, x0, y(yd + NOTE_SEP * 0.4), self.width / 3.0, 0.5, [60, 60, 66, 255]);
                yd += NOTE_SEP;
                for id in ids {
                    if let Some((_, _, para, lay)) = self.note_lays.get(id) {
                        for ln in &lay.lines {
                            self.pdf_line(&mut w, &mut out, para, ln, x0, yd + ln.y, &y);
                        }
                        yd += lay.h;
                    }
                }
            }
            contents.push(out);
        }
        // Objetos: 1 catálogo, 2 páginas, 3 informações, depois as fontes e as páginas.
        let mut objects: Vec<Vec<u8>> = vec![Vec::new(), Vec::new()];
        let title: String = self.title.chars().map(|c| if c == '(' || c == ')' || c == '\\' { ' ' } else { c }).collect();
        let utf16: Vec<u16> = title.encode_utf16().collect();
        let mut t_hex = String::from("FEFF");
        for u in utf16 {
            let _ = write!(t_hex, "{u:04X}");
        }
        objects.push(format!("<< /Title <{t_hex}> /Producer (Glove Editor de Texto) >>").into_bytes());
        let mut font_refs = String::new();
        for (i, f) in w.fonts.iter().enumerate() {
            let upem = f.font.units_per_em().unwrap_or(1000.0);
            let k = 1000.0 / upem;
            let base = objects.len() + 1;
            // base: Type0, base+1: CIDFont, base+2: descritor, base+3: arquivo, base+4: ToUnicode.
            let mut widths = String::new();
            for g in f.used.keys() {
                let adv = f.font.h_advance_unscaled(GlyphId(*g)) * k;
                let _ = write!(widths, "{g} [{}] ", num(adv));
            }
            let sub = if f.cff { "CIDFontType0" } else { "CIDFontType2" };
            objects.push(format!("<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>", f.name, base + 1, base + 4).into_bytes());
            let gidmap = if f.cff { "" } else { "/CIDToGIDMap /Identity" };
            objects.push(format!("<< /Type /Font /Subtype /{sub} /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 500 /W [{widths}] {gidmap} >>", f.name, base + 2).into_bytes());
            let flags = 32 + if f.key.2 { 64 } else { 0 };
            let (asc, desc) = (f.font.ascent_unscaled() * k, f.font.descent_unscaled() * k);
            let file_key = if f.cff { "FontFile3" } else { "FontFile2" };
            objects.push(format!("<< /Type /FontDescriptor /FontName /{} /Flags {flags} /FontBBox [-600 {} 1600 {}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight 700 /StemV 80 /{file_key} {} 0 R >>", f.name, num(desc), num(asc), if f.key.2 { -12 } else { 0 }, num(asc), num(desc), base + 3).into_bytes());
            let dict = if f.cff { "/Subtype /OpenType".to_string() } else { format!("/Length1 {}", f.data.len()) };
            objects.push(stream(&dict, &f.data));
            // ToUnicode: copiar texto do PDF dá as letras certas.
            let mut cmap = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Adobe-Identity-UCS def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
            let pairs: Vec<(&u16, &char)> = f.used.iter().collect();
            for chunk in pairs.chunks(100) {
                let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
                for (g, c) in chunk {
                    let mut buf = [0u16; 2];
                    let u: String = c.encode_utf16(&mut buf).iter().map(|x| format!("{x:04X}")).collect();
                    let _ = writeln!(cmap, "<{:04X}> <{u}>", g);
                }
                cmap.push_str("endbfchar\n");
            }
            cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
            objects.push(stream("", cmap.as_bytes()));
            let _ = write!(font_refs, "/F{} {} 0 R ", i + 1, base);
        }
        let mut kids = Vec::new();
        for c in &contents {
            let page_id = objects.len() + 1;
            kids.push(format!("{page_id} 0 R"));
            objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {font_refs}>> >> /Contents {} 0 R >>", num(pw), num(ph), page_id + 1).into_bytes());
            objects.push(stream("", c.as_bytes()));
        }
        objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
        objects[1] = format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), contents.len()).into_bytes();
        let mut pdf = assemble(objects);
        // As informações (objeto 3) no trailer.
        if let Some(pos) = pdf.windows(9).rposition(|w| w == b"/Root 1 0") {
            pdf.splice(pos..pos, b"/Info 3 0 R ".iter().copied());
        }
        pdf
    }
}

/// O documento em PDF, com as folhas como no modo página.
pub fn export_pdf(doc: &Doc, fb: &FontBook, title: &str) -> Vec<u8> {
    let mut v = DocView::new();
    v.page_mode = true;
    v.title = title.to_string();
    let area = doc.page.width + 2.0 * super::GAP;
    v.ensure(doc, fb, area);
    v.pdf_bytes(doc, fb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich::{Align, ParaStyle, Pos};
    use std::rc::Rc;

    fn sample() -> Doc {
        let mut d = Doc::from_plain("Relatório de Atividades\nEste parágrafo é justificado e tem uma nota de rodapé, para conferir o PDF.\nPrimeiro item\nSegundo item\nDepois da lista.", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 0), false);
        d.apply_role(Role::Title);
        d.anchor = Pos::new(1, 0);
        d.cursor = Pos::new(1, 3);
        d.set_para_style(&|s| {
            s.align = Align::Justify;
            s.first_indent = 35.4;
        });
        d.set_cursor(Pos::new(1, d.paras[1].text.len()), false);
        d.insert_note("Fonte: Glove, 2026.");
        d.anchor = Pos::new(2, 0);
        d.cursor = Pos::new(3, 0);
        d.set_list(Some(false));
        let n = d.paras.len();
        d.set_cursor(Pos::new(n - 1, d.paras[n - 1].text.len()), false);
        crate::table::insert(&mut d, 2, 2);
        let f = d.cursor.p;
        for (i, t) in ["Mês", "Horas", "Março", "120"].iter().enumerate() {
            d.set_cursor(Pos::new(f + i, 0), false);
            d.insert_text(t);
        }
        d.footer.center = "Página {página} de {páginas}".into();
        d.header.right = "Glove".into();
        d
    }

    #[test]
    fn pdf_valido() {
        let fb = FontBook::new();
        let pdf = export_pdf(&sample(), &fb, "Teste");
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.6"));
        assert!(s.contains("/FontFile"));
        assert!(s.trim_end().ends_with("%%EOF"));
    }

    /// Manual: grava o PDF de exemplo em $PDF_OUT.
    #[test]
    #[ignore]
    fn gravar_pdf() {
        let Some(out) = std::env::var_os("PDF_OUT") else { return };
        let fb = FontBook::new();
        std::fs::write(out, export_pdf(&sample(), &fb, "Relatório")).unwrap();
    }
}
