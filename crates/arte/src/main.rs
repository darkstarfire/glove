//! glove-arte: gera por código a arte que o Glove precisa fora dele mesmo.
//!
//!   glove-arte plymouth <pasta>   tema do Plymouth (início e desligamento do sistema):
//!                                 orbe, 12 quadros do giratório, glove.plymouth e glove.script
//!   glove-arte icones <pasta>     tema de ícones "Glove" (freedesktop): PNGs de 16 a 256
//!                                 pixels (1× e 2×), index.theme e os nomes comuns como atalhos

use std::path::Path;

use glove_paint::{
    draw::{gel_circle, rgba, spinner, Canvas},
    theme::{Theme, Variant},
};

/// Tamanho base dos desenhos (a escala 2 deixa nítido em telas densas).
const SCALE: f32 = 2.0;

fn save(c: &Canvas, dir: &Path, name: &str) -> Result<(), String> {
    c.pm.save_png(dir.join(name)).map_err(|e| format!("{name}: {e}"))
}

fn plymouth(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let theme = Theme::new(Variant::Claro);

    // Orbe em gel, a mesma do "Sobre o Glove" e da tela de encerramento.
    let s = 96.0;
    let mut c = Canvas::new(s, s, SCALE).ok_or("canvas")?;
    gel_circle(&mut c, s / 2.0, s / 2.0, s / 2.0 - 2.0, theme.accent);
    save(&c, dir, "logo.png")?;

    // Giratório de 12 raios, um quadro por passo.
    for i in 0..12 {
        let s = 32.0;
        let mut c = Canvas::new(s, s, SCALE).ok_or("canvas")?;
        spinner(&mut c, s / 2.0, s / 2.0, s / 2.0, i as f32 / 12.0, rgba(70, 72, 76, 255));
        save(&c, dir, &format!("spinner-{i}.png"))?;
    }

    std::fs::write(dir.join("glove.plymouth"), PLYMOUTH).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("glove.script"), SCRIPT).map_err(|e| e.to_string())?;
    Ok(())
}

const PLYMOUTH: &str = "[Plymouth Theme]
Name=Glove
Description=Início e desligamento do Glove: fundo cinza claro, orbe e giratório (arte gerada por código).
ModuleName=script

[script]
ImageDir=/usr/share/plymouth/themes/glove
ScriptFile=/usr/share/plymouth/themes/glove/glove.script
";

/// Script do Plymouth: fundo cinza claro em degradê, orbe no meio, giratório
/// embaixo; "Desligando…" ao desligar; pedido de senha (disco criptografado).
const SCRIPT: &str = r#"# Tema do Plymouth do Glove (gerado por glove-arte).
Window.SetBackgroundTopColor(0.84, 0.85, 0.86);
Window.SetBackgroundBottomColor(0.74, 0.75, 0.77);

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
    msg.image = Image.Text(label, 0.24, 0.24, 0.26);
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
    prompt_sprite = Sprite(Image.Text(prompt, 0.2, 0.2, 0.22));
    prompt_sprite.SetPosition(cx - prompt_sprite.GetImage().GetWidth() / 2, cy + 130, 2);
    dots = "";
    for (i = 0; i < bullets; i++)
        dots = dots + "●";
    dots_sprite = Sprite(Image.Text(dots + " ", 0.1, 0.1, 0.12));
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
            for (icon, c, names) in entries.iter().filter(|e| e.1 == *ctx) {
                let _ = c;
                let s = size as f32;
                let mut canvas = Canvas::new(s, s, scale as f32).ok_or("canvas")?;
                icons::draw(&mut canvas, &fonts, *icon, 0.0, 0.0, s, None);
                let file = format!("{}.png", names[0]);
                save(&canvas, &out, &file)?;
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
    let mut index = String::from("[Icon Theme]\nName=Glove\nComment=Ícones do Glove no estilo do Mac OS X Tiger, desenhados por código (arte própria)\nInherits=Adwaita,hicolor\nExample=folder\n");
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("plymouth") => plymouth(Path::new(args.get(1).map(String::as_str).unwrap_or("data/plymouth/glove"))),
        Some("icones") => match args.get(1) {
            Some(d) => icones(Path::new(d)).map(|n| println!("{n} imagens em {d}")),
            None => Err("uso: glove-arte icones <pasta>".into()),
        },
        _ => Err("uso: glove-arte plymouth <pasta> | glove-arte icones <pasta>".into()),
    };
    if let Err(e) = r {
        eprintln!("glove-arte: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn gera_o_tema() {
        let dir = std::env::temp_dir().join(format!("glove-arte-{}", std::process::id()));
        super::plymouth(&dir).unwrap();
        for f in ["logo.png", "spinner-0.png", "spinner-11.png", "glove.plymouth", "glove.script"] {
            assert!(dir.join(f).is_file(), "{f}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
