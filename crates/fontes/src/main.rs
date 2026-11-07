//! Livro de Fontes (como o do Panther): coleções à esquerda, famílias no
//! meio e a amostra à direita. Instala fontes na pasta pessoal e desativa
//! famílias com uma regra do fontconfig (sem apagar nada).

use glove_config::tr;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, font::Fonts, Color},
    run, AlertKind, App, Choice, FileBrowser, Menu, MenuItem, Rect, Ticks, Ui, WindowOptions, SEARCH_H, SEGMENT_H,
};

const COLLECTIONS: [&str; 4] = ["Todas as Fontes", "Pessoais", "Computador", "Desativadas"];
const SAMPLE: &str = "À noite, vovô Kowalsky vê o ímã cair no pé do pinguim queixoso e vovó põe açúcar no chá de tâmaras do jabuti feliz.";
const REPERTOIRE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,;:!?¿¡\"'()[]{}<>@#$%&*+-=/\\|_~^`ÀÁÂÃÇÉÊÍÓÔÕÚÜàáâãçéêíóôõúü€£¥©®°§¶•…–—«»";
const BAR: f32 = 40.0;

#[derive(Clone)]
struct Face {
    style: String,
    file: PathBuf,
    index: u32,
}

#[derive(Clone)]
struct Family {
    name: String,
    faces: Vec<Face>,
    personal: bool,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

/// Pasta das fontes do usuário (o fontconfig já procura nela).
fn user_fonts() -> PathBuf {
    data_home().join("fonts")
}

/// Regra do fontconfig com as famílias desativadas.
fn rule_file() -> PathBuf {
    config_home().join("fontconfig/conf.d/50-glove-desativadas.conf")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

/// Famílias desativadas, lidas da própria regra.
pub fn disabled_from(xml: &str) -> Vec<String> {
    xml.split("<string>").skip(1).filter_map(|p| p.split_once("</string>").map(|(v, _)| xml_unescape(v))).collect()
}

/// Regra que faz o fontconfig ignorar essas famílias.
pub fn rule_xml(families: &[String]) -> String {
    let mut s = String::from("<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n<!-- Escrito pelo Livro de Fontes do Glove: famílias desativadas. -->\n<fontconfig>\n  <selectfont>\n    <rejectfont>\n");
    for f in families {
        s.push_str(&format!("      <pattern><patelt name=\"family\"><string>{}</string></patelt></pattern>\n", xml_escape(f)));
    }
    s.push_str("    </rejectfont>\n  </selectfont>\n</fontconfig>\n");
    s
}

fn read_disabled() -> Vec<String> {
    std::fs::read_to_string(rule_file()).map(|x| disabled_from(&x)).unwrap_or_default()
}

fn write_disabled(v: &[String]) -> std::io::Result<()> {
    let p = rule_file();
    if v.is_empty() {
        let _ = std::fs::remove_file(&p);
        return Ok(());
    }
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, rule_xml(v))
}

/// Ordem dos estilos: Regular primeiro, depois Itálico, Negrito…
fn style_rank(s: &str) -> (u8, String) {
    let l = s.to_lowercase();
    let r = if l == "regular" || l == "book" || l == "roman" || l == "normal" {
        0
    } else if l.contains("bold") || l.contains("negrito") {
        if l.contains("italic") || l.contains("oblique") { 4 } else { 3 }
    } else if l.contains("italic") || l.contains("oblique") {
        1
    } else {
        2
    };
    (r, l)
}

fn list_fonts() -> Vec<Family> {
    let out = Command::new("fc-list").args(["--format", "%{family[0]}\t%{style[0]}\t%{file}\t%{index}\n"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let personal_dirs = [user_fonts(), home().join(".fonts")];
    let mut map: BTreeMap<String, Family> = BTreeMap::new();
    for l in out.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 4 || f[0].is_empty() {
            continue;
        }
        let file = PathBuf::from(f[2]);
        let personal = personal_dirs.iter().any(|d| file.starts_with(d));
        let fam = map.entry(f[0].to_lowercase()).or_insert_with(|| Family { name: f[0].to_string(), faces: Vec::new(), personal });
        fam.personal |= personal;
        // O mesmo estilo em dois formatos: fica o TrueType/OpenType (o Type 1 não abre).
        let sfnt = |p: &PathBuf| p.extension().is_some_and(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"));
        let face = Face { style: f[1].to_string(), file, index: f[3].parse().unwrap_or(0) };
        match fam.faces.iter_mut().find(|x| x.style == face.style) {
            Some(old) if !sfnt(&old.file) && sfnt(&face.file) => *old = face,
            Some(_) => {}
            None => fam.faces.push(face),
        }
    }
    let mut v: Vec<Family> = map.into_values().collect();
    for fam in &mut v {
        fam.faces.sort_by_key(|f| style_rank(&f.style));
    }
    v
}

/// Copia as fontes para a pasta pessoal e refaz o cache do fontconfig.
fn install(files: &[PathBuf]) -> Result<usize, String> {
    let dest = user_fonts();
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let mut n = 0;
    for f in files {
        let name = f.file_name().ok_or(tr!("arquivo sem nome"))?;
        std::fs::copy(f, dest.join(name)).map_err(|e| e.to_string())?;
        n += 1;
    }
    let _ = Command::new("fc-cache").arg("-f").arg(&dest).status();
    Ok(n)
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Sample,
    Repertoire,
    Custom,
}

struct Livro {
    families: Arc<Mutex<Option<Vec<Family>>>>,
    disabled: Vec<String>,
    collection: Option<usize>,
    family: Option<String>,
    filter: String,
    mode: Mode,
    size: f32,
    custom: String,
    /// Fontes já abertas para a amostra (poucas, para não pesar).
    loaded: Vec<((PathBuf, u32), Option<Fonts>)>,
    scroll: f32,
    open_fb: Option<FileBrowser>,
    message: Option<String>,
    /// Fontes abertas pelo Arquivos, esperando o "Instalar".
    pending: Vec<PathBuf>,
}

impl Livro {
    fn reload(&mut self) {
        let slot = self.families.clone();
        *slot.lock().unwrap() = None;
        self.disabled = read_disabled();
        self.loaded.clear();
        std::thread::spawn(move || {
            let v = list_fonts();
            *slot.lock().unwrap() = Some(v);
        });
    }

    fn font(&mut self, face: &Face) -> Option<usize> {
        let key = (face.file.clone(), face.index);
        if let Some(i) = self.loaded.iter().position(|(k, _)| *k == key) {
            return self.loaded[i].1.as_ref().map(|_| i);
        }
        if self.loaded.len() >= 12 {
            self.loaded.remove(0);
        }
        self.loaded.push((key, Fonts::load_file(&face.file, face.index)));
        let i = self.loaded.len() - 1;
        self.loaded[i].1.as_ref().map(|_| i)
    }

    fn set_disabled(&mut self, name: &str, off: bool) {
        self.disabled.retain(|d| !d.eq_ignore_ascii_case(name));
        if off {
            self.disabled.push(name.to_string());
        }
        match write_disabled(&self.disabled) {
            Ok(()) => {
                self.message = Some(if off { tr!("“{}” desativada. Os apps abertos a deixam de usar quando forem reabertos.", name) } else { format!("“{name}” ativada de novo.") });
                let _ = Command::new("fc-cache").spawn();
            }
            Err(e) => self.message = Some(tr!("Não deu para salvar: {}", e)),
        }
    }

    fn preview(&mut self, ui: &mut Ui, fam: &Family, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        let grey = rgba(120, 120, 126, 255);
        let ink = rgba(20, 20, 24, 255);
        let px = self.size;
        let pad = 18.0;
        let max = r.w - 2.0 * pad;
        let mut y = r.y + 14.0 - self.scroll;
        let top = r.y;
        let bottom = r.y + r.h;
        let total;
        match self.mode {
            Mode::Repertoire => {
                let face = fam.faces[0].clone();
                let Some(fi) = self.font(&face) else { return };
                let fonts = self.loaded[fi].1.as_ref().unwrap();
                let cell = (px * 1.8).max(28.0);
                let cols = ((max / cell) as usize).max(1);
                let chars: Vec<char> = REPERTOIRE.chars().filter(|c| fonts.has_char(*c)).collect();
                for (k, ch) in chars.iter().enumerate() {
                    let (cx, cy) = (r.x + pad + (k % cols) as f32 * cell, y + (k / cols) as f32 * cell);
                    if cy + cell > top && cy < bottom {
                        ui.c.stroke(rrect(cx + 0.5, cy + 0.5, cell - 1.0, cell - 1.0, 0.0), &solid(rgba(226, 226, 230, 255)), 1.0);
                        let t = ch.to_string();
                        let tw = fonts.measure(&t, px, false);
                        fonts.draw(ui.c, &t, cx + (cell - tw) / 2.0, cy + cell / 2.0 + px * 0.35, px, false, ink);
                    }
                }
                total = (chars.len().div_ceil(cols)) as f32 * cell + 28.0;
            }
            Mode::Sample | Mode::Custom => {
                let text = if self.mode == Mode::Custom && !self.custom.trim().is_empty() { self.custom.clone() } else { tr(SAMPLE).to_string() };
                for face in fam.faces.clone() {
                    let label = format!("{} {}", fam.name, face.style);
                    y += 16.0;
                    if y > top && y < bottom + 12.0 {
                        ui.fonts.draw(ui.c, &label, r.x + pad, y, 10.0, false, grey);
                    }
                    let Some(fi) = self.font(&face) else {
                        y += 18.0;
                        if y > top && y < bottom {
                            ui.fonts.draw(ui.c, tr!("(não deu para abrir este arquivo)"), r.x + pad, y, 11.0, false, grey);
                        }
                        continue;
                    };
                    let fonts = self.loaded[fi].1.as_ref().unwrap();
                    // Quebra a amostra em linhas que caibam.
                    let mut line = String::new();
                    let mut lines = Vec::new();
                    for w in text.split(' ') {
                        let t = if line.is_empty() { w.to_string() } else { format!("{line} {w}") };
                        if fonts.measure(&t, px, false) > max && !line.is_empty() {
                            lines.push(std::mem::take(&mut line));
                            line = w.to_string();
                        } else {
                            line = t;
                        }
                    }
                    lines.push(line);
                    for l in lines {
                        y += px * 1.3;
                        if y > top && y - px < bottom {
                            fonts.draw(ui.c, &l, r.x + pad, y, px, false, ink);
                        }
                    }
                    y += 14.0;
                }
                total = y + self.scroll - r.y + 10.0;
            }
        }
        self.scroll = (self.scroll - ui.scroll_in(r)).clamp(0.0, (total - r.h).max(0.0));
    }
}

impl App for Livro {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.open_fb.is_none() {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+W" => acts.push("close".into()),
                    "Ctrl+O" => acts.push("add".into()),
                    "Ctrl+Shift+D" => acts.push("toggle".into()),
                    _ => {}
                }
            }
        }
        // Barra: modo da amostra, tamanho e busca.
        ui.c.fill_rect(0.0, 0.0, w, BAR, &vgrad(0.0, BAR, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(204, 204, 208, 255))]));
        ui.c.hline(0.0, BAR - 1.0, w, rgba(140, 140, 146, 255));
        let modes = [tr!("Amostra"), tr!("Repertório"), tr!("Personalizado")];
        let cur = match self.mode {
            Mode::Sample => 0,
            Mode::Repertoire => 1,
            Mode::Custom => 2,
        };
        let mut pick = None;
        ui.area(Rect::new(12.0, (BAR - SEGMENT_H) / 2.0, 300.0, SEGMENT_H), |ui| {
            pick = ui.segmented_text(&modes, Some(cur));
        });
        if let Some(p) = pick {
            self.mode = [Mode::Sample, Mode::Repertoire, Mode::Custom][p];
            self.scroll = 0.0;
        }
        ui.fonts.draw(ui.c, tr!("Tamanho"), 330.0, BAR / 2.0 + 4.0, 11.0, false, rgba(40, 40, 44, 255));
        let mut size = self.size;
        ui.area(Rect::new(382.0, BAR / 2.0 - 10.0, 130.0, 24.0), |ui| {
            if ui.slider_ticks(&mut size, 9.0, 72.0, 120.0, &Ticks { count: 0, labels: &[], snap: false }, true) {
                self.size = size.round();
            }
        });
        ui.fonts.draw(ui.c, &tr!("{} pt", self.size as i32), 518.0, BAR / 2.0 + 4.0, 11.0, false, rgba(40, 40, 44, 255));
        ui.search_field_in(Rect::new(w - 200.0, (BAR - SEARCH_H) / 2.0, 188.0, SEARCH_H), &mut self.filter, tr!("Buscar fonte"));

        let slot = self.families.clone();
        let guard = slot.lock().unwrap();
        let Some(all) = guard.as_ref() else {
            drop(guard);
            ui.fonts.draw(ui.c, tr!("Lendo as fontes instaladas…"), 20.0, BAR + 36.0, 12.0, false, rgba(110, 110, 116, 255));
            ui.wake_after(Duration::from_millis(200));
            return;
        };
        // Coleções.
        let cw = 150.0;
        let foot = 34.0;
        let names: Vec<String> = COLLECTIONS.iter().map(|s| tr(s).to_string()).collect();
        let ch = h - BAR - 16.0 - foot;
        ui.area(Rect::new(8.0, BAR + 8.0, cw, ch), |ui| {
            ui.list(&mut self.collection, &names, cw, ch);
        });
        // Famílias da coleção (com a busca).
        let col = self.collection.unwrap_or(0);
        let q = self.filter.trim().to_lowercase();
        let disabled_rows: Vec<Family>;
        let shown: Vec<&Family> = if col == 3 {
            disabled_rows = self.disabled.iter().map(|d| Family { name: d.clone(), faces: Vec::new(), personal: false }).collect();
            disabled_rows.iter().filter(|f| q.is_empty() || f.name.to_lowercase().contains(&q)).collect()
        } else {
            all.iter()
                .filter(|f| match col {
                    1 => f.personal,
                    2 => !f.personal,
                    _ => true,
                })
                .filter(|f| q.is_empty() || f.name.to_lowercase().contains(&q))
                .collect()
        };
        let fw = 210.0;
        let rows: Vec<String> = shown.iter().map(|f| f.name.clone()).collect();
        let mut sel = self.family.as_ref().and_then(|n| shown.iter().position(|f| &f.name == n));
        if sel.is_none() && !shown.is_empty() {
            sel = Some(0);
        }
        let before = sel;
        ui.area(Rect::new(cw + 16.0, BAR + 8.0, fw, ch), |ui| {
            ui.list(&mut sel, &rows, fw, ch);
        });
        let fam = sel.and_then(|i| shown.get(i)).map(|f| (*f).clone());
        if sel != before || self.family.as_ref() != fam.as_ref().map(|f| &f.name) {
            self.family = fam.as_ref().map(|f| f.name.clone());
            self.scroll = 0.0;
        }
        let count = format!("{} {}", shown.len(), if shown.len() == 1 { tr!("família") } else { tr!("famílias") });
        drop(guard);
        // Rodapé: + (adicionar) e Desativar/Ativar.
        let fy = h - foot - 2.0;
        ui.area(Rect::new(8.0, fy, cw, 26.0), |ui| {
            if ui.small_button(tr!("Adicionar…")) {
                acts.push("add".into());
            }
        });
        let label = if col == 3 { "Ativar" } else { "Desativar" };
        ui.area(Rect::new(cw + 16.0, fy, 100.0, 26.0), |ui| {
            ui.set_enabled(fam.is_some());
            if ui.small_button(label) {
                acts.push("toggle".into());
            }
            ui.set_enabled(true);
        });
        ui.fonts.draw(ui.c, &count, cw + 126.0, fy + 16.0, 10.0, false, rgba(90, 90, 96, 255));
        // Amostra.
        let px0 = cw + fw + 24.0;
        let mut r = Rect::new(px0, BAR + 8.0, w - px0 - 8.0, h - BAR - 16.0);
        if self.mode == Mode::Custom {
            let mut t = self.custom.clone();
            ui.text_field_in(Rect::new(r.x, r.y, r.w, 22.0), &mut t, false);
            self.custom = t;
            r = Rect::new(r.x, r.y + 30.0, r.w, r.h - 30.0);
        }
        match &fam {
            Some(f) if !f.faces.is_empty() => {
                let f = f.clone();
                self.preview(ui, &f, r);
            }
            Some(f) => {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
                ui.fonts.draw(ui.c, &tr!("“{}” está desativada.", f.name), r.x + 18.0, r.y + 30.0, 13.0, true, rgba(60, 60, 66, 255));
                ui.fonts.draw(ui.c, tr!("Clique em Ativar para que os apps voltem a usá-la."), r.x + 18.0, r.y + 52.0, 11.0, false, rgba(120, 120, 126, 255));
            }
            None => ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE)),
        }
        ui.c.stroke(rrect(r.x - 0.5, r.y - 0.5, r.w + 1.0, r.h + 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        if let Some(m) = &self.message {
            let m = ui.fonts.ellipsize(m, 10.0, false, r.w - 16.0);
            ui.c.fill_rect(r.x + 1.0, r.y + r.h - 20.0, r.w - 2.0, 19.0, &solid(rgba(255, 250, 210, 255)));
            ui.fonts.draw(ui.c, &m, r.x + 8.0, r.y + r.h - 6.0, 10.0, false, rgba(60, 60, 50, 255));
        }

        for a in acts {
            match a.as_str() {
                "close" => ui.close = true,
                "add" => self.open_fb = Some(FileBrowser::open(None, &["ttf", "otf", "ttc"])),
                "toggle" => {
                    if let Some(f) = &fam {
                        let off = col != 3;
                        let name = f.name.clone();
                        self.set_disabled(&name, off);
                        if off {
                            self.family = None;
                        }
                        self.reload();
                    }
                }
                "reload" => self.reload(),
                _ => {}
            }
        }
        if !self.pending.is_empty() {
            let names: Vec<String> = self.pending.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
            let title = if names.len() == 1 { tr!("Instalar a fonte “{}”?", names[0]) } else { tr!("Instalar {} fontes?", names.len()) };
            match ui.alert(AlertKind::Note, &title, tr!("Ela fica na sua pasta pessoal e os apps passam a vê-la quando forem reabertos."), &[tr!("Cancelar"), tr!("Instalar")]) {
                Some(1) => {
                    let files = std::mem::take(&mut self.pending);
                    self.message = Some(match install(&files) {
                        Ok(n) => tr!("{} {} em {}.", n, if n == 1 { "fonte instalada" } else { "fontes instaladas" }, user_fonts().display()),
                        Err(e) => tr!("Não deu para instalar: {}", e),
                    });
                    self.collection = Some(1);
                    self.reload();
                }
                Some(_) => self.pending.clear(),
                None => {}
            }
        }
        if let Some(fb) = &mut self.open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.open_fb = None;
                if let Choice::Picked(p) = c {
                    self.message = Some(match install(std::slice::from_ref(&p)) {
                        Ok(_) => tr!("“{}” instalada em {}.", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), user_fonts().display()),
                        Err(e) => tr!("Não deu para instalar: {}", e),
                    });
                    self.collection = Some(1);
                    self.reload();
                }
                ui.redraw = true;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new(tr!("Arquivo"), vec![MenuItem::new("add", tr!("Adicionar Fontes…"), "Ctrl+O"), MenuItem::new("reload", tr!("Recarregar"), ""), MenuItem::Separator, MenuItem::new("close", tr!("Fechar"), "Ctrl+W")]),
            Menu::new(tr!("Editar"), vec![MenuItem::new("toggle", tr!("Desativar/Ativar Família"), "Ctrl+Shift+D")]),
        ]
    }
}

