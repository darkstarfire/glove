//! Widgets do Painel (como o Dashboard do Tiger), com arte própria: Relógio,
//! Calendário, Calculadora, Clima, Lembrete, Monitor e Dicionário; a barra de
//! widgets e os botões de abrir a barra e de fechar um widget.
//!
//! Cada desenho tem uma margem `M` em volta para a sombra; as funções de clique
//! (`*_hit`) usam coordenadas do conteúdo (sem a margem).

use glove_i18n::tr;
use std::f32::consts::{FRAC_PI_2, TAU};

use tiny_skia::{Color, PathBuilder};

use crate::draw::*;
use crate::font::Fonts;
use crate::icons::{self, Icon};

/// Margem para a sombra em volta de cada widget.
pub const M: f32 = 12.0;
/// Altura da barra de widgets (embaixo da tela).
pub const BAR_H: f32 = 118.0;
/// Botão de fechar no canto do widget (diâmetro).
pub const CLOSE: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Clock,
    Calendar,
    Calculator,
    Weather,
    Note,
    Monitor,
    Dictionary,
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 7] = [Self::Clock, Self::Calendar, Self::Calculator, Self::Weather, Self::Note, Self::Monitor, Self::Dictionary];

    pub fn id(self) -> &'static str {
        match self {
            Self::Clock => "relogio",
            Self::Calendar => "calendario",
            Self::Calculator => "calculadora",
            Self::Weather => "clima",
            Self::Note => "lembrete",
            Self::Monitor => "monitor",
            Self::Dictionary => "dicionario",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Clock => tr!("Relógio"),
            Self::Calendar => tr!("Calendário"),
            Self::Calculator => tr!("Calculadora"),
            Self::Weather => tr!("Clima"),
            Self::Note => tr!("Lembrete"),
            Self::Monitor => tr!("Monitor"),
            Self::Dictionary => tr!("Dicionário"),
        }
    }

    /// Tamanho do conteúdo (sem a margem da sombra).
    pub fn size(self) -> (f32, f32) {
        match self {
            Self::Clock => (150.0, 150.0),
            Self::Calendar => (164.0, 226.0),
            Self::Calculator => (162.0, 236.0),
            Self::Weather => (268.0, 140.0),
            Self::Note => (190.0, 190.0),
            Self::Monitor => (190.0, 100.0),
            Self::Dictionary => (280.0, 62.0),
        }
    }
}

/// Tela com a margem e a sombra do formato do widget já desenhada.
fn base(kind: WidgetKind, radius: f32, scale: f32) -> Option<Canvas> {
    let (w, h) = kind.size();
    let mut c = Canvas::new(w + 2.0 * M, h + 2.0 * M, scale)?;
    for (k, a) in [(7.0, 14), (4.0, 22), (2.0, 34)] {
        c.fill(rrect(M - k * 0.5, M - k * 0.2 + 3.0, w + k, h + k, radius + k * 0.5), &solid(rgba(0, 0, 0, a)));
    }
    Some(c)
}

/// Brilho de vidro na metade de cima.
fn gloss(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, a: u8) {
    c.fill(rrect(x + 1.0, y + 1.0, w - 2.0, h * 0.45, r), &vgrad(y, y + h * 0.45, &[(0.0, rgba(255, 255, 255, a)), (1.0, rgba(255, 255, 255, 0))]));
}

fn centered(c: &mut Canvas, f: &Fonts, t: &str, cx: f32, y: f32, px: f32, bold: bool, col: Color) {
    let w = f.measure(t, px, bold);
    f.draw(c, t, (cx - w / 2.0).round(), y, px, bold, col);
}

/// Botão redondo de fechar (preto com borda branca e ×).
pub fn close_button(scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(CLOSE, CLOSE, scale)?;
    let r = CLOSE / 2.0;
    c.fill(circle(r, r, r - 0.5), &solid(rgba(255, 255, 255, 240)));
    c.fill(circle(r, r, r - 2.5), &solid(rgba(20, 20, 22, 255)));
    let k = r * 0.38;
    for (a, b) in [((r - k, r - k), (r + k, r + k)), ((r + k, r - k), (r - k, r + k))] {
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        c.stroke_round(pb.finish(), &solid(Color::WHITE), 2.2);
    }
    Some(c)
}

