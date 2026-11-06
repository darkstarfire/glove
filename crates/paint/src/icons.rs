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
    TrashFull,
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
    /// Livro de capa marrom com abas e um @ (Contatos).
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
    /// Envelope com selo e carimbo (Correio).
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
        matches!(self, Icon::Folder | Icon::Files | Icon::SpecialFolder(_) | Icon::Terminal | Icon::Calculator | Icon::Clock(..) | Icon::Mail | Icon::Calendar | Icon::Contacts | Icon::Trash | Icon::TrashFull
            | Icon::Globe | Icon::TextEditor | Icon::Settings | Icon::Disk | Icon::Notes | Icon::Search | Icon::Chars | Icon::Camera | Icon::Monitor | Icon::Chess
            | Icon::Video | Icon::Logs | Icon::Info | Icon::Help | Icon::Dictionary | Icon::FontBook | Icon::Display
            | Icon::Document | Icon::File | Icon::Picture | Icon::AudioFile | Icon::VideoFile | Icon::ArchiveFile | Icon::PdfFile | Icon::CodeFile
            | Icon::Eject | Icon::Removable | Icon::Optical | Icon::Server)
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
            "trash-full" => Icon::TrashFull,
            "disk" => Icon::Disk,
            "removable" | "pendrive" => Icon::Removable,
            "optical" | "cd" | "dvd" => Icon::Optical,
            "server" => Icon::Server,
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
            Icon::TrashFull => "user-trash-full",
            Icon::Eject => "media-eject",
            Icon::Disk => "drive-harddisk",
            Icon::Removable => "drive-removable-media-usb",
            Icon::Optical => "media-optical",
            Icon::Server => "network-server",
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
        Icon::Terminal => crate::tiger::terminal(pm, x, y, s),
        Icon::Folder => folder(pm, x, y, s),
        Icon::Files => crate::tiger::files_app(pm, x, y, s),
        Icon::Globe => crate::tiger::globe(pm, x, y, s),
        Icon::TextEditor => crate::tiger::editor(pm, x, y, s),
        Icon::Calculator => crate::tiger::calculator(pm, fonts, x, y, s),
        Icon::Settings => crate::tiger::gears(pm, x, y, s),
        Icon::Window | Icon::System(_) => window(pm, fonts, x, y, s, letter),
        Icon::Generic => generic(pm, fonts, x, y, s, letter),
        Icon::Clock(h, m) => crate::tiger::clock(pm, fonts, x, y, s, h, m),
        Icon::Trash => crate::tiger::trash(pm, x, y, s, false),
        Icon::TrashFull => crate::tiger::trash(pm, x, y, s, true),
        Icon::Eject => crate::tiger::eject(pm, x, y, s),
        Icon::Removable => crate::tiger::removable(pm, x, y, s),
        Icon::Optical => crate::tiger::optical(pm, x, y, s),
        Icon::Server => crate::tiger::server(pm, x, y, s),
        Icon::Disk => crate::tiger::disk(pm, x, y, s),
        Icon::Document => crate::tiger::document(pm, x, y, s, true),
        Icon::File => crate::tiger::document(pm, x, y, s, false),
        Icon::Picture => crate::tiger::picture(pm, x, y, s),
        Icon::Notes => crate::tiger::notes(pm, x, y, s),
        Icon::Search => crate::tiger::search(pm, x, y, s),
        Icon::Chars => crate::tiger::chars(pm, fonts, x, y, s),
        Icon::Camera => crate::tiger::camera(pm, x, y, s),
        Icon::Monitor => crate::tiger::activity(pm, x, y, s),
        Icon::Chess => crate::tiger::chess(pm, x, y, s),
        Icon::Music => music(pm, x, y, s),
        Icon::Contacts => crate::tiger::contacts(pm, x, y, s),
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
        Icon::Keychain => keychain(pm, x, y, s),
        Icon::FontBook => crate::tiger::font_book(pm, fonts, x, y, s),
        Icon::Clip => clip(pm, x, y, s),
        Icon::Mail => crate::tiger::mail(pm, x, y, s),
        Icon::Display => crate::tiger::display_pref(pm, x, y, s),
        Icon::Mouse => mouse(pm, x, y, s),
        Icon::Trackpad => trackpad(pm, x, y, s),
        Icon::SpecialFolder(FolderKind::Smart) => {
            crate::tiger::folder(pm, x, y, s, rgba(150, 100, 210, 255));
            folder_emblem(pm, FolderKind::Smart, x + s * 0.5, y + s * 0.585, s * 0.34);
        }
        Icon::SpecialFolder(k) => {
            folder(pm, x, y, s);
            folder_emblem(pm, k, x + s * 0.5, y + s * 0.585, s * 0.34);
        }
        Icon::Pref(k) => pref_icon(pm, k, x, y, s),
        Icon::AudioFile => crate::tiger::typed_document(pm, fonts, x, y, s, 1),
        Icon::VideoFile => crate::tiger::typed_document(pm, fonts, x, y, s, 2),
        Icon::ArchiveFile => crate::tiger::typed_document(pm, fonts, x, y, s, 3),
        Icon::PdfFile => crate::tiger::typed_document(pm, fonts, x, y, s, 4),
        Icon::CodeFile => crate::tiger::typed_document(pm, fonts, x, y, s, 5),
    }
}

/// Desenho em relevo na frente da pasta: azul escuro com um brilho embaixo.
fn folder_emblem(pm: &mut Canvas, k: FolderKind, cx: f32, cy: f32, s: f32) {
    let dark = rgba(38, 88, 162, 205);
    let light = rgba(235, 245, 255, 190);
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

fn gloss(pm: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32) {
    pm.fill(rrect4(x, y, w, h, [r, r, 2.0, 2.0]),
        &vgrad(y, y + h, &[(0.0, rgba(255, 255, 255, 170)), (1.0, rgba(255, 255, 255, 10))]),
    );
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

fn generic(pm: &mut Canvas, fonts: &Fonts, x: f32, y: f32, s: f32, letter: Option<char>) {
    let (bx, by, bw) = (x + s * 0.12, y + s * 0.1, s * 0.76);
    gel_rect(pm, bx, by, bw, bw, s * 0.18, rgba(120, 130, 160, 255));
    if let Some(l) = letter {
        let t = l.to_uppercase().to_string();
        let px = s * 0.42;
        let tw = fonts.measure(&t, px, true);
        fonts.draw(pm, &t, bx + (bw - tw) / 2.0, by + bw * 0.68, px, true, rgba(255, 255, 255, 240));
    }
}

/// Marca do menu principal: uma gota de gel azul.
pub fn logo(pm: &mut Canvas, x: f32, y: f32, s: f32, accent: tiny_skia::Color) {
    gel_circle(pm, x + s / 2.0, y + s / 2.0, s / 2.0, accent);
}


/// Chaveiro: argola prateada com duas chaves douradas.
fn keychain(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy, r) = (x + s * 0.36, y + s * 0.3, s * 0.17);
    pm.stroke(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(240, 242, 246, 255)), (0.5, rgba(140, 144, 154, 255)), (1.0, rgba(220, 222, 228, 255))]), s * 0.05);
    let key = |pm: &mut Canvas, ang: f32, len: f32| {
        let (c, sn) = (ang.cos(), ang.sin());
        // Cabeça redonda com furo, haste e dentes.
        let (hx, hy) = (cx + c * r * 1.05, cy + sn * r * 1.05);
        let hr = s * 0.11;
        let (kx, ky) = (hx + c * hr, hy + sn * hr);
        let gold = linear(kx - hr, ky - hr, kx + hr, ky + hr, &[(0.0, rgba(255, 238, 150, 255)), (0.5, rgba(214, 160, 40, 255)), (1.0, rgba(250, 214, 100, 255))]);
        let mut pb = PathBuilder::new();
        let (ex, ey) = (kx + c * len, ky + sn * len);
        pb.move_to(kx, ky);
        pb.line_to(ex, ey);
        pm.stroke(pb.finish(), &gold, s * 0.075);
        for k in [0.62, 0.82] {
            let (tx, ty) = (kx + c * len * k, ky + sn * len * k);
            let mut pb = PathBuilder::new();
            pb.move_to(tx, ty);
            pb.line_to(tx - sn * s * 0.09, ty + c * s * 0.09);
            pm.stroke(pb.finish(), &gold, s * 0.06);
        }
        pm.fill(circle(kx, ky, hr), &gold);
        pm.stroke(circle(kx, ky, hr), &solid(rgba(150, 100, 20, 200)), (s / 64.0).max(0.8));
        pm.fill(circle(kx, ky, hr * 0.38), &solid(rgba(90, 60, 10, 255)));
    };
    key(pm, 0.75, s * 0.42);
    key(pm, 1.35, s * 0.36);
}

