//! glove-art: gera por código a arte que o Glove precisa fora dele mesmo.
//!
//!   glove-art plymouth <pasta> [tema]  tema do Plymouth (início e desligamento do sistema):
//!                                 logo (a luva), 12 quadros do giratório, .plymouth e .script;
//!                                 tema: claro (o "glove"), moderno ou moderno-escuro
//!   glove-art icons <pasta>     tema de ícones "Glove" (freedesktop): PNGs de 16 a 256
//!                                 pixels (1× e 2×), index.theme e os nomes comuns como atalhos
//!   glove-art firefox <pasta>    tema do Glove para o Firefox (metal escovado, como o
//!                                 Safari clássico): manifest.json e metal.png
//!   glove-art preview <arquivo>   folha de amostra do tema de ícones (PNG): todos os ícones
//!                                 a 128 px com o nome freedesktop embaixo (preview do glove-icons)

use std::path::Path;

use glove_paint::{
    draw::{rgba, spinner, Canvas},
    theme::{Theme, Variant},
};

/// Tamanho base dos desenhos (a escala 2 deixa nítido em telas densas).
const SCALE: f32 = 2.0;

fn save(c: &Canvas, dir: &Path, name: &str) -> Result<(), String> {
    c.pm.save_png(dir.join(name)).map_err(|e| format!("{name}: {e}"))
}

/// PNG compacto e sem perda (para o tema de ícones): compressão máxima com
/// filtro adaptativo, cor zerada onde o pixel é transparente e sem o canal
/// alfa quando a imagem é toda opaca. Os pixels visíveis saem idênticos.
fn save_compact(c: &Canvas, dir: &Path, name: &str) -> Result<(), String> {
    let (w, h) = (c.pm.width(), c.pm.height());
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let mut opaque = true;
    for p in c.pm.pixels() {
        let d = p.demultiply();
        if d.alpha() == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[d.red(), d.green(), d.blue(), d.alpha()]);
        }
        opaque &= d.alpha() == 255;
    }
    let (data, color) = if opaque {
        (rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect(), png::ColorType::Rgb)
    } else {
        (rgba, png::ColorType::Rgba)
    };
    let file = std::fs::File::create(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(color);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Best);
    enc.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let mut wr = enc.write_header().map_err(|e| format!("{name}: {e}"))?;
    wr.write_image_data(&data).map_err(|e| format!("{name}: {e}"))
}

fn plymouth(dir: &Path, tema: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let variante = Variant::from_config(tema);
    let theme = Theme::new(variante);
    // O nome do tema: "glove" (clássico), "glove-moderno" e "glove-moderno-escuro".
    let nome = if theme.modern { format!("glove-{}", variante.id()) } else { "glove".to_string() };

    // A logo: a luva de gel (clássico) ou chapada, preta ou branca (Moderno), a mesma da
    // tela de encerramento.
    let s = 112.0;
    let mut c = Canvas::new(s, s, SCALE).ok_or("canvas")?;
    glove_paint::icons::logo_for(&mut c, &theme, 0.0, 0.0, s);
    save(&c, dir, "logo.png")?;

    // Giratório de 12 raios, um quadro por passo (claro no fundo escuro).
    let raio = if theme.dark { rgba(210, 210, 216, 255) } else { rgba(70, 72, 76, 255) };
    for i in 0..12 {
        let s = 32.0;
        let mut c = Canvas::new(s, s, SCALE).ok_or("canvas")?;
        spinner(&mut c, s / 2.0, s / 2.0, s / 2.0, i as f32 / 12.0, raio);
        save(&c, dir, &format!("spinner-{i}.png"))?;
    }

    // Fundo (cima, baixo) e texto, como a tela de encerramento do Glove.
    let (cima, baixo, texto) = match (theme.modern, theme.dark) {
        (true, true) => ("0.118, 0.118, 0.129", "0.086, 0.086, 0.098", "0.84, 0.84, 0.86"),
        (true, false) => ("0.949, 0.949, 0.957", "0.902, 0.902, 0.918", "0.24, 0.24, 0.26"),
        _ => ("0.84, 0.85, 0.86", "0.74, 0.75, 0.77", "0.24, 0.24, 0.26"),
    };
    let descricao = match (theme.modern, theme.dark) {
        (true, true) => "fundo grafite, a luva branca",
        (true, false) => "fundo claro e liso, a luva preta",
        _ => "fundo cinza claro, a luva de gel",
    };
    let titulo = match (theme.modern, theme.dark) {
        (true, true) => "Glove Moderno Escuro",
        (true, false) => "Glove Moderno",
        _ => "Glove",
    };
    let cfg = PLYMOUTH.replace("{titulo}", titulo).replace("{nome}", &nome).replace("{descricao}", descricao);
    let script = SCRIPT.replace("{cima}", cima).replace("{baixo}", baixo).replace("{texto}", texto);
    std::fs::write(dir.join(format!("{nome}.plymouth")), cfg).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{nome}.script")), script).map_err(|e| e.to_string())?;
    Ok(())
}

