//! Folha com todos os ícones desenhados pelo Glove (para comparar o estilo):
//! `cargo run -p glove-paint --example icones -- saida.png [tamanho]`
use glove_paint::draw::{rgba, solid, Canvas};
use glove_paint::icons::{self, FolderKind, Icon, PrefKind};
use glove_paint::Fonts;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or("icones.png".into());
    let s: f32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(112.0);
    let list: Vec<(&str, Icon)> = vec![
        ("Arquivos", Icon::Folder), ("Terminal", Icon::Terminal), ("Navegador", Icon::Globe), ("Editor", Icon::TextEditor),
        ("Calculadora", Icon::Calculator), ("Preferências", Icon::Settings), ("Relógio", Icon::Clock(10, 9)), ("Lixo", Icon::Trash),
        ("Lixo cheio", Icon::TrashFull), ("Disco", Icon::Disk), ("Documento", Icon::Document), ("Imagem", Icon::Picture),
        ("Arquivo", Icon::File), ("Lembretes", Icon::Notes), ("Busca", Icon::Search), ("Caracteres", Icon::Chars),
        ("Captura", Icon::Camera), ("Monitor", Icon::Monitor), ("Xadrez", Icon::Chess), ("Música", Icon::Music),
        ("Contatos", Icon::Contacts), ("Agenda", Icon::Calendar), ("Vídeo", Icon::Video), ("Registros", Icon::Logs),
        ("Info", Icon::Info), ("Ajuda", Icon::Help), ("Dicionário", Icon::Dictionary), ("Fontes", Icon::FontBook),
        ("Correio", Icon::Mail), ("Monitores", Icon::Display), ("Chaves", Icon::Keychain), ("Ejetar", Icon::Eject),
        ("Áudio", Icon::AudioFile), ("Vídeo arq.", Icon::VideoFile), ("Compactado", Icon::ArchiveFile), ("PDF", Icon::PdfFile),
        ("Mesa", Icon::SpecialFolder(FolderKind::Desktop)), ("Documentos", Icon::SpecialFolder(FolderKind::Documents)), ("Início", Icon::SpecialFolder(FolderKind::Home)), ("Aplicativos", Icon::SpecialFolder(FolderKind::Applications)),
        ("Aparência", Icon::Pref(PrefKind::Appearance)), ("Dock", Icon::Pref(PrefKind::Dock)), ("Som", Icon::Pref(PrefKind::Sound)), ("Energia", Icon::Pref(PrefKind::Energy)),
        ("Rede", Icon::Pref(PrefKind::Network)), ("Segurança", Icon::Pref(PrefKind::Security)), ("Impressão", Icon::Pref(PrefKind::Printer)), ("Contas", Icon::Pref(PrefKind::Accounts)),
    ];
    let fonts = Fonts::load_system().expect("fonte");
    let (cell, cols) = (s + 28.0, 8usize);
    let rows = list.len().div_ceil(cols);
    let mut c = Canvas::new(cols as f32 * cell, rows as f32 * cell, 1.0).unwrap();
    c.fill_rect(0.0, 0.0, c.w, c.h, &solid(rgba(236, 236, 236, 255)));
    for (i, (name, icon)) in list.iter().enumerate() {
        let (x, y) = ((i % cols) as f32 * cell, (i / cols) as f32 * cell);
        icons::draw(&mut c, &fonts, *icon, x + 14.0, y + 4.0, s, None);
        fonts.draw(&mut c, name, x + 4.0, y + s + 20.0, 11.0, false, rgba(30, 30, 30, 255));
    }
    c.pm.save_png(&out).unwrap();
}