/// Botão de abrir (+) ou fechar (×) a barra de widgets, no canto de baixo.
pub fn bar_button(open: bool, scale: f32) -> Option<Canvas> {
    let s = 30.0;
    let mut c = Canvas::new(s, s, scale)?;
    let r = s / 2.0;
    c.fill(circle(r, r, r - 1.0), &solid(rgba(0, 0, 0, 90)));
    c.stroke(circle(r, r, r - 2.0), &solid(rgba(255, 255, 255, 230)), 2.0);
    let k = r * 0.42;
    let lines = if open { [((r - k * 0.8, r - k * 0.8), (r + k * 0.8, r + k * 0.8)), ((r + k * 0.8, r - k * 0.8), (r - k * 0.8, r + k * 0.8))] } else { [((r - k, r), (r + k, r)), ((r, r - k), (r, r + k))] };
    for (a, b) in lines {
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        c.stroke_round(pb.finish(), &solid(Color::WHITE), 2.4);
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Relógio
// ---------------------------------------------------------------------------

pub fn clock(f: &Fonts, h: u32, m: u32, s: u32, label: &str, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Clock;
    let (w, hh) = kind.size();
    let mut c = base(kind, 16.0, scale)?;
    let (x, y) = (M, M);
    c.fill(rrect(x, y, w, hh, 16.0), &vgrad(y, y + hh, &[(0.0, rgba(70, 70, 74, 255)), (0.5, rgba(24, 24, 26, 255)), (1.0, rgba(10, 10, 12, 255))]));
    c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, hh - 1.0, 16.0), &solid(rgba(0, 0, 0, 200)), 1.0);
    let (cx, cy, r) = (x + w / 2.0, y + 70.0, 56.0);
    c.fill(circle(cx, cy, r + 3.0), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(200, 202, 208, 255)), (0.5, rgba(110, 112, 120, 255)), (1.0, rgba(190, 192, 200, 255))]));
    c.fill(circle(cx, cy, r), &radial(cx, cy + r * 0.3, r * 1.2, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(222, 224, 230, 255))]));
    for i in 0..60 {
        let a = i as f32 / 60.0 * TAU;
        let (r0, wd) = if i % 5 == 0 { (r * 0.84, 2.0) } else { (r * 0.91, 1.0) };
        let mut pb = PathBuilder::new();
        pb.move_to(cx + a.cos() * r0, cy + a.sin() * r0);
        pb.line_to(cx + a.cos() * r * 0.96, cy + a.sin() * r * 0.96);
        c.stroke(pb.finish(), &solid(rgba(40, 40, 46, 255)), wd);
    }
    for n in 1..=12 {
        let a = n as f32 / 12.0 * TAU - FRAC_PI_2;
        let t = n.to_string();
        let tw = f.measure(&t, 11.0, false);
        f.draw(&mut c, &t, cx + a.cos() * r * 0.68 - tw / 2.0, cy + a.sin() * r * 0.68 + 4.0, 11.0, false, rgba(30, 30, 34, 255));
    }
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * TAU - FRAC_PI_2;
    let ma = (m as f32 + s as f32 / 60.0) / 60.0 * TAU - FRAC_PI_2;
    let sa = s as f32 / 60.0 * TAU - FRAC_PI_2;
    for (a, len, wd, col) in [(ha, r * 0.5, 4.0, rgba(20, 20, 24, 255)), (ma, r * 0.78, 2.6, rgba(20, 20, 24, 255)), (sa, r * 0.86, 1.2, rgba(210, 40, 30, 255))] {
        let mut pb = PathBuilder::new();
        pb.move_to(cx - a.cos() * 6.0, cy - a.sin() * 6.0);
        pb.line_to(cx + a.cos() * len, cy + a.sin() * len);
        c.stroke_round(pb.finish(), &solid(col), wd);
    }
    c.fill(circle(cx, cy, 3.0), &solid(rgba(210, 40, 30, 255)));
    c.fill(oval(cx - r * 0.75, cy - r * 0.95, r * 1.5, r * 0.85), &vgrad(cy - r, cy - r * 0.1, &[(0.0, rgba(255, 255, 255, 120)), (1.0, rgba(255, 255, 255, 0))]));
    let label = f.ellipsize(&label.to_uppercase(), 10.0, true, w - 16.0);
    centered(&mut c, f, &label, cx, y + hh - 9.0, 10.0, true, rgba(235, 235, 240, 255));
    Some(c)
}

// ---------------------------------------------------------------------------
// Calendário
// ---------------------------------------------------------------------------

const WEEKDAYS: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
const MONTHS: [&str; 12] = ["JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ"];

/// Dias do mês (mês 1–12).
pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Dia da semana (0 = domingo) de y-m-d.
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i32).rem_euclid(7)) as u32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CalHit {
    Prev,
    Today,
    Next,
}

