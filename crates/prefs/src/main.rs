//! Preferências do Glove. Cada mudança é salva no config.toml e o
//! compositor aplica na hora.

use std::time::{Duration, Instant};

use glove_config::{Config, OutputsInfo, ACTIONS};
use glove_kit::{
    paint::{draw::*, icons::Icon, Variant},
    run, App, Rect, Ui, WindowOptions,
};

const W: u32 = 660;
const H: u32 = 480;
const TOOLBAR_H: f32 = 78.0;

#[derive(Clone, Copy, PartialEq)]
enum Pane {
    Aparencia,
    Monitor,
    Dock,
    Teclado,
    Mouse,
    Trackpad,
    Atalhos,
    Terminal,
}

const PANES: [(Pane, Icon, &str); 8] = [
    (Pane::Aparencia, Icon::Window, "Aparência"),
    (Pane::Monitor, Icon::Display, "Monitor"),
    (Pane::Dock, Icon::Generic, "Dock"),
    (Pane::Teclado, Icon::TextEditor, "Teclado"),
    (Pane::Mouse, Icon::Mouse, "Mouse"),
    (Pane::Trackpad, Icon::Trackpad, "Trackpad"),
    (Pane::Atalhos, Icon::Settings, "Atalhos"),
    (Pane::Terminal, Icon::Terminal, "Terminal"),
];

