//! Calculadora do Glove.

use glove_config::tr;
use glove_kit::{
    paint::draw::*,
    run,
    ui::Key,
    App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    /// xʸ
    Pow,
}

struct Calc {
    display: String,
    acc: Option<f64>,
    op: Option<Op>,
    /// O próximo dígito começa um número novo.
    fresh: bool,
    error: bool,
    /// Memória (MC, M+, M−, MR).
    memory: f64,
    /// Modo científico (teclas de funções à esquerda) e ângulos em graus.
    scientific: bool,
    degrees: bool,
}

/// Número no formato brasileiro, com no máximo 12 dígitos significativos.
fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "Erro".into();
    }
    if v != 0.0 && (v.abs() >= 1e12 || v.abs() < 1e-9) {
        return format!("{v:.6e}").replace('.', ",");
    }
    let int_digits = (v.abs().log10().floor() as i32 + 1).max(1);
    let dec = (12 - int_digits).clamp(0, 10) as usize;
    let mut s = format!("{v:.dec$}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    s.replace('.', ",")
}

impl Calc {
    fn new() -> Self {
        Self { display: "0".into(), acc: None, op: None, fresh: true, error: false, memory: 0.0, scientific: false, degrees: true }
    }

    fn value(&self) -> f64 {
        self.display.replace(',', ".").parse().unwrap_or(0.0)
    }

    fn digit(&mut self, d: char) {
        if self.error {
            *self = Self::new();
        }
        if self.fresh {
            self.display = if d == ',' { "0,".into() } else { d.to_string() };
            self.fresh = false;
            return;
        }
        if d == ',' && self.display.contains(',') {
            return;
        }
        if self.display.chars().filter(|c| c.is_ascii_digit()).count() >= 12 {
            return;
        }
        if self.display == "0" && d != ',' {
            self.display = d.to_string();
        } else {
            self.display.push(d);
        }
    }

