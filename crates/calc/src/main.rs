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
        Self { display: "0".into(), acc: None, op: None, fresh: true, error: false }
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

    fn clear(&mut self) {
        *self = Self::new();
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

const PAD: f32 = 12.0;
const BW: f32 = 46.0;
const BH: f32 = 36.0;
const GAP: f32 = 8.0;
const DISPLAY_H: f32 = 60.0;

impl App for Calc {
    fn ui(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        let w = ui.c.w;

        // Visor: vidro azulado afundado, com reflexo diagonal.
        let (dx, dy, dw, dh) = (PAD, PAD, w - 2.0 * PAD, DISPLAY_H);
        ui.c.fill(rrect(dx - 1.0, dy - 1.0, dw + 2.0, dh + 2.0, 7.0), &vgrad(dy, dy + dh, &[(0.0, rgba(90, 95, 110, 255)), (1.0, rgba(200, 204, 214, 255))]));
        ui.c.fill(rrect(dx, dy, dw, dh, 6.0), &vgrad(dy, dy + dh, &[(0.0, rgba(196, 214, 228, 255)), (1.0, rgba(232, 240, 246, 255))]));
        ui.c.fill(rrect(dx, dy, dw, 6.0, 6.0), &vgrad(dy, dy + 6.0, &[(0.0, rgba(0, 0, 0, 50)), (1.0, rgba(0, 0, 0, 0))]));
        let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
        pb.move_to(dx + dw * 0.55, dy);
        pb.line_to(dx + dw * 0.85, dy);
        pb.line_to(dx + dw * 0.62, dy + dh);
        pb.line_to(dx + dw * 0.32, dy + dh);
        pb.close();
        ui.c.fill(pb.finish(), &solid(rgba(255, 255, 255, 45)));
        let shown = if self.error { "Erro".to_string() } else { self.display.clone() };
        let mut px = 28.0;
        while ui.text_width(&shown, px, false) > dw - 20.0 && px > 12.0 {
            px -= 1.0;
        }
        let tw = ui.text_width(&shown, px, false);
        ui.fonts.draw(ui.c, &shown, dx + dw - tw - 10.0, dy + dh - 14.0, px, false, rgba(20, 30, 45, 255));
        if let Some(op) = self.op {
            let s = match op {
                Op::Add => "+",
                Op::Sub => "−",
                Op::Mul => "×",
                Op::Div => "÷",
            };
            ui.fonts.draw(ui.c, s, dx + 10.0, dy + 18.0, 13.0, true, rgba(60, 80, 110, 255));
        }

        // Teclas
        let accent = ui.theme.accent;
        let light = rgba(240, 240, 243, 255);
        let mid = rgba(196, 198, 206, 255);
        let eq = rgba(240, 150, 40, 255);
        let y0 = PAD + DISPLAY_H + 14.0;
        let cell = |col: f32, row: f32, span: f32| Rect::new(PAD + col * (BW + GAP), y0 + row * (BH + GAP), BW * span + GAP * (span - 1.0), BH);
        let (cur_op, fresh) = (self.op, self.fresh);
        let op_lit = |o: Op| cur_op == Some(o) && fresh;

        if ui.gel_button(cell(0.0, 0.0, 1.0), "C", mid, 15.0, false) {
            self.clear();
        }
        if ui.gel_button(cell(1.0, 0.0, 1.0), "±", mid, 15.0, false) {
            self.negate();
        }
        if ui.gel_button(cell(2.0, 0.0, 1.0), "%", mid, 15.0, false) {
            self.percent();
        }
        for (i, (label, op)) in [("÷", Op::Div), ("×", Op::Mul), ("−", Op::Sub), ("+", Op::Add)].into_iter().enumerate() {
            let lit = op_lit(op);
            if ui.gel_button(cell(3.0, i as f32, 1.0), label, accent, 17.0, lit) {
                self.operator(op);
            }
        }
        for (row, digits) in [['7', '8', '9'], ['4', '5', '6'], ['1', '2', '3']].iter().enumerate() {
            for (col, d) in digits.iter().enumerate() {
                if ui.gel_button(cell(col as f32, row as f32 + 1.0, 1.0), &d.to_string(), light, 15.0, false) {
                    self.digit(*d);
                }
            }
        }
        if ui.gel_button(cell(0.0, 4.0, 2.0), "0", light, 15.0, false) {
            self.digit('0');
        }
        if ui.gel_button(cell(2.0, 4.0, 1.0), ",", light, 15.0, false) {
            self.digit(',');
        }
        if ui.gel_button(cell(3.0, 4.0, 1.0), "=", eq, 17.0, false) {
            self.equals();
        }
    }
}

fn main() {
    let w = (PAD * 2.0 + BW * 4.0 + GAP * 3.0) as u32;
    let h = (PAD + DISPLAY_H + 14.0 + BH * 5.0 + GAP * 4.0 + PAD) as u32;
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
    fn contas() {
        for (k, r) in [("2+3=", "5"), ("10/4=", "2,5"), ("1,5*4=", "6"), ("7-10=", "-3"), ("2+3*4=", "20"), ("1/3=", "0,3333333333"), ("200+10%", "20"), ("5/0=", "Erro"), ("0,1+0,2=", "0,3")] {
            let mut c = Calc::new();
            press(&mut c, k);
            let shown = if c.error { "Erro".to_string() } else { c.display.clone() };
            assert_eq!(shown, r, "conta {k}");
        }
    }
}
