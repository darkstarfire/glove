//! Painel de Widgets (como o Dashboard do Tiger): F12 (ou um canto ativo)
//! escurece a tela e mostra os widgets; o "+" no canto abre a barra com os
//! widgets que dá para pôr; com a barra aberta, cada widget ganha o × de
//! fechar. Clicar fora dos widgets (ou Esc, ou F12 de novo) fecha o Painel.
//!
//! Tudo desenhado pelo compositor (glove-paint/widgets.rs), sem processos novos;
//! só o Clima chama o `curl` (numa thread) para buscar a previsão no Open-Meteo.
//! Os widgets e o que eles guardam ficam em `~/.config/glove/painel.toml`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glove_paint::widgets::{self as wg, WidgetKind};
use serde::{Deserialize, Serialize};
use smithay::{
    backend::renderer::element::solid::SolidColorBuffer,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    desk::{DeskKey, DeskKeyKind},
    shell::{to_tex, Tex},
    state::Glove,
};

/// Duração do aparecer e sumir.
const FADE: f32 = 0.22;
/// Previsão relida a cada meia hora (com o Painel aberto).
const WEATHER_EVERY: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub struct Calc {
    display: String,
    acc: Option<f64>,
    op: Option<&'static str>,
    /// O próximo dígito começa um número novo.
    fresh: bool,
    memory: f64,
    pub pressed: Option<&'static str>,
}

pub struct Widget {
    pub kind: WidgetKind,
    pub x: i32,
    pub y: i32,
    /// Texto do Lembrete ou palavra do Dicionário.
    pub text: String,
    /// Cidade do Clima.
    pub city: String,
    pub calc: Calc,
    /// Mês mostrado no Calendário (None = o de hoje).
    pub cal_shown: Option<(i32, u32)>,
    pub tex: Option<(u64, Tex)>,
}

impl Widget {
    fn new(kind: WidgetKind, x: i32, y: i32) -> Self {
        Self { kind, x, y, text: String::new(), city: String::new(), calc: Calc { display: "0".into(), fresh: true, ..Default::default() }, cal_shown: None, tex: None }
    }

    pub fn rect(&self) -> Rectangle<i32, Logical> {
        let (w, h) = self.kind.size();
        Rectangle::new((self.x, self.y).into(), (w as i32, h as i32).into())
    }
}