/// Hoje: (ano, mês, dia); `shown`: (ano, mês) mostrado na grade.
pub fn calendar(f: &Fonts, today: (i32, u32, u32), shown: (i32, u32), scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Calendar;
    let (w, h) = kind.size();
    let mut c = base(kind, 12.0, scale)?;
    let (x, y) = (M, M);
    let red = rgba(196, 30, 26, 255);
    c.fill(rrect(x, y, w, h, 12.0), &vgrad(y, y + h, &[(0.0, rgba(216, 50, 40, 255)), (1.0, rgba(150, 18, 16, 255))]));
    c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 12.0), &solid(rgba(90, 0, 0, 200)), 1.0);
    // Duas folhas brancas: dia da semana e mês; o dia grande.
    let (ty, th) = (y + 10.0, 70.0);
    let pw = (w - 26.0) / 2.0;
    for (k, px) in [(0, x + 9.0), (1, x + 17.0 + pw)] {
        c.fill(rrect(px, ty, pw, th, 5.0), &vgrad(ty, ty + th, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(226, 226, 230, 255))]));
        c.fill(rrect(px + pw / 2.0 - 12.0, ty - 4.0, 24.0, 6.0, 3.0), &solid(rgba(120, 10, 10, 255)));
        let _ = k;
    }
    let wd = weekday(today.0, today.1, today.2);
    f.draw(&mut c, glove_i18n::tr(WEEKDAYS[wd as usize]), x + 15.0, ty + 30.0, 17.0, true, red);
    f.draw(&mut c, glove_i18n::tr(MONTHS[(today.1 - 1) as usize]), x + 15.0, ty + 56.0, 22.0, true, red);
    centered(&mut c, f, &today.2.to_string(), x + 17.0 + pw * 1.5, ty + 56.0, 42.0, true, red);
    // Grade do mês mostrado.
    let (gy, cw) = (y + 96.0, (w - 16.0) / 7.0);
    for (i, d) in ["D", "S", "T", "Q", "Q", "S", "S"].iter().enumerate() {
        centered(&mut c, f, d, x + 8.0 + cw * (i as f32 + 0.5), gy, 10.0, true, rgba(255, 214, 120, 255));
    }
    let (sy, sm) = shown;
    let first = weekday(sy, sm, 1);
    let n = days_in_month(sy, sm);
    for d in 1..=n {
        let k = first + d - 1;
        let (col, row) = (k % 7, k / 7);
        let cx = x + 8.0 + cw * (col as f32 + 0.5);
        let by = gy + 16.0 + row as f32 * 15.0;
        let is_today = (sy, sm, d) == today;
        if is_today {
            c.fill(rrect(cx - 9.0, by - 11.0, 18.0, 14.0, 3.0), &solid(Color::WHITE));
        }
        centered(&mut c, f, &d.to_string(), cx, by, 10.5, true, if is_today { red } else { Color::WHITE });
    }
    // Mês e ano mostrados + setas.
    let fy = y + h - 22.0;
    c.fill(rrect(x + 8.0, fy, w - 16.0, 16.0, 8.0), &solid(rgba(110, 10, 10, 180)));
    let label = format!("{} {sy}", glove_i18n::tr(MONTHS[(sm - 1) as usize]));
    centered(&mut c, f, &label, x + w / 2.0, fy + 12.0, 10.0, true, Color::WHITE);
    let tri = |c: &mut Canvas, cx: f32, left: bool| {
        let mut pb = PathBuilder::new();
        let d = if left { -1.0 } else { 1.0 };
        pb.move_to(cx + 4.0 * d, fy + 8.0);
        pb.line_to(cx - 3.0 * d, fy + 4.0);
        pb.line_to(cx - 3.0 * d, fy + 12.0);
        pb.close();
        c.fill(pb.finish(), &solid(Color::WHITE));
    };
    tri(&mut c, x + 22.0, true);
    tri(&mut c, x + w - 22.0, false);
    Some(c)
}

pub fn calendar_hit(lx: f32, ly: f32) -> Option<CalHit> {
    let (w, h) = WidgetKind::Calendar.size();
    if ly < h - 26.0 || ly > h - 4.0 {
        return None;
    }
    if lx < 40.0 {
        Some(CalHit::Prev)
    } else if lx > w - 40.0 {
        Some(CalHit::Next)
    } else {
        Some(CalHit::Today)
    }
}

// ---------------------------------------------------------------------------
// Calculadora
// ---------------------------------------------------------------------------

