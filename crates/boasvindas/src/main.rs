//! glove-boasvindas: assistente de primeiro uso (abre na primeira entrada).
//! Passos: boas-vindas, teclado, data e hora, aparência, foto e pronto. As
//! escolhas valem na hora (o Glove relê a configuração).
//!
//! Com GLOVE_TESTE=1 não mexe no sistema (fuso e foto da conta): só na configuração.

use glove_config::tr;
use std::time::Duration;

use glove_config::Config;
use glove_kit::{
    paint::{
        chrome::avatar,
        draw::{gel_circle, rgba, rrect, solid, Canvas},
        Variant,
    },
    run, App, Rect, Ui, WindowOptions,
};

const W: f32 = 560.0;
const H: f32 = 440.0;

/// Teclados mais comuns (layout, variante, nome).
const LAYOUTS: [(&str, &str, &str); 6] = [
    ("br", "", "Português (Brasil, ABNT2)"),
    ("us", "intl", "Inglês (EUA, internacional)"),
    ("us", "", "Inglês (EUA)"),
    ("pt", "", "Português (Portugal)"),
    ("latam", "", "Espanhol (América Latina)"),
    ("es", "", "Espanhol (Espanha)"),
];

/// Fusos do Brasil (mais o "manter").
const ZONES: [(&str, &str); 7] = [
    ("", "Manter o fuso atual"),
    ("America/Sao_Paulo", "Brasília (São Paulo, Rio, Sul, Nordeste)"),
    ("America/Manaus", "Amazonas (Manaus)"),
    ("America/Cuiaba", "Mato Grosso (Cuiabá)"),
    ("America/Porto_Velho", "Rondônia (Porto Velho)"),
    ("America/Rio_Branco", "Acre (Rio Branco)"),
    ("America/Noronha", "Fernando de Noronha"),
];

const STEPS: usize = 6;
/// Quantos avatares para escolher.
const AVATARS: usize = 12;

struct Welcome {
    step: usize,
    config: Config,
    layout: usize,
    zone: usize,
    zone_now: String,
    theme: usize,
    /// None = manter a foto atual.
    avatar: Option<usize>,
    note: Option<String>,
    test: bool,
}

fn current_zone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| p.to_string_lossy().split("zoneinfo/").nth(1).map(str::to_string))
        .unwrap_or_else(|| "desconhecido".into())
}

/// Semente de cada avatar da grade (as 6 figuras em cores variadas).
fn seed(i: usize) -> u64 {
    // Figura = coluna; cor deslocada a cada linha (nenhum par se repete).
    let (shape, color) = (i % 6, (i % 6 + 1 + (i / 6) * 3) % 6);
    (shape * 6 + color) as u64
}

impl Welcome {
    fn save(&mut self) {
        let _ = self.config.save();
    }

    /// Ao sair de um passo: aplica o que foi escolhido nele.
    fn apply(&mut self) {
        match self.step {
            1 => {
                let (l, v, _) = LAYOUTS[self.layout];
                self.config.keyboard.layout = l.into();
                self.config.keyboard.variant = v.into();
                self.save();
            }
            2 => {
                let (zone, _) = ZONES[self.zone];
                if !zone.is_empty() && !self.test {
                    let ok = std::process::Command::new("timedatectl").args(["set-timezone", zone]).status().map(|s| s.success()).unwrap_or(false);
                    if !ok {
                        self.note = Some(tr!("Não foi possível mudar o fuso (pode pedir senha de administrador). Dá para mudar depois.").into());
                    }
                }
            }
            3 => {
                self.config.theme = Variant::ALL[self.theme].id().into();
                self.save();
            }
            4 => {
                if let Some(i) = self.avatar {
                    if let Err(e) = self.set_photo(i) {
                        self.note = Some(tr!("Não foi possível guardar a foto: {}", e));
                    }
                }
            }
            _ => {}
        }
    }

