//! Caracteres: mapa de caracteres por categoria; clique para ver, copie com um botão.

use glove_kit::{
    paint::{draw::*, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const BLOCKS: [(&str, u32, u32); 15] = [
    ("Latim básico", 0x21, 0x7E),
    ("Acentos e latim estendido", 0xA1, 0x17F),
    ("Grego", 0x391, 0x3C9),
    ("Cirílico", 0x410, 0x44F),
    ("Pontuação", 0x2010, 0x205E),
    ("Moedas", 0x20A0, 0x20C0),
    ("Letras e números especiais", 0x2100, 0x218B),
    ("Setas", 0x2190, 0x21FF),
    ("Matemática", 0x2200, 0x22FF),
    ("Técnicos", 0x2300, 0x23FF),
    ("Linhas e blocos", 0x2500, 0x259F),
    ("Formas geométricas", 0x25A0, 0x25FF),
    ("Símbolos diversos", 0x2600, 0x26FF),
    ("Dingbats", 0x2700, 0x27BF),
    ("Ícones (Nerd Font)", 0xF000, 0xF2E0),
];

const CELL: f32 = 34.0;
const SIDE: f32 = 190.0;

struct Chars {
    block: Option<usize>,
    selected: Option<char>,
    scroll: f32,
    copied: Option<char>,
}

impl Chars {
    fn chars(&self) -> Vec<char> {
        let (_, a, b) = BLOCKS[self.block.unwrap_or(0)];
        (a..=b).filter_map(char::from_u32).filter(|c| !c.is_control()).collect()
    }
}

impl App for Chars {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let copy = ui.keys().iter().any(|k| k.combo == "Ctrl+C") || ui.menu_actions().iter().any(|a| a == "copy");
        if ui.keys().iter().any(|k| k.combo == "Ctrl+W") || ui.menu_actions().iter().any(|a| a == "close") {
            ui.close = true;
        }
        if copy {
            if let Some(c) = self.selected {
                ui.copy(&c.to_string());
                self.copied = Some(c);
            }
        }

        // Categorias
        let names: Vec<String> = BLOCKS.iter().map(|b| b.0.to_string()).collect();
        let before = self.block;
        ui.area(Rect::new(10.0, 10.0, SIDE, h - 20.0), |ui| {
            ui.list(&mut self.block, &names, SIDE, h - 20.0);
        });
        if self.block != before {
            self.scroll = 0.0;
            self.selected = None;
        }

        // Grade
        let grid = Rect::new(SIDE + 20.0, 10.0, w - SIDE - 30.0, h - 120.0);
        let chars = self.chars();
        let per_row = (grid.w / CELL).floor().max(1.0) as usize;
        let rows = chars.len().div_ceil(per_row);
        self.scroll = (self.scroll + ui.scroll_in(grid)).clamp(0.0, (rows as f32 * CELL - grid.h).max(0.0));
        let first = (self.scroll / CELL).floor() as usize;
        let visible = (grid.h / CELL).ceil() as usize + 1;
        for (i, ch) in chars.iter().enumerate().skip(first * per_row).take(visible * per_row) {
            let (col, row) = (i % per_row, i / per_row);
            let y = grid.y + row as f32 * CELL - self.scroll;
            let (top, bot) = (y.max(grid.y), (y + CELL).min(grid.y + grid.h));
            if bot > top {
                let r = ui.hit(Rect::new(grid.x + col as f32 * CELL, top, CELL, bot - top));
                if r.pressed {
                    self.selected = Some(*ch);
                    self.copied = None;
                    if r.double {
                        ui.copy(&ch.to_string());
                        self.copied = Some(*ch);
                    }
                }
            }
        }
        let (sel, off, accent) = (self.selected, self.scroll, ui.theme.accent);
        ui.clipped(3, grid, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, ch) in chars.iter().enumerate().skip(first * per_row).take(visible * per_row) {
                let (col, row) = (i % per_row, i / per_row);
                let (x, y) = (col as f32 * CELL, row as f32 * CELL - off);
                if sel == Some(*ch) {
                    c.fill_rect(x, y, CELL, CELL, &vgrad(y, y + CELL, &[(0.0, lighten(accent, 0.2)), (1.0, accent)]));
                }
                let s = ch.to_string();
                let tw = fonts.measure(&s, 18.0, false);
                fonts.draw(c, &s, x + (CELL - tw) / 2.0, y + 24.0, 18.0, false, if sel == Some(*ch) { Color::WHITE } else { rgba(20, 20, 20, 255) });
                c.hline(x, y + CELL - 1.0, CELL, rgba(225, 225, 230, 255));
                c.vline(x + CELL - 1.0, y, CELL, rgba(225, 225, 230, 255));
            }
        });
        ui.c.stroke(rrect(grid.x - 0.5, grid.y - 0.5, grid.w + 1.0, grid.h + 1.0, 0.0), &solid(rgba(130, 130, 136, 255)), 1.0);

        // Prévia
        let py = h - 100.0;
        let pv = Rect::new(SIDE + 20.0, py, 84.0, 84.0);
        ui.c.fill(rrect(pv.x, pv.y, pv.w, pv.h, 6.0), &solid(Color::WHITE));
        ui.c.stroke(rrect(pv.x, pv.y, pv.w, pv.h, 6.0), &solid(rgba(130, 130, 136, 255)), 1.0);
        if let Some(c) = self.selected {
            let s = c.to_string();
            let tw = ui.text_width(&s, 52.0, false);
            ui.fonts.draw(ui.c, &s, pv.x + (pv.w - tw) / 2.0, pv.y + 62.0, 52.0, false, rgba(10, 10, 10, 255));
            let code = format!("U+{:04X}", c as u32);
            ui.fonts.draw(ui.c, &code, pv.x + 100.0, py + 22.0, 15.0, true, rgba(20, 20, 20, 255));
            let msg = if self.copied == Some(c) { "Copiado para a área de transferência." } else { "Clique duplo também copia." };
            ui.fonts.draw(ui.c, msg, pv.x + 100.0, py + 44.0, 11.0, false, rgba(90, 90, 90, 255));
            let mut go = false;
            ui.area(Rect::new(pv.x + 100.0, py + 56.0, 120.0, 24.0), |ui| go = ui.default_button("Copiar"));
            if go {
                ui.copy(&s);
                self.copied = Some(c);
            }
        } else {
            ui.fonts.draw(ui.c, "Escolha um caractere.", pv.x + 100.0, py + 46.0, 12.0, false, rgba(110, 110, 110, 255));
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new("Arquivo", vec![MenuItem::new("close", "Fechar", "Ctrl+W")]),
            Menu::new("Editar", vec![MenuItem::new("copy", "Copiar caractere", "Ctrl+C").enabled(self.selected.is_some())]),
        ]
    }
}

fn main() {
    let opts = WindowOptions { title: "Caracteres".into(), app_id: "glove.Chars".into(), size: (640, 440), fixed: false };
    if let Err(e) = run(opts, Chars { block: Some(1), selected: None, scroll: 0.0, copied: None }) {
        eprintln!("glove-chars: {e}");
    }
}
