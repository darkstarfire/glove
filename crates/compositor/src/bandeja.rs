//! Bandeja de ícones na barra de menu (padrão StatusNotifierItem do freedesktop/KDE):
//! o Glove atende `org.kde.StatusNotifierWatcher` no D-Bus da sessão, e os apps que põem
//! ícone na bandeja (Telegram, Discord, Steam, Nextcloud…) aparecem como extras à esquerda
//! dos outros. Sem esse serviço o app acha que não há bandeja e, ao fechar a janela,
//! encerra em vez de continuar rodando.
//!
//! Clique: o app abre (`Activate`); com o botão direito (ou num app que só tem menu), o
//! menu dele (`com.canonical.dbusmenu`) abre como um menu do Glove.
//!
//! Linhas de execução: a da bandeja (lê os ícones e fala com os apps), uma que escuta os
//! sinais dos ícones (ícone novo, título…) e uma que vê os apps saírem do D-Bus. O
//! compositor recebe a lista pronta por um canal do calloop.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use glove_paint::chrome::TrayIcon;
use smithay::reexports::calloop::channel::{channel, Event as ChannelEvent, Sender};
use zbus::zvariant::{OwnedValue, Value};

use crate::shell::{BarTarget, MenuAction, MenuNode};
use crate::state::Glove;
use glove_paint::chrome::MenuItem;

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";

/// Um ícone da bandeja, como o compositor o recebe.
#[derive(Clone, Debug)]
pub struct ItemInfo {
    pub key: String,
    pub title: String,
    /// "Active", "Passive" (escondido) ou "NeedsAttention".
    pub status: String,
    pub icon_name: String,
    pub theme_path: String,
    /// IconPixmap já em RGBA pré-multiplicado (o maior tamanho que o app mandou).
    pub pixmap: Option<(u32, u32, Vec<u8>)>,
    pub item_is_menu: bool,
    pub has_menu: bool,
}

/// Um item do menu do app (dbusmenu), já lido.
#[derive(Clone, Debug)]
pub struct No {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub checked: bool,
    pub children: Vec<No>,
}

pub enum Msg {
    Itens(Vec<ItemInfo>),
    Menu { key: String, nos: Vec<No> },
}

/// Pedidos para a linha da bandeja.
pub enum Pedido {
    Registrar { service: String, sender: String },
    Mudou { sender: String },
    Sumiu { name: String },
    Ativar { key: String, x: i32, y: i32 },
    Menu { key: String },
    Evento { key: String, id: i32 },
}

#[derive(Default)]
pub struct Bandeja {
    pub itens: Vec<ItemInfo>,
    /// A imagem pronta de cada ícone (pela chave), refeita quando o ícone muda.
    icons: HashMap<String, (u64, Option<TrayIcon>)>,
    tx: Option<mpsc::Sender<Pedido>>,
    /// Menu pedido e ainda não chegado: (chave do ícone, posição na barra).
    menu_pedido: Option<(String, usize)>,
}

impl Bandeja {
    /// Os ícones que aparecem na barra (os "Passive" ficam escondidos).
    pub fn visiveis(&self) -> Vec<usize> {
        (0..self.itens.len()).filter(|&i| self.itens[i].status != "Passive").collect()
    }
    pub fn icon(&self, i: usize) -> Option<TrayIcon> {
        self.itens.get(i).and_then(|it| self.icons.get(&it.key)).and_then(|(_, t)| t.clone())
    }
}

struct Watcher {
    tx: mpsc::Sender<Pedido>,
    lista: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: String, #[zbus(header)] hdr: zbus::message::Header<'_>) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let _ = self.tx.send(Pedido::Registrar { service, sender });
    }

    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.lista.lock().map(|l| l.clone()).unwrap_or_default()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Um ícone registrado, do lado da linha da bandeja.
struct Item {
    bus: String,
    path: String,
    /// O nome único de quem registrou (os sinais e a saída do app vêm dele).
    sender: String,
    menu: Option<String>,
    info: ItemInfo,
}

/// ARGB de 32 bits em ordem de rede (o IconPixmap) para RGBA pré-multiplicado.
fn argb_para_rgba(w: i32, h: i32, data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h) = (u32::try_from(w).ok()?, u32::try_from(h).ok()?);
    if w == 0 || h == 0 || data.len() < (w * h * 4) as usize {
        return None;
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for p in data.chunks_exact(4).take((w * h) as usize) {
        let (a, r, g, b) = (p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32);
        let m = |c: u32| ((c * a + 127) / 255) as u8;
        out.extend_from_slice(&[m(r), m(g), m(b), a as u8]);
    }
    Some((w, h, out))
}

