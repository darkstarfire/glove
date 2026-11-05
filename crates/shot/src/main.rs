//! Captura: tira uma foto da tela inteira (na hora ou com atraso).
//! A janela fecha antes da foto; o Glove salva em ~/Imagens.

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

struct Shot {
    open: bool,
    delay: usize,
    /// 0 = tela inteira, 1 = uma área.
    what: usize,
}

const DELAYS: [(u64, &str); 3] = [(0, "Agora"), (5000, "Em 5 segundos"), (10000, "Em 10 segundos")];

impl Shot {
    fn take(&mut self, ui: &mut Ui, delay_ms: u64) {
        if self.what == 1 {
            ui.request_screenshot_area(self.open);
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
                "Ctrl+Z" | "Return" => acts.push("shoot".into()),
                "Ctrl+Shift+Z" => acts.push("shoot-5".into()),
                "Ctrl+Shift+A" => acts.push("shoot-area".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "shoot" => self.take(ui, 0),
                "shoot-5" => self.take(ui, 5000),
                "shoot-area" => {
                    self.what = 1;
                    self.take(ui, 0);
                }
                "close" => ui.close = true,
                _ => {}
            }
        }

        icons::draw(ui.c, ui.fonts, Icon::Camera, 20.0, 18.0, 64.0, None);
        ui.fonts.draw(ui.c, "Captura de tela", 100.0, 38.0, 15.0, true, rgba(20, 20, 20, 255));
        ui.fonts.draw(ui.c, "Fotografa a tela inteira ou uma área", 100.0, 58.0, 12.0, false, rgba(70, 70, 70, 255));
        ui.fonts.draw(ui.c, "e salva na sua pasta Imagens.", 100.0, 74.0, 12.0, false, rgba(70, 70, 70, 255));

        ui.area(Rect::new(24.0, 100.0, w - 48.0, 110.0), |ui| {
            ui.horizontal(|ui| {
                ui.label_right("O quê:", 70.0);
                let opts = vec!["Tela inteira".to_string(), "Uma área (arraste)".to_string()];
                ui.popup(&mut self.what, &opts, 180.0);
            });
            ui.space(2.0);
            ui.horizontal(|ui| {
                ui.label_right("Quando:", 70.0);
                let opts: Vec<String> = DELAYS.iter().map(|(_, l)| l.to_string()).collect();
                ui.set_active(self.what == 0);
                ui.popup(&mut self.delay, &opts, 180.0);
                ui.set_active(true);
            });
            ui.space(2.0);
            ui.indent(78.0, |ui| {
                ui.checkbox(&mut self.open, "Abrir no Visualizador");
            });
        });
        let mut go = false;
        ui.area(Rect::new(w - 150.0, 212.0, 130.0, 24.0), |ui| {
            go = ui.default_button("Capturar");
        });
        if go {
            let d = DELAYS[self.delay].0;
            self.take(ui, d);
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            "Captura",
            vec![
                MenuItem::new("shoot", "Tela inteira", "Ctrl+Z"),
                MenuItem::new("shoot-5", "Tela inteira em 5 segundos", "Ctrl+Shift+Z"),
                MenuItem::new("shoot-area", "Uma área…", "Ctrl+Shift+A"),
                MenuItem::Separator,
                MenuItem::new("close", "Fechar", "Ctrl+W"),
            ],
        )]
    }
}

fn main() {
    let opts = WindowOptions { title: "Captura".into(), app_id: "glove.Shot".into(), size: (380, 252), fixed: true };
    if let Err(e) = run(opts, Shot { open: true, delay: 0, what: 0 }) {
        eprintln!("glove-shot: {e}");
    }
}
