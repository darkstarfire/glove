//! Agenda (como o iCal da era, no essencial): o mês em grade com os eventos,
//! o dia escolhido ao lado e a folha de evento. Fica em
//! ~/.local/share/glove/agenda/agenda.ics; importa outros .ics.

use glove_config::tr;
mod ics;

use std::path::PathBuf;

use glove_ui::{
    paint::{draw::*, Color},
    run, App, Choice, FileBrowser, Glyph, Menu, MenuItem, Rect, Ui, WindowOptions, SEGMENT_H, SEGMENT_W,
};
use ics::{Event, Stamp};

const TOOL_H: f32 = 44.0;
const SIDE_W: f32 = 230.0;
const MESES: [&str; 12] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];
const DIAS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];

fn file() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("glove/agenda/agenda.ics")
}

pub fn days_in(month: u32, year: i32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Dia da semana (0 = domingo).
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    let (m2, y2) = if m < 3 { (m + 12, y - 1) } else { (m, y) };
    let k = y2.rem_euclid(100);
    let j = y2.div_euclid(100);
    let h = (d as i32 + (13 * (m2 as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    ((h + 6) % 7) as u32
}

fn add_days((y, m, d): (i32, u32, u32), n: i32) -> (i32, u32, u32) {
    let (mut y, mut m, mut d) = (y, m, d as i32 + n);
    while d < 1 {
        m = if m == 1 { y -= 1; 12 } else { m - 1 };
        d += days_in(m, y) as i32;
    }
    while d > days_in(m, y) as i32 {
        d -= days_in(m, y) as i32;
        m = if m == 12 { y += 1; 1 } else { m + 1 };
    }
    (y, m, d as u32)
}

/// O evento acontece neste dia? (dia inteiro: o fim é exclusivo).
fn on_day(e: &Event, day: (i32, u32, u32)) -> bool {
    let s = (e.start.0, e.start.1, e.start.2);
    let en = (e.end.0, e.end.1, e.end.2);
    if e.all_day {
        day >= s && (day < en || s == en && day == s)
    } else {
        day >= s && day <= en
    }
}

fn hm(s: Stamp) -> String {
    format!("{:02}:{:02}", s.3, s.4)
}

fn new_uid() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{t:x}@glove")
}

/// Folha de evento: (uid em edição ou novo, título, data dd/mm/aaaa, início, fim, dia inteiro, notas).
struct Sheet {
    uid: Option<String>,
    title: String,
    date: String,
    start: String,
    end: String,
    all_day: bool,
    notes: String,
    error: Option<String>,
}

/// "hh:mm" → (h, m).
fn parse_hm(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let p: Vec<&str> = s.trim().split('/').collect();
    let [d, m, y] = p[..] else { return None };
    let (d, m, y): (u32, u32, i32) = (d.parse().ok()?, m.parse().ok()?, y.parse().ok()?);
    ((1..=12).contains(&m) && d >= 1 && d <= days_in(m, y)).then_some((y, m, d))
}

struct Agenda {
    events: Vec<Event>,
    /// Mês mostrado e o dia escolhido.
    month: (i32, u32),
    day: (i32, u32, u32),
    today: (i32, u32, u32),
    sheet: Option<Sheet>,
    import: Option<FileBrowser>,
    message: Option<String>,
}

impl Agenda {
    fn save(&mut self) {
        let p = file();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = std::fs::write(&p, ics::serialize(&self.events)) {
            self.message = Some(tr!("Não foi possível salvar: {}", e));
        }
    }

    fn of_day(&self, day: (i32, u32, u32)) -> Vec<Event> {
        let mut v: Vec<Event> = self.events.iter().filter(|e| on_day(e, day)).cloned().collect();
        v.sort_by_key(|e| (!e.all_day, e.start));
        v
    }

    fn open_sheet(&mut self, e: Option<Event>) {
        let (y, m, d) = self.day;
        self.sheet = Some(match e {
            Some(e) => Sheet { uid: Some(e.uid.clone()), title: e.title, date: format!("{:02}/{:02}/{}", e.start.2, e.start.1, e.start.0), start: hm(e.start), end: hm(e.end), all_day: e.all_day, notes: e.notes, error: None },
            None => Sheet { uid: None, title: String::new(), date: format!("{d:02}/{m:02}/{y}"), start: "09:00".into(), end: "10:00".into(), all_day: false, notes: String::new(), error: None },
        });
    }

    fn move_month(&mut self, delta: i32) {
        let (mut y, mut m) = self.month;
        let mi = m as i32 - 1 + delta;
        y += mi.div_euclid(12);
        m = mi.rem_euclid(12) as u32 + 1;
        self.month = (y, m);
        self.day = (y, m, self.day.2.min(days_in(m, y)));
    }

    /// Grade do mês: dias da semana no alto e seis semanas.
    fn month_grid(&mut self, ui: &mut Ui, r: Rect) {
        let (y, m) = self.month;
        let first = weekday(y, m, 1);
        let head = 20.0;
        let cw = r.w / 7.0;
        let ch = (r.h - head) / 6.0;
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        for (i, d) in [tr!("Dom"), tr!("Seg"), tr!("Ter"), tr!("Qua"), tr!("Qui"), tr!("Sex"), tr!("Sáb")].iter().enumerate() {
            let tw = ui.text_width(d, 11.0, true);
            ui.fonts.draw(ui.c, d, r.x + i as f32 * cw + (cw - tw) / 2.0, r.y + 14.0, 11.0, true, rgba(90, 90, 96, 255));
        }
        ui.c.hline(r.x, r.y + head, r.w, rgba(190, 190, 196, 255));
        let start = add_days((y, m, 1), -(first as i32));
        let mut open = None;
        for k in 0..42 {
            let day = add_days(start, k);
            let (col, row) = ((k % 7) as f32, (k / 7) as f32);
            let cell = Rect::new(r.x + col * cw, r.y + head + row * ch, cw, ch);
            let other = day.1 != m;
            if other {
                ui.c.fill_rect(cell.x, cell.y, cell.w, cell.h, &solid(rgba(244, 244, 246, 255)));
            }
            if day == self.day {
                ui.c.fill_rect(cell.x, cell.y, cell.w, cell.h, &solid(rgba(222, 232, 250, 255)));
            }
            ui.c.vline(cell.x + cell.w - 0.5, cell.y, cell.h, rgba(220, 220, 224, 255));
            ui.c.hline(cell.x, cell.y + cell.h - 0.5, cell.w, rgba(220, 220, 224, 255));
            let n = day.2.to_string();
            let nw = ui.text_width(&n, 11.0, true);
            if day == self.today {
                ui.c.fill(circle(cell.x + cell.w - 12.0, cell.y + 10.0, 9.0), &solid(rgba(220, 60, 50, 255)));
            }
            let ink = if day == self.today { Color::WHITE } else if other { rgba(170, 170, 176, 255) } else { rgba(40, 40, 46, 255) };
            ui.fonts.draw(ui.c, &n, cell.x + cell.w - 12.0 - nw / 2.0, cell.y + 14.0, 11.0, true, ink);
            // Eventos do dia (até caber).
            let evs = self.of_day(day);
            let fit = ((cell.h - 22.0) / 15.0).max(0.0) as usize;
            for (i, e) in evs.iter().take(fit).enumerate() {
                let ey = cell.y + 20.0 + i as f32 * 15.0;
                ui.c.fill(rrect(cell.x + 3.0, ey, cell.w - 6.0, 13.0, 3.0), &solid(if e.all_day { rgba(80, 140, 230, 255) } else { rgba(80, 140, 230, 60) }));
                let label = if e.all_day { e.title.clone() } else { format!("{} {}", hm(e.start), e.title) };
                let label = ui.fonts.ellipsize(&label, 10.0, false, cell.w - 12.0);
                ui.fonts.draw(ui.c, &label, cell.x + 6.0, ey + 10.0, 10.0, false, if e.all_day { Color::WHITE } else { rgba(20, 50, 120, 255) });
            }
            if evs.len() > fit && fit > 0 {
                let more = tr!("+{} mais", evs.len() - fit + 1);
                ui.fonts.draw(ui.c, &more, cell.x + 6.0, cell.y + cell.h - 4.0, 9.0, false, rgba(90, 90, 96, 255));
            }
            let h = ui.hit(cell);
            if h.clicked {
                self.day = day;
                if other {
                    self.month = (day.0, day.1);
                }
            }
            if h.double {
                self.day = day;
                open = Some(day);
            }
        }
        if open.is_some() {
            self.open_sheet(None);
        }
    }

    /// O dia escolhido: a data por extenso e os eventos (clique abre).
    fn side(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(236, 238, 242, 255)));
        ui.c.vline(r.x + 0.5, r.y, r.h, rgba(170, 170, 176, 255));
        let (y, m, d) = self.day;
        let wd = tr(DIAS[weekday(y, m, d) as usize]);
        ui.fonts.draw(ui.c, wd, r.x + 14.0, r.y + 22.0, 12.0, false, rgba(90, 90, 96, 255));
        let mes = tr(MESES[(m - 1) as usize]);
        let mes = if glove_config::idioma() == "pt" { mes.to_lowercase() } else { mes.to_string() };
        let t = tr!("{0} de {1} de {2}", d, mes, y);
        ui.fonts.draw(ui.c, &t, r.x + 14.0, r.y + 42.0, 15.0, true, rgba(20, 20, 24, 255));
        let evs = self.of_day(self.day);
        let mut yy = r.y + 64.0;
        let mut pick = None;
        if evs.is_empty() {
            ui.fonts.draw(ui.c, tr!("Nenhum evento."), r.x + 14.0, yy + 14.0, 12.0, false, rgba(130, 130, 136, 255));
        }
        for e in &evs {
            let card = Rect::new(r.x + 10.0, yy, r.w - 20.0, if e.notes.is_empty() { 38.0 } else { 54.0 });
            ui.c.fill(rrect(card.x, card.y, card.w, card.h, 5.0), &solid(Color::WHITE));
            ui.c.fill(rrect(card.x, card.y, 4.0, card.h, 2.0), &solid(rgba(80, 140, 230, 255)));
            let when = if e.all_day { tr!("dia inteiro").to_string() } else { format!("{} – {}", hm(e.start), hm(e.end)) };
            ui.fonts.draw(ui.c, &when, card.x + 12.0, card.y + 14.0, 10.0, false, rgba(90, 90, 96, 255));
            let title = ui.fonts.ellipsize(&e.title, 12.0, true, card.w - 20.0);
            ui.fonts.draw(ui.c, &title, card.x + 12.0, card.y + 30.0, 12.0, true, rgba(20, 20, 24, 255));
            if !e.notes.is_empty() {
                let n = ui.fonts.ellipsize(e.notes.lines().next().unwrap_or(""), 10.0, false, card.w - 20.0);
                ui.fonts.draw(ui.c, &n, card.x + 12.0, card.y + 46.0, 10.0, false, rgba(100, 100, 106, 255));
            }
            if ui.hit(card).clicked {
                pick = Some(e.clone());
            }
            yy += card.h + 8.0;
        }
        ui.area(Rect::new(r.x + 10.0, r.y + r.h - 34.0, r.w - 20.0, 26.0), |ui| {
            if ui.button(tr!("Novo Evento")) {
                pick = Some(Event::default());
            }
        });
        if let Some(e) = pick {
            self.open_sheet(if e.uid.is_empty() { None } else { Some(e) });
        }
    }

    fn sheet_ui(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.sheet else { return };
        let (sw, sh) = (420.0, 300.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = TOOL_H;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        let title = if s.uid.is_some() { tr!("Evento") } else { tr!("Novo Evento") };
        ui.fonts.draw(ui.c, title, x + 20.0, y + 26.0, 13.0, true, rgba(20, 20, 24, 255));
        let label = |ui: &mut Ui, t: &str, yy: f32| {
            let tw = ui.text_width(t, 12.0, false);
            ui.fonts.draw(ui.c, t, x + 100.0 - tw, yy + 15.0, 12.0, false, rgba(40, 40, 44, 255));
        };
        let fresh = s.title.is_empty() && s.uid.is_none();
        label(ui, tr!("Título:"), y + 44.0);
        ui.text_field_in(Rect::new(x + 108.0, y + 44.0, sw - 128.0, 20.0), &mut s.title, fresh && s.error.is_none());
        label(ui, tr!("Data:"), y + 74.0);
        ui.text_field_in(Rect::new(x + 108.0, y + 74.0, 110.0, 20.0), &mut s.date, false);
        let mut ad = s.all_day;
        ui.area(Rect::new(x + 232.0, y + 74.0, 160.0, 22.0), |ui| {
            ui.checkbox(&mut ad, tr!("Dia inteiro"));
        });
        s.all_day = ad;
        if !s.all_day {
            label(ui, tr!("Das:"), y + 104.0);
            ui.text_field_in(Rect::new(x + 108.0, y + 104.0, 60.0, 20.0), &mut s.start, false);
            ui.fonts.draw(ui.c, tr!("às"), x + 178.0, y + 119.0, 12.0, false, rgba(40, 40, 44, 255));
            ui.text_field_in(Rect::new(x + 200.0, y + 104.0, 60.0, 20.0), &mut s.end, false);
        }
        label(ui, tr!("Notas:"), y + 134.0);
        ui.text_field_in(Rect::new(x + 108.0, y + 134.0, sw - 128.0, 20.0), &mut s.notes, false);
        if let Some(e) = &s.error {
            ui.fonts.draw(ui.c, e, x + 108.0, y + 176.0, 11.0, false, rgba(180, 40, 30, 255));
        }
        let editing = s.uid.is_some();
        let (mut cancel, mut ok, mut del) = (false, false, false);
        ui.area(Rect::new(x + 16.0, y + sh - 40.0, sw - 32.0, 30.0), |ui| {
            ui.horizontal(|ui| {
                if editing {
                    del = ui.button(tr!("Apagar"));
                    ui.space(sw - 32.0 - 290.0);
                } else {
                    ui.space(sw - 32.0 - 200.0);
                }
                cancel = ui.button(tr!("Cancelar"));
                ok = ui.default_button(tr!("Salvar"));
            });
        });
        if cancel || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            self.sheet = None;
            return;
        }
        if del {
            if let Some(uid) = s.uid.clone() {
                self.events.retain(|e| e.uid != uid);
                self.sheet = None;
                self.save();
            }
            return;
        }
        if ok {
            let Some(date) = parse_date(&s.date) else {
                s.error = Some(tr!("Use a data como 05/11/2026.").into());
                return;
            };
            let (start, end) = if s.all_day {
                let n = add_days(date, 1);
                ((date.0, date.1, date.2, 0, 0), (n.0, n.1, n.2, 0, 0))
            } else {
                let (Some(a), Some(b)) = (parse_hm(&s.start), parse_hm(&s.end)) else {
                    s.error = Some(tr!("Use os horários como 14:30.").into());
                    return;
                };
                let b = if b < a { a } else { b };
                ((date.0, date.1, date.2, a.0, a.1), (date.0, date.1, date.2, b.0, b.1))
            };
            let ev = Event { uid: s.uid.clone().unwrap_or_else(new_uid), title: if s.title.trim().is_empty() { tr!("Evento").into() } else { s.title.trim().to_string() }, start, end, all_day: s.all_day, notes: s.notes.trim().to_string() };
            self.events.retain(|e| e.uid != ev.uid);
            self.day = date;
            self.month = (date.0, date.1);
            self.events.push(ev);
            self.sheet = None;
            self.save();
        }
    }
}

