//! Captura: fotografa uma área, uma janela ou a tela inteira (na hora ou com
//! atraso). A janela fecha antes da foto; o Glove salva em ~/Imagens, põe a
//! imagem na área de transferência e a abre no Visualizador para desenhar por cima.

use glove_config::tr;
use glove_ui::{
    paint::{draw::*, icons::{self, Icon}},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

struct Shot {
    open: bool,
    delay: usize,
    /// 0 = uma área, 1 = uma janela, 2 = tela inteira (a ordem do menu).
    what: usize,
}

const AREA: usize = 0;
const WINDOW: usize = 1;
const SCREEN: usize = 2;

const DELAYS: [(u64, &str); 3] = [(0, "Agora"), (5000, "Em 5 segundos"), (10000, "Em 10 segundos")];

impl Shot {
    fn take(&mut self, ui: &mut Ui, delay_ms: u64) {
        if self.what == AREA {
            ui.request_screenshot_area(self.open);
        } else if self.what == WINDOW {
            ui.request_screenshot_window(self.open);
        } else {
            ui.request_screenshot(delay_ms.max(400), self.open);
        }
        ui.close = true;
    }
}

impl App for Shot {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                // Enter faz o que está escolhido na janela (por padrão, uma área).
                "Return" => acts.push("go".into()),
                "Ctrl+Z" => acts.push("shoot".into()),
                "Ctrl+Shift+Z" => acts.push("shoot-5".into()),
                "Ctrl+Shift+A" => acts.push("shoot-area".into()),
                "Ctrl+Shift+J" => acts.push("shoot-window".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "go" => {
                    let d = DELAYS[self.delay].0;
                    self.take(ui, d);
                }
                "shoot" => {
                    self.what = SCREEN;
                    self.take(ui, 0);
                }
                "shoot-5" => {
                    self.what = SCREEN;
                    self.take(ui, 5000);
                }
                "shoot-area" => {
                    self.what = AREA;
                    self.take(ui, 0);
                }
                "shoot-window" => {
                    self.what = WINDOW;
                    self.take(ui, 0);
                }
                "close" => ui.close = true,
                _ => {}
            }
        }

        icons::draw(ui.c, ui.fonts, Icon::Camera, 20.0, 18.0, 64.0, None);
        ui.fonts.draw(ui.c, tr!("Captura de tela"), 100.0, 38.0, 15.0, true, rgba(20, 20, 20, 255));
        ui.fonts.draw(ui.c, tr!("Fotografa uma área, uma janela ou a tela,"), 100.0, 58.0, 12.0, false, rgba(70, 70, 70, 255));
        ui.fonts.draw(ui.c, tr!("salva em Imagens e já copia (Ctrl+V cola)."), 100.0, 74.0, 12.0, false, rgba(70, 70, 70, 255));

        ui.area(Rect::new(24.0, 100.0, w - 48.0, 110.0), |ui| {
            ui.horizontal(|ui| {
                ui.label_right(tr!("O quê:"), 70.0);
                let opts = vec![tr!("Uma área (arraste)").to_string(), tr!("Uma janela (clique)").to_string(), tr!("Tela inteira").to_string()];
                ui.popup(&mut self.what, &opts, 180.0);
            });
            ui.space(2.0);
            ui.horizontal(|ui| {
                ui.label_right(tr!("Quando:"), 70.0);
                let opts: Vec<String> = DELAYS.iter().map(|(_, l)| tr(l).to_string()).collect();
                ui.set_active(self.what == SCREEN);
                ui.popup(&mut self.delay, &opts, 180.0);
                ui.set_active(true);
            });
            ui.space(2.0);
            ui.indent(78.0, |ui| {
                ui.checkbox(&mut self.open, tr!("Abrir no Visualizador para desenhar"));
            });
        });
        let mut go = false;
        ui.area(Rect::new(w - 150.0, 212.0, 130.0, 24.0), |ui| {
            go = ui.default_button(tr!("Capturar"));
        });
        if go {
            let d = DELAYS[self.delay].0;
            self.take(ui, d);
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            tr!("Captura"),
            vec![
                MenuItem::new("shoot-area", tr!("Uma área…"), "Ctrl+Shift+A"),
                MenuItem::new("shoot-window", tr!("Uma janela…"), "Ctrl+Shift+J"),
                MenuItem::new("shoot", tr!("Tela inteira"), "Ctrl+Z"),
                MenuItem::new("shoot-5", tr!("Tela inteira em 5 segundos"), "Ctrl+Shift+Z"),
                MenuItem::Separator,
                MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
            ],
        )]
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Captura").into(), app_id: "glove.Shot".into(), size: (380, 252), fixed: true, min_size: None };
    if let Err(e) = run(opts, Shot { open: true, delay: 0, what: AREA }) {
        eprintln!("glove-shot: {e}");
    }
}
