//! Menu do app (o do nome em negrito) e menu Janela, montados pelo Glove para
//! qualquer app, e o "Ocultar" (janelas escondidas, que não vão para o Dock).

use glove_paint::chrome::MenuItem;
use smithay::{desktop::Window, reexports::wayland_server::Resource};

use crate::{
    menus::{AppMenu, Entry},
    shell::{client_nodes, fixed_size, window_info, window_surface, BarTarget, MenuAction, MenuNode},
    state::Glove,
};

/// Título do menu em que o app manda os itens do menu do app (Sobre, Preferências…).
pub const APP_MENU: &str = "@app";
pub const WINDOW_MENU: &str = "Janela";

/// Um menu da barra depois do nome do app.
pub enum BarMenu {
    Client(AppMenu),
    /// Menu Janela do Glove, com os itens que o app tiver posto no "Janela" dele.
    Window(Vec<Entry>),
}

impl Glove {
    pub fn window_pid(&self, w: &Window) -> Option<u32> {
        let s = window_surface(w)?;
        let client = self.display_handle.get_client(s.id()).ok()?;
        client.get_credentials(&self.display_handle).ok().map(|c| c.pid as u32)
    }

    /// Menus da barra depois do nome do app: os do app e, por último, Janela.
    pub fn bar_menus(&self) -> Vec<BarMenu> {
        let mut out = Vec::new();
        let mut extra = Vec::new();
        for m in self.focused_menus() {
            if m.title == APP_MENU {
                continue;
            }
            if m.title == WINDOW_MENU {
                extra = m.items;
                continue;
            }
            out.push(BarMenu::Client(m));
        }
        if self.focused_window().is_some() {
            out.push(BarMenu::Window(extra));
        }
        out
    }

    /// Menu com o nome do app.
    pub fn open_app_menu(&mut self) {
        let focused = self.focused_window();
        let app = self.focused_app_name();
        let pid = self.focused_pid().unwrap_or(0);
        let app_id = focused.as_ref().map(|w| window_info(w).1).unwrap_or_default();
        let mut nodes = Vec::new();

        // Itens do próprio app (Sobre, Preferências…) ou o "Sobre" genérico dos apps do Glove.
        let own = self.focused_menus().into_iter().find(|m| m.title == APP_MENU).map(|m| m.items).unwrap_or_default();
        let own_about = own.iter().any(|e| e.id == "about");
        if !own_about && app_id.starts_with("glove.") && !matches!(app_id.as_str(), "glove.About" | "glove.Sair" | "glove.Navegador") {
            nodes.push(MenuNode::new(MenuItem::item(format!("Sobre {}", with_article(&app))), Some(MenuAction::AboutApp(app_id.clone(), app.clone()))));
        }
        if !own.is_empty() {
            if !nodes.is_empty() {
                nodes.push(MenuNode::sep());
            }
            nodes.extend(client_nodes(pid, own));
        }
        if !nodes.is_empty() {
            nodes.push(MenuNode::sep());
        }

        let has_window = focused.is_some();
        let others = self.space.elements().any(|w| self.window_pid(w) != Some(pid)) && has_window;
        let enable = |it: MenuItem, on: bool| if on { it } else { it.disabled() };
        nodes.push(MenuNode::new(enable(MenuItem::item(format!("Ocultar {app}")).with_shortcut(self.binding("hide")), has_window), Some(MenuAction::HideApp(pid))));
        nodes.push(MenuNode::new(enable(MenuItem::item("Ocultar Outros").with_shortcut(self.binding("hide_others")), others), Some(MenuAction::HideOthers(pid))));
        nodes.push(MenuNode::new(enable(MenuItem::item("Mostrar Todos"), !self.hidden.is_empty()), Some(MenuAction::ShowAll)));
        nodes.push(MenuNode::sep());
        nodes.push(MenuNode::new(enable(MenuItem::item(format!("Encerrar {app}")), has_window), Some(MenuAction::QuitApp(pid))));
        self.show_menu(BarTarget::Title(0), nodes);
    }

