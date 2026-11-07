//! Ícones vetoriais originais, desenhados em qualquer tamanho.

use crate::draw::*;
use crate::font::Fonts;
use tiny_skia::{Color, PathBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Terminal,
    Folder,
    /// O app Arquivos: pasta de gel azul com um rosto sorrindo.
    Files,
    Globe,
    TextEditor,
    Calculator,
    Settings,
    Window,
    Generic,
    /// Relógio mostrando (hora, minuto).
    Clock(u8, u8),
    Trash,
    /// Lixeira com papéis (há itens nela).
    TrashFull(u8),
    /// Símbolo de ejetar (o Lixo do Dock enquanto se arrasta um disco).
    Eject,
    /// Chaveiro com duas chaves douradas (Acesso às Chaves).
    Keychain,
    Disk,
    /// Arquivo de texto.
    Document,
    /// Arquivo de imagem.
    Picture,
    /// Arquivo de tipo desconhecido.
    File,
    Notes,
    Search,
    Chars,
    Camera,
    Monitor,
    Chess,
    /// Disco brilhante com uma nota musical (Música).
    Music,
    /// Caderno de couro marrom com um @ dourado (Contatos).
    Contacts,
    /// Folha de calendário com o dia de hoje (Agenda).
    Calendar,
    /// Tela escura com o triângulo de tocar e furos de filme (Vídeo).
    Video,
    /// Folha de registro com linhas coloridas (Registros).
    Logs,
    /// Chip prateado com um "i" (Informações do Sistema).
    Info,
    /// Bola azul brilhante com um "?" (Ajuda).
    Help,
    /// Livro escuro com "Aa" dourado (Dicionário).
    Dictionary,
    /// Livro claro com um "F" grande e uma fita (Livro de Fontes).
    FontBook,
    /// Clipe de papel (anexo).
    Clip,
    /// Selo picotado com uma andorinha e o carimbo (Correio).
    Mail,
    /// Tela plana (preferência de Monitor).
    Display,
    Mouse,
    Trackpad,
    /// Pasta especial: a pasta azul com um desenho em relevo na frente.
    SpecialFolder(FolderKind),
    AudioFile,
    VideoFile,
    ArchiveFile,
    PdfFile,
    CodeFile,
    /// Painéis das Preferências.
    Pref(PrefKind),
    /// Utilitário de Rede: globo em grade com a tela de radar.
    NetUtility,
    /// Visualizador: fotos com a lupa.
    Viewer,
    /// Casa (a pasta de início na barra lateral do Arquivos).
    House,
    /// Compasso de desenho em forma de "A" (Aplicativos na barra lateral).
    AppsPlace,
    /// Utilitário de Disco: o HD com o estetoscópio.
    DiskUtility,
    /// Utilitário de Arquivo: caixa de papelão com zíper.
    Archiver,
    /// Pendrive (disco removível).
    Removable,
    /// CD ou DVD.
    Optical,
    /// Servidor conectado (compartilhamento de rede).
    Server,
    /// Ícone de um tema do sistema, pelo nome (ex.: "firefox"); número da tabela de nomes.
    System(u32),
}

/// Ícones dos painéis das Preferências (desenhos próprios).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrefKind {
    /// Grade de painéis ("Mostrar Todos").
    ShowAll,
    /// Paleta com as cores de destaque.
    Appearance,
    /// Prateleira de vidro com ícones.
    Dock,
    Keyboard,
    /// Teclado com a tecla de atalho acesa.
    Shortcuts,
    /// Quadro com a mesa (papel de parede).
    Desktop,
    /// Alto-falante com ondas.
    Sound,
    /// Lâmpada acesa.
    Energy,
    /// Esfera azul com as ondas do Wi-Fi.
    Network,
    /// Folha de calendário com um relógio na frente.
    DateTime,
    /// Lista de apps com a seta verde de "abrir".
    LoginItems,
    /// Figura de braços abertos num círculo azul (símbolo de acessibilidade).
    Access,
    /// Duas silhuetas (contas de usuário).
    Accounts,
    /// Balão de fala com letras de vários alfabetos.
    International,
    /// Pendrive na frente de um disco.
    Removable,
    /// Fone sem fio com ondas.
    Wireless,
    /// Pasta azul com uma placa de "compartilhada".
    Sharing,
    /// Três ícones de app com um selo de "padrão".
    DefaultApps,
    /// Impressora com uma folha saindo.
    Printer,
    /// Caixa com as setas circulares de atualizar.
    Update,
    /// Disco rígido com o símbolo de ligar.
    StartupDisk,
    /// Rosto de perfil com ondas de som saindo da boca.
    Speech,
    /// Cadeado (Segurança).
    Security,
    /// Janelas espalhadas (Exposé e Painel).
    Expose,
}

/// Pastas especiais (Mesa, Documentos…), cada uma com seu desenho.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Applications,
    Home,
    Public,
    Templates,
    /// Pasta inteligente (busca salva): pasta roxa com engrenagem.
    Smart,
}

impl Icon {
    /// Já redesenhado na linguagem do Tiger (Fase 9.11): traz a própria sombra.
    pub fn tiger_style(self) -> bool {
        // Todos, menos os dos temas do sistema.
        !matches!(self, Icon::System(_))
    }

    pub fn from_name(s: &str) -> Self {
        match s {
            "terminal" => Icon::Terminal,
            "folder" => Icon::Folder,
            "files" => Icon::Files,
            "globe" | "browser" => Icon::Globe,
            "editor" | "text" => Icon::TextEditor,
            "calculator" => Icon::Calculator,
            "settings" => Icon::Settings,
            "window" => Icon::Window,
            "clock" => Icon::Clock(10, 10),
            "trash" => Icon::Trash,
            "trash-full" => Icon::TrashFull(crate::tiger::TRASH_FULL_AT),
            "disk" => Icon::Disk,
            "removable" | "pendrive" => Icon::Removable,
            "optical" | "cd" | "dvd" => Icon::Optical,
            "server" => Icon::Server,
            "netutil" => Icon::NetUtility,
            "viewer" => Icon::Viewer,
            "diskutil" => Icon::DiskUtility,
            "archiver" => Icon::Archiver,
            "house" => Icon::House,
            "apps-place" => Icon::AppsPlace,
            "document" => Icon::Document,
            "picture" | "image" => Icon::Picture,
            "file" => Icon::File,
            "notes" => Icon::Notes,
            "search" => Icon::Search,
            "chars" => Icon::Chars,
            "camera" => Icon::Camera,
            "monitor" => Icon::Monitor,
            "display" => Icon::Display,
            "mouse" => Icon::Mouse,
            "trackpad" => Icon::Trackpad,
            "chess" => Icon::Chess,
            "music" => Icon::Music,
            "contacts" => Icon::Contacts,
            "calendar" => Icon::Calendar,
            "video" => Icon::Video,
            "logs" => Icon::Logs,
            "info" => Icon::Info,
            "help" => Icon::Help,
            "dictionary" => Icon::Dictionary,
            "keychain" | "chaves" => Icon::Keychain,
            "fontbook" => Icon::FontBook,
            "mail" => Icon::Mail,
            "" | "generic" => Icon::Generic,
            // Qualquer outro nome: o ícone do tema do sistema (apps de fora).
            other => Icon::System(intern(other)),
        }
    }

