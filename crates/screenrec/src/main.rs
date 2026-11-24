//! Gravar a tela (⇧⌘5): o painel escolhe o quê (tela inteira, uma área ou uma janela)
//! e o áudio (do sistema e/ou do microfone). Ao gravar, a janela fecha e o Glove
//! começa; a área ou a janela é escolhida na tela, como na captura.

use glove_config::tr;
use glove_ui::{
    paint::{draw::*, icons::{self, Icon}},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

/// Modo 0 = tela inteira; 1 = uma área; 2 = uma janela (a ordem do menu, a mesma do Glove).
const TELA: usize = 0;

struct Painel {
    modo: usize,
    sistema: bool,
    microfone: bool,
}

impl Painel {
    /// Manda o pedido ao Glove e fecha a janela (ela não pode aparecer no vídeo).
    fn gravar(&mut self, ui: &mut Ui) {
        ui.request_recording(self.modo, self.sistema, self.microfone);
        ui.close = true;
    }
}

impl App for Painel {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Return" => acts.push("gravar".into()),
                "Escape" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "gravar" => self.gravar(ui),
                "close" => ui.close = true,
                _ => {}
            }
        }

        icons::draw(ui.c, ui.fonts, Icon::Video, 20.0, 18.0, 64.0, None);
        ui.fonts.draw(ui.c, tr!("Gravação de tela"), 100.0, 38.0, 15.0, true, rgba(20, 20, 20, 255));
        ui.fonts.draw(ui.c, tr!("Grava a tela, uma área ou uma janela,"), 100.0, 58.0, 12.0, false, rgba(70, 70, 70, 255));
        ui.fonts.draw(ui.c, tr!("salva em Videos e avisa quando terminar."), 100.0, 74.0, 12.0, false, rgba(70, 70, 70, 255));

        ui.area(Rect::new(24.0, 100.0, w - 48.0, 110.0), |ui| {
            ui.horizontal(|ui| {
                ui.label_right(tr!("O quê:"), 70.0);
                let opts = vec![
                    tr!("Tela inteira").to_string(),
                    tr!("Uma área (arraste)").to_string(),
                    tr!("Uma janela (clique)").to_string(),
                ];
                ui.popup(&mut self.modo, &opts, 180.0);
            });
            ui.space(4.0);
            ui.indent(78.0, |ui| {
                ui.checkbox(&mut self.sistema, tr!("Áudio do sistema"));
            });
            ui.space(2.0);
            ui.indent(78.0, |ui| {
                ui.checkbox(&mut self.microfone, tr!("Microfone"));
            });
        });

        let mut cancelar = false;
        let mut gravar = false;
        ui.area(Rect::new(w - 290.0, 212.0, 130.0, 24.0), |ui| {
            cancelar = ui.button(tr!("Cancelar"));
        });
        ui.area(Rect::new(w - 150.0, 212.0, 130.0, 24.0), |ui| {
            gravar = ui.default_button(tr!("Gravar"));
        });
        if cancelar {
            ui.close = true;
        }
        if gravar {
            self.gravar(ui);
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            tr!("Gravação"),
            vec![
                MenuItem::new("gravar", tr!("Gravar"), "Return"),
                MenuItem::Separator,
                MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
            ],
        )]
    }
}

pub fn main() {
    let opts = WindowOptions {
        title: tr!("Gravação de tela").into(),
        app_id: "glove.ScreenRec".into(),
        size: (380, 252),
        fixed: true,
        min_size: None,
    };
    let painel = Painel { modo: TELA, sistema: false, microfone: false };
    if let Err(e) = run(opts, painel) {
        eprintln!("glove-screenrec: {e}");
    }
}