/// Mouse de um botão em plástico translúcido, com o fio saindo por cima.
fn mouse(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    // Fio
    let mut pb = PathBuilder::new();
    pb.move_to(x + s * 0.5, y + s * 0.2);
    pb.cubic_to(x + s * 0.5, y + s * 0.08, x + s * 0.72, y + s * 0.1, x + s * 0.78, y + s * 0.02);
    pm.stroke(pb.finish(), &solid(rgba(120, 124, 135, 255)), (s * 0.03).max(1.0));
    let (mx, my, mw, mh) = (x + s * 0.24, y + s * 0.18, s * 0.52, s * 0.74);
    // Sombra
    pm.fill(oval(mx + s * 0.02, my + s * 0.04, mw, mh), &solid(rgba(0, 0, 0, 50)));
    // Casca transparente
    pm.fill(oval(mx, my, mw, mh), &vgrad(my, my + mh, &[(0.0, rgba(250, 251, 253, 235)), (1.0, rgba(185, 190, 200, 235))]));
    // Miolo cinza visto pelo plástico
    let (ix, iy, iw, ih) = (mx + mw * 0.16, my + mh * 0.12, mw * 0.68, mh * 0.78);
    pm.fill(oval(ix, iy, iw, ih), &vgrad(iy, iy + ih, &[(0.0, rgba(120, 124, 136, 255)), (1.0, rgba(60, 62, 72, 255))]));
    // Botão (metade de cima) e sua linha
    pm.fill(rrect4(ix, iy, iw, ih * 0.36, [iw * 0.5, iw * 0.5, 0.0, 0.0]), &vgrad(iy, iy + ih * 0.36, &[(0.0, rgba(170, 174, 186, 255)), (1.0, rgba(110, 114, 126, 255))]));
    pm.fill_rect(ix, iy + ih * 0.36, iw, lw, &solid(rgba(40, 42, 50, 255)));
    pm.stroke(oval(mx, my, mw, mh), &solid(rgba(110, 114, 126, 255)), lw);
    // Brilho
    pm.fill(oval(mx + mw * 0.18, my + mh * 0.04, mw * 0.64, mh * 0.3), &vgrad(my, my + mh * 0.34, &[(0.0, rgba(255, 255, 255, 200)), (1.0, rgba(255, 255, 255, 0))]));
}