    /// Nome do ícone equivalente nos temas do sistema (freedesktop), para usar o
    /// tema escolhido nas Preferências também nos apps e pastas do Glove.
    fn freedesktop_name(self) -> Option<&'static str> {
        Some(match self {
            Icon::Terminal => "utilities-terminal",
            Icon::Folder => "folder",
            Icon::Files => "system-file-manager",
            Icon::Globe => "web-browser",
            Icon::TextEditor => "accessories-text-editor",
            Icon::Calculator => "accessories-calculator",
            Icon::Settings => "preferences-system",
            Icon::Trash => "user-trash",
            Icon::TrashFull(_) => "user-trash-full",
            Icon::Eject => "media-eject",
            Icon::Disk => "drive-harddisk",
            Icon::Removable => "drive-removable-media-usb",
            Icon::Optical => "media-optical",
            Icon::Server => "network-server",
            Icon::NetUtility => "network-workgroup",
            Icon::Viewer => "image-viewer",
            Icon::DiskUtility => "gnome-disks",
            Icon::Archiver => "file-roller",
            Icon::House => "user-home",
            Icon::AppsPlace => "folder-applications",
            Icon::Document => "text-x-generic",
            Icon::Picture => "image-x-generic",
            Icon::File => "text-x-generic",
            Icon::Notes => "accessories-notes",
            Icon::Search => "system-search",
            Icon::Chars => "accessories-character-map",
            Icon::Camera => "applets-screenshooter",
            Icon::Monitor => "utilities-system-monitor",
            Icon::Chess => "gnome-chess",
            Icon::Music => "multimedia-audio-player",
            Icon::Contacts => "x-office-address-book",
            Icon::Video => "multimedia-video-player",
            Icon::Logs => "logviewer",
            Icon::Info => "dialog-information",
            Icon::Help => "help-browser",
            Icon::Dictionary => "accessories-dictionary",
            Icon::Keychain => "dialog-password",
            Icon::FontBook => "font-x-generic",
            Icon::Mail => "mail-client",
            Icon::Display => "video-display",
            Icon::Mouse => "input-mouse",
            Icon::Trackpad => "input-touchpad",
            Icon::AudioFile => "audio-x-generic",
            Icon::VideoFile => "video-x-generic",
            Icon::ArchiveFile => "package-x-generic",
            Icon::PdfFile => "application-pdf",
            Icon::CodeFile => "text-x-script",
            Icon::SpecialFolder(k) => match k {
                FolderKind::Desktop => "user-desktop",
                FolderKind::Documents => "folder-documents",
                FolderKind::Downloads => "folder-download",
                FolderKind::Pictures => "folder-pictures",
                FolderKind::Music => "folder-music",
                FolderKind::Videos => "folder-videos",
                FolderKind::Applications => "folder",
                FolderKind::Home => "user-home",
                FolderKind::Public => "folder-publicshare",
                FolderKind::Templates => "folder-templates",
                FolderKind::Smart => "folder-saved-search",
            },
            _ => return None,
        })
    }
}

/// Nomes dos ícones do sistema já vistos (o `Icon` guarda só o número).
fn names() -> &'static std::sync::Mutex<Vec<String>> {
    static N: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    N.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn intern(name: &str) -> u32 {
    let mut v = names().lock().unwrap();
    match v.iter().position(|n| n == name) {
        Some(i) => i as u32,
        None => {
            v.push(name.to_string());
            (v.len() - 1) as u32
        }
    }
}

/// Tema de ícones escolhido nas Preferências ("" = os desenhados pelo Glove).
static THEME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Troca o tema de ícones (chamado ao ler a configuração).
pub fn set_theme(name: &str) {
    if let Ok(mut t) = THEME.lock() {
        if *t != name {
            *t = name.to_string();
        }
    }
}

/// Tema do sistema para os apps de fora quando o Glove usa os próprios ícones:
/// o do GTK (o que o resto do sistema mostra), senão o Adwaita.
fn system_theme() -> String {
    static T: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        for f in [home.join(".config/gtk-3.0/settings.ini"), home.join(".config/gtk-4.0/settings.ini"), "/etc/gtk-3.0/settings.ini".into()] {
            if let Ok(t) = std::fs::read_to_string(f) {
                if let Some(v) = t.lines().find_map(|l| l.trim().strip_prefix("gtk-icon-theme-name")).and_then(|v| v.split('=').nth(1)) {
                    return v.trim().trim_matches('"').to_string();
                }
            }
        }
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "icon-theme"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Adwaita".into())
    })
    .clone()
}

/// Desenha a imagem de um tema do sistema; falso se o tema não tem o ícone.
fn draw_themed(pm: &mut Canvas, theme: &str, name: &str, x: f32, y: f32, s: f32) -> bool {
    let k = pm.scale;
    let px = (s * k).round() as u32;
    let Some(img) = crate::sysicons::image(theme, name, px) else { return false };
    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
    pm.draw_pixmap_t(img.as_ref().as_ref(), &paint, tiny_skia::Transform::from_translate((x * k).round(), (y * k).round()));
    true
}

/// Desenha o ícone ocupando o quadrado (x, y, s, s).
pub fn draw(pm: &mut Canvas, fonts: &Fonts, icon: Icon, x: f32, y: f32, s: f32, letter: Option<char>) {
    // Os ícones novos (estilo Tiger) já têm a sombra no formato do objeto.
    if icon.tiger_style() {
        draw_bare(pm, fonts, icon, x, y, s, letter);
        return;
    }
    // Sombra de contato suave sob o ícone (mesa, Arquivos).
    pm.fill(oval(x + s * 0.12, y + s * 0.86, s * 0.76, s * 0.12),
        &radial(x + s * 0.5, y + s * 0.92, s * 0.4, &[(0.0, rgba(0, 0, 0, 90)), (1.0, rgba(0, 0, 0, 0))]),
    );
    draw_bare(pm, fonts, icon, x, y, s, letter);
}

/// O ícone sem a sombra de contato (no Dock os ícones ficam direto no vidro).
pub fn draw_bare(pm: &mut Canvas, fonts: &Fonts, icon: Icon, x: f32, y: f32, s: f32, letter: Option<char>) {
    let chosen = THEME.lock().map(|t| t.clone()).unwrap_or_default();
    // Apps de fora: o ícone deles no tema (o escolhido ou o do sistema).
    if let Icon::System(i) = icon {
        let name = names().lock().ok().and_then(|v| v.get(i as usize).cloned()).unwrap_or_default();
        let theme = if chosen.is_empty() { system_theme() } else { chosen };
        if !draw_themed(pm, &theme, &name, x, y, s) {
            window(pm, fonts, x, y, s, letter);
        }
        return;
    }
    // Tema escolhido: os do Glove também (se o tema tiver o equivalente).
    if !chosen.is_empty() {
        if let Some(name) = icon.freedesktop_name() {
            if draw_themed(pm, &chosen, name, x, y, s) {
                return;
            }
        }
    }
    // Os ícones novos são detalhados: desenhados uma vez por tamanho e reaproveitados.
    if icon.tiger_style() && s >= 12.0 {
        if let Some((img, pad, side)) = cached(fonts, icon, letter, s, pm.scale) {
            // Centrado: o tamanho guardado pode diferir 1 pixel do pedido.
            let off = (s - side) / 2.0 - pad;
            pm.draw_canvas(&img, x + off, y + off);
            return;
        }
    }
    draw_raw(pm, fonts, icon, x, y, s, letter);
}

