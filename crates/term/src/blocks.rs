//! Caracteres de bloco (U+2580–259F) e de linhas (U+2500–257F) desenhados como
//! retângulos exatos na célula, como fazem os terminais modernos: com a fonte, eles
//! não se encostam (frestas entre as células) e logos e bordas ficam quebrados.

use glove_ui::paint::{draw::*, Color};

/// Pesos das quatro pontas (cima, direita, baixo, esquerda): 1 fina, 2 grossa, 3 dupla.
fn arms(c: char) -> Option<[u8; 4]> {
    let u = c as u32;
    Some(match u {
        0x2500 | 0x2504 | 0x2508 | 0x254C => [0, 1, 0, 1],
        0x2501 | 0x2505 | 0x2509 | 0x254D => [0, 2, 0, 2],
        0x2502 | 0x2506 | 0x250A | 0x254E => [1, 0, 1, 0],
        0x2503 | 0x2507 | 0x250B | 0x254F => [2, 0, 2, 0],
        0x250C..=0x254B => {
            const T: [[u8; 4]; 64] = [
                [0, 1, 1, 0], [0, 2, 1, 0], [0, 1, 2, 0], [0, 2, 2, 0], [0, 0, 1, 1], [0, 0, 1, 2], [0, 0, 2, 1], [0, 0, 2, 2],
                [1, 1, 0, 0], [1, 2, 0, 0], [2, 1, 0, 0], [2, 2, 0, 0], [1, 0, 0, 1], [1, 0, 0, 2], [2, 0, 0, 1], [2, 0, 0, 2],
                [1, 1, 1, 0], [1, 2, 1, 0], [2, 1, 1, 0], [1, 1, 2, 0], [2, 1, 2, 0], [2, 2, 1, 0], [1, 2, 2, 0], [2, 2, 2, 0],
                [1, 0, 1, 1], [1, 0, 1, 2], [2, 0, 1, 1], [1, 0, 2, 1], [2, 0, 2, 1], [2, 0, 1, 2], [1, 0, 2, 2], [2, 0, 2, 2],
                [0, 1, 1, 1], [0, 1, 1, 2], [0, 2, 1, 1], [0, 2, 1, 2], [0, 1, 2, 1], [0, 1, 2, 2], [0, 2, 2, 1], [0, 2, 2, 2],
                [1, 1, 0, 1], [1, 1, 0, 2], [1, 2, 0, 1], [1, 2, 0, 2], [2, 1, 0, 1], [2, 1, 0, 2], [2, 2, 0, 1], [2, 2, 0, 2],
                [1, 1, 1, 1], [1, 1, 1, 2], [1, 2, 1, 1], [1, 2, 1, 2], [2, 1, 1, 1], [1, 1, 2, 1], [2, 1, 2, 1], [2, 1, 1, 2],
                [2, 2, 1, 1], [1, 1, 2, 2], [1, 2, 2, 1], [2, 2, 1, 2], [1, 2, 2, 2], [2, 1, 2, 2], [2, 2, 2, 1], [2, 2, 2, 2],
            ];
            T[(u - 0x250C) as usize]
        }
        0x2550..=0x256C => {
            const D: [[u8; 4]; 29] = [
                [0, 3, 0, 3], [3, 0, 3, 0], [0, 3, 1, 0], [0, 1, 3, 0], [0, 3, 3, 0], [0, 0, 1, 3], [0, 0, 3, 1], [0, 0, 3, 3],
                [1, 3, 0, 0], [3, 1, 0, 0], [3, 3, 0, 0], [1, 0, 0, 3], [3, 0, 0, 1], [3, 0, 0, 3], [1, 3, 1, 0], [3, 1, 3, 0],
                [3, 3, 3, 0], [1, 0, 1, 3], [3, 0, 3, 1], [3, 0, 3, 3], [0, 3, 1, 3], [0, 1, 3, 1], [0, 3, 3, 3], [1, 3, 0, 3],
                [3, 1, 0, 1], [3, 3, 0, 3], [1, 3, 1, 3], [3, 1, 3, 1], [3, 3, 3, 3],
            ];
            D[(u - 0x2550) as usize]
        }
        0x2574 => [0, 0, 0, 1],
        0x2575 => [1, 0, 0, 0],
        0x2576 => [0, 1, 0, 0],
        0x2577 => [0, 0, 1, 0],
        0x2578 => [0, 0, 0, 2],
        0x2579 => [2, 0, 0, 0],
        0x257A => [0, 2, 0, 0],
        0x257B => [0, 0, 2, 0],
        0x257C => [0, 2, 0, 1],
        0x257D => [1, 0, 2, 0],
        0x257E => [0, 1, 0, 2],
        0x257F => [2, 0, 1, 0],
        _ => return None,
    })
}

