//! A área de edição do documento rico: teclado, mouse, seleção, rolagem e o
//! desenho (só o que aparece na tela). Cada parágrafo guarda a sua diagramação
//! e só é refeito quando muda.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use glove_ui::paint::draw::*;
use glove_ui::ui::Key;
use glove_ui::{Rect, Ui};
use glove_ui::paint::{tiny_skia, Pixmap};

mod pdf;
pub use pdf::export_pdf;

use crate::fontbook::FontBook;
use crate::layout::{hit_line, layout_para, line_of, note_label, x_of, NoteNums, ParaLayout};
use crate::rich::{CharStyle, Doc, Frag, HeaderFooter, PageSetup, Para, ParaStyle, Pos};

/// Margens do texto no modo "ajustar à janela" (pontos).
const MARGIN_X: f32 = 24.0;
const MARGIN_Y: f32 = 14.0;
/// Limites do zoom e os degraus de Aumentar/Diminuir.
pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 4.0;
pub const ZOOM_STEPS: [f32; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
/// Modo página: espaço entre as folhas e em volta delas (pontos).
const GAP: f32 = 18.0;
/// Espaço do fio que separa as notas do texto (pontos).
const NOTE_SEP: f32 = 12.0;
/// Margem interna das células e espaço depois de uma tabela (pontos).
const CELL_PAD: f32 = 4.0;
const TABLE_AFTER: f32 = 6.0;

/// Onde uma tabela ficou: colunas (x a partir da coluna de texto) e linhas.
struct TableGeo {
    id: u32,
    borders: bool,
    bounds: Vec<f32>,
    /// (topo, altura, células: (coluna, quantas colunas)).
    rows: Vec<(f32, f32, Vec<(usize, usize)>)>,
    /// Parágrafos da tabela.
    first: usize,
    end: usize,
}

thread_local! {
    /// O último pedaço copiado no Editor, com os estilos (os outros apps só veem o texto).
    static CLIP: std::cell::RefCell<Option<(String, Frag)>> = const { std::cell::RefCell::new(None) };
}

pub struct DocView {
    /// Rolagem (pixels da tela) e zoom (pixels por ponto).
    pub scroll: f32,
    /// Rolagem para os lados (folha mais larga que a janela).
    pub hscroll: f32,
    /// Largura do documento (pontos): a folha com a mesa em volta, ou a área.
    doc_w: f32,
    pub zoom: f32,
    cache: HashMap<u64, ParaLayout>,
    /// Modo página (folhas com margens) ou ajustado à janela.
    pub page_mode: bool,
    /// Zoom que faz a folha caber na largura da janela.
    pub fit_width: bool,
    /// Topo de cada linha de cada parágrafo (pontos, no espaço do documento).
    lys: Vec<Vec<f32>>,
    /// Altura total do documento (pontos), largura da coluna de texto e onde ela começa.
    total: f32,
    width: f32,
    col_x: f32,
    /// Modo página: o papel, quantas folhas e onde fica a folha (x, pontos).
    page: PageSetup,
    pages: usize,
    page_x: f32,
    /// Notas: número de cada uma, a diagramação delas e quais vão em cada folha.
    nums: NoteNums,
    note_lays: HashMap<u32, (String, f32, Para, ParaLayout)>,
    page_notes: Vec<Vec<u32>>,
    /// Modo janela: as notas no fim do texto (número interno e topo).
    endnotes: Vec<(u32, f32)>,
    header: HeaderFooter,
    footer: HeaderFooter,
    title_page: bool,
    base_family: String,
    /// Nome do documento e a data de hoje (campos do cabeçalho e rodapé).
    pub title: String,
    date: String,
    /// Clique duplo numa marca de nota: o editor abre a nota.
    pub edit_note: Option<u32>,
    /// Marcador ou número de cada item de lista.
    labels: Vec<Option<String>>,
    /// Folha em que cada parágrafo começa (modo página).
    para_page: Vec<usize>,
    /// Posição x e largura de cada parágrafo (células de tabela são mais estreitas).
    para_x: Vec<f32>,
    para_w: Vec<f32>,
    tables: Vec<TableGeo>,
    /// Arrastando a borda entre duas colunas: (tabela, qual borda).
    col_drag: Option<(u32, usize)>,
    /// Leva o cursor para dentro da área visível no próximo quadro.
    pub reveal: bool,
    want_x: Option<f32>,
    dragging: bool,
    /// Cliques seguidos: quando, onde e quantos (duplo, triplo).
    last_click: Option<(Instant, (f32, f32), u8)>,
    /// Esperando o texto colado.
    pasting: bool,
    /// Imagens abertas: a original e a reduzida para o tamanho da tela.
    pics: std::cell::RefCell<HashMap<u64, Pic>>,
    /// Puxando o canto de uma imagem: onde ela está e o tamanho novo (pontos).
    img_drag: Option<(Pos, f32, f32)>,
    /// Onde o documento começa na tela (pixels, já com a rolagem para os lados).
    origin: (f32, f32),
}

/// Uma imagem do documento já aberta.
struct Pic {
    full: Option<std::rc::Rc<Pixmap>>,
    /// Reduzida para o último tamanho desenhado (largura e altura em pixels).
    scaled: Option<(u32, u32, Pixmap)>,
}

/// Reduz a imagem para `w` × `h` pixels (pela metade várias vezes e depois o
/// resto, para não serrilhar uma foto grande).
fn scale_pixmap(src: &Pixmap, w: u32, h: u32) -> Option<Pixmap> {
    let mut cur: Option<Pixmap> = None;
    loop {
        let s = cur.as_ref().unwrap_or(src);
        let half = s.width() / 2 >= w && s.height() / 2 >= h;
        let (tw, th) = if half { (s.width() / 2, s.height() / 2) } else { (w, h) };
        let mut out = Pixmap::new(tw.max(1), th.max(1))?;
        let t = tiny_skia::Transform::from_scale(tw.max(1) as f32 / s.width() as f32, th.max(1) as f32 / s.height() as f32);
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
        out.draw_pixmap(0, 0, s.as_ref(), &paint, t, None);
        cur = Some(out);
        if !half {
            return cur;
        }
    }
}

impl Default for DocView {
    fn default() -> Self {
        Self::new()
    }
}

/// Uma posição na tela: x, topo da linha e altura do texto (pontos).
#[derive(Clone, Copy, Debug)]
pub struct Caret {
    pub x: f32,
    pub y: f32,
    pub h: f32,
}

impl DocView {
    pub fn new() -> Self {
        Self { scroll: 0.0, hscroll: 0.0, doc_w: 0.0, zoom: 1.0, cache: HashMap::new(), page_mode: false, fit_width: false, lys: Vec::new(), total: 0.0, width: 0.0, col_x: MARGIN_X, page: PageSetup::default(), pages: 1, page_x: GAP, nums: NoteNums::new(), note_lays: HashMap::new(), page_notes: Vec::new(), endnotes: Vec::new(), header: HeaderFooter::default(), footer: HeaderFooter::default(), title_page: false, base_family: String::new(), title: String::new(), date: today(), edit_note: None, labels: Vec::new(), para_page: Vec::new(), para_x: Vec::new(), para_w: Vec::new(), tables: Vec::new(), col_drag: None, reveal: false, want_x: None, dragging: false, last_click: None, pasting: false, pics: Default::default(), img_drag: None, origin: (0.0, 0.0) }
    }

    fn lay<'a>(&'a self, p: &Para) -> &'a ParaLayout {
        &self.cache[&p.id]
    }

    /// Topo de cada folha (pontos).
    fn page_top(&self, k: usize) -> f32 {
        GAP + k as f32 * (self.page.height + GAP)
    }

    /// Diagrama o que mudou e põe cada linha no lugar (numa coluna ou em folhas).
    /// `area_w` = largura da área em pontos.
    fn ensure(&mut self, doc: &Doc, fb: &FontBook, area_w: f32) {
        self.page = doc.page.clone();
        self.header = doc.header.clone();
        self.footer = doc.footer.clone();
        self.title_page = doc.title_page;
        self.base_family = doc.paras[0].runs[0].style.family.clone();
        let pg = &self.page;
        let width = if self.page_mode { pg.width - pg.margin_left - pg.margin_right } else { area_w - 2.0 * MARGIN_X }.max(60.0);
        self.width = width;
        // Números das notas: mudou a ordem, as marcas mudam de largura.
        let order = doc.note_order();
        let nums: NoteNums = order.iter().enumerate().map(|(i, id)| (*id, i + 1)).collect();
        if nums != self.nums {
            self.nums = nums;
            self.cache.retain(|_, l| l.ver == 0);
        }
        // Tabelas: cada célula com a sua largura (as colunas pelas frações da tabela).
        let n = doc.paras.len();
        self.para_x = vec![0.0; n];
        self.para_w = vec![width; n];
        self.tables.clear();
        let mut i = 0;
        while i < n {
            let Some(c0) = doc.paras[i].style.cell else {
                i += 1;
                continue;
            };
            let end = (i..n).find(|&k| doc.paras[k].style.cell.is_none_or(|c| c.table != c0.table)).unwrap_or(n);
            let cols = doc.paras[i..end].iter().filter_map(|p| p.style.cell.map(|c| (c.col + c.span.max(1)) as usize)).max().unwrap_or(1);
            let props = doc.tables.get(&c0.table).cloned().unwrap_or_default();
            let fr = props.fractions(cols);
            let mut bounds = vec![0.0f32];
            for f in &fr {
                let last = *bounds.last().unwrap();
                bounds.push(last + f * width);
            }
            for k in i..end {
                let c = doc.paras[k].style.cell.unwrap();
                let x0 = bounds[(c.col as usize).min(cols)];
                let x1 = bounds[((c.col + c.span.max(1)) as usize).min(cols)];
                self.para_x[k] = x0 + CELL_PAD;
                self.para_w[k] = (x1 - x0 - 2.0 * CELL_PAD).max(12.0);
            }
            self.tables.push(TableGeo { id: c0.table, borders: props.borders, bounds, rows: Vec::new(), first: i, end });
            i = end;
        }
        for (k, p) in doc.paras.iter().enumerate() {
            let w = self.para_w[k];
            let fresh = self.cache.get(&p.id).is_some_and(|l| l.ver == p.ver && (l.width - w).abs() < 0.01);
            if !fresh {
                self.cache.insert(p.id, layout_para(p, w, fb, &self.nums));
            }
        }
        // O texto das notas, diagramado em letra menor.
        let note_style = std::rc::Rc::new(CharStyle { size: 9.0, family: self.base_family.clone(), ..CharStyle::default() });
        for (i, id) in order.iter().enumerate() {
            let text = format!("{}. {}", i + 1, doc.note_text(*id).unwrap_or(""));
            let fresh = self.note_lays.get(id).is_some_and(|(t, w, _, _)| *t == text && (*w - width).abs() < 0.01);
            if !fresh {
                let para = Para::new(&text, note_style.clone(), ParaStyle { space_after: 2.0, ..ParaStyle::default() });
                let lay = layout_para(&para, width, fb, &NoteNums::new());
                self.note_lays.insert(*id, (text, width, para, lay));
            }
        }
        self.note_lays.retain(|id, _| order.contains(id));
        let note_h = |id: &u32, lays: &HashMap<u32, (String, f32, Para, ParaLayout)>| lays.get(id).map_or(0.0, |l| l.3.h);
        // Marcadores e números das listas (a numeração recomeça quando a lista para).
        self.labels.clear();
        let mut counters = [0usize; 9];
        for p in &doc.paras {
            match p.style.list {
                Some(l) => {
                    let lv = (l.level as usize).min(8);
                    for c in counters.iter_mut().skip(lv + 1) {
                        *c = 0;
                    }
                    if l.numbered {
                        counters[lv] += 1;
                        self.labels.push(Some(format!("{}.", counters[lv])));
                    } else {
                        self.labels.push(Some(["•", "◦", "▪"][lv % 3].to_string()));
                    }
                }
                None => {
                    counters = [0; 9];
                    self.labels.push(None);
                }
            }
        }
        self.para_page = vec![0; n];
        self.lys = vec![Vec::new(); n];
        let paged = self.page_mode;
        if paged {
            // Folhas centralizadas; o texto corre de uma para a outra pelas linhas.
            self.page_x = ((area_w - pg.width) / 2.0).max(GAP);
            self.col_x = self.page_x + pg.margin_left;
        } else {
            self.col_x = MARGIN_X;
        }
        let room = if paged { (pg.height - pg.margin_top - pg.margin_bottom).max(40.0) } else { f32::INFINITY };
        let (mt, ph) = (pg.margin_top, pg.height);
        let top = |k: usize| if paged { GAP + k as f32 * (ph + GAP) + mt } else { MARGIN_Y };
        let (mut page, mut y) = (0usize, 0.0f32);
        // Notas de cada folha e a altura que elas ocupam na folha atual.
        let mut page_notes: Vec<Vec<u32>> = vec![Vec::new()];
        let mut notes_h = 0.0f32;
        let mut placed: Vec<u32> = Vec::new();
        let mut ti = 0usize;
        let mut i = 0usize;
        while i < n {
            let p = &doc.paras[i];
            if p.style.page_break_before && y > 0.0 && paged {
                page += 1;
                y = 0.0;
                notes_h = 0.0;
                page_notes.push(Vec::new());
            }
            // Tabela: linha a linha (a linha inteira na mesma folha).
            if p.style.cell.is_some() && ti < self.tables.len() && self.tables[ti].first == i {
                let end = self.tables[ti].end;
                let mut rows: Vec<(f32, f32, Vec<(usize, usize)>)> = Vec::new();
                let mut k = i;
                while k < end {
                    let row = doc.paras[k].style.cell.unwrap().row;
                    let rend = (k..end).find(|&q| doc.paras[q].style.cell.unwrap().row != row).unwrap_or(end);
                    // Células da linha e a altura de cada uma.
                    let mut cells: Vec<(usize, usize, Vec<usize>)> = Vec::new();
                    for q in k..rend {
                        let c = doc.paras[q].style.cell.unwrap();
                        match cells.last_mut() {
                            Some(cl) if cl.0 == c.col as usize => cl.2.push(q),
                            _ => cells.push((c.col as usize, c.span.max(1) as usize, vec![q])),
                        }
                    }
                    let content = cells.iter().map(|cl| cl.2.iter().map(|q| self.cache[&doc.paras[*q].id].h).sum::<f32>()).fold(0.0f32, f32::max);
                    let row_h = content + 2.0 * CELL_PAD;
                    if paged && y + row_h > room && y > 0.0 {
                        page += 1;
                        y = 0.0;
                        notes_h = 0.0;
                        page_notes.push(Vec::new());
                    }
                    let row_top = top(page) + y;
                    for cl in &cells {
                        let mut cy = row_top + CELL_PAD;
                        for q in &cl.2 {
                            let l = &self.cache[&doc.paras[*q].id];
                            self.lys[*q] = l.lines.iter().map(|ln| cy + ln.y).collect();
                            self.para_page[*q] = page;
                            cy += l.h;
                        }
                    }
                    rows.push((row_top, row_h, cells.iter().map(|cl| (cl.0, cl.1)).collect()));
                    y += row_h;
                    k = rend;
                }
                self.tables[ti].rows = rows;
                y += TABLE_AFTER;
                ti += 1;
                i = end;
                continue;
            }
            let l = &self.cache[&p.id];
            let first_y = l.lines.first().map_or(0.0, |ln| ln.y);
            // O espaço antes do parágrafo some no topo da folha.
            if y > 0.0 || !paged {
                y += first_y;
            }
            let mut ys = Vec::with_capacity(l.lines.len());
            let mut first_page = None;
            for ln in &l.lines {
                // Notas que a linha traz: precisam caber no pé da mesma folha.
                let mut new: Vec<u32> = Vec::new();
                if paged {
                    for s in &ln.segs {
                        if let Some(id) = s.style.note {
                            if !placed.contains(&id) && !new.contains(&id) {
                                new.push(id);
                            }
                        }
                    }
                }
                let add = |notes_h: f32| new.iter().map(|id| note_h(id, &self.note_lays)).sum::<f32>() + if notes_h == 0.0 && !new.is_empty() { NOTE_SEP } else { 0.0 };
                if paged && y + ln.h + notes_h + add(notes_h) > room && y > 0.0 {
                    page += 1;
                    y = 0.0;
                    notes_h = 0.0;
                    page_notes.push(Vec::new());
                }
                ys.push(top(page) + y);
                first_page.get_or_insert(page);
                y += ln.h;
                notes_h += add(notes_h);
                placed.extend(new.iter().copied());
                page_notes[page].extend(new);
            }
            let last_end = l.lines.last().map_or(0.0, |ln| ln.y + ln.h);
            y += (l.h - last_end).max(0.0);
            self.lys[i] = ys;
            self.para_page[i] = first_page.unwrap_or(page);
            i += 1;
        }
        if paged {
            self.pages = page + 1;
            self.page_notes = page_notes;
            self.endnotes.clear();
            self.total = self.page_top(self.pages);
            self.doc_w = self.page.width + 2.0 * GAP;
        } else {
            self.pages = 1;
            self.page_notes.clear();
            // As notas vão no fim, depois de um fio.
            self.endnotes.clear();
            let mut y = MARGIN_Y + y;
            if !order.is_empty() {
                y += NOTE_SEP * 1.5;
                for id in &order {
                    self.endnotes.push((*id, y));
                    y += note_h(id, &self.note_lays);
                }
            }
            self.total = y + MARGIN_Y;
            self.doc_w = area_w;
        }
        // Esquece os parágrafos que não existem mais (de vez em quando).
        if self.cache.len() > doc.paras.len() * 2 + 64 {
            let ids: std::collections::HashSet<u64> = doc.paras.iter().map(|p| p.id).collect();
            self.cache.retain(|k, _| ids.contains(k));
        }
    }

    /// Onde fica a posição (pontos, no espaço do documento).
    pub fn caret(&self, doc: &Doc, fb: &FontBook, pos: Pos) -> Caret {
        let p = &doc.paras[pos.p];
        let l = self.lay(p);
        let li = line_of(l, pos.o);
        let ln = &l.lines[li];
        Caret { x: self.col_x + self.para_x[pos.p] + x_of(p, l, fb, li, pos.o), y: self.lys[pos.p][li], h: ln.h }
    }

    /// Posição mais perto do ponto (pontos, no espaço do documento).
    pub fn hit(&self, doc: &Doc, fb: &FontBook, x: f32, y: f32) -> Pos {
        // Numa tabela: a célula pela linha e pela coluna.
        let rx = x - self.col_x;
        for t in &self.tables {
            for (row_i, (top, h, cells)) in t.rows.iter().enumerate() {
                if y < *top || y >= top + h {
                    continue;
                }
                let Some(&(col, span)) = cells.iter().find(|(c, sp)| rx >= t.bounds[*c] && rx < t.bounds[(c + sp).min(t.bounds.len() - 1)]).or(cells.first().filter(|_| rx < 0.0)).or(cells.last()) else { continue };
                let _ = span;
                let members: Vec<usize> = (t.first..t.end).filter(|&q| doc.paras[q].style.cell.is_some_and(|c| c.col as usize == col) && self.row_of(t, q) == row_i).collect();
                let Some(&last) = members.last() else { continue };
                // O parágrafo da célula na altura do ponto (abaixo do texto: o último).
                let q = members.iter().copied().find(|&q| self.lys[q].last().zip(self.cache[&doc.paras[q].id].lines.last()).is_some_and(|(&ly, ln)| y < ly + ln.h)).unwrap_or(last);
                let l = self.lay(&doc.paras[q]);
                let ys = &self.lys[q];
                let li = (0..l.lines.len()).find(|&i| y < ys[i] + l.lines[i].h).unwrap_or(l.lines.len() - 1);
                return Pos::new(q, hit_line(&doc.paras[q], l, fb, li, rx - self.para_x[q]));
            }
        }
        // O parágrafo: o último cuja primeira linha começa acima do ponto.
        let pi = self.lys.partition_point(|ys| ys.first().is_some_and(|&t| t <= y)).saturating_sub(1).min(doc.paras.len() - 1);
        let p = &doc.paras[pi];
        let l = self.lay(p);
        let ys = &self.lys[pi];
        let li = (0..l.lines.len()).find(|&i| y < ys[i] + l.lines[i].h).unwrap_or(l.lines.len() - 1);
        // Acima de tudo, no topo de uma folha: a primeira linha do parágrafo.
        Pos::new(pi, hit_line(p, l, fb, li, x - self.col_x - self.para_x[pi]))
    }

    /// Muda o zoom mantendo parado o ponto `at` da área (pixels a partir do canto).
    pub fn zoom_around(&mut self, zoom: f32, at: (f32, f32)) {
        let old = self.zoom;
        let new = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        self.fit_width = false;
        if (new - old).abs() < 0.0001 {
            return;
        }
        self.zoom = new;
        self.scroll = ((self.scroll + at.1) / old * new - at.1).max(0.0);
        self.hscroll = ((self.hscroll + at.0) / old * new - at.0).max(0.0);
    }

    /// Quantas folhas (modo página) e em qual está o cursor.
    pub fn page_info(&self, doc: &Doc) -> (usize, usize) {
        let cur = if self.page_mode { self.caret_page(doc) } else { 0 };
        (cur + 1, self.pages.max(1))
    }

    /// Folha em que o cursor está (modo página).
    fn caret_page(&self, doc: &Doc) -> usize {
        if self.lys.len() != doc.paras.len() {
            return 0;
        }
        let p = &doc.paras[doc.cursor.p];
        let Some(l) = self.cache.get(&p.id) else { return 0 };
        let y = self.lys[doc.cursor.p][line_of(l, doc.cursor.o)];
        (((y - GAP) / (self.page.height + GAP)).floor().max(0.0) as usize).min(self.pages.saturating_sub(1))
    }

    /// Largura da coluna de texto do parágrafo (pontos).
    pub fn width_at(&self, p: usize) -> f32 {
        self.para_w.get(p).copied().unwrap_or(self.width).max(20.0)
    }

    /// Posição no documento de um ponto da janela (pixels).
    pub fn pos_at_screen(&self, doc: &Doc, fb: &FontBook, pt: (f32, f32)) -> Pos {
        if self.lys.len() != doc.paras.len() {
            return doc.cursor;
        }
        let z = self.zoom;
        self.hit(doc, fb, (pt.0 - self.origin.0) / z, (pt.1 - self.origin.1 + self.scroll) / z)
    }

    /// Retângulo da imagem que começa em `pos` (pontos: x, y, largura, altura).
    pub fn image_rect(&self, doc: &Doc, pos: Pos) -> Option<(f32, f32, f32, f32)> {
        let p = doc.paras.get(pos.p)?;
        let img = p.style_after(pos.o).image.clone()?;
        let l = self.lay(p);
        for (li, ln) in l.lines.iter().enumerate() {
            if let Some(sg) = ln.segs.iter().find(|sg| sg.a == pos.o && sg.style.image.is_some()) {
                let h = img.h * sg.w / img.w.max(0.01);
                return Some((self.col_x + self.para_x[pos.p] + sg.x, self.lys[pos.p][li] + ln.base - h, sg.w, h));
            }
        }
        None
    }

    /// A imagem debaixo do ponto (pontos, no espaço do documento).
    fn image_at(&self, doc: &Doc, fb: &FontBook, x: f32, y: f32) -> Option<Pos> {
        let pos = self.hit(doc, fb, x, y);
        let p = &doc.paras[pos.p];
        let prev = p.text[..pos.o].char_indices().next_back().map(|(i, _)| Pos::new(pos.p, i));
        [Some(pos), prev].into_iter().flatten().find(|&q| self.image_rect(doc, q).is_some_and(|(rx, ry, rw, rh)| x >= rx && x <= rx + rw && y >= ry && y <= ry + rh))
    }

    /// Índice da linha da tabela em que o parágrafo está.
    fn row_of(&self, t: &TableGeo, q: usize) -> usize {
        let y = self.lys[q].first().copied().unwrap_or(0.0);
        t.rows.iter().position(|(top, h, _)| y >= *top && y < top + h).unwrap_or(0)
    }

    /// Uma linha acima ou abaixo, mantendo o x (pela geometria: serve em tabelas e folhas).
    fn vertical(&mut self, doc: &Doc, fb: &FontBook, pos: Pos, delta: i32) -> Pos {
        let c = self.caret(doc, fb, pos);
        let x = *self.want_x.get_or_insert(c.x);
        let l = self.lay(&doc.paras[pos.p]);
        let li = line_of(l, pos.o);
        // Dentro do mesmo parágrafo, a linha vizinha direto.
        let target = li as i32 + delta;
        if target >= 0 && (target as usize) < l.lines.len() {
            return Pos::new(pos.p, hit_line(&doc.paras[pos.p], l, fb, target as usize, x - self.col_x - self.para_x[pos.p]));
        }
        if delta < 0 {
            if pos.p == 0 {
                return Pos::new(0, 0);
            }
            self.hit(doc, fb, x, c.y - 1.0)
        } else {
            let p = self.hit(doc, fb, x, c.y + c.h + 1.0);
            if p.p == pos.p && p.o == pos.o || (pos.p + 1 == doc.paras.len() && li + 1 == l.lines.len()) {
                return doc.end();
            }
            // Ainda no mesmo lugar (fim da tabela, salto de folha): o próximo parágrafo.
            if p <= pos {
                let q = (pos.p + 1).min(doc.paras.len() - 1);
                return Pos::new(q, 0);
            }
            p
        }
    }

    /// Começo e fim da linha (na tela) da posição.
    fn line_bounds(&self, doc: &Doc, pos: Pos) -> (Pos, Pos) {
        let p = &doc.paras[pos.p];
        let l = self.lay(p);
        let li = line_of(l, pos.o);
        let ln = &l.lines[li];
        let end = if li + 1 == l.lines.len() {
            ln.b
        } else {
            let t = &p.text[ln.a..ln.b];
            let trimmed = t.trim_end_matches(' ').len();
            ln.a + if trimmed < t.len() { trimmed } else { t.char_indices().next_back().map_or(0, |(i, _)| i) }
        };
        (Pos::new(pos.p, ln.a), Pos::new(pos.p, end))
    }

    /// Onde começa o texto na área (pixels a partir da esquerda dela).
    pub fn text_left(&self) -> f32 {
        self.col_x * self.zoom - self.hscroll
    }

    /// Folha (contando de 1) em que o parágrafo começa, como no modo página.
    pub fn page_of(&self, pi: usize) -> usize {
        self.para_page.get(pi).copied().unwrap_or(0) + 1
    }

    /// Largura da coluna de texto (pontos).
    pub fn column_width(&self) -> f32 {
        self.width
    }

    /// Altura do documento em pixels (com as margens).
    pub fn content_h(&self) -> f32 {
        self.total * self.zoom
    }

    /// A área de edição em `r`. Devolve verdadeiro se o documento mudou.
    pub fn ui(&mut self, ui: &mut Ui, doc: &mut Doc, fb: &FontBook, r: Rect, enabled: bool) -> bool {
        let sb_w = glove_ui::SCROLLBAR_W;
        // Ctrl + roda do mouse: zoom em volta do ponteiro (o ponto debaixo dele fica parado).
        let wheel0 = if enabled { ui.scroll_in(r) } else { 0.0 };
        let ctrl_zoom = wheel0 != 0.0 && ui.mods().ctrl;
        if ctrl_zoom {
            // Um dente da roda (15) = 10%.
            let k = (-wheel0 * 1.1f32.ln() / 15.0).exp();
            let pt = ui.pointer();
            self.zoom_around(self.zoom * k, (pt.0 - r.x, pt.1 - r.y));
        }
        if self.page_mode && self.fit_width {
            self.zoom = ((r.w - sb_w) / (doc.page.width + 2.0 * GAP)).clamp(ZOOM_MIN, ZOOM_MAX);
        }
        let z = self.zoom;
        let width = (r.w - sb_w) / z;
        self.ensure(doc, fb, width);
        let ver = doc.version;
        // Folha mais larga que a janela (zoom alto): barra de rolagem embaixo.
        let full = r;
        let wide = self.doc_w * z > r.w - sb_w + 0.5;
        let r = if wide { Rect::new(r.x, r.y, r.w, r.h - sb_w) } else { r };

        // Barra de rolagem (e a roda do mouse).
        let mut scroll = self.scroll;
        // Shift + roda: para os lados.
        let wheel = ui.scroll_in(r);
        let sideways = wheel != 0.0 && ui.mods().shift;
        let inner = ui.scroll_area(r, &mut scroll, self.content_h().max(r.h), 30.0);
        if ctrl_zoom {
            // A roda já virou zoom: não rola.
        } else if sideways {
            self.hscroll += wheel;
        } else {
            self.scroll = scroll;
        }
        let max_h = (self.doc_w * z - inner.w).max(0.0);
        self.hscroll = self.hscroll.clamp(0.0, max_h);
        let origin = (inner.x - self.hscroll, inner.y);
        self.origin = origin;
        let to_doc = |sx: f32, sy: f32, scroll: f32| ((sx - origin.0) / z, (sy - origin.1 + scroll) / z);

        // Mouse
        // O controle existe sempre (a numeração dos controles não pode mudar entre
        // quadros, senão um campo que pega o foco sozinho acerta o vizinho).
        let hit = ui.hit(inner);
        let resp = enabled.then_some(hit);
        // Borda entre colunas de uma tabela: arrastar muda as larguras.
        if let Some(resp) = resp {
            if resp.pressed {
                let pt = ui.pointer();
                let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                let rx = x - self.col_x;
                self.col_drag = self.tables.iter().find_map(|t| {
                    let inside = t.rows.iter().any(|(top, h, _)| y >= *top && y < top + h);
                    let k = (1..t.bounds.len() - 1).find(|&k| (t.bounds[k] - rx).abs() <= 3.0 / z)?;
                    inside.then_some((t.id, k))
                });
            }
        }
        if let Some((id, k)) = self.col_drag {
            if ui.mouse_down() {
                if ui.moved() {
                    if let Some(t) = self.tables.iter().find(|t| t.id == id) {
                        let pt = ui.pointer();
                        let (x, _) = to_doc(pt.0, pt.1, self.scroll);
                        let total = *t.bounds.last().unwrap();
                        let rx = (x - self.col_x).clamp(t.bounds[k - 1] + 12.0, t.bounds[k + 1] - 12.0);
                        let mut b = t.bounds.clone();
                        b[k] = rx;
                        let widths: Vec<f32> = b.windows(2).map(|w| (w[1] - w[0]) / total).collect();
                        doc.tables.entry(id).or_default().widths = widths;
                        doc.modified = true;
                        doc.version = doc.version.wrapping_add(1);
                    }
                }
            } else {
                self.col_drag = None;
            }
        }
        // Canto da imagem selecionada: puxar muda o tamanho (sem deformar).
        if let Some(resp) = resp.filter(|_| self.col_drag.is_none()) {
            if resp.pressed {
                let pt = ui.pointer();
                let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                if let Some((at, img)) = doc.selected_image() {
                    if let Some((rx, ry, rw, rh)) = self.image_rect(doc, at) {
                        if (x - (rx + rw)).abs() <= 6.0 / z && (y - (ry + rh)).abs() <= 6.0 / z {
                            let _ = img;
                            self.img_drag = Some((at, rw, rh));
                        }
                    }
                }
            }
        }
        if let Some((at, _, _)) = self.img_drag {
            if ui.mouse_down() {
                if let (Some((rx, ry, _, _)), Some(img)) = (self.image_rect(doc, at), doc.paras[at.p].style_after(at.o).image.clone()) {
                    let pt = ui.pointer();
                    let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                    // Pela diagonal: a maior das duas proporções.
                    let k = ((x - rx) / img.w).max((y - ry) / img.h).max(12.0 / img.w.min(img.h));
                    let max_w = self.para_w[at.p];
                    let k = k.min(max_w / img.w);
                    self.img_drag = Some((at, img.w * k, img.h * k));
                }
            } else {
                if let Some((at, w, h)) = self.img_drag.take() {
                    doc.resize_image(at, w.round(), h.round());
                }
            }
        }
        if let Some(resp) = resp.filter(|_| self.col_drag.is_none() && self.img_drag.is_none()) {
            if resp.pressed {
                let pt = ui.pointer();
                let n = match self.last_click {
                    Some((t, p, n)) if t.elapsed() < Duration::from_millis(450) && (p.0 - pt.0).abs() < 5.0 && (p.1 - pt.1).abs() < 5.0 => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), pt, n));
                let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                let pos = self.hit(doc, fb, x, y);
                // Clique duplo numa marca de nota: abre a nota.
                let at_note = {
                    let p = &doc.paras[pos.p];
                    p.style_after(pos.o).note.filter(|_| pos.o < p.text.len()).or(p.style_at(pos.o).note.filter(|_| pos.o > 0))
                };
                // Clique numa imagem: seleciona a imagem.
                let (dx, dy) = to_doc(pt.0, pt.1, self.scroll);
                let on_img = if ui.mods().shift { None } else { self.image_at(doc, fb, dx, dy) };
                match n {
                    _ if on_img.is_some() => {
                        let a = on_img.unwrap();
                        doc.set_cursor(a, false);
                        doc.set_cursor(Pos::new(a.p, a.o + crate::rich::NOTE_CHAR.len_utf8()), true);
                    }
                    2 if at_note.is_some() => {
                        self.edit_note = at_note;
                        doc.select_word(pos);
                    }
                    2 => doc.select_word(pos),
                    3 => doc.select_para(pos.p),
                    _ => doc.set_cursor(pos, ui.mods().shift),
                }
                self.want_x = None;
                self.dragging = true;
            }
        }
        if self.dragging {
            if ui.mouse_down() {
                if ui.moved() {
                    let pt = ui.pointer();
                    // Arrastando para fora: rola junto.
                    if pt.1 < inner.y {
                        self.scroll = (self.scroll - (inner.y - pt.1)).max(0.0);
                    } else if pt.1 > inner.y + inner.h {
                        self.scroll += pt.1 - inner.y - inner.h;
                    }
                    let (x, y) = to_doc(pt.0, pt.1, self.scroll);
                    let pos = self.hit(doc, fb, x, y);
                    if pos != doc.cursor {
                        doc.cursor = pos;
                        doc.typing = None;
                    }
                }
            } else {
                self.dragging = false;
            }
        }

        // Teclado (só com nenhum campo de texto com o cursor).
        if enabled && !ui.has_focus() {
            self.keyboard(ui, doc, fb, inner);
        }

        let changed = doc.version != ver;
        if changed {
            self.ensure(doc, fb, width);
        }
        // Cursor sempre à vista depois de mexer pelo teclado.
        if self.reveal {
            self.reveal = false;
            let c = self.caret(doc, fb, doc.cursor);
            let top = c.y * z;
            let bot = (c.y + c.h) * z;
            if top < self.scroll {
                // Na primeira linha de uma folha, mostra a folha desde o topo.
                self.scroll = if self.page_mode {
                    let k = ((c.y - GAP) / (self.page.height + GAP)).floor().max(0.0) as usize;
                    let pt = self.page_top(k);
                    if c.y - pt <= self.page.margin_top + 1.0 { (pt - GAP / 2.0) * z } else { top - 30.0 }
                } else if c.y <= MARGIN_Y + 1.0 {
                    0.0
                } else {
                    top - 30.0
                };
            } else if bot > self.scroll + inner.h {
                self.scroll = bot - inner.h + 30.0;
            }
            // E para os lados.
            let cx = c.x * z;
            if cx < self.hscroll + 10.0 {
                self.hscroll = cx - 30.0;
            } else if cx > self.hscroll + inner.w - 10.0 {
                self.hscroll = cx - inner.w + 30.0;
            }
            self.hscroll = self.hscroll.clamp(0.0, max_h);
        }
        self.scroll = self.scroll.clamp(0.0, (self.content_h() - inner.h).max(0.0));

        // A barra de baixo (o controle existe sempre, para a numeração não mudar).
        if wide {
            let mut hs = self.hscroll;
            ui.hscrollbar(Rect::new(full.x, full.y + full.h - sb_w, full.w - sb_w, sb_w), &mut hs, self.doc_w * z, inner.w, 30.0);
            self.hscroll = hs;
            ui.c.fill_rect(full.x + full.w - sb_w, full.y + full.h - sb_w, sb_w, sb_w, &solid(rgba(232, 232, 236, 255)));
        } else {
            ui.hit(Rect::new(0.0, 0.0, 0.0, 0.0));
        }

        // Desenho
        let active = ui.window_active();
        let show_caret = enabled && !ui.has_focus();
        let scroll = self.scroll;
        let hscroll = self.hscroll;
        let me = &*self;
        let docr = &*doc;
        ui.clipped(0xd0c_u64, inner, |c, _| {
            me.draw(c, docr, fb, (hscroll, scroll), inner.w, inner.h, active, show_caret);
        });
        changed
    }

    fn keyboard(&mut self, ui: &mut Ui, doc: &mut Doc, fb: &FontBook, inner: Rect) {
        // Texto colado que chegou.
        if self.pasting {
            if let Some(t) = ui.take_paste() {
                self.pasting = false;
                let rich = CLIP.with(|c| c.borrow().as_ref().filter(|(s, _)| *s == t).map(|(_, f)| f.clone()));
                match rich {
                    Some(f) => doc.insert_frag(f),
                    None => {
                        doc.insert_text(&t.replace("\r\n", "\n"));
                        doc.break_undo();
                    }
                }
                self.reveal = true;
            }
        }
        let typed: String = ui.typed().chars().filter(|c| !c.is_control()).collect();
        if !typed.is_empty() {
            for ch in typed.chars() {
                doc.insert_text(&ch.to_string());
            }
            self.want_x = None;
            self.reveal = true;
        }
        let keys = ui.keys().to_vec();
        for k in keys {
            let combo = k.combo.as_str();
            let shift = combo.contains("Shift+");
            let ctrl = combo.contains("Ctrl+");
            let mut moved: Option<Pos> = None;
            let mut keep_x = false;
            match k.key {
                Key::Left => {
                    moved = Some(if doc.has_selection() && !shift { doc.selection().0 } else if ctrl { doc.word_left(doc.cursor) } else { doc.prev_pos(doc.cursor) });
                }
                Key::Right => {
                    moved = Some(if doc.has_selection() && !shift { doc.selection().1 } else if ctrl { doc.word_right(doc.cursor) } else { doc.next_pos(doc.cursor) });
                }
                Key::Up => {
                    moved = Some(if ctrl { Pos::new(doc.cursor.p.saturating_sub(if doc.cursor.o == 0 { 1 } else { 0 }), 0) } else { self.vertical(doc, fb, doc.cursor, -1) });
                    keep_x = !ctrl;
                }
                Key::Down => {
                    moved = Some(if ctrl {
                        let p = (doc.cursor.p + 1).min(doc.paras.len() - 1);
                        if p == doc.cursor.p { doc.end() } else { Pos::new(p, 0) }
                    } else {
                        self.vertical(doc, fb, doc.cursor, 1)
                    });
                    keep_x = !ctrl;
                }
                Key::Home => moved = Some(if ctrl { Pos::new(0, 0) } else { self.line_bounds(doc, doc.cursor).0 }),
                Key::End => moved = Some(if ctrl { doc.end() } else { self.line_bounds(doc, doc.cursor).1 }),
                Key::PageUp | Key::PageDown => {
                    let c = self.caret(doc, fb, doc.cursor);
                    let x = *self.want_x.get_or_insert(c.x);
                    let page = (inner.h / self.zoom - 20.0).max(20.0);
                    let dy = if k.key == Key::PageUp { -page } else { page };
                    self.scroll = (self.scroll + dy * self.zoom).max(0.0);
                    moved = Some(self.hit(doc, fb, x, c.y + c.h / 2.0 + dy));
                    keep_x = true;
                }
                // Backspace no começo de uma célula (ou logo depois de uma tabela) não junta com a outra.
                Key::Backspace if !doc.has_selection() && doc.cursor.o == 0 && doc.cursor.p > 0 && doc.paras[doc.cursor.p].style.cell.map(|c| (c.table, c.row, c.col)) != doc.paras[doc.cursor.p - 1].style.cell.map(|c| (c.table, c.row, c.col)) => {
                    let p = doc.cursor.p;
                    if doc.paras[p].style.cell.is_none() && doc.paras[p].text.is_empty() {
                        // Parágrafo vazio depois da tabela: some, e o cursor vai para a última célula.
                        doc.delete_range(Pos::new(p, 0), doc.next_pos(Pos::new(p, 0)));
                        if p > 0 {
                            doc.set_cursor(Pos::new(p - 1, doc.paras[p - 1].text.len()), false);
                        }
                    } else {
                        doc.set_cursor(doc.prev_pos(doc.cursor), false);
                    }
                    self.reveal = true;
                }
                Key::Delete if !doc.has_selection() && doc.cursor.p + 1 < doc.paras.len() && doc.cursor.o == doc.paras[doc.cursor.p].text.len() && doc.paras[doc.cursor.p].style.cell.map(|c| (c.table, c.row, c.col)) != doc.paras[doc.cursor.p + 1].style.cell.map(|c| (c.table, c.row, c.col)) => {}
                Key::Backspace => {
                    if doc.has_selection() {
                        doc.delete_selection();
                    } else {
                        let to = if ctrl { doc.word_left(doc.cursor) } else { doc.prev_pos(doc.cursor) };
                        doc.delete_range(to, doc.cursor);
                    }
                    self.want_x = None;
                    self.reveal = true;
                }
                Key::Delete => {
                    if doc.has_selection() {
                        doc.delete_selection();
                    } else {
                        let to = if ctrl { doc.word_right(doc.cursor) } else { doc.next_pos(doc.cursor) };
                        doc.delete_range(doc.cursor, to);
                    }
                    self.want_x = None;
                    self.reveal = true;
                }
                // Tabela: Tab vai para a célula seguinte (na última, cria uma linha); Shift+Tab volta.
                Key::Tab if !ctrl && doc.paras[doc.cursor.p].style.cell.is_some() => {
                    match crate::table::next_cell(doc, doc.cursor.p, !shift) {
                        Some(p) => doc.set_cursor(p, false),
                        None if !shift => {
                            crate::table::add_row(doc, true);
                            if let Some(p) = crate::table::next_cell(doc, doc.cursor.p, true) {
                                doc.set_cursor(p, false);
                            }
                            let p = doc.cursor.p;
                            // O cursor fica na primeira célula da linha nova.
                            if let Some((_, a, b)) = crate::table::block(doc, p) {
                                let row = doc.paras[b - 1].style.cell.unwrap().row;
                                if let Some(q) = (a..b).find(|&q| doc.paras[q].style.cell.unwrap().row == row) {
                                    doc.set_cursor(Pos::new(q, 0), false);
                                }
                            }
                        }
                        None => {}
                    }
                    self.want_x = None;
                    self.reveal = true;
                }
                // Enter num item de lista vazio: a lista acaba.
                Key::Enter if !ctrl && !doc.has_selection() && doc.paras[doc.cursor.p].style.list.is_some() && doc.paras[doc.cursor.p].text.is_empty() => {
                    doc.set_list(None);
                    self.reveal = true;
                }
                // Tab no começo de um item: aprofunda; Shift+Tab: volta (no nível 0, sai da lista).
                Key::Tab if !ctrl && doc.paras[doc.cursor.p].style.list.is_some() && (doc.cursor.o == 0 || doc.has_selection()) => {
                    if shift {
                        if doc.paras[doc.cursor.p].style.list.is_some_and(|l| l.level == 0) {
                            doc.set_list(None);
                        } else {
                            doc.shift_list(-1);
                        }
                    } else {
                        doc.shift_list(1);
                    }
                    self.reveal = true;
                }
                Key::Enter if !ctrl => {
                    doc.insert_text("\n");
                    self.want_x = None;
                    self.reveal = true;
                }
                Key::Tab if !ctrl => {
                    doc.insert_text("\t");
                    self.want_x = None;
                    self.reveal = true;
                }
                _ => match combo {
                    "Ctrl+A" => doc.select_all(),
                    "Ctrl+C" | "Ctrl+X" => {
                        if doc.has_selection() {
                            let text = doc.selected_text();
                            let frag = doc.selected_frag();
                            ui.copy(&text);
                            CLIP.with(|c| *c.borrow_mut() = Some((text, frag)));
                            if combo == "Ctrl+X" {
                                doc.delete_selection();
                                self.reveal = true;
                            }
                        }
                    }
                    "Ctrl+V" => {
                        ui.request_paste();
                        self.pasting = true;
                    }
                    "Ctrl+Z" => {
                        doc.undo();
                        self.reveal = true;
                    }
                    "Ctrl+Shift+Z" | "Ctrl+Y" => {
                        doc.redo();
                        self.reveal = true;
                    }
                    _ => {}
                },
            }
            if let Some(p) = moved {
                // Com seleção, Shift estende; sem Shift, a seta só desfaz a seleção.
                doc.set_cursor(p, shift);
                if !keep_x {
                    self.want_x = None;
                }
                self.reveal = true;
            }
        }
    }

    /// Desenha uma linha (texto, marca-texto, sublinhado, tachado, marcas de nota).
    fn draw_line(&self, c: &mut Canvas, p: &Para, ln: &crate::layout::Line, offx: f32, ly: f32, fb: &FontBook) {
        let z = self.zoom;
        let base = ly + ln.base * z;
        for s in &ln.segs {
            let st = &s.style;
            let x = offx + s.x * z;
            if let Some(img) = &st.image {
                let h = img.h * s.w / img.w.max(0.01);
                self.draw_image(c, img, x, base - h * z, s.w * z, h * z);
                continue;
            }
            if let Some(h) = st.highlight {
                // Da altura da própria letra (numa linha com letras maiores, não cobre tudo).
                let (asc, desc) = fb.metrics(st);
                c.fill_rect(x, base - asc * z, s.w * z, (asc + desc) * z, &solid(rgba(h[0], h[1], h[2], h[3])));
            }
            if s.tab {
                // Linha do sumário: pontilhado até o número da página.
                if p.style.role == crate::rich::Role::Toc {
                    let mut dx = x + 4.0 * z;
                    while dx < x + s.w * z - 6.0 * z {
                        c.fill_rect(dx, base - 1.0 * z, (1.2 * z).max(1.0), (1.2 * z).max(1.0), &solid(rgba(90, 90, 96, 255)));
                        dx += 4.0 * z;
                    }
                }
                continue;
            }
            // Marca de nota: o número dela, em sobrescrito.
            let label = note_label(st, &self.nums);
            let (st, text): (&CharStyle, &str) = match &label {
                Some((sup, t)) => (sup, t.as_str()),
                None => (st, &p.text[s.a..s.b]),
            };
            let size = FontBook::eff_size(st) * z;
            let shift = match st.script {
                1 => -st.size * 0.33 * z,
                -1 => st.size * 0.15 * z,
                _ => 0.0,
            };
            let color = rgba(st.color[0], st.color[1], st.color[2], st.color[3]);
            let face = fb.face(st);
            if s.w > s.nat + 0.01 && text.contains(' ') {
                // Justificado: cada palavra no seu lugar.
                face.draw(c, text.trim_end_matches(' '), x, base + shift, size, false, color);
            } else {
                face.draw(c, text, x, base + shift, size, false, color);
            }
            let th = (st.size / 14.0 * z).max(1.0);
            if st.underline {
                let w = if text.ends_with(' ') && s.b == ln.b { fb.measure(st, text.trim_end_matches(' ')) * z } else { s.w * z };
                c.fill_rect(x, base + shift + 1.5 * z, w, th, &solid(color));
            }
            if st.strike {
                c.fill_rect(x, base + shift - st.size * 0.3 * z, s.w * z, th, &solid(color));
            }
        }
    }

    /// Desenha a imagem em (x, y) com `w` × `h` pixels.
    fn draw_image(&self, c: &mut Canvas, img: &crate::rich::Image, x: f32, y: f32, w: f32, h: f32) {
        let (wp, hp) = (w.round().max(1.0) as u32, h.round().max(1.0) as u32);
        if wp > 8000 || hp > 8000 {
            return;
        }
        let mut pics = self.pics.borrow_mut();
        let pic = pics.entry(img.data.id).or_insert_with(|| Pic { full: glove_ui::image::decode_bytes(&img.data.bytes).ok().map(std::rc::Rc::new), scaled: None });
        let Some(full) = pic.full.clone() else {
            // Não abriu: um quadro cinza no lugar.
            c.fill_rect(x, y, w, h, &solid(rgba(220, 220, 224, 255)));
            c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
            return;
        };
        if !pic.scaled.as_ref().is_some_and(|(sw, sh, _)| *sw == wp && *sh == hp) {
            pic.scaled = scale_pixmap(&full, wp, hp).map(|p| (wp, hp, p));
        }
        if let Some((_, _, pm)) = &pic.scaled {
            let paint = tiny_skia::PixmapPaint::default();
            c.draw_pixmap_t(pm.as_ref(), &paint, tiny_skia::Transform::from_translate(x.round(), y.round()));
        }
    }

    /// Cabeçalho e rodapé de uma folha (topo da folha em `py`, pixels).
    fn draw_header_footer(&self, c: &mut Canvas, k: usize, px: f32, py: f32, fb: &FontBook) {
        if k == 0 && self.title_page {
            return;
        }
        let z = self.zoom;
        let st = CharStyle { size: 10.0, color: [80, 80, 86, 255], family: self.base_family.clone(), ..CharStyle::default() };
        let face = fb.face(&st);
        let (asc, _) = fb.metrics(&st);
        let x0 = px + self.page.margin_left * z;
        let w = self.width * z;
        let parts = [(&self.header, py + (self.page.margin_top / 2.0 + asc / 2.0) * z), (&self.footer, py + (self.page.height - self.page.margin_bottom / 2.0 + asc / 2.0) * z)];
        for (hf, base) in parts {
            if hf.is_empty() {
                continue;
            }
            let [l, m, r] = hf.fill(k + 1, self.pages, &self.title, &self.date);
            let tw = |t: &str| fb.measure(&st, t) * z;
            let color = rgba(80, 80, 86, 255);
            face.draw(c, &l, x0, base, 10.0 * z, false, color);
            face.draw(c, &m, x0 + (w - tw(&m)) / 2.0, base, 10.0 * z, false, color);
            face.draw(c, &r, x0 + w - tw(&r), base, 10.0 * z, false, color);
        }
    }

    /// Desenha o documento rolado de `scroll` pixels numa área de `view_w` × `view_h`.
    #[allow(clippy::too_many_arguments)]
    fn draw(&self, c: &mut Canvas, doc: &Doc, fb: &FontBook, (hscroll, scroll): (f32, f32), view_w: f32, view_h: f32, active: bool, show_caret: bool) {
        let z = self.zoom;
        let off = (self.col_x * z - hscroll, -scroll);
        let (sel_a, sel_b) = doc.selection();
        let has_sel = doc.has_selection();
        let sel_color = if active { rgba(181, 213, 255, 255) } else { rgba(212, 212, 216, 255) };
        if self.page_mode {
            // Mesa cinza e as folhas brancas com sombra.
            c.fill_rect(0.0, 0.0, view_w, view_h, &solid(rgba(150, 152, 158, 255)));
            let (pw, ph) = (self.page.width * z, self.page.height * z);
            let px = self.page_x * z - hscroll;
            for k in 0..self.pages {
                let py = self.page_top(k) * z - scroll;
                if py > view_h || py + ph < 0.0 {
                    continue;
                }
                c.fill_rect(px + 2.0, py + 3.0, pw, ph, &solid(rgba(0, 0, 0, 50)));
                c.fill_rect(px, py, pw, ph, &solid(rgba(255, 255, 255, 255)));
                c.stroke(rrect(px - 0.5, py - 0.5, pw + 1.0, ph + 1.0, 0.0), &solid(rgba(90, 90, 96, 120)), 1.0);
                self.draw_header_footer(c, k, px, py, fb);
                // Notas da folha, no pé, abaixo de um fio.
                if let Some(ids) = self.page_notes.get(k).filter(|v| !v.is_empty()) {
                    let total: f32 = NOTE_SEP + ids.iter().map(|id| self.note_lays.get(id).map_or(0.0, |l| l.3.h)).sum::<f32>();
                    let mut y = py + (self.page.height - self.page.margin_bottom - total) * z;
                    let x0 = px + self.page.margin_left * z;
                    c.fill_rect(x0, y + NOTE_SEP * 0.4 * z, self.width * z / 3.0, 1.0, &solid(rgba(60, 60, 66, 255)));
                    y += NOTE_SEP * z;
                    for id in ids {
                        if let Some((_, _, para, lay)) = self.note_lays.get(id) {
                            for ln in &lay.lines {
                                self.draw_line(c, para, ln, x0, y + ln.y * z, fb);
                            }
                            y += lay.h * z;
                        }
                    }
                }
            }
        } else {
            c.fill_rect(0.0, 0.0, view_w, view_h, &solid(rgba(255, 255, 255, 255)));
        }
        // Primeiro parágrafo visível.
        let first = self.lys.partition_point(|ys| ys.first().is_some_and(|&t| t * z - scroll < 0.0)).saturating_sub(1);
        for pi in first..doc.paras.len() {
            let ys = &self.lys[pi];
            if ys.first().is_some_and(|&t| t * z - scroll > view_h) {
                break;
            }
            let p = &doc.paras[pi];
            let l = self.lay(p);
            // Quebra de página no modo janela: uma linha tracejada antes do parágrafo.
            if !self.page_mode && p.style.page_break_before && pi > 0 {
                let y = (ys[0] - l.lines[0].y - 2.0) * z - scroll;
                let mut x = 0.0;
                while x < view_w {
                    c.fill_rect(x, y, 6.0, 1.0, &solid(rgba(140, 140, 150, 255)));
                    x += 10.0;
                }
            }
            let px = off.0 + self.para_x[pi] * z;
            // Marcador da lista, no recuo antes do texto (na primeira linha).
            if let Some(Some(label)) = self.labels.get(pi) {
                let ln = &l.lines[0];
                let st = &p.runs[0].style;
                let x = px + (p.style.left_indent - crate::rich::LIST_INDENT + 4.0) * z;
                let base = ys[0] * z - scroll + ln.base * z;
                let color = rgba(st.color[0], st.color[1], st.color[2], st.color[3]);
                fb.face(st).draw(c, label, x, base, FontBook::eff_size(st) * z, false, color);
            }
            for (li, ln) in l.lines.iter().enumerate() {
                let ly = ys[li] * z - scroll;
                if ly > view_h || ly + ln.h * z < 0.0 {
                    continue;
                }
                // Seleção: da posição inicial até a final; linha inteira no meio.
                if has_sel && Pos::new(pi, ln.a) <= sel_b && Pos::new(pi, ln.b) >= sel_a && !(sel_b == Pos::new(pi, ln.a) && ln.a != ln.b) {
                    let last_line = li + 1 == l.lines.len();
                    let x0 = if sel_a >= Pos::new(pi, ln.a) && sel_a.p == pi { x_of(p, l, fb, li, sel_a.o) } else { 0.0 };
                    let x1 = if sel_b.p == pi && sel_b.o <= ln.b && !(sel_b.o == ln.b && !last_line) {
                        x_of(p, l, fb, li, sel_b.o)
                    } else {
                        self.para_w[pi]
                    };
                    if x1 > x0 {
                        c.fill_rect(px + x0 * z, ly, (x1 - x0) * z, ln.h * z, &solid(sel_color));
                    }
                }
                self.draw_line(c, p, ln, px, ly, fb);
            }
        }
        // Bordas das tabelas (fios finos em volta de cada célula).
        for t in &self.tables {
            for (top, h, cells) in &t.rows {
                let (y0, y1) = (top * z - scroll, (top + h) * z - scroll);
                if y1 < 0.0 || y0 > view_h {
                    continue;
                }
                for (col, span) in cells {
                    let x0 = off.0 + t.bounds[*col] * z;
                    let x1 = off.0 + t.bounds[(col + span).min(t.bounds.len() - 1)] * z;
                    let ink = if t.borders { rgba(60, 60, 66, 255) } else { rgba(200, 205, 215, 255) };
                    // Sem bordas: um tracejado claro só na tela (não imprime).
                    c.stroke(rrect(x0.round() + 0.5, y0.round() + 0.5, (x1 - x0).round(), (y1 - y0).round(), 0.0), &solid(ink), 1.0);
                }
            }
        }
        // Modo janela: as notas no fim do texto.
        if let Some(&(_, y0)) = self.endnotes.first() {
            let y = (y0 - NOTE_SEP) * z - scroll;
            c.fill_rect(off.0, y, self.width * z / 3.0, 1.0, &solid(rgba(60, 60, 66, 255)));
            for (id, ny) in &self.endnotes {
                if let Some((_, _, para, lay)) = self.note_lays.get(id) {
                    for ln in &lay.lines {
                        self.draw_line(c, para, ln, off.0, ny * z - scroll + ln.y * z, fb);
                    }
                }
            }
        }
        // Imagem selecionada: moldura e o canto para puxar (ou o tamanho novo, puxando).
        if let Some((at, _)) = doc.selected_image() {
            if let Some((rx, ry, rw, rh)) = self.image_rect(doc, at) {
                let (rw, rh) = self.img_drag.filter(|d| d.0 == at).map_or((rw, rh), |d| (d.1, d.2));
                let (x, y) = ((rx * z - hscroll).round(), (ry * z - scroll).round());
                let (w, h) = ((rw * z).round(), (rh * z).round());
                let blue = rgba(56, 117, 215, 255);
                c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 0.0), &solid(blue), 1.0);
                c.fill_rect(x + w - 4.0, y + h - 4.0, 8.0, 8.0, &solid(rgba(255, 255, 255, 255)));
                c.stroke(rrect(x + w - 3.5, y + h - 3.5, 7.0, 7.0, 0.0), &solid(blue), 1.0);
            }
        }
        // Cursor
        if show_caret && !has_sel {
            let cpos = self.caret(doc, fb, doc.cursor);
            let p = &doc.paras[doc.cursor.p];
            let l = self.lay(p);
            let ln = &l.lines[line_of(l, doc.cursor.o)];
            let x = (cpos.x * z - hscroll).round();
            let y = cpos.y * z - scroll;
            let _ = view_w;
            c.fill_rect(x, y, 1.0, ln.text_h * z, &solid(rgba(0, 0, 0, if active { 255 } else { 120 })));
        }
    }
}

