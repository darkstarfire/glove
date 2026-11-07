//! Painel Itens de Início: apps que abrem sozinhos ao entrar (autostart do XDG).

use glove_config::tr;
use glove_config::inicio::{self, Item};
use glove_kit::{
    paint::{draw::*, Color},
    Rect, Ui,
};

#[derive(Default)]
pub struct Inicio {
    items: Vec<Item>,
    sel: Option<usize>,
    /// Folha "+" aberta: (apps instalados, escolhido).
    picker: Option<(Vec<(String, String)>, Option<usize>)>,
    message: Option<String>,
}

impl Inicio {
    pub fn refresh(&mut self) {
        self.items = inicio::items();
        if self.sel.is_some_and(|s| s >= self.items.len()) {
            self.sel = None;
        }
    }

    pub fn sheet_open(&self) -> bool {
        self.picker.is_some()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.label(tr!("Estes itens abrem sozinhos quando você entra no Glove:"));
        ui.space(6.0);
        let w = ui.available_width();
        // A lista cresce com os itens (a janela acompanha), até 16 linhas.
        let (row_h, head_h) = (22.0, 20.0);
        let list_h = head_h + 2.0 + self.items.len().clamp(6, 16) as f32 * row_h;
        let r = ui.alloc(w, list_h);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        // Cabeçalho das colunas.
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, head_h, &vgrad(r.y, r.y + head_h, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(r.x + 1.0, r.y + head_h, r.w - 2.0, rgba(170, 170, 176, 255));
        let ink = rgba(20, 20, 20, 255);
        ui.fonts.draw(ui.c, tr!("Abrir"), r.x + 8.0, r.y + 14.0, 11.0, false, ink);
        ui.fonts.draw(ui.c, tr!("Item"), r.x + 56.0, r.y + 14.0, 11.0, false, ink);
        ui.fonts.draw(ui.c, tr!("Comando"), r.x + r.w * 0.5, r.y + 14.0, 11.0, false, ink);
        if self.items.is_empty() {
            ui.fonts.draw(ui.c, tr!("Nenhum item. Use + para escolher um aplicativo."), r.x + 12.0, r.y + head_h + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let mut toggle = None;
        for (k, it) in self.items.iter().enumerate().take(((list_h - head_h - 2.0) / row_h) as usize) {
            let y = r.y + head_h + 1.0 + k as f32 * row_h;
            let rr = Rect::new(r.x + 1.0, y, r.w - 2.0, row_h);
            let sel = self.sel == Some(k);
            if sel {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(ui.theme.accent));
            } else if k % 2 == 1 {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(rgba(237, 243, 254, 255)));
            }
            let t = if sel { Color::WHITE } else { ink };
            // Caixinha de "abrir".
            let cb = Rect::new(rr.x + 14.0, y + 4.0, 14.0, 14.0);
            ui.c.fill(rrect(cb.x, cb.y, cb.w, cb.h, 3.0), &solid(if it.enabled { ui.theme.accent } else { Color::WHITE }));
            ui.c.stroke(rrect(cb.x + 0.5, cb.y + 0.5, cb.w - 1.0, cb.h - 1.0, 3.0), &solid(rgba(90, 90, 96, 255)), 1.0);
            if it.enabled {
                ui.fonts.draw(ui.c, "✓", cb.x + 2.0, cb.y + 12.0, 12.0, true, Color::WHITE);
            }
            let name = if it.name.is_empty() { it.id.clone() } else { it.name.clone() };
            let name = ui.fonts.ellipsize(&name, 12.0, false, r.w * 0.5 - 60.0);
            ui.fonts.draw(ui.c, &name, rr.x + 55.0, y + 15.0, 12.0, false, t);
            let cmd = ui.fonts.ellipsize(&it.exec, 11.0, false, r.w * 0.5 - 12.0);
            ui.fonts.draw(ui.c, &cmd, r.x + r.w * 0.5, y + 15.0, 11.0, false, if sel { Color::WHITE } else { rgba(90, 90, 96, 255) });
            // Um clique só por linha: na coluna "Abrir" liga/desliga; no resto, escolhe.
            if ui.hit(rr).clicked {
                if ui.pointer().0 < rr.x + 44.0 {
                    toggle = Some(k);
                } else {
                    self.sel = Some(k);
                }
            }
        }
        if let Some(k) = toggle {
            let it = self.items[k].clone();
            if let Err(e) = inicio::set_enabled(&it.id, !it.enabled) {
                self.message = Some(tr!("Não foi possível mudar “{}”: {}", it.name, e));
            }
            self.refresh();
        }
        ui.space(4.0);
        let (mut plus, mut minus) = (false, false);
        ui.horizontal(|ui| {
            ui.spacing(4.0);
            plus = ui.small_button("+");
            ui.set_active(self.sel.is_some());
            minus = ui.small_button("−");
            ui.set_active(true);
        });
        if plus {
            self.picker = Some((inicio::installed_apps(), None));
        }
        if minus {
            if let Some(it) = self.sel.and_then(|k| self.items.get(k)).cloned() {
                if let Err(e) = inicio::remove(&it) {
                    self.message = Some(tr!("Não foi possível tirar “{}”: {}", it.name, e));
                }
                self.sel = None;
                self.refresh();
            }
        }
        ui.space(4.0);
        ui.small(tr!("Itens do sistema não são apagados: tirar só faz com que não abram. Vale na próxima entrada."));
        if let Some(m) = &self.message {
            let m = m.clone();
            ui.small(&m);
        }
    }

    /// Folha com os apps instalados para escolher um.
    pub fn picker_sheet(&mut self, ui: &mut Ui) {
        let Some((apps, mut sel)) = self.picker.take() else { return };
        let (sw, sh) = (420.0, 330.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = 78.0;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        ui.fonts.draw(ui.c, tr!("Escolha um aplicativo para abrir ao entrar:"), x + 20.0, y + 26.0, 13.0, true, rgba(20, 20, 20, 255));
        let rows: Vec<String> = apps.iter().map(|a| a.0.clone()).collect();
        let (mut add, mut cancel) = (false, false);
        ui.area(Rect::new(x + 20.0, y + 40.0, sw - 40.0, sh - 50.0), |ui| {
            ui.list(&mut sel, &rows, sw - 40.0, 230.0);
            ui.space(8.0);
            ui.horizontal(|ui| {
                ui.space(sw - 40.0 - 200.0);
                cancel = ui.button(tr!("Cancelar"));
                ui.set_active(sel.is_some());
                add |= ui.default_button(tr!("Adicionar"));
                ui.set_active(true);
            });
        });
        let esc = ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Escape);
        if cancel || esc {
            return;
        }
        if add {
            if let Some((name, exec)) = sel.and_then(|i| apps.get(i)) {
                if let Err(e) = inicio::add(name, exec) {
                    self.message = Some(tr!("Não foi possível pôr “{}”: {}", name, e));
                }
                self.refresh();
                self.sel = self.items.iter().position(|i| i.name == *name);
            }
            return;
        }
        self.picker = Some((apps, sel));
    }
}
