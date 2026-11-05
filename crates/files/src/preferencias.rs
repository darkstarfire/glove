//! Preferências do Arquivos (Arquivos ▸ Preferências…, Ctrl+,): Geral, Rótulos,
//! Barra Lateral e Avançado. E as Opções de Visualização das janelas (Ctrl+J).
//! Tudo vai para o config.toml ([files] e [desk]); as janelas abertas e a
//! mesa mudam na hora.

use glove_kit::{paint::draw::*, run, ui::Ticks, App, Rect, Ui, WindowOptions};

/// Lugares da barra lateral: (id, nome).
const PLACES: [(&str, &str); 10] = [
    ("disco", "Disco"),
    ("mesa", "Mesa"),
    ("inicio", "Início"),
    ("aplicativos", "Aplicativos"),
    ("documentos", "Documentos"),
    ("downloads", "Downloads"),
    ("imagens", "Imagens"),
    ("musica", "Música"),
    ("videos", "Vídeos"),
    ("buscas", "Pastas inteligentes"),
];
const NEW_WINDOW: [(&str, &str); 4] = [("inicio", "Início"), ("mesa", "Mesa"), ("documentos", "Documentos"), ("computador", "Computador")];
const ARRANGE: [(&str, &str); 4] = [("nome", "Nome"), ("data", "Data de Modificação"), ("tamanho", "Tamanho"), ("tipo", "Tipo")];
/// Fundos da vista de ícones: (valor, nome).
const BACKGROUNDS: [(&str, &str); 6] = [("", "Branco"), ("#e8eef8", "Azul-claro"), ("#eef4e6", "Verde-claro"), ("#f6efe2", "Bege"), ("#d8d8dc", "Cinza"), ("#2e3440", "Grafite")];

pub fn preferences() {
    let opts = WindowOptions { title: "Preferências do Arquivos".into(), app_id: "glove.Files".into(), size: (420, 380), fixed: true, min_size: None };
    if let Err(e) = run(opts, Prefs { config: glove_config::Config::load(), tab: 0 }) {
        eprintln!("glove-files: {e}");
    }
}

struct Prefs {
    config: glove_config::Config,
    tab: usize,
}

impl App for Prefs {
    fn config_changed(&mut self) {
        self.config = glove_config::Config::load();
    }

    fn ui(&mut self, ui: &mut Ui) {
        let mut save = false;
        ui.area(Rect::new(16.0, 14.0, ui.c.w - 32.0, ui.c.h - 28.0), |ui| {
            let mut tab = self.tab;
            ui.tab_box(&mut tab, &["Geral", "Rótulos", "Barra Lateral", "Avançado"], ui.c.h - 40.0, |ui| {
                ui.indent(18.0, |ui| {
                    ui.space(14.0);
                    match self.tab {
                        0 => {
                            ui.label("Mostrar estes itens na mesa:");
                            ui.space(4.0);
                            let d = &mut self.config.desk;
                            save |= ui.checkbox(&mut d.show_disks, "Disco do sistema");
                            save |= ui.checkbox(&mut d.show_removable, "Pendrives e discos externos");
                            ui.space(14.0);
                            ui.label("Novas janelas do Arquivos abrem em:");
                            ui.space(4.0);
                            let f = &mut self.config.files;
                            let mut i = NEW_WINDOW.iter().position(|n| n.0 == f.new_window).unwrap_or(0);
                            if ui.popup(&mut i, &NEW_WINDOW.iter().map(|n| n.1.to_string()).collect::<Vec<_>>(), 180.0) {
                                f.new_window = NEW_WINDOW[i].0.into();
                                save = true;
                            }
                        }
                        1 => {
                            ui.label("Nomes dos rótulos:");
                            ui.space(6.0);
                            let f = &mut self.config.files;
                            while f.labels.len() < 7 {
                                f.labels.push(glove_config::arquivos::DEFAULT_LABELS[f.labels.len()].into());
                            }
                            for (k, (r, g, b)) in glove_config::arquivos::LABEL_COLORS.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let sw = ui.alloc(18.0, 22.0);
                                    ui.c.fill(circle(sw.x + 8.0, sw.y + 11.0, 7.0), &solid(rgba(*r, *g, *b, 255)));
                                    ui.c.stroke(circle(sw.x + 8.0, sw.y + 11.0, 7.0), &solid(rgba(0, 0, 0, 90)), 1.0);
                                    ui.spacing(6.0);
                                    if ui.text_field(&mut f.labels[k], 200.0) {
                                        save = true;
                                    }
                                });
                                ui.space(2.0);
                            }
                        }
                        2 => {
                            ui.label("Mostrar na barra lateral:");
                            ui.space(4.0);
                            let f = &mut self.config.files;
                            for (id, name) in PLACES {
                                let mut on = !f.sidebar_hidden.iter().any(|h| h == id);
                                if ui.checkbox(&mut on, name) {
                                    f.sidebar_hidden.retain(|h| h != id);
                                    if !on {
                                        f.sidebar_hidden.push(id.into());
                                    }
                                    save = true;
                                }
                            }
                        }
                        _ => {
                            let f = &mut self.config.files;
                            save |= ui.checkbox(&mut f.show_extensions, "Mostrar todas as extensões dos arquivos");
                            ui.space(4.0);
                            save |= ui.checkbox(&mut f.warn_empty_trash, "Avisar antes de esvaziar o Lixo");
                        }
                    }
                });
            });
            self.tab = tab;
        });
        if save {
            let _ = self.config.save();
        }
    }
}

