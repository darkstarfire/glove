//! Contatos (como a Agenda de Endereços da era): grupos, nomes e a ficha,
//! com Editar. Cada contato é um .vcf em ~/.local/share/glove/contatos.

mod vcard;

use std::path::PathBuf;

use glove_kit::{
    paint::{draw::*, chrome, Color},
    run, App, Choice, FileBrowser, Menu, MenuItem, Rect, Ui, WindowOptions, SEARCH_H,
};
use vcard::Contact;

const TOOL_H: f32 = 40.0;
const GROUP_W: f32 = 140.0;
const NAME_W: f32 = 190.0;
const ROW_H: f32 = 20.0;

fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("glove/contatos")
}

fn new_uid() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("glove-{t:x}-{:x}", std::process::id())
}

fn load_all() -> Vec<Contact> {
    let mut v: Vec<Contact> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "vcf"))
        .filter_map(|e| {
            let mut c = vcard::parse(&std::fs::read_to_string(e.path()).ok()?).into_iter().next()?;
            c.uid = e.path().file_stem()?.to_string_lossy().into_owned();
            Some(c)
        })
        .collect();
    sort(&mut v);
    v
}

fn sort(v: &mut [Contact]) {
    v.sort_by_key(|c| c.display().to_lowercase());
}

fn save(c: &Contact) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    std::fs::write(dir().join(format!("{}.vcf", c.uid)), vcard::serialize(c))
}

/// "1990-05-04" → "04/05/1990" (e de volta).
fn br_date(iso: &str) -> String {
    let p: Vec<&str> = iso.split('-').collect();
    if p.len() == 3 { format!("{}/{}/{}", p[2], p[1], p[0]) } else { iso.to_string() }
}

fn iso_date(br: &str) -> String {
    let p: Vec<&str> = br.split('/').map(str::trim).collect();
    if p.len() == 3 && p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit())) { format!("{}-{:0>2}-{:0>2}", p[2], p[1], p[0]) } else { br.to_string() }
}

/// Campos do modo Editar (rótulo, valor).
const FIELDS: [&str; 11] = ["Nome", "Sobrenome", "Empresa", "Celular", "Trabalho", "Casa", "E-mail", "E-mail do trabalho", "Endereço", "Aniversário", "Grupo"];

fn phone(c: &Contact, k: &str) -> String {
    c.phones.iter().find(|p| p.0 == k).map(|p| p.1.clone()).unwrap_or_default()
}

fn email(c: &Contact, k: &str) -> String {
    c.emails.iter().find(|p| p.0 == k).map(|p| p.1.clone()).unwrap_or_default()
}

fn to_form(c: &Contact) -> Vec<String> {
    vec![
        c.first.clone(),
        c.last.clone(),
        c.org.clone(),
        phone(c, "celular"),
        phone(c, "trabalho"),
        phone(c, "casa"),
        email(c, "casa").max(email(c, "outro")),
        email(c, "trabalho"),
        c.address.clone(),
        br_date(&c.birthday),
        c.group.clone(),
    ]
}

fn from_form(c: &mut Contact, f: &[String], note: &str) {
    let t = |i: usize| f[i].trim().to_string();
    c.first = t(0);
    c.last = t(1);
    c.org = t(2);
    c.phones = [("celular", 3), ("trabalho", 4), ("casa", 5)].iter().filter(|(_, i)| !t(*i).is_empty()).map(|(k, i)| (k.to_string(), t(*i))).collect();
    c.emails = [("casa", 6), ("trabalho", 7)].iter().filter(|(_, i)| !t(*i).is_empty()).map(|(k, i)| (k.to_string(), t(*i))).collect();
    c.address = t(8);
    c.birthday = iso_date(&t(9));
    c.group = t(10);
    c.note = note.trim().to_string();
}

struct Contatos {
    all: Vec<Contact>,
    group: Option<String>,
    selected: Option<String>,
    filter: String,
    /// Editando: o formulário e a nota.
    editing: Option<(Vec<String>, String)>,
    file: Option<(FileBrowser, bool)>,
    message: Option<String>,
    name_scroll: f32,
}

impl Contatos {
    fn groups(&self) -> Vec<String> {
        let mut g: Vec<String> = self.all.iter().map(|c| c.group.clone()).filter(|g| !g.is_empty()).collect();
        g.sort_by_key(|s| s.to_lowercase());
        g.dedup();
        g
    }