/// O valor dentro de uma variante (os dois jeitos que chegam).
fn dentro<'a>(v: &'a Value<'a>) -> &'a Value<'a> {
    match v {
        Value::Value(inner) => inner,
        _ => v,
    }
}

fn texto(p: &HashMap<String, OwnedValue>, k: &str) -> String {
    p.get(k).and_then(|v| match dentro(v) {
        Value::Str(s) => Some(s.to_string()),
        Value::ObjectPath(s) => Some(s.to_string()),
        _ => None,
    })
    .unwrap_or_default()
}

fn sim(p: &HashMap<String, OwnedValue>, k: &str) -> Option<bool> {
    p.get(k).and_then(|v| match dentro(v) {
        Value::Bool(b) => Some(*b),
        _ => None,
    })
}

/// O maior IconPixmap (a barra reduz).
fn pixmap(p: &HashMap<String, OwnedValue>) -> Option<(u32, u32, Vec<u8>)> {
    let Value::Array(a) = dentro(p.get("IconPixmap")?) else { return None };
    let mut melhor: Option<(u32, u32, Vec<u8>)> = None;
    for v in a.inner() {
        let Value::Structure(s) = dentro(v) else { continue };
        let f = s.fields();
        let (Some(Value::I32(w)), Some(Value::I32(h)), Some(Value::Array(bytes))) = (f.first(), f.get(1), f.get(2)) else { continue };
        let data: Vec<u8> = bytes.inner().iter().filter_map(|b| if let Value::U8(x) = b { Some(*x) } else { None }).collect();
        if let Some(img) = argb_para_rgba(*w, *h, &data) {
            if melhor.as_ref().is_none_or(|m| img.0 * img.1 > m.0 * m.1) {
                melhor = Some(img);
            }
        }
    }
    melhor
}

/// Um item do menu do app e os filhos dele (o formato `(ia{sv}av)` do dbusmenu).
fn no_do_menu(v: &Value) -> Option<No> {
    let Value::Structure(s) = dentro(v) else { return None };
    let f = s.fields();
    let Some(Value::I32(id)) = f.first() else { return None };
    let mut props: HashMap<String, OwnedValue> = HashMap::new();
    if let Some(Value::Dict(d)) = f.get(1) {
        for (k, val) in d.iter() {
            if let (Value::Str(k), Ok(o)) = (k, val.try_to_owned()) {
                props.insert(k.to_string(), o);
            }
        }
    }
    if sim(&props, "visible") == Some(false) {
        return None;
    }
    let mut children = Vec::new();
    if let Some(Value::Array(a)) = f.get(2) {
        children.extend(a.inner().iter().filter_map(no_do_menu));
    }
    Some(No {
        id: *id,
        label: sem_mnemonico(&texto(&props, "label")),
        enabled: sim(&props, "enabled").unwrap_or(true),
        separator: texto(&props, "type") == "separator",
        checked: matches!(props.get("toggle-state").map(|v| dentro(v)), Some(Value::I32(1))),
        children,
    })
}