    fn apply(&mut self) {
        let b = self.value();
        if let (Some(a), Some(op)) = (self.acc, self.op) {
            let r = match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => a / b,
                Op::Pow => a.powf(b),
            };
            self.error = !r.is_finite();
            self.display = fmt(r);
            self.acc = Some(r);
        } else {
            self.acc = Some(b);
        }
    }

    fn operator(&mut self, op: Op) {
        if self.error {
            return;
        }
        if !self.fresh || self.acc.is_none() {
            self.apply();
        }
        self.op = Some(op);
        self.fresh = true;
    }

    fn equals(&mut self) {
        if self.op.is_some() && !self.error {
            self.apply();
            self.op = None;
            self.acc = None;
            self.fresh = true;
        }
    }

    /// C limpa a conta, mas não a memória (essa só com MC) nem o modo.
    fn clear(&mut self) {
        *self = Self { memory: self.memory, scientific: self.scientific, degrees: self.degrees, ..Self::new() };
    }

    /// Função de uma tecla científica aplicada ao número do visor.
    fn function(&mut self, f: &str) {
        if self.error {
            return;
        }
        let v = self.value();
        let to_rad = |x: f64| if self.degrees { x.to_radians() } else { x };
        let from_rad = |x: f64| if self.degrees { x.to_degrees() } else { x };
        let r = match f {
            "sin" => to_rad(v).sin(),
            "cos" => to_rad(v).cos(),
            "tan" => to_rad(v).tan(),
            "asin" => from_rad(v.asin()),
            "acos" => from_rad(v.acos()),
            "atan" => from_rad(v.atan()),
            "sinh" => v.sinh(),
            "cosh" => v.cosh(),
            "tanh" => v.tanh(),
            "x²" => v * v,
            "x³" => v * v * v,
            "√" => v.sqrt(),
            "∛" => v.cbrt(),
            "ln" => v.ln(),
            "log" => v.log10(),
            "eˣ" => v.exp(),
            "10ˣ" => 10f64.powf(v),
            "1/x" => 1.0 / v,
            "x!" => {
                if v < 0.0 || v.fract() != 0.0 || v > 170.0 {
                    f64::NAN
                } else {
                    (1..=v as u64).map(|i| i as f64).product()
                }
            }
            "π" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "Rand" => {
                let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1);
                (t.wrapping_mul(2654435761) % 1_000_000) as f64 / 1_000_000.0
            }
            _ => v,
        };
        // Resultados quase inteiros (sen 180° etc.) viram o inteiro.
        let r = if (r - r.round()).abs() < 1e-12 { r.round() } else { r };
        self.error = !r.is_finite();
        self.display = fmt(r);
        self.fresh = true;
    }

    fn negate(&mut self) {
        if self.display.starts_with('-') {
            self.display.remove(0);
        } else if self.display != "0" {
            self.display.insert(0, '-');
        }
    }

    fn percent(&mut self) {
        let v = self.value();
        let r = match (self.acc, self.op) {
            (Some(a), Some(Op::Add | Op::Sub)) => a * v / 100.0,
            _ => v / 100.0,
        };
        self.display = fmt(r);
        self.fresh = true;
    }

    fn backspace(&mut self) {
        if self.fresh || self.error {
            return;
        }
        self.display.pop();
        if self.display.is_empty() || self.display == "-" {
            self.display = "0".into();
        }
    }

    fn memory_add(&mut self, sign: f64) {
        if !self.error {
            self.memory += sign * self.value();
            self.fresh = true;
        }
    }

    fn memory_recall(&mut self) {
        self.display = fmt(self.memory);
        self.error = false;
        self.fresh = true;
    }

    fn keyboard(&mut self, ui: &Ui) {
        for ch in ui.typed().chars() {
            match ch {
                '0'..='9' => self.digit(ch),
                ',' | '.' => self.digit(','),
                '+' => self.operator(Op::Add),
                '-' => self.operator(Op::Sub),
                '*' | 'x' | 'X' => self.operator(Op::Mul),
                '/' => self.operator(Op::Div),
                '=' => self.equals(),
                '%' => self.percent(),
                'c' | 'C' => self.clear(),
                _ => {}
            }
        }
        for k in ui.keys() {
            match k.key {
                Key::Enter => self.equals(),
                Key::Backspace => self.backspace(),
                Key::Escape | Key::Delete => self.clear(),
                _ => {}
            }
        }
    }
}

/// Teclas científicas (4 colunas).
const SCI: [[&str; 4]; 6] = [
    ["sin", "cos", "tan", "π"],
    ["asin", "acos", "atan", "e"],
    ["sinh", "cosh", "tanh", "Rand"],
    ["x²", "x³", "xʸ", "√"],
    ["ln", "log", "eˣ", "10ˣ"],
    ["1/x", "x!", "∛", "Graus"],
];
const SCI_W: f32 = 4.0 * 46.0 + 3.0 * 7.0 + 14.0;

const PAD: f32 = 14.0;
const BW: f32 = 46.0;
const BH: f32 = 26.0;
const GAP: f32 = 7.0;
const DISPLAY_H: f32 = 50.0;
/// Fileiras de teclas: memória + 5 da grade.
const ROWS: f32 = 6.0;

