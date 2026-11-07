//! Painel Aplicativos Padrão: qual app abre cada tipo (xdg-mime, só para este
//! usuário). Os apps do Glove ganham um .desktop em ~/.local/share/applications
//! quando escolhidos, para o xdg-open saber abri-los.

use glove_config::tr;
use std::path::PathBuf;
use std::process::Command;

use glove_ui::Ui;

/// (tipos, nome da categoria, app do Glove que serve: (id do .desktop, nome, binário)).
const KINDS: [(&[&str], &str, Option<(&str, &str, &str)>); 7] = [
    (&["x-scheme-handler/http", "x-scheme-handler/https", "text/html"], "Navegador", None),
    (&["x-scheme-handler/mailto"], "E-mail", None),
    (&["text/plain"], "Texto", Some(("glove-edit.desktop", "Editor de Texto (Glove)", "glove-edit"))),
    (&["image/png", "image/jpeg"], "Imagens", Some(("glove-view.desktop", "Visualizador (Glove)", "glove-view"))),
    (&["application/pdf"], "PDF", None),
    (&["audio/mpeg", "audio/ogg", "audio/flac"], "Música", None),
    (&["video/mp4", "video/x-matroska", "video/webm"], "Vídeo", None),
];

fn apps_dirs() -> Vec<PathBuf> {
    let mut v = vec![std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))).unwrap_or_default().join("applications")];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    // Apps do Flatpak (às vezes fora do XDG_DATA_DIRS).
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h).join(".local/share/flatpak/exports/share/applications"));
    }
    v.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    v
}

/// Só o grupo [Desktop Entry] (as ações vêm depois, com "Name" próprios).
fn main_group(t: &str) -> &str {
    let start = t.find("[Desktop Entry]").unwrap_or(0);
    let rest = &t[start..];
    match rest[1..].find("\n[") {
        Some(i) => &rest[..i + 1],
        None => rest,
    }
}

/// Nome de um .desktop pelo id (para os padrões que não estão na lista).
fn desktop_name(id: &str) -> Option<String> {
    apps_dirs().into_iter().find_map(|d| {
        let full = std::fs::read_to_string(d.join(id)).ok()?;
        let t = main_group(&full);
        nome_local(t).or_else(|| t.lines().find_map(|l| l.strip_prefix("Name="))).map(|n| n.trim().to_string())
    })
}

/// Apps instalados que dizem abrir algum dos tipos: (id do .desktop, nome).
fn handlers(mimes: &[&str]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for d in apps_dirs() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id.ends_with(".desktop") || out.iter().any(|(i, _)| *i == id) {
                continue;
            }
            let Ok(full) = std::fs::read_to_string(e.path()) else { continue };
            let t = main_group(&full);
            let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(str::trim);
            if get("NoDisplay") == Some("true") {
                continue;
            }
            let types = get("MimeType").unwrap_or("");
            if mimes.iter().any(|m| types.split(';').any(|x| x == *m)) {
                let name = nome_local(&t).or(get("Name")).unwrap_or(&id).to_string();
                out.push((id, name));
            }
        }
    }
    out.sort_by_key(|(_, n)| n.to_lowercase());
    out
}

fn query(mime: &str) -> String {
    Command::new("xdg-mime").args(["query", "default", mime]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// Binário do Glove ao lado das Preferências (instalado ou do projeto).
fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

/// Garante o .desktop de um app do Glove (para o xdg-open achar).
fn ensure_glove_desktop(id: &str, name: &str, bin: &str, mimes: &[&str]) -> std::io::Result<()> {
    let dir = apps_dirs().remove(0);
    std::fs::create_dir_all(&dir)?;
    let exec = sibling(bin);
    std::fs::write(dir.join(id), format!("[Desktop Entry]\nType=Application\nName={name}\nExec={} %f\nMimeType={};\nNoDisplay=true\n", exec.display(), mimes.join(";")))
}

#[derive(Default)]
pub struct Padrao {
    /// Por categoria: (apps possíveis, escolhido).
    rows: Vec<(Vec<(String, String)>, usize)>,
    loaded: bool,
    message: Option<String>,
}

impl Padrao {
    pub fn refresh(&mut self) {
        self.rows = KINDS
            .iter()
            .map(|(mimes, _, glove)| {
                let mut opts = vec![(String::new(), tr!("— (nenhum escolhido)").to_string())];
                if let Some((id, name, _)) = glove {
                    opts.push((id.to_string(), tr(name).to_string()));
                }
                for h in handlers(mimes) {
                    if !opts.iter().any(|(i, _)| *i == h.0) {
                        opts.push(h);
                    }
                }
                let cur = query(mimes[0]);
                let sel = opts.iter().position(|(i, _)| *i == cur).unwrap_or_else(|| {
                    if cur.is_empty() {
                        0
                    } else {
                        let name = desktop_name(&cur).unwrap_or_else(|| cur.trim_end_matches(".desktop").to_string());
                        opts.push((cur.clone(), name));
                        opts.len() - 1
                    }
                });
                (opts, sel)
            })
            .collect();
        self.loaded = true;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.loaded {
            self.refresh();
        }
        ui.small(tr!("Escolha o app que abre cada tipo de arquivo ou link:"));
        ui.space(8.0);
        let mut set = None;
        for (k, (mimes, label, glove)) in KINDS.iter().enumerate() {
            let (opts, sel) = &mut self.rows[k];
            let names: Vec<String> = opts.iter().map(|(_, n)| n.clone()).collect();
            ui.push_id(label, |ui| {
                ui.form_row(tr(label), 120.0, |ui| {
                    if ui.popup(sel, &names, 300.0) && *sel > 0 {
                        set = Some((k, opts[*sel].0.clone(), *mimes, *glove));
                    }
                });
            });
        }
        if let Some((_, id, mimes, glove)) = set {
            if let Some((gid, name, bin)) = glove.filter(|g| g.0 == id) {
                if let Err(e) = ensure_glove_desktop(gid, name, bin, mimes) {
                    self.message = Some(tr!("Não foi possível preparar o app: {}", e));
                }
            }
            let mut args = vec!["default", id.as_str()];
            args.extend(mimes.iter());
            self.message = match Command::new("xdg-mime").args(&args).status() {
                Ok(s) if s.success() => Some(tr!("Pronto.").into()),
                _ => Some(tr!("Não foi possível mudar (o xdg-mime falhou).").into()),
            };
        }
        ui.space(8.0);
        ui.small(tr!("Vale para o Arquivos (clique duplo) e para os links abertos pelos apps."));
        if let Some(m) = &self.message {
            let m = m.clone();
            ui.small(&m);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn grupo_principal() {
        let t = "[Desktop Entry]\nName=Chrome\nExec=x\n\n[Desktop Action new-window]\nName=Nova janela\n";
        assert!(super::main_group(t).contains("Name=Chrome"));
        assert!(!super::main_group(t).contains("Nova janela"));
    }
}

/// O Name[…] do .desktop no idioma em uso (Name[pt_BR], Name[en]…), se houver.
fn nome_local(t: &str) -> Option<&str> {
    let lang = glove_config::idioma();
    let region = std::env::var("LANG").ok().and_then(|l| l.split('.').next().map(String::from)).unwrap_or_default();
    t.lines()
        .filter_map(|l| l.strip_prefix("Name[").and_then(|r| r.split_once("]=")))
        .find(|(loc, _)| *loc == region || *loc == lang)
        .map(|(_, v)| v)
}
