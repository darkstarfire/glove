//! As folhas das operações do Discos: criar partição, formatar (volume e disco),
//! editar (tipo e nome), redimensionar. Cada folha devolve o que fazer quando o
//! usuário confirma; quem faz é a janela (numa linha à parte).

use glove_config::tr;
use glove_ui::{paint::draw::*, Rect, Ui};

/// Sistemas de arquivos oferecidos: (tipo no udisks, nome).
pub const FORMATS: [(&str, &str); 5] = [("ext4", "Ext4 (Linux)"), ("vfat", "FAT32"), ("exfat", "exFAT"), ("ntfs", "NTFS (Windows)"), ("btrfs", "Btrfs (Linux)")];

fn format_names() -> Vec<String> {
    vec![
        tr!("Ext4 — Linux (recomendado para o Glove OS)").into(),
        tr!("FAT32 — todos os sistemas (arquivos até 4 GB)").into(),
        tr!("exFAT — pendrives e cartões (Windows, Mac, Linux)").into(),
        tr!("NTFS — Windows").into(),
        tr!("Btrfs — Linux (instantâneos)").into(),
    ]
}

/// Tipos de partição para editar: (GPT, MBR, nome).
pub const TYPES: [(&str, &str, &str); 6] = [
    ("0fc63daf-8483-4772-8e79-3d69d8477de4", "0x83", "Linux"),
    ("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7", "0x07", "Windows / exFAT / NTFS"),
    ("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7", "0x0c", "FAT32"),
    ("c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "0xef", "Sistema EFI"),
    ("0657fd6d-a4ab-43c4-84e5-0933c84b4f4f", "0x82", "Linux (troca)"),
    ("21686148-6449-6e6f-744e-656564454649", "0x83", "Inicialização da BIOS (GPT)"),
];

/// O tipo de partição que combina com o sistema de arquivos.
pub fn ptype_for(fs: &str, gpt: bool) -> &'static str {
    match (fs, gpt) {
        ("ext4" | "btrfs", true) => TYPES[0].0,
        ("ext4" | "btrfs", false) => "0x83",
        (_, true) => TYPES[1].0,
        ("vfat", false) => "0x0c",
        (_, false) => "0x07",
    }
}

/// Tamanho digitado em GB ("12", "12,5", "12.5") para bytes.
pub fn parse_gb(t: &str) -> Option<u64> {
    let v: f64 = t.trim().replace(',', ".").trim_end_matches(|c: char| c.is_alphabetic() || c == ' ').parse().ok()?;
    (v > 0.0).then(|| (v * 1e9) as u64)
}

pub fn gb_text(n: u64) -> String {
    format!("{:.1}", n as f64 / 1e9).replace('.', glove_config::virgula())
}

pub enum Dialog {
    /// No espaço livre: tamanho, formato e nome.
    NewPart { offset: u64, max: u64, gpt: bool, table: String, size: String, fs: usize, name: String },
    /// Formatar um volume (apaga o conteúdo).
    Format { block: String, what: String, fs: usize, name: String },
    /// Formatar o disco inteiro: tabela nova (GPT ou MBR), opção de zerar.
    FormatDisk { block: String, what: String, scheme: usize, zero: bool },
    /// Tipo e nome da partição.
    Edit { block: String, gpt: bool, kind: usize, name: String },
    /// Novo tamanho, entre o mínimo e o máximo.
    Resize { block: String, fs: String, min: u64, max: u64, size: String },
}

/// O que fazer depois de confirmar.
pub enum Do {
    NewPart { table: String, offset: u64, size: u64, ptype: String, fs: String, name: String },
    Format { block: String, fs: String, name: String },
    FormatDisk { block: String, scheme: String, zero: bool },
    Edit { block: String, ptype: String, name: String },
    Resize { block: String, fs: String, size: u64 },
}

pub enum Answer {
    Waiting,
    Cancel,
    Go(Do),
}