/// Superfície de toque com o botão largo embaixo.
fn trackpad(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.1, y + s * 0.1, s * 0.8, s * 0.8);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.08), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.08), &vgrad(by, by + bh, &[(0.0, rgba(225, 227, 232, 255)), (1.0, rgba(165, 168, 176, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.08), &solid(rgba(100, 104, 114, 255)), lw);
    // Área de toque
    let (px, py, pw, ph) = (bx + s * 0.08, by + s * 0.08, bw - s * 0.16, bh * 0.58);
    pm.fill(rrect(px, py, pw, ph, s * 0.04), &vgrad(py, py + ph, &[(0.0, rgba(150, 154, 164, 255)), (1.0, rgba(196, 199, 206, 255))]));
    pm.stroke(rrect(px, py, pw, ph, s * 0.04), &solid(rgba(110, 114, 124, 255)), lw);
    // Botão
    let (qy, qh) = (py + ph + s * 0.06, bh * 0.16);
    pm.fill(rrect(px, qy, pw, qh, s * 0.04), &vgrad(qy, qy + qh, &[(0.0, rgba(245, 246, 248, 255)), (1.0, rgba(175, 178, 186, 255))]));
    pm.stroke(rrect(px, qy, pw, qh, s * 0.04), &solid(rgba(110, 114, 124, 255)), lw);
    // Dedo (marca de toque)
    pm.fill(circle(px + pw * 0.62, py + ph * 0.45, s * 0.06), &solid(rgba(70, 130, 230, 150)));
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

fn pref_icon(pm: &mut Canvas, k: PrefKind, x: f32, y: f32, s: f32) {
    match k {
        PrefKind::ShowAll => pref_show_all(pm, x, y, s),
        PrefKind::Appearance => pref_appearance(pm, x, y, s),
        PrefKind::Dock => pref_dock(pm, x, y, s),
        PrefKind::Keyboard => pref_keyboard(pm, x, y, s, false),
        PrefKind::Shortcuts => pref_keyboard(pm, x, y, s, true),
        PrefKind::Desktop => pref_desktop(pm, x, y, s),
        PrefKind::Sound => pref_sound(pm, x, y, s),
        PrefKind::Energy => pref_energy(pm, x, y, s),
        PrefKind::Network => pref_network(pm, x, y, s),
        PrefKind::DateTime => pref_datetime(pm, x, y, s),
        PrefKind::LoginItems => pref_login_items(pm, x, y, s),
        PrefKind::Access => pref_access(pm, x, y, s),
        PrefKind::Accounts => pref_accounts(pm, x, y, s),
        PrefKind::International => pref_international(pm, x, y, s),
        PrefKind::Removable => pref_removable(pm, x, y, s),
        PrefKind::Wireless => pref_wireless(pm, x, y, s),
        PrefKind::Sharing => pref_sharing(pm, x, y, s),
        PrefKind::DefaultApps => pref_default_apps(pm, x, y, s),
        PrefKind::Printer => pref_printer(pm, x, y, s),
        PrefKind::Update => pref_update(pm, x, y, s),
        PrefKind::StartupDisk => pref_startup_disk(pm, x, y, s),
        PrefKind::Speech => pref_speech(pm, x, y, s),
        PrefKind::Security => pref_security(pm, x, y, s),
        PrefKind::Expose => pref_expose(pm, x, y, s),
    }
}

/// Cadeado prateado com o arco por cima (Segurança).
fn pref_security(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (cx, top) = (x + s * 0.5, y + s * 0.12);
    let r = s * 0.2;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, y + s * 0.46);
    pb.line_to(cx - r, top + r);
    pb.quad_to(cx - r, top, cx, top);
    pb.quad_to(cx + r, top, cx + r, top + r);
    pb.line_to(cx + r, y + s * 0.46);
    pm.stroke(pb.finish(), &linear(cx - r, 0.0, cx + r, 0.0, &[(0.0, rgba(140, 144, 152, 255)), (0.5, rgba(236, 238, 242, 255)), (1.0, rgba(120, 124, 132, 255))]), s * 0.09);
    let (bx, by, bw, bh) = (x + s * 0.2, y + s * 0.42, s * 0.6, s * 0.46);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.06), &solid(rgba(0, 0, 0, 60)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.06), &vgrad(by, by + bh, &[(0.0, rgba(250, 214, 110, 255)), (1.0, rgba(196, 140, 40, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.06), &solid(rgba(130, 90, 20, 255)), lw);
    pm.fill(circle(cx, by + bh * 0.42, s * 0.06), &solid(rgba(70, 46, 10, 255)));
    pm.fill_rect(cx - s * 0.025, by + bh * 0.42, s * 0.05, bh * 0.32, &solid(rgba(70, 46, 10, 255)));
}

/// Quatro janelinhas espalhadas sobre a mesa (Exposé e Painel).
fn pref_expose(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    pm.fill(rrect(x + s * 0.06, y + s * 0.14, s * 0.88, s * 0.72, s * 0.05), &vgrad(y + s * 0.14, y + s * 0.86, &[(0.0, rgba(90, 140, 220, 255)), (1.0, rgba(30, 70, 150, 255))]));
    pm.stroke(rrect(x + s * 0.06, y + s * 0.14, s * 0.88, s * 0.72, s * 0.05), &solid(rgba(20, 40, 90, 255)), lw);
    for (k, (wx, wy)) in [(0.12, 0.2), (0.52, 0.2), (0.12, 0.52), (0.52, 0.52)].iter().enumerate() {
        let (px, py, pw, ph) = (x + s * wx, y + s * wy, s * 0.36, s * 0.28);
        pm.fill(rrect(px + s * 0.01, py + s * 0.02, pw, ph, s * 0.02), &solid(rgba(0, 0, 0, 70)));
        pm.fill(rrect(px, py, pw, ph, s * 0.02), &solid(rgba(250, 250, 252, 255)));
        pm.fill_rect(px, py, pw, ph * 0.22, &vgrad(py, py + ph * 0.22, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(200, 200, 206, 255))]));
        let dot = [rgba(240, 80, 70, 255), rgba(250, 190, 50, 255), rgba(100, 200, 80, 255), rgba(120, 160, 230, 255)][k];
        pm.fill(circle(px + pw * 0.12, py + ph * 0.11, s * 0.018), &solid(dot));
    }
}

/// Doze quadradinhos coloridos, como uma janela cheia de painéis.
fn pref_show_all(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.14, s * 0.84, s * 0.72);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.06), &solid(rgba(0, 0, 0, 45)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.06), &vgrad(by, by + bh, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(200, 203, 210, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.06), &solid(rgba(110, 114, 124, 255)), lw);
    let colors = [
        rgba(70, 130, 230, 255), rgba(120, 190, 80, 255), rgba(240, 170, 50, 255), rgba(150, 100, 210, 255),
        rgba(220, 80, 70, 255), rgba(90, 170, 210, 255), rgba(140, 145, 155, 255), rgba(60, 170, 140, 255),
        rgba(230, 120, 170, 255), rgba(100, 110, 200, 255), rgba(200, 160, 90, 255), rgba(80, 140, 90, 255),
    ];
    let (cw, ch) = (bw * 0.17, bh * 0.22);
    for (i, c) in colors.iter().enumerate() {
        let (col, row) = ((i % 4) as f32, (i / 4) as f32);
        let cx = bx + bw * 0.08 + col * (cw + bw * 0.06);
        let cy = by + bh * 0.1 + row * (ch + bh * 0.08);
        gel_rect(pm, cx, cy, cw, ch, s * 0.03, *c);
    }
}

/// Paleta de pintor com as cores de destaque (azul e grafite) e um pincel.
fn pref_appearance(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let mut pb = PathBuilder::new();
    let (cx, cy) = (x + s * 0.48, y + s * 0.52);
    pb.move_to(cx + s * 0.38, cy - s * 0.05);
    pb.cubic_to(cx + s * 0.42, cy - s * 0.4, cx - s * 0.3, cy - s * 0.46, cx - s * 0.42, cy - s * 0.06);
    pb.cubic_to(cx - s * 0.5, cy + s * 0.3, cx - s * 0.05, cy + s * 0.46, cx + s * 0.16, cy + s * 0.34);
    pb.cubic_to(cx + s * 0.3, cy + s * 0.26, cx + s * 0.06, cy + s * 0.12, cx + s * 0.2, cy + s * 0.06);
    pb.cubic_to(cx + s * 0.3, cy + s * 0.02, cx + s * 0.36, cy + s * 0.06, cx + s * 0.38, cy - s * 0.05);
    pb.close();
    let path = pb.finish();
    pm.fill(path.clone().map(|p| p.transform(tiny_skia::Transform::from_translate(s * 0.015, s * 0.03))).flatten(), &solid(rgba(0, 0, 0, 45)));
    pm.fill(path.clone(), &vgrad(y + s * 0.1, y + s * 0.9, &[(0.0, rgba(236, 214, 170, 255)), (1.0, rgba(196, 160, 104, 255))]));
    pm.stroke(path, &solid(rgba(130, 98, 56, 255)), lw);
    // Furo do polegar
    pm.fill(oval(x + s * 0.24, y + s * 0.56, s * 0.12, s * 0.09), &solid(rgba(255, 255, 255, 0)));
    pm.fill(oval(x + s * 0.24, y + s * 0.56, s * 0.12, s * 0.09), &vgrad(y + s * 0.56, y + s * 0.65, &[(0.0, rgba(150, 118, 70, 255)), (1.0, rgba(220, 196, 150, 255))]));
    // Tintas
    for (dx, dy, c) in [(0.3, 0.3, rgba(60, 120, 230, 255)), (0.5, 0.24, rgba(130, 138, 150, 255)), (0.68, 0.34, rgba(230, 70, 60, 255)), (0.44, 0.44, rgba(250, 200, 50, 255))] {
        gel_circle(pm, x + s * dx, y + s * dy, s * 0.075, c);
    }
}