    /// Menu Janela: Minimizar, Zoom, Trazer Todas para a Frente e as janelas do app.
    pub fn window_menu_nodes(&self, extra: Vec<Entry>) -> Vec<MenuNode> {
        let focused = self.focused_window();
        let pid = self.focused_pid().unwrap_or(0);
        let enable = |it: MenuItem, on: bool| if on { it } else { it.disabled() };
        let mut nodes = vec![
            MenuNode::new(enable(MenuItem::item("Minimizar").with_shortcut(self.binding("minimize")), focused.is_some()), focused.clone().map(MenuAction::MinimizeWindow)),
            MenuNode::new(enable(MenuItem::item("Zoom"), focused.as_ref().is_some_and(|w| !fixed_size(w))), focused.clone().map(MenuAction::ZoomWindow)),
            MenuNode::sep(),
            MenuNode::new(enable(MenuItem::item("Trazer Todas para a Frente"), focused.is_some()), Some(MenuAction::BringAllToFront(pid))),
        ];
        if !extra.is_empty() {
            nodes.push(MenuNode::sep());
            nodes.extend(client_nodes(pid, extra));
        }
        // Janelas do app: ✓ na da frente, ◆ nas minimizadas.
        let app_name = self.focused_app_name();
        let title_of = |w: &Window| {
            let t = window_info(w).0;
            if t.is_empty() { app_name.clone() } else { t }
        };
        let mut list: Vec<MenuNode> = self
            .windows_of_pid(pid)
            .into_iter()
            .map(|w| MenuNode::new(MenuItem::check(title_of(&w), focused.as_ref() == Some(&w)), Some(MenuAction::FocusWindow(w))))
            .collect();
        for (w, _) in &self.minimized {
            if self.window_pid(w) == Some(pid) {
                let mut it = MenuItem::item(title_of(w));
                it.diamond = true;
                list.push(MenuNode::new(it, Some(MenuAction::FocusWindow(w.clone()))));
            }
        }
        if !list.is_empty() {
            nodes.push(MenuNode::sep());
            nodes.extend(list);
        }
        nodes
    }

    /// Esconde as janelas do processo `pid` (sem ir para o Dock).
    pub fn hide_pid(&mut self, pid: u32) {
        for w in self.windows_of_pid(pid) {
            if let Some(loc) = self.space.element_location(&w) {
                self.space.unmap_elem(&w);
                self.hidden.push((w, loc));
            }
        }
        self.focus_topmost(None);
        self.dirty = true;
    }

    pub fn hide_others(&mut self, pid: u32) {
        let others: Vec<u32> = self.space.elements().filter_map(|w| self.window_pid(w)).filter(|p| *p != pid).collect();
        let mut seen = Vec::new();
        for p in others {
            if !seen.contains(&p) {
                seen.push(p);
                self.hide_pid(p);
            }
        }
        if let Some(w) = self.windows_of_pid(pid).pop() {
            self.focus_window(Some(w));
        }
    }

    /// Mostra as janelas escondidas que satisfazem `which` (na ordem em que estavam).
    pub fn unhide_where(&mut self, which: impl Fn(&Window) -> bool) -> Option<Window> {
        let (back, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.hidden).into_iter().partition(|(w, _)| which(w));
        self.hidden = keep;
        let last = back.last().map(|(w, _)| w.clone());
        for (w, loc) in back {
            self.space.map_element(w, loc, false);
        }
        if let Some(w) = &last {
            self.focus_window(Some(w.clone()));
        }
        self.dirty = true;
        last
    }

    /// Janela escolhida no menu Janela: traz para frente (ou volta do Dock).
    pub fn bring_window(&mut self, w: Window) {
        if let Some(i) = self.minimized.iter().position(|(m, _)| *m == w) {
            self.restore_minimized(i);
        } else if self.hidden.iter().any(|(h, _)| *h == w) {
            self.unhide_where(|h| *h == w);
        } else {
            self.focus_window(Some(w));
        }
    }

    /// Todas as janelas do app à frente das outras, mantendo a ordem entre elas.
    pub fn bring_all_to_front(&mut self, pid: u32) {
        let mine = self.windows_of_pid(pid);
        let focused = self.focused_window();
        for w in &mine {
            if Some(w) != focused.as_ref() {
                self.space.raise_element(w, false);
            }
        }
        if let Some(f) = focused {
            self.space.raise_element(&f, true);
        }
        self.dirty = true;
    }
}

/// "Calculadora" → "a Calculadora", "Terminal" → "o Terminal" (para "Sobre …").
fn with_article(name: &str) -> String {
    let first = name.split_whitespace().next().unwrap_or(name);
    let article = if first.ends_with("as") || first.ends_with("ências") {
        "as"
    } else if first.ends_with('s') {
        "os"
    } else if first.ends_with('a') {
        "a"
    } else {
        "o"
    };
    format!("{article} {name}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn artigos() {
        assert_eq!(super::with_article("Calculadora"), "a Calculadora");
        assert_eq!(super::with_article("Terminal"), "o Terminal");
        assert_eq!(super::with_article("Lembretes"), "os Lembretes");
        assert_eq!(super::with_article("Preferências"), "as Preferências");
        assert_eq!(super::with_article("Editor de Texto"), "o Editor de Texto");
        assert_eq!(super::with_article("Xadrez"), "o Xadrez");
    }
}