thread_local! {
    /// Ícones já desenhados: (ícone e o dia, para a Agenda; lado em pixels; escala) → imagem.
    static CACHE: std::cell::RefCell<Vec<((String, u32, u32), std::rc::Rc<Canvas>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// O ícone pronto no tamanho `s` (em pixels inteiros), com uma margem para a sombra.
fn cached(fonts: &Fonts, icon: Icon, letter: Option<char>, s: f32, scale: f32) -> Option<(std::rc::Rc<Canvas>, f32, f32)> {
    // Grandes (ampliação do Dock): de 2 em 2 pixels, metade dos desenhos.
    let raw = (s * scale).max(1.0);
    let px = if raw > 64.0 { ((raw / 2.0).round() * 2.0) as u32 } else { raw.round() as u32 };
    let side = px as f32 / scale;
    let pad = (side * 0.1).ceil();
    // A Agenda mostra o dia de hoje: muda de imagem à meia-noite.
    let day = if matches!(icon, Icon::Calendar) {
        unsafe {
            let t = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&t, &mut tm);
            tm.tm_yday as u32
        }
    } else {
        0
    };
    let key = (format!("{icon:?}{letter:?}{day}"), px, scale.to_bits());
    if let Some(c) = CACHE.with(|m| m.borrow().iter().find(|(k, _)| *k == key).map(|(_, c)| c.clone())) {
        return Some((c, pad, side));
    }
    let mut c = Canvas::new(side + 2.0 * pad, side + 2.0 * pad, scale)?;
    draw_raw(&mut c, fonts, icon, pad, pad, side, letter);
    let c = std::rc::Rc::new(c);
    CACHE.with(|m| {
        let mut m = m.borrow_mut();
        m.push((key, c.clone()));
        // No máximo uns 12 MB por processo: os mais antigos saem primeiro.
        let mut total: usize = m.iter().map(|(_, c)| c.data().len()).sum();
        while m.len() > 1 && total > 12 << 20 {
            total -= m[0].1.data().len();
            m.remove(0);
        }
    });
    Some((c, pad, side))
}

/// O desenho em si (sem tema e sem guardar).
fn draw_raw(pm: &mut Canvas, fonts: &Fonts, icon: Icon, x: f32, y: f32, s: f32, letter: Option<char>) {
    match icon {
        Icon::Terminal => crate::tiger_real::terminal(pm, x, y, s),
        Icon::Folder => folder(pm, x, y, s),
        Icon::Files => crate::tiger_real::files_app(pm, x, y, s),
        Icon::Globe => crate::tiger_real::compass(pm, fonts, x, y, s),
        Icon::TextEditor => crate::tiger_real::editor(pm, x, y, s),
        Icon::Calculator => crate::tiger::calculator(pm, fonts, x, y, s),
        Icon::Settings => crate::tiger_real::settings(pm, x, y, s),
        Icon::Window => crate::tiger_prefs::window(pm, fonts, x, y, s, letter),
        Icon::System(_) => window(pm, fonts, x, y, s, letter),
        Icon::Generic => crate::tiger_prefs::generic(pm, fonts, x, y, s, letter),
        Icon::Clock(h, m) => crate::tiger::clock(pm, fonts, x, y, s, h, m),
        Icon::Trash => crate::tiger_real::trash(pm, x, y, s, 0),
        Icon::TrashFull(n) => crate::tiger_real::trash(pm, x, y, s, n),
        Icon::Eject => crate::tiger::eject(pm, x, y, s),
        Icon::Removable => crate::tiger::removable(pm, x, y, s),
        Icon::Optical => crate::tiger::optical(pm, x, y, s),
        Icon::Server => crate::tiger::server(pm, x, y, s),
        Icon::NetUtility => crate::tiger_prefs::net_utility(pm, x, y, s),
        Icon::Viewer => crate::tiger_real::viewer(pm, x, y, s),
        Icon::DiskUtility => crate::tiger_real::disk_utility(pm, x, y, s),
        Icon::Archiver => crate::tiger_prefs::archiver(pm, x, y, s),
        Icon::House => crate::tiger_real::house(pm, x, y, s),
        Icon::AppsPlace => crate::tiger_real::apps_place(pm, x, y, s),
        Icon::Disk => crate::tiger_real::disk(pm, x, y, s),
        Icon::Document => crate::tiger::document(pm, x, y, s, true),
        Icon::File => crate::tiger::document(pm, x, y, s, false),
        Icon::Picture => crate::tiger::picture(pm, x, y, s),
        Icon::Notes => crate::tiger::notes(pm, x, y, s),
        Icon::Search => crate::tiger::search(pm, x, y, s),
        Icon::Chars => crate::tiger::chars(pm, fonts, x, y, s),
        Icon::Camera => crate::tiger_real::capture(pm, x, y, s),
        Icon::Monitor => crate::tiger_real::activity(pm, x, y, s),
        Icon::Chess => crate::tiger_real::chess(pm, x, y, s),
        Icon::Music => crate::tiger_real::music(pm, x, y, s),
        Icon::Contacts => crate::tiger_real::contacts(pm, fonts, x, y, s),
        Icon::Calendar => {
            // Mês e dia de hoje no horário local.
            let (m, d) = unsafe {
                let t = libc::time(std::ptr::null_mut());
                let mut tm: libc::tm = std::mem::zeroed();
                libc::localtime_r(&t, &mut tm);
                (tm.tm_mon as u32 + 1, tm.tm_mday as u32)
            };
            crate::tiger::calendar(pm, fonts, x, y, s, m, d)
        }
        Icon::Video => crate::tiger::video(pm, x, y, s),
        Icon::Logs => crate::tiger::logs(pm, x, y, s),
        Icon::Info => crate::tiger::info(pm, fonts, x, y, s),
        Icon::Help => crate::tiger::help(pm, fonts, x, y, s),
        Icon::Dictionary => crate::tiger::dictionary(pm, fonts, x, y, s),
        Icon::Keychain => crate::tiger_prefs::keychain(pm, x, y, s),
        Icon::FontBook => crate::tiger_real::font_book(pm, fonts, x, y, s),
        Icon::Clip => crate::tiger_prefs::clip(pm, x, y, s),
        Icon::Mail => crate::tiger_real::mail(pm, fonts, x, y, s),
        Icon::Display => crate::tiger::display_pref(pm, x, y, s),
        Icon::Mouse => crate::tiger_prefs::mouse(pm, x, y, s),
        Icon::Trackpad => crate::tiger_prefs::trackpad(pm, x, y, s),
        Icon::SpecialFolder(FolderKind::Smart) => {
            crate::tiger::folder(pm, x, y, s, rgba(150, 100, 210, 255));
            folder_emblem(pm, FolderKind::Smart, x + s * 0.5, y + s * 0.585, s * 0.34);
        }
        Icon::SpecialFolder(k) => {
            folder(pm, x, y, s);
            folder_emblem(pm, k, x + s * 0.5, y + s * 0.585, s * 0.34);
        }
        Icon::Pref(k) => pref_icon(pm, fonts, k, x, y, s),
        Icon::AudioFile => crate::tiger::typed_document(pm, fonts, x, y, s, 1),
        Icon::VideoFile => crate::tiger::typed_document(pm, fonts, x, y, s, 2),
        Icon::ArchiveFile => crate::tiger::typed_document(pm, fonts, x, y, s, 3),
        Icon::PdfFile => crate::tiger::typed_document(pm, fonts, x, y, s, 4),
        Icon::CodeFile => crate::tiger::typed_document(pm, fonts, x, y, s, 5),
    }
}

/// Desenho em relevo na frente da pasta: azul escuro com um brilho embaixo.
fn folder_emblem(pm: &mut Canvas, k: FolderKind, cx: f32, cy: f32, s: f32) {
    // Pequenas (listas e barra lateral): desenho maior, sem o brilho e bem escuro,
    // senão o relevo some na pasta de 16 a 24 pixels.
    if s < 9.0 {
        emblem(pm, k, cx, cy, s * 1.2, rgba(22, 62, 140, 255));
        return;
    }
    let dark = rgba(38, 88, 162, 215);
    let light = rgba(235, 245, 255, 200);
    for (dy, col) in [(s * 0.04, light), (0.0, dark)] {
        emblem(pm, k, cx, cy + dy, s, col);
    }
}

fn emblem(pm: &mut Canvas, k: FolderKind, cx: f32, cy: f32, s: f32, col: Color) {
    let w = (s * 0.12).max(1.0);
    let ink = solid(col);
    let h = s / 2.0;
    let mut pb = PathBuilder::new();
    match k {
        FolderKind::Desktop => {
            pm.stroke(rrect(cx - h, cy - h * 0.75, s, h * 1.3, s * 0.06), &ink, w);
            pm.fill_rect(cx - s * 0.08, cy + h * 0.55, s * 0.16, s * 0.14, &ink);
            pm.fill_rect(cx - s * 0.25, cy + h * 0.82, s * 0.5, w, &ink);
        }
        FolderKind::Documents | FolderKind::Templates => {
            pm.stroke(rrect(cx - h * 0.7, cy - h, s * 0.7, s, s * 0.04), &ink, w);
            for i in 0..3 {
                let y = cy - h * 0.45 + i as f32 * s * 0.22;
                pm.fill_rect(cx - h * 0.45, y, s * 0.45, w * 0.8, &ink);
            }
        }
        FolderKind::Downloads => {
            pb.move_to(cx, cy - h);
            pb.line_to(cx, cy + h * 0.35);
            pb.move_to(cx - h * 0.55, cy - h * 0.15);
            pb.line_to(cx, cy + h * 0.4);
            pb.line_to(cx + h * 0.55, cy - h * 0.15);
            pb.move_to(cx - h, cy + h * 0.5);
            pb.line_to(cx - h, cy + h);
            pb.line_to(cx + h, cy + h);
            pb.line_to(cx + h, cy + h * 0.5);
            pm.stroke_round(pb.finish(), &ink, w);
        }
        FolderKind::Pictures => {
            pm.stroke(rrect(cx - h, cy - h * 0.75, s, h * 1.5, s * 0.06), &ink, w);
            pb.move_to(cx - h * 0.8, cy + h * 0.6);
            pb.line_to(cx - h * 0.2, cy - h * 0.1);
            pb.line_to(cx + h * 0.2, cy + h * 0.3);
            pb.line_to(cx + h * 0.45, cy + h * 0.05);
            pb.line_to(cx + h * 0.8, cy + h * 0.6);
            pb.close();
            pm.fill(pb.finish(), &ink);
            pm.fill(circle(cx + h * 0.45, cy - h * 0.35, s * 0.08), &ink);
        }
        FolderKind::Music => {
            pm.fill(oval(cx - h * 0.75, cy + h * 0.35, s * 0.36, s * 0.26), &ink);
            pm.fill_rect(cx - h * 0.05, cy - h, w, h * 1.55, &ink);
            pb.move_to(cx - h * 0.05 + w, cy - h);
            pb.quad_to(cx + h * 0.4, cy - h * 0.7, cx + h * 0.6, cy - h * 0.2);
            pm.stroke_round(pb.finish(), &ink, w);
        }
        FolderKind::Videos => {
            pm.stroke(rrect(cx - h, cy - h * 0.7, s, h * 1.4, s * 0.04), &ink, w);
            for i in 0..4 {
                let x = cx - h * 0.8 + i as f32 * s * 0.24;
                pm.fill_rect(x, cy - h * 0.55, s * 0.1, s * 0.08, &ink);
                pm.fill_rect(x, cy + h * 0.39, s * 0.1, s * 0.08, &ink);
            }
        }
        FolderKind::Applications => {
            // Um "A" de régua e lápis, sem letra.
            pb.move_to(cx - h * 0.8, cy + h);
            pb.line_to(cx, cy - h);
            pb.line_to(cx + h * 0.8, cy + h);
            pb.move_to(cx - h * 0.45, cy + h * 0.2);
            pb.line_to(cx + h * 0.45, cy + h * 0.2);
            pm.stroke_round(pb.finish(), &ink, w * 1.2);
        }
        FolderKind::Home => {
            pb.move_to(cx - h, cy);
            pb.line_to(cx, cy - h);
            pb.line_to(cx + h, cy);
            pb.move_to(cx - h * 0.7, cy - h * 0.2);
            pb.line_to(cx - h * 0.7, cy + h);
            pb.line_to(cx + h * 0.7, cy + h);
            pb.line_to(cx + h * 0.7, cy - h * 0.2);
            pm.stroke_round(pb.finish(), &ink, w);
            pm.fill_rect(cx - s * 0.08, cy + h * 0.3, s * 0.16, h * 0.7, &ink);
        }
        FolderKind::Smart => {
            // Engrenagem: dentes em volta de um aro.
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let (c, sn) = (a.cos(), a.sin());
                pm.fill(circle(cx + c * h * 0.78, cy + sn * h * 0.78, s * 0.11), &ink);
            }
            pm.fill(circle(cx, cy, h * 0.72), &ink);
            pm.fill(circle(cx, cy, h * 0.3), &solid(rgba(150, 100, 210, 255)));
        }
        FolderKind::Public => {
            pm.fill(circle(cx, cy - h * 0.45, s * 0.17), &ink);
            pm.fill(rrect4(cx - h * 0.65, cy + h * 0.05, s * 0.65, h * 0.95, [s * 0.3, s * 0.3, 0.0, 0.0]), &ink);
        }
    }
}