/// Mesa azul com a prateleira de vidro e quatro ícones.
fn pref_dock(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.06, y + s * 0.12, s * 0.88, s * 0.72);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.05), &solid(rgba(0, 0, 0, 45)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.05), &vgrad(by, by + bh, &[(0.0, rgba(60, 120, 210, 255)), (1.0, rgba(25, 60, 150, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.05), &solid(rgba(30, 50, 100, 255)), lw);
    // Vidro em perspectiva
    let (gy, gh) = (by + bh * 0.62, bh * 0.24);
    let mut pb = PathBuilder::new();
    pb.move_to(bx + bw * 0.14, gy);
    pb.line_to(bx + bw * 0.86, gy);
    pb.line_to(bx + bw * 0.95, gy + gh);
    pb.line_to(bx + bw * 0.05, gy + gh);
    pb.close();
    pm.fill(pb.finish(), &vgrad(gy, gy + gh, &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(220, 230, 245, 200))]));
    let colors = [rgba(70, 140, 235, 255), rgba(120, 190, 80, 255), rgba(240, 170, 50, 255), rgba(150, 155, 165, 255)];
    let iw = bw * 0.15;
    for (i, c) in colors.iter().enumerate() {
        let ix = bx + bw * 0.16 + i as f32 * (iw + bw * 0.04);
        gel_rect(pm, ix, gy - iw * 0.55, iw, iw, s * 0.03, *c);
    }
}

/// Teclado visto de cima; `hot` acende uma tecla (atalhos).
fn pref_keyboard(pm: &mut Canvas, x: f32, y: f32, s: f32, hot: bool) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.04, y + s * 0.24, s * 0.92, s * 0.52);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.06), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.06), &vgrad(by, by + bh, &[(0.0, rgba(235, 237, 242, 255)), (1.0, rgba(175, 179, 188, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.06), &solid(rgba(100, 104, 114, 255)), lw);
    let (kx, ky) = (bx + bw * 0.05, by + bh * 0.1);
    let (cols, rows) = (8, 3);
    let kw = (bw * 0.9 - (cols - 1) as f32 * s * 0.012) / cols as f32;
    let kh = (bh * 0.62 - (rows - 1) as f32 * s * 0.012) / rows as f32;
    for r in 0..rows {
        for c in 0..cols {
            let px = kx + c as f32 * (kw + s * 0.012);
            let py = ky + r as f32 * (kh + s * 0.012);
            let lit = hot && r == 1 && c == 1;
            let (top, bot) = if lit { (rgba(120, 180, 250, 255), rgba(40, 100, 210, 255)) } else { (rgba(255, 255, 255, 255), rgba(214, 216, 222, 255)) };
            pm.fill(rrect(px, py, kw, kh, s * 0.015), &vgrad(py, py + kh, &[(0.0, top), (1.0, bot)]));
        }
    }
    // Barra de espaço
    let sy = ky + rows as f32 * (kh + s * 0.012);
    pm.fill(rrect(bx + bw * 0.25, sy, bw * 0.5, kh * 0.9, s * 0.015), &vgrad(sy, sy + kh, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(214, 216, 222, 255))]));
    if hot {
        // Raio sobre o canto: "faz na hora".
        let (lx, ly, l) = (x + s * 0.66, y + s * 0.02, s * 0.3);
        let mut pb = PathBuilder::new();
        pb.move_to(lx + l * 0.55, ly);
        pb.line_to(lx + l * 0.1, ly + l * 0.58);
        pb.line_to(lx + l * 0.45, ly + l * 0.58);
        pb.line_to(lx + l * 0.3, ly + l);
        pb.line_to(lx + l * 0.9, ly + l * 0.38);
        pb.line_to(lx + l * 0.55, ly + l * 0.38);
        pb.close();
        let p = pb.finish();
        pm.fill(p.clone(), &vgrad(ly, ly + l, &[(0.0, rgba(255, 230, 90, 255)), (1.0, rgba(240, 160, 30, 255))]));
        pm.stroke(p, &solid(rgba(160, 100, 20, 255)), lw);
    }
}

/// Moldura de madeira clara com a paisagem de ondas azuis.
fn pref_desktop(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.06, y + s * 0.14, s * 0.88, s * 0.7);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.03), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.03), &vgrad(by, by + bh, &[(0.0, rgba(236, 210, 160, 255)), (1.0, rgba(180, 140, 90, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.03), &solid(rgba(120, 88, 50, 255)), lw);
    let m = s * 0.07;
    let (sx, sy, sw, sh) = (bx + m, by + m, bw - 2.0 * m, bh - 2.0 * m);
    pm.fill_rect(sx, sy, sw, sh, &vgrad(sy, sy + sh, &[(0.0, rgba(30, 80, 180, 255)), (1.0, rgba(90, 160, 235, 255))]));
    for (k, a) in [(0.45, 120u8), (0.7, 80)] {
        let mut pb = PathBuilder::new();
        pb.move_to(sx, sy + sh * k);
        pb.quad_to(sx + sw * 0.4, sy + sh * (k - 0.3), sx + sw, sy + sh * (k - 0.05));
        pb.line_to(sx + sw, sy + sh * (k + 0.08));
        pb.quad_to(sx + sw * 0.45, sy + sh * (k - 0.15), sx, sy + sh * (k + 0.15));
        pb.close();
        pm.fill(pb.finish(), &solid(rgba(255, 255, 255, a)));
    }
    pm.stroke(rrect(sx, sy, sw, sh, 0.0), &solid(rgba(120, 88, 50, 200)), lw);
}

/// Alto-falante prateado com o cone escuro e as ondas saindo.
fn pref_sound(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.14, y + s * 0.08, s * 0.5, s * 0.84);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.08), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.08), &linear(bx, 0.0, bx + bw, 0.0, &[(0.0, rgba(170, 174, 184, 255)), (0.45, rgba(240, 242, 246, 255)), (1.0, rgba(150, 154, 164, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.08), &solid(rgba(100, 104, 114, 255)), lw);
    // Agudo e grave.
    let cx = bx + bw / 2.0;
    for (cy, r) in [(by + bh * 0.24, s * 0.08), (by + bh * 0.64, s * 0.17)] {
        pm.fill(circle(cx, cy, r + s * 0.02), &solid(rgba(200, 202, 208, 255)));
        pm.fill(circle(cx, cy, r), &radial(cx - r * 0.3, cy - r * 0.3, r * 1.4, &[(0.0, rgba(110, 114, 124, 255)), (1.0, rgba(30, 32, 38, 255))]));
        pm.fill(circle(cx, cy, r * 0.28), &solid(rgba(150, 154, 164, 255)));
    }
    // Ondas.
    for (k, a) in [(0.0, 220u8), (1.0, 150)] {
        let r = s * (0.16 + k * 0.1);
        let ox = bx + bw + s * 0.02;
        let oy = by + bh * 0.5;
        let mut pb = PathBuilder::new();
        pb.move_to(ox + r * 0.35, oy - r);
        pb.quad_to(ox + r * 1.15, oy, ox + r * 0.35, oy + r);
        pm.stroke(pb.finish(), &solid(rgba(60, 120, 220, a)), (s * 0.045).max(1.0));
    }
}

/// Lâmpada de vidro acesa, com a rosca prateada.
fn pref_energy(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.38, s * 0.3);
    // Brilho em volta
    pm.fill(circle(cx, cy, r * 1.5), &radial(cx, cy, r * 1.5, &[(0.0, rgba(255, 240, 160, 150)), (1.0, rgba(255, 240, 160, 0))]));
    // Bulbo (círculo que afina até a rosca)
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r * 0.45, cy + r * 1.05);
    pb.cubic_to(cx - r * 0.5, cy + r * 0.7, cx - r, cy + r * 0.5, cx - r, cy);
    pb.cubic_to(cx - r, cy - r * 0.6, cx - r * 0.55, cy - r, cx, cy - r);
    pb.cubic_to(cx + r * 0.55, cy - r, cx + r, cy - r * 0.6, cx + r, cy);
    pb.cubic_to(cx + r, cy + r * 0.5, cx + r * 0.5, cy + r * 0.7, cx + r * 0.45, cy + r * 1.05);
    pb.close();
    let bulb = pb.finish();
    pm.fill(bulb.clone(), &radial(cx - r * 0.3, cy - r * 0.4, r * 1.6, &[(0.0, rgba(255, 255, 235, 255)), (0.6, rgba(255, 236, 140, 255)), (1.0, rgba(240, 190, 60, 255))]));
    pm.stroke(bulb, &solid(rgba(190, 140, 40, 255)), lw);
    // Filamento
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r * 0.3, cy + r * 0.2);
    pb.line_to(cx - r * 0.15, cy - r * 0.15);
    pb.line_to(cx, cy + r * 0.15);
    pb.line_to(cx + r * 0.15, cy - r * 0.15);
    pb.line_to(cx + r * 0.3, cy + r * 0.2);
    pm.stroke(pb.finish(), &solid(rgba(200, 110, 20, 255)), (s * 0.025).max(1.0));
    // Rosca
    let (bx, by, bw) = (cx - r * 0.45, cy + r * 1.05, r * 0.9);
    for k in 0..3 {
        let yy = by + k as f32 * s * 0.055;
        pm.fill(rrect(bx, yy, bw, s * 0.05, s * 0.02), &vgrad(yy, yy + s * 0.05, &[(0.0, rgba(235, 236, 240, 255)), (1.0, rgba(140, 144, 152, 255))]));
    }
    pm.fill(oval(cx - r * 0.2, by + s * 0.16, r * 0.4, s * 0.06), &solid(rgba(70, 72, 80, 255)));
}