pub fn main() {
    // "glove-fontes arquivo.ttf" (clique duplo numa fonte no Arquivos): pergunta se instala.
    let pending: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).filter(|p| p.is_file()).collect();
    let mut app = Livro {
        families: Arc::default(),
        disabled: Vec::new(),
        collection: Some(0),
        family: None,
        filter: String::new(),
        mode: Mode::Sample,
        size: 24.0,
        custom: String::new(),
        loaded: Vec::new(),
        scroll: 0.0,
        open_fb: None,
        message: None,
        pending,
    };
    app.reload();
    let opts = WindowOptions { title: tr!("Livro de Fontes").into(), app_id: "glove.Fontes".into(), size: (860, 520), fixed: false, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-fontes: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regra_ida_e_volta() {
        let v = vec!["Comic & Co".to_string(), "DejaVu Serif".to_string()];
        let x = rule_xml(&v);
        assert!(x.contains("<string>Comic &amp; Co</string>"));
        assert_eq!(disabled_from(&x), v);
    }

    #[test]
    fn ordem_dos_estilos() {
        let mut s = vec!["Bold", "Italic", "Regular", "Bold Italic", "Light"];
        s.sort_by_key(|x| style_rank(x));
        assert_eq!(s, ["Regular", "Italic", "Light", "Bold", "Bold Italic"]);
    }
}