pub fn view_options() {
    let opts = WindowOptions { title: "Opções de Visualização".into(), app_id: "glove.Files".into(), size: (300, 360), fixed: true, min_size: None };
    if let Err(e) = run(opts, ViewOptions { config: glove_config::Config::load(), pending: false }) {
        eprintln!("glove-files: {e}");
    }
}

struct ViewOptions {
    config: glove_config::Config,
    /// Mudou algo arrastando o slider: grava ao soltar.
    pending: bool,
}

impl App for ViewOptions {
    fn config_changed(&mut self) {
        if !self.pending {
            self.config = glove_config::Config::load();
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        if self.pending && !ui.mouse_down() {
            self.pending = false;
            let _ = self.config.save();
        }
        let mut save = false;
        let mut drag = false;
        ui.area(Rect::new(20.0, 16.0, ui.c.w - 40.0, ui.c.h - 30.0), |ui| {
            ui.small("Vale para a vista de ícones de todas as janelas.");
            ui.space(8.0);
            let f = &mut self.config.files;
            ui.label(&format!("Tamanho dos ícones: {0} × {0}", f.icon_size));
            ui.space(4.0);
            let mut size = f.icon_size as f32;
            let t = Ticks { count: 7, labels: &[(0.0, "Pequeno"), (1.0, "Grande")], snap: false };
            if ui.slider_ticks(&mut size, 32.0, 128.0, 250.0, &t, true) {
                f.icon_size = (size / 4.0).round() as u32 * 4;
                drag = true;
            }
            ui.space(10.0);
            ui.horizontal(|ui| {
                ui.label("Tamanho do texto:");
                ui.spacing(8.0);
                let sizes: Vec<String> = (10..=16).map(|s| format!("{s} pt")).collect();
                let mut i = f.text_size.clamp(10, 16) as usize - 10;
                if ui.popup(&mut i, &sizes, 80.0) {
                    f.text_size = 10 + i as u32;
                    save = true;
                }
            });
            ui.space(10.0);
            ui.label("Posição do nome:");
            ui.horizontal(|ui| {
                ui.spacing(14.0);
                let mut pos = f.label_right as usize;
                if ui.radio_group(&mut pos, &["Embaixo", "À direita"]) {
                    f.label_right = pos == 1;
                    save = true;
                }
            });
            ui.space(10.0);
            ui.horizontal(|ui| {
                ui.label("Organizar por:");
                ui.spacing(8.0);
                let mut i = ARRANGE.iter().position(|a| a.0 == f.arrange).unwrap_or(0);
                if ui.popup(&mut i, &ARRANGE.iter().map(|a| a.1.to_string()).collect::<Vec<_>>(), 160.0) {
                    f.arrange = ARRANGE[i].0.into();
                    save = true;
                }
            });
            ui.space(10.0);
            ui.horizontal(|ui| {
                ui.label("Fundo:");
                ui.spacing(8.0);
                let mut i = BACKGROUNDS.iter().position(|b| b.0 == f.background).unwrap_or(0);
                if ui.popup(&mut i, &BACKGROUNDS.iter().map(|b| b.1.to_string()).collect::<Vec<_>>(), 140.0) {
                    f.background = BACKGROUNDS[i].0.into();
                    save = true;
                }
            });
        });
        if drag {
            self.pending = true;
        }
        if save {
            let _ = self.config.save();
        }
    }
}