/// Esfera de vidro azul com meridianos e as ondas do Wi-Fi saindo do alto.
fn pref_network(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.58, s * 0.32);
    pm.fill(oval(cx - r, cy + r * 0.8, r * 2.0, r * 0.5), &solid(rgba(0, 0, 0, 40)));
    pm.fill(circle(cx, cy, r), &radial(cx - r * 0.35, cy - r * 0.4, r * 1.7, &[(0.0, rgba(150, 205, 255, 255)), (0.6, rgba(40, 110, 220, 255)), (1.0, rgba(15, 50, 140, 255))]));
    let line = solid(rgba(255, 255, 255, 110));
    pm.stroke(oval(cx - r * 0.45, cy - r, r * 0.9, r * 2.0), &line, lw);
    pm.stroke(oval(cx - r, cy - r * 0.35, r * 2.0, r * 0.7), &line, lw);
    pm.stroke(circle(cx, cy, r), &solid(rgba(10, 40, 110, 255)), lw);
    gloss(pm, cx - r * 0.7, cy - r * 0.95, r * 1.4, r * 0.8, r * 0.4);
    // Ondas do Wi-Fi saindo do alto da esfera.
    for (k, a) in [(1.0, 255u8), (1.6, 200), (2.2, 150)] {
        let rr = s * 0.09 * k;
        let (ox, oy) = (cx, cy - r - s * 0.02);
        let mut pb = PathBuilder::new();
        pb.move_to(ox - rr * 0.8, oy - rr * 0.35);
        pb.quad_to(ox, oy - rr * 1.25, ox + rr * 0.8, oy - rr * 0.35);
        pm.stroke(pb.finish(), &solid(rgba(255, 150, 30, a)), (s * 0.045).max(1.2));
    }
}

/// Folha de calendário (faixa vermelha no alto) com um relógio redondo na frente.
fn pref_datetime(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.08, s * 0.62, s * 0.66);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.05), &solid(rgba(0, 0, 0, 45)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.05), &vgrad(by, by + bh, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(228, 228, 232, 255))]));
    pm.fill(rrect4(bx, by, bw, bh * 0.24, [s * 0.05, s * 0.05, 0.0, 0.0]), &vgrad(by, by + bh * 0.24, &[(0.0, rgba(240, 90, 80, 255)), (1.0, rgba(190, 40, 35, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.05), &solid(rgba(110, 110, 118, 255)), lw);
    for r in 0..3 {
        for c in 0..4 {
            pm.fill_rect(bx + bw * (0.1 + c as f32 * 0.21), by + bh * (0.36 + r as f32 * 0.18), bw * 0.13, bh * 0.1, &solid(rgba(150, 150, 158, 255)));
        }
    }
    // Relógio
    let (cx, cy, r) = (x + s * 0.66, y + s * 0.66, s * 0.27);
    pm.fill(circle(cx + s * 0.01, cy + s * 0.025, r), &solid(rgba(0, 0, 0, 50)));
    pm.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(235, 237, 242, 255)), (1.0, rgba(140, 144, 154, 255))]));
    pm.fill(circle(cx, cy, r * 0.82), &radial(cx - r * 0.2, cy - r * 0.3, r * 1.2, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(225, 228, 235, 255))]));
    let hand = |pm: &mut Canvas, a: f32, len: f32, w: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(cx, cy);
        pb.line_to(cx + len * a.sin(), cy - len * a.cos());
        pm.stroke_round(pb.finish(), &solid(rgba(40, 40, 46, 255)), w);
    };
    hand(pm, std::f32::consts::TAU * 10.0 / 12.0, r * 0.45, (s * 0.04).max(1.2));
    hand(pm, std::f32::consts::TAU * 2.0 / 12.0, r * 0.68, (s * 0.03).max(1.0));
    pm.fill(circle(cx, cy, s * 0.025), &solid(rgba(200, 50, 40, 255)));
}

