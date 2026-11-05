//! Editor de texto do Glove.

use std::path::{Path, PathBuf};

use glove_kit::paint::draw::*;
use glove_kit::{run, Align, AlertKind, App, Choice, FileBrowser, Glyph, Menu, MenuItem, Rect, TextBuffer, Ui, WindowOptions, SEGMENT_H, SEGMENT_W};

/// Altura da régua e da barra de Localizar.
const RULER_H: f32 = 30.0;
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
    AskSave,
}

struct Editor {
    buf: TextBuffer,
    path: Option<PathBuf>,
    sheet: Sheet,
    last_title: String,
    ruler: bool,
    find: Option<Find>,
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
    fn new(path: Option<PathBuf>) -> Self {
        let mut ed = Self { buf: TextBuffer::new(""), path: None, sheet: Sheet::None, last_title: String::new(), ruler: true, find: None };
        if let Some(p) = path {
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    ed.buf = TextBuffer::new(t);
                    ed.path = Some(p);
                }
                // Arquivo novo: começa vazio e salva nesse caminho.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ed.path = Some(p),
                Err(e) => ed.sheet = Sheet::Error(format!("Não consegui abrir “{}”: {e}", p.display())),
            }
        }
        ed
    }

    fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Sem título".into())
    }

    fn save(&mut self, then_close: bool, ui: &mut Ui) {
        match self.path.clone() {
            Some(p) => match save_file(&p, self.buf.text()) {
                Ok(()) => {
                    self.buf.modified = false;
                    if then_close {
                        ui.close = true;
                    }
                }
                Err(e) => self.sheet = Sheet::Error(format!("Não consegui salvar “{}”: {e}", p.display())),
            },
            None => {
                self.sheet = Sheet::SaveAs { fb: FileBrowser::save(Some(documents_dir()), "Sem título.txt"), then_close };
            }
        }
    }

    fn open(&mut self, p: PathBuf) -> Result<(), String> {
        let t = std::fs::read_to_string(&p).map_err(|e| format!("Não consegui abrir: {e}"))?;
        // Documento atual vazio e intocado: abre aqui; senão, numa janela nova.
        if self.buf.is_empty() && !self.buf.modified && self.path.is_none() {
            self.buf = TextBuffer::new(t);
            self.path = Some(p);
        } else {
            spawn_self(Some(&p));
        }
        Ok(())
    }

    fn font_px(&self) -> f32 {
        if self.buf.font_px > 0.0 { self.buf.font_px } else { 13.0 }
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
                if self.buf.modified {
                    self.sheet = Sheet::AskSave;
                } else {
                    ui.close = true;
                }
            }
            Some("minimize") => ui.minimize(),
            Some("find") => {
                let q = if self.buf.has_selection() && !self.buf.selected_text().contains('\n') { self.buf.selected_text().to_string() } else { self.find.as_ref().map(|f| f.query.clone()).unwrap_or_default() };
                let (replace, case) = self.find.as_ref().map(|f| (f.replace.clone(), f.case)).unwrap_or_default();
                self.find = Some(Find { query: q, replace, case, focus: true, message: String::new() });
            }
            Some("find-next") => self.find_step(true),
            Some("find-prev") => self.find_step(false),
            Some("use-selection") => {
                if self.buf.has_selection() {
                    let q = self.buf.selected_text().to_string();
                    match &mut self.find {
                        Some(f) => f.query = q,
                        None => self.find = Some(Find { query: q, replace: String::new(), case: false, focus: false, message: String::new() }),
                    }
                }
            }
            Some("ruler") => self.ruler = !self.ruler,
            Some("align-left") => self.buf.align = Align::Left,
            Some("align-center") => self.buf.align = Align::Center,
            Some("align-right") => self.buf.align = Align::Right,
            Some("bigger") => self.buf.font_px = (self.font_px() + 2.0).min(48.0),
            Some("smaller") => self.buf.font_px = (self.font_px() - 2.0).max(9.0),
            _ => {}
        }
    }

    /// Vai para a próxima (ou anterior) ocorrência e a seleciona.
    fn find_step(&mut self, forward: bool) {
        let Some(f) = &mut self.find else { return };
        let hits = find_all(self.buf.text(), &f.query, f.case);
        if hits.is_empty() {
            f.message = if f.query.is_empty() { String::new() } else { "Não encontrado".into() };
            return;
        }
        let (a, b) = self.buf.selection();
        let pick = if forward {
            // A próxima depois da seleção atual (ou do cursor); no fim, volta ao começo.
            hits.iter().find(|h| h.0 >= b && (h.0, h.1) != (a, b)).or(hits.first())
        } else {
            hits.iter().rev().find(|h| h.1 <= a).or(hits.last())
        };
        if let Some(&(s, e)) = pick {
            self.buf.set_cursor(s, false);
            self.buf.set_cursor(e, true);
            self.buf.reveal = true;
            let n = hits.iter().position(|h| h.0 == s).map(|i| i + 1).unwrap_or(0);
            f.message = format!("{n} de {}", hits.len());
        }
    }

    /// Troca a seleção (se for a ocorrência) e vai para a próxima.
    fn replace_one(&mut self) {
        let Some(f) = &self.find else { return };
        let sel = self.buf.selected_text();
        let same = if f.case { sel == f.query } else { sel.to_lowercase() == f.query.to_lowercase() };
        if same && !f.query.is_empty() {
            let r = f.replace.clone();
            self.buf.insert(&r);
        }
        self.find_step(true);
    }

    /// Troca todas as ocorrências de uma vez (um passo só no Desfazer).
    fn replace_all(&mut self) {
        let Some(f) = &mut self.find else { return };
        let hits = find_all(self.buf.text(), &f.query, f.case);
        if hits.is_empty() {
            f.message = "Não encontrado".into();
            return;
        }
        let mut out = String::with_capacity(self.buf.len());
        let mut last = 0;
        for (a, b) in &hits {
            out.push_str(&self.buf.text()[last..*a]);
            out.push_str(&f.replace);
            last = *b;
        }
        out.push_str(&self.buf.text()[last..]);
        f.message = format!("{} substituídas", hits.len());
        self.buf.select_all();
        self.buf.insert(&out);
        self.buf.set_cursor(0, false);
    }

    /// Régua: centímetros e os botões de alinhamento (como no TextEdit da era).
    fn draw_ruler(&mut self, ui: &mut Ui, y: f32) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, y, w, RULER_H, &vgrad(y, y + RULER_H, &[(0.0, rgba(244, 244, 246, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(0.0, y + RULER_H - 1.0, w, rgba(150, 150, 156, 255));
        // Alinhamento (segmentado com os glifos de linhas).
        let cur = match self.buf.align {
            Align::Left => 0,
            Align::Center => 1,
            Align::Right => 2,
        };
        let r = Rect::new(6.0, y + (RULER_H - SEGMENT_H) / 2.0, 3.0 * SEGMENT_W, SEGMENT_H);
        if let Some(i) = ui.segmented_in(r, &[Glyph::AlignLeft, Glyph::AlignCenter, Glyph::AlignRight], Some(cur), &[true, true, true]) {
            self.buf.align = [Align::Left, Align::Center, Align::Right][i];
        }
        // Centímetros (1 cm ≈ 38 pontos), com o meio centímetro mais curto.
        let x0 = 3.0 * SEGMENT_W + 20.0;
        let cm = 37.8;
        let ink = rgba(90, 90, 96, 255);
        let mut k = 0;
        while x0 + k as f32 * cm / 2.0 < w - 8.0 {
            let x = x0 + k as f32 * cm / 2.0;
            let tall = k % 2 == 0;
            ui.c.vline(x.round() + 0.5, y + RULER_H - if tall { 9.0 } else { 5.0 }, if tall { 8.0 } else { 4.0 }, ink);
            if tall && k > 0 {
                let t = (k / 2).to_string();
                let tw = ui.text_width(&t, 9.0, false);
                ui.fonts.draw(ui.c, &t, x - tw / 2.0, y + 13.0, 9.0, false, ink);
            }
            k += 1;
        }
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
        x += label(ui, "Localizar:", x) + 6.0;
        let focus = std::mem::take(&mut f.focus);
        let (changed, enter) = ui.text_field_in(Rect::new(x, row, fw, 20.0), &mut f.query, focus);
        // "2 de 3" dentro do campo, à direita (como na busca do Safari da era).
        if !f.message.is_empty() {
            let mw = ui.text_width(&f.message, 10.0, false);
            ui.fonts.draw(ui.c, &f.message, x + fw - mw - 5.0, row + 14.0, 10.0, false, rgba(130, 130, 136, 255));
        }
        x += fw + 10.0;
        x += label(ui, "Substituir:", x) + 6.0;
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
                if ui.small_button("Substituir") {
                    act = Some("one");
                }
                if ui.small_button("Tudo") {
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
        if ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Escape) {
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
            let title = format!("Deseja salvar as alterações feitas no documento “{}”?", self.name());
            match ui.alert(AlertKind::Caution, &title, "Se você não salvar, as alterações serão perdidas.", &["Não salvar", "Cancelar", "Salvar"]) {
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

fn spawn_self(path: Option<&Path>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| "glove-edit".into());
    let mut cmd = std::process::Command::new(exe);
    if let Some(p) = path {
        cmd.arg(p);
    }
    let _ = cmd.spawn();
}

impl App for Editor {
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
                    "Ctrl+M" => action = Some("minimize"),
                    "Ctrl+equal" | "Ctrl+plus" => action = Some("bigger"),
                    "Ctrl+minus" => action = Some("smaller"),
                    "Ctrl+F" => action = Some("find"),
                    "Ctrl+G" => action = Some("find-next"),
                    "Ctrl+Shift+G" => action = Some("find-prev"),
                    "Ctrl+E" => action = Some("use-selection"),
                    "Ctrl+R" => action = Some("ruler"),
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
                    "delete" => self.buf.delete(),
                    "font-reset" => self.buf.font_px = 0.0,
                    "align-left" => menu_action = Some("align-left"),
                    "align-center" => menu_action = Some("align-center"),
                    "align-right" => menu_action = Some("align-right"),
                    "replace-all" => self.replace_all(),
                    _ => {}
                }
            }
            if menu_action.is_some() {
                self.run_action(menu_action, ui);
            }
            self.run_action(action, ui);
        }

        let title = format!("{}{}", self.name(), if self.buf.modified { " — Editado" } else { "" });
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }

        // Régua e barra de Localizar no alto; o resto é o documento.
        ui.set_enabled(!sheet_open);
        let (w, h) = (ui.c.w, ui.c.h);
        let mut top = 0.0;
        if self.ruler {
            self.draw_ruler(ui, top);
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
        ui.text_area(&mut self.buf, Rect::new(0.0, top, w, h - top));
        ui.set_enabled(!sheet_open);
        self.sheet(ui);
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.buf.has_selection();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new", "Novo", "Ctrl+N"),
                    MenuItem::new("open", "Abrir…", "Ctrl+O"),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                    MenuItem::new("save", "Salvar", "Ctrl+S"),
                    MenuItem::new("save-as", "Salvar como…", "Ctrl+Shift+S"),
                ],
            ),
            Menu::new(
                "Editar",
                vec![
                    MenuItem::new("undo", "Desfazer", "Ctrl+Z").enabled(self.buf.can_undo()),
                    MenuItem::new("redo", "Refazer", "Ctrl+Shift+Z").enabled(self.buf.can_redo()),
                    MenuItem::Separator,
                    MenuItem::new("cut", "Recortar", "Ctrl+X").enabled(sel),
                    MenuItem::new("copy", "Copiar", "Ctrl+C").enabled(sel),
                    MenuItem::new("paste", "Colar", "Ctrl+V"),
                    MenuItem::new("delete", "Apagar", "").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("select-all", "Selecionar tudo", "Ctrl+A"),
                    MenuItem::Separator,
                    MenuItem::submenu(
                        "Localizar",
                        vec![
                            MenuItem::new("find", "Localizar…", "Ctrl+F"),
                            MenuItem::new("find-next", "Localizar Próximo", "Ctrl+G"),
                            MenuItem::new("find-prev", "Localizar Anterior", "Ctrl+Shift+G"),
                            MenuItem::new("use-selection", "Usar Seleção para Localizar", "Ctrl+E").enabled(sel),
                            MenuItem::Separator,
                            MenuItem::new("replace-all", "Substituir Tudo", "").enabled(self.find.is_some()),
                        ],
                    ),
                ],
            ),
            Menu::new(
                "Formatar",
                vec![
                    MenuItem::new("bigger", "Aumentar fonte", "Ctrl+equal"),
                    MenuItem::new("smaller", "Diminuir fonte", "Ctrl+minus"),
                    MenuItem::new("font-reset", "Tamanho normal", "").enabled(self.buf.font_px > 0.0),
                    MenuItem::Separator,
                    MenuItem::new("align-left", "Alinhar à Esquerda", "").checked(self.buf.align == Align::Left),
                    MenuItem::new("align-center", "Centralizar", "").checked(self.buf.align == Align::Center),
                    MenuItem::new("align-right", "Alinhar à Direita", "").checked(self.buf.align == Align::Right),
                    MenuItem::Separator,
                    MenuItem::new("ruler", if self.ruler { "Ocultar Régua" } else { "Mostrar Régua" }, "Ctrl+R"),
                ],
            ),
        ]
    }

    fn close_requested(&mut self) -> bool {
        if self.buf.modified {
            self.sheet = Sheet::AskSave;
            false
        } else {
            true
        }
    }
}

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let ed = Editor::new(path);
    let title = ed.name();
    let opts = WindowOptions { title, app_id: "glove.TextEditor".into(), size: (640, 480), fixed: false };
    if let Err(e) = run(opts, ed) {
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