/// Desenha a folha e trata OK/Cancelar/Esc.
pub fn sheet(ui: &mut Ui, d: &mut Dialog) -> Answer {
    let (sw, sh) = (470.0, 236.0);
    let x = ((ui.c.w - sw) / 2.0).round();
    let y = 0.0;
    ui.c.fill_rect(0.0, 0.0, ui.c.w, ui.c.h, &solid(rgba(0, 0, 0, 30)));
    ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
    ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
    ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
    let ink = rgba(20, 20, 20, 255);
    let warn = rgba(170, 30, 30, 255);
    let lw = 110.0;
    let mut ok_label = tr!("OK").to_string();
    let mut valid = true;
    let mut go: Option<Do> = None;
    ui.area(Rect::new(x + 20.0, y + 14.0, sw - 40.0, sh - 60.0), |ui| match d {
        Dialog::NewPart { offset, max, gpt, table, size, fs, name } => {
            ui.fonts.draw(ui.c, tr!("Nova partição"), x + 20.0, y + 30.0, 13.0, true, ink);
            ui.space(28.0);
            ui.form_row(tr!("Tamanho (GB):"), lw, |ui| {
                ui.text_field(size, 120.0);
                ui.small(&tr!("até {}", gb_text(*max)));
            });
            ui.form_row(tr!("Formato:"), lw, |ui| {
                ui.popup(fs, &format_names(), 300.0);
            });
            ui.form_row(tr!("Nome:"), lw, |ui| {
                ui.text_field(name, 200.0);
            });
            let bytes = parse_gb(size).map(|b| b.min(*max));
            valid = bytes.is_some();
            ok_label = tr!("Criar").into();
            if let Some(b) = bytes {
                let f = FORMATS[*fs].0;
                // O tamanho sugerido (todo o espaço) ou quase tudo: o espaço inteiro.
                let b = if size.trim() == gb_text(*max) || *max - b < 1_000_000 { *max } else { b };
                go = Some(Do::NewPart { table: table.clone(), offset: *offset, size: b, ptype: ptype_for(f, *gpt).into(), fs: f.into(), name: name.clone() });
            }
        }
        Dialog::Format { block, what, fs, name } => {
            ui.fonts.draw(ui.c, &tr!("Formatar “{}”", what), x + 20.0, y + 30.0, 13.0, true, ink);
            ui.space(28.0);
            ui.form_row(tr!("Formato:"), lw, |ui| {
                ui.popup(fs, &format_names(), 300.0);
            });
            ui.form_row(tr!("Nome:"), lw, |ui| {
                ui.text_field(name, 200.0);
            });
            ui.space(6.0);
            ui.fonts.draw(ui.c, tr!("Tudo o que está neste volume será apagado."), x + 20.0, y + 140.0, 11.5, true, warn);
            ok_label = tr!("Formatar").into();
            go = Some(Do::Format { block: block.clone(), fs: FORMATS[*fs].0.into(), name: name.clone() });
        }
        Dialog::FormatDisk { block, what, scheme, zero } => {
            ui.fonts.draw(ui.c, &tr!("Formatar o disco “{}”", what), x + 20.0, y + 30.0, 13.0, true, ink);
            ui.space(28.0);
            let schemes = vec![tr!("GPT — computadores com UEFI (recomendado)").to_string(), tr!("MBR — computadores antigos e aparelhos (TV, som)").to_string(), tr!("Nenhuma — o disco vazio, sem partições").to_string()];
            ui.form_row(tr!("Partições:"), lw, |ui| {
                ui.popup(scheme, &schemes, 300.0);
            });
            ui.form_row("", lw, |ui| {
                ui.checkbox(zero, tr!("Apagar com zeros (lento, sem volta)"));
            });
            ui.fonts.draw(ui.c, tr!("Todas as partições e arquivos deste disco serão apagados."), x + 20.0, y + 140.0, 11.5, true, warn);
            ok_label = tr!("Formatar").into();
            go = Some(Do::FormatDisk { block: block.clone(), scheme: ["gpt", "dos", "empty"][*scheme].into(), zero: *zero });
        }
        Dialog::Edit { block, gpt: show_name, kind, name } => {
            ui.fonts.draw(ui.c, tr!("Editar partição"), x + 20.0, y + 30.0, 13.0, true, ink);
            ui.space(28.0);
            let names: Vec<String> = TYPES.iter().map(|t| glove_config::tr_dyn(t.2)).collect();
            ui.form_row(tr!("Tipo:"), lw, |ui| {
                ui.popup(kind, &names, 300.0);
            });
            if *show_name {
                ui.form_row(tr!("Nome:"), lw, |ui| {
                    ui.text_field(name, 200.0);
                });
            }
            ui.paragraph(tr!("O tipo só avisa os sistemas o que a partição guarda; os arquivos não mudam."));
            ok_label = tr!("Mudar").into();
            let t = TYPES[*kind];
            // O tipo vai no formato da tabela (o udisks diz qual é pelo próprio tipo atual).
            go = Some(Do::Edit { block: block.clone(), ptype: format!("{}|{}", t.0, t.1), name: name.clone() });
        }
        Dialog::Resize { block, fs, min, max, size } => {
            ui.fonts.draw(ui.c, tr!("Redimensionar"), x + 20.0, y + 30.0, 13.0, true, ink);
            ui.space(28.0);
            ui.form_row(tr!("Tamanho (GB):"), lw, |ui| {
                ui.text_field(size, 120.0);
                ui.small(&tr!("de {} a {}", gb_text(*min), gb_text(*max)));
            });
            ui.paragraph(tr!("Os arquivos ficam. Para diminuir, o volume é desmontado e conferido antes; faça uma cópia do que for importante."));
            let b = if size.trim() == gb_text(*max) { Some(*max) } else { parse_gb(size).filter(|b| *b >= *min && *b <= *max + 50_000_000).map(|b| b.min(*max)) };
            valid = b.is_some();
            ok_label = tr!("Redimensionar").into();
            if let Some(b) = b {
                go = Some(Do::Resize { block: block.clone(), fs: fs.clone(), size: b });
            }
        }
    });
    let (mut cancel, mut ok) = (false, false);
    ui.area(Rect::new(x + sw - 250.0, y + sh - 40.0, 240.0, 30.0), |ui| {
        ui.horizontal(|ui| {
            cancel = ui.button(tr!("Cancelar"));
            ui.set_active(valid);
            ok = ui.default_button(&ok_label);
            ui.set_active(true);
        });
    });
    if cancel || ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
        return Answer::Cancel;
    }
    if ok && valid {
        if let Some(g) = go {
            return Answer::Go(g);
        }
    }
    Answer::Waiting
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanho_digitado() {
        assert_eq!(parse_gb("12"), Some(12_000_000_000));
        assert_eq!(parse_gb("12,5"), Some(12_500_000_000));
        assert_eq!(parse_gb(" 0.5 GB"), Some(500_000_000));
        assert_eq!(parse_gb("0"), None);
        assert_eq!(parse_gb("abc"), None);
        assert_eq!(ptype_for("vfat", false), "0x0c");
        assert_eq!(ptype_for("ext4", true), TYPES[0].0);
    }
}