/// Marca de atalho: setinha curva num quadradinho branco, no canto de baixo à esquerda.
pub fn alias_badge(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let b = s * 0.32;
    let (bx, by) = (x + s * 0.08, y + s - b - s * 0.06);
    pm.fill(rrect(bx, by, b, b, b * 0.18), &solid(Color::WHITE));
    pm.stroke(rrect(bx, by, b, b, b * 0.18), &solid(rgba(60, 60, 66, 255)), (s / 64.0).max(0.8));
    let mut pb = PathBuilder::new();
    pb.move_to(bx + b * 0.25, by + b * 0.8);
    pb.quad_to(bx + b * 0.25, by + b * 0.35, bx + b * 0.68, by + b * 0.35);
    pm.stroke_round(pb.finish(), &solid(rgba(20, 20, 24, 255)), (b * 0.12).max(1.0));
    let mut t = PathBuilder::new();
    t.move_to(bx + b * 0.6, by + b * 0.16);
    t.line_to(bx + b * 0.85, by + b * 0.35);
    t.line_to(bx + b * 0.6, by + b * 0.54);
    t.close();
    pm.fill(t.finish(), &solid(rgba(20, 20, 24, 255)));
}

fn folder(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    crate::tiger::folder(pm, x, y, s, rgba(88, 150, 226, 255));
}



