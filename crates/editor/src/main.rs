//! Editor de texto do Glove.

mod bar;
mod fontbook;
mod layout;
mod para;
mod rich;
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
    SaveAs { fb: FileBrowser, then_close: bool },
    /// Erro ao abrir ou salvar (alerta com OK).
    Error(String),
    /// Imprimir (Ctrl+P).
    Print(glove_ui::print::PrintSheet),
    AskSave,
    /// Formatar > Texto > Espaçamento.
    Spacing(para::SpacingSheet),
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
    /// Marcador da régua sendo arrastado.
    ruler_drag: Option<para::RulerDrag>,
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
        let mut ed = Self { doc: Doc::new(), view: DocView::new(), fb, path: None, sheet: Sheet::None, last_title: String::new(), ruler: true, find: None, fonts: FontPanel::new(rich::DEFAULT_FAMILY, rich::DEFAULT_SIZE), colors: ColorPanel::new((0, 0, 0)), color_target: ColorTarget::Text, panel_sync: None, bar_end: 0.0, ruler_drag: None, tab_top: 0.0, open_in_tab: None };
        if let Some(p) = path {
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    ed.doc = plain_doc(&t);
                    ed.path = Some(p);
                }
                // Arquivo novo: começa vazio e salva nesse caminho.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ed.path = Some(p),
                Err(e) => ed.sheet = Sheet::Error(tr!("Não consegui abrir “{}”: {}", p.display(), e)),
            }
        }
        ed
    }

    fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| tr!("Sem título").into())
    }

    fn save(&mut self, then_close: bool, ui: &mut Ui) {
        match self.path.clone() {
            Some(p) => match save_file(&p, &self.doc.plain_text()) {
                Ok(()) => {
                    self.doc.modified = false;
                    if then_close {
                        ui.close = true;
                    }
                }
                Err(e) => self.sheet = Sheet::Error(tr!("Não consegui salvar “{}”: {}", p.display(), e)),
            },
            None => {
                self.sheet = Sheet::SaveAs { fb: FileBrowser::save(Some(documents_dir()), tr!("Sem título.txt")), then_close };
            }
        }
    }

    fn open(&mut self, p: PathBuf) -> Result<(), String> {
        let t = std::fs::read_to_string(&p).map_err(|e| format!("Não consegui abrir: {e}"))?;
        // Documento atual vazio e intocado: abre aqui; senão, numa janela nova.
        if self.doc.is_empty() && !self.doc.modified && self.path.is_none() {
            self.doc = plain_doc(&t);
            self.view = DocView::new();
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
            Sheet::Open(fb) | Sheet::SaveAs { fb, .. } => ui.file_sheet(fb),
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
            (Sheet::SaveAs { then_close, .. }, Choice::Picked(p)) => {
                self.path = Some(p);
                self.save(then_close, ui);
            }
            _ => {}
        }
    }
}

/// Documento de texto simples (arquivo .txt), na fonte padrão.
fn plain_doc(text: &str) -> Doc {
    Doc::from_plain(text, Rc::new(CharStyle::default()), ParaStyle::default())
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

        // Atalhos (e itens de menu, que chegam como os mesmos atalhos)
        if !sheet_open {
            let mut action = None;
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+N" => action = Some("new"),
                    "Ctrl+O" => action = Some("open"),
                    "Ctrl+S" => action = Some("save"),
                    "Ctrl+W" => action = Some("close"),
                    "Ctrl+P" => {
                        let content = glove_ui::print::PrintContent::Text(self.doc.plain_text());
                        self.sheet = Sheet::Print(glove_ui::print::PrintSheet::new(&self.name(), content));
                    }
                    "Ctrl+M" => action = Some("minimize"),
                    "Ctrl+equal" | "Ctrl+plus" => action = Some("bigger"),
                    "Ctrl+minus" => action = Some("smaller"),
                    "Ctrl+F" => action = Some("find"),
                    "Ctrl+G" => action = Some("find-next"),
                    "Ctrl+Shift+G" => action = Some("find-prev"),
                    "Ctrl+E" => action = Some("use-selection"),
                    "Ctrl+R" => action = Some("ruler"),
                    "Ctrl+T" => action = Some("fonts"),
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
        if self.ruler {
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
        self.view.ui(ui, &mut self.doc, &self.fb, Rect::new(0.0, top, w, h - top), enabled);
        ui.set_enabled(!sheet_open);
        // Painéis de Fontes e Cores: mostram o da seleção e mudam só o que se mexeu.
        if self.fonts.open || self.colors.open {
            self.sync_panels();
        }
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
        self.sheet(ui);
    }

    fn menus(&self, tabs: usize) -> Vec<Menu> {
        let align = view::para_style(&self.doc).align;
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
                    MenuItem::new("open", tr!("Abrir…"), "Ctrl+O"),
                    MenuItem::Separator,
                    MenuItem::new("next-tab", tr!("Próxima aba"), "Ctrl+Page_Down").enabled(tabs > 1),
                    MenuItem::new("prev-tab", tr!("Aba anterior"), "Ctrl+Page_Up").enabled(tabs > 1),
                    MenuItem::Separator,
                    MenuItem::new("close", if tabs > 1 { tr!("Fechar aba") } else { tr!("Fechar") }, "Ctrl+W"),
                    MenuItem::new("save", tr!("Salvar"), "Ctrl+S"),
                    MenuItem::new("save-as", tr!("Salvar como…"), "Ctrl+Shift+S"),
                    MenuItem::Separator,
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
                    MenuItem::Separator,
                    MenuItem::new("ruler", if self.ruler { tr!("Ocultar Régua") } else { tr!("Mostrar Régua") }, "Ctrl+R"),
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
        self.docs[cur].ui(ui);
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
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let fb = Rc::new(FontBook::new());
    let ed = Editor::new(path, fb.clone());
    let title = ed.name();
    let opts = WindowOptions { title, app_id: "glove.TextEditor".into(), size: (640, 480), fixed: false, min_size: None };
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
