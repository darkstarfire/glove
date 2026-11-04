//! Menus do app, mostrados pelo Glove na barra de menu global.

#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem {
    Item { id: String, label: String, shortcut: String, enabled: bool, checked: bool },
    Separator,
}

impl MenuItem {
    /// Item comum. Se tiver atalho (ex.: "Ctrl+S"), escolher o item equivale a
    /// pressionar o atalho: o app trata os dois casos no mesmo lugar.
    pub fn new(id: &str, label: &str, shortcut: &str) -> Self {
        MenuItem::Item { id: id.into(), label: label.into(), shortcut: shortcut.into(), enabled: true, checked: false }
    }
    pub fn enabled(mut self, on: bool) -> Self {
        if let MenuItem::Item { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
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
}

/// Texto do protocolo enviado ao compositor.
pub(crate) fn serialize(menus: &[Menu]) -> String {
    let clean = |s: &str| s.replace(['\t', '\n'], " ");
    let mut out = String::from("BEGIN\n");
    for m in menus {
        out.push_str(&format!("MENU\t{}\n", clean(&m.title)));
        for it in &m.items {
            match it {
                MenuItem::Separator => out.push_str("SEP\n"),
                MenuItem::Item { id, label, shortcut, enabled, checked } => {
                    let flags = format!("{}{}", if *enabled { "" } else { "d" }, if *checked { "c" } else { "" });
                    out.push_str(&format!("ITEM\t{}\t{}\t{}\t{}\n", clean(id), clean(label), clean(shortcut), flags));
                }
            }
        }
    }
    out.push_str("END\n");
    out
}
