//! AppImage: abrir com duplo clique e instalar arrastando, como as janelas de
//! instalação do Mac (o ícone do app, uma seta e a pasta Aplicativos).
//!
//! - **Abrir sem instalar:** torna o arquivo executável e roda.
//! - **Instalar** (arrastar o ícone para Aplicativos, ou o botão): copia para
//!   `~/Aplicativos`, tira o ícone e o nome de dentro do AppImage e cria o atalho
//!   (`~/.local/share/applications`): o app aparece em Todos os Apps, na Busca Rápida
//!   e pode ir para o Dock.
//!
//! O AppImage roda pelo FUSE (libfuse2); sem ele, com `APPIMAGE_EXTRACT_AND_RUN=1`.

use glove_config::tr;
use std::path::{Path, PathBuf};
use std::process::Command;

use glove_ui::{
    paint::{draw::*, icons::{self, Icon}},
    run, App, Rect, Ui, WindowOptions,
};

/// É um AppImage? (pela extensão, ou pela marca "AI" 2 no cabeçalho do ELF)
pub fn is_appimage(p: &Path) -> bool {
    if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("appimage")) {
        return true;
    }
    let mut head = [0u8; 11];
    std::fs::File::open(p).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head[..4] == b"\x7fELF" && &head[8..11] == b"AI\x02"
}

/// O FUSE de que o AppImage precisa para rodar montado (senão, extrai e roda).
fn has_fuse2() -> bool {
    ["/usr/lib/x86_64-linux-gnu/libfuse.so.2", "/lib/x86_64-linux-gnu/libfuse.so.2", "/usr/lib/libfuse.so.2", "/usr/lib64/libfuse.so.2"].iter().any(|p| Path::new(p).exists())
}

fn make_exec(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(p)?.permissions();
    perm.set_mode(perm.mode() | 0o111);
    std::fs::set_permissions(p, perm)
}

/// Roda o AppImage (desligado do Arquivos: continua aberto se o Arquivos fechar).
pub fn run_appimage(p: &Path) -> Result<(), String> {
    make_exec(p).map_err(|e| tr!("Não foi possível torná-lo executável: {}", e))?;
    let mut c = Command::new(p);
    if !has_fuse2() {
        c.env("APPIMAGE_EXTRACT_AND_RUN", "1");
    }
    if let Some(d) = p.parent() {
        c.current_dir(d);
    }
    c.spawn().map(|mut ch| {
        std::thread::spawn(move || ch.wait());
    }).map_err(|e| e.to_string())
}

/// O que vem dentro do AppImage: o nome do app (do .desktop) e o ícone (.DirIcon).
struct Inside {
    name: Option<String>,
    icon: Option<PathBuf>,
}

/// Tira o .desktop e o ícone de dentro do AppImage numa pasta temporária (sem montar).
fn look_inside(p: &Path) -> Inside {
    let mut out = Inside { name: None, icon: None };
    let Ok(tmp) = std::env::temp_dir().join(format!("glove-appimage-{}", std::process::id())).canonicalize().or_else(|_| {
        let d = std::env::temp_dir().join(format!("glove-appimage-{}", std::process::id()));
        std::fs::create_dir_all(&d).map(|_| d)
    }) else {
        return out;
    };
    if make_exec(p).is_err() {
        return out;
    }
    let extract = |pat: &str| {
        let _ = Command::new(p).arg("--appimage-extract").arg(pat).current_dir(&tmp).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    };
    extract("*.desktop");
    extract(".DirIcon");
    let root = tmp.join("squashfs-root");
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "desktop") {
                let t = std::fs::read_to_string(e.path()).unwrap_or_default();
                out.name = t.lines().find_map(|l| l.strip_prefix("Name=")).map(|s| s.trim().to_string());
                break;
            }
        }
    }
    let dir_icon = root.join(".DirIcon");
    // O .DirIcon costuma ser um atalho para o PNG do app: tira também o alvo.
    if let Ok(target) = std::fs::read_link(&dir_icon) {
        extract(&target.to_string_lossy());
        let real = root.join(&target);
        if real.is_file() {
            out.icon = Some(real);
        }
    } else if dir_icon.is_file() {
        out.icon = Some(dir_icon);
    }
    out
}

