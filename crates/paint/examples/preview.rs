//! Prévia do visual: `cargo run -p glove-paint --example preview -- saida.png [tema] [escala]`
use glove_paint::chrome::*;
use glove_paint::draw::Canvas;
use glove_paint::icons::Icon;
use glove_paint::theme::{MENUBAR_H, TITLE_H};
use glove_paint::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or("preview.png".into());
    let variant = args.next().and_then(|s| Variant::from_id(&s)).unwrap_or(Variant::Listras);
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let theme = Theme::new(variant);
    let fonts = Fonts::load_system().expect("fonte");
    let (w, h) = (1280.0, 800.0);
    let mut screen = wallpaper::render(&theme, w, h, scale).unwrap();

    for (x, y, ww, wh, active, title) in [(120, 90, 560, 380, false, "Documento sem título"), (380, 200, 640, 420, true, "Terminal — bash — 80×24")] {
        for (sx, sy, sh) in window_shadow(ww, wh + TITLE_H, active, scale) {
            screen.draw_canvas(&sh, (x + sx) as f32, (y + sy) as f32);
        }
        let st = TitleBarState { active, hover: active, pressed: None };
        let tb = titlebar(&fonts, &theme, ww as f32, title, &st, scale).unwrap();
        screen.draw_canvas(&tb, x as f32, y as f32);
        let mut body = Canvas::new(ww as f32, wh as f32, scale).unwrap();
        body.clear(if active { Color::from_rgba8(30, 30, 30, 255) } else { Color::WHITE });
        if active {
            fonts.draw(&mut body, "glove:~$ uname -a", 10.0, 20.0, 13.0, false, Color::from_rgba8(220, 220, 220, 255));
        }
        screen.draw_canvas(&body, x as f32, (y + TITLE_H) as f32);
    }

    let titles = vec!["Terminal".to_string(), "Shell".into(), "Editar".into()];
    let (mb, _) = menubar(&fonts, &theme, w, &titles, "Ter 19:47", Some(0), scale).unwrap();
    screen.draw_canvas(&mb, 0.0, 0.0);
    let items = vec![
        MenuItem::item("Sobre o Glove"),
        MenuItem::sep(),
        MenuItem::item("Novo Terminal"),
        MenuItem::sep(),
        MenuItem::check("Tema: Listras", variant == Variant::Listras),
        MenuItem::check("Tema: Metal", variant == Variant::Metal),
        MenuItem::check("Tema: Grafite", variant == Variant::Grafite),
    ];
    let m = menu(&fonts, &theme, &items, Some(2), scale).unwrap();
    screen.draw_canvas(&m, (8 - MENU_SHADOW) as f32, MENUBAR_H as f32);

    let dock_items: Vec<DockItem> = [
        (Icon::Folder, "Arquivos", true),
        (Icon::Terminal, "Terminal", true),
        (Icon::Globe, "Navegador", false),
        (Icon::TextEditor, "Editor", false),
        (Icon::Calculator, "Calculadora", false),
        (Icon::Settings, "Preferências", false),
    ]
    .into_iter()
    .map(|(icon, label, running)| DockItem { icon, label: label.into(), letter: None, running, separator_before: false })
    .collect();
    let base = dock_layout(&dock_items, None, 0.0, DOCK_BASE, DOCK_MAX);
    let layout = dock_layout(&dock_items, Some(base.items[1].0 + 24.0), 1.0, DOCK_BASE, DOCK_MAX);
    let d = dock(&fonts, &theme, &dock_items, &layout, Some(1), scale).unwrap();
    screen.draw_canvas(&d, ((w as u32 - layout.width) / 2) as f32, h - layout.height as f32);
    screen.pm.save_png(&out).unwrap();
    println!("salvo em {out}");
}
