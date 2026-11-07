//! Acesso às Chaves (como o Keychain Access do Tiger): os chaveiros à esquerda,
//! as senhas guardadas na tabela e os detalhes embaixo, com Mostrar senha,
//! Copiar senha e Apagar; Trancar/Destrancar e Mudar a senha do chaveiro.
//! Fala com o serviço de senhas do sistema (Secret Service: gnome-keyring,
//! KeePassXC…) pelo D-Bus. `GLOVE_CHAVES_TESTE=1` usa itens de mentira.

mod secret;

use glove_kit::{
    paint::{draw::*, icons::Icon, Color},
    run,
    ui::Key,
    App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

use secret::{Backend, Keyring};

const LEFT_W: f32 = 180.0;
const DETAIL_H: f32 = 172.0;
const TOOL_H: f32 = 44.0;

enum Sheet {
    None,
    /// Apagar o item escolhido (confirmação).
    Delete,
    /// Mudar a senha do chaveiro: (atual, nova, repetida, erro).
    Password(String, String, String, Option<String>),
    Error(String),
}

struct Chaves {
    backend: Backend,
    rings: Vec<Keyring>,
    ring: usize,
    item: Option<usize>,
    /// Senha mostrada (do item escolhido).
    shown: Option<String>,
    search: String,
    table: TableState,
    sheet: Sheet,
    message: Option<String>,
}

impl Chaves {
    fn reload(&mut self) {
        match self.backend.keyrings() {
            Ok(r) => self.rings = r,
            Err(e) => self.message = Some(e),
        }
        self.ring = self.ring.min(self.rings.len().saturating_sub(1));
        self.shown = None;
    }

    fn visible(&self) -> Vec<usize> {
        let Some(r) = self.rings.get(self.ring) else { return Vec::new() };
        let q = self.search.to_lowercase();
        let mut v: Vec<usize> = (0..r.items.len()).filter(|&i| q.is_empty() || r.items[i].label.to_lowercase().contains(&q) || r.items[i].account.to_lowercase().contains(&q) || r.items[i].place.to_lowercase().contains(&q)).collect();
        v.sort_by_key(|&i| r.items[i].label.to_lowercase());
        v
    }

    fn current(&self) -> Option<&secret::Item> {
        self.item.and_then(|i| self.rings.get(self.ring)?.items.get(i))
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, TOOL_H, &vgrad(0.0, TOOL_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(150, 150, 156, 255));
        let locked = self.rings.get(self.ring).is_some_and(|r| r.locked);
        let mut toggle = false;
        ui.area(Rect::new(12.0, 10.0, 130.0, 24.0), |ui| {
            toggle = ui.button(if locked { "Destrancar" } else { "Trancar" });
        });
        if toggle {
            if let Some(r) = self.rings.get(self.ring) {
                let res = if locked { self.backend.unlock(&r.path) } else { self.backend.lock(&r.path) };
                if let Err(e) = res {
                    self.sheet = Sheet::Error(e);
                }
                self.reload();
            }
        }
        ui.area(Rect::new(w - 210.0, 11.0, 196.0, 22.0), |ui| {
            ui.search_field(&mut self.search, "Buscar", 196.0);
        });
    }

    fn details(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(236, 236, 238, 255)));
        ui.c.hline(r.x, r.y, r.w, rgba(150, 150, 156, 255));
        let ink = rgba(20, 20, 20, 255);
        let grey = rgba(90, 90, 96, 255);
        let Some(it) = self.current().cloned() else {
            ui.fonts.draw(ui.c, "Escolha um item para ver os detalhes.", r.x + 20.0, r.y + 30.0, 12.0, false, grey);
            return;
        };
        ui.area(Rect::new(r.x + 16.0, r.y + 14.0, 48.0, 48.0), |ui| ui.icon(Icon::Keychain, 48.0));
        let x = r.x + 80.0;
        ui.fonts.draw(ui.c, &ui.fonts.ellipsize(&it.label, 14.0, true, r.w - 100.0), x, r.y + 28.0, 14.0, true, ink);
        let rows = [("Tipo:", it.kind.clone()), ("Conta:", it.account.clone()), ("Onde:", it.place.clone())];
        for (k, (l, v)) in rows.iter().enumerate() {
            let y = r.y + 50.0 + k as f32 * 17.0;
            ui.fonts.draw(ui.c, l, x, y, 11.0, false, grey);
            ui.fonts.draw(ui.c, &ui.fonts.ellipsize(if v.is_empty() { "—" } else { v }, 11.0, false, r.w - 160.0), x + 52.0, y, 11.0, false, ink);
        }
        // Senha: escondida até pedir.
        let y = r.y + 108.0;
        let mut show = self.shown.is_some();
        let mut copy = false;
        let mut delete = false;
        ui.area(Rect::new(x, y, r.w - 100.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut show, "Mostrar senha") {}
                ui.spacing(14.0);
                copy = ui.button("Copiar senha");
                ui.spacing(8.0);
                delete = ui.button("Apagar…");
            });
        });
        if show && self.shown.is_none() {
            match self.backend.secret(&it.path) {
                Ok(s) => self.shown = Some(s),
                Err(e) => self.message = Some(e),
            }
        } else if !show {
            self.shown = None;
        }
        if let Some(s) = &self.shown {
            ui.fonts.draw(ui.c, &ui.fonts.ellipsize(s, 12.0, false, r.w - 120.0), x, y + 40.0, 12.0, false, ink);
        }
        if copy {
            match self.backend.secret(&it.path) {
                Ok(s) => {
                    ui.copy(&s);
                    self.message = Some("Senha copiada.".into());
                }
                Err(e) => self.message = Some(e),
            }
        }
        if delete {
            self.sheet = Sheet::Delete;
        }
    }

    fn sheets(&mut self, ui: &mut Ui) {
        use glove_kit::AlertKind;
        match &mut self.sheet {
            Sheet::None => {}
            Sheet::Error(e) => {
                let e = e.clone();
                if ui.alert(AlertKind::Stop, "Não foi possível concluir.", &e, &["OK"]).is_some() {
                    self.sheet = Sheet::None;
                }
            }
            Sheet::Delete => {
                let name = self.current().map(|i| i.label.clone()).unwrap_or_default();
                match ui.alert(AlertKind::Caution, &format!("Apagar “{name}”?"), "A senha guardada some do chaveiro. Os apps que a usam vão pedir a senha de novo.", &["Cancelar", "Apagar"]) {
                    Some(1) => {
                        if let Some(p) = self.current().map(|i| i.path.clone()) {
                            if let Err(e) = self.backend.delete(&p) {
                                self.sheet = Sheet::Error(e);
                                return;
                            }
                        }
                        self.item = None;
                        self.sheet = Sheet::None;
                        self.reload();
                    }
                    Some(_) => self.sheet = Sheet::None,
                    None => {}
                }
            }
            Sheet::Password(old, new, again, err) => {
                // Folha própria: três campos de senha.
                let (w, h) = (ui.c.w, ui.c.h);
                let (sw, sh) = (420.0f32.min(w - 24.0), 220.0);
                let sx = ((w - sw) / 2.0).round();
                ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
                ui.c.fill(rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]), &solid(ui.theme.body_b));
                ui.c.pinstripes(sx, 0.0, sw, sh - 6.0, ui.theme.body_a, ui.theme.body_b);
                let name = self.rings.get(self.ring).map(|r| r.label.clone()).unwrap_or_default();
                ui.fonts.draw(ui.c, &format!("Mudar a senha do chaveiro “{name}”"), sx + 18.0, 28.0, 13.0, true, rgba(20, 20, 20, 255));
                let mut enter = false;
                for (k, (label, v)) in [("Senha atual:", &mut *old), ("Nova senha:", &mut *new), ("Confirmar:", &mut *again)].into_iter().enumerate() {
                    let y = 48.0 + k as f32 * 32.0;
                    let lw = ui.text_width(label, 12.0, false);
                    ui.fonts.draw(ui.c, label, sx + 120.0 - lw, y + 15.0, 12.0, false, rgba(20, 20, 20, 255));
                    enter |= ui.password_field_in(Rect::new(sx + 128.0, y, sw - 150.0, 22.0), v, false);
                }
                if let Some(e) = err {
                    ui.fonts.draw(ui.c, e, sx + 18.0, 160.0, 11.0, false, rgba(170, 30, 30, 255));
                }
                let (mut cancel, mut ok) = (false, false);
                ui.area(Rect::new(sx + sw - 200.0, sh - 40.0, 184.0, 24.0), |ui| {
                    ui.horizontal(|ui| {
                        cancel = ui.button("Cancelar");
                        ui.spacing(8.0);
                        ok = ui.default_button("Mudar");
                    });
                });
                if cancel || ui.keys().iter().any(|k| k.key == Key::Escape) {
                    self.sheet = Sheet::None;
                } else if ok || enter {
                    if new.is_empty() || new != again {
                        *err = Some("A nova senha e a confirmação não são iguais.".into());
                    } else if let Some(r) = self.rings.get(self.ring) {
                        match self.backend.change_password(&r.path, old, new) {
                            Ok(()) => {
                                self.sheet = Sheet::None;
                                self.message = Some("Senha do chaveiro mudada.".into());
                            }
                            Err(e) => *err = Some(e),
                        }
                    }
                }
            }
        }
    }
}