/// Folha com uma lista de itens e o botão verde de "abrir" na frente.
fn pref_login_items(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.1, y + s * 0.06, s * 0.62, s * 0.78);
    pm.fill(rrect(bx + s * 0.015, by + s * 0.03, bw, bh, s * 0.04), &solid(rgba(0, 0, 0, 45)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.04), &vgrad(by, by + bh, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(226, 228, 234, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.04), &solid(rgba(110, 112, 120, 255)), lw);
    let colors = [rgba(70, 130, 230, 255), rgba(240, 170, 50, 255), rgba(120, 190, 80, 255), rgba(150, 100, 210, 255)];
    for (k, c) in colors.iter().enumerate() {
        let yy = by + bh * (0.1 + k as f32 * 0.2);
        gel_rect(pm, bx + bw * 0.1, yy, bw * 0.16, bw * 0.16, s * 0.02, *c);
        pm.fill_rect(bx + bw * 0.34, yy + bw * 0.05, bw * 0.52, bw * 0.06, &solid(rgba(150, 152, 160, 255)));
    }
    // Botão verde redondo com a seta.
    let (cx, cy, r) = (x + s * 0.7, y + s * 0.72, s * 0.2);
    gel_circle(pm, cx, cy, r, rgba(70, 180, 60, 255));
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r * 0.3, cy - r * 0.45);
    pb.line_to(cx + r * 0.5, cy);
    pb.line_to(cx - r * 0.3, cy + r * 0.45);
    pb.close();
    pm.fill(pb.finish(), &solid(rgba(255, 255, 255, 240)));
}

/// Círculo azul em gel com uma figura humana de braços abertos.
fn pref_access(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.5, s * 0.42);
    pm.fill(circle(cx + s * 0.01, cy + s * 0.03, r), &solid(rgba(0, 0, 0, 50)));
    gel_circle(pm, cx, cy, r, rgba(40, 110, 220, 255));
    pm.stroke(circle(cx, cy, r * 0.86), &solid(rgba(255, 255, 255, 230)), (s * 0.03).max(1.0));
    let white = solid(rgba(255, 255, 255, 255));
    pm.fill(circle(cx, cy - r * 0.48, r * 0.12), &white);
    let w = (s * 0.06).max(1.4);
    let line = |pm: &mut Canvas, a: (f32, f32), b: (f32, f32)| {
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        pm.stroke_round(pb.finish(), &white, w);
    };
    // Braços abertos, corpo e pernas.
    line(pm, (cx - r * 0.5, cy - r * 0.22), (cx + r * 0.5, cy - r * 0.22));
    line(pm, (cx, cy - r * 0.22), (cx, cy + r * 0.15));
    line(pm, (cx, cy + r * 0.15), (cx - r * 0.25, cy + r * 0.55));
    line(pm, (cx, cy + r * 0.15), (cx + r * 0.25, cy + r * 0.55));
}

/// Duas silhuetas, uma na frente da outra (contas).
fn pref_accounts(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let person = |pm: &mut Canvas, cx: f32, top: f32, k: f32, body: Color| {
        let r = s * 0.13 * k;
        pm.fill(circle(cx, top + r, r), &radial(cx - r * 0.3, top + r * 0.6, r * 1.6, &[(0.0, rgba(255, 226, 196, 255)), (1.0, rgba(214, 160, 120, 255))]));
        let (bw, bh) = (s * 0.42 * k, s * 0.34 * k);
        let by = top + r * 2.1;
        pm.fill(rrect4(cx - bw / 2.0, by, bw, bh, [bw * 0.45, bw * 0.45, s * 0.03, s * 0.03]), &vgrad(by, by + bh, &[(0.0, lighten(body, 0.25)), (1.0, body)]));
    };
    person(pm, x + s * 0.64, y + s * 0.1, 0.85, rgba(110, 120, 140, 255));
    person(pm, x + s * 0.4, y + s * 0.22, 1.0, rgba(40, 110, 220, 255));
}

/// Balão de fala azul com três sinais de escrita desenhados (a, ñ, ö estilizados).
fn pref_international(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.06, y + s * 0.12, s * 0.88, s * 0.6);
    let mut pb = PathBuilder::new();
    let r = s * 0.12;
    pb.move_to(bx + r, by);
    pb.line_to(bx + bw - r, by);
    pb.quad_to(bx + bw, by, bx + bw, by + r);
    pb.line_to(bx + bw, by + bh - r);
    pb.quad_to(bx + bw, by + bh, bx + bw - r, by + bh);
    pb.line_to(bx + bw * 0.42, by + bh);
    pb.line_to(bx + bw * 0.22, by + bh + s * 0.16);
    pb.line_to(bx + bw * 0.26, by + bh);
    pb.line_to(bx + r, by + bh);
    pb.quad_to(bx, by + bh, bx, by + bh - r);
    pb.line_to(bx, by + r);
    pb.quad_to(bx, by, bx + r, by);
    pb.close();
    let p = pb.finish();
    pm.fill(p.clone(), &vgrad(by, by + bh, &[(0.0, rgba(120, 180, 250, 255)), (1.0, rgba(30, 90, 200, 255))]));
    pm.stroke(p, &solid(rgba(20, 60, 140, 255)), lw);
    gloss(pm, bx + s * 0.04, by + s * 0.02, bw - s * 0.08, bh * 0.45, r);
    // Três "letras" brancas: um círculo com haste, um til sobre um arco e dois pontos sobre um anel.
    let white = solid(rgba(255, 255, 255, 240));
    let w = (s * 0.045).max(1.2);
    let cy = by + bh * 0.58;
    pm.stroke(circle(bx + bw * 0.22, cy, s * 0.07), &white, w);
    let mut pb = PathBuilder::new();
    pb.move_to(bx + bw * 0.22 + s * 0.07, cy - s * 0.08);
    pb.line_to(bx + bw * 0.22 + s * 0.07, cy + s * 0.07);
    pm.stroke(pb.finish(), &white, w);
    let mut pb = PathBuilder::new();
    pb.move_to(bx + bw * 0.42, cy + s * 0.07);
    pb.line_to(bx + bw * 0.42, cy - s * 0.02);
    pb.quad_to(bx + bw * 0.5, cy - s * 0.09, bx + bw * 0.58, cy - s * 0.02);
    pb.line_to(bx + bw * 0.58, cy + s * 0.07);
    pb.move_to(bx + bw * 0.42, cy - s * 0.15);
    pb.quad_to(bx + bw * 0.46, cy - s * 0.2, bx + bw * 0.5, cy - s * 0.15);
    pb.quad_to(bx + bw * 0.54, cy - s * 0.1, bx + bw * 0.58, cy - s * 0.15);
    pm.stroke(pb.finish(), &white, w);
    pm.stroke(circle(bx + bw * 0.78, cy + s * 0.01, s * 0.065), &white, w);
    pm.fill(circle(bx + bw * 0.74, cy - s * 0.12, s * 0.022), &white);
    pm.fill(circle(bx + bw * 0.82, cy - s * 0.12, s * 0.022), &white);
}