#[derive(Serialize, Deserialize)]
struct Saved {
    kind: String,
    x: i32,
    y: i32,
    #[serde(default)]
    text: String,
    #[serde(default)]
    city: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SavedAll {
    #[serde(default)]
    widget: Vec<Saved>,
}

/// Previsão do tempo (preenchida pela thread do `curl`).
#[derive(Clone, Default)]
pub struct Weather {
    pub city: String,
    pub now: Option<(i32, u8)>,
    pub hi_lo: Option<(i32, i32)>,
    pub days: Vec<(u32, u8, i32)>,
    pub status: Option<String>,
    /// Muda a cada resposta (para refazer o desenho).
    pub version: u64,
}

#[derive(Default)]
pub struct Dashboard {
    pub open: bool,
    /// Opacidade (0 a 1) e a animação em curso: (de, para, início).
    pub alpha: f32,
    anim: Option<(f32, f32, Instant)>,
    pub bar: bool,
    loaded: bool,
    pub widgets: Vec<Widget>,
    /// Widget que recebe as teclas.
    pub focus: Option<usize>,
    /// Editando a cidade do Clima: (widget, texto).
    pub editing_city: Option<(usize, String)>,
    /// Arrastando um widget: (índice, ponto do mouse dentro dele).
    drag: Option<(usize, Point<f64, Logical>)>,
    pub dim: Option<SolidColorBuffer>,
    pub bar_tex: Option<((i32, u64), Tex)>,
    pub button_tex: Option<((bool, u64), Tex)>,
    pub close_tex: Option<(u64, Tex)>,
    timer_gen: u64,
    pub weather: Arc<Mutex<Weather>>,
    weather_asked: Option<(String, Instant)>,
    cpu_prev: Option<(u64, u64)>,
    pub cpu: f32,
    pub mem: (f32, String),
}

fn saved_path() -> std::path::PathBuf {
    glove_config::dir().join("painel.toml")
}

/// Cidade do fuso horário ("America/Sao_Paulo" → "Sao Paulo").
fn default_city() -> String {
    let tz = std::env::var("TZ").ok().or_else(|| std::fs::read_to_string("/etc/timezone").ok()).or_else(|| {
        std::fs::read_link("/etc/localtime").ok().map(|p| p.to_string_lossy().split("zoneinfo/").nth(1).unwrap_or("").to_string())
    });
    tz.and_then(|t| t.trim().rsplit('/').next().map(|c| c.replace('_', " "))).filter(|c| !c.is_empty()).unwrap_or_else(|| "São Paulo".into())
}

/// "3,5" → 3.5
fn parse_num(s: &str) -> f64 {
    s.replace(',', ".").parse().unwrap_or(0.0)
}

/// Número no formato brasileiro, até 12 algarismos.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "Erro".into();
    }
    if v.abs() >= 1e12 {
        return format!("{v:.4e}").replace('.', ",");
    }
    let mut s = format!("{v:.10}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    // Corta casas decimais que não cabem.
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    if digits > 12 {
        if let Some(dot) = s.find('.') {
            let keep = (12usize.saturating_sub(dot)).max(1);
            s.truncate((dot + 1 + keep).min(s.len()));
            s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
    }
    s.replace('.', ",")
}

impl Calc {
    fn apply(&mut self) {
        let cur = parse_num(&self.display);
        if let (Some(a), Some(op)) = (self.acc, self.op) {
            let r = match op {
                "+" => a + cur,
                "−" => a - cur,
                "×" => a * cur,
                "÷" => a / cur,
                _ => cur,
            };
            self.display = fmt_num(r);
        }
    }

    /// Uma tecla (o rótulo da tecla na calculadora).
    pub fn key(&mut self, k: &'static str) {
        match k {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                if self.fresh || self.display == "0" || self.display == "Erro" {
                    self.display = k.into();
                    self.fresh = false;
                } else if self.display.chars().filter(|c| c.is_ascii_digit()).count() < 12 {
                    self.display.push_str(k);
                }
            }
            "," => {
                if self.fresh || self.display == "Erro" {
                    self.display = "0,".into();
                    self.fresh = false;
                } else if !self.display.contains(',') {
                    self.display.push(',');
                }
            }
            "+" | "−" | "×" | "÷" => {
                if !self.fresh {
                    self.apply();
                }
                self.acc = Some(parse_num(&self.display));
                self.op = Some(k);
                self.fresh = true;
            }
            "=" => {
                self.apply();
                self.acc = None;
                self.op = None;
                self.fresh = true;
            }
            "C" => {
                self.display = "0".into();
                self.acc = None;
                self.op = None;
                self.fresh = true;
            }
            "±" => {
                if let Some(rest) = self.display.strip_prefix('-') {
                    self.display = rest.to_string();
                } else if self.display != "0" {
                    self.display.insert(0, '-');
                }
            }
            "mc" => self.memory = 0.0,
            "m+" => self.memory += parse_num(&self.display),
            "m−" => self.memory -= parse_num(&self.display),
            "mr" => {
                self.display = fmt_num(self.memory);
                self.fresh = true;
            }
            "⌫" => {
                if !self.fresh {
                    self.display.pop();
                    if self.display.is_empty() || self.display == "-" {
                        self.display = "0".into();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Busca a previsão de `city` (Open-Meteo: primeiro a cidade, depois o tempo).
fn fetch_weather(city: String, out: Arc<Mutex<Weather>>) {
    std::thread::spawn(move || {
        let get = |url: &str| -> Option<serde_json::Value> {
            let o = std::process::Command::new("curl").args(["-s", "-m", "12", url]).output().ok()?;
            serde_json::from_slice(&o.stdout).ok()
        };
        let enc: String = city.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        let fail = |msg: &str| {
            let mut w = out.lock().unwrap();
            w.status = Some(msg.into());
            w.now = None;
            w.version += 1;
        };
        let Some(geo) = get(&format!("https://geocoding-api.open-meteo.com/v1/search?name={enc}&count=1&language=pt&format=json")) else {
            return fail("Sem conexão");
        };
        let Some(place) = geo["results"].get(0) else {
            return fail("Cidade não achada");
        };
        let (lat, lon) = (place["latitude"].as_f64().unwrap_or(0.0), place["longitude"].as_f64().unwrap_or(0.0));
        let name = place["name"].as_str().unwrap_or(&city).to_string();
        let url = format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=6");
        let Some(f) = get(&url) else {
            return fail("Sem conexão");
        };
        let num = |v: &serde_json::Value| v.as_f64().map(|x| x.round() as i32);
        let now = num(&f["current"]["temperature_2m"]).map(|t| (t, f["current"]["weather_code"].as_u64().unwrap_or(0) as u8));
        let d = &f["daily"];
        let mut days = Vec::new();
        if let (Some(times), Some(codes), Some(maxs)) = (d["time"].as_array(), d["weather_code"].as_array(), d["temperature_2m_max"].as_array()) {
            for ((t, c), m) in times.iter().zip(codes).zip(maxs) {
                let date = t.as_str().unwrap_or("");
                let p: Vec<i64> = date.split('-').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    days.push((wg::weekday(p[0] as i32, p[1] as u32, p[2] as u32), c.as_u64().unwrap_or(0) as u8, num(m).unwrap_or(0)));
                }
            }
        }
        let hi_lo = num(&d["temperature_2m_max"][0]).zip(num(&d["temperature_2m_min"][0]));
        let mut w = out.lock().unwrap();
        w.city = name;
        w.now = now;
        w.hi_lo = hi_lo;
        w.days = days;
        w.status = if now.is_none() { Some("Sem dados".into()) } else { None };
        w.version += 1;
    });
}

impl Glove {
    fn dash_load(&mut self) {
        if self.dashboard.loaded {
            return;
        }
        self.dashboard.loaded = true;
        let saved: Option<SavedAll> = std::fs::read_to_string(saved_path()).ok().and_then(|t| toml::from_str(&t).ok());
        match saved {
            Some(s) => {
                for w in s.widget {
                    if let Some(k) = WidgetKind::from_id(&w.kind) {
                        let mut wd = Widget::new(k, w.x, w.y);
                        wd.text = w.text;
                        wd.city = w.city;
                        self.dashboard.widgets.push(wd);
                    }
                }
            }
            None => {
                // Primeira vez: Calculadora, Relógio e Calendário em linha, o Clima embaixo (como no Tiger).
                let (sw, sh) = self.output_size();
                let place = |k: WidgetKind, fx: f32, fy: f32| {
                    let (w, h) = k.size();
                    Widget::new(k, (sw as f32 * fx - w / 2.0) as i32, (sh as f32 * fy - h / 2.0) as i32)
                };
                self.dashboard.widgets = vec![
                    place(WidgetKind::Calculator, 0.27, 0.42),
                    place(WidgetKind::Clock, 0.5, 0.4),
                    place(WidgetKind::Calendar, 0.73, 0.42),
                    place(WidgetKind::Weather, 0.5, 0.72),
                ];
            }
        }
        for w in &mut self.dashboard.widgets {
            if w.kind == WidgetKind::Weather && w.city.is_empty() {
                w.city = default_city();
            }
        }
    }

    fn dash_save(&self) {
        let all = SavedAll {
            widget: self.dashboard.widgets.iter().map(|w| Saved { kind: w.kind.id().into(), x: w.x, y: w.y, text: w.text.clone(), city: w.city.clone() }).collect(),
        };
        if let Ok(t) = toml::to_string(&all) {
            let p = saved_path();
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    pub fn toggle_dashboard(&mut self) {
        if self.dashboard.open {
            self.close_dashboard();
        } else {
            self.open_dashboard();
        }
    }

    pub fn open_dashboard(&mut self) {
        if self.dashboard.open || self.greeter || self.lock.is_some() {
            return;
        }
        self.close_menu();
        self.close_search();
        self.dash_load();
        self.dashboard.open = true;
        self.dashboard.anim = Some((self.dashboard.alpha, 1.0, Instant::now()));
        self.dash_tick();
        // Relógio, Monitor e Clima: um redesenho por segundo enquanto o Painel está aberto.
        self.dashboard.timer_gen += 1;
        let generation = self.dashboard.timer_gen;
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(1)), move |_, _, data| {
            let st = &mut data.state;
            if !st.dashboard.open || st.dashboard.timer_gen != generation {
                return TimeoutAction::Drop;
            }
            st.dash_tick();
            TimeoutAction::ToDuration(Duration::from_secs(1))
        });
        self.dirty = true;
    }

    pub fn close_dashboard(&mut self) {
        if !self.dashboard.open {
            return;
        }
        self.dashboard.open = false;
        self.dashboard.bar = false;
        self.dashboard.drag = None;
        self.dashboard.editing_city = None;
        self.dashboard.anim = Some((self.dashboard.alpha, 0.0, Instant::now()));
        self.dash_save();
        self.dirty = true;
    }

    /// Uma vez por segundo: processador e memória, previsão nova, relógio.
    fn dash_tick(&mut self) {
        let has = |k: WidgetKind| self.dashboard.widgets.iter().any(|w| w.kind == k);
        if has(WidgetKind::Monitor) {
            if let Ok(t) = std::fs::read_to_string("/proc/stat") {
                let v: Vec<u64> = t.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
                let total: u64 = v.iter().sum();
                let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
                if let Some((pt, pi)) = self.dashboard.cpu_prev {
                    let (dt, di) = (total.saturating_sub(pt), idle.saturating_sub(pi));
                    if dt > 0 {
                        self.dashboard.cpu = 1.0 - di as f32 / dt as f32;
                    }
                }
                self.dashboard.cpu_prev = Some((total, idle));
            }
            if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
                let get = |k: &str| t.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                let (total, avail) = (get("MemTotal:"), get("MemAvailable:"));
                if total > 0.0 {
                    let used = total - avail;
                    let gb = |k: f64| format!("{:.1}", k / 1024.0 / 1024.0).replace('.', ",");
                    self.dashboard.mem = ((used / total) as f32, format!("{} de {} GB", gb(used), gb(total)));
                }
            }
        }
        if let Some(w) = self.dashboard.widgets.iter().find(|w| w.kind == WidgetKind::Weather) {
            let city = w.city.clone();
            let stale = self.dashboard.weather_asked.as_ref().is_none_or(|(c, t)| *c != city || t.elapsed() > WEATHER_EVERY);
            if stale && !city.is_empty() {
                self.dashboard.weather_asked = Some((city.clone(), Instant::now()));
                {
                    let mut wd = self.dashboard.weather.lock().unwrap();
                    if wd.now.is_none() {
                        wd.status = Some("Buscando…".into());
                        wd.version += 1;
                    }
                }
                fetch_weather(city, self.dashboard.weather.clone());
            }
        }
        self.dirty = true;
    }

    /// O Painel cobre a tela (para o teste de clique e o teclado).
    pub fn dashboard_active(&self) -> bool {
        self.dashboard.open
    }

    // -------------------------------------------------------------------
    // Desenho
    // -------------------------------------------------------------------

    pub fn prepare_dashboard(&mut self) {
        if let Some((from, to, start)) = self.dashboard.anim {
            let t = (start.elapsed().as_secs_f32() / FADE).min(1.0);
            self.dashboard.alpha = from + (to - from) * t * t * (3.0 - 2.0 * t);
            if t >= 1.0 {
                self.dashboard.anim = None;
            } else {
                self.dirty = true;
            }
        }
        if self.dashboard.alpha <= 0.0 {
            return;
        }
        let sc = self.scale() as f32;
        let tgen = self.theme_gen ^ (sc.to_bits() as u64);
        let (sw, sh) = self.output_size();
        match &mut self.dashboard.dim {
            Some(d) => {
                d.resize((sw, sh));
                d.set_color([0.0, 0.0, 0.0, 0.42 * self.dashboard.alpha]);
            }
            None => self.dashboard.dim = Some(SolidColorBuffer::new((sw, sh), [0.0, 0.0, 0.0, 0.42 * self.dashboard.alpha])),
        }
        let (wd, h, m, _, _, _, s) = {
            let n = crate::shell::local_now();
            (n.0, n.4, n.5, n.1, n.2, n.3, n.6)
        };
        let now = crate::shell::local_now();
        let today = (now.3, now.2, now.1);
        let _ = wd;
        let weather = self.dashboard.weather.lock().unwrap().clone();
        // O Relógio mostra a hora daqui: a cidade do fuso (com os acentos do Clima, se for a mesma).
        let tz_city = default_city();
        let n = crate::search::normalize;
        let clock_city = if n(&weather.city) == n(&tz_city) { weather.city.clone() } else { tz_city };
        let focus = self.dashboard.focus;
        let editing = self.dashboard.editing_city.clone();
        let cpu = self.dashboard.cpu;
        let mem = self.dashboard.mem.clone();
        for (i, w) in self.dashboard.widgets.iter_mut().enumerate() {
            use std::hash::{Hash, Hasher};
            let mut k = std::collections::hash_map::DefaultHasher::new();
            tgen.hash(&mut k);
            let focused = focus == Some(i);
            match w.kind {
                WidgetKind::Clock => (h, m, s).hash(&mut k),
                WidgetKind::Calendar => (today, w.cal_shown).hash(&mut k),
                WidgetKind::Calculator => (&w.calc.display, w.calc.pressed, w.calc.memory != 0.0).hash(&mut k),
                WidgetKind::Weather => (weather.version, &w.city, editing.as_ref().filter(|e| e.0 == i).map(|e| &e.1)).hash(&mut k),
                WidgetKind::Note | WidgetKind::Dictionary => (&w.text, focused).hash(&mut k),
                WidgetKind::Monitor => ((cpu * 100.0) as u32, &mem.1).hash(&mut k),
            }
            let key = k.finish();
            if w.tex.as_ref().is_some_and(|t| t.0 == key) {
                continue;
            }
            let c = match w.kind {
                WidgetKind::Clock => wg::clock(&self.fonts, h, m, s, &clock_city, sc),
                WidgetKind::Calendar => wg::calendar(&self.fonts, today, w.cal_shown.unwrap_or((today.0, today.1)), sc),
                WidgetKind::Calculator => wg::calculator(&self.fonts, &w.calc.display, w.calc.pressed, w.calc.memory != 0.0, sc),
                WidgetKind::Weather => {
                    let ed = editing.as_ref().filter(|e| e.0 == i).map(|e| e.1.as_str());
                    let city = if weather.city.is_empty() { w.city.as_str() } else { weather.city.as_str() };
                    let v = wg::WeatherView { city, editing: ed, now: weather.now, hi_lo: weather.hi_lo, days: &weather.days, status: weather.status.as_deref() };
                    wg::weather(&self.fonts, &v, sc)
                }
                WidgetKind::Note => wg::note(&self.fonts, &w.text, focused, sc),
                WidgetKind::Monitor => wg::monitor(&self.fonts, cpu, mem.0, &mem.1, sc),
                WidgetKind::Dictionary => wg::dictionary(&self.fonts, &w.text, focused, sc),
            };
            w.tex = c.map(|c| (key, to_tex(c)));
        }
        let key = (sw, tgen);
        if self.dashboard.bar && self.dashboard.bar_tex.as_ref().map(|t| t.0) != Some(key) {
            self.dashboard.bar_tex = wg::widget_bar(&self.fonts, sw as f32, sc).map(|c| (key, to_tex(c)));
        }
        let key = (self.dashboard.bar, tgen);
        if self.dashboard.button_tex.as_ref().map(|t| t.0) != Some(key) {
            self.dashboard.button_tex = wg::bar_button(self.dashboard.bar, sc).map(|c| (key, to_tex(c)));
        }
        if self.dashboard.close_tex.as_ref().map(|t| t.0) != Some(tgen) {
            self.dashboard.close_tex = wg::close_button(sc).map(|c| (tgen, to_tex(c)));
        }
    }

    /// Onde fica o botão da barra (canto de baixo à esquerda; sobe com a barra aberta).
    pub fn dash_button_pos(&self) -> Point<i32, Logical> {
        let (_, sh) = self.output_size();
        let lift = if self.dashboard.bar { wg::BAR_H as i32 } else { 0 };
        (14, sh - 30 - 14 - lift).into()
    }

    fn dash_close_rect(&self, i: usize) -> Rectangle<i32, Logical> {
        let r = self.dashboard.widgets[i].rect();
        let c = wg::CLOSE as i32;
        Rectangle::new((r.loc.x - c / 2 + 2, r.loc.y - c / 2 + 2).into(), (c, c).into())
    }

    // -------------------------------------------------------------------
    // Mouse e teclado
    // -------------------------------------------------------------------

    pub fn dashboard_button(&mut self, pos: Point<f64, Logical>, pressed: bool) {
        if !pressed {
            for w in &mut self.dashboard.widgets {
                w.calc.pressed = None;
            }
            if self.dashboard.drag.take().is_some() {
                self.dash_save();
            }
            self.dirty = true;
            return;
        }
        let (sw, sh) = self.output_size();
        let b = self.dash_button_pos();
        if Rectangle::<i32, Logical>::new(b, (30, 30).into()).to_f64().contains(pos) {
            self.dashboard.bar = !self.dashboard.bar;
            self.dirty = true;
            return;
        }
        if self.dashboard.bar && pos.y >= (sh as f64 - wg::BAR_H as f64) {
            if let Some(k) = wg::bar_item_at(sw as f32, pos.x as f32, (pos.y - (sh as f64 - wg::BAR_H as f64)) as f32) {
                self.dash_add(k);
            }
            return;
        }
        self.dashboard.editing_city = None;
        let n = self.dashboard.widgets.len();
        if self.dashboard.bar {
            if let Some(i) = (0..n).rev().find(|&i| self.dash_close_rect(i).to_f64().contains(pos)) {
                self.dashboard.widgets.remove(i);
                self.dashboard.focus = None;
                self.dash_save();
                self.dirty = true;
                return;
            }
        }
        let Some(i) = (0..n).rev().find(|&i| self.dashboard.widgets[i].rect().to_f64().contains(pos)) else {
            // Fora dos widgets: fecha o Painel (como no Tiger).
            self.close_dashboard();
            return;
        };
        // O clicado vai para a frente.
        let w = self.dashboard.widgets.remove(i);
        self.dashboard.widgets.push(w);
        let i = self.dashboard.widgets.len() - 1;
        self.dashboard.focus = Some(i);
        let w = &mut self.dashboard.widgets[i];
        let (lx, ly) = ((pos.x - w.x as f64) as f32, (pos.y - w.y as f64) as f32);
        self.dirty = true;
        match w.kind {
            WidgetKind::Calculator => {
                if let Some(k) = wg::calc_hit(lx, ly) {
                    w.calc.pressed = Some(k);
                    w.calc.key(k);
                    return;
                }
            }
            WidgetKind::Calendar => {
                if let Some(h) = wg::calendar_hit(lx, ly) {
                    let now = crate::shell::local_now();
                    let (y, m) = w.cal_shown.unwrap_or((now.3, now.2));
                    w.cal_shown = match h {
                        wg::CalHit::Prev => Some(if m == 1 { (y - 1, 12) } else { (y, m - 1) }),
                        wg::CalHit::Next => Some(if m == 12 { (y + 1, 1) } else { (y, m + 1) }),
                        wg::CalHit::Today => None,
                    };
                    return;
                }
            }
            WidgetKind::Weather => {
                if wg::weather_city_hit(lx, ly) {
                    self.dashboard.editing_city = Some((i, String::new()));
                    return;
                }
            }
            _ => {}
        }
        // O resto do widget: arrastar.
        self.dashboard.drag = Some((i, (lx as f64, ly as f64).into()));
    }

    pub fn dashboard_motion(&mut self, pos: Point<f64, Logical>) {
        if let Some((i, off)) = self.dashboard.drag {
            let (sw, sh) = self.output_size();
            if let Some(w) = self.dashboard.widgets.get_mut(i) {
                let (ww, hh) = w.kind.size();
                w.x = ((pos.x - off.x) as i32).clamp(-(ww as i32) / 2, sw - ww as i32 / 2);
                w.y = ((pos.y - off.y) as i32).clamp(0, sh - hh as i32 / 2);
                self.dirty = true;
            }
        }
    }

    /// Põe um widget novo no Painel: no espaço livre mais perto do meio da tela.
    fn dash_add(&mut self, k: WidgetKind) {
        let (sw, sh) = self.output_size();
        let (w, h) = k.size();
        let (w, h) = (w as i32, h as i32);
        let bottom = sh - wg::BAR_H as i32 - 10;
        let (cx, cy) = ((sw - w) / 2, (bottom + glove_paint::theme::MENUBAR_H - h) / 2);
        let taken: Vec<Rectangle<i32, Logical>> = self
            .dashboard
            .widgets
            .iter()
            .map(|o| {
                let r = o.rect();
                Rectangle::new((r.loc.x - 16, r.loc.y - 16).into(), (r.size.w + 32, r.size.h + 32).into())
            })
            .collect();
        let mut best: Option<((i32, i32), i64)> = None;
        let mut y = glove_paint::theme::MENUBAR_H + 16;
        while y + h <= bottom {
            let mut x = 16;
            while x + w <= sw - 16 {
                let r = Rectangle::new((x, y).into(), (w, h).into());
                if !taken.iter().any(|t| t.overlaps(r)) {
                    let d = ((x - cx) as i64).pow(2) + ((y - cy) as i64).pow(2);
                    if best.is_none_or(|b| d < b.1) {
                        best = Some(((x, y), d));
                    }
                }
                x += 16;
            }
            y += 16;
        }
        // Sem lugar livre: no meio, por cima dos outros.
        let (x, y) = best.map(|b| b.0).unwrap_or((cx, cy));
        let mut wd = Widget::new(k, x, y);
        if k == WidgetKind::Weather {
            wd.city = default_city();
        }
        self.dashboard.widgets.push(wd);
        self.dashboard.focus = Some(self.dashboard.widgets.len() - 1);
        self.dash_save();
        self.dirty = true;
    }

    pub fn dashboard_key(&mut self, k: DeskKey) {
        self.dirty = true;
        // Trocando a cidade do Clima.
        if let Some((i, text)) = &mut self.dashboard.editing_city {
            match k.kind {
                DeskKeyKind::Char(c) if !c.is_control() && text.chars().count() < 40 => text.push(c),
                DeskKeyKind::Back => {
                    text.pop();
                }
                DeskKeyKind::Enter => {
                    let (i, text) = (*i, text.trim().to_string());
                    self.dashboard.editing_city = None;
                    if let Some(w) = self.dashboard.widgets.get_mut(i).filter(|_| !text.is_empty()) {
                        w.city = text;
                        self.dashboard.weather_asked = None;
                        *self.dashboard.weather.lock().unwrap() = Weather { status: Some("Buscando…".into()), ..Default::default() };
                        self.dash_save();
                        self.dash_tick();
                    }
                }
                DeskKeyKind::Esc => self.dashboard.editing_city = None,
                _ => {}
            }
            return;
        }
        if k.kind == DeskKeyKind::Esc {
            self.close_dashboard();
            return;
        }
        let Some(i) = self.dashboard.focus.filter(|&i| i < self.dashboard.widgets.len()) else { return };
        let w = &mut self.dashboard.widgets[i];
        match w.kind {
            WidgetKind::Calculator => {
                let key: Option<&'static str> = match k.kind {
                    DeskKeyKind::Char(c) => match c {
                        '0' => Some("0"),
                        '1' => Some("1"),
                        '2' => Some("2"),
                        '3' => Some("3"),
                        '4' => Some("4"),
                        '5' => Some("5"),
                        '6' => Some("6"),
                        '7' => Some("7"),
                        '8' => Some("8"),
                        '9' => Some("9"),
                        ',' | '.' => Some(","),
                        '+' => Some("+"),
                        '-' => Some("−"),
                        '*' | 'x' => Some("×"),
                        '/' => Some("÷"),
                        '=' => Some("="),
                        'c' | 'C' => Some("C"),
                        _ => None,
                    },
                    DeskKeyKind::Enter => Some("="),
                    DeskKeyKind::Back => Some("⌫"),
                    DeskKeyKind::Del => Some("C"),
                    _ => None,
                };
                if let Some(key) = key {
                    w.calc.key(key);
                }
            }
            WidgetKind::Note => {
                match k.kind {
                    DeskKeyKind::Char(c) if !c.is_control() && w.text.len() < 2000 => w.text.push(c),
                    DeskKeyKind::Enter => w.text.push('\n'),
                    DeskKeyKind::Back => {
                        w.text.pop();
                    }
                    _ => {}
                }
            }
            WidgetKind::Dictionary => match k.kind {
                DeskKeyKind::Char(c) if !c.is_control() && w.text.chars().count() < 60 => w.text.push(c),
                DeskKeyKind::Back => {
                    w.text.pop();
                }
                DeskKeyKind::Enter if !w.text.trim().is_empty() => {
                    let word = w.text.trim().replace('\'', "'\\''");
                    self.close_dashboard();
                    self.spawn(&format!("glove-dicionario '{word}'"));
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(keys: &[&'static str]) -> String {
        let mut c = Calc { display: "0".into(), fresh: true, ..Default::default() };
        for k in keys {
            c.key(k);
        }
        c.display
    }

    #[test]
    fn calculadora() {
        assert_eq!(run(&["1", "2", "+", "3", "="]), "15");
        assert_eq!(run(&["7", "÷", "2", "="]), "3,5");
        assert_eq!(run(&["2", "×", "3", "+", "4", "="]), "10");
        assert_eq!(run(&["1", ",", "5", "×", "2", "="]), "3");
        assert_eq!(run(&["1", "÷", "0", "="]), "Erro");
        assert_eq!(run(&["5", "m+", "C", "mr"]), "5");
    }

    #[test]
    fn numeros() {
        assert_eq!(fmt_num(0.1 + 0.2), "0,3");
        assert_eq!(fmt_num(-2.0), "-2");
        assert_eq!(fmt_num(1.0 / 3.0), "0,3333333333");
    }
}
