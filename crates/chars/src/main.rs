//! Caracteres: mapa de caracteres por categoria; clique para ver, copie com um botão.
//! Também mostra o teclado na tela e, na visão Fonte, os caracteres de uma fonte.

use glove_config::tr;
mod fonte;

use glove_kit::{
    paint::{draw::*, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions, SCROLLBAR_W,
};

/// Teclado ABNT2 (sem e com Shift), fileira por fileira.
const BR: [(&str, &str); 4] = [
    ("'1234567890-=", "\"!@#$%¨&*()_+"),
    ("qwertyuiop´[", "QWERTYUIOP`{"),
    ("asdfghjklç~]", "ASDFGHJKLÇ^}"),
    ("\\zxcvbnm,.;/", "|ZXCVBNM<>:?"),
];
/// Teclado americano.
const US: [(&str, &str); 4] = [
    ("`1234567890-=", "~!@#$%^&*()_+"),
    ("qwertyuiop[]\\", "QWERTYUIOP{}|"),
    ("asdfghjkl;'", "ASDFGHJKL:\""),
    ("zxcvbnm,./", "ZXCVBNM<>?"),
];

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
/// Visões, no segmentado do alto.
const MODES: [&str; 3] = ["Caracteres", "Teclado", "Fonte"];
const SIDE: f32 = 190.0;

struct Chars {
    block: Option<usize>,
    selected: Option<char>,
    scroll: f32,
    copied: Option<char>,
    /// 0 = mapa de caracteres, 1 = teclado, 2 = uma fonte.
    mode: usize,
    font: fonte::FontView,
    shift: bool,
    /// Texto montado clicando nas teclas.
    text: String,
    /// Tecla acesa (digitada no teclado de verdade) e até quando.
    lit: Option<(char, std::time::Instant)>,
}

impl Chars {
    /// Teclado na tela (como o Visualizador de Teclado da era): o layout em uso,
    /// Shift, e cada clique vai para o texto embaixo.
    fn keyboard(&mut self, ui: &mut Ui, r: Rect) {
        let layout = glove_config::Config::load().keyboard.layout;
        let rows = if layout == "br" { BR } else { US };
        // O que foi digitado de verdade acende a tecla.
        if let Some(ch) = ui.typed().chars().last() {
            self.lit = Some((ch, std::time::Instant::now() + std::time::Duration::from_millis(350)));
        }
        let lit = self.lit.filter(|(_, t)| std::time::Instant::now() < *t).map(|(c, _)| c);
        if lit.is_some() {
            ui.wake_after(std::time::Duration::from_millis(120));
        }
        ui.c.fill(rrect(r.x, r.y, r.w, r.h, 8.0), &vgrad(r.y, r.y + r.h, &[(0.0, rgba(214, 216, 222, 255)), (1.0, rgba(180, 184, 192, 255))]));
        ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 8.0), &solid(rgba(110, 112, 120, 255)), 1.0);
        let key = ((r.w - 24.0) / 14.0).floor();
        let kh = key.min((r.h - 24.0) / 5.0);
        let mut typed = None;
        for (ri, (normal, shifted)) in rows.iter().enumerate() {
            let keys: Vec<char> = if self.shift { shifted.chars().collect() } else { normal.chars().collect() };
            let indent = [0.0, 0.5, 0.75, 1.25][ri] * key;
            for (ki, ch) in keys.iter().enumerate() {
                let kr = Rect::new(r.x + 12.0 + indent + ki as f32 * key, r.y + 12.0 + ri as f32 * kh, key - 4.0, kh - 4.0);
                let on = lit == Some(*ch) || lit.is_some_and(|l| l.to_lowercase().eq(ch.to_lowercase()));
                if ui.key_button(kr, &ch.to_string(), 14.0, on) {
                    typed = Some(*ch);
                }
            }
        }
        // Shift e espaço.
        let y5 = r.y + 12.0 + 4.0 * kh;
        if ui.key_button(Rect::new(r.x + 12.0, y5, key * 2.5 - 4.0, kh - 4.0), "Shift", 12.0, self.shift) {
            self.shift = !self.shift;
        }
        if ui.key_button(Rect::new(r.x + 12.0 + key * 3.0, y5, key * 7.0 - 4.0, kh - 4.0), tr!("espaço"), 12.0, lit == Some(' ')) {
            typed = Some(' ');
        }
        if ui.key_button(Rect::new(r.x + 12.0 + key * 10.5, y5, key * 2.5 - 4.0, kh - 4.0), "⌫", 14.0, false) {
            self.text.pop();
        }
        if let Some(c) = typed {
            self.text.push(c);
            // Shift vale para uma tecla só (como no teclado de verdade).
            self.shift = false;
        }
    }
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
            if let Some(c) = if self.mode == 2 { self.font.selected } else { self.selected } {
                ui.copy(&c.to_string());
                self.copied = Some(c);
            }
        }

        // Mapa ou teclado (segmentado no alto da lateral).
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "mode-map" => self.mode = 0,
                "mode-kbd" => self.mode = 1,
                "mode-font" => self.mode = 2,
                "install-font" => {
                    self.mode = 2;
                    self.font.ask_install();
                }
                _ => {}
            }
        }
        if ui.keys().iter().any(|k| k.combo == "Ctrl+Shift+I") {
            self.mode = 2;
            self.font.ask_install();
        }
        if self.mode == 2 {
            let mut m = self.mode;
            ui.area(Rect::new(10.0, 10.0, 300.0, 24.0), |ui| {
                if let Some(i) = ui.segmented_text(&MODES.map(tr), Some(m)) {
                    m = i;
                }
            });
            self.mode = m;
            self.font.ui(ui, Rect::new(10.0, 44.0, w - 20.0, h - 54.0));
            self.font.sheet(ui);
            return;
        }
        if self.mode == 1 {
            let mut m = self.mode;
            ui.area(Rect::new(10.0, 10.0, 300.0, 24.0), |ui| {
                if let Some(i) = ui.segmented_text(&MODES.map(tr), Some(m)) {
                    m = i;
                }
            });
            self.mode = m;
            let kr = Rect::new(10.0, 44.0, w - 20.0, h - 44.0 - 70.0);
            self.keyboard(ui, kr);
            // Texto montado, Copiar e Limpar.
            let ty = h - 58.0;
            ui.fonts.draw(ui.c, tr!("Texto:"), 12.0, ty + 15.0, 12.0, false, rgba(40, 40, 44, 255));
            ui.text_field_in(Rect::new(60.0, ty, w - 270.0, 22.0), &mut self.text, false);
            let (mut copy_t, mut clear) = (false, false);
            ui.area(Rect::new(w - 200.0, ty - 2.0, 190.0, 26.0), |ui| {
                ui.horizontal(|ui| {
                    clear = ui.button(tr!("Limpar"));
                    copy_t = ui.default_button(tr!("Copiar"));
                });
            });
            if clear {
                self.text.clear();
            }
            if copy_t && !self.text.is_empty() {
                ui.copy(&self.text);
            }
            return;
        }
        let mut m = self.mode;
        ui.area(Rect::new(10.0, 10.0, 300.0, 24.0), |ui| {
            if let Some(i) = ui.segmented_text(&MODES.map(tr), Some(m)) {
                m = i;
            }
        });
        self.mode = m;
        if m != 0 {
            ui.redraw = true;
            return;
        }

        // Categorias
        let names: Vec<String> = BLOCKS.iter().map(|b| tr(b.0).to_string()).collect();
        let before = self.block;
        ui.area(Rect::new(10.0, 44.0, SIDE, h - 54.0), |ui| {
            ui.list(&mut self.block, &names, SIDE, h - 54.0);
        });
        if self.block != before {
            self.scroll = 0.0;
            self.selected = None;
        }

        // Grade
        let full = Rect::new(SIDE + 20.0, 44.0, w - SIDE - 30.0, h - 154.0);
        let chars = self.chars();
        let per_row = ((full.w - SCROLLBAR_W) / CELL).floor().max(1.0) as usize;
        let rows = chars.len().div_ceil(per_row);
        let grid = ui.scroll_area(full, &mut self.scroll, rows as f32 * CELL, CELL);
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
        ui.c.stroke(rrect(full.x - 0.5, full.y - 0.5, full.w + 1.0, full.h + 1.0, 0.0), &solid(rgba(130, 130, 136, 255)), 1.0);

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
            let msg = if self.copied == Some(c) { tr!("Copiado para a área de transferência.") } else { tr!("Clique duplo também copia.") };
            ui.fonts.draw(ui.c, msg, pv.x + 100.0, py + 44.0, 11.0, false, rgba(90, 90, 90, 255));
            let mut go = false;
            ui.area(Rect::new(pv.x + 100.0, py + 56.0, 120.0, 24.0), |ui| go = ui.default_button(tr!("Copiar")));
            if go {
                ui.copy(&s);
                self.copied = Some(c);
            }
        } else {
            ui.fonts.draw(ui.c, tr!("Escolha um caractere."), pv.x + 100.0, py + 46.0, 12.0, false, rgba(110, 110, 110, 255));
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new(tr!("Arquivo"), vec![MenuItem::new("install-font", tr!("Instalar Fonte…"), "Ctrl+Shift+I"), MenuItem::Separator, MenuItem::new("close", tr!("Fechar"), "Ctrl+W")]),
            Menu::new(tr!("Editar"), vec![MenuItem::new("copy", tr!("Copiar caractere"), "Ctrl+C").enabled(if self.mode == 2 { self.font.selected.is_some() } else { self.selected.is_some() })]),
            Menu::new(
                tr!("Visualizar"),
                vec![
                    MenuItem::new("mode-map", "Caracteres", "").checked(self.mode == 0),
                    MenuItem::new("mode-kbd", "Teclado", "").checked(self.mode == 1),
                    MenuItem::new("mode-font", "Fonte", "").checked(self.mode == 2),
                ],
            ),
        ]
    }
}

pub fn main() {
    let opts = WindowOptions { title: "Caracteres".into(), app_id: "glove.Chars".into(), size: (760, 470), fixed: false, min_size: Some((600, 380)) };
    if let Err(e) = run(opts, Chars { block: Some(1), selected: None, scroll: 0.0, copied: None, mode: 0, font: fonte::FontView::default(), shift: false, text: String::new(), lit: None }) {
        eprintln!("glove-chars: {e}");
    }
}