/// Disco prateado atrás e um pendrive azul na frente.
fn pref_removable(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (cx, cy, r) = (x + s * 0.42, y + s * 0.44, s * 0.36);
    pm.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(250, 250, 252, 255)), (0.5, rgba(190, 200, 215, 255)), (1.0, rgba(240, 236, 250, 255))]));
    pm.stroke(circle(cx, cy, r), &solid(rgba(130, 134, 144, 255)), lw);
    pm.fill(circle(cx, cy, r * 0.22), &solid(rgba(210, 214, 222, 255)));
    pm.fill(circle(cx, cy, r * 0.1), &solid(rgba(255, 255, 255, 255)));
    // Pendrive
    let (bx, by, bw, bh) = (x + s * 0.5, y + s * 0.42, s * 0.24, s * 0.46);
    pm.fill_rect(bx + bw * 0.2, by - s * 0.1, bw * 0.6, s * 0.12, &vgrad(by - s * 0.1, by, &[(0.0, rgba(235, 236, 240, 255)), (1.0, rgba(160, 164, 172, 255))]));
    pm.fill(rrect(bx + s * 0.01, by + s * 0.02, bw, bh, s * 0.05), &solid(rgba(0, 0, 0, 50)));
    gel_rect(pm, bx, by, bw, bh, s * 0.05, rgba(40, 110, 220, 255));
}

/// Fone de ouvido sem fio com as ondas do sinal.
fn pref_wireless(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let w = (s * 0.07).max(1.5);
    let (cx, cy, r) = (x + s * 0.44, y + s * 0.5, s * 0.3);
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, cy + r * 0.2);
    pb.cubic_to(cx - r, cy - r * 1.3, cx + r, cy - r * 1.3, cx + r, cy + r * 0.2);
    pm.stroke_round(pb.finish(), &solid(rgba(90, 94, 104, 255)), w);
    for side in [-1.0f32, 1.0] {
        let ex = cx + side * r - s * 0.08;
        gel_rect(pm, ex, cy, s * 0.16, s * 0.26, s * 0.07, rgba(60, 64, 74, 255));
    }
    for (k, a) in [(1.0, 230u8), (1.7, 170)] {
        let rr = s * 0.08 * k;
        let (ox, oy) = (x + s * 0.8, y + s * 0.22);
        let mut pb = PathBuilder::new();
        pb.move_to(ox - rr * 0.3, oy - rr);
        pb.quad_to(ox + rr * 0.6, oy, ox - rr * 0.3, oy + rr);
        pm.stroke(pb.finish(), &solid(rgba(40, 110, 220, a)), (s * 0.04).max(1.0));
    }
}

/// Pasta azul com uma plaquinha amarela de "compartilhada".
fn pref_sharing(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    folder(pm, x, y + s * 0.04, s * 0.92);
    let (bx, by, bw, bh) = (x + s * 0.5, y + s * 0.56, s * 0.44, s * 0.3);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.02, bw, bh, s * 0.05), &solid(rgba(0, 0, 0, 50)));
    gel_rect(pm, bx, by, bw, bh, s * 0.05, rgba(240, 180, 40, 255));
    // Duas cabecinhas na placa.
    for k in 0..2 {
        let cx = bx + bw * (0.33 + k as f32 * 0.34);
        pm.fill(circle(cx, by + bh * 0.35, s * 0.045), &solid(rgba(255, 255, 255, 240)));
        pm.fill(rrect4(cx - s * 0.06, by + bh * 0.55, s * 0.12, s * 0.07, [s * 0.05, s * 0.05, 0.0, 0.0]), &solid(rgba(255, 255, 255, 240)));
    }
}

/// Três aplicativos em leque com o selo verde de "escolhido".
fn pref_default_apps(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let colors = [rgba(150, 100, 210, 255), rgba(240, 170, 50, 255), rgba(40, 110, 220, 255)];
    for (k, c) in colors.iter().enumerate() {
        let o = k as f32 * s * 0.12;
        let (bx, by) = (x + s * 0.08 + o, y + s * 0.1 + o);
        pm.fill(rrect(bx + s * 0.01, by + s * 0.02, s * 0.5, s * 0.5, s * 0.08), &solid(rgba(0, 0, 0, 45)));
        gel_rect(pm, bx, by, s * 0.5, s * 0.5, s * 0.08, *c);
    }
    let (cx, cy, r) = (x + s * 0.74, y + s * 0.74, s * 0.18);
    gel_circle(pm, cx, cy, r, rgba(70, 180, 60, 255));
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r * 0.45, cy);
    pb.line_to(cx - r * 0.1, cy + r * 0.38);
    pb.line_to(cx + r * 0.5, cy - r * 0.38);
    pm.stroke_round(pb.finish(), &solid(rgba(255, 255, 255, 255)), (s * 0.05).max(1.2));
}

/// Impressora clara com a folha saindo por cima e a bandeja embaixo.
fn pref_printer(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    pm.fill_rect(x + s * 0.28, y + s * 0.08, s * 0.44, s * 0.36, &solid(rgba(255, 255, 255, 255)));
    pm.stroke(rrect(x + s * 0.28, y + s * 0.08, s * 0.44, s * 0.36, 0.0), &solid(rgba(150, 150, 156, 255)), lw);
    for k in 0..3 {
        pm.fill_rect(x + s * 0.33, y + s * (0.16 + k as f32 * 0.07), s * 0.34, s * 0.025, &solid(rgba(170, 170, 178, 255)));
    }
    let (bx, by, bw, bh) = (x + s * 0.08, y + s * 0.36, s * 0.84, s * 0.36);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.06), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.06), &vgrad(by, by + bh, &[(0.0, rgba(236, 238, 242, 255)), (1.0, rgba(168, 172, 182, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.06), &solid(rgba(100, 104, 114, 255)), lw);
    pm.fill_rect(bx + bw * 0.15, by + bh * 0.55, bw * 0.7, bh * 0.14, &solid(rgba(60, 62, 70, 255)));
    pm.fill(circle(bx + bw * 0.88, by + bh * 0.25, s * 0.03), &solid(rgba(80, 200, 80, 255)));
    pm.fill_rect(x + s * 0.2, y + s * 0.72, s * 0.6, s * 0.12, &vgrad(y + s * 0.72, y + s * 0.84, &[(0.0, rgba(200, 202, 208, 255)), (1.0, rgba(150, 152, 160, 255))]));
}

/// Caixa (pacote) com duas setas verdes em círculo.
fn pref_update(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (bx, by, bw, bh) = (x + s * 0.12, y + s * 0.3, s * 0.76, s * 0.56);
    pm.fill(rrect(bx + s * 0.01, by + s * 0.03, bw, bh, s * 0.04), &solid(rgba(0, 0, 0, 50)));
    pm.fill(rrect(bx, by, bw, bh, s * 0.04), &vgrad(by, by + bh, &[(0.0, rgba(222, 186, 130, 255)), (1.0, rgba(176, 132, 80, 255))]));
    pm.stroke(rrect(bx, by, bw, bh, s * 0.04), &solid(rgba(120, 86, 46, 255)), lw);
    pm.fill_rect(bx + bw * 0.42, by, bw * 0.16, bh, &solid(rgba(240, 220, 170, 200)));
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.32, s * 0.2);
    pm.fill(circle(cx, cy, r + s * 0.04), &solid(rgba(255, 255, 255, 230)));
    for k in 0..2 {
        let a0 = k as f32 * std::f32::consts::PI;
        let mut pb = PathBuilder::new();
        for i in 0..=10 {
            let a = a0 + 0.3 + i as f32 * 0.25;
            let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        pm.stroke_round(pb.finish(), &solid(rgba(60, 170, 60, 255)), (s * 0.06).max(1.4));
        let a = a0 + 0.3 + 2.5;
        let (tx, ty) = (cx + r * a.cos(), cy + r * a.sin());
        let mut pb = PathBuilder::new();
        pb.move_to(tx + s * 0.07 * (a + 1.2).cos(), ty + s * 0.07 * (a + 1.2).sin());
        pb.line_to(tx + s * 0.06 * (a + 2.4).cos(), ty + s * 0.06 * (a + 2.4).sin());
        pb.line_to(tx - s * 0.05 * (a + 0.6).cos(), ty - s * 0.05 * (a + 0.6).sin());
        pb.close();
        pm.fill(pb.finish(), &solid(rgba(60, 170, 60, 255)));
    }
}

/// Disco rígido prateado com o símbolo de ligar.
fn pref_startup_disk(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    crate::tiger::disk(pm, x, y, s);
    let (cx, cy, r) = (x + s * 0.72, y + s * 0.3, s * 0.17);
    gel_circle(pm, cx, cy, r, rgba(40, 110, 220, 255));
    let w = (s * 0.04).max(1.2);
    let mut pb = PathBuilder::new();
    for i in 0..=12 {
        let a = -std::f32::consts::FRAC_PI_2 + 0.6 + i as f32 * (std::f32::consts::TAU - 1.2) / 12.0;
        let (px, py) = (cx + r * 0.55 * a.cos(), cy + r * 0.55 * a.sin());
        if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
    }
    pm.stroke_round(pb.finish(), &solid(rgba(255, 255, 255, 255)), w);
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - r * 0.7);
    pb.line_to(cx, cy - r * 0.1);
    pm.stroke_round(pb.finish(), &solid(rgba(255, 255, 255, 255)), w);
}

