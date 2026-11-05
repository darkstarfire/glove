//! Painel Data e Hora: calendário e relógio, hora automática (NTP), fuso
//! horário e o relógio da barra de menu. Muda o sistema pelo `timedatectl`
//! (o polkit pode pedir a senha de administrador).

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, Color},
    Ui,
};

#[derive(Default)]
pub struct DataHora {
    pub tab: usize,
    /// (fuso, NTP ligado, sincronizado).
    status: Option<(String, bool, bool)>,
    zones: Vec<String>,
    region: usize,
    city: Option<usize>,
    /// Data e hora digitadas (com o NTP desligado).
    date: String,
    time: String,
    busy: bool,
    done: Arc<Mutex<Option<Result<String, String>>>>,
    message: Option<(bool, String)>,
}

/// Regiões do `timedatectl list-timezones`, com os nomes em português.
const REGIONS: [(&str, &str); 11] = [
    ("America", "América"),
    ("Europe", "Europa"),
    ("Africa", "África"),
    ("Asia", "Ásia"),
    ("Australia", "Austrália"),
    ("Pacific", "Pacífico"),
    ("Atlantic", "Atlântico"),
    ("Indian", "Índico"),
    ("Antarctica", "Antártida"),
    ("Arctic", "Ártico"),
    ("Etc", "Outros"),
];

/// "America/Argentina/Buenos_Aires" → "Buenos Aires (Argentina)".
pub fn city_name(zone: &str) -> String {
    let parts: Vec<&str> = zone.split('/').collect();
    let city = parts.last().unwrap_or(&zone).replace('_', " ");
    if parts.len() > 2 {
        format!("{city} ({})", parts[1..parts.len() - 1].join(", ").replace('_', " "))
    } else {
        city
    }
}