/// "_Abrir" → "Abrir" (o sublinhado marca a tecla de atalho; "__" é um sublinhado).
fn sem_mnemonico(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '_' {
            if it.peek() == Some(&'_') {
                out.push('_');
                it.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A linha da bandeja: registra os ícones, lê as propriedades e fala com os apps.
fn linha(conn: zbus::blocking::Connection, rx: mpsc::Receiver<Pedido>, lista: Arc<Mutex<Vec<String>>>, out: Sender<Msg>) {
    let mut itens: Vec<(String, Item)> = Vec::new();
    let ler = |bus: &str, path: &str| -> Option<HashMap<String, OwnedValue>> {
        let r = conn.call_method(Some(bus), path, Some("org.freedesktop.DBus.Properties"), "GetAll", &(ITEM,)).ok()?;
        r.body().deserialize::<HashMap<String, OwnedValue>>().ok()
    };
    let enviar = |itens: &Vec<(String, Item)>| {
        let _ = out.send(Msg::Itens(itens.iter().map(|(_, i)| i.info.clone()).collect()));
    };
    let atualizar_lista = |itens: &Vec<(String, Item)>| {
        if let Ok(mut l) = lista.lock() {
            *l = itens.iter().map(|(k, _)| k.clone()).collect();
        }
    };
    let completar = |key: &str, it: &mut Item, p: &HashMap<String, OwnedValue>| {
        let menu = texto(p, "Menu");
        it.menu = (!menu.is_empty() && menu != "/").then_some(menu);
        let mut title = texto(p, "Title");
        if title.is_empty() {
            title = texto(p, "Id");
        }
        it.info = ItemInfo {
            key: key.to_string(),
            title,
            status: texto(p, "Status"),
            icon_name: texto(p, "IconName"),
            theme_path: texto(p, "IconThemePath"),
            pixmap: pixmap(p),
            // Os apps da appindicator (Discord, Steam, nm-applet…) não dizem: com menu, o
            // clique abre o menu (o "abrir" deles mostra o menu GTK como janela solta).
            item_is_menu: sim(p, "ItemIsMenu").unwrap_or(it.menu.is_some()),
            has_menu: it.menu.is_some(),
        };
    };
    while let Ok(p) = rx.recv() {
        match p {
            Pedido::Registrar { service, sender } => {
                // O app manda o nome no barramento (o caminho é o padrão) ou só o caminho.
                let (bus, path) = if service.starts_with('/') { (sender.clone(), service) } else { (service, "/StatusNotifierItem".to_string()) };
                let key = format!("{bus}{path}");
                let Some(props) = ler(&bus, &path) else {
                    tracing::info!("bandeja: não consegui ler {key}");
                    continue;
                };
                let mut it = Item { bus, path, sender, menu: None, info: ItemInfo { key: key.clone(), title: String::new(), status: String::new(), icon_name: String::new(), theme_path: String::new(), pixmap: None, item_is_menu: false, has_menu: false } };
                completar(&key, &mut it, &props);
                tracing::info!("bandeja: {} ({key})", it.info.title);
                itens.retain(|(k, _)| *k != key);
                itens.push((key.clone(), it));
                atualizar_lista(&itens);
                let _ = conn.emit_signal(None::<&str>, WATCHER_PATH, WATCHER, "StatusNotifierItemRegistered", &(key.as_str(),));
                enviar(&itens);
            }
            Pedido::Mudou { sender } => {
                let mut mudou = false;
                for (key, it) in itens.iter_mut().filter(|(_, i)| i.sender == sender) {
                    if let Some(props) = ler(&it.bus, &it.path) {
                        completar(key, it, &props);
                        mudou = true;
                    }
                }
                if mudou {
                    enviar(&itens);
                }
            }
            Pedido::Sumiu { name } => {
                let antes = itens.len();
                let saem: Vec<String> = itens.iter().filter(|(_, i)| i.sender == name || i.bus == name).map(|(k, _)| k.clone()).collect();
                itens.retain(|(k, _)| !saem.contains(k));
                if itens.len() != antes {
                    atualizar_lista(&itens);
                    for k in &saem {
                        let _ = conn.emit_signal(None::<&str>, WATCHER_PATH, WATCHER, "StatusNotifierItemUnregistered", &(k.as_str(),));
                    }
                    enviar(&itens);
                }
            }
            Pedido::Ativar { key, x, y } => {
                if let Some((_, it)) = itens.iter().find(|(k, _)| *k == key) {
                    let _ = conn.call_method(Some(it.bus.as_str()), it.path.as_str(), Some(ITEM), "Activate", &(x, y));
                }
            }
            Pedido::Menu { key } => {
                let Some((_, it)) = itens.iter().find(|(k, _)| *k == key) else { continue };
                let Some(menu) = it.menu.clone() else { continue };
                // Alguns apps só montam o menu quando ele vai abrir.
                let _ = conn.call_method(Some(it.bus.as_str()), menu.as_str(), Some(MENU), "AboutToShow", &(0i32,));
                let nos = conn
                    .call_method(Some(it.bus.as_str()), menu.as_str(), Some(MENU), "GetLayout", &(0i32, -1i32, Vec::<&str>::new()))
                    .ok()
                    .and_then(|r| r.body().deserialize::<(u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>))>().ok())
                    .map(|(_, (_, _, filhos))| filhos.iter().filter_map(|v| no_do_menu(v)).collect::<Vec<_>>())
                    .unwrap_or_default();
                let _ = out.send(Msg::Menu { key, nos });
            }
            Pedido::Evento { key, id } => {
                let Some((_, it)) = itens.iter().find(|(k, _)| *k == key) else { continue };
                let Some(menu) = it.menu.as_deref() else { continue };
                let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0);
                let _ = conn.call_method(Some(it.bus.as_str()), menu, Some(MENU), "Event", &(id, "clicked", Value::from(0i32), agora));
            }
        }
    }
}

impl Glove {
    /// Liga a bandeja (se outro programa já atende, como o painel do KDE ao rodar dentro
    /// dele, o Glove não mostra a bandeja).
    pub fn start_bandeja(&mut self) {
        let (tx_out, rx_out) = channel::<Msg>();
        let _ = self.loop_handle.insert_source(rx_out, |ev, _, data| {
            if let ChannelEvent::Msg(m) = ev {
                data.state.bandeja_msg(m);
            }
        });
        let (tx, rx) = mpsc::channel::<Pedido>();
        self.bandeja.tx = Some(tx.clone());
        std::thread::spawn(move || {
            let lista = Arc::new(Mutex::new(Vec::new()));
            let w = Watcher { tx: tx.clone(), lista: lista.clone() };
            let built = zbus::blocking::connection::Builder::session()
                .and_then(|b| b.method_timeout(Duration::from_secs(3)).name(WATCHER))
                .and_then(|b| b.serve_at(WATCHER_PATH, w))
                .and_then(|b| b.build());
            let conn = match built {
                Ok(c) => c,
                Err(e) => {
                    tracing::info!("bandeja: outro programa já atende ({e})");
                    return;
                }
            };
            let _ = conn.request_name(format!("org.kde.StatusNotifierHost-{}", std::process::id()));
            let _ = conn.emit_signal(None::<&str>, WATCHER_PATH, WATCHER, "StatusNotifierHostRegistered", &());
            tracing::info!("bandeja: atendendo {WATCHER}");
            // Sinais dos ícones (ícone novo, título, estado…): relê o que veio de quem mandou.
            let (c2, t2) = (conn.clone(), tx.clone());
            std::thread::spawn(move || {
                let Ok(rule) = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).interface(ITEM).map(|b| b.build()) else { return };
                let Ok(it) = zbus::blocking::MessageIterator::for_match_rule(rule, &c2, Some(64)) else { return };
                for m in it.flatten() {
                    if let Some(s) = m.header().sender() {
                        let _ = t2.send(Pedido::Mudou { sender: s.to_string() });
                    }
                }
            });
            // Um app que sai do barramento leva os ícones dele junto.
            let (c3, t3) = (conn.clone(), tx.clone());
            std::thread::spawn(move || {
                let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&c3) else { return };
                let Ok(sinais) = proxy.receive_name_owner_changed() else { return };
                for s in sinais {
                    if let Ok(a) = s.args() {
                        if a.new_owner().is_none() {
                            let _ = t3.send(Pedido::Sumiu { name: a.name().to_string() });
                        }
                    }
                }
            });
            linha(conn, rx, lista, tx_out);
        });
    }

    fn bandeja_msg(&mut self, m: Msg) {
        match m {
            Msg::Itens(v) => {
                let theme = { let t = self.config.appearance.icon_theme.clone(); if t.is_empty() { "hicolor".to_string() } else { t } };
                let px = (glove_paint::chrome::TRAY_ICON * self.scale().max(1.0) as f32).ceil() as u32;
                for it in &v {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    (&it.icon_name, &it.theme_path, it.pixmap.as_ref().map(|p| (p.0, p.1, &p.2)), px, &theme).hash(&mut h);
                    let sig = h.finish();
                    if self.bandeja.icons.get(&it.key).is_some_and(|(s, _)| *s == sig) {
                        continue;
                    }
                    let icon = icone(it, &theme, px);
                    self.bandeja.icons.insert(it.key.clone(), (sig, icon));
                }
                self.bandeja.icons.retain(|k, _| v.iter().any(|i| &i.key == k));
                self.bandeja.itens = v;
                self.dirty = true;
            }
            Msg::Menu { key, nos } => {
                let Some((pedido, k)) = self.bandeja.menu_pedido.take() else { return };
                if pedido != key || nos.is_empty() {
                    return;
                }
                let nodes = nos.iter().map(|n| no_para_menu(&key, n)).collect();
                self.show_menu(BarTarget::Extra(k), nodes);
            }
        }
    }

    /// Clique num ícone da bandeja: abre o app (ou o menu, se o app só tem menu).
    pub fn bandeja_clique(&mut self, i: usize, k: usize) {
        let Some(it) = self.bandeja.itens.get(i).cloned() else { return };
        if it.item_is_menu && it.has_menu {
            self.bandeja_menu(i, k);
            return;
        }
        self.close_menu();
        let x = self.chrome.extra_ranges.get(k).map(|r| (r.0 + r.1) / 2.0).unwrap_or(0.0) as i32;
        if let Some(tx) = &self.bandeja.tx {
            let _ = tx.send(Pedido::Ativar { key: it.key, x, y: glove_paint::theme::MENUBAR_H });
        }
    }

    /// O menu do app (botão direito no ícone da bandeja).
    pub fn bandeja_menu(&mut self, i: usize, k: usize) {
        let Some(it) = self.bandeja.itens.get(i).cloned() else { return };
        if !it.has_menu {
            self.bandeja_clique_sem_menu(&it, k);
            return;
        }
        self.bandeja.menu_pedido = Some((it.key.clone(), k));
        if let Some(tx) = &self.bandeja.tx {
            let _ = tx.send(Pedido::Menu { key: it.key });
        }
    }

    fn bandeja_clique_sem_menu(&mut self, it: &ItemInfo, k: usize) {
        let x = self.chrome.extra_ranges.get(k).map(|r| (r.0 + r.1) / 2.0).unwrap_or(0.0) as i32;
        if let Some(tx) = &self.bandeja.tx {
            let _ = tx.send(Pedido::Ativar { key: it.key.clone(), x, y: glove_paint::theme::MENUBAR_H });
        }
    }

    /// A posição na bandeja do extra `k` da barra (se for um ícone da bandeja).
    pub fn tray_index(&self, k: usize) -> Option<usize> {
        match self.extra_kinds().get(k) {
            Some(crate::extras::ExtraKind::Tray(i)) => Some(*i),
            _ => None,
        }
    }

    /// Um item do menu do app foi escolhido.
    pub fn bandeja_evento(&mut self, key: String, id: i32) {
        if let Some(tx) = &self.bandeja.tx {
            let _ = tx.send(Pedido::Evento { key, id });
        }
    }
}

