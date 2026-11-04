//! Menus do app, mostrados pelo Glove na barra de menu global.

#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem {
    Item { id: String, label: String, shortcut: String, enabled: bool, checked: bool },
    Separator,
    /// Item com ▸ que abre outro menu.
    Submenu { label: String, items: Vec<MenuItem>, enabled: bool },
}

impl MenuItem {
    /// Item comum. Se tiver atalho (ex.: "Ctrl+S"), escolher o item equivale a
    /// pressionar o atalho: o app trata os dois casos no mesmo lugar.
    pub fn new(id: &str, label: &str, shortcut: &str) -> Self {
        MenuItem::Item { id: id.into(), label: label.into(), shortcut: shortcut.into(), enabled: true, checked: false }
    }
    /// Submenu (ex.: "Abrir Recente ▸").
    pub fn submenu(label: &str, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu { label: label.into(), items, enabled: true }
    }
    pub fn enabled(mut self, on: bool) -> Self {
        if let MenuItem::Item { enabled, .. } | MenuItem::Submenu { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }
    /// Este item e os dos submenus, em ordem.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a MenuItem>) {
        out.push(self);
        if let MenuItem::Submenu { items, .. } = self {
            for it in items {
                it.walk(out);
            }
        }
    }
    pub fn checked(mut self, on: bool) -> Self {
        if let MenuItem::Item { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub title: String,
    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn new(title: &str, items: Vec<MenuItem>) -> Self {
        Self { title: title.into(), items }
    }

    /// Itens que vão no menu com o nome do app (ex.: "Sobre…", "Preferências…"),
    /// acima de Ocultar/Encerrar, que o Glove põe sozinho.
    pub fn app(items: Vec<MenuItem>) -> Self {
        Self::new("@app", items)
    }

    /// Itens a mais no menu Janela (o Glove já põe Minimizar, Zoom e a lista de janelas).
    pub fn window(items: Vec<MenuItem>) -> Self {
        Self::new("Janela", items)
    }
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n'], " ")
}

fn serialize_items(items: &[MenuItem], out: &mut String) {
    for it in items {
        match it {
            MenuItem::Separator => out.push_str("SEP\n"),
            MenuItem::Item { id, label, shortcut, enabled, checked } => {
                let flags = format!("{}{}", if *enabled { "" } else { "d" }, if *checked { "c" } else { "" });
                out.push_str(&format!("ITEM\t{}\t{}\t{}\t{}\n", clean(id), clean(label), clean(shortcut), flags));
            }
            MenuItem::Submenu { label, items, enabled } => {
                out.push_str(&format!("SUB\t{}\t{}\n", clean(label), if *enabled { "" } else { "d" }));
                serialize_items(items, out);
                out.push_str("ENDSUB\n");
            }
        }
    }
}

/// Texto do protocolo enviado ao compositor.
pub(crate) fn serialize(menus: &[Menu]) -> String {
    let mut out = String::from("BEGIN\n");
    for m in menus {
        out.push_str(&format!("MENU\t{}\n", clean(&m.title)));
        serialize_items(&m.items, &mut out);
    }
    out.push_str("END\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submenu_no_protocolo() {
        let m = Menu::new("Arquivo", vec![
            MenuItem::new("novo", "Novo", "Ctrl+N"),
            MenuItem::submenu("Abrir Recente", vec![MenuItem::new("r1", "a.txt", "")]),
            MenuItem::Separator,
        ]);
        assert_eq!(
            serialize(&[m]),
            "BEGIN\nMENU\tArquivo\nITEM\tnovo\tNovo\tCtrl+N\t\nSUB\tAbrir Recente\t\nITEM\tr1\ta.txt\t\t\nENDSUB\nSEP\nEND\n"
        );
    }
}