fn window(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let (wx, wy, ww, wh) = (x + s * 0.1, y + s * 0.16, s * 0.8, s * 0.66);
    pm.fill(rrect(wx, wy, ww, wh, s * 0.06), &solid(rgba(250, 250, 250, 255)));
    pm.pinstripes(wx, wy + s * 0.14, ww, wh - s * 0.14, rgba(232, 232, 232, 255), rgba(246, 246, 246, 255));
    pm.fill(rrect4(wx, wy, ww, s * 0.14, [s * 0.06, s * 0.06, 0.0, 0.0]), &vgrad(wy, wy + s * 0.14, &[(0.0, rgba(240, 240, 240, 255)), (1.0, rgba(195, 195, 200, 255))]));
    pm.stroke(rrect(wx, wy, ww, wh, s * 0.06), &solid(rgba(110, 110, 120, 255)), 1.0);
    for (i, c) in [rgba(230, 70, 60, 255), rgba(240, 180, 40, 255), rgba(80, 190, 70, 255)].into_iter().enumerate() {
        pm.fill(circle(wx + s * 0.08 + i as f32 * s * 0.08, wy + s * 0.07, s * 0.03), &solid(c));
    }
    if let Some(l) = letter {
        let t = l.to_uppercase().to_string();
        let px = s * 0.34;
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, wx + (ww - tw) / 2.0, wy + wh * 0.78, px, true, rgba(60, 70, 90, 255));
    }
}

/// Marca do menu principal: uma gota de gel azul.
pub fn logo(pm: &mut Canvas, x: f32, y: f32, s: f32, accent: tiny_skia::Color) {
    gel_circle(pm, x + s / 2.0, y + s / 2.0, s / 2.0, accent);
}


// ---------------------------------------------------------------------------
// Ícones de alerta (arte própria: placa vermelha de oito lados e triângulo amarelo)
// ---------------------------------------------------------------------------

/// Ponto de exclamação vetorial centrado em `cx`, ocupando de `top` até `top + h`.
fn exclamation(pm: &mut Canvas, cx: f32, top: f32, h: f32, color: Color) {
    let w = h * 0.2;
    let bar = h * 0.66;
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(cx - w / 2.0, top + w / 2.0);
    pb.quad_to(cx, top - w * 0.15, cx + w / 2.0, top + w / 2.0);
    pb.line_to(cx + w * 0.28, top + bar);
    pb.line_to(cx - w * 0.28, top + bar);
    pb.close();
    pm.fill(pb.finish(), &solid(color));
    pm.fill(circle(cx, top + h - w * 0.55, w * 0.55), &solid(color));
}

/// Alerta grave: placa vermelha de oito lados em gel, com "!" branco.
pub fn alert_stop(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    let oct = |r: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        for i in 0..8 {
            let a = (i as f32 + 0.5) / 8.0 * std::f32::consts::TAU;
            let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        pb.close();
        pb.finish()
    };
    let r = s * 0.46;
    pm.fill(oval(x + s * 0.18, y + s * 0.88, s * 0.64, s * 0.1), &radial(cx, y + s * 0.93, s * 0.32, &[(0.0, rgba(0, 0, 0, 70)), (1.0, rgba(0, 0, 0, 0))]));
    pm.fill(oct(r), &solid(rgba(255, 255, 255, 255)));
    pm.fill(oct(r - s * 0.035), &solid(rgba(150, 10, 10, 255)));
    pm.fill(oct(r - s * 0.05), &radial(cx, cy + r * 0.5, r * 1.3, &[(0.0, rgba(255, 70, 60, 255)), (0.6, rgba(220, 20, 20, 255)), (1.0, rgba(170, 0, 0, 255))]));
    // Brilho do gel na metade de cima.
    pm.fill(oval(cx - r * 0.7, cy - r * 0.92, r * 1.4, r * 0.8), &vgrad(cy - r, cy - r * 0.15, &[(0.0, rgba(255, 255, 255, 170)), (1.0, rgba(255, 255, 255, 10))]));
    exclamation(pm, cx, cy - r * 0.62, r * 1.22, rgba(255, 255, 255, 255));
}

/// Cuidado: triângulo amarelo de cantos arredondados em gel, com "!" escuro.
pub fn alert_caution(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let tri = |inset: f32| {
        let (l, r, t, b) = (x + s * 0.04 + inset, x + s * 0.96 - inset, y + s * 0.08 + inset * 1.4, y + s * 0.9 - inset * 0.6);
        let mx = x + s / 2.0;
        let k = s * 0.07;
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(mx - k * 0.6, t + k);
        pb.quad_to(mx, t - k * 0.2, mx + k * 0.6, t + k);
        pb.line_to(r - k * 0.2, b - k);
        pb.quad_to(r + k * 0.3, b, r - k, b);
        pb.line_to(l + k, b);
        pb.quad_to(l - k * 0.3, b, l + k * 0.2, b - k);
        pb.close();
        pb.finish()
    };
    pm.fill(oval(x + s * 0.12, y + s * 0.86, s * 0.76, s * 0.1), &radial(x + s / 2.0, y + s * 0.91, s * 0.38, &[(0.0, rgba(0, 0, 0, 60)), (1.0, rgba(0, 0, 0, 0))]));
    pm.fill(tri(0.0), &solid(rgba(150, 105, 0, 255)));
    pm.fill(tri(s * 0.025), &vgrad(y + s * 0.1, y + s * 0.9, &[(0.0, rgba(255, 236, 120, 255)), (0.55, rgba(252, 204, 30, 255)), (1.0, rgba(236, 160, 10, 255))]));
    pm.fill(tri(s * 0.09), &vgrad(y + s * 0.15, y + s * 0.6, &[(0.0, rgba(255, 255, 255, 120)), (1.0, rgba(255, 255, 255, 0))]));
    exclamation(pm, x + s / 2.0, y + s * 0.3, s * 0.48, rgba(30, 25, 10, 255));
}

