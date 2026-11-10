//! Editor de texto do Glove.

mod abnt;
mod bar;
mod fontbook;
mod layout;
mod notes;
mod pages;
mod para;
mod rich;
mod rtf;
mod table;
mod view;

use glove_config::{tr, tr_dyn};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glove_ui::paint::draw::*;
use glove_ui::{run, AlertKind, App, Choice, ColorPanel, FileBrowser, FontPanel, Menu, MenuItem, Rect, Ui, WindowOptions};

use fontbook::FontBook;
use rich::{Align, CharStyle, Doc, ParaStyle};
use view::DocView;

/// Altura da régua e da barra de Localizar.
const RULER_H: f32 = 22.0;
const FIND_H: f32 = 34.0;

/// Barra de Localizar/Substituir.
struct Find {
    query: String,
    replace: String,
    case: bool,
    /// Põe o cursor no campo Localizar no próximo quadro.
    focus: bool,
    message: String,
}

/// Começos (bytes) de todas as ocorrências de `q` em `text`, sem ou com
/// diferença de maiúsculas.
pub fn find_all(text: &str, q: &str, case: bool) -> Vec<(usize, usize)> {
    if q.is_empty() {
        return Vec::new();
    }
    let qc: Vec<char> = q.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let mut end = i;
        let mut ok = true;
        let mut it = text[i..].char_indices();
        for want in &qc {
            match it.next() {
                Some((j, ch)) if (if case { ch == *want } else { ch.to_lowercase().eq(want.to_lowercase()) }) => end = i + j + ch.len_utf8(),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            out.push((i, end));
            i = end;
        } else {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Folha (diálogo que desce da barra de título).
enum Sheet {
    None,
    Open(FileBrowser),
    /// Inserir > Imagem.
    Image(FileBrowser),
    SaveAs { fb: FileBrowser, then_close: bool },
    /// Erro ao abrir ou salvar (alerta com OK).
    Error(String),
    /// Imprimir (Ctrl+P).
    Print(glove_ui::print::PrintSheet),
    AskSave,
    /// Formatar > Texto > Espaçamento.
    Spacing(para::SpacingSheet),
    /// Arquivo > Configurar Página.
    Page(pages::PageSheet),
    /// Inserir > Nota de Rodapé (ou editar uma).
    Note(notes::NoteSheet),
    /// Formatar > Cabeçalho e Rodapé.
    HeaderFooter(notes::HfSheet),
    /// Inserir > Tabela.
    Table(notes::TableSheet),
    /// Arquivo > Exportar como PDF / Word / OpenDocument.
    Export { fb: FileBrowser, kind: &'static str },
}

/// O que o painel de Cores muda: a cor do texto ou o marca-texto.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ColorTarget {
    Text,
    Highlight,
}

struct Editor {
    doc: Doc,
    view: DocView,
    fb: Rc<FontBook>,
    path: Option<PathBuf>,
    sheet: Sheet,
    last_title: String,
    ruler: bool,
    find: Option<Find>,
    fonts: FontPanel,
    colors: ColorPanel,
    color_target: ColorTarget,
    /// Seleção da última vez que os painéis foram acertados com ela.
    panel_sync: Option<(rich::Pos, rich::Pos, u64)>,
    /// Onde a barra de formatação termina.
    bar_end: f32,
    /// Palavras do documento (versão em que foram contadas, quantas).
    words: (u64, usize),
    /// Marcador da régua sendo arrastado.
    ruler_drag: Option<para::RulerDrag>,
    /// Texto simples (.txt): sem barra de formatação nem régua; grava sem estilos.
    plain: bool,
    /// Sumário a inserir (false) ou atualizar (true) quando a paginação estiver pronta.
    pending_toc: Option<bool>,
    /// Onde o documento começa (abaixo da faixa de abas).
    tab_top: f32,
    /// Arquivo aberto pelo Abrir…: vai para uma aba nova.
    open_in_tab: Option<PathBuf>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn documents_dir() -> PathBuf {
    let h = home();
    ["Documentos", "Documents"].iter().map(|d| h.join(d)).find(|p| p.is_dir()).unwrap_or(h)
}

/// Grava de forma segura: arquivo temporário ao lado + renomear.
fn save_file(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("{}.glove-tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl Editor {
    fn new(path: Option<PathBuf>, fb: Rc<FontBook>) -> Self {
        let mut ed = Self { doc: Doc::new(), view: DocView::new(), fb, path: None, sheet: Sheet::None, last_title: String::new(), ruler: true, find: None, fonts: FontPanel::new(rich::DEFAULT_FAMILY, rich::DEFAULT_SIZE), colors: ColorPanel::new((0, 0, 0)), color_target: ColorTarget::Text, panel_sync: None, bar_end: 0.0, words: (u64::MAX, 0), ruler_drag: None, plain: false, pending_toc: None, tab_top: 0.0, open_in_tab: None };
        if let Some(p) = path {
            match load(&p) {
                Ok((doc, plain)) => {
                    ed.doc = doc;
                    ed.plain = plain;
                    ed.path = Some(p);
                }

                // Arquivo novo: começa vazio e salva nesse caminho.
                Err(_) if !p.exists() => {
                    ed.plain = is_plain_path(&p);
                    ed.path = Some(p);
                }
                Err(e) => ed.sheet = Sheet::Error(tr!("Não consegui abrir “{}”: {}", p.display(), e)),
            }
        }
        // Documento rico em folhas; texto simples ajustado à janela.
        ed.view.page_mode = !ed.plain;
        ed
    }

    fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| tr!("Sem título").into())
    }

    fn save(&mut self, then_close: bool, ui: &mut Ui) {
        match self.path.clone() {
            Some(p) => match save_file(&p, &self.contents(&p)) {
                Ok(()) => {
                    self.doc.modified = false;
                    if then_close {
                        ui.close = true;
                    }
                }
                Err(e) => self.sheet = Sheet::Error(tr!("Não consegui salvar “{}”: {}", p.display(), e)),
            },
            None => {
                let name = if self.plain { tr!("Sem título.txt") } else { tr!("Sem título.rtf") };
                self.sheet = Sheet::SaveAs { fb: FileBrowser::save(Some(documents_dir()), name), then_close };
            }
        }
    }

    /// O que vai para o arquivo: texto simples (.txt ou modo simples) ou RTF.
    fn contents(&self, p: &Path) -> String {
        if self.plain || is_plain_path(p) { self.doc.plain_text() } else { rtf::write(&self.doc) }
    }

    fn open(&mut self, p: PathBuf) -> Result<(), String> {
        let (doc, plain) = load(&p)?;
        // Documento atual vazio e intocado: abre aqui; senão, numa janela nova.
        if self.doc.is_empty() && !self.doc.modified && self.path.is_none() {
            self.doc = doc;
            self.plain = plain;
            self.view = DocView::new();
            self.view.page_mode = !plain;
            self.path = Some(p);
        } else {
            self.open_in_tab = Some(p);
        }
        Ok(())
    }

    /// Tamanho da fonte no cursor (ou da seleção, se for um só).
    fn font_size(&self) -> f32 {
        view::common(&self.doc, |s| s.size.to_bits()).map(f32::from_bits).unwrap_or(rich::DEFAULT_SIZE)
    }

    fn set_align(&mut self, a: Align) {
        self.doc.set_para_style(&|s| s.align = a);
    }

    /// Liga ou desliga um atributo na seleção (todo ligado → desliga; senão, liga).
    fn toggle(&mut self, get: fn(&CharStyle) -> bool, set: fn(&mut CharStyle, bool)) {
        let on = view::common(&self.doc, get) == Some(true);
        self.doc.set_char_style(&move |s| set(s, !on));
    }

    /// Abre o painel de Cores para o texto ou o marca-texto (o mesmo de novo fecha).
    pub(crate) fn open_colors(&mut self, t: ColorTarget) {
        if self.colors.open && self.color_target == t {
            self.colors.open = false;
            return;
        }
        self.color_target = t;
        self.colors.open = true;
        self.panel_sync = None;
    }

    /// Acerta os painéis de Fontes e Cores com a seleção (quando ela muda).
    fn sync_panels(&mut self) {
        let key = (self.doc.cursor, self.doc.anchor, self.doc.version);
        if self.panel_sync == Some(key) {
            return;
        }
        self.panel_sync = Some(key);
        if let Some(f) = view::common(&self.doc, |s| s.family.clone()) {
            self.fonts.family = f;
        }
        self.fonts.size = self.font_size();
        let c = match self.color_target {
            ColorTarget::Text => view::common(&self.doc, |s| s.color).unwrap_or([0, 0, 0, 255]),
            ColorTarget::Highlight => view::common(&self.doc, |s| s.highlight).flatten().unwrap_or(bar::HIGHLIGHTS[0].1),
        };
        self.colors.set((c[0], c[1], c[2]));
    }

    /// Põe as imagens no cursor, cabendo na coluna e na folha.
    fn insert_images(&mut self, paths: &[PathBuf]) -> Result<(), String> {
        let mut imgs = Vec::new();
        for p in paths {
            let bytes = std::fs::read(p).map_err(|e| tr!("Não consegui abrir “{}”: {}", p.display(), e))?;
            imgs.push(rich::Image::from_bytes(bytes).ok_or_else(|| tr!("“{}” não é uma imagem PNG, JPEG ou WebP.", p.display()))?);
        }
        let max_w = self.view.width_at(self.doc.cursor.p);
        let pg = &self.doc.page;
        let max_h = pg.height - pg.margin_top - pg.margin_bottom - 24.0;
        self.doc.group(|d| {
            for img in imgs {
                let (w, h) = img.fit(max_w, max_h);
                d.insert_image(rich::Image { w: w.round().max(1.0), h: h.round().max(1.0), ..img });
            }
        });
        self.view.reveal = true;
        Ok(())
    }

    fn run_action(&mut self, action: Option<&str>, ui: &mut Ui) {
        match action {
            Some("new") => spawn_self(None),
            Some("open") => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(documents_dir);
                self.sheet = Sheet::Open(FileBrowser::open(Some(dir), &[]));
            }
            Some("save") => self.save(false, ui),
            Some("close") => {
                if self.doc.modified {
                    self.sheet = Sheet::AskSave;
                } else {
                    ui.close = true;
                }
            }
            Some("minimize") => ui.minimize(),
            Some("find") => {
                let q = if self.doc.has_selection() && !self.doc.selected_text().contains('\n') { self.doc.selected_text() } else { self.find.as_ref().map(|f| f.query.clone()).unwrap_or_default() };
                let (replace, case) = self.find.as_ref().map(|f| (f.replace.clone(), f.case)).unwrap_or_default();
                self.find = Some(Find { query: q, replace, case, focus: true, message: String::new() });
            }
            Some("find-next") => self.find_step(true),
            Some("find-prev") => self.find_step(false),
            Some("use-selection") => {
                if self.doc.has_selection() {
                    let q = self.doc.selected_text();
                    match &mut self.find {
                        Some(f) => f.query = q,
                        None => self.find = Some(Find { query: q, replace: String::new(), case: false, focus: false, message: String::new() }),
                    }
                }
            }
            Some("ruler") => self.ruler = !self.ruler,
            Some("fonts") => {
                self.fonts.size = self.font_size();
                self.fonts.open = !self.fonts.open;
            }
            Some("align-left") => self.set_align(Align::Left),
            Some("align-center") => self.set_align(Align::Center),
            Some("align-right") => self.set_align(Align::Right),
            Some("align-justify") => self.set_align(Align::Justify),
            Some("bigger") => self.doc.set_char_style(&|s| s.size = (s.size + 1.0).min(288.0)),
            Some("smaller") => self.doc.set_char_style(&|s| s.size = (s.size - 1.0).max(4.0)),
            Some("bold") => self.toggle(|s| s.bold, |s, v| s.bold = v),
            Some("italic") => self.toggle(|s| s.italic, |s, v| s.italic = v),
            Some("underline") => self.toggle(|s| s.underline, |s, v| s.underline = v),
            Some("strike") => self.toggle(|s| s.strike, |s, v| s.strike = v),
            Some("baseline") => self.doc.set_char_style(&|s| s.script = 0),
            Some("superscript") => self.doc.set_char_style(&|s| s.script = 1),
            Some("subscript") => self.doc.set_char_style(&|s| s.script = -1),
            Some("colors") => self.open_colors(ColorTarget::Text),
            Some("to-plain") => {
                // Tira os estilos (um passo do Desfazer) e passa a gravar texto simples.
                let text = self.doc.plain_text();
                let mut d = plain_doc(&text);
                d.modified = true;
                d.page = self.doc.page.clone();
                self.doc = d;
                self.view = DocView::new();
                self.plain = true;
                self.view.page_mode = false;
                if let Some(p) = &self.path {
                    if !is_plain_path(p) {
                        self.path = Some(p.with_extension("txt"));
                    }
                }
            }
            Some("to-rich") => {
                self.plain = false;
                self.view.page_mode = true;
                self.doc.modified = true;
                if let Some(p) = &self.path {
                    if is_plain_path(p) {
                        self.path = Some(p.with_extension("rtf"));
                    }
                }
            }
            Some("print") => {
                // Documento rico: o PDF com as folhas, cabeçalho, rodapé e notas.
                let content = if self.plain {
                    glove_ui::print::PrintContent::Text(self.doc.plain_text())
                } else {
                    let pdf = view::export_pdf(&self.doc, &self.fb, &self.name());
                    let path = std::env::temp_dir().join(format!("glove-imprimir-{}.pdf", std::process::id()));
                    match std::fs::write(&path, pdf) {
                        Ok(()) => glove_ui::print::PrintContent::Pdf(path),
                        Err(e) => {
                            self.sheet = Sheet::Error(tr!("Não consegui preparar a impressão: {}", e));
                            return;
                        }
                    }
                };
                self.sheet = Sheet::Print(glove_ui::print::PrintSheet::new(&self.name(), content));
            }
            Some("export-pdf") => {
                let ext = "pdf";
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).or_else(|| Some(documents_dir()));
                let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| tr!("Sem título").into());
                let kind: &'static str = "pdf";
                self.sheet = Sheet::Export { fb: FileBrowser::save(dir, &format!("{stem}.{ext}")), kind };
            }
            Some("image") => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(documents_dir);
                self.sheet = Sheet::Image(FileBrowser::open(Some(dir), &glove_ui::image::EXTENSIONS).labeled(tr!("Inserir Imagem"), tr!("Inserir")));
            }
            Some("table") => self.sheet = Sheet::Table(notes::TableSheet { rows: "3".into(), cols: "3".into() }),
            Some("row-above") => table::add_row(&mut self.doc, false),
            Some("row-below") => table::add_row(&mut self.doc, true),
            Some("col-left") => table::add_col(&mut self.doc, false),
            Some("col-right") => table::add_col(&mut self.doc, true),
            Some("row-del") => table::del_row(&mut self.doc),
            Some("col-del") => table::del_col(&mut self.doc),
            Some("cells-merge") => table::merge(&mut self.doc),
            Some("cells-split") => table::split(&mut self.doc),
            Some("table-borders") => table::toggle_borders(&mut self.doc),
            Some("table-equal") => table::equal_widths(&mut self.doc),
            Some("list-none") => self.doc.set_list(None),
            Some("list-bullet") => self.doc.set_list(Some(false)),
            Some("list-number") => self.doc.set_list(Some(true)),
            Some(a) if a.starts_with("role-") => {
                if let Some(r) = a[5..].parse::<usize>().ok().and_then(|i| rich::Role::MENU.get(i)) {
                    self.doc.apply_role(*r);
                }
            }
            Some("toc") | Some("toc-update") => {
                // O sumário precisa das páginas: liga o modo página e monta no próximo quadro.
                self.view.page_mode = true;
                self.pending_toc = Some(action == Some("toc-update"));
                ui.redraw = true;
            }
            Some("abnt") => abnt::apply(&mut self.doc),
            Some("new-abnt") => spawn_abnt(),
            Some("footnote") => self.sheet = Sheet::Note(notes::NoteSheet { buf: glove_ui::TextBuffer::new(""), id: None }),
            Some("header-footer") => self.sheet = Sheet::HeaderFooter(notes::HfSheet::from(&self.doc.header, &self.doc.footer, self.doc.title_page)),
            Some("page-number") => {
                if self.doc.footer.center.trim().is_empty() {
                    self.doc.footer.center = "{página}".into();
                    self.doc.modified = true;
                    self.doc.version = self.doc.version.wrapping_add(1);
                }
                self.view.page_mode = true;
            }
            Some("page-setup") => self.sheet = Sheet::Page(pages::PageSheet::from(&self.doc.page)),
            Some("page-mode") => {
                self.view.page_mode = !self.view.page_mode;
                self.view.scroll = 0.0;
                self.view.reveal = true;
            }
            Some("zoom-fit") => self.view.fit_width = true,
            Some("zoom-in") | Some("zoom-out") => {
                let z = self.view.zoom;
                let next = if action == Some("zoom-in") {
                    view::ZOOM_STEPS.iter().copied().find(|v| *v > z + 0.005).unwrap_or(view::ZOOM_MAX)
                } else {
                    view::ZOOM_STEPS.iter().rev().copied().find(|v| *v < z - 0.005).unwrap_or(view::ZOOM_MIN)
                };
                self.view.fit_width = false;
                self.view.zoom = next;
                self.view.reveal = true;
            }
            Some(a) if a.starts_with("zoom-") => {
                if let Ok(v) = a[5..].parse::<f32>() {
                    self.view.fit_width = false;
                    self.view.zoom = v / 100.0;
                    self.view.reveal = true;
                }
            }
            Some("page-break") => {
                // Parágrafo novo começando numa folha nova (um passo do Desfazer).
                self.doc.group(|d| {
                    d.insert_text("\n");
                    let p = d.cursor.p;
                    let mut st = d.paras[p].style.clone();
                    st.page_break_before = true;
                    d.set_para_styles(p, p, vec![st]);
                });
                self.view.reveal = true;
            }
            Some("spacing") => self.sheet = Sheet::Spacing(para::SpacingSheet::from(view::para_style(&self.doc))),
            Some(a) if a.starts_with("line-") => {
                if let Some(v) = a[5..].parse::<f32>().ok() {
                    self.doc.set_para_style(&move |s| s.line_spacing = v);
                }
            }
            Some("hl-none") => self.doc.set_char_style(&|s| s.highlight = None),
            Some(a) if a.starts_with("hl-") => {
                if let Some(&(_, c)) = a[3..].parse::<usize>().ok().and_then(|i| bar::HIGHLIGHTS.get(i)) {
                    self.doc.set_char_style(&move |s| s.highlight = Some(c));
                }
            }
            _ => {}
        }
    }

    /// Vai para a próxima (ou anterior) ocorrência e a seleciona.
    fn find_step(&mut self, forward: bool) {
        let Some(f) = &mut self.find else { return };
        let text = self.doc.plain_text();
        let hits = find_all(&text, &f.query, f.case);
        if hits.is_empty() {
            f.message = if f.query.is_empty() { String::new() } else { tr!("Não encontrado").into() };
            return;
        }
        let (a, b) = self.doc.selection();
        let (a, b) = (self.doc.offset_of(a), self.doc.offset_of(b));
        let pick = if forward {
            // A próxima depois da seleção atual (ou do cursor); no fim, volta ao começo.
            hits.iter().find(|h| h.0 >= b && (h.0, h.1) != (a, b)).or(hits.first())
        } else {
            hits.iter().rev().find(|h| h.1 <= a).or(hits.last())
        };
        if let Some(&(s, e)) = pick {
            let (ps, pe) = (self.doc.pos_of_offset(s), self.doc.pos_of_offset(e));
            self.doc.set_cursor(ps, false);
            self.doc.set_cursor(pe, true);
            self.view.reveal = true;
            let n = hits.iter().position(|h| h.0 == s).map(|i| i + 1).unwrap_or(0);
            f.message = tr!("{} de {}", n, hits.len());
        }
    }

    /// Troca a seleção (se for a ocorrência) e vai para a próxima.
    fn replace_one(&mut self) {
        let Some(f) = &self.find else { return };
        let sel = self.doc.selected_text();
        let same = if f.case { sel == f.query } else { sel.to_lowercase() == f.query.to_lowercase() };
        if same && !f.query.is_empty() {
            let r = f.replace.clone();
            self.doc.insert_text(&r);
            self.doc.break_undo();
        }
        self.find_step(true);
    }

    /// Troca todas as ocorrências de uma vez (um passo só no Desfazer).
    fn replace_all(&mut self) {
        let Some(f) = &mut self.find else { return };
        let text = self.doc.plain_text();
        let hits = find_all(&text, &f.query, f.case);
        if hits.is_empty() {
            f.message = tr!("Não encontrado").into();
            return;
        }
        f.message = tr!("{} substituídas", hits.len());
        let replace = f.replace.clone();
        // De trás para a frente: as posições das outras não mudam. Cada troca fica no
        // estilo do texto que estava ali; tudo num passo só do Desfazer.
        self.doc.group(|d| {
            for &(a, b) in hits.iter().rev() {
                let (pa, pb) = (d.pos_of_offset(a), d.pos_of_offset(b));
                d.set_cursor(pa, false);
                d.set_cursor(pb, true);
                d.insert_text(&replace);
                d.break_undo();
            }
        });
        self.doc.set_cursor(rich::Pos::default(), false);
    }

    /// Barra de Localizar/Substituir. Devolve verdadeiro se usou o Enter (o
    /// documento não pode receber a mesma tecla).
    fn draw_find(&mut self, ui: &mut Ui, y: f32) -> bool {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, y, w, FIND_H, &vgrad(y, y + FIND_H, &[(0.0, rgba(236, 236, 240, 255)), (1.0, rgba(214, 214, 220, 255))]));
        ui.c.hline(0.0, y + FIND_H - 1.0, w, rgba(150, 150, 156, 255));
        let Some(f) = &mut self.find else { return false };
        let fw = ((w - 420.0) / 2.0).clamp(90.0, 200.0);
        let row = y + 7.0;
        let mut x = 8.0;
        let label = |ui: &mut Ui, t: &str, x: f32| {
            ui.fonts.draw(ui.c, t, x, row + 14.0, 12.0, false, rgba(30, 30, 34, 255));
            ui.text_width(t, 12.0, false)
        };
        x += label(ui, tr!("Localizar:"), x) + 6.0;
        let focus = std::mem::take(&mut f.focus);
        let (changed, enter) = ui.text_field_in(Rect::new(x, row, fw, 20.0), &mut f.query, focus);
        // "2 de 3" dentro do campo, à direita (como na busca do Safari da era).
        if !f.message.is_empty() {
            let mw = ui.text_width(&f.message, 10.0, false);
            ui.fonts.draw(ui.c, &f.message, x + fw - mw - 5.0, row + 14.0, 10.0, false, rgba(130, 130, 136, 255));
        }
        x += fw + 10.0;
        x += label(ui, tr!("Substituir:"), x) + 6.0;
        let (_, enter_r) = ui.text_field_in(Rect::new(x, row, fw, 20.0), &mut f.replace, false);
        x += fw + 8.0;
        let msg = f.message.clone();
        let mut act = None;
        if changed {
            f.message.clear();
        }
        ui.area(Rect::new(x, row - 2.0, w - x - 4.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                ui.spacing(4.0);
                if ui.small_button("◀") {
                    act = Some("prev");
                }
                if ui.small_button("▶") {
                    act = Some("next");
                }
                if ui.small_button(tr!("Substituir")) {
                    act = Some("one");
                }
                if ui.small_button(tr!("Tudo")) {
                    act = Some("all");
                }
                let _ = &msg;
            });
        });
        if enter {
            act = Some("next");
        }
        if enter_r {
            act = Some("one");
        }
        // O Enter tira o foco do campo: devolve no próximo quadro.
        if enter {
            f.focus = true;
        }
        if ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            self.find = None;
            return true;
        }
        match act {
            Some("next") => self.find_step(true),
            Some("prev") => self.find_step(false),
            Some("one") => self.replace_one(),
            Some("all") => self.replace_all(),
            _ => {}
        }
        enter || enter_r
    }

    /// Folha centralizada no topo, com o fundo da janela escurecido.
    fn sheet(&mut self, ui: &mut Ui) {
        if matches!(self.sheet, Sheet::None) {
            return;
        }
        if matches!(self.sheet, Sheet::AskSave) {
            let title = tr!("Deseja salvar as alterações feitas no documento “{}”?", self.name());
            match ui.alert(AlertKind::Caution, &title, tr!("Se você não salvar, as alterações serão perdidas."), &[tr!("Não salvar"), tr!("Cancelar"), tr!("Salvar")]) {
                Some(0) => ui.close = true,
                Some(1) => {
                    self.sheet = Sheet::None;
                    ui.redraw = true;
                }
                Some(_) => {
                    self.sheet = Sheet::None;
                    self.save(true, ui);
                    ui.redraw = true;
                }
                None => {}
            }
            return;
        }
        if let Sheet::Error(msg) = &self.sheet {
            let msg = msg.clone();
            if ui.alert(AlertKind::Stop, &msg, "", &["OK"]).is_some() {
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        if let Sheet::Note(f) = &mut self.sheet {
            if let Some(r) = notes::note_sheet(ui, f) {
                if let Some(text) = r {
                    match f.id {
                        Some(id) => self.doc.set_note_text(id, &text),
                        None => {
                            self.doc.insert_note(&text);
                            self.view.reveal = true;
                        }
                    }
                }
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        if let Sheet::Table(f) = &mut self.sheet {
            if let Some(r) = notes::table_sheet(ui, f) {
                if let Some((rows, cols)) = r {
                    table::insert(&mut self.doc, rows, cols);
                    self.view.reveal = true;
                }
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        if let Sheet::HeaderFooter(f) = &mut self.sheet {
            if let Some(ok) = notes::hf_sheet(ui, f) {
                if ok {
                    let (h, ft) = f.parts();
                    if h != self.doc.header || ft != self.doc.footer || f.title_page != self.doc.title_page {
                        self.doc.header = h;
                        self.doc.footer = ft;
                        self.doc.title_page = f.title_page;
                        self.doc.modified = true;
                        self.doc.version = self.doc.version.wrapping_add(1);
                    }
                }
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        if let Sheet::Page(f) = &mut self.sheet {
            match pages::page_sheet(ui, f) {
                Some(ok) => {
                    if ok {
                        let pg = f.apply(&self.doc.page);
                        if pg != self.doc.page {
                            self.doc.page = pg;
                            self.doc.modified = true;
                            self.doc.version = self.doc.version.wrapping_add(1);
                        }
                    }
                    self.sheet = Sheet::None;
                    ui.redraw = true;
                }
                None => {}
            }
            return;
        }
        if let Sheet::Spacing(f) = &mut self.sheet {
            match Editor::spacing_sheet(ui, f) {
                Some(true) => {
                    let n = |s: &str| para::parse_num(s);
                    let (line, before, after) = (n(&f.line), n(&f.before), n(&f.after));
                    let (first, left, right) = (n(&f.first), n(&f.left), n(&f.right));
                    self.doc.set_para_style(&|s| {
                        if let Some(v) = line {
                            s.line_spacing = v.clamp(0.5, 5.0);
                        }
                        if let Some(v) = before {
                            s.space_before = v.clamp(0.0, 500.0);
                        }
                        if let Some(v) = after {
                            s.space_after = v.clamp(0.0, 500.0);
                        }
                        if let Some(v) = first {
                            s.first_indent = v * para::CM;
                        }
                        if let Some(v) = left {
                            s.left_indent = (v * para::CM).max(0.0);
                        }
                        if let Some(v) = right {
                            s.right_indent = (v * para::CM).max(0.0);
                        }
                    });
                    self.sheet = Sheet::None;
                    ui.redraw = true;
                }
                Some(false) => {
                    self.sheet = Sheet::None;
                    ui.redraw = true;
                }
                None => {}
            }
            return;
        }
        // Imprimir: a folha do kit (impressora, cópias, PDF).
        if let Sheet::Print(p) = &mut self.sheet {
            if ui.print_sheet(p) {
                self.sheet = Sheet::None;
                ui.redraw = true;
            }
            return;
        }
        // Abrir e Salvar como: o diálogo em colunas do kit.
        let choice = match &mut self.sheet {
            Sheet::Open(fb) | Sheet::Image(fb) | Sheet::SaveAs { fb, .. } | Sheet::Export { fb, .. } => ui.file_sheet(fb),
            _ => None,
        };
        let Some(choice) = choice else { return };
        let sheet = std::mem::replace(&mut self.sheet, Sheet::None);
        ui.redraw = true;
        match (sheet, choice) {
            (_, Choice::Cancelled) => {}
            (Sheet::Open(_), Choice::Picked(p)) => {
                if let Err(e) = self.open(p) {
                    self.sheet = Sheet::Error(e);
                }
            }
            (Sheet::Image(_), Choice::Picked(p)) => {
                if let Err(e) = self.insert_images(&[p]) {
                    self.sheet = Sheet::Error(e);
                }
            }
            (Sheet::SaveAs { then_close, .. }, Choice::Picked(p)) => {
                self.path = Some(p);
                self.save(then_close, ui);
            }
            (Sheet::Export { kind: "pdf", .. }, Choice::Picked(p)) => {
                let pdf = view::export_pdf(&self.doc, &self.fb, &self.name());
                if let Err(e) = save_bytes(&p, &pdf) {
                    self.sheet = Sheet::Error(tr!("Não consegui exportar “{}”: {}", p.display(), e));
                }
            }
            _ => {}
        }
    }
}

/// Arquivo que é texto simples (não RTF): .txt, código, configurações…
fn is_plain_path(p: &Path) -> bool {
    !matches!(ext_of(p).as_deref(), Some("rtf"))
}

fn ext_of(p: &Path) -> Option<String> {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

/// Grava bytes de forma segura (temporário + renomear).
fn save_bytes(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("glove-tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

/// Lê um arquivo: RTF vira documento rico; o resto, texto simples. Devolve se é simples.
fn load(p: &Path) -> Result<(Doc, bool), String> {
    // Word e OpenDocument são pacotes (zip): o Editor trabalha com RTF e texto simples.
    if matches!(ext_of(p).as_deref(), Some("docx" | "odt" | "doc" | "pages")) {
        return Err(tr!("O Editor abre documentos RTF e texto simples. Salve o arquivo como RTF no outro programa.").to_string());
    }
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    if !is_plain_path(p) || bytes.starts_with(b"{\\rtf") {
        return rtf::read_doc(&bytes).map(|d| (d, false));
    }
    Ok((plain_doc(&String::from_utf8_lossy(&bytes)), true))
}

/// Documento de texto simples (arquivo .txt), na fonte padrão.
fn plain_doc(text: &str) -> Doc {
    Doc::from_plain(text, Rc::new(CharStyle::default()), ParaStyle::default())
}

/// Abre outra janela com um documento novo no modelo ABNT.
fn spawn_abnt() {
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-edit"))).unwrap_or_else(|| "glove-edit".into());
    let _ = std::process::Command::new(exe).arg("--abnt").spawn();
}

fn spawn_self(path: Option<&Path>) {
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-edit"))).unwrap_or_else(|| "glove-edit".into());
    let mut cmd = std::process::Command::new(exe);
    if let Some(p) = path {
        cmd.arg(p);
    }
    let _ = cmd.spawn();
}

impl Editor {
    fn ui(&mut self, ui: &mut Ui) {
        let sheet_open = !matches!(self.sheet, Sheet::None);
        // A área dos painéis flutuantes é deles: o que se desenha antes não pega o clique ali.
        if !sheet_open {
            let (w, h) = (ui.c.w, ui.c.h);
            for r in [self.fonts.rect(w, h), self.colors.rect(w, h)].into_iter().flatten() {
                ui.reserve(r);
            }
        }

        // Atalhos (e itens de menu, que chegam como os mesmos atalhos)
        if !sheet_open {
            let mut action = None;
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+N" => action = Some("new"),
                    "Ctrl+O" => action = Some("open"),
                    "Ctrl+S" => action = Some("save"),
                    "Ctrl+W" => action = Some("close"),
                    "Ctrl+P" => action = Some("print"),
                    "Ctrl+M" => action = Some("minimize"),
                    "Ctrl+equal" | "Ctrl+plus" => action = Some("bigger"),
                    "Ctrl+minus" => action = Some("smaller"),
                    "Ctrl+F" => action = Some("find"),
                    "Ctrl+G" => action = Some("find-next"),
                    "Ctrl+Shift+G" => action = Some("find-prev"),
                    "Ctrl+E" => action = Some("use-selection"),
                    "Ctrl+R" => action = Some("ruler"),
                    "Ctrl+T" => action = Some("fonts"),
                    "Ctrl+Shift+P" => action = Some("page-setup"),
                    "Ctrl+Alt+N" => action = Some("footnote"),
                    "Ctrl+Shift+W" => action = Some("page-mode"),
                    "Ctrl+0" | "Ctrl+KP_0" => action = Some("zoom-100"),
                    "Ctrl+Shift+greater" | "Ctrl+Shift+period" | "Ctrl+greater" => action = Some("zoom-in"),
                    "Ctrl+Shift+less" | "Ctrl+Shift+comma" | "Ctrl+less" => action = Some("zoom-out"),
                    "Ctrl+Return" | "Ctrl+KP_Enter" => action = Some("page-break"),
                    "Ctrl+B" => action = Some("bold"),
                    "Ctrl+I" => action = Some("italic"),
                    "Ctrl+U" => action = Some("underline"),
                    "Ctrl+Shift+C" => action = Some("colors"),
                    "Ctrl+Shift+L" => action = Some("align-left"),
                    "Ctrl+Shift+E" => action = Some("align-center"),
                    "Ctrl+Shift+R" => action = Some("align-right"),
                    "Ctrl+Shift+J" => action = Some("align-justify"),
                    "Ctrl+Shift+S" => {
                        let dir = self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).or_else(|| Some(documents_dir()));
                        self.sheet = Sheet::SaveAs { fb: FileBrowser::save(dir, &self.name()), then_close: false };
                    }
                    _ => {}
                }
            }
            let mut menu_action = None;
            for a in ui.menu_actions() {
                match a.as_str() {
                    "delete" => self.doc.delete_selection(),
                    "font-reset" => self.doc.set_char_style(&|s| s.size = rich::DEFAULT_SIZE),
                    "align-left" => menu_action = Some("align-left"),
                    "align-center" => menu_action = Some("align-center"),
                    "align-right" => menu_action = Some("align-right"),
                    "align-justify" => menu_action = Some("align-justify"),
                    "spacing" => menu_action = Some("spacing"),
                    "zoom-fit" => menu_action = Some("zoom-fit"),
                    "list-none" => menu_action = Some("list-none"),
                    "table" => menu_action = Some("table"),
                    "image" => menu_action = Some("image"),
                    "export-pdf" => menu_action = Some("export-pdf"),
                    "row-above" => menu_action = Some("row-above"),
                    "row-below" => menu_action = Some("row-below"),
                    "col-left" => menu_action = Some("col-left"),
                    "col-right" => menu_action = Some("col-right"),
                    "row-del" => menu_action = Some("row-del"),
                    "col-del" => menu_action = Some("col-del"),
                    "cells-merge" => menu_action = Some("cells-merge"),
                    "cells-split" => menu_action = Some("cells-split"),
                    "table-borders" => menu_action = Some("table-borders"),
                    "table-equal" => menu_action = Some("table-equal"),
                    "list-bullet" => menu_action = Some("list-bullet"),
                    "list-number" => menu_action = Some("list-number"),
                    "toc" => menu_action = Some("toc"),
                    "toc-update" => menu_action = Some("toc-update"),
                    "abnt" => menu_action = Some("abnt"),
                    "new-abnt" => menu_action = Some("new-abnt"),
                    "role-0" => menu_action = Some("role-0"),
                    "role-1" => menu_action = Some("role-1"),
                    "role-2" => menu_action = Some("role-2"),
                    "role-3" => menu_action = Some("role-3"),
                    "role-4" => menu_action = Some("role-4"),
                    "role-5" => menu_action = Some("role-5"),
                    "header-footer" => menu_action = Some("header-footer"),
                    "page-number" => menu_action = Some("page-number"),
                    "zoom-50" => menu_action = Some("zoom-50"),
                    "zoom-in" => menu_action = Some("zoom-in"),
                    "zoom-out" => menu_action = Some("zoom-out"),
                    "zoom-75" => menu_action = Some("zoom-75"),
                    "zoom-100" => menu_action = Some("zoom-100"),
                    "zoom-125" => menu_action = Some("zoom-125"),
                    "zoom-150" => menu_action = Some("zoom-150"),
                    "zoom-200" => menu_action = Some("zoom-200"),
                    "to-plain" => menu_action = Some("to-plain"),
                    "to-rich" => menu_action = Some("to-rich"),
                    "strike" | "baseline" | "superscript" | "subscript" | "hl-none" | "hl-0" | "hl-1" | "hl-2" | "hl-3" | "hl-4" => {
                        let a: &'static str = match a.as_str() {
                            "strike" => "strike",
                            "baseline" => "baseline",
                            "superscript" => "superscript",
                            "subscript" => "subscript",
                            "hl-none" => "hl-none",
                            "hl-0" => "hl-0",
                            "hl-1" => "hl-1",
                            "hl-2" => "hl-2",
                            "hl-3" => "hl-3",
                            _ => "hl-4",
                        };
                        menu_action = Some(a);
                    }
                    "replace-all" => self.replace_all(),
                    _ => {}
                }
            }
            if menu_action.is_some() {
                self.run_action(menu_action, ui);
            }
            self.run_action(action, ui);
        }

        // Como no Tiger: o ponto no botão de fechar diz que há alterações; o ícone do arquivo vai no título.
        ui.set_edited(self.doc.modified);
        ui.set_document(self.path.as_deref());
        let title = self.name();
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }

        // Régua e barra de Localizar no alto; o resto é o documento.
        ui.set_enabled(!sheet_open);
        let (w, h) = (ui.c.w, ui.c.h);
        let mut top = self.tab_top;
        if self.ruler && !self.plain {
            self.draw_format_bar(ui, top);
            top += bar::BAR_H;
            self.draw_ruler(ui, top, RULER_H);
            top += RULER_H;
        }
        let mut used_key = false;
        if self.find.is_some() {
            used_key = self.draw_find(ui, top);
            if self.find.is_some() {
                top += FIND_H;
            }
        }
        if used_key {
            ui.set_enabled(false);
        }
        let enabled = !sheet_open && !used_key;
        self.view.title = self.name();
        // Imagens arrastadas para dentro do documento: entram onde caíram.
        if enabled && !self.plain {
            if let Some((paths, pt)) = ui.take_drop() {
                let imgs: Vec<PathBuf> = paths.into_iter().filter(|p| glove_ui::image::format_of(p).is_some()).collect();
                if !imgs.is_empty() {
                    let pos = self.view.pos_at_screen(&self.doc, &self.fb, pt);
                    self.doc.set_cursor(pos, false);
                    if let Err(e) = self.insert_images(&imgs) {
                        self.sheet = Sheet::Error(e);
                    }
                }
            }
        }
        self.view.ui(ui, &mut self.doc, &self.fb, Rect::new(0.0, top, w, h - top - bar::STATUS_H), enabled);
        self.draw_status_bar(ui, h - bar::STATUS_H);
        // Sumário pedido: agora a paginação está pronta.
        if let Some(update) = self.pending_toc.take() {
            abnt::toc(&mut self.doc, &self.view, update);
            self.view.reveal = true;
        }
        // Clique duplo numa marca de nota: abre a nota para editar.
        if let Some(id) = self.view.edit_note.take() {
            let text = self.doc.note_text(id).unwrap_or("").to_string();
            self.sheet = Sheet::Note(notes::NoteSheet { buf: glove_ui::TextBuffer::new(text), id: Some(id) });
        }
        ui.set_enabled(!sheet_open);
        // Painéis de Fontes e Cores: mostram o da seleção e mudam só o que se mexeu.
        if self.fonts.open || self.colors.open {
            self.sync_panels();
        }
        // Por cima de tudo: o clique dentro dos painéis não atravessa.
        ui.unreserved(|ui| self.panels(ui));
        self.sheet(ui);
    }

    /// O que aparece acima do documento (barra de formatação, régua, abas): se não
    /// mudou durante o quadro, o quadro já saiu certo e não precisa da segunda passada.
    /// None = não dá para garantir (Localizar aberto, folha aberta).
    fn chrome_sig(&self) -> Option<String> {
        if self.find.is_some() || !matches!(self.sheet, Sheet::None) {
            return None;
        }
        let d = &self.doc;
        let st = (
            view::common(d, |s| s.family.clone()),
            view::common(d, |s| s.size.to_bits()),
            view::common(d, |s| (s.bold, s.italic, s.underline, s.strike)),
            view::common(d, |s| s.color),
            view::common(d, |s| s.highlight),
        );
        Some(format!("{st:?}|{:?}|{}{}{}{}|{}|{}", view::para_style(d), self.ruler, self.plain, self.fonts.open, self.colors.open, d.modified, self.name()))
    }

    /// Painéis de Fontes e Cores (aplicam na seleção na hora).
    fn panels(&mut self, ui: &mut Ui) {
        let (fam0, size0) = (self.fonts.family.clone(), self.fonts.size);
        if ui.font_panel(&mut self.fonts) {
            let (family, size) = (self.fonts.family.clone(), self.fonts.size);
            if family != fam0 && !family.is_empty() {
                self.doc.set_char_style(&|s| s.family = family.clone());
            }
            if (size - size0).abs() > 0.01 {
                self.doc.set_char_style(&|s| s.size = size);
            }
            self.panel_sync = Some((self.doc.cursor, self.doc.anchor, self.doc.version));
        }
        if ui.color_panel(&mut self.colors) {
            let (r, g, b) = self.colors.rgb;
            let c = [r, g, b, 255];
            match self.color_target {
                ColorTarget::Text => self.doc.set_char_style(&|s| s.color = c),
                ColorTarget::Highlight => self.doc.set_char_style(&|s| s.highlight = Some(c)),
            }
            self.panel_sync = Some((self.doc.cursor, self.doc.anchor, self.doc.version));
        }
    }

    fn menus(&self, tabs: usize) -> Vec<Menu> {
        let align = view::para_style(&self.doc).align;
        let role = view::para_style(&self.doc).role;
        let list = view::para_style(&self.doc).list;
        let in_table = table::in_table(&self.doc);
        let on = |f: fn(&CharStyle) -> bool| view::common(&self.doc, f) == Some(true);
        let script = view::common(&self.doc, |s| s.script);
        let hl = view::common(&self.doc, |s| s.highlight);
        let sel = self.doc.has_selection();
        vec![
            Menu::new(
                tr!("Arquivo"),
                vec![
                    MenuItem::new("new", tr!("Novo"), "Ctrl+N"),
                    MenuItem::new("new-tab", tr!("Nova aba"), "Ctrl+Shift+T"),
                    MenuItem::new("new-abnt", tr!("Novo com Modelo ABNT"), ""),
                    MenuItem::new("open", tr!("Abrir…"), "Ctrl+O"),
                    MenuItem::Separator,
                    MenuItem::new("next-tab", tr!("Próxima aba"), "Ctrl+Page_Down").enabled(tabs > 1),
                    MenuItem::new("prev-tab", tr!("Aba anterior"), "Ctrl+Page_Up").enabled(tabs > 1),
                    MenuItem::Separator,
                    MenuItem::new("close", if tabs > 1 { tr!("Fechar aba") } else { tr!("Fechar") }, "Ctrl+W"),
                    MenuItem::new("save", tr!("Salvar"), "Ctrl+S"),
                    MenuItem::new("save-as", tr!("Salvar como…"), "Ctrl+Shift+S"),
                    MenuItem::Separator,
                    MenuItem::new("page-setup", tr!("Configurar Página…"), "Ctrl+Shift+P").enabled(!self.plain),
                    MenuItem::Separator,
                    MenuItem::new("export-pdf", tr!("Exportar como PDF…"), "").enabled(!self.plain),
                    MenuItem::new("print", tr!("Imprimir…"), "Ctrl+P"),
                ],
            ),
            Menu::new(
                tr!("Editar"),
                vec![
                    MenuItem::new("undo", tr!("Desfazer"), "Ctrl+Z").enabled(self.doc.can_undo()),
                    MenuItem::new("redo", tr!("Refazer"), "Ctrl+Shift+Z").enabled(self.doc.can_redo()),
                    MenuItem::Separator,
                    MenuItem::new("cut", tr!("Recortar"), "Ctrl+X").enabled(sel),
                    MenuItem::new("copy", tr!("Copiar"), "Ctrl+C").enabled(sel),
                    MenuItem::new("paste", tr!("Colar"), "Ctrl+V"),
                    MenuItem::new("delete", tr!("Apagar"), "").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("select-all", tr!("Selecionar tudo"), "Ctrl+A"),
                    MenuItem::Separator,
                    MenuItem::submenu(
                        tr!("Localizar"),
                        vec![
                            MenuItem::new("find", tr!("Localizar…"), "Ctrl+F"),
                            MenuItem::new("find-next", tr!("Localizar Próximo"), "Ctrl+G"),
                            MenuItem::new("find-prev", tr!("Localizar Anterior"), "Ctrl+Shift+G"),
                            MenuItem::new("use-selection", tr!("Usar Seleção para Localizar"), "Ctrl+E").enabled(sel),
                            MenuItem::Separator,
                            MenuItem::new("replace-all", tr!("Substituir Tudo"), "").enabled(self.find.is_some()),
                        ],
                    ),
                ],
            ),
            Menu::new(
                tr!("Formatar"),
                vec![
                    MenuItem::submenu(
                        tr!("Fonte"),
                        vec![
                            MenuItem::new("fonts", if self.fonts.open { tr!("Ocultar Fontes") } else { tr!("Mostrar Fontes") }, "Ctrl+T"),
                            MenuItem::Separator,
                            MenuItem::new("bold", tr!("Negrito"), "Ctrl+B").checked(on(|s| s.bold)),
                            MenuItem::new("italic", tr!("Itálico"), "Ctrl+I").checked(on(|s| s.italic)),
                            MenuItem::new("underline", tr!("Sublinhado"), "Ctrl+U").checked(on(|s| s.underline)),
                            MenuItem::new("strike", tr!("Tachado"), "").checked(on(|s| s.strike)),
                            MenuItem::Separator,
                            MenuItem::new("bigger", tr!("Maior"), "Ctrl+equal"),
                            MenuItem::new("smaller", tr!("Menor"), "Ctrl+minus"),
                            MenuItem::new("font-reset", tr!("Tamanho Normal"), ""),
                            MenuItem::Separator,
                            MenuItem::submenu(
                                tr!("Linha de Base"),
                                vec![
                                    MenuItem::new("baseline", tr!("Normal"), "").checked(script == Some(0)),
                                    MenuItem::new("superscript", tr!("Sobrescrito"), "").checked(script == Some(1)),
                                    MenuItem::new("subscript", tr!("Subscrito"), "").checked(script == Some(-1)),
                                ],
                            ),
                            MenuItem::Separator,
                            MenuItem::new("colors", if self.colors.open { tr!("Ocultar Cores") } else { tr!("Mostrar Cores") }, "Ctrl+Shift+C"),
                            MenuItem::submenu(tr!("Marca-texto"), {
                                let mut v: Vec<MenuItem> = bar::HIGHLIGHTS.iter().enumerate().map(|(i, (n, c))| MenuItem::new(&format!("hl-{i}"), &tr_dyn(n), "").checked(hl == Some(Some(*c)))).collect();
                                v.push(MenuItem::Separator);
                                v.push(MenuItem::new("hl-none", tr!("Sem Marca-texto"), "").checked(hl == Some(None)));
                                v
                            }),
                        ],
                    ),
                    MenuItem::submenu(
                        tr!("Texto"),
                        vec![
                            MenuItem::new("align-left", tr!("Alinhar à Esquerda"), "Ctrl+Shift+L").checked(align == Align::Left),
                            MenuItem::new("align-center", tr!("Centralizar"), "Ctrl+Shift+E").checked(align == Align::Center),
                            MenuItem::new("align-justify", tr!("Justificar"), "Ctrl+Shift+J").checked(align == Align::Justify),
                            MenuItem::new("align-right", tr!("Alinhar à Direita"), "Ctrl+Shift+R").checked(align == Align::Right),
                            MenuItem::Separator,
                            MenuItem::new("spacing", tr!("Espaçamento…"), ""),
                        ],
                    ),
                    MenuItem::submenu(tr!("Estilo"), rich::Role::MENU.iter().enumerate().map(|(i, r)| MenuItem::new(&format!("role-{i}"), &tr_dyn(r.name()), "").checked(role == *r)).collect()),
                    MenuItem::submenu(
                        tr!("Lista"),
                        vec![
                            MenuItem::new("list-none", tr!("Sem Lista"), "").checked(list.is_none()),
                            MenuItem::new("list-bullet", tr!("Marcadores"), "").checked(list.is_some_and(|l| !l.numbered)),
                            MenuItem::new("list-number", tr!("Numerada"), "").checked(list.is_some_and(|l| l.numbered)),
                        ],
                    ),
                    MenuItem::submenu(
                        tr!("Tabela"),
                        vec![
                            MenuItem::new("row-above", tr!("Inserir Linha Acima"), "").enabled(in_table),
                            MenuItem::new("row-below", tr!("Inserir Linha Abaixo"), "").enabled(in_table),
                            MenuItem::new("col-left", tr!("Inserir Coluna à Esquerda"), "").enabled(in_table),
                            MenuItem::new("col-right", tr!("Inserir Coluna à Direita"), "").enabled(in_table),
                            MenuItem::Separator,
                            MenuItem::new("row-del", tr!("Apagar Linha"), "").enabled(in_table),
                            MenuItem::new("col-del", tr!("Apagar Coluna"), "").enabled(in_table),
                            MenuItem::Separator,
                            MenuItem::new("cells-merge", tr!("Juntar Células"), "").enabled(in_table && sel),
                            MenuItem::new("cells-split", tr!("Separar Células"), "").enabled(in_table && self.doc.paras[self.doc.cursor.p].style.cell.is_some_and(|c| c.span > 1)),
                            MenuItem::Separator,
                            MenuItem::new("table-borders", tr!("Bordas"), "").checked(in_table && table::borders(&self.doc)).enabled(in_table),
                            MenuItem::new("table-equal", tr!("Colunas da Mesma Largura"), "").enabled(in_table),
                        ],
                    ),
                    MenuItem::new("header-footer", tr!("Cabeçalho e Rodapé…"), "").enabled(!self.plain),
                    MenuItem::new("abnt", tr!("Aplicar Formatação ABNT"), "").enabled(!self.plain),
                    MenuItem::Separator,
                    if self.plain { MenuItem::new("to-rich", tr!("Converter em Texto Rico"), "") } else { MenuItem::new("to-plain", tr!("Converter em Texto Simples"), "") },
                    MenuItem::new("ruler", if self.ruler { tr!("Ocultar Régua") } else { tr!("Mostrar Régua") }, "Ctrl+R").enabled(!self.plain),
                ],
            ),
            Menu::new(
                tr!("Inserir"),
                vec![
                    MenuItem::new("page-break", tr!("Quebra de Página"), "Ctrl+Return").enabled(!self.plain),
                    MenuItem::Separator,
                    MenuItem::new("footnote", tr!("Nota de Rodapé…"), "Ctrl+Alt+N").enabled(!self.plain),
                    MenuItem::new("page-number", tr!("Número de Página"), "").enabled(!self.plain),
                    MenuItem::new("table", tr!("Tabela…"), "").enabled(!self.plain),
                    MenuItem::new("image", tr!("Imagem…"), "").enabled(!self.plain),
                    MenuItem::Separator,
                    MenuItem::new("toc", tr!("Sumário"), "").enabled(!self.plain),
                    MenuItem::new("toc-update", tr!("Atualizar Sumário"), "").enabled(!self.plain && self.doc.paras.iter().any(|p| p.style.role == rich::Role::Toc)),
                ],
            ),
            Menu::new(
                tr!("Visualizar"),
                vec![
                    MenuItem::new("page-mode", tr!("Modo Página"), "Ctrl+Shift+W").checked(self.view.page_mode),
                    MenuItem::submenu(
                        tr!("Zoom"),
                        vec![
                            MenuItem::new("zoom-in", tr!("Aumentar Zoom"), "Ctrl+Shift+period"),
                            MenuItem::new("zoom-out", tr!("Diminuir Zoom"), "Ctrl+Shift+comma"),
                            MenuItem::new("zoom-100", tr!("Tamanho Real"), "Ctrl+0"),
                            MenuItem::Separator,
                            MenuItem::new("zoom-50", "50%", "").checked(!self.view.fit_width && (self.view.zoom - 0.5).abs() < 0.01),
                            MenuItem::new("zoom-75", "75%", "").checked(!self.view.fit_width && (self.view.zoom - 0.75).abs() < 0.01),
                            MenuItem::new("zoom-100", "100%", "").checked(!self.view.fit_width && (self.view.zoom - 1.0).abs() < 0.01),
                            MenuItem::new("zoom-125", "125%", "").checked(!self.view.fit_width && (self.view.zoom - 1.25).abs() < 0.01),
                            MenuItem::new("zoom-150", "150%", "").checked(!self.view.fit_width && (self.view.zoom - 1.5).abs() < 0.01),
                            MenuItem::new("zoom-200", "200%", "").checked(!self.view.fit_width && (self.view.zoom - 2.0).abs() < 0.01),
                            MenuItem::Separator,
                            MenuItem::new("zoom-fit", tr!("Ajustar à Largura"), "").checked(self.view.fit_width).enabled(self.view.page_mode),
                        ],
                    ),
                ],
            ),
        ]
    }

}

/// A janela do Editor: um ou mais documentos em abas.
struct Tabs {
    docs: Vec<Editor>,
    cur: usize,
    fb: Rc<FontBook>,
}

impl Tabs {
    fn close_tab(&mut self, i: usize) {
        self.docs.remove(i);
        if self.cur >= self.docs.len() || self.cur > i {
            self.cur = self.cur.saturating_sub(1).min(self.docs.len() - 1);
        }
    }
}

impl App for Tabs {
    fn ui(&mut self, ui: &mut Ui) {
        let n = self.docs.len();
        let sheet_open = !matches!(self.docs[self.cur].sheet, Sheet::None);
        if !sheet_open {
            for k in ui.keys().to_vec() {
                match k.combo.as_str() {
                    "Ctrl+Shift+T" => {
                        self.docs.push(Editor::new(None, self.fb.clone()));
                        self.cur = self.docs.len() - 1;
                    }
                    "Ctrl+Page_Down" | "Ctrl+Next" => self.cur = (self.cur + 1) % n,
                    "Ctrl+Page_Up" | "Ctrl+Prior" => self.cur = (self.cur + n - 1) % n,
                    _ => {}
                }
            }
        }
        self.cur = self.cur.min(self.docs.len() - 1);
        let w = ui.c.w;
        let top = if self.docs.len() > 1 { glove_ui::TAB_STRIP_H } else { 0.0 };
        if top > 0.0 {
            let titles: Vec<String> = self.docs.iter().map(|d| if d.doc.modified { format!("{} •", d.name()) } else { d.name() }).collect();
            ui.set_enabled(!sheet_open);
            match ui.tab_strip(Rect::new(0.0, 0.0, w, top), &titles, self.cur) {
                Some(glove_ui::TabAction::Select(i)) => self.cur = i,
                Some(glove_ui::TabAction::Close(i)) => {
                    // Com alterações: pergunta na aba dela antes de fechar.
                    if self.docs[i].doc.modified {
                        self.cur = i;
                        self.docs[i].sheet = Sheet::AskSave;
                    } else {
                        self.close_tab(i);
                    }
                }
                Some(glove_ui::TabAction::New) => {
                    self.docs.push(Editor::new(None, self.fb.clone()));
                    self.cur = self.docs.len() - 1;
                }
                None => {}
            }
            ui.set_enabled(true);
        }
        let cur = self.cur;
        self.docs[cur].tab_top = top;
        let before = (self.docs.len(), self.docs[cur].chrome_sig());
        self.docs[cur].ui(ui);
        // Digitar só muda o documento (desenhado depois da barra e da régua): se o que
        // fica acima dele não mudou, o quadro já está certo (sem a segunda passada).
        if self.cur == cur && before.1.is_some() && self.docs.get(cur).is_some_and(|d| (self.docs.len(), d.chrome_sig()) == before) {
            ui.settled();
        }
        // "Fechar" num documento com outras abas: fecha só a aba.
        if ui.close && self.docs.len() > 1 {
            ui.close = false;
            self.close_tab(cur);
            ui.redraw = true;
        }
        if let Some(p) = self.docs.get_mut(cur).and_then(|d| d.open_in_tab.take()) {
            self.docs.push(Editor::new(Some(p), self.fb.clone()));
            self.cur = self.docs.len() - 1;
            ui.redraw = true;
        }
    }

    fn menus(&self) -> Vec<Menu> {
        self.docs[self.cur.min(self.docs.len() - 1)].menus(self.docs.len())
    }

    fn close_requested(&mut self) -> bool {
        // A primeira aba com alterações pergunta; as outras esperam a próxima vez.
        match self.docs.iter().position(|d| d.doc.modified) {
            Some(i) => {
                self.cur = i;
                self.docs[i].sheet = Sheet::AskSave;
                false
            }
            None => true,
        }
    }
}

pub fn main() {
    let arg = std::env::args_os().nth(1);
    let abnt_new = arg.as_deref() == Some(std::ffi::OsStr::new("--abnt"));
    let path = arg.filter(|_| !abnt_new).map(PathBuf::from);
    let fb = Rc::new(FontBook::new());
    let mut ed = Editor::new(path, fb.clone());
    if abnt_new {
        ed.doc = abnt::new_doc();
        ed.view.page_mode = true;
    }
    let title = ed.name();
    let opts = WindowOptions { title, app_id: "glove.TextEditor".into(), size: (760, 620), fixed: false, min_size: None };
    if let Err(e) = run(opts, Tabs { docs: vec![ed], cur: 0, fb }) {
        eprintln!("glove-edit: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::find_all;

    #[test]
    fn localizar() {
        assert_eq!(find_all("Ação ação AÇÃO", "ação", false), [(0, 6), (7, 13), (14, 20)]);
        assert_eq!(find_all("Ação ação AÇÃO", "ação", true), [(7, 13)]);
        assert_eq!(find_all("aaaa", "aa", false), [(0, 2), (2, 4)]);
        assert!(find_all("abc", "", false).is_empty());
    }
}