/// Desenha `c` na célula (x, y, w, h). Devolve falso se não é um desses caracteres.
pub fn draw(cv: &mut Canvas, c: char, x: f32, y: f32, w: f32, h: f32, col: Color) -> bool {
    let u = c as u32;
    let s = cv.scale;
    // Espessura da linha fina: um pixel físico ou mais, conforme o tamanho da letra.
    let t = ((w * 0.12 * s).round().max(1.0)) / s;
    let snap = |v: f32| (v * s).round() / s;
    match u {
        // Metades, oitavos, sombreados e quadrantes.
        0x2580 => cv.fill_cell(x, y, w, h / 2.0, col),
        0x2581..=0x2587 => {
            let f = (u - 0x2580) as f32 / 8.0;
            cv.fill_cell(x, y + h * (1.0 - f), w, h * f, col);
        }
        0x2588 => cv.fill_cell(x, y, w, h, col),
        0x2589..=0x258F => {
            let f = (0x2590 - u) as f32 / 8.0;
            cv.fill_cell(x, y, w * f, h, col);
        }
        0x2590 => cv.fill_cell(x + w / 2.0, y, w / 2.0, h, col),
        0x2591..=0x2593 => cv.fill_cell(x, y, w, h, with_alpha(col, (u - 0x2590) as f32 * 0.25)),
        0x2594 => cv.fill_cell(x, y, w, h / 8.0, col),
        0x2595 => cv.fill_cell(x + w * 7.0 / 8.0, y, w / 8.0, h, col),
        0x2596..=0x259F => {
            // Quadrantes: bits (cima-esq, cima-dir, baixo-esq, baixo-dir).
            const Q: [u8; 10] = [0b0010, 0b0001, 0b1000, 0b1011, 0b1001, 0b1110, 0b1101, 0b0100, 0b0110, 0b0111];
            let q = Q[(u - 0x2596) as usize];
            let (hw, hh) = (w / 2.0, h / 2.0);
            for (bit, qx, qy) in [(0b1000, 0.0, 0.0), (0b0100, hw, 0.0), (0b0010, 0.0, hh), (0b0001, hw, hh)] {
                if q & bit != 0 {
                    cv.fill_cell(x + qx, y + qy, hw, hh, col);
                }
            }
        }
        // Cantos arredondados: um quarto de círculo.
        0x256D..=0x2570 => {
            let (cx, cy) = (snap(x + w / 2.0 - t / 2.0) + t / 2.0, snap(y + h / 2.0 - t / 2.0) + t / 2.0);
            let r = (w / 2.0).min(h / 2.0);
            let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
            let (a, b, ctl) = match u {
                0x256D => ((x + w, cy), (cx, y + h), (cx + 0.0, cy)),
                0x256E => ((x, cy), (cx, y + h), (cx, cy)),
                0x256F => ((x, cy), (cx, y), (cx, cy)),
                _ => ((x + w, cy), (cx, y), (cx, cy)),
            };
            let _ = r;
            pb.move_to(a.0, a.1);
            pb.quad_to(ctl.0, ctl.1, b.0, b.1);
            cv.stroke(pb.finish(), &solid(col), t);
        }
        _ => {
            let Some([up, right, down, left]) = arms(c) else { return false };
            // Centro da célula alinhado ao pixel (as linhas vizinhas casam).
            let cx = snap(x + w / 2.0);
            let cy = snap(y + h / 2.0);
            let thick = |k: u8| if k == 2 { t * 2.0 } else { t };
            // Comprimento até o centro, cobrindo a espessura do cruzamento.
            let reach_v = thick(left.max(right)).max(if left == 3 || right == 3 { t * 3.0 } else { 0.0 }) / 2.0;
            let reach_h = thick(up.max(down)).max(if up == 3 || down == 3 { t * 3.0 } else { 0.0 }) / 2.0;
            let line_h = |cv: &mut Canvas, x0: f32, x1: f32, k: u8| {
                if k == 3 {
                    cv.fill_cell(x0, cy - t * 1.5, x1 - x0, t, col);
                    cv.fill_cell(x0, cy + t * 0.5, x1 - x0, t, col);
                } else {
                    let th = thick(k);
                    cv.fill_cell(x0, cy - th / 2.0, x1 - x0, th, col);
                }
            };
            let line_v = |cv: &mut Canvas, y0: f32, y1: f32, k: u8| {
                if k == 3 {
                    cv.fill_cell(cx - t * 1.5, y0, t, y1 - y0, col);
                    cv.fill_cell(cx + t * 0.5, y0, t, y1 - y0, col);
                } else {
                    let th = thick(k);
                    cv.fill_cell(cx - th / 2.0, y0, th, y1 - y0, col);
                }
            };
            if left > 0 {
                line_h(cv, x, cx + reach_h, left);
            }
            if right > 0 {
                line_h(cv, cx - reach_h, x + w, right);
            }
            if up > 0 {
                line_v(cv, y, cy + reach_v, up);
            }
            if down > 0 {
                line_v(cv, cy - reach_v, y + h, down);
            }
        }
    }
    true
}