/// Dias do mês (com ano bissexto).
fn days_in(month: u32, year: i32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Dia da semana (0 = domingo) do dia 1 de um mês (Zeller).
pub fn first_weekday(month: u32, year: i32) -> u32 {
    let (m, y) = if month < 3 { (month + 12, year - 1) } else { (month, year) };
    let k = y.rem_euclid(100);
    let j = y.div_euclid(100);
    let h = (1 + (13 * (m as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    ((h + 6) % 7) as u32
}

const MESES: [&str; 12] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];

/// "dd/mm/aaaa" + "hh:mm[:ss]" → "aaaa-mm-dd hh:mm:ss" para o timedatectl.
pub fn parse_datetime(date: &str, time: &str) -> Option<String> {
    let d: Vec<u32> = date.trim().split('/').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let t: Vec<u32> = time.trim().split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [day, month, year] = d[..] else { return None };
    let (h, m, s) = match t[..] {
        [h, m] => (h, m, 0),
        [h, m, s] => (h, m, s),
        _ => return None,
    };
    let ok = (1..=12).contains(&month) && year >= 1970 && day >= 1 && day <= days_in(month, year as i32) && h < 24 && m < 60 && s < 60;
    ok.then(|| format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}"))
}

fn timedatectl(args: &[&str]) -> Result<(), String> {
    let out = Command::new("timedatectl").args(args).output().map_err(|e| format!("Não foi possível usar o timedatectl: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let msg = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("o sistema recusou").trim().to_string();
    // Sem permissão: explica em português.
    if msg.contains("ccess denied") || msg.contains("nteractive authentication required") || msg.contains("not authorized") {
        return Err("o sistema pediu permissão de administrador e ela não foi dada.".into());
    }
    Err(msg)
}

impl DataHora {
    pub fn refresh(&mut self) {
        let out = Command::new("timedatectl").args(["show", "-p", "Timezone", "-p", "NTP", "-p", "NTPSynchronized"]).output().ok();
        self.status = out.filter(|o| o.status.success()).map(|o| {
            let t = String::from_utf8_lossy(&o.stdout).into_owned();
            let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
            (get("Timezone"), get("NTP") == "yes", get("NTPSynchronized") == "yes")
        });
        if self.zones.is_empty() {
            self.zones = Command::new("timedatectl")
                .arg("list-timezones")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect())
                .unwrap_or_default();
            // A região do fuso atual já vem escolhida.
            if let Some((tz, ..)) = &self.status {
                let head = tz.split('/').next().unwrap_or("");
                self.region = REGIONS.iter().position(|(r, _)| *r == head).unwrap_or(0);
            }
        }
    }

    fn start(&mut self, ok: String, job: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.busy = true;
        self.message = None;
        let done = self.done.clone();
        std::thread::spawn(move || {
            let r = job().map(|_| ok);
            *done.lock().unwrap() = Some(r);
        });
    }

    pub fn ui(&mut self, ui: &mut Ui, clock: &mut glove_config::Clock) -> bool {
        let finished = self.done.lock().unwrap().take();
        if let Some(r) = finished {
            self.busy = false;
            self.message = Some(match r {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            });
            self.refresh();
        }
        if self.busy {
            ui.wake_after(Duration::from_millis(200));
        }
        let mut save = false;
        let mut tab = self.tab;
        let shown = tab;
        ui.space(4.0);
        ui.tab_box(&mut tab, &["Data e Hora", "Fuso Horário", "Relógio"], 300.0, |ui| match shown {
            0 => self.date_tab(ui),
            1 => self.zone_tab(ui),
            _ => save = self.clock_tab(ui, clock),
        });
        self.tab = tab;
        if let Some((ok, m)) = self.message.clone() {
            ui.space(6.0);
            let t = if ok { m } else { format!("Não deu certo: {m}") };
            ui.small(&t);
        }
        save
    }

    fn date_tab(&mut self, ui: &mut Ui) {
        let Some((_, ntp, synced)) = self.status.clone() else {
            ui.small("O timedatectl não respondeu.");
            return;
        };
        let mut auto = ntp;
        ui.horizontal(|ui| {
            if ui.checkbox(&mut auto, "Ajustar data e hora automaticamente (pela internet)") && !self.busy {
                let on = auto;
                self.start(if on { "Hora automática ligada.".into() } else { "Hora automática desligada.".into() }, move || timedatectl(&["set-ntp", if on { "true" } else { "false" }]));
            }
        });
        if ntp {
            ui.indent(22.0, |ui| ui.small(if synced { "Sincronizado com o servidor de hora." } else { "Esperando o servidor de hora…" }));
        }
        ui.space(8.0);
        let (year, month, day, _, h, m, s) = glove_kit::local_time();
        let (x0, y0) = ui.cursor();
        // Calendário do mês (à esquerda).
        let (cw, ch) = (28.0, 20.0);
        let title = format!("{} de {year}", MESES[(month - 1) as usize]);
        ui.fonts.draw(ui.c, &title, x0 + 4.0, y0 + 13.0, 13.0, true, rgba(20, 20, 20, 255));
        let gy = y0 + 22.0;
        ui.c.fill_rect(x0, gy, cw * 7.0, ch * 7.0, &solid(Color::WHITE));
        ui.c.stroke(rrect(x0 + 0.5, gy + 0.5, cw * 7.0 - 1.0, ch * 7.0 - 1.0, 0.0), &solid(rgba(130, 130, 135, 255)), 1.0);
        ui.c.fill_rect(x0 + 1.0, gy + 1.0, cw * 7.0 - 2.0, ch - 1.0, &vgrad(gy, gy + ch, &[(0.0, rgba(240, 240, 242, 255)), (1.0, rgba(214, 214, 218, 255))]));
        for (k, d) in ["D", "S", "T", "Q", "Q", "S", "S"].iter().enumerate() {
            let tw = ui.text_width(d, 11.0, true);
            ui.fonts.draw(ui.c, d, x0 + k as f32 * cw + (cw - tw) / 2.0, gy + 14.0, 11.0, true, rgba(60, 60, 64, 255));
        }
        let first = first_weekday(month, year);
        for dd in 1..=days_in(month, year) {
            let slot = first + dd - 1;
            let (col, row) = ((slot % 7) as f32, (slot / 7) as f32 + 1.0);
            let (cx, cy) = (x0 + col * cw, gy + row * ch);
            if dd == day {
                ui.c.fill(rrect(cx + 2.0, cy + 1.0, cw - 4.0, ch - 2.0, 4.0), &solid(ui.theme.accent));
            }
            let t = dd.to_string();
            let tw = ui.text_width(&t, 11.0, false);
            ui.fonts.draw(ui.c, &t, cx + (cw - tw) / 2.0, cy + 14.0, 11.0, dd == day, if dd == day { Color::WHITE } else { rgba(30, 30, 34, 255) });
        }
        // Relógio de ponteiros (à direita).
        let (rx, ry, r) = (x0 + cw * 7.0 + 110.0, gy + 70.0, 66.0);
        ui.c.fill(circle(rx + 2.0, ry + 4.0, r), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill(circle(rx, ry, r), &radial(rx - r * 0.3, ry - r * 0.4, r * 1.6, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(220, 222, 228, 255))]));
        ui.c.stroke(circle(rx, ry, r), &solid(rgba(120, 124, 132, 255)), 2.0);
        for k in 0..60 {
            let a = k as f32 * std::f32::consts::TAU / 60.0;
            let (l, w) = if k % 5 == 0 { (8.0, 2.0) } else { (3.0, 1.0) };
            let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
            pb.move_to(rx + (r - 4.0) * a.sin(), ry - (r - 4.0) * a.cos());
            pb.line_to(rx + (r - 4.0 - l) * a.sin(), ry - (r - 4.0 - l) * a.cos());
            ui.c.stroke(pb.finish(), &solid(rgba(60, 60, 66, 255)), w);
        }
        let hand = |ui: &mut Ui, frac: f32, len: f32, w: f32, c: Color| {
            let a = frac * std::f32::consts::TAU;
            let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
            pb.move_to(rx - len * 0.15 * a.sin(), ry + len * 0.15 * a.cos());
            pb.line_to(rx + len * a.sin(), ry - len * a.cos());
            ui.c.stroke_round(pb.finish(), &solid(c), w);
        };
        hand(ui, ((h % 12) as f32 + m as f32 / 60.0) / 12.0, r * 0.5, 5.0, rgba(40, 40, 46, 255));
        hand(ui, (m as f32 + s as f32 / 60.0) / 60.0, r * 0.75, 3.5, rgba(40, 40, 46, 255));
        hand(ui, s as f32 / 60.0, r * 0.82, 1.4, rgba(210, 50, 40, 255));
        ui.c.fill(circle(rx, ry, 3.5), &solid(rgba(210, 50, 40, 255)));
        let t = format!("{h:02}:{m:02}:{s:02}");
        let tw = ui.text_width(&t, 13.0, true);
        ui.fonts.draw(ui.c, &t, rx - tw / 2.0, ry + r + 22.0, 13.0, true, rgba(20, 20, 20, 255));
        ui.alloc(ui.available_width(), ch * 7.0 + 22.0 + 30.0);
        ui.wake_after(Duration::from_millis(1000 - (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_millis()).unwrap_or(0) as u64).min(999)));
        // Acertar à mão (só com a hora automática desligada).
        if !ntp {
            if self.date.is_empty() {
                self.date = format!("{day:02}/{month:02}/{year}");
                self.time = format!("{h:02}:{m:02}:{s:02}");
            }
            ui.horizontal(|ui| {
                ui.label("Data:");
                ui.text_field(&mut self.date, 110.0);
                ui.label("Hora:");
                ui.text_field(&mut self.time, 90.0);
                ui.set_enabled(!self.busy);
                if ui.button("Definir") {
                    match parse_datetime(&self.date, &self.time) {
                        Some(dt) => {
                            self.start("Data e hora definidas.".into(), move || timedatectl(&["set-time", &dt]));
                            self.date.clear();
                        }
                        None => self.message = Some((false, "use a data como 05/11/2026 e a hora como 14:30.".into())),
                    }
                }
                ui.set_enabled(true);
            });
        }
    }

    fn zone_tab(&mut self, ui: &mut Ui) {
        let Some((tz, ..)) = self.status.clone() else {
            ui.small("O timedatectl não respondeu.");
            return;
        };
        // Diferença do UTC pelo próprio libc.
        let offset = {
            unsafe {
                let t = libc::time(std::ptr::null_mut());
                let mut tm: libc::tm = std::mem::zeroed();
                libc::localtime_r(&t, &mut tm);
                tm.tm_gmtoff
            }
        };
        let sign = if offset < 0 { '−' } else { '+' };
        let t = format!("Fuso horário atual: {} (UTC {sign}{:02}:{:02})", city_name(&tz), offset.abs() / 3600, offset.abs() % 3600 / 60);
        ui.label(&t);
        ui.space(6.0);
        let names: Vec<String> = REGIONS.iter().map(|(_, n)| n.to_string()).collect();
        ui.horizontal(|ui| {
            ui.label("Região:");
            if ui.popup(&mut self.region, &names, 160.0) {
                self.city = None;
            }
        });
        ui.space(4.0);
        let prefix = format!("{}/", REGIONS[self.region].0);
        let zones: Vec<&String> = self.zones.iter().filter(|z| z.starts_with(&prefix)).collect();
        let mut rows: Vec<(String, &String)> = zones.iter().map(|z| (city_name(z), *z)).collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let labels: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
        if self.city.is_none() {
            self.city = rows.iter().position(|r| *r.1 == tz);
        }
        ui.list(&mut self.city, &labels, ui.available_width(), 150.0);
        ui.space(6.0);
        let chosen = self.city.and_then(|i| rows.get(i)).map(|r| r.1.clone());
        let can = !self.busy && chosen.as_ref().is_some_and(|z| *z != tz);
        ui.horizontal(|ui| {
            ui.set_enabled(can);
            if ui.default_button("Definir Fuso Horário") {
                if let Some(z) = chosen.clone() {
                    let ok = format!("Fuso horário: {}.", city_name(&z));
                    self.start(ok, move || timedatectl(&["set-timezone", &z]));
                }
            }
            ui.set_enabled(true);
        });
    }

    fn clock_tab(&mut self, ui: &mut Ui, c: &mut glove_config::Clock) -> bool {
        let mut save = false;
        save |= ui.checkbox(&mut c.show, "Mostrar data e hora na barra de menu");
        ui.space(6.0);
        ui.indent(22.0, |ui| {
            let mut v = usize::from(c.analog);
            ui.horizontal(|ui| {
                ui.label("Ver como:");
                ui.spacing(14.0);
                if ui.radio_group(&mut v, &["Digital", "Analógico"]) {
                    c.analog = v == 1;
                    save = true;
                }
            });
            ui.space(4.0);
            let digital = !c.analog;
            ui.set_enabled(digital && c.show);
            save |= ui.checkbox(&mut c.seconds, "Mostrar os segundos");
            save |= ui.checkbox(&mut c.weekday, "Mostrar o dia da semana");
            save |= ui.checkbox(&mut c.date, "Mostrar a data");
            save |= ui.checkbox(&mut c.h24, "Usar o relógio de 24 horas");
            save |= ui.checkbox(&mut c.blink, "Piscar os dois-pontos");
            ui.set_enabled(true);
        });
        ui.space(10.0);
        let (y, mo, d, wd, h, m, s) = glove_kit::local_time();
        let _ = y;
        let preview = if c.analog { "Mostrador de ponteiros".to_string() } else { c.text(wd, d, mo, h, m, s) };
        let t = format!("Na barra: {preview}");
        ui.small(&t);
        save
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn calendario() {
        // 1º de novembro de 2026 é um domingo; 1º de março de 2024, sexta.
        assert_eq!(super::first_weekday(11, 2026), 0);
        assert_eq!(super::first_weekday(3, 2024), 5);
        assert_eq!(super::days_in(2, 2024), 29);
        assert_eq!(super::days_in(2, 2100), 28);
    }

    #[test]
    fn nomes_e_datas() {
        assert_eq!(super::city_name("America/Sao_Paulo"), "Sao Paulo");
        assert_eq!(super::city_name("America/Argentina/Buenos_Aires"), "Buenos Aires (Argentina)");
        assert_eq!(super::parse_datetime("05/11/2026", "14:30").as_deref(), Some("2026-11-05 14:30:00"));
        assert_eq!(super::parse_datetime("31/02/2026", "14:30"), None);
        assert_eq!(super::parse_datetime("05/11/2026", "25:00"), None);
    }
}