fn apps_folder() -> PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Aplicativos")
}

/// Instala: copia para ~/Aplicativos, guarda o ícone e cria o atalho do app.
fn install(src: &Path, inside: &Inside) -> Result<PathBuf, String> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let dir = apps_folder();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = src.file_name().ok_or("nome")?;
    let dest = dir.join(file);
    if dest != src {
        std::fs::copy(src, &dest).map_err(|e| tr!("Não foi possível copiar: {}", e))?;
    }
    make_exec(&dest).map_err(|e| e.to_string())?;
    let id: String = file.to_string_lossy().trim_end_matches(".AppImage").trim_end_matches(".appimage").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let icons = home.join(".local/share/glove/appimages");
    let _ = std::fs::create_dir_all(&icons);
    let icon = inside.icon.as_ref().and_then(|i| {
        let to = icons.join(format!("{id}.png"));
        std::fs::copy(i, &to).ok().map(|_| to.display().to_string())
    });
    let name = inside.name.clone().unwrap_or_else(|| id.replace('-', " "));
    let apps = home.join(".local/share/applications");
    std::fs::create_dir_all(&apps).map_err(|e| e.to_string())?;
    let exec = format!("\"{}\" %U", dest.display());
    let exec = if has_fuse2() { exec } else { format!("env APPIMAGE_EXTRACT_AND_RUN=1 {exec}") };
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nIcon={}\nTerminal=false\nCategories=Utility;\nComment={}\nX-Glove-AppImage={}\n",
        icon.unwrap_or_else(|| "application-x-executable".into()),
        tr!("AppImage instalado pelo Glove"),
        dest.display()
    );
    std::fs::write(apps.join(format!("appimage-{id}.desktop")), entry).map_err(|e| e.to_string())?;
    Ok(dest)
}

struct Installer {
    path: PathBuf,
    name: String,
    inside: Inside,
    icon_img: Option<glove_ui::paint::Pixmap>,
    /// Arrastando o ícone: o deslocamento do ponteiro.
    drag: Option<(f32, f32)>,
    message: Option<(bool, String)>,
    installed: Option<PathBuf>,
}

const ICON: f32 = 96.0;