/// Teclas: (rótulo, coluna, linha, largura em colunas, altura em linhas).
pub const CALC_KEYS: [(&str, u32, u32, u32, u32); 21] = [
    ("mc", 0, 0, 1, 1),
    ("m+", 1, 0, 1, 1),
    ("m−", 2, 0, 1, 1),
    ("mr", 3, 0, 1, 1),
    ("C", 0, 1, 1, 1),
    ("±", 1, 1, 1, 1),
    ("÷", 2, 1, 1, 1),
    ("×", 3, 1, 1, 1),
    ("7", 0, 2, 1, 1),
    ("8", 1, 2, 1, 1),
    ("9", 2, 2, 1, 1),
    ("−", 3, 2, 1, 1),
    ("4", 0, 3, 1, 1),
    ("5", 1, 3, 1, 1),
    ("6", 2, 3, 1, 1),
    ("+", 3, 3, 1, 1),
    ("1", 0, 4, 1, 1),
    ("2", 1, 4, 1, 1),
    ("3", 2, 4, 1, 1),
    ("=", 3, 4, 1, 2),
    ("0", 0, 5, 2, 1),
];
/// A vírgula fica ao lado do zero.
pub const CALC_COMMA: (&str, u32, u32, u32, u32) = (",", 2, 5, 1, 1);

/// Lado da tecla, passo entre colunas e entre linhas, começo da grade.
const CK: f32 = 26.0;
const COL: f32 = 36.0;
const ROW: f32 = 28.0;
const CX0: f32 = 18.0;
const CY0: f32 = 64.0;

fn calc_key_rect(k: &(&str, u32, u32, u32, u32)) -> (f32, f32, f32, f32) {
    let (_, col, row, cw, rh) = *k;
    (CX0 + col as f32 * COL, CY0 + row as f32 * ROW, cw as f32 * CK + (cw - 1) as f32 * (COL - CK), rh as f32 * CK + (rh - 1) as f32 * (ROW - CK))
}

fn calc_keys() -> impl Iterator<Item = (&'static str, u32, u32, u32, u32)> {
    CALC_KEYS.into_iter().chain(std::iter::once(CALC_COMMA))
}

/// Tecla da calculadora sob o ponto (rótulo).
pub fn calc_hit(lx: f32, ly: f32) -> Option<&'static str> {
    calc_keys().find(|k| {
        let (x, y, w, h) = calc_key_rect(k);
        lx >= x && lx <= x + w && ly >= y && ly <= y + h
    }).map(|k| k.0)
}

