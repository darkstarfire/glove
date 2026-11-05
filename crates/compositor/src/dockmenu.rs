//! Menu dos ícones do Dock (botão direito ou clique segurado), com bico apontando
//! para o ícone: janelas do app, Manter no Dock, Mostrar no Arquivos, Ocultar,
//! Encerrar ou Abrir. E os apps abertos que não estão no Dock (aparecem depois
//! dos fixos, como no Mac, e podem ser mantidos).

use std::path::PathBuf;

use glove_paint::chrome::MenuItem;
use smithay::desktop::Window;

use crate::{
    shell::{app_display_name, window_info, BarTarget, DockAction, MenuAction, MenuNode},
    state::Glove,
};

/// Apps que não aparecem no Dock (painéis do próprio Glove).
const NOT_IN_DOCK: [&str; 2] = ["glove.About", "glove.Sair"];

impl Glove {
    /// Todas as janelas: na tela, minimizadas e ocultas.
    fn every_window(&self) -> Vec<Window> {
        self.space.elements().cloned().chain(self.minimized.iter().chain(&self.hidden).map(|(w, _)| w.clone())).collect()
    }

    /// Apps abertos que não estão fixos no Dock: (app_id, uma janela), na ordem em que abriram.
    pub fn running_unpinned(&self) -> Vec<(String, Window)> {
        let mut out: Vec<(String, Window)> = Vec::new();
        for w in self.every_window() {
            let id = window_info(&w).1;
            if id.is_empty() || NOT_IN_DOCK.contains(&id.as_str()) {
                continue;
            }
            if self.config.dock_apps.iter().any(|a| a.app_id.eq_ignore_ascii_case(&id)) || out.iter().any(|(o, _)| o.eq_ignore_ascii_case(&id)) {
                continue;
            }
            out.push((id, w));
        }
        out
    }

    /// pid de uma janela do app (pelo app_id).
    pub fn every_window_pid(&self, app_id: &str) -> Option<u32> {
        self.windows_of_app_id(app_id).first().and_then(|w| self.window_pid(w))
    }

    fn windows_of_app_id(&self, app_id: &str) -> Vec<Window> {
        self.every_window().into_iter().filter(|w| !app_id.is_empty() && window_info(w).1.eq_ignore_ascii_case(app_id)).collect()
    }

    /// Abre o menu do ícone `i` do Dock (se ele tiver um).
    pub fn open_dock_menu(&mut self, i: usize) {
        let Some(action) = self.chrome.dock_actions.get(i).cloned() else { return };
        let nodes = match &action {
            DockAction::App(idx) => {
                let Some(app) = self.config.dock_apps.get(*idx).cloned() else { return };
                self.app_menu_nodes(&app.app_id, true, Some(action.clone()), &app.command)
            }
            DockAction::Running(id) => {
                let command = self.windows_of_app_id(id).first().and_then(|w| self.window_pid(w)).and_then(command_of_pid).unwrap_or_default();
                self.app_menu_nodes(id, false, None, &command)
            }
            DockAction::Trash => vec![
                MenuNode::new(MenuItem::item("Abrir"), Some(MenuAction::DockRun(DockAction::Trash))),
                MenuNode::sep(),
                // Esvaziar com alerta chega na etapa 6.2.
                MenuNode::new(MenuItem::item("Esvaziar Lixeira…").disabled(), None),
            ],
            DockAction::Restore(_) => return,
        };
        // Bico apontando para o meio do ícone; o menu fica logo acima dele.
        let Some(l) = &self.chrome.dock_layout else { return };
        let Some(&(x, y, s)) = l.items.get(i) else { return };
        let d = self.chrome.dock_pos();
        if self.chrome.dock_side.vertical() {
            // Dock na lateral: o menu abre ao lado do ícone.
            let r = (d.x as f32 + x, d.y as f32 + y, s);
            self.show_dock_menu_side(i, nodes, r);
            return;
        }
        let center = d.x as f32 + x + s / 2.0;
        let top = d.y as f32 + y;
        self.show_dock_menu(i, nodes, center, top);
    }

