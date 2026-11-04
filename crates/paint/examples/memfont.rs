fn rss() -> String { std::fs::read_to_string("/proc/self/status").unwrap().lines().filter(|l| l.starts_with("RssAnon")).collect() }
fn main() {
    println!("antes:   {}", rss());
    let f = glove_paint::Fonts::load_system().unwrap();
    println!("fontes:  {}", rss());
    let mut c = glove_paint::draw::Canvas::new(400.0, 100.0, 1.5).unwrap();
    f.draw(&mut c, "Olá, mundo! Preferências ÁÉÍÓÚ 0123456789", 0.0, 20.0, 13.0, false, glove_paint::Color::BLACK);
    println!("desenho: {}", rss());
}