const PLYMOUTH: &str = "[Plymouth Theme]
Name={titulo}
Description=Início e desligamento do Glove: {descricao} e o giratório (arte gerada por código).
ModuleName=script

[script]
ImageDir=/usr/share/plymouth/themes/{nome}
ScriptFile=/usr/share/plymouth/themes/{nome}/{nome}.script
";

/// Script do Plymouth: fundo cinza claro em degradê, a logo no meio, giratório
/// embaixo; "Desligando…" ao desligar; pedido de senha (disco criptografado).
const SCRIPT: &str = r#"# Tema do Plymouth do Glove (gerado por glove-art).
Window.SetBackgroundTopColor({cima});
Window.SetBackgroundBottomColor({baixo});

cx = Window.GetX() + Window.GetWidth() / 2;
cy = Window.GetY() + Window.GetHeight() / 2;

logo.image = Image("logo.png");
logo.sprite = Sprite(logo.image);
logo.sprite.SetPosition(cx - logo.image.GetWidth() / 2, cy - logo.image.GetHeight() / 2 - 40, 1);

for (i = 0; i < 12; i++)
    spin[i] = Image("spinner-" + i + ".png");
spinner.sprite = Sprite(spin[0]);
spinner.sprite.SetPosition(cx - spin[0].GetWidth() / 2, cy + 90, 1);

if (Plymouth.GetMode() == "shutdown" || Plymouth.GetMode() == "reboot") {
    label = "Desligando…";
    if (Plymouth.GetMode() == "reboot") label = "Reiniciando…";
    msg.image = Image.Text(label, {texto});
    msg.sprite = Sprite(msg.image);
    msg.sprite.SetPosition(cx - msg.image.GetWidth() / 2, cy + 140, 1);
}

frame = 0;
fun refresh_callback() {
    frame++;
    spinner.sprite.SetImage(spin[Math.Int(frame / 4) % 12]);
}
Plymouth.SetRefreshFunction(refresh_callback);

# Senha do disco criptografado: o texto do pedido e uma bolinha por letra.
fun display_password_callback(prompt, bullets) {
    prompt_sprite = Sprite(Image.Text(prompt, {texto}));
    prompt_sprite.SetPosition(cx - prompt_sprite.GetImage().GetWidth() / 2, cy + 130, 2);
    dots = "";
    for (i = 0; i < bullets; i++)
        dots = dots + "●";
    dots_sprite = Sprite(Image.Text(dots + " ", {texto}));
    dots_sprite.SetPosition(cx - dots_sprite.GetImage().GetWidth() / 2, cy + 155, 2);
    spinner.sprite.SetOpacity(0);
}
Plymouth.SetDisplayPasswordFunction(display_password_callback);