    fn app_menu_nodes(&self, app_id: &str, pinned: bool, open: Option<DockAction>, command: &str) -> Vec<MenuNode> {
        let wins = self.windows_of_app_id(app_id);
        let focused = self.focused_window();
        let mut v = Vec::new();
        for w in &wins {
            let title = window_info(w).0;
            let title = if title.is_empty() { app_display_name(w) } else { title };
            let mut it = MenuItem::check(title, focused.as_ref() == Some(w));
            it.diamond = self.minimized.iter().any(|(m, _)| m == w);
            v.push(MenuNode::new(it, Some(MenuAction::FocusWindow(w.clone()))));
        }
        if !v.is_empty() {
            v.push(MenuNode::sep());
        }
        let pid = wins.first().and_then(|w| self.window_pid(w));
        v.push(MenuNode::new(MenuItem::check("Manter no Dock", pinned), Some(MenuAction::ToggleKeepInDock(app_id.to_string(), pid.unwrap_or(0)))));
        let show = MenuItem::item("Mostrar no Arquivos");
        let show = if command.is_empty() { show.disabled() } else { show };
        v.push(MenuNode::new(show, Some(MenuAction::ShowInFiles(command.to_string()))));
        v.push(MenuNode::sep());
        match (pid, open) {
            (Some(pid), _) => {
                v.push(MenuNode::new(MenuItem::item("Ocultar"), Some(MenuAction::HideApp(pid))));
                v.push(MenuNode::new(MenuItem::item("Encerrar"), Some(MenuAction::QuitApp(pid))));
            }
            (None, Some(a)) => v.push(MenuNode::new(MenuItem::item("Abrir"), Some(MenuAction::DockRun(a)))),
            (None, None) => {}
        }
        v
    }

    /// Manter no Dock: fixa (com o comando que abriu o app) ou tira.
    pub fn toggle_keep_in_dock(&mut self, app_id: &str, pid: u32) {
        if let Some(i) = self.config.dock_apps.iter().position(|a| a.app_id.eq_ignore_ascii_case(app_id)) {
            self.config.dock_apps.remove(i);
        } else {
            let Some(command) = command_of_pid(pid) else { return };
            let name = self.windows_of_app_id(app_id).first().map(app_display_name).unwrap_or_else(|| app_id.to_string());
            self.config.dock_apps.push(glove_config::DockApp { name, icon: "window".into(), command, app_id: app_id.to_string() });
        }
        let _ = self.config.save();
        self.dirty = true;
    }

    /// Mostrar no Arquivos: a pasta do programa do app.
    pub fn show_in_files(&mut self, command: &str) {
        let Some(first) = command.split_whitespace().next() else { return };
        let path = if first.starts_with("glove-") {
            PathBuf::from(crate::shell::sibling_binary(first))
        } else if first.contains('/') {
            PathBuf::from(first)
        } else {
            std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(first)).find(|p| p.is_file())).unwrap_or_default()
        };
        if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
            let _ = std::process::Command::new(crate::shell::sibling_binary("glove-files")).arg(dir).env("WAYLAND_DISPLAY", &self.socket_name).spawn();
        }
    }

    pub fn dock_menu_open(&self) -> bool {
        self.chrome.menu.as_ref().is_some_and(|m| matches!(m.target, BarTarget::Dock(_)))
    }
}

/// Linha de comando de um processo (para abrir de novo o app fixado).
fn command_of_pid(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let parts: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| {
            let s = String::from_utf8_lossy(p).into_owned();
            if s.chars().all(|c| c.is_alphanumeric() || "/._-=:,+@%".contains(c)) { s } else { format!("'{}'", s.replace('\'', "'\\''")) }
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(test)]
mod tests {
    #[test]
    fn linha_de_comando() {
        let me = super::command_of_pid(std::process::id()).unwrap();
        assert!(!me.is_empty());
    }
}
