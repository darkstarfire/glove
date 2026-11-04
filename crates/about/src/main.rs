//! "Sobre o Glove".

use glove_kit::{paint::draw::*, run, App, Rect, Ui, WindowOptions};

struct About;

impl App for About {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let accent = ui.theme.accent;
        gel_circle(ui.c, w / 2.0, 70.0, 42.0, accent);
        let center = |ui: &mut Ui, t: &str, y: f32, px: f32, bold: bool, c| {
            let tw = ui.text_width(t, px, bold);
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0, y, px, bold, c);
        };
        center(ui, "Glove", 145.0, 24.0, true, rgba(20, 20, 20, 255));
        center(ui, "Versão 0.2 — prévia", 168.0, 12.0, false, rgba(90, 90, 90, 255));
        center(ui, "Um ambiente gráfico leve,", 196.0, 12.0, false, rgba(50, 50, 50, 255));
        center(ui, "feito em Rust, com visual de gel.", 212.0, 12.0, false, rgba(50, 50, 50, 255));
        ui.area(Rect::new(16.0, 232.0, w - 32.0, 40.0), |ui| {
            ui.separator();
        });
        ui.area(Rect::new(w - 96.0, 246.0, 80.0, 24.0), |ui| {
            if ui.default_button("OK") {
                ui.close = true;
            }
        });
    }
}

fn main() {
    let opts = WindowOptions { title: "Sobre o Glove".into(), app_id: "glove.About".into(), size: (340, 290), fixed: true };
    if let Err(e) = run(opts, About) {
        eprintln!("glove-about: {e}");
    }
}