pub fn calculator(f: &Fonts, display: &str, pressed: Option<&str>, memory: bool, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Calculator;
    let (w, h) = kind.size();
    let mut c = base(kind, 16.0, scale)?;
    let (x, y) = (M, M);
    // Moldura laranja de gel e o corpo cinza claro.
    c.fill(rrect(x, y, w, h, 16.0), &vgrad(y, y + h, &[(0.0, rgba(255, 170, 60, 255)), (1.0, rgba(232, 110, 10, 255))]));
    c.fill(rrect(x + 6.0, y + 6.0, w - 12.0, h - 12.0, 11.0), &vgrad(y, y + h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(196, 196, 200, 255))]));
    gloss(&mut c, x, y, w, h, 16.0, 70);
    // Visor
    let (lx, ly, lw, lh) = (x + 14.0, y + 14.0, w - 28.0, 40.0);
    c.fill(rrect(lx, ly, lw, lh, 4.0), &vgrad(ly, ly + lh, &[(0.0, rgba(150, 156, 150, 255)), (1.0, rgba(196, 204, 196, 255))]));
    c.stroke(rrect(lx + 0.5, ly + 0.5, lw - 1.0, lh - 1.0, 4.0), &solid(rgba(80, 84, 80, 200)), 1.0);
    let t = f.ellipsize(display, 24.0, false, lw - 14.0);
    let tw = f.measure(&t, 24.0, false);
    f.draw(&mut c, &t, lx + lw - 8.0 - tw, ly + 30.0, 24.0, false, rgba(30, 34, 30, 255));
    if memory {
        f.draw(&mut c, "M", lx + 5.0, ly + 13.0, 9.0, true, rgba(30, 34, 30, 255));
    }
    for k in calc_keys() {
        let (kx, ky, kw, kh) = calc_key_rect(&k);
        let (kx, ky) = (x + kx, y + ky);
        let down = pressed == Some(k.0);
        let r = kw.min(kh) / 2.0;
        c.fill(rrect(kx, ky + 1.0, kw, kh, r), &solid(rgba(0, 0, 0, 50)));
        let (top, bot) = if down { (rgba(170, 170, 176, 255), rgba(214, 214, 218, 255)) } else { (rgba(255, 255, 255, 255), rgba(218, 218, 222, 255)) };
        c.fill(rrect(kx, ky, kw, kh, r), &vgrad(ky, ky + kh, &[(0.0, top), (1.0, bot)]));
        c.stroke(rrect(kx + 0.5, ky + 0.5, kw - 1.0, kh - 1.0, r), &solid(rgba(120, 120, 128, 200)), 1.0);
        let px = if k.0.len() > 1 && k.0.chars().count() > 1 { 10.0 } else { 14.0 };
        // A tecla decimal mostra o separador do idioma (o identificador continua ",").
        let shown = if k.0 == "," { glove_i18n::virgula() } else { k.0 };
        let tw = f.measure(shown, px, true);
        f.draw(&mut c, shown, (kx + (kw - tw) / 2.0).round(), (ky + kh / 2.0 + px * 0.36).round(), px, true, rgba(60, 60, 66, 255));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Clima
// ---------------------------------------------------------------------------

/// Desenho do tempo pelo código WMO (0 sol, 1–3 nuvens, 45 névoa, 51–67 chuva,
/// 71–77 neve, 80–82 pancadas, 95+ trovoada).
pub fn weather_icon(c: &mut Canvas, x: f32, y: f32, s: f32, code: u8) {
    let sun = |c: &mut Canvas, cx: f32, cy: f32, r: f32| {
        c.fill(circle(cx, cy, r * 1.5), &radial(cx, cy, r * 1.5, &[(0.0, rgba(255, 230, 120, 140)), (1.0, rgba(255, 200, 60, 0))]));
        c.fill(circle(cx, cy, r), &radial(cx - r * 0.3, cy - r * 0.3, r * 1.3, &[(0.0, rgba(255, 252, 200, 255)), (0.6, rgba(255, 206, 40, 255)), (1.0, rgba(245, 150, 10, 255))]));
    };
    let cloud = |c: &mut Canvas, cx: f32, cy: f32, r: f32, dark: bool| {
        let (a, b) = if dark { (rgba(170, 176, 190, 255), rgba(110, 116, 132, 255)) } else { (rgba(255, 255, 255, 255), rgba(200, 208, 222, 255)) };
        let g = vgrad(cy - r, cy + r * 0.8, &[(0.0, a), (1.0, b)]);
        c.fill(circle(cx - r * 0.55, cy + r * 0.1, r * 0.55), &g);
        c.fill(circle(cx + r * 0.1, cy - r * 0.2, r * 0.75), &g);
        c.fill(circle(cx + r * 0.75, cy + r * 0.15, r * 0.5), &g);
        c.fill(rrect(cx - r * 1.1, cy + r * 0.05, r * 2.35, r * 0.62, r * 0.3), &g);
    };
    let (cx, cy, r) = (x + s / 2.0, y + s / 2.0, s * 0.3);
    match code {
        0 => sun(c, cx, cy, r),
        1 | 2 => {
            sun(c, cx - r * 0.4, cy - r * 0.4, r * 0.85);
            cloud(c, cx + r * 0.3, cy + r * 0.45, r * 0.95, false);
        }
        3 | 45 | 48 => cloud(c, cx, cy, r * 1.1, code != 3),
        71..=77 | 85 | 86 => {
            cloud(c, cx, cy - r * 0.3, r, false);
            for k in 0..3 {
                c.fill(circle(cx - r * 0.6 + k as f32 * r * 0.6, cy + r * 1.0, s * 0.04), &solid(Color::WHITE));
            }
        }
        95..=99 => {
            cloud(c, cx, cy - r * 0.3, r, true);
            let mut pb = PathBuilder::new();
            pb.move_to(cx + r * 0.1, cy + r * 0.3);
            pb.line_to(cx - r * 0.35, cy + r * 0.95);
            pb.line_to(cx + r * 0.05, cy + r * 0.9);
            pb.line_to(cx - r * 0.2, cy + r * 1.5);
            pb.line_to(cx + r * 0.45, cy + r * 0.75);
            pb.line_to(cx + r * 0.05, cy + r * 0.8);
            pb.close();
            c.fill(pb.finish(), &solid(rgba(255, 220, 60, 255)));
        }
        _ => {
            // Chuva e garoa.
            cloud(c, cx, cy - r * 0.3, r, code >= 61);
            for k in 0..3 {
                let dx = cx - r * 0.6 + k as f32 * r * 0.6;
                let mut pb = PathBuilder::new();
                pb.move_to(dx, cy + r * 0.65);
                pb.line_to(dx - r * 0.2, cy + r * 1.2);
                c.stroke_round(pb.finish(), &solid(rgba(120, 190, 255, 255)), (s * 0.05).max(1.2));
            }
        }
    }
}

/// O que o widget do Clima mostra.
pub struct WeatherView<'a> {
    pub city: &'a str,
    /// Editando o nome da cidade (texto digitado).
    pub editing: Option<&'a str>,
    /// Agora: (temperatura, código), e máxima/mínima de hoje.
    pub now: Option<(i32, u8)>,
    pub hi_lo: Option<(i32, i32)>,
    /// Próximos dias: (dia da semana 0–6, código, máxima).
    pub days: &'a [(u32, u8, i32)],
    /// Aviso no lugar dos dados ("Buscando…", "Sem conexão").
    pub status: Option<&'a str>,
}