    /// Grava o avatar em ~/.face (256 px) e avisa o AccountsService (tela de login).
    fn set_photo(&self, i: usize) -> Result<(), String> {
        let home = std::env::var_os("HOME").ok_or("sem HOME")?;
        let path = std::path::PathBuf::from(home).join(".face");
        let mut c = Canvas::new(128.0, 128.0, 2.0).ok_or("canvas")?;
        avatar(&mut c, 0.0, 0.0, 128.0, seed(i));
        c.pm.save_png(&path).map_err(|e| e.to_string())?;
        if !self.test {
            let uid = unsafe_uid();
            let _ = std::process::Command::new("busctl")
                .args(["call", "org.freedesktop.Accounts", &format!("/org/freedesktop/Accounts/User{uid}"), "org.freedesktop.Accounts.User", "SetIconFile", "s"])
                .arg(&path)
                .status();
        }
        Ok(())
    }
}

/// UID do usuário (pelo dono da pasta pessoal, sem libc).
fn unsafe_uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::env::var_os("HOME").and_then(|h| std::fs::metadata(h).ok()).map(|m| m.uid()).unwrap_or(1000)
}

fn title(ui: &mut Ui, t: &str, sub: &str) {
    ui.fonts.draw(ui.c, t, 40.0, 54.0, 20.0, true, rgba(20, 20, 20, 255));
    if !sub.is_empty() {
        let w = ui.c.w;
        let mut y = 76.0;
        // Quebra simples do subtítulo.
        let mut line = String::new();
        for word in sub.split_whitespace() {
            let t = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if ui.text_width(&t, 12.0, false) > w - 80.0 {
                ui.fonts.draw(ui.c, &line, 40.0, y, 12.0, false, rgba(60, 60, 60, 255));
                y += 17.0;
                line = word.to_string();
            } else {
                line = t;
            }
        }
        ui.fonts.draw(ui.c, &line, 40.0, y, 12.0, false, rgba(60, 60, 60, 255));
    }
}

impl App for Welcome {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let accent = ui.theme.accent;
        match self.step {
            0 => {
                gel_circle(ui.c, w / 2.0, 120.0, 48.0, accent);
                let t = tr!("Bem-vindo ao Glove");
                let tw = ui.text_width(t, 24.0, true);
                ui.fonts.draw(ui.c, t, (w - tw) / 2.0, 214.0, 24.0, true, rgba(20, 20, 20, 255));
                for (k, line) in [tr!("Em poucos passos você escolhe o teclado, o fuso,"), tr!("a aparência e a sua foto. Tudo pode ser mudado"), tr!("depois nas Preferências.")].iter().enumerate() {
                    let lw = ui.text_width(line, 13.0, false);
                    ui.fonts.draw(ui.c, line, (w - lw) / 2.0, 250.0 + k as f32 * 19.0, 13.0, false, rgba(50, 50, 50, 255));
                }
            }
            1 => {
                title(ui, tr!("Teclado"), tr!("Escolha o teclado que você usa. Ele vale na hora: experimente digitar acentos em qualquer app."));
                let names: Vec<&str> = LAYOUTS.iter().map(|l| tr(l.2)).collect();
                let mut cur = self.layout;
                ui.area(Rect::new(60.0, 120.0, w - 120.0, 240.0), |ui| {
                    ui.radio_group(&mut cur, &names);
                });
                self.layout = cur;
            }
            2 => {
                let now = glove_kit::local_time();
                let sub = format!("Agora são {:02}:{:02} no fuso {}.", now.4, now.5, self.zone_now);
                title(ui, tr!("Data e hora"), &sub);
                ui.wake_after(Duration::from_secs(15));
                let names: Vec<&str> = ZONES.iter().map(|z| tr(z.1)).collect();
                let mut cur = self.zone;
                ui.area(Rect::new(60.0, 110.0, w - 120.0, 260.0), |ui| {
                    ui.radio_group(&mut cur, &names);
                });
                self.zone = cur;
            }
            3 => {
                title(ui, tr!("Aparência"), tr!("Escolha o visual das janelas, menus e botões."));
                let names: Vec<&str> = Variant::ALL.iter().map(|v| v.label()).collect();
                let mut cur = self.theme;
                ui.area(Rect::new(60.0, 120.0, w - 120.0, 200.0), |ui| {
                    ui.radio_group(&mut cur, &names);
                });
                if cur != self.theme {
                    // Vale na hora, para ver como fica.
                    self.theme = cur;
                    self.config.theme = Variant::ALL[cur].id().into();
                    self.save();
                }
            }
            4 => {
                title(ui, tr!("Sua foto"), tr!("Ela aparece na tela de login e no bloqueio de tela."));
                let (cols, s, gap) = (6, 64.0, 14.0);
                let x0 = (w - (cols as f32 * (s + gap) - gap)) / 2.0;
                for i in 0..AVATARS {
                    let (x, y) = (x0 + (i % cols) as f32 * (s + gap), 110.0 + (i / cols) as f32 * (s + gap));
                    let r = Rect::new(x - 4.0, y - 4.0, s + 8.0, s + 8.0);
                    if self.avatar == Some(i) {
                        ui.c.fill(rrect(r.x, r.y, r.w, r.h, 10.0), &solid(accent));
                    }
                    avatar(ui.c, x, y, s, seed(i));
                    if ui.hit(r).clicked {
                        self.avatar = Some(i);
                    }
                }
                let mut keep = self.avatar.is_none();
                ui.area(Rect::new(x0, 290.0, 300.0, 24.0), |ui| {
                    ui.checkbox(&mut keep, tr!("Manter a foto atual"));
                });
                if keep && self.avatar.is_some() {
                    self.avatar = None;
                }
            }
            _ => {
                gel_circle(ui.c, w / 2.0, 120.0, 40.0, accent);
                let t = tr!("Tudo pronto");
                let tw = ui.text_width(t, 24.0, true);
                ui.fonts.draw(ui.c, t, (w - tw) / 2.0, 204.0, 24.0, true, rgba(20, 20, 20, 255));
                let line = tr!("Bom uso! A Busca Rápida (Super+Espaço) acha apps e arquivos.");
                let lw = ui.text_width(line, 13.0, false);
                ui.fonts.draw(ui.c, line, (w - lw) / 2.0, 240.0, 13.0, false, rgba(50, 50, 50, 255));
            }
        }
        if let Some(n) = self.note.clone() {
            let nw = ui.text_width(&n, 11.0, false).min(w - 40.0);
            ui.fonts.draw(ui.c, &n, (w - nw) / 2.0, H - 64.0, 11.0, false, rgba(180, 40, 40, 255));
        }