    fn visible(&self) -> Vec<&Contact> {
        let f = self.filter.trim().to_lowercase();
        self.all
            .iter()
            .filter(|c| self.group.as_ref().is_none_or(|g| &c.group == g))
            .filter(|c| f.is_empty() || [c.display(), c.org.clone()].iter().chain(c.phones.iter().map(|p| &p.1)).chain(c.emails.iter().map(|p| &p.1)).any(|t| t.to_lowercase().contains(&f)))
            .collect()
    }

    fn current(&self) -> Option<&Contact> {
        self.selected.as_ref().and_then(|u| self.all.iter().find(|c| &c.uid == u))
    }

    fn new_contact(&mut self) {
        let c = Contact { uid: new_uid(), group: self.group.clone().unwrap_or_default(), ..Default::default() };
        self.selected = Some(c.uid.clone());
        self.editing = Some((to_form(&c), String::new()));
        self.all.push(c);
    }

    fn finish_edit(&mut self) {
        let Some((form, note)) = self.editing.take() else { return };
        let Some(uid) = self.selected.clone() else { return };
        if let Some(c) = self.all.iter_mut().find(|c| c.uid == uid) {
            from_form(c, &form, &note);
            if let Err(e) = save(c) {
                self.message = Some(format!("Não foi possível salvar: {e}"));
            }
        }
        sort(&mut self.all);
    }

    fn delete(&mut self) {
        let Some(uid) = self.selected.take() else { return };
        let _ = std::fs::remove_file(dir().join(format!("{uid}.vcf")));
        self.all.retain(|c| c.uid != uid);
        self.editing = None;
    }

    /// Coluna com linhas clicáveis (grupos ou nomes).
    fn column(ui: &mut Ui, r: Rect, title: &str, rows: &[String], sel: Option<usize>, scroll: &mut f32) -> Option<usize> {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        ui.c.fill_rect(r.x, r.y, r.w, 18.0, &vgrad(r.y, r.y + 18.0, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(r.x, r.y + 18.0, r.w, rgba(170, 170, 176, 255));
        ui.fonts.draw(ui.c, title, r.x + 8.0, r.y + 13.0, 11.0, false, rgba(40, 40, 44, 255));
        ui.c.vline(r.x + r.w - 0.5, r.y, r.h, rgba(170, 170, 176, 255));
        let list = Rect::new(r.x, r.y + 19.0, r.w - 1.0, r.h - 19.0);
        *scroll = (*scroll - ui.scroll_in(list)).clamp(0.0, (rows.len() as f32 * ROW_H - list.h).max(0.0));
        let mut clicked = None;
        for (i, t) in rows.iter().enumerate() {
            let y = list.y + i as f32 * ROW_H - *scroll;
            if y + ROW_H < list.y || y > list.y + list.h {
                continue;
            }
            let rr = Rect::new(list.x, y, list.w, ROW_H);
            if sel == Some(i) {
                ui.c.fill_rect(rr.x, rr.y.max(list.y), rr.w, ROW_H, &solid(ui.theme.accent));
            }
            let ink = if sel == Some(i) { Color::WHITE } else { rgba(20, 20, 24, 255) };
            let t = ui.fonts.ellipsize(t, 12.0, false, rr.w - 14.0);
            ui.fonts.draw(ui.c, &t, rr.x + 8.0, y + 14.0, 12.0, false, ink);
            if ui.hit(rr).clicked {
                clicked = Some(i);
            }
        }
        clicked
    }

    fn card(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(252, 252, 252, 255)));
        let Some(c) = self.current().cloned() else {
            ui.fonts.draw(ui.c, "Nenhum contato escolhido.", r.x + 20.0, r.y + 40.0, 12.0, false, rgba(140, 140, 146, 255));
            return;
        };
        // Retrato desenhado e o nome.
        chrome::avatar(ui.c, r.x + 16.0, r.y + 14.0, 56.0, chrome::avatar_seed(&c.display()));
        if let Some((form, note)) = &mut self.editing {
            // Formulário.
            let lw = 120.0;
            let mut y = r.y + 82.0;
            for (i, label) in FIELDS.iter().enumerate() {
                let tw = ui.text_width(label, 11.0, false);
                ui.fonts.draw(ui.c, label, r.x + lw - tw, y + 14.0, 11.0, false, rgba(110, 110, 116, 255));
                ui.push_id(label, |ui| {
                    ui.text_field_in(Rect::new(r.x + lw + 8.0, y, (r.w - lw - 24.0).min(260.0), 20.0), &mut form[i], false);
                });
                y += 25.0;
            }
            let tw = ui.text_width("Nota", 11.0, false);
            ui.fonts.draw(ui.c, "Nota", r.x + lw - tw, y + 14.0, 11.0, false, rgba(110, 110, 116, 255));
            ui.text_field_in(Rect::new(r.x + lw + 8.0, y, (r.w - lw - 24.0).min(260.0), 20.0), note, false);
            let first = form[0].clone();
            let last = form[1].clone();
            let t = format!("{first} {last}");
            ui.fonts.draw(ui.c, if t.trim().is_empty() { "Contato novo" } else { t.trim() }, r.x + 84.0, r.y + 40.0, 16.0, true, rgba(20, 20, 24, 255));
            return;
        }
        ui.fonts.draw(ui.c, &c.display(), r.x + 84.0, r.y + 40.0, 16.0, true, rgba(20, 20, 24, 255));
        if !c.org.is_empty() && c.display() != c.org {
            ui.fonts.draw(ui.c, &c.org, r.x + 84.0, r.y + 58.0, 12.0, false, rgba(90, 90, 96, 255));
        }
        // Linhas "rótulo  valor", como na ficha da era.
        let mut rows: Vec<(String, String)> = Vec::new();
        rows.extend(c.phones.iter().map(|(k, v)| (k.clone(), v.clone())));
        rows.extend(c.emails.iter().map(|(k, v)| (if k == "trabalho" { "e-mail trab.".into() } else { "e-mail".into() }, v.clone())));
        if !c.address.is_empty() {
            rows.push(("endereço".into(), c.address.clone()));
        }
        if !c.birthday.is_empty() {
            rows.push(("aniversário".into(), br_date(&c.birthday)));
        }
        if !c.group.is_empty() {
            rows.push(("grupo".into(), c.group.clone()));
        }
        let lw = 100.0;
        let mut y = r.y + 100.0;
        for (k, v) in rows {
            let tw = ui.text_width(&k, 11.0, true);
            ui.fonts.draw(ui.c, &k, r.x + lw - tw, y, 11.0, true, rgba(130, 130, 136, 255));
            let v = ui.fonts.ellipsize(&v, 12.0, false, r.w - lw - 24.0);
            ui.fonts.draw(ui.c, &v, r.x + lw + 10.0, y, 12.0, false, rgba(20, 20, 24, 255));
            y += 22.0;
        }
        if !c.note.is_empty() {
            y += 8.0;
            ui.c.hline(r.x + 16.0, y - 12.0, r.w - 32.0, rgba(220, 220, 224, 255));
            for line in c.note.lines().take(6) {
                ui.fonts.draw(ui.c, line, r.x + 20.0, y, 12.0, false, rgba(60, 60, 66, 255));
                y += 17.0;
            }
        }
    }
}