fun display_normal_callback() {
    prompt_sprite = NULL;
    dots_sprite = NULL;
    spinner.sprite.SetOpacity(1);
}
Plymouth.SetDisplayNormalFunction(display_normal_callback);
"#;

/// Tamanhos do tema: (tamanho, escala).
const ICON_SIZES: [(u32, u32); 16] = [(16, 1), (22, 1), (24, 1), (32, 1), (48, 1), (64, 1), (96, 1), (128, 1), (256, 1), (16, 2), (22, 2), (24, 2), (32, 2), (48, 2), (64, 2), (128, 2)];

fn context_name(ctx: &str) -> &'static str {
    match ctx {
        "apps" => "Applications",
        "places" => "Places",
        "devices" => "Devices",
        "mimetypes" => "MimeTypes",
        "categories" => "Categories",
        "status" => "Status",
        _ => "Actions",
    }
}

/// Gera o tema de ícones "Glove" a partir dos desenhos do código.
fn icones(dir: &Path) -> Result<usize, String> {
    use glove_paint::icons;
    let fonts = glove_paint::Fonts::load_system().ok_or("nenhuma fonte do sistema")?;
    let entries = icons::theme_entries();
    let mut contexts: Vec<&str> = entries.iter().map(|e| e.1).collect();
    contexts.sort();
    contexts.dedup();
    let mut dirs = Vec::new();
    let mut count = 0;
    for (size, scale) in ICON_SIZES {
        let base = if scale == 1 { format!("{size}x{size}") } else { format!("{size}x{size}@{scale}") };
        for ctx in &contexts {
            let sub = format!("{base}/{ctx}");
            let out = dir.join(&sub);
            std::fs::create_dir_all(&out).map_err(|e| format!("{sub}: {e}"))?;
            // Versão 2× com os mesmos pixels de um tamanho 1× que já existe:
            // atalho para ele em vez de outra cópia (o tema fica bem menor).
            let twin = (scale > 1 && ICON_SIZES.contains(&(size * scale, 1))).then(|| format!("../../{0}x{0}/{ctx}", size * scale));
            for (icon, c, names) in entries.iter().filter(|e| e.1 == *ctx) {
                let _ = c;
                if let Some(t) = &twin {
                    for name in names.iter() {
                        let link = out.join(format!("{name}.png"));
                        let _ = std::fs::remove_file(&link);
                        std::os::unix::fs::symlink(format!("{t}/{name}.png"), &link).map_err(|e| format!("{name}: {e}"))?;
                    }
                    continue;
                }
                let s = size as f32;
                let mut canvas = Canvas::new(s, s, scale as f32).ok_or("canvas")?;
                icons::draw(&mut canvas, &fonts, *icon, 0.0, 0.0, s, None);
                let file = format!("{}.png", names[0]);
                save_compact(&canvas, &out, &file)?;
                count += 1;
                for alias in &names[1..] {
                    let link = out.join(format!("{alias}.png"));
                    let _ = std::fs::remove_file(&link);
                    std::os::unix::fs::symlink(&file, &link).map_err(|e| format!("{alias}: {e}"))?;
                }
            }
            dirs.push((sub, size, scale, *ctx));
        }
    }
    let mut index = String::from("[Icon Theme]\nName=Glove\nComment=Original code-drawn icons in the style of Mac OS X Tiger\nInherits=Adwaita,hicolor\nExample=folder\n");
    index += &format!("Directories={}\n", dirs.iter().map(|d| d.0.as_str()).collect::<Vec<_>>().join(","));
    for (sub, size, scale, ctx) in &dirs {
        index += &format!("\n[{sub}]\nSize={size}\n");
        if *scale > 1 {
            index += &format!("Scale={scale}\n");
        }
        index += &format!("Context={}\nType=Fixed\n", context_name(ctx));
    }
    std::fs::write(dir.join("index.theme"), index).map_err(|e| e.to_string())?;
    Ok(count)
}