impl App for Agenda {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let modal = self.sheet.is_some() || self.import.is_some();
        let mut acts: Vec<String> = if modal { Vec::new() } else { ui.menu_actions().to_vec() };
        if !modal {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+N" => acts.push("new".into()),
                    "Ctrl+T" => acts.push("today".into()),
                    "Ctrl+Right" => acts.push("next".into()),
                    "Ctrl+Left" => acts.push("prev".into()),
                    "Ctrl+W" => acts.push("close".into()),
                    _ => {}
                }
            }
        }
        ui.set_enabled(!modal);
        // Barra: setas, Hoje e o mês.
        ui.c.fill_rect(0.0, 0.0, w, TOOL_H, &vgrad(0.0, TOOL_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(206, 206, 210, 255))]));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(140, 140, 146, 255));
        match ui.segmented_in(Rect::new(12.0, (TOOL_H - SEGMENT_H) / 2.0, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &[true, true]) {
            Some(0) => acts.push("prev".into()),
            Some(_) => acts.push("next".into()),
            None => {}
        }
        ui.area(Rect::new(12.0 + 2.0 * SEGMENT_W + 10.0, (TOOL_H - 22.0) / 2.0, 80.0, 24.0), |ui| {
            if ui.small_button(tr!("Hoje")) {
                acts.push("today".into());
            }
        });
        let (y, m) = self.month;
        let t = tr!("{} de {}", tr(MESES[(m - 1) as usize]), y);
        let tw = ui.text_width(&t, 16.0, true);
        ui.fonts.draw(ui.c, &t, (w - SIDE_W - tw) / 2.0 + 60.0, TOOL_H / 2.0 + 6.0, 16.0, true, rgba(30, 30, 34, 255));

        self.month_grid(ui, Rect::new(0.0, TOOL_H, w - SIDE_W, h - TOOL_H));
        self.side(ui, Rect::new(w - SIDE_W, TOOL_H, SIDE_W, h - TOOL_H));
        ui.set_enabled(true);
        if let Some(msg) = &self.message {
            let msg = msg.clone();
            ui.fonts.draw(ui.c, &msg, 12.0, h - 6.0, 10.0, false, rgba(160, 50, 40, 255));
        }

        for a in acts {
            match a.as_str() {
                "new" => self.open_sheet(None),
                "today" => {
                    self.day = self.today;
                    self.month = (self.today.0, self.today.1);
                }
                "next" => self.move_month(1),
                "prev" => self.move_month(-1),
                "import" => self.import = Some(FileBrowser::open(None, &["ics"])),
                "close" => ui.close = true,
                _ => {}
            }
        }
        self.sheet_ui(ui);
        if let Some(fb) = &mut self.import {
            if let Some(c) = ui.file_sheet(fb) {
                self.import = None;
                if let Choice::Picked(p) = c {
                    let mut n = 0;
                    for mut e in ics::parse(&std::fs::read_to_string(&p).unwrap_or_default()) {
                        if e.uid.is_empty() {
                            e.uid = format!("{}-{n}", new_uid());
                        }
                        if !self.events.iter().any(|x| x.uid == e.uid) {
                            self.events.push(e);
                            n += 1;
                        }
                    }
                    self.message = Some(tr!("{} evento(s) importado(s).", n));
                    self.save();
                }
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new(
                tr!("Arquivo"),
                vec![
                    MenuItem::new("new", tr!("Novo Evento"), "Ctrl+N"),
                    MenuItem::Separator,
                    MenuItem::new("import", tr!("Importar .ics…"), ""),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
                ],
            ),
            Menu::new(
                tr!("Visualizar"),
                vec![
                    MenuItem::new("today", tr!("Ir para Hoje"), "Ctrl+T"),
                    MenuItem::new("next", tr!("Próximo Mês"), "Ctrl+Right"),
                    MenuItem::new("prev", tr!("Mês Anterior"), "Ctrl+Left"),
                ],
            ),
        ]
    }
}

pub fn main() {
    let (y, m, d, ..) = glove_ui::local_time();
    let events = std::fs::read_to_string(file()).map(|t| ics::parse(&t)).unwrap_or_default();
    let app = Agenda { events, month: (y, m), day: (y, m, d), today: (y, m, d), sheet: None, import: None, message: None };
    let opts = WindowOptions { title: tr!("Agenda").into(), app_id: "glove.Agenda".into(), size: (860, 560), fixed: false, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-agenda: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendario() {
        assert_eq!(weekday(2026, 11, 1), 0);
        assert_eq!(weekday(2026, 11, 5), 4);
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(add_days((2026, 3, 1), -1), (2026, 2, 28));
        let e = Event { start: (2026, 11, 15, 0, 0), end: (2026, 11, 16, 0, 0), all_day: true, ..Default::default() };
        assert!(on_day(&e, (2026, 11, 15)) && !on_day(&e, (2026, 11, 16)));
        assert_eq!(parse_hm("14:30"), Some((14, 30)));
        assert_eq!(parse_date("31/02/2026"), None);
    }
}