// ---------------------------------------------------------------------------
// Glifos pequenos (controles segmentados e botões de barra de ferramentas)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// Visualizar como ícones: grade de 2×2.
    ViewIcons,
    /// Visualizar como lista: linhas.
    ViewList,
    /// Visualizar em colunas: três colunas.
    ViewColumns,
    Back,
    Forward,
    /// Menu de ação: engrenagem com setinha para baixo.
    Action,
    Plus,
    Minus,
    /// Linhas de comprimentos diferentes, encostadas à esquerda/no meio/à direita.
    AlignLeft,
    AlignCenter,
    AlignRight,
    /// Seta curva girando para a esquerda / direita.
    RotateLeft,
    RotateRight,
    /// Gaveta lateral (miniaturas).
    Sidebar,
}

/// Desenha o glifo centrado em (cx, cy), num quadrado de `s` pontos.
pub fn glyph(pm: &mut Canvas, g: Glyph, cx: f32, cy: f32, s: f32, color: Color) {
    let ink = solid(color);
    let h = s / 2.0;
    match g {
        Glyph::ViewIcons => {
            let q = s * 0.36;
            let gap = s * 0.14;
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let x = cx - q - gap / 2.0 + dx * (q + gap);
                let y = cy - q - gap / 2.0 + dy * (q + gap);
                pm.stroke(rrect(x + 0.5, y + 0.5, q - 1.0, q - 1.0, 0.5), &ink, 1.0);
            }
        }
        Glyph::RotateLeft | Glyph::RotateRight => {
            let left = g == Glyph::RotateLeft;
            let r = s * 0.36;
            let mut pb = PathBuilder::new();
            let (a0, a1) = (-2.6f32, 1.2f32);
            for i in 0..=14 {
                let a = a0 + (a1 - a0) * i as f32 / 14.0;
                let (x, y) = (cx + if left { -1.0 } else { 1.0 } * r * a.cos(), cy + r * a.sin());
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            pm.stroke(pb.finish(), &ink, 1.4);
            // Ponta da seta no começo do arco.
            let (tx, ty) = (cx + if left { -1.0 } else { 1.0 } * r * a0.cos(), cy + r * a0.sin());
            let d = if left { 1.0 } else { -1.0 };
            let mut pb = PathBuilder::new();
            pb.move_to(tx - d * s * 0.2, ty - s * 0.02);
            pb.line_to(tx + d * s * 0.06, ty - s * 0.2);
            pb.line_to(tx + d * s * 0.08, ty + s * 0.1);
            pb.close();
            pm.fill(pb.finish(), &ink);
        }
        Glyph::Sidebar => {
            pm.stroke(rrect(cx - h + 0.5, cy - h * 0.75 + 0.5, s - 1.0, s * 0.75 * 2.0 - 1.0, 1.0), &ink, 1.0);
            pm.fill_rect(cx - h + s * 0.32, cy - h * 0.75, 1.0, s * 0.75 * 2.0, &ink);
            for i in 0..3 {
                pm.fill_rect(cx - h + s * 0.08, cy - h * 0.5 + i as f32 * s * 0.22, s * 0.16, s * 0.12, &ink);
            }
        }
        Glyph::AlignLeft | Glyph::AlignCenter | Glyph::AlignRight => {
            let lens = [1.0, 0.6, 0.85, 0.5];
            for (i, l) in lens.iter().enumerate() {
                let y = (cy - h * 0.75 + i as f32 * h * 0.5).round() + 0.5;
                let lw = (s * l).round();
                let x = match g {
                    Glyph::AlignLeft => cx - h,
                    Glyph::AlignCenter => cx - lw / 2.0,
                    _ => cx + h - lw,
                };
                pm.fill_rect(x.round(), y - 0.5, lw, 1.0, &ink);
            }
        }
        Glyph::ViewList => {
            for i in 0..4 {
                let y = (cy - h * 0.75 + i as f32 * h * 0.5).round() + 0.5;
                pm.fill_rect(cx - h, y - 0.5, s, 1.0, &ink);
            }
        }
        Glyph::ViewColumns => {
            pm.stroke(rrect(cx - h + 0.5, cy - h * 0.75 + 0.5, s - 1.0, s * 0.75 - 1.0, 0.0), &ink, 1.0);
            for k in [1.0, 2.0] {
                let x = (cx - h + s * k / 3.0).round() + 0.5;
                pm.fill_rect(x - 0.5, cy - h * 0.75, 1.0, s * 0.75, &ink);
            }
        }
        Glyph::Back | Glyph::Forward => {
            let d = if g == Glyph::Back { -1.0 } else { 1.0 };
            let mut pb = PathBuilder::new();
            pb.move_to(cx + d * h * 0.55, cy);
            pb.line_to(cx - d * h * 0.45, cy - h * 0.6);
            pb.line_to(cx - d * h * 0.45, cy + h * 0.6);
            pb.close();
            pm.fill(pb.finish(), &ink);
        }
        Glyph::Action => {
            // Engrenagem à esquerda e setinha à direita.
            let (gx, r) = (cx - s * 0.18, s * 0.36);
            let mut pb = PathBuilder::new();
            let teeth = 8;
            for i in 0..teeth * 2 {
                let a = i as f32 / (teeth * 2) as f32 * std::f32::consts::TAU;
                let a2 = (i as f32 + 1.0) / (teeth * 2) as f32 * std::f32::consts::TAU;
                let rr = if i % 2 == 0 { r } else { r * 0.72 };
                let (p1, p2) = ((gx + rr * a.cos(), cy + rr * a.sin()), (gx + rr * a2.cos(), cy + rr * a2.sin()));
                if i == 0 { pb.move_to(p1.0, p1.1) } else { pb.line_to(p1.0, p1.1) }
                pb.line_to(p2.0, p2.1);
            }
            pb.close();
            pb.push_circle(gx, cy, r * 0.32);
            if let Some(p) = pb.finish() {
                pm.fill_rule(Some(p), &ink, tiny_skia::FillRule::EvenOdd);
            }
            let ax = cx + s * 0.38;
            let mut pb = PathBuilder::new();
            pb.move_to(ax - s * 0.13, cy - s * 0.04);
            pb.line_to(ax + s * 0.13, cy - s * 0.04);
            pb.line_to(ax, cy + s * 0.12);
            pb.close();
            pm.fill(pb.finish(), &ink);
        }
        Glyph::Plus | Glyph::Minus => {
            let t = (s * 0.14).max(1.5);
            pm.fill_rect(cx - h * 0.7, cy - t / 2.0, s * 0.7, t, &ink);
            if g == Glyph::Plus {
                pm.fill_rect(cx - t / 2.0, cy - h * 0.7, t, s * 0.7, &ink);
            }
        }
    }
}