pub fn weather(f: &Fonts, v: &WeatherView, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Weather;
    let (w, h) = kind.size();
    let mut c = base(kind, 12.0, scale)?;
    let (x, y) = (M, M);
    c.fill(rrect(x, y, w, h, 12.0), &vgrad(y, y + h, &[(0.0, rgba(120, 170, 225, 245)), (0.55, rgba(70, 120, 190, 245)), (1.0, rgba(44, 86, 150, 245))]));
    c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 12.0), &solid(rgba(230, 240, 255, 200)), 1.0);
    gloss(&mut c, x, y, w, 80.0, 12.0, 60);
    // Cidade (clique para trocar).
    match v.editing {
        Some(t) => {
            c.fill(rrect(x + 10.0, y + 12.0, 118.0, 20.0, 3.0), &solid(Color::WHITE));
            let t2 = f.ellipsize(t, 12.0, false, 110.0);
            f.draw(&mut c, &t2, x + 14.0, y + 26.0, 12.0, false, rgba(20, 20, 20, 255));
            let tw = f.measure(&t2, 12.0, false);
            c.fill_rect(x + 15.0 + tw, y + 15.0, 1.0, 14.0, &solid(rgba(20, 20, 20, 255)));
            f.draw(&mut c, tr!("Digite a cidade e Enter"), x + 12.0, y + 46.0, 9.5, false, rgba(235, 242, 255, 230));
        }
        None => {
            let city = f.ellipsize(v.city, 13.0, true, 120.0);
            f.draw(&mut c, &city, x + 14.0, y + 28.0, 13.0, true, Color::WHITE);
            if let Some((hi, lo)) = v.hi_lo {
                f.draw(&mut c, &tr!("Máx {}°   Mín {}°", hi, lo), x + 14.0, y + 46.0, 10.0, false, rgba(235, 242, 255, 240));
            }
        }
    }
    match (v.now, v.status) {
        (Some((t, code)), _) => {
            weather_icon(&mut c, x + 118.0, y + 2.0, 70.0, code);
            let s = format!("{t}°");
            let tw = f.measure(&s, 40.0, false);
            f.draw(&mut c, &s, x + w - 14.0 - tw, y + 56.0, 40.0, false, Color::WHITE);
        }
        (None, Some(st)) => {
            let tw = f.measure(st, 11.0, false);
            f.draw(&mut c, st, x + w - 14.0 - tw, y + 46.0, 11.0, false, Color::WHITE);
        }
        _ => {}
    }
    // Faixa dos próximos dias.
    let by = y + 76.0;
    c.fill(rrect4(x + 1.0, by, w - 2.0, h - 77.0, [0.0, 0.0, 11.0, 11.0]), &solid(rgba(20, 50, 100, 110)));
    c.hline(x + 1.0, by, w - 2.0, rgba(200, 220, 255, 120));
    let n = v.days.len().clamp(1, 6) as f32;
    let cw = (w - 8.0) / n;
    for (i, (wd, code, max)) in v.days.iter().take(6).enumerate() {
        let cx = x + 4.0 + cw * (i as f32 + 0.5);
        centered(&mut c, f, &glove_i18n::tr(WEEKDAYS[*wd as usize % 7]).to_uppercase(), cx, by + 15.0, 10.0, true, Color::WHITE);
        weather_icon(&mut c, cx - 13.0, by + 17.0, 26.0, *code);
        centered(&mut c, f, &format!("{max}°"), cx, by + 58.0, 12.0, true, Color::WHITE);
    }
    Some(c)
}

/// Clique no nome da cidade (para trocar).
pub fn weather_city_hit(lx: f32, ly: f32) -> bool {
    lx < 135.0 && ly < 52.0
}

// ---------------------------------------------------------------------------
// Lembrete
// ---------------------------------------------------------------------------

