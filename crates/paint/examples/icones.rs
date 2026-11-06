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
        ("Arquivos", Icon::Files), ("Terminal", Icon::Terminal), ("Navegador", Icon::Globe), ("Editor", Icon::TextEditor),
        ("Calculadora", Icon::Calculator), ("Preferências", Icon::Settings), ("Relógio", Icon::Clock(10, 9)), ("Lixo", Icon::Trash),
        ("Lixo 1", Icon::TrashFull(1)), ("Lixo 7", Icon::TrashFull(7)), ("Lixo 14", Icon::TrashFull(14)), ("Lixo cheio", Icon::TrashFull(20)), ("Disco", Icon::Disk), ("Documento", Icon::Document), ("Imagem", Icon::Picture),
        ("Arquivo", Icon::File), ("Lembretes", Icon::Notes), ("Busca", Icon::Search), ("Caracteres", Icon::Chars),
        ("Captura", Icon::Camera), ("Monitor", Icon::Monitor), ("Xadrez", Icon::Chess), ("Música", Icon::Music),
        ("Contatos", Icon::Contacts), ("Agenda", Icon::Calendar), ("Vídeo", Icon::Video), ("Registros", Icon::Logs),
        ("Info", Icon::Info), ("Ajuda", Icon::Help), ("Dicionário", Icon::Dictionary), ("Fontes", Icon::FontBook),
        ("Correio", Icon::Mail), ("Monitores", Icon::Display), ("Chaves", Icon::Keychain), ("Ejetar", Icon::Eject),
        ("Áudio", Icon::AudioFile), ("Vídeo arq.", Icon::VideoFile), ("Compactado", Icon::ArchiveFile), ("PDF", Icon::PdfFile),
        ("Mesa", Icon::SpecialFolder(FolderKind::Desktop)), ("Documentos", Icon::SpecialFolder(FolderKind::Documents)), ("Início", Icon::SpecialFolder(FolderKind::Home)), ("Aplicativos", Icon::SpecialFolder(FolderKind::Applications)),
        ("Aparência", Icon::Pref(PrefKind::Appearance)), ("Dock", Icon::Pref(PrefKind::Dock)), ("Som", Icon::Pref(PrefKind::Sound)), ("Energia", Icon::Pref(PrefKind::Energy)),
        ("Rede", Icon::Pref(PrefKind::Network)), ("Segurança", Icon::Pref(PrefKind::Security)), ("Impressão", Icon::Pref(PrefKind::Printer)), ("Contas", Icon::Pref(PrefKind::Accounts)),
        ("Pasta", Icon::Folder), ("Código", Icon::CodeFile), ("Pendrive", Icon::Removable), ("CD/DVD", Icon::Optical), ("Servidor", Icon::Server),
        ("Downloads", Icon::SpecialFolder(FolderKind::Downloads)), ("Imagens", Icon::SpecialFolder(FolderKind::Pictures)), ("Músicas", Icon::SpecialFolder(FolderKind::Music)), ("Inteligente", Icon::SpecialFolder(FolderKind::Smart)),
    ];
    // Terceiro argumento "prefs": a folha dos painéis das Preferências e dos ícones restantes.
    let list = if args.next().as_deref() == Some("prefs") {
        use PrefKind::*;
        let mut v: Vec<(&str, Icon)> = [("Mostrar Todos", ShowAll), ("Aparência", Appearance), ("Dock", Dock), ("Teclado", Keyboard), ("Atalhos", Shortcuts), ("Mesa", Desktop), ("Som", Sound), ("Energia", Energy), ("Rede", Network), ("Data e Hora", DateTime), ("Itens", LoginItems), ("Acesso", Access), ("Contas", Accounts), ("Internacional", International), ("CDs e DVDs", Removable), ("Sem Fio", Wireless), ("Compartilhar", Sharing), ("Apps Padrão", DefaultApps), ("Impressão", Printer), ("Atualização", Update), ("Disco de Início", StartupDisk), ("Fala", Speech), ("Segurança", Security), ("Exposé", Expose)]
            .into_iter().map(|(n, k)| (n, Icon::Pref(k))).collect();
        v.extend([("Mouse", Icon::Mouse), ("Trackpad", Icon::Trackpad), ("Monitores", Icon::Display), ("Chaves", Icon::Keychain), ("Clipe", Icon::Clip), ("Janela", Icon::Window), ("Genérico", Icon::Generic), ("Música", Icon::Music), ("Util. de Rede", Icon::NetUtility), ("Visualizador", Icon::Viewer), ("Util. de Disco", Icon::DiskUtility), ("Util. de Arquivo", Icon::Archiver)]);
        v
    } else {
        list
    };
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