/// Lupa (campo de busca).
pub fn magnifier(pm: &mut Canvas, cx: f32, cy: f32, s: f32, color: Color) {
    let r = s * 0.3;
    let (ox, oy) = (cx - s * 0.1, cy - s * 0.1);
    pm.stroke(circle(ox, oy, r), &solid(color), (s * 0.13).max(1.3));
    let mut pb = PathBuilder::new();
    pb.move_to(ox + r * 0.75, oy + r * 0.75);
    pb.line_to(cx + s * 0.42, cy + s * 0.42);
    pm.stroke_round(pb.finish(), &solid(color), (s * 0.18).max(1.8));
}

// ---------------------------------------------------------------------------
// Painéis das Preferências
// ---------------------------------------------------------------------------

fn pref_icon(pm: &mut Canvas, fonts: &Fonts, k: PrefKind, x: f32, y: f32, s: f32) {
    use crate::tiger_prefs as p;
    match k {
        PrefKind::ShowAll => p::show_all(pm, x, y, s),
        PrefKind::Appearance => p::appearance(pm, x, y, s),
        PrefKind::Dock => p::dock(pm, x, y, s),
        PrefKind::Keyboard => p::keyboard(pm, x, y, s, false),
        PrefKind::Shortcuts => p::keyboard(pm, x, y, s, true),
        PrefKind::Desktop => p::desktop(pm, x, y, s),
        PrefKind::Sound => p::sound(pm, x, y, s),
        PrefKind::Energy => p::energy(pm, x, y, s),
        PrefKind::Network => p::network(pm, x, y, s),
        PrefKind::DateTime => p::date_time(pm, fonts, x, y, s),
        PrefKind::LoginItems => p::login_items(pm, x, y, s),
        PrefKind::Access => p::access(pm, x, y, s),
        PrefKind::Accounts => p::accounts(pm, x, y, s),
        PrefKind::International => p::international(pm, fonts, x, y, s),
        PrefKind::Removable => p::cds(pm, x, y, s),
        PrefKind::Wireless => p::wireless(pm, x, y, s),
        PrefKind::Sharing => p::sharing(pm, x, y, s),
        PrefKind::DefaultApps => p::default_apps(pm, x, y, s),
        PrefKind::Printer => p::printer(pm, x, y, s),
        PrefKind::Update => p::update(pm, x, y, s),
        PrefKind::StartupDisk => p::startup_disk(pm, x, y, s),
        PrefKind::Speech => p::speech(pm, x, y, s),
        PrefKind::Security => p::security(pm, x, y, s),
        PrefKind::Expose => p::expose(pm, x, y, s),
    }
}

/// Ícone de um arquivo pela extensão (o mesmo critério do Arquivos); pastas = `Folder`.
pub fn file_icon(path: &std::path::Path, is_dir: bool) -> Icon {
    if is_dir {
        return Icon::Folder;
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" | "log" | "conf" | "ini" | "csv" | "rtf" => Icon::Document,
        "rs" | "toml" | "json" | "c" | "h" | "cpp" | "hpp" | "py" | "sh" | "xml" | "html" | "css" | "js" | "ts" | "yaml" | "yml" | "go" | "java" | "rb" | "lua" => Icon::CodeFile,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "tif" | "tiff" => Icon::Picture,
        "mp3" | "ogg" | "oga" | "flac" | "wav" | "m4a" | "opus" | "aac" => Icon::AudioFile,
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "ogv" => Icon::VideoFile,
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" | "zst" | "deb" | "rpm" => Icon::ArchiveFile,
        "pdf" => Icon::PdfFile,
        _ => Icon::File,
    }
}

/// Ícone de um disco montado: servidor (gvfs), CD/DVD, pendrive ou disco externo.
pub fn mount_icon(dev: &str, path: &std::path::Path) -> Icon {
    if dev.starts_with("gvfs:") || path.to_string_lossy().contains("/gvfs/") {
        return Icon::Server;
    }
    let name = dev.trim_start_matches("/dev/");
    if name.starts_with("sr") || name.starts_with("cdrom") {
        return Icon::Optical;
    }
    // "sdb1" -> "sdb"; o kernel diz se o aparelho é removível.
    let base: String = name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    let base = base.strip_suffix('p').filter(|b| b.starts_with("nvme") || b.starts_with("mmcblk")).map(str::to_string).unwrap_or(base);
    match std::fs::read_to_string(format!("/sys/block/{base}/removable")) {
        Ok(v) if v.trim() == "1" => Icon::Removable,
        _ if name.starts_with("mmcblk") => Icon::Removable,
        _ => Icon::Disk,
    }
}