/// Folha de amostra do tema de ícones: grade com todos os ícones a 128 px e o
/// nome freedesktop de cada um embaixo (a imagem do README do glove-icons).
fn previa(file: &Path) -> Result<usize, String> {
    use glove_paint::{draw::solid, icons};
    let fonts = glove_paint::Fonts::load_system().ok_or("nenhuma fonte do sistema")?;
    let entries = icons::theme_entries();
    const COLS: usize = 10;
    let (cell_w, cell_h, icon, margin, px) = (176.0, 176.0, 128.0, 24.0, 11.0);
    let rows = entries.len().div_ceil(COLS);
    let w = margin * 2.0 + cell_w * COLS as f32;
    let h = margin * 2.0 + cell_h * rows as f32;
    let mut c = Canvas::new(w, h, 1.0).ok_or("canvas")?;
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(236, 236, 236, 255)));
    for (i, (ic, _, names)) in entries.iter().enumerate() {
        let x = margin + cell_w * (i % COLS) as f32;
        let y = margin + cell_h * (i / COLS) as f32;
        icons::draw(&mut c, &fonts, *ic, x + (cell_w - icon) / 2.0, y, icon, None);
        let label = fonts.ellipsize(names[0], px, false, cell_w - 8.0);
        let tw = fonts.measure(&label, px, false);
        fonts.draw(&mut c, &label, x + (cell_w - tw) / 2.0, y + icon + 22.0, px, false, rgba(40, 40, 42, 255));
    }
    if let Some(d) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    save_compact(&c, file.parent().unwrap_or(Path::new(".")), &file.file_name().ok_or("arquivo")?.to_string_lossy())?;
    Ok(entries.len())
}

