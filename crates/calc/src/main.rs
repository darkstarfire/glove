//! Calculadora do Glove.

use glove_kit::{
    paint::draw::*,
    run,
    ui::Key,
    App, Rect, Ui, WindowOptions,
};

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
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
        Self { display: "0".into(), acc: None, op: None, fresh: true, error: false, memory: 0.0 }
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

    /// C limpa a conta, mas não a memória (essa só com MC).
    fn clear(&mut self) {
        *self = Self { memory: self.memory, ..Self::new() };
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

    fn ui(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        // Corpo em metal escovado, como a calculadora do 10.3.
        let (top, bot) = (ui.theme.metal_top, ui.theme.metal_bottom);
        ui.c.brushed_metal(0.0, 0.0, w, h, top, bot);

        // Visor de cristal líquido verde-amarelado, afundado no metal.
        let (dx, dy, dw, dh) = (PAD, PAD, w - 2.0 * PAD, DISPLAY_H);
        ui.c.fill(rrect(dx - 1.0, dy - 1.0, dw + 2.0, dh + 2.0, 3.0), &vgrad(dy, dy + dh, &[(0.0, rgba(110, 112, 104, 255)), (1.0, rgba(235, 235, 230, 255))]));
        ui.c.fill(rrect(dx, dy, dw, dh, 2.0), &solid(rgba(236, 242, 200, 255)));
        ui.c.fill(rrect(dx, dy, dw, 4.0, 2.0), &vgrad(dy, dy + 4.0, &[(0.0, rgba(0, 0, 0, 45)), (1.0, rgba(0, 0, 0, 0))]));
        let shown = if self.error { "Erro".to_string() } else { self.display.clone() };
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
            });
        }
        ui.fonts.draw(ui.c, &marks.join("  "), dx + 10.0, dy + dh - 9.0, 11.0, false, ink);

        // Teclas
        let y0 = PAD + DISPLAY_H + 12.0;
        let cell = |col: f32, row: f32, span: f32, vspan: f32| {
            Rect::new(PAD + col * (BW + GAP), y0 + row * (BH + GAP), BW * span + GAP * (span - 1.0), BH * vspan + GAP * (vspan - 1.0))
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

fn main() {
    let w = (PAD * 2.0 + BW * 4.0 + GAP * 3.0) as u32;
    let h = (PAD + DISPLAY_H + 12.0 + BH * ROWS + GAP * (ROWS - 1.0) + PAD) as u32;
    let opts = WindowOptions { title: "Calculadora".into(), app_id: "glove.Calculator".into(), size: (w, h), fixed: true };
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
    fn contas() {
        for (k, r) in [("2+3=", "5"), ("10/4=", "2,5"), ("1,5*4=", "6"), ("7-10=", "-3"), ("2+3*4=", "20"), ("1/3=", "0,3333333333"), ("200+10%", "20"), ("5/0=", "Erro"), ("0,1+0,2=", "0,3")] {
            let mut c = Calc::new();
            press(&mut c, k);
            let shown = if c.error { "Erro".to_string() } else { c.display.clone() };
            assert_eq!(shown, r, "conta {k}");
        }
    }
}