impl App for Calc {
    fn paints_background(&self) -> bool {
        true
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            "Visualizar",
            vec![MenuItem::new("basic", "Básica", "Ctrl+1").checked(!self.scientific), MenuItem::new("sci", "Científica", "Ctrl+2").checked(self.scientific)],
        )]
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "basic" | "sci" => {
                    self.scientific = a == "sci";
                    let (w, h) = sizes(self.scientific);
                    ui.set_window_size(w as f32, h as f32, true);
                }
                _ => {}
            }
        }
        for k in ui.keys().to_vec() {
            match k.combo.as_str() {
                "Ctrl+1" => {
                    self.scientific = false;
                    let (w, h) = sizes(false);
                    ui.set_window_size(w as f32, h as f32, true);
                }
                "Ctrl+2" => {
                    self.scientific = true;
                    let (w, h) = sizes(true);
                    ui.set_window_size(w as f32, h as f32, true);
                }
                _ => {}
            }
        }
        let (w, h) = (ui.c.w, ui.c.h);
        // Corpo em metal escovado, como a calculadora do 10.3.
        let (top, bot) = (ui.theme.metal_top, ui.theme.metal_bottom);
        ui.c.brushed_metal(0.0, 0.0, w, h, top, bot);

        // Visor de cristal líquido verde-amarelado, afundado no metal.
        let (dx, dy, dw, dh) = (PAD, PAD, w - 2.0 * PAD, DISPLAY_H);
        ui.c.fill(rrect(dx - 1.0, dy - 1.0, dw + 2.0, dh + 2.0, 3.0), &vgrad(dy, dy + dh, &[(0.0, rgba(110, 112, 104, 255)), (1.0, rgba(235, 235, 230, 255))]));
        ui.c.fill(rrect(dx, dy, dw, dh, 2.0), &solid(rgba(236, 242, 200, 255)));
        ui.c.fill(rrect(dx, dy, dw, 4.0, 2.0), &vgrad(dy, dy + 4.0, &[(0.0, rgba(0, 0, 0, 45)), (1.0, rgba(0, 0, 0, 0))]));
        // Por dentro sempre vírgula; no visor, o separador do idioma.
        let shown = if self.error { tr!("Erro").to_string() } else { self.display.replace(',', glove_config::virgula()) };
        let mut px = 22.0;
        while ui.text_width(&shown, px, false) > dw - 20.0 && px > 11.0 {
            px -= 1.0;
        }
        let tw = ui.text_width(&shown, px, false);
        let ink = rgba(25, 30, 20, 255);
        ui.fonts.draw(ui.c, &shown, dx + dw - tw - 10.0, dy + 26.0, px, false, ink);
        let mut marks = Vec::new();
        if self.memory != 0.0 {
            marks.push("M");
        }
        if let Some(op) = self.op {
            marks.push(match op {
                Op::Add => "+",
                Op::Sub => "−",
                Op::Mul => "×",
                Op::Div => "÷",
                Op::Pow => "xʸ",
            });
        }
        if self.scientific {
            marks.push(if self.degrees { "Graus" } else { "Rad" });
        }
        ui.fonts.draw(ui.c, &marks.join("  "), dx + 10.0, dy + dh - 9.0, 11.0, false, ink);

        // Teclas (no modo científico, as funções ficam à esquerda).
        let y0 = PAD + DISPLAY_H + 12.0;
        if self.scientific {
            for (row, keys) in SCI.iter().enumerate() {
                for (col, k) in keys.iter().enumerate() {
                    let r = Rect::new(PAD + col as f32 * (BW + GAP), y0 + row as f32 * (BH + GAP), BW, BH);
                    let label = if *k == "Graus" { if self.degrees { "Rad" } else { "Graus" } } else { k };
                    let lit = *k == "xʸ" && self.op == Some(Op::Pow) && self.fresh;
                    if ui.key_button(r, label, 12.0, lit) {
                        match *k {
                            "Graus" => self.degrees = !self.degrees,
                            "xʸ" => self.operator(Op::Pow),
                            f => self.function(f),
                        }
                    }
                }
            }
        }
        let x_off = if self.scientific { SCI_W } else { 0.0 };
        let cell = |col: f32, row: f32, span: f32, vspan: f32| {
            Rect::new(x_off + PAD + col * (BW + GAP), y0 + row * (BH + GAP), BW * span + GAP * (span - 1.0), BH * vspan + GAP * (vspan - 1.0))
        };
        let (cur_op, fresh) = (self.op, self.fresh);
        let op_lit = |o: Op| cur_op == Some(o) && fresh;
        let px = 14.0;

        for (i, label) in ["MC", "M+", "M−", "MR"].into_iter().enumerate() {
            if ui.key_button(cell(i as f32, 0.0, 1.0, 1.0), label, 12.0, false) {
                match i {
                    0 => self.memory = 0.0,
                    1 => self.memory_add(1.0),
                    2 => self.memory_add(-1.0),
                    _ => self.memory_recall(),
                }
            }
        }
        if ui.key_button(cell(0.0, 1.0, 1.0, 1.0), "C", px, false) {
            self.clear();
        }
        if ui.key_button(cell(1.0, 1.0, 1.0, 1.0), "±", px, false) {
            self.negate();
        }
        for (r, (label, op)) in [(cell(2.0, 1.0, 1.0, 1.0), ("÷", Op::Div)), (cell(3.0, 1.0, 1.0, 1.0), ("×", Op::Mul)), (cell(3.0, 2.0, 1.0, 1.0), ("−", Op::Sub)), (cell(3.0, 3.0, 1.0, 1.0), ("+", Op::Add))] {
            if ui.key_button(r, label, px + 2.0, op_lit(op)) {
                self.operator(op);
            }
        }
        for (row, digits) in [['7', '8', '9'], ['4', '5', '6'], ['1', '2', '3']].iter().enumerate() {
            for (col, d) in digits.iter().enumerate() {
                if ui.key_button(cell(col as f32, row as f32 + 2.0, 1.0, 1.0), &d.to_string(), px, false) {
                    self.digit(*d);
                }
            }
        }
        if ui.key_button(cell(0.0, 5.0, 2.0, 1.0), "0", px, false) {
            self.digit('0');
        }
        if ui.key_button(cell(2.0, 5.0, 1.0, 1.0), ",", px, false) {
            self.digit(',');
        }
        if ui.key_button(cell(3.0, 4.0, 1.0, 2.0), "=", px + 2.0, false) {
            self.equals();
        }
    }
}