/// Estilo comum de um pedaço do documento (para os menus marcarem Negrito etc.).
pub fn common<T: PartialEq + Clone>(doc: &Doc, f: impl Fn(&crate::rich::CharStyle) -> T) -> Option<T> {
    if !doc.has_selection() {
        return Some(f(&doc.typing_style()));
    }
    let (a, b) = doc.selection();
    let mut out: Option<T> = None;
    for pi in a.p..=b.p {
        let p = &doc.paras[pi];
        let (s, e) = (if pi == a.p { a.o } else { 0 }, if pi == b.p { b.o } else { p.text.len() });
        for (x, y, st) in p.spans() {
            if y <= s || x >= e {
                if !(p.text.is_empty()) {
                    continue;
                }
            }
            let v = f(st);
            match &out {
                None => out = Some(v),
                Some(o) if *o != v => return None,
                _ => {}
            }
        }
    }
    out.or_else(|| Some(f(&doc.typing_style())))
}

/// Para a régua e os menus: o estilo do parágrafo do cursor.
pub fn para_style(doc: &Doc) -> &crate::rich::ParaStyle {
    &doc.paras[doc.cursor.p].style
}

/// A data de hoje (dd/mm/aaaa), para o campo {data}.
fn today() -> String {
    let (y, m, d, ..) = glove_ui::local_time();
    format!("{d:02}/{m:02}/{y}")
}