/// Perfil de um rosto com ondas de som saindo da boca.
fn pref_speech(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + s * 0.18, y + s * 0.9);
    pb.line_to(x + s * 0.18, y + s * 0.62);
    pb.cubic_to(x + s * 0.06, y + s * 0.5, x + s * 0.08, y + s * 0.12, x + s * 0.38, y + s * 0.1);
    pb.cubic_to(x + s * 0.58, y + s * 0.1, x + s * 0.62, y + s * 0.3, x + s * 0.6, y + s * 0.38);
    pb.line_to(x + s * 0.66, y + s * 0.5);
    pb.line_to(x + s * 0.6, y + s * 0.52);
    pb.line_to(x + s * 0.6, y + s * 0.62);
    pb.cubic_to(x + s * 0.6, y + s * 0.7, x + s * 0.5, y + s * 0.7, x + s * 0.44, y + s * 0.7);
    pb.line_to(x + s * 0.44, y + s * 0.9);
    pb.close();
    pm.fill(pb.finish(), &vgrad(y + s * 0.1, y + s * 0.9, &[(0.0, rgba(120, 160, 230, 255)), (1.0, rgba(30, 80, 180, 255))]));
    for (k, a) in [(1.0, 230u8), (1.7, 180), (2.4, 130)] {
        let rr = s * 0.07 * k;
        let (ox, oy) = (x + s * 0.68, y + s * 0.55);
        let mut pb = PathBuilder::new();
        pb.move_to(ox + rr * 0.3, oy - rr);
        pb.quad_to(ox + rr * 1.2, oy, ox + rr * 0.3, oy + rr);
        pm.stroke(pb.finish(), &solid(rgba(40, 110, 220, a)), (s * 0.035).max(1.0));
    }
}

/// Disco prateado com reflexo de arco-íris e uma nota azul na frente.
fn music(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let lw = (s / 64.0).max(1.0);
    let (cx, cy, r) = (x + s * 0.46, y + s * 0.5, s * 0.4);
    pm.fill(circle(cx + s * 0.01, cy + s * 0.03, r), &solid(rgba(0, 0, 0, 50)));
    pm.fill(circle(cx, cy, r), &linear(cx - r, cy - r, cx + r, cy + r, &[(0.0, rgba(250, 250, 252, 255)), (0.3, rgba(200, 220, 250, 255)), (0.5, rgba(250, 230, 200, 255)), (0.7, rgba(210, 240, 220, 255)), (1.0, rgba(190, 196, 210, 255))]));
    pm.stroke(circle(cx, cy, r), &solid(rgba(130, 134, 146, 255)), lw);
    pm.fill(circle(cx, cy, r * 0.28), &solid(rgba(215, 218, 226, 255)));
    pm.stroke(circle(cx, cy, r * 0.28), &solid(rgba(150, 154, 164, 255)), lw);
    pm.fill(circle(cx, cy, r * 0.1), &solid(rgba(255, 255, 255, 255)));
    // Nota (colcheia dupla) em azul.
    let ink = solid(rgba(30, 90, 200, 255));
    let (nx, ny) = (x + s * 0.58, y + s * 0.5);
    pm.fill(oval(nx - s * 0.02, ny + s * 0.24, s * 0.16, s * 0.12), &ink);
    pm.fill(oval(nx + s * 0.22, ny + s * 0.18, s * 0.16, s * 0.12), &ink);
    pm.fill_rect(nx + s * 0.12, ny - s * 0.12, s * 0.04, s * 0.42, &ink);
    pm.fill_rect(nx + s * 0.36, ny - s * 0.18, s * 0.04, s * 0.42, &ink);
    let mut pb = PathBuilder::new();
    pb.move_to(nx + s * 0.12, ny - s * 0.12);
    pb.line_to(nx + s * 0.4, ny - s * 0.18);
    pb.line_to(nx + s * 0.4, ny - s * 0.08);
    pb.line_to(nx + s * 0.12, ny - s * 0.02);
    pb.close();
    pm.fill(pb.finish(), &ink);
}



/// Clipe de papel inclinado, em metal cinza.
fn clip(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let (cx, cy) = (x + s * 0.5, y + s * 0.5);
    let k = s / 24.0;
    let mut pb = PathBuilder::new();
    // Coordenadas num quadrado de 24, giradas ~30 graus.
    let pts = [(-2.0, 9.0), (-2.0, -6.0), (2.0, -10.0), (6.0, -6.0), (6.0, 7.0), (2.0, 11.0), (-1.5, 7.0), (-1.5, -4.0), (2.0, -6.5), (2.5, -4.0), (2.5, 6.0)];
    let (sa, ca) = (0.5f32, 0.866f32);
    for (i, (px, py)) in pts.iter().enumerate() {
        let (rx, ry) = (px * ca - py * sa, px * sa + py * ca);
        let (qx, qy) = (cx + rx * k, cy + ry * k);
        if i == 0 { pb.move_to(qx, qy) } else { pb.line_to(qx, qy) }
    }
    if let Some(p) = pb.finish() {
        pm.stroke(Some(p.clone()), &solid(rgba(70, 72, 80, 255)), (s / 9.0).max(1.4));
        pm.stroke(Some(p), &solid(rgba(200, 204, 212, 255)), (s / 22.0).max(0.6));
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