impl App for Contatos {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if self.file.is_none() {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+N" => acts.push("new".into()),
                    "Ctrl+L" => acts.push("edit".into()),
                    "Ctrl+W" => acts.push("close".into()),
                    "Ctrl+Shift+I" => acts.push("import".into()),
                    "Ctrl+Shift+E" => acts.push("export".into()),
                    _ => {}
                }
            }
        }
        // Barra: busca à direita.
        ui.c.fill_rect(0.0, 0.0, w, TOOL_H, &vgrad(0.0, TOOL_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(206, 206, 210, 255))]));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(140, 140, 146, 255));
        ui.search_field_in(Rect::new(w - 200.0, (TOOL_H - SEARCH_H) / 2.0, 186.0, SEARCH_H), &mut self.filter, "Buscar");
        let n = self.all.len();
        let t = format!("{n} contato{}", if n == 1 { "" } else { "s" });
        ui.fonts.draw(ui.c, &t, 14.0, TOOL_H / 2.0 + 4.0, 12.0, false, rgba(60, 60, 66, 255));

        let foot = 34.0;
        let body = Rect::new(0.0, TOOL_H, w, h - TOOL_H - foot);
        // Grupos.
        let mut groups = vec!["Todos".to_string()];
        groups.extend(self.groups());
        let gsel = match &self.group {
            None => Some(0),
            Some(g) => groups.iter().position(|x| x == g),
        };
        let mut s0 = 0.0;
        if let Some(i) = Self::column(ui, Rect::new(0.0, body.y, GROUP_W, body.h), "Grupo", &groups, gsel, &mut s0) {
            self.finish_edit();
            self.group = (i > 0).then(|| groups[i].clone());
        }
        // Nomes.
        let vis: Vec<(String, String)> = self.visible().iter().map(|c| (c.uid.clone(), c.display())).collect();
        let names: Vec<String> = vis.iter().map(|v| v.1.clone()).collect();
        let nsel = self.selected.as_ref().and_then(|u| vis.iter().position(|v| &v.0 == u));
        let mut sc = self.name_scroll;
        if let Some(i) = Self::column(ui, Rect::new(GROUP_W, body.y, NAME_W, body.h), "Nome", &names, nsel, &mut sc) {
            self.finish_edit();
            self.selected = Some(vis[i].0.clone());
        }
        self.name_scroll = sc;
        // Ficha.
        self.card(ui, Rect::new(GROUP_W + NAME_W, body.y, w - GROUP_W - NAME_W, body.h));

        // Rodapé: + e − e Editar/OK.
        let fy = h - foot;
        ui.c.fill_rect(0.0, fy, w, foot, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        let has = self.selected.is_some();
        let editing = self.editing.is_some();
        ui.area(Rect::new(GROUP_W + 8.0, fy + 6.0, 120.0, 24.0), |ui| {
            ui.horizontal(|ui| {
                ui.spacing(4.0);
                if ui.small_button("+") {
                    acts.push("new".into());
                }
                ui.set_active(has);
                if ui.small_button("−") {
                    acts.push("delete".into());
                }
                ui.set_active(true);
            });
        });
        ui.area(Rect::new(GROUP_W + NAME_W + 16.0, fy + 5.0, 140.0, 26.0), |ui| {
            ui.set_active(has);
            if if editing { ui.default_button("OK") } else { ui.button("Editar") } {
                acts.push("edit".into());
            }
            ui.set_active(true);
        });
        if let Some(m) = &self.message {
            let m = m.clone();
            ui.fonts.draw(ui.c, &m, GROUP_W + NAME_W + 170.0, fy + 21.0, 11.0, false, rgba(160, 50, 40, 255));
        }

        for a in acts {
            match a.as_str() {
                "new" => {
                    self.finish_edit();
                    self.new_contact();
                }
                "edit" => {
                    if self.editing.is_some() {
                        self.finish_edit();
                    } else if let Some(c) = self.current() {
                        self.editing = Some((to_form(c), c.note.clone()));
                    }
                }
                "delete" => self.delete(),
                "import" => self.file = Some((FileBrowser::open(None, &["vcf"]), false)),
                "export" => {
                    if let Some(c) = self.current() {
                        self.file = Some((FileBrowser::save(None, &format!("{}.vcf", c.display().replace('/', " "))), true));
                    }
                }
                "close" => {
                    self.finish_edit();
                    ui.close = true;
                }
                _ => {}
            }
        }
        // Importar / Exportar.
        if let Some((fb, export)) = &mut self.file {
            let export = *export;
            if let Some(c) = ui.file_sheet(fb) {
                self.file = None;
                if let Choice::Picked(p) = c {
                    if export {
                        if let Some(c) = self.current() {
                            if let Err(e) = std::fs::write(&p, vcard::serialize(c)) {
                                self.message = Some(format!("Não foi possível exportar: {e}"));
                            }
                        }
                    } else {
                        let text = std::fs::read_to_string(&p).unwrap_or_default();
                        let mut n = 0;
                        for mut c in vcard::parse(&text) {
                            if c.uid.is_empty() || self.all.iter().any(|x| x.uid == c.uid) {
                                c.uid = format!("{}-{n}", new_uid());
                            }
                            if save(&c).is_ok() {
                                self.all.push(c);
                                n += 1;
                            }
                        }
                        sort(&mut self.all);
                        self.message = Some(format!("{n} contato(s) importado(s)."));
                    }
                }
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let has = self.selected.is_some();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new", "Novo Contato", "Ctrl+N"),
                    MenuItem::Separator,
                    MenuItem::new("import", "Importar vCards…", "Ctrl+Shift+I"),
                    MenuItem::new("export", "Exportar vCard…", "Ctrl+Shift+E").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                ],
            ),
            Menu::new(
                "Ficha",
                vec![
                    MenuItem::new("edit", if self.editing.is_some() { "Terminar Edição" } else { "Editar Ficha" }, "Ctrl+L").enabled(has),
                    MenuItem::new("delete", "Apagar Contato", "").enabled(has),
                ],
            ),
        ]
    }

    fn close_requested(&mut self) -> bool {
        self.finish_edit();
        true
    }
}

fn main() {
    let all = load_all();
    let selected = all.first().map(|c| c.uid.clone());
    let app = Contatos { all, group: None, selected, filter: String::new(), editing: None, file: None, message: None, name_scroll: 0.0 };
    let opts = WindowOptions { title: "Contatos".into(), app_id: "glove.Contatos".into(), size: (760, 470), fixed: false };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-contatos: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn datas() {
        assert_eq!(super::br_date("1990-05-04"), "04/05/1990");
        assert_eq!(super::iso_date("4/5/1990"), "1990-05-04");
        assert_eq!(super::iso_date("maio"), "maio");
    }
}