/// O item do menu do app como um item do menu do Glove.
fn no_para_menu(key: &str, n: &No) -> MenuNode {
    if n.separator {
        return MenuNode::sep();
    }
    if !n.children.is_empty() {
        let mut node = MenuNode::new(MenuItem::sub(n.label.clone()), None);
        node.sub = n.children.iter().map(|c| no_para_menu(key, c)).collect();
        return node;
    }
    let item = MenuItem::check(n.label.clone(), n.checked);
    let item = if n.enabled { item } else { item.disabled() };
    MenuNode::new(item, n.enabled.then(|| MenuAction::Bandeja(key.to_string(), n.id)))
}

/// A imagem do ícone: a que o app desenhou (IconPixmap: traz o estado, como a contagem
/// de não lidas) ou a do tema pelo nome (na pasta que o app indicou, se houver).
fn icone(it: &ItemInfo, theme: &str, px: u32) -> Option<TrayIcon> {
    if let Some((w, h, d)) = &it.pixmap {
        return Some(TrayIcon { w: *w, h: *h, rgba: Arc::new(d.clone()) });
    }
    if !it.icon_name.is_empty() {
        let mut achada = None;
        if !it.theme_path.is_empty() {
            for ext in ["png", "svg"] {
                let f = format!("{}/{}.{ext}", it.theme_path.trim_end_matches('/'), it.icon_name);
                if std::path::Path::new(&f).is_file() {
                    achada = glove_paint::sysicons::image_full(theme, &f, px);
                    break;
                }
            }
        }
        let img = achada.or_else(|| glove_paint::sysicons::image_full(theme, &it.icon_name, px));
        if let Some(p) = img {
            return Some(TrayIcon { w: p.width(), h: p.height(), rgba: Arc::new(p.data().to_vec()) });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonico_sai() {
        assert_eq!(sem_mnemonico("_Abrir"), "Abrir");
        assert_eq!(sem_mnemonico("Sair__já"), "Sair_já");
        assert_eq!(sem_mnemonico("Sem nada"), "Sem nada");
    }

    #[test]
    fn argb_vira_rgba_pre_multiplicado() {
        // Um pixel vermelho opaco e um branco meio transparente.
        let (w, h, d) = argb_para_rgba(2, 1, &[255, 255, 0, 0, 128, 255, 255, 255]).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(&d[..4], &[255, 0, 0, 255]);
        assert_eq!(&d[4..], &[128, 128, 128, 128]);
        assert!(argb_para_rgba(2, 2, &[0; 8]).is_none());
    }
}