/// Tamanho da janela: básica ou científica (mais larga).
fn sizes(scientific: bool) -> (u32, u32) {
    let w = PAD * 2.0 + BW * 4.0 + GAP * 3.0 + if scientific { SCI_W } else { 0.0 };
    let h = PAD + DISPLAY_H + 12.0 + BH * ROWS + GAP * (ROWS - 1.0) + PAD;
    (w as u32, h as u32)
}

pub fn main() {
    let opts = WindowOptions { title: "Calculadora".into(), app_id: "glove.Calculator".into(), size: sizes(false), fixed: true, min_size: None };
    if let Err(e) = run(opts, Calc::new()) {
        eprintln!("glove-calc: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: &mut Calc, keys: &str) {
        for ch in keys.chars() {
            match ch {
                '0'..='9' | ',' => c.digit(ch),
                '+' => c.operator(Op::Add),
                '-' => c.operator(Op::Sub),
                '*' => c.operator(Op::Mul),
                '/' => c.operator(Op::Div),
                '=' => c.equals(),
                '%' => c.percent(),
                _ => {}
            }
        }
    }

    #[test]
    fn memoria() {
        let mut c = Calc::new();
        press(&mut c, "12");
        c.memory_add(1.0);
        press(&mut c, "5");
        c.memory_add(-1.0);
        c.clear();
        c.memory_recall();
        assert_eq!(c.display, "7");
    }

    #[test]
    fn cientifica() {
        let mut c = Calc::new();
        press(&mut c, "90");
        c.function("sin");
        assert_eq!(c.display, "1");
        let mut c = Calc::new();
        press(&mut c, "5");
        c.function("x!");
        assert_eq!(c.display, "120");
        let mut c = Calc::new();
        press(&mut c, "2");
        c.operator(Op::Pow);
        press(&mut c, "10=");
        assert_eq!(c.display, "1024");
        let mut c = Calc::new();
        press(&mut c, "100");
        c.function("log");
        assert_eq!(c.display, "2");
    }

    #[test]
    fn contas() {
        for (k, r) in [("2+3=", "5"), ("10/4=", "2,5"), ("1,5*4=", "6"), ("7-10=", "-3"), ("2+3*4=", "20"), ("1/3=", "0,3333333333"), ("200+10%", "20"), ("5/0=", "Erro"), ("0,1+0,2=", "0,3")] {
            let mut c = Calc::new();
            press(&mut c, k);
            let shown = if c.error { "Erro".to_string() } else { c.display.clone() };
            assert_eq!(shown, r, "conta {k}");
        }
    }
}