impl App for Chaves {
    fn ui(&mut self, ui: &mut Ui) {
        let sheet_open = !matches!(self.sheet, Sheet::None);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if !sheet_open {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+W" => acts.push("close".into()),
                    "Ctrl+R" => acts.push("reload".into()),
                    "Ctrl+C" => acts.push("copy".into()),
                    "Ctrl+BackSpace" | "Delete" => acts.push("delete".into()),
                    _ => {}
                }
            }
        }
        for a in acts {
            match a.as_str() {
                "close" => ui.close = true,
                "reload" => self.reload(),
                "delete" if self.item.is_some() => self.sheet = Sheet::Delete,
                "password" => self.sheet = Sheet::Password(String::new(), String::new(), String::new(), None),
                "copy" => {
                    if let Some(p) = self.current().map(|i| i.path.clone()) {
                        if let Ok(s) = self.backend.secret(&p) {
                            ui.copy(&s);
                            self.message = Some("Senha copiada.".into());
                        }
                    }
                }
                _ => {}
            }
        }
        ui.set_enabled(!sheet_open);
        let (w, h) = (ui.c.w, ui.c.h);
        self.toolbar(ui);
        // Chaveiros
        let left = Rect::new(0.0, TOOL_H, LEFT_W, h - TOOL_H);
        ui.c.fill_rect(left.x, left.y, left.w, left.h, &solid(rgba(231, 235, 241, 255)));
        ui.c.vline(LEFT_W, TOOL_H, h - TOOL_H, rgba(150, 150, 156, 255));
        ui.fonts.draw(ui.c, "CHAVEIROS", 12.0, TOOL_H + 20.0, 10.0, true, rgba(110, 116, 128, 255));
        for (i, r) in self.rings.iter().enumerate() {
            let row = Rect::new(0.0, TOOL_H + 28.0 + i as f32 * 26.0, LEFT_W, 26.0);
            if i == self.ring {
                ui.c.fill_rect(row.x, row.y, row.w, row.h, &solid(ui.theme.menu_hot));
            }
            let col = if i == self.ring { Color::WHITE } else { rgba(20, 20, 20, 255) };
            ui.area(Rect::new(10.0, row.y + 3.0, 20.0, 20.0), |ui| ui.icon(Icon::Keychain, 20.0));
            let label = format!("{}{}", r.label, if r.locked { " (trancado)" } else { "" });
            ui.fonts.draw(ui.c, &ui.fonts.ellipsize(&label, 12.0, false, LEFT_W - 44.0), 36.0, row.y + 17.0, 12.0, false, col);
            if ui.hit(row).clicked {
                self.ring = i;
                self.item = None;
                self.shown = None;
            }
        }
        // Tabela das senhas
        let list = Rect::new(LEFT_W + 1.0, TOOL_H, w - LEFT_W - 1.0, h - TOOL_H - DETAIL_H);
        let vis = self.visible();
        let ring = self.rings.get(self.ring);
        let rows: Vec<TableRow> = vis
            .iter()
            .filter_map(|&i| ring.and_then(|r| r.items.get(i)))
            .map(|it| TableRow { cells: vec![it.label.clone(), it.kind.clone(), secret::date(it.modified)], icon: Some(Icon::Keychain), ..TableRow::default() })
            .collect();
        let cols = [Column::new("Nome", 0.5), Column::new("Tipo", 0.28), Column::new("Modificado", 0.22)];
        let sel = self.item.and_then(|i| vis.iter().position(|&v| v == i));
        let resp = ui.table(list, &cols, &rows, sel, &mut self.table, &TableStyle::default());
        if let Some(r) = resp.pressed {
            let new = vis.get(r).copied();
            if new != self.item {
                self.item = new;
                self.shown = None;
            }
        }
        if ring.is_some_and(|r| r.locked) {
            ui.fonts.draw(ui.c, "Este chaveiro está trancado. Clique em Destrancar para ver as senhas.", list.x + 20.0, list.y + 60.0, 12.0, false, rgba(90, 90, 96, 255));
        }
        self.details(ui, Rect::new(LEFT_W + 1.0, h - DETAIL_H, w - LEFT_W - 1.0, DETAIL_H));
        if let Some(m) = &self.message {
            ui.fonts.draw(ui.c, m, LEFT_W + 16.0, h - 10.0, 11.0, false, rgba(90, 90, 96, 255));
        }
        ui.set_enabled(true);
        self.sheets(ui);
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.item.is_some();
        let name = self.rings.get(self.ring).map(|r| r.label.clone()).unwrap_or_default();
        vec![
            Menu::new("Arquivo", vec![MenuItem::new("reload", "Recarregar", "Ctrl+R"), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")]),
            Menu::new(
                "Editar",
                vec![
                    MenuItem::new("copy", "Copiar Senha", "Ctrl+C").enabled(sel),
                    MenuItem::new("delete", "Apagar…", "Ctrl+BackSpace").enabled(sel),
                    MenuItem::Separator,
                    MenuItem::new("password", &format!("Mudar Senha do Chaveiro “{name}”…"), "").enabled(!self.rings.is_empty()),
                ],
            ),
        ]
    }
}

pub fn main() {
    let mut backend = Backend::new();
    // Diagnóstico sem mostrar nada pessoal: só quantos chaveiros e itens o serviço tem.
    if std::env::args().any(|a| a == "--contar") {
        match backend.keyrings() {
            Ok(r) => println!("{} chaveiros, {} itens", r.len(), r.iter().map(|k| k.items.len()).sum::<usize>()),
            Err(e) => println!("erro: {e}"),
        }
        return;
    }
    let mut app = Chaves { backend, rings: Vec::new(), ring: 0, item: None, shown: None, search: String::new(), table: TableState::default(), sheet: Sheet::None, message: None };
    app.reload();
    // O chaveiro "login" primeiro (o de todo dia).
    if let Some(i) = app.rings.iter().position(|r| r.path.ends_with("/login")) {
        app.ring = i;
    }
    let opts = WindowOptions { title: "Acesso às Chaves".into(), app_id: "glove.Chaves".into(), size: (820, 520), fixed: false, min_size: Some((640, 400)) };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-chaves: {e}");
    }
}