const SCALES: [f64; 8] = [0.0, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
/// Tempo para confirmar uma resolução nova antes de voltar à anterior.
const CONFIRM_SECS: u64 = 15;

fn scale_label(v: f64) -> String {
    format!("{}×", format!("{v}").replace('.', ","))
}

/// "2256x1504@60" -> "2256 × 1504, 60 Hz".
fn mode_label(m: &str) -> String {
    match glove_config::parse_mode(m) {
        Some((w, h, hz)) if hz > 0 => format!("{w} × {h}, {hz} Hz"),
        Some((w, h, _)) => format!("{w} × {h}"),
        None => m.to_string(),
    }
}

const LAYOUTS: [(&str, &str, &str); 10] = [
    ("br", "", "Português (Brasil, ABNT2)"),
    ("us", "", "Inglês (EUA)"),
    ("us", "intl", "Inglês (EUA, internacional)"),
    ("pt", "", "Português (Portugal)"),
    ("latam", "", "Espanhol (América Latina)"),
    ("es", "", "Espanhol (Espanha)"),
    ("gb", "", "Inglês (Reino Unido)"),
    ("fr", "", "Francês"),
    ("de", "", "Alemão"),
    ("it", "", "Italiano"),
];

const WALLPAPERS: [(&str, &str); 3] = [("ondas", "Ondas"), ("aurora", "Aurora"), ("liso", "Liso")];

struct Prefs {
    config: Config,
    pane: Pane,
    /// Imagens PNG encontradas em ~/Imagens (para usar de papel de parede).
    images: Vec<std::path::PathBuf>,
    /// Monitores conectados (escritos pelo compositor).
    outputs: Option<OutputsInfo>,
    /// Resolução nova esperando confirmação: (monitor e resolução anteriores, prazo).
    confirm: Option<(String, String, Instant)>,
}

impl Prefs {
    fn new() -> Self {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let mut images = Vec::new();
        for d in ["Imagens", "Pictures"] {
            if let Ok(rd) = std::fs::read_dir(home.join(d)) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                        images.push(p);
                    }
                }
            }
        }
        images.sort();
        let pane = match std::env::args().nth(1).as_deref() {
            Some("dock") => Pane::Dock,
            Some("teclado") => Pane::Teclado,
            Some("mouse") => Pane::Mouse,
            Some("monitor") => Pane::Monitor,
            Some("trackpad") => Pane::Trackpad,
            Some("atalhos") => Pane::Atalhos,
            Some("terminal") => Pane::Terminal,
            _ => Pane::Aparencia,
        };
        Self { config: Config::load(), pane, images, outputs: OutputsInfo::load(), confirm: None }
    }

    fn save(&self) {
        if let Err(e) = self.config.save() {
            eprintln!("glove-prefs: não consegui salvar: {e}");
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        // Faixa clara com brilho, separada do conteúdo por uma linha.
        ui.c.fill_rect(0.0, 0.0, w, TOOLBAR_H, &vgrad(0.0, TOOLBAR_H, &[(0.0, rgba(250, 250, 250, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.pinstripes(0.0, 0.0, w, TOOLBAR_H, rgba(255, 255, 255, 0), rgba(255, 255, 255, 70));
        ui.c.hline(0.0, TOOLBAR_H - 1.0, w, rgba(120, 120, 124, 255));
        let total: f32 = PANES.iter().map(|(_, _, l)| (ui.text_width(l, 11.0, false) + 12.0).max(64.0)).sum::<f32>() + 6.0 * (PANES.len() as f32 - 1.0);
        ui.area(Rect::new((w - total) / 2.0, 10.0, total, TOOLBAR_H), |ui| {
            ui.spacing(6.0);
            ui.horizontal(|ui| {
                for (p, icon, label) in PANES {
                    if ui.icon_tile(icon, label, self.pane == p) && self.pane != p {
                        self.pane = p;
                        ui.redraw = true;
                    }
                }
            });
        });
    }

    fn aparencia(&mut self, ui: &mut Ui) {
        ui.heading("Tema");
        let mut v = Variant::ALL.iter().position(|v| v.id() == self.config.theme).unwrap_or(0);
        let labels: Vec<&str> = Variant::ALL.iter().map(|v| v.label()).collect();
        ui.indent(16.0, |ui| {
            ui.spacing(4.0);
            ui.horizontal(|ui| {
                ui.spacing(18.0);
                if ui.radio_group(&mut v, &labels) {
                    self.config.theme = Variant::ALL[v].id().into();
                    self.save();
                }
            });
            ui.small("Listras: gel azul e listras finas. Metal: barras de metal escovado. Grafite: tons de cinza.");
        });
        ui.space(10.0);
        ui.separator();
        ui.space(6.0);
        ui.heading("Papel de parede");
        let mut options: Vec<String> = WALLPAPERS.iter().map(|(_, l)| l.to_string()).collect();
        let mut ids: Vec<String> = WALLPAPERS.iter().map(|(i, _)| i.to_string()).collect();
        for p in &self.images {
            options.push(p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            ids.push(p.to_string_lossy().into_owned());
        }
        let mut cur = ids.iter().position(|i| *i == self.config.wallpaper).unwrap_or(0);
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.label_right("Imagem:", 70.0);
                if ui.popup(&mut cur, &options, 260.0) {
                    self.config.wallpaper = ids[cur].clone();
                    self.save();
                }
            });
            ui.small("Imagens PNG da sua pasta Imagens também aparecem na lista.");
        });
    }

    fn dock(&mut self, ui: &mut Ui) {
        ui.heading("Tamanho");
        let mut size = self.config.dock.size as f32;
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.small("Pequeno");
                if ui.slider(&mut size, 32.0, 72.0, 320.0) {
                    self.config.dock.size = size.round() as u32;
                    self.config.dock.max_size = self.config.dock.max_size.max(self.config.dock.size);
                    self.save();
                }
                ui.small("Grande");
            });
        });
        ui.space(8.0);
        let mut mag = self.config.dock.magnification;
        if ui.checkbox(&mut mag, "Ampliar ícones ao passar o mouse") {
            self.config.dock.magnification = mag;
            self.save();
        }
        let mut max = self.config.dock.max_size as f32;
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.small("Mínimo");
                if ui.slider(&mut max, self.config.dock.size as f32, 128.0, 320.0) && mag {
                    self.config.dock.max_size = max.round() as u32;
                    self.save();
                }
                ui.small("Máximo");
            });
        });
    }

    fn teclado(&mut self, ui: &mut Ui) {
        ui.heading("Layout do teclado");
        let names: Vec<String> = LAYOUTS.iter().map(|(_, _, n)| n.to_string()).collect();
        let mut cur = LAYOUTS
            .iter()
            .position(|(l, v, _)| *l == self.config.keyboard.layout && *v == self.config.keyboard.variant)
            .unwrap_or(0);
        ui.indent(16.0, |ui| {
            if ui.popup(&mut cur, &names, 300.0) {
                self.config.keyboard.layout = LAYOUTS[cur].0.into();
                self.config.keyboard.variant = LAYOUTS[cur].1.into();
                self.save();
            }
        });
        ui.space(10.0);
        ui.separator();
        ui.space(6.0);
        ui.heading("Repetição de teclas");
        let mut delay = self.config.keyboard.repeat_delay as f32;
        let mut rate = self.config.keyboard.repeat_rate as f32;
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.label_right("Atraso:", 80.0);
                ui.small("Longo");
                // Invertido: à direita = atraso menor.
                let mut inv = 1000.0 - delay;
                if ui.slider(&mut inv, 0.0, 850.0, 260.0) {
                    delay = 1000.0 - inv;
                    self.config.keyboard.repeat_delay = delay.round() as u32;
                    self.save();
                }
                ui.small("Curto");
            });
            ui.horizontal(|ui| {
                ui.label_right("Velocidade:", 80.0);
                ui.small("Lenta");
                if ui.slider(&mut rate, 2.0, 60.0, 260.0) {
                    self.config.keyboard.repeat_rate = rate.round() as u32;
                    self.save();
                }
                ui.small("Rápida");
            });
            ui.space(6.0);
            ui.small("Teste aqui:");
            let mut test = TEST.with(|t| t.borrow().clone());
            if ui.text_field(&mut test, 400.0) {
                TEST.with(|t| *t.borrow_mut() = test);
            }
        });
    }

    fn nested(&self) -> bool {
        self.outputs.as_ref().is_none_or(|o| o.nested)
    }

    fn monitor(&mut self, ui: &mut Ui) {
        let nested = self.nested();
        let outputs = self.outputs.clone().unwrap_or_default();
        if nested {
            ui.small("O Glove está numa janela dentro de outro desktop: a resolução é a da janela.");
            ui.small("O que for escolhido aqui vale quando você entrar na sessão Glove.");
            ui.space(8.0);
        } else {
            ui.heading("Monitor");
            let mut names = vec!["Automático (externo, se houver)".to_string()];
            let mut ids = vec![String::new()];
            for o in &outputs.outputs {
                names.push(o.description.clone());
                ids.push(o.name.clone());
            }
            let mut cur = ids.iter().position(|i| *i == self.config.monitor.output).unwrap_or(0);
            ui.indent(16.0, |ui| {
                if ui.popup(&mut cur, &names, 300.0) {
                    self.ask_confirm();
                    self.config.monitor.output = ids[cur].clone();
                    self.config.monitor.mode.clear();
                    self.save();
                }
            });
            ui.space(8.0);

            let active = outputs.outputs.iter().find(|o| o.active).cloned().unwrap_or_default();
            ui.heading("Resolução");
            let mut names = vec![format!("Recomendada ({})", mode_label(&active.preferred))];
            let mut ids = vec![String::new()];
            for m in &active.modes {
                names.push(mode_label(m));
                ids.push(m.clone());
            }
            let mut cur = ids.iter().position(|i| *i == self.config.monitor.mode).unwrap_or(0);
            ui.indent(16.0, |ui| {
                if ui.popup(&mut cur, &names, 300.0) {
                    self.ask_confirm();
                    self.config.monitor.mode = ids[cur].clone();
                    self.save();
                }
                if !active.current.is_empty() {
                    let t = format!("Em uso: {}", mode_label(&active.current));
                    ui.small(&t);
                }
            });
            ui.space(8.0);
        }

        ui.heading("Tamanho da interface");
        let auto = outputs.outputs.iter().find(|o| o.active).map(|o| o.auto_scale).unwrap_or(1.0);
        let names: Vec<String> = SCALES
            .iter()
            .map(|v| if *v == 0.0 { format!("Automático ({})", scale_label(auto)) } else { scale_label(*v) })
            .collect();
        let mut cur = SCALES.iter().position(|v| *v == self.config.monitor.scale).unwrap_or(0);
        ui.indent(16.0, |ui| {
            if ui.popup(&mut cur, &names, 200.0) {
                self.config.monitor.scale = SCALES[cur];
                self.save();
            }
            ui.small("Maior = textos e janelas maiores. Automático segue a densidade da tela.");
        });

        if let Some((_, _, until)) = &self.confirm {
            let left = until.saturating_duration_since(Instant::now()).as_secs() + 1;
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            let t = format!("Manter esta resolução? Volta à anterior em {left} s.");
            ui.label(&t);
            let (mut keep, mut back) = (false, false);
            ui.horizontal(|ui| {
                keep = ui.default_button("Manter");
                back = ui.button("Voltar");
            });
            if keep {
                self.confirm = None;
            } else if back {
                self.revert();
            }
        }
    }

    /// Guarda o monitor e a resolução atuais para poder voltar.
    fn ask_confirm(&mut self) {
        if self.confirm.is_none() {
            let m = &self.config.monitor;
            self.confirm = Some((m.output.clone(), m.mode.clone(), Instant::now() + Duration::from_secs(CONFIRM_SECS)));
        } else if let Some(c) = self.confirm.as_mut() {
            c.2 = Instant::now() + Duration::from_secs(CONFIRM_SECS);
        }
    }

    fn revert(&mut self) {
        if let Some((output, mode, _)) = self.confirm.take() {
            self.config.monitor.output = output;
            self.config.monitor.mode = mode;
            self.save();
        }
    }

    fn mouse(&mut self, ui: &mut Ui) {
        let p = self.config.pointer.clone();
        ui.heading("Velocidade do cursor");
        let mut speed = p.speed as f32;
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.small("Lenta");
                if ui.slider(&mut speed, -1.0, 1.0, 300.0) {
                    self.config.pointer.speed = (speed as f64 * 20.0).round() / 20.0;
                    self.save();
                }
                ui.small("Rápida");
            });
            let mut acc = p.acceleration;
            if ui.checkbox(&mut acc, "Aceleração (anda mais quando o mouse se move rápido)") {
                self.config.pointer.acceleration = acc;
                self.save();
            }
        });
        ui.space(8.0);
        ui.heading("Rolagem e botões");
        ui.indent(16.0, |ui| {
            let mut nat = p.natural_scroll;
            if ui.checkbox(&mut nat, "Rolagem natural (o conteúdo acompanha a roda)") {
                self.config.pointer.natural_scroll = nat;
                self.save();
            }
            let mut left = p.left_handed;
            if ui.checkbox(&mut left, "Trocar os botões (para canhotos)") {
                self.config.pointer.left_handed = left;
                self.save();
            }
        });
        if self.nested() {
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            ui.small("Dentro de outro desktop só a rolagem vale agora; o resto vale na sessão Glove.");
        }
    }

    fn trackpad(&mut self, ui: &mut Ui) {
        let t = self.config.touchpad.clone();
        ui.heading("Clique");
        ui.indent(16.0, |ui| {
            let mut tap = t.tap_to_click;
            if ui.checkbox(&mut tap, "Tocar para clicar") {
                self.config.touchpad.tap_to_click = tap;
                self.save();
            }
            ui.space(4.0);
            ui.small("Clique secundário:");
            let mut v = if t.two_finger_right_click { 0 } else { 1 };
            ui.horizontal(|ui| {
                ui.spacing(18.0);
                if ui.radio_group(&mut v, &["Com dois dedos", "No canto inferior direito"]) {
                    self.config.touchpad.two_finger_right_click = v == 0;
                    self.save();
                }
            });
        });
        ui.space(8.0);
        ui.heading("Movimento");
        ui.indent(16.0, |ui| {
            let mut speed = t.speed as f32;
            ui.horizontal(|ui| {
                ui.small("Lenta");
                if ui.slider(&mut speed, -1.0, 1.0, 300.0) {
                    self.config.touchpad.speed = (speed as f64 * 20.0).round() / 20.0;
                    self.save();
                }
                ui.small("Rápida");
            });
            let mut nat = t.natural_scroll;
            if ui.checkbox(&mut nat, "Rolagem natural (o conteúdo acompanha os dedos)") {
                self.config.touchpad.natural_scroll = nat;
                self.save();
            }
            let mut dwt = t.disable_while_typing;
            if ui.checkbox(&mut dwt, "Ignorar o trackpad enquanto digito") {
                self.config.touchpad.disable_while_typing = dwt;
                self.save();
            }
        });
        if self.nested() {
            ui.space(10.0);
            ui.separator();
            ui.space(6.0);
            ui.small("Dentro de outro desktop o trackpad segue o desktop de lá; isto vale na sessão Glove.");
        }
    }

    fn atalhos(&mut self, ui: &mut Ui) {
        ui.heading("Atalhos de teclado");
        ui.small("Clique num atalho e pressione a nova combinação (Esc cancela).");
        ui.space(6.0);
        for (action, label) in ACTIONS {
            let mut keys = self.config.shortcut(action).unwrap_or("").to_string();
            let mut changed = false;
            ui.push_id(action, |ui| {
                ui.horizontal(|ui| {
                    ui.label_right(label, 200.0);
                    changed = ui.shortcut_field(&mut keys, 220.0);
                });
            });
            if changed {
                match self.config.shortcuts.iter_mut().find(|s| s.action == *action) {
                    Some(s) => s.keys = keys,
                    None => self.config.shortcuts.push(glove_config::Shortcut { action: action.to_string(), keys }),
                }
                self.save();
            }
        }
        ui.space(6.0);
        if ui.button("Restaurar padrões") {
            self.config.shortcuts = glove_config::default_shortcuts();
            self.save();
        }
    }
}