/// Ícones do tema "Glove" (freedesktop): (ícone, contexto, nomes). O primeiro
/// nome é o arquivo; os outros viram atalhos para ele (nomes comuns dos apps
/// e dos temas GNOME, para os apps de fora também usarem os desenhos).
pub fn theme_entries() -> Vec<(Icon, &'static str, &'static [&'static str])> {
    use FolderKind as F;
    use PrefKind as P;
    vec![
        // Apps
        (Icon::Files, "apps", &["system-file-manager", "org.gnome.Nautilus", "nautilus", "glove-files"]),
        (Icon::Terminal, "apps", &["utilities-terminal", "org.gnome.Terminal", "org.gnome.Ptyxis", "org.gnome.Console", "glove-term"]),
        (Icon::Globe, "apps", &["web-browser", "glove-navegador"]),
        (Icon::TextEditor, "apps", &["accessories-text-editor", "org.gnome.TextEditor", "gedit", "glove-edit"]),
        (Icon::Calculator, "apps", &["accessories-calculator", "org.gnome.Calculator", "glove-calc"]),
        (Icon::Settings, "apps", &["preferences-system", "preferences-desktop", "org.gnome.Settings", "gnome-control-center", "glove-prefs"]),
        (Icon::Clock(10, 9), "apps", &["preferences-system-time", "org.gnome.clocks", "glove-clock"]),
        (Icon::Mail, "apps", &["mail-client", "internet-mail", "org.gnome.Evolution", "thunderbird", "glove-correio"]),
        (Icon::Notes, "apps", &["accessories-notes", "org.gnome.Notes", "glove-notes"]),
        (Icon::Contacts, "apps", &["x-office-address-book", "org.gnome.Contacts", "glove-contatos"]),
        (Icon::Calendar, "apps", &["x-office-calendar", "org.gnome.Calendar", "glove-agenda"]),
        (Icon::Search, "apps", &["system-search", "glove-find"]),
        (Icon::Chars, "apps", &["accessories-character-map", "org.gnome.Characters", "glove-chars"]),
        (Icon::Camera, "apps", &["applets-screenshooter", "org.gnome.Screenshot", "glove-shot"]),
        (Icon::Monitor, "apps", &["utilities-system-monitor", "org.gnome.SystemMonitor", "gnome-system-monitor", "glove-monitor"]),
        (Icon::Chess, "apps", &["gnome-chess", "org.gnome.Chess", "glove-chess"]),
        (Icon::Music, "apps", &["multimedia-audio-player", "org.gnome.Music", "rhythmbox", "glove-musica"]),
        (Icon::Video, "apps", &["multimedia-video-player", "org.gnome.Totem", "totem", "glove-video"]),
        (Icon::Logs, "apps", &["logviewer", "org.gnome.Logs", "glove-registros"]),
        (Icon::Info, "apps", &["dialog-information", "glove-info"]),
        (Icon::Help, "apps", &["help-browser", "help-contents", "org.gnome.Yelp", "glove-ajuda"]),
        (Icon::Dictionary, "apps", &["accessories-dictionary", "org.gnome.Dictionary", "glove-dicionario"]),
        (Icon::FontBook, "apps", &["preferences-desktop-font", "org.gnome.font-viewer", "glove-fontes"]),
        (Icon::Keychain, "apps", &["dialog-password", "seahorse", "org.gnome.seahorse.Application", "glove-chaves"]),
        (Icon::NetUtility, "apps", &["network-workgroup", "glove-utilrede"]),
        (Icon::Viewer, "apps", &["image-viewer", "org.gnome.eog", "org.gnome.Loupe", "eog", "glove-view"]),
        (Icon::DiskUtility, "apps", &["gnome-disks", "org.gnome.DiskUtility", "glove-disks"]),
        (Icon::Archiver, "apps", &["file-roller", "org.gnome.FileRoller", "glove-arquivo"]),
        (Icon::Window, "apps", &["application-default-icon", "application-x-executable"]),
        // Lugares
        (Icon::Folder, "places", &["folder", "inode-directory", "folder-remote"]),
        (Icon::SpecialFolder(F::Desktop), "places", &["user-desktop", "folder-desktop"]),
        (Icon::SpecialFolder(F::Documents), "places", &["folder-documents"]),
        (Icon::SpecialFolder(F::Downloads), "places", &["folder-download", "folder-downloads"]),
        (Icon::SpecialFolder(F::Pictures), "places", &["folder-pictures", "folder-images"]),
        (Icon::SpecialFolder(F::Music), "places", &["folder-music"]),
        (Icon::SpecialFolder(F::Videos), "places", &["folder-videos"]),
        (Icon::SpecialFolder(F::Applications), "places", &["folder-applications"]),
        (Icon::SpecialFolder(F::Home), "places", &["user-home", "folder-home"]),
        (Icon::SpecialFolder(F::Public), "places", &["folder-publicshare"]),
        (Icon::SpecialFolder(F::Templates), "places", &["folder-templates"]),
        (Icon::SpecialFolder(F::Smart), "places", &["folder-saved-search"]),
        (Icon::Trash, "places", &["user-trash"]),
        (Icon::TrashFull(crate::tiger::TRASH_FULL_AT), "status", &["user-trash-full"]),
        (Icon::Server, "places", &["network-server"]),
        // Aparelhos
        (Icon::Disk, "devices", &["drive-harddisk", "drive-harddisk-system", "drive-harddisk-usb"]),
        (Icon::Removable, "devices", &["drive-removable-media-usb", "drive-removable-media", "media-flash"]),
        (Icon::Optical, "devices", &["media-optical", "drive-optical", "media-optical-cd", "media-optical-dvd"]),
        (Icon::Display, "devices", &["video-display", "preferences-desktop-display"]),
        (Icon::Mouse, "devices", &["input-mouse", "preferences-desktop-peripherals"]),
        (Icon::Trackpad, "devices", &["input-touchpad"]),
        (Icon::Pref(P::Keyboard), "devices", &["input-keyboard", "preferences-desktop-keyboard"]),
        (Icon::Pref(P::Printer), "devices", &["printer", "printer-network", "preferences-desktop-printer"]),
        (Icon::Pref(P::Speech), "devices", &["audio-input-microphone"]),
        (Icon::Pref(P::Sound), "devices", &["audio-speakers", "preferences-desktop-sound", "multimedia-volume-control"]),
        (Icon::Pref(P::Wireless), "devices", &["bluetooth", "preferences-system-bluetooth", "audio-headphones"]),
        // Tipos de arquivo
        (Icon::Document, "mimetypes", &["text-x-generic", "text-plain", "x-office-document"]),
        (Icon::File, "mimetypes", &["application-octet-stream", "unknown", "empty"]),
        (Icon::Picture, "mimetypes", &["image-x-generic"]),
        (Icon::AudioFile, "mimetypes", &["audio-x-generic"]),
        (Icon::VideoFile, "mimetypes", &["video-x-generic"]),
        (Icon::ArchiveFile, "mimetypes", &["package-x-generic", "application-zip", "application-x-compressed-tar", "application-x-tar"]),
        (Icon::PdfFile, "mimetypes", &["application-pdf"]),
        (Icon::CodeFile, "mimetypes", &["text-x-script", "text-x-csrc", "text-x-python", "text-x-rust", "application-x-shellscript"]),
        // Preferências
        (Icon::Pref(P::ShowAll), "categories", &["preferences-other"]),
        (Icon::Pref(P::Appearance), "categories", &["preferences-desktop-theme", "preferences-desktop-appearance"]),
        (Icon::Pref(P::Dock), "categories", &["preferences-desktop-dock"]),
        (Icon::Pref(P::Shortcuts), "categories", &["preferences-desktop-keyboard-shortcuts"]),
        (Icon::Pref(P::Desktop), "categories", &["preferences-desktop-wallpaper", "preferences-desktop-screensaver"]),
        (Icon::Pref(P::Energy), "categories", &["preferences-system-power", "gnome-power-manager"]),
        (Icon::Pref(P::Network), "categories", &["preferences-system-network", "network-wired"]),
        (Icon::Pref(P::DateTime), "categories", &["preferences-system-date-time"]),
        (Icon::Pref(P::LoginItems), "categories", &["preferences-desktop-startup", "session-properties"]),
        (Icon::Pref(P::Access), "categories", &["preferences-desktop-accessibility", "accessibility"]),
        (Icon::Pref(P::Accounts), "categories", &["system-users", "preferences-system-users"]),
        (Icon::Pref(P::International), "categories", &["preferences-desktop-locale", "preferences-desktop-language"]),
        (Icon::Pref(P::Removable), "categories", &["preferences-desktop-removable-media"]),
        (Icon::Pref(P::Sharing), "categories", &["preferences-system-sharing"]),
        (Icon::Pref(P::DefaultApps), "categories", &["preferences-desktop-default-applications"]),
        (Icon::Pref(P::Update), "categories", &["system-software-update", "software-update-available"]),
        (Icon::Pref(P::StartupDisk), "categories", &["preferences-system-startup-disk"]),
        (Icon::Pref(P::Security), "categories", &["preferences-system-privacy", "system-lock-screen", "security-high"]),
        (Icon::Pref(P::Expose), "categories", &["preferences-system-windows"]),
        // Estados e ações
        (Icon::Eject, "actions", &["media-eject"]),
        (Icon::Clip, "status", &["mail-attachment"]),
    ]
}