impl App for Installer {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(238, 238, 240, 255)), (1.0, rgba(222, 222, 226, 255))]));
        let ink = rgba(20, 20, 24, 255);
        let title = tr!("Instalar “{}”", self.name);
        let tw = ui.text_width(&title, 15.0, true);
        ui.fonts.draw(ui.c, &title, (w - tw) / 2.0, 34.0, 15.0, true, ink);
        let sub = tr!("Arraste o ícone para a pasta Aplicativos para instalar.");
        let sw = ui.text_width(sub, 11.5, false);
        ui.fonts.draw(ui.c, sub, (w - sw) / 2.0, 54.0, 11.5, false, rgba(80, 80, 86, 255));
        // O app à esquerda, a seta e a pasta à direita (como as janelas do Mac).
        let (ax, ay) = (w * 0.27 - ICON / 2.0, 80.0);
        let (fx, fy) = (w * 0.73 - ICON / 2.0, 80.0);
        let folder = Rect::new(fx - 10.0, fy - 10.0, ICON + 20.0, ICON + 40.0);
        let mut pb = glove_ui::paint::tiny_skia::PathBuilder::new();
        let (y, x0, x1) = (ay + ICON / 2.0, ax + ICON + 18.0, fx - 18.0);
        pb.move_to(x0, y);
        pb.line_to(x1, y);
        pb.move_to(x1 - 12.0, y - 10.0);
        pb.line_to(x1, y);
        pb.line_to(x1 - 12.0, y + 10.0);
        ui.c.stroke_round(pb.finish(), &solid(rgba(120, 120, 128, 255)), 5.0);
        let over = self.drag.is_some() && folder.contains(ui.pointer());
        if over {
            ui.c.fill(rrect(folder.x, folder.y, folder.w, folder.h, 10.0), &solid(rgba(60, 120, 220, 50)));
        }
        icons::draw(ui.c, ui.fonts, Icon::AppsPlace, fx, fy, ICON, None);
        let fl = tr!("Aplicativos");
        let flw = ui.text_width(fl, 12.0, false);
        ui.fonts.draw(ui.c, fl, fx + (ICON - flw) / 2.0, fy + ICON + 18.0, 12.0, false, ink);
        // O ícone do app (arrastável).
        let app_r = Rect::new(ax, ay, ICON, ICON);
        let resp = ui.hit(app_r);
        if resp.pressed && self.installed.is_none() {
            let (px, py) = ui.pointer();
            self.drag = Some((px - ax, py - ay));
        }
        let (dx, dy) = match self.drag {
            Some((ox, oy)) => {
                let (px, py) = ui.pointer();
                (px - ox, py - oy)
            }
            None => (ax, ay),
        };
        match &self.icon_img {
            Some(img) => {
                let s = ICON / img.width().max(img.height()) as f32;
                let t = glove_ui::paint::tiny_skia::Transform::from_scale(s * ui.c.scale, s * ui.c.scale).post_translate(dx * ui.c.scale, dy * ui.c.scale);
                ui.c.draw_pixmap_t(img.as_ref(), &glove_ui::paint::tiny_skia::PixmapPaint { quality: glove_ui::paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t);
            }
            None => icons::draw(ui.c, ui.fonts, Icon::Window, dx, dy, ICON, None),
        }
        let nl = ui.fonts.ellipsize(&self.name, 12.0, false, ICON + 60.0);
        let nlw = ui.text_width(&nl, 12.0, false);
        ui.fonts.draw(ui.c, &nl, ax + (ICON - nlw) / 2.0, ay + ICON + 18.0, 12.0, false, ink);
        if self.drag.is_some() {
            ui.wake_after(std::time::Duration::from_millis(16));
            if !ui.mouse_down() {
                self.drag = None;
                if over {
                    self.do_install();
                }
            }
        }
        // Mensagem e botões.
        if let Some((ok, m)) = &self.message {
            let m = ui.fonts.ellipsize(m, 11.5, false, w - 40.0);
            let mw = ui.text_width(&m, 11.5, false);
            ui.fonts.draw(ui.c, &m, (w - mw) / 2.0, h - 54.0, 11.5, !*ok, if *ok { rgba(30, 110, 50, 255) } else { rgba(180, 30, 30, 255) });
        }
        let (mut open, mut inst, mut close) = (false, false, false);
        ui.area(Rect::new(20.0, h - 40.0, w - 40.0, 28.0), |ui| {
            ui.horizontal(|ui| {
                open = ui.button(if self.installed.is_some() { tr!("Abrir") } else { tr!("Abrir sem Instalar") });
                ui.set_active(self.installed.is_none());
                inst = ui.button(tr!("Instalar"));
                ui.set_active(true);
                close = ui.default_button(tr!("Fechar"));
            });
        });
        if inst {
            self.do_install();
        }
        if open {
            let p = self.installed.clone().unwrap_or_else(|| self.path.clone());
            match run_appimage(&p) {
                Ok(()) => ui.close = true,
                Err(e) => self.message = Some((false, e)),
            }
        }
        if close || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            ui.close = true;
        }
    }
}

impl Installer {
    fn do_install(&mut self) {
        match install(&self.path, &self.inside) {
            Ok(dest) => {
                self.message = Some((true, tr!("Instalado em Aplicativos. Ele já aparece em Todos os Apps.").into()));
                self.installed = Some(dest);
            }
            Err(e) => self.message = Some((false, tr!("Não deu certo: {}", e))),
        }
    }
}

/// A janela de instalar (`glove-files --appimage ARQUIVO`).
pub fn main(path: PathBuf) {
    let inside = look_inside(&path);
    let icon_img = inside.icon.as_ref().and_then(|i| glove_ui::image::decode(i).ok());
    let name = inside.name.clone().unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let opts = WindowOptions { title: tr!("Instalar App").into(), app_id: "glove.Files".into(), size: (520, 320), fixed: true, min_size: None };
    let app = Installer { path, name, inside, icon_img, drag: None, message: None, installed: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-files: {e}");
    }
}