pub fn note(f: &Fonts, text: &str, focused: bool, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Note;
    let (w, h) = kind.size();
    let mut c = base(kind, 2.0, scale)?;
    let (x, y) = (M, M);
    c.fill(rrect(x, y, w, h, 2.0), &vgrad(y, y + h, &[(0.0, rgba(255, 250, 170, 255)), (1.0, rgba(250, 236, 120, 255))]));
    // Dobra no canto de baixo.
    c.fill(rrect(x + w - 22.0, y + h - 22.0, 22.0, 22.0, 0.0), &linear(x + w - 22.0, y + h - 22.0, x + w, y + h, &[(0.0, rgba(250, 236, 120, 0)), (0.5, rgba(220, 200, 80, 120)), (1.0, rgba(190, 170, 60, 160))]));
    let px = 13.0;
    let max = w - 24.0;
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        lines.extend(crate::chrome::wrap_lines(f, para, px, false, max));
    }
    if text.is_empty() && !focused {
        f.draw(&mut c, tr!("Escreva aqui…"), x + 12.0, y + 26.0, px, false, rgba(150, 140, 80, 255));
    }
    let shown = ((h - 24.0) / (px + 4.0)) as usize;
    let start = lines.len().saturating_sub(shown);
    for (k, l) in lines[start..].iter().enumerate() {
        f.draw(&mut c, l, x + 12.0, y + 26.0 + k as f32 * (px + 4.0), px, false, rgba(40, 36, 20, 255));
    }
    if focused {
        let last = lines.last().cloned().unwrap_or_default();
        let k = (lines.len().max(1) - 1 - start) as f32;
        let lw = f.measure(&last, px, false);
        c.fill_rect((x + 12.0 + lw + 1.0).round(), y + 14.0 + k * (px + 4.0), 1.0, px + 2.0, &solid(rgba(40, 36, 20, 255)));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Monitor
// ---------------------------------------------------------------------------

/// cpu e mem de 0 a 1; `mem_text` = "3,1 de 7,6 GB".
pub fn monitor(f: &Fonts, cpu: f32, mem: f32, mem_text: &str, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Monitor;
    let (w, h) = kind.size();
    let mut c = base(kind, 12.0, scale)?;
    let (x, y) = (M, M);
    c.fill(rrect(x, y, w, h, 12.0), &vgrad(y, y + h, &[(0.0, rgba(80, 84, 92, 250)), (1.0, rgba(30, 32, 38, 250))]));
    c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 12.0), &solid(rgba(0, 0, 0, 200)), 1.0);
    gloss(&mut c, x, y, w, h, 12.0, 40);
    let bar = |c: &mut Canvas, by: f32, v: f32, col: Color| {
        let (bx, bw, bh) = (x + 14.0, w - 28.0, 10.0);
        c.fill(rrect(bx, by, bw, bh, 5.0), &solid(rgba(0, 0, 0, 140)));
        let fw = (bw * v.clamp(0.0, 1.0)).max(bh);
        c.fill(rrect(bx, by, fw, bh, 5.0), &vgrad(by, by + bh, &[(0.0, lighten(col, 0.35)), (1.0, col)]));
    };
    f.draw(&mut c, tr!("Processador"), x + 14.0, y + 22.0, 11.0, true, Color::WHITE);
    let t = format!("{:.0}%", cpu * 100.0);
    let tw = f.measure(&t, 11.0, false);
    f.draw(&mut c, &t, x + w - 14.0 - tw, y + 22.0, 11.0, false, rgba(220, 224, 230, 255));
    bar(&mut c, y + 28.0, cpu, rgba(60, 200, 80, 255));
    f.draw(&mut c, tr!("Memória"), x + 14.0, y + 64.0, 11.0, true, Color::WHITE);
    let tw = f.measure(mem_text, 11.0, false);
    f.draw(&mut c, mem_text, x + w - 14.0 - tw, y + 64.0, 11.0, false, rgba(220, 224, 230, 255));
    bar(&mut c, y + 70.0, mem, rgba(70, 140, 235, 255));
    Some(c)
}

// ---------------------------------------------------------------------------
// Dicionário
// ---------------------------------------------------------------------------

