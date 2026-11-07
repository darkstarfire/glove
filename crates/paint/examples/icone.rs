//! Desenha um ícone sozinho: `icone NOME SAIDA.png TAMANHO` (para conferir de perto).
use glove_paint::draw::{rgba, solid, Canvas};
use glove_paint::icons::{self, Icon};
use glove_paint::Fonts;

fn main() {
    let mut a = std::env::args().skip(1);
    let (name, out) = (a.next().expect("nome"), a.next().expect("saída"));
    let s: f32 = a.next().and_then(|v| v.parse().ok()).unwrap_or(512.0);
    let fonts = Fonts::load_system().expect("fonte");
    let mut c = Canvas::new(s, s, 1.0).unwrap();
    c.fill_rect(0.0, 0.0, s, s, &solid(rgba(236, 236, 236, 255)));
    icons::draw(&mut c, &fonts, Icon::from_name(&name), 0.0, 0.0, s, None);
    c.pm.save_png(&out).unwrap();
}