        // Pontinhos do passo e os botões.
        for i in 0..STEPS {
            let x = 40.0 + i as f32 * 14.0;
            let col = if i == self.step { accent } else { rgba(190, 190, 194, 255) };
            ui.c.fill(glove_kit::paint::draw::circle(x, H - 30.0, 4.0), &solid(col));
        }
        let last = self.step == STEPS - 1;
        let (mut back, mut next) = (false, false);
        if self.step > 0 && !last {
            ui.area(Rect::new(w - 260.0, H - 44.0, 100.0, 24.0), |ui| back = ui.button(tr!("Voltar")));
        }
        ui.area(Rect::new(w - 150.0, H - 44.0, 120.0, 24.0), |ui| next = ui.default_button(if last { tr!("Começar") } else { tr!("Continuar") }));
        if ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Enter) {
            next = true;
        }
        if back {
            self.note = None;
            self.step -= 1;
        } else if next {
            self.note = None;
            self.apply();
            if last {
                self.config.welcome_done = true;
                self.save();
                ui.close = true;
            } else {
                self.step += 1;
            }
        }
    }
}

pub fn main() {
    let config = Config::load();
    let layout = LAYOUTS.iter().position(|(l, v, _)| *l == config.keyboard.layout && *v == config.keyboard.variant).unwrap_or(0);
    let theme = Variant::ALL.iter().position(|v| v.id() == config.theme).unwrap_or(0);
    let app = Welcome { step: 0, config, layout, zone: 0, zone_now: current_zone(), theme, avatar: None, note: None, test: std::env::var_os("GLOVE_TESTE").is_some() };
    let opts = WindowOptions { title: tr!("Bem-vindo").into(), app_id: "glove.Boasvindas".into(), size: (W as u32, H as u32), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-boasvindas: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn avatares_diferentes() {
        let s: std::collections::HashSet<u64> = (0..super::AVATARS).map(super::seed).collect();
        assert_eq!(s.len(), super::AVATARS);
    }
}