pub fn dictionary(f: &Fonts, text: &str, focused: bool, scale: f32) -> Option<Canvas> {
    let kind = WidgetKind::Dictionary;
    let (w, h) = kind.size();
    let mut c = base(kind, 12.0, scale)?;
    let (x, y) = (M, M);
    c.fill(rrect(x, y, w, h, 12.0), &vgrad(y, y + h, &[(0.0, rgba(110, 70, 40, 250)), (1.0, rgba(60, 34, 18, 250))]));
    c.stroke(rrect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 12.0), &solid(rgba(30, 14, 4, 220)), 1.0);
    gloss(&mut c, x, y, w, h, 12.0, 50);
    icons::draw(&mut c, f, Icon::Dictionary, x + 10.0, y + 12.0, 38.0, None);
    let (fx, fy, fw, fh) = (x + 58.0, y + 20.0, w - 72.0, 22.0);
    c.fill(rrect(fx, fy, fw, fh, fh / 2.0), &solid(Color::WHITE));
    c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, fh / 2.0), &solid(rgba(40, 20, 6, 200)), 1.0);
    if text.is_empty() && !focused {
        f.draw(&mut c, tr!("Procurar palavra"), fx + 12.0, fy + 15.5, 12.0, false, rgba(150, 150, 150, 255));
    } else {
        let t = f.ellipsize(text, 12.0, false, fw - 24.0);
        f.draw(&mut c, &t, fx + 12.0, fy + 15.5, 12.0, false, rgba(20, 20, 20, 255));
        if focused {
            let tw = f.measure(&t, 12.0, false);
            c.fill_rect((fx + 13.0 + tw).round(), fy + 4.0, 1.0, fh - 8.0, &solid(rgba(20, 20, 20, 255)));
        }
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Barra de widgets
// ---------------------------------------------------------------------------

const BAR_ITEM: f32 = 104.0;

/// Item da barra sob x (relativo à barra de largura `w`).
pub fn bar_item_at(w: f32, lx: f32, ly: f32) -> Option<WidgetKind> {
    let n = WidgetKind::ALL.len() as f32;
    let x0 = (w - n * BAR_ITEM) / 2.0;
    if ly < 8.0 || lx < x0 {
        return None;
    }
    WidgetKind::ALL.get(((lx - x0) / BAR_ITEM) as usize).copied()
}

/// Ícone de cada widget na barra.
fn kind_icon(c: &mut Canvas, f: &Fonts, k: WidgetKind, x: f32, y: f32, s: f32) {
    match k {
        WidgetKind::Clock => icons::draw(c, f, Icon::Clock(10, 9), x, y, s, None),
        WidgetKind::Calendar => icons::draw(c, f, Icon::Calendar, x, y, s, None),
        WidgetKind::Calculator => icons::draw(c, f, Icon::Calculator, x, y, s, None),
        WidgetKind::Weather => weather_icon(c, x, y, s, 2),
        WidgetKind::Note => icons::draw(c, f, Icon::Notes, x, y, s, None),
        WidgetKind::Monitor => icons::draw(c, f, Icon::Monitor, x, y, s, None),
        WidgetKind::Dictionary => icons::draw(c, f, Icon::Dictionary, x, y, s, None),
    }
}

/// Barra embaixo da tela com os widgets que dá para pôr no Painel.
pub fn widget_bar(f: &Fonts, w: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, BAR_H, scale)?;
    c.fill_rect(0.0, 0.0, w, BAR_H, &vgrad(0.0, BAR_H, &[(0.0, rgba(214, 214, 218, 255)), (1.0, rgba(186, 186, 192, 255))]));
    // Trama de pontinhos.
    let mut yy = 3.0;
    let mut row = 0;
    while yy < BAR_H {
        let mut xx = if row % 2 == 0 { 2.0 } else { 5.0 };
        while xx < w {
            c.fill_cell(xx, yy, 1.0, 1.0, rgba(150, 150, 156, 160));
            xx += 6.0;
        }
        yy += 3.0;
        row += 1;
    }
    c.hline(0.0, 0.0, w, rgba(120, 120, 126, 255));
    c.hline(0.0, 1.0, w, rgba(250, 250, 252, 255));
    let n = WidgetKind::ALL.len() as f32;
    let x0 = (w - n * BAR_ITEM) / 2.0;
    for (i, k) in WidgetKind::ALL.iter().enumerate() {
        let cx = x0 + BAR_ITEM * (i as f32 + 0.5);
        kind_icon(&mut c, f, *k, cx - 32.0, 14.0, 64.0);
        centered(&mut c, f, k.name(), cx, 100.0, 12.0, false, rgba(40, 40, 46, 255));
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas() {
        assert_eq!(weekday(2026, 11, 5), 4); // quinta
        assert_eq!(weekday(2000, 1, 1), 6); // sábado
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
    }

    #[test]
    fn teclas_da_calculadora() {
        let (x, y, w, h) = calc_key_rect(&CALC_KEYS[8]); // 7
        assert_eq!(calc_hit(x + w / 2.0, y + h / 2.0), Some("7"));
        let (x, y, _, _) = calc_key_rect(&CALC_COMMA);
        assert_eq!(calc_hit(x + 3.0, y + 3.0), Some(","));
    }
}