/// Tema do Firefox no visual do Glove: o metal escovado (o mesmo do tema Metal) atrás
/// das abas e da barra de ferramentas, campo de endereço branco com o foco azul e
/// menus claros, como o Safari da era. O Glove OS o instala para todos os perfis.
fn firefox(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let theme = Theme::new(Variant::Metal);
    // Largo o bastante para monitores grandes (o Firefox o encosta à direita, sem repetir).
    let (w, h) = (3840.0, 220.0);
    let mut c = Canvas::new(w, h, 1.0).ok_or("canvas")?;
    c.brushed_metal(0.0, 0.0, w, h, theme.metal_top, theme.metal_bottom);
    c.metal_sheen(0.0, 0.0, w, h, w);
    save_compact(&c, dir, "metal.png")?;
    let manifest = format!(
        r##"{{
  "manifest_version": 2,
  "name": "Glove",
  "version": "{v}",
  "description": "O visual do Glove OS no Firefox: metal escovado, como nos navegadores da época.",
  "browser_specific_settings": {{ "gecko": {{ "id": "glove-tema@glove-os" }} }},
  "theme": {{
    "images": {{ "theme_frame": "metal.png" }},
    "colors": {{
      "frame": "#b9b9b9",
      "frame_inactive": "#d2d2d2",
      "tab_background_text": "#1b1b1b",
      "tab_text": "#111111",
      "tab_selected": "rgba(255, 255, 255, 0.45)",
      "tab_line": "#6f6f6f",
      "tab_loading": "#3875d7",
      "toolbar": "rgba(255, 255, 255, 0)",
      "toolbar_text": "#1b1b1b",
      "icons": "#2c2c2c",
      "icons_attention": "#3875d7",
      "toolbar_top_separator": "rgba(0, 0, 0, 0)",
      "toolbar_bottom_separator": "#7d7d7d",
      "toolbar_vertical_separator": "rgba(0, 0, 0, 0.18)",
      "toolbar_field": "#ffffff",
      "toolbar_field_text": "#000000",
      "toolbar_field_border": "#8c8c8c",
      "toolbar_field_focus": "#ffffff",
      "toolbar_field_border_focus": "#5b9ae8",
      "toolbar_field_highlight": "#b5d5ff",
      "toolbar_field_highlight_text": "#000000",
      "button_background_hover": "rgba(0, 0, 0, 0.10)",
      "button_background_active": "rgba(0, 0, 0, 0.20)",
      "popup": "#f5f6f8",
      "popup_text": "#141414",
      "popup_border": "#9a9a9a",
      "popup_highlight": "#3875d7",
      "popup_highlight_text": "#ffffff",
      "sidebar": "#e6ebf1",
      "sidebar_text": "#141414",
      "sidebar_border": "#a7a7a7",
      "sidebar_highlight": "#3875d7",
      "sidebar_highlight_text": "#ffffff",
      "bookmark_text": "#1b1b1b",
      "ntp_background": "#ececec",
      "ntp_text": "#222222"
    }}
  }}
}}
"##,
        v = env!("CARGO_PKG_VERSION")
    );
    std::fs::write(dir.join("manifest.json"), manifest).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // O tema de ícones e a amostra: calendários com uma data só (17 de julho), para os
    // arquivos não mudarem conforme o dia em que foram gerados.
    glove_paint::icons::fix_calendar_date(7, 17);
    let r = match args.first().map(String::as_str) {
        Some("plymouth") => plymouth(Path::new(args.get(1).map(String::as_str).unwrap_or("data/plymouth/glove")), args.get(2).map(String::as_str).unwrap_or("claro")),
        Some("icons") => match args.get(1) {
            Some(d) => icones(Path::new(d)).map(|n| println!("{n} imagens em {d}")),
            None => Err("usage: glove-art icons <folder>".into()),
        },
        Some("firefox") => match args.get(1) {
            Some(d) => firefox(Path::new(d)).map(|_| println!("tema do Firefox em {d}")),
            None => Err("usage: glove-art firefox <folder>".into()),
        },
        Some("preview") => match args.get(1) {
            Some(f) => previa(Path::new(f)).map(|n| println!("{n} ícones em {f}")),
            None => Err("usage: glove-art preview <file.png>".into()),
        },
        _ => Err("usage: glove-art plymouth <folder> | glove-art icons <folder> | glove-art firefox <folder> | glove-art preview <file.png>".into()),
    };
    if let Err(e) = r {
        eprintln!("glove-art: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    const TEMAS: [(&str, &str); 3] = [("glove", "claro"), ("glove-moderno", "moderno"), ("glove-moderno-escuro", "moderno-escuro")];

    #[test]
    fn gera_os_temas() {
        for (nome, tema) in TEMAS {
            let dir = std::env::temp_dir().join(format!("glove-art-{}-{nome}", std::process::id()));
            super::plymouth(&dir, tema).unwrap();
            for f in ["logo.png".to_string(), "spinner-0.png".into(), "spinner-11.png".into(), format!("{nome}.plymouth"), format!("{nome}.script")] {
                assert!(dir.join(&f).is_file(), "{f}");
            }
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// Os temas guardados em data/plymouth (os que vão no pacote glove-os e na ISO) são
    /// os que o código gera hoje: mudou a logo, tem de regenerar (glove-art plymouth).
    #[test]
    fn temas_do_repositorio_em_dia() {
        for (nome, tema) in TEMAS {
            let dir = std::env::temp_dir().join(format!("glove-art-repo-{}-{nome}", std::process::id()));
            super::plymouth(&dir, tema).unwrap();
            let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/plymouth").join(nome);
            for f in ["logo.png".to_string(), "spinner-0.png".into(), "spinner-11.png".into(), format!("{nome}.plymouth"), format!("{nome}.script")] {
                assert!(std::fs::read(dir.join(&f)).unwrap() == std::fs::read(repo.join(&f)).unwrap(), "{nome}/{f} desatualizado: rode glove-art plymouth data/plymouth/{nome} {tema}");
            }
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