impl Prefs {
    fn terminal(&mut self, ui: &mut Ui) {
        ui.heading("Fonte do Terminal");
        ui.indent(16.0, |ui| {
            ui.horizontal(|ui| {
                ui.label_right("Família:", 80.0);
                let mut fam = self.config.terminal_font.clone();
                if ui.text_field(&mut fam, 300.0) {
                    self.config.terminal_font = fam;
                    self.save();
                }
            });
            ui.small("Ex.: JetBrainsMono Nerd Font. Vazio = DejaVu Sans Mono.");
            ui.small("Ícones de Nerd Font aparecem mesmo com outra fonte, se houver uma instalada.");
            ui.space(8.0);
            ui.horizontal(|ui| {
                ui.label_right("Tamanho:", 80.0);
                let mut px = self.config.terminal_font_size.clamp(8.0, 24.0);
                if ui.slider(&mut px, 8.0, 24.0, 260.0) {
                    self.config.terminal_font_size = px.round();
                    self.save();
                }
                let t = format!("{} pt", self.config.terminal_font_size.round());
                ui.small(&t);
            });
            ui.space(6.0);
            ui.small("Vale para as próximas janelas do Terminal.");
        });
    }
}

thread_local! {
    static TEST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

impl App for Prefs {
    fn ui(&mut self, ui: &mut Ui) {
        self.toolbar(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        ui.area(Rect::new(28.0, TOOLBAR_H + 20.0, w - 56.0, h - TOOLBAR_H - 40.0), |ui| match self.pane {
            Pane::Aparencia => self.aparencia(ui),
            Pane::Monitor => self.monitor(ui),
            Pane::Trackpad => self.trackpad(ui),
            Pane::Dock => self.dock(ui),
            Pane::Teclado => self.teclado(ui),
            Pane::Mouse => self.mouse(ui),
            Pane::Atalhos => self.atalhos(ui),
            Pane::Terminal => self.terminal(ui),
        });
    }

    fn config_changed(&mut self) {
        self.config = Config::load();
        self.outputs = OutputsInfo::load();
    }

    fn tick(&self) -> Option<Duration> {
        // Contagem da confirmação de resolução; e os monitores conectados (cabo).
        if self.confirm.is_some() {
            Some(Duration::from_millis(500))
        } else if self.pane == Pane::Monitor {
            Some(Duration::from_secs(2))
        } else {
            None
        }
    }

    fn on_tick(&mut self) {
        if self.confirm.as_ref().is_some_and(|c| Instant::now() >= c.2) {
            self.revert();
        }
        self.outputs = OutputsInfo::load();
    }

    fn close_requested(&mut self) -> bool {
        // Fechar sem confirmar = voltar à resolução anterior.
        self.revert();
        true
    }
}

fn main() {
    let opts = WindowOptions { title: "Preferências".into(), app_id: "glove.Preferences".into(), size: (W, H), fixed: true };
    if let Err(e) = run(opts, Prefs::new()) {
        eprintln!("glove-prefs: {e}");
    }
}
