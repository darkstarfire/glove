//! glove-web: o navegador do Glove. O motor é o do Chromium (CEF, o mesmo do Chrome, com
//! todos os recursos da web de hoje); a janela, as abas e a barra de endereço são do
//! glove-ui (visual clássico ou moderno, como o resto do Glove). Feito para pesar pouco:
//! sem o processo da placa de vídeo, poucos processos de página e nada rodando de fundo.
//!
//!   glove-web [ENDEREÇO…]   abre os endereços (numa janela que já esteja aberta, se houver)

mod motor;

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cef::{ImplBrowser, ImplBrowserHost, ImplCommandLine, ImplFrame, ImplJsdialogCallback};
use glove_config::tr;
use glove_ui::{
    paint::{draw::*, icons::Glyph, Color},
    run, App, Menu, MenuItem, Rect, TabAction, Ui, WindowOptions, SEGMENT_H, SEGMENT_W, TAB_STRIP_H,
};

use motor::{Pagina, Shared};

/// A página inicial e a busca (sem rastrear quem busca).
const INICIO: &str = "https://duckduckgo.com/";
const BUSCA: &str = "https://duckduckgo.com/?q=";

const BARRA_H: f32 = 40.0;
const STATUS_H: f32 = 20.0;

pub fn pasta_downloads() -> PathBuf {
    glove_config::arquivos::user_dir("DOWNLOAD").unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Downloads"))
}

fn socket_path() -> PathBuf {
    PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into())).join("glove-web.sock")
}

/// O que se digitou na barra: endereço (com ou sem https://) ou busca.
pub fn endereco(t: &str) -> String {
    let t = t.trim();
    if t.is_empty() {
        return INICIO.to_string();
    }
    if t.contains("://") || t.starts_with("about:") || t.starts_with("data:") {
        return t.to_string();
    }
    if t.starts_with('/') {
        return format!("file://{t}");
    }
    let parece = !t.contains(' ') && (t.contains('.') || t.starts_with("localhost")) && !t.ends_with('.');
    if parece {
        return format!("https://{t}");
    }
    let q: String = t.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else if b == b' ' { "+".into() } else { format!("%{b:02X}") }).collect();
    format!("{BUSCA}{q}")
}

/// O endereço como aparece na barra: sem o https:// do começo e com os acentos de
/// volta (%C3%A9 → é), se o resultado for texto válido.
pub fn mostrar(url: &str) -> String {
    let u = url.strip_prefix("https://").unwrap_or(url).trim_end_matches('/');
    let b = u.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&u[i + 1..i + 3], 16) {
                // Espaços e barras codificados ficam como estão (mudariam o endereço).
                if v >= 0x80 {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barra_de_endereco() {
        assert_eq!(endereco("wikipedia.org"), "https://wikipedia.org");
        assert_eq!(endereco("gatos fofos"), "https://duckduckgo.com/?q=gatos+fofos");
        assert_eq!(endereco("http://a.b/c"), "http://a.b/c");
        assert_eq!(endereco("/home/x.html"), "file:///home/x.html");
        assert_eq!(mostrar("https://pt.wikipedia.org/wiki/Dia_Mundial_da_Televis%C3%A3o"), "pt.wikipedia.org/wiki/Dia_Mundial_da_Televisão");
        assert_eq!(mostrar("https://a.b/x%20y/"), "a.b/x%20y");
    }
}

struct Aba {
    p: Shared,
    /// A imagem da página pronta para a janela (refeita quando chega imagem nova).
    tela: Option<Canvas>,
    popup: Option<Canvas>,
}

struct Navegador {
    abas: Vec<Aba>,
    atual: usize,
    /// O texto da barra de endereço (editado) e se ela está sendo editada.
    barra: String,
    barra_foco: bool,
    focar_barra: bool,
    /// Área da página na janela (do quadro anterior).
    area: Rect,
    escala: f32,
    /// O mouse estava sobre a página (para avisar a saída).
    dentro: bool,
    apertado: bool,
    aviso: Option<(String, Instant)>,
    servidor: Option<UnixListener>,
    /// Última vez que algo mexeu (o motor trabalha mais rápido logo depois).
    ativo: Instant,
    /// O que o último menu de contexto pegou (o item escolhido chega depois).
    ctx_dados: Option<motor::Contexto>,
}

impl Navegador {
    fn aba(&self) -> Option<&Aba> {
        self.abas.get(self.atual)
    }

    fn browser(&self) -> Option<cef::Browser> {
        self.aba().and_then(|a| a.p.borrow().browser.clone())
    }

    fn host(&self) -> Option<cef::BrowserHost> {
        self.browser().and_then(|b| b.host())
    }

    fn abrir(&mut self, url: &str) {
        let p: Shared = std::rc::Rc::new(std::cell::RefCell::new(Pagina::new(self.escala)));
        {
            let mut pg = p.borrow_mut();
            pg.w = self.area.w.max(100.0) as i32;
            pg.h = self.area.h.max(100.0) as i32;
            pg.url = url.to_string();
        }
        if motor::nova_aba(&p, url) {
            self.abas.push(Aba { p, tela: None, popup: None });
            self.mudar(self.abas.len() - 1);
            self.barra = mostrar(url);
        }
    }

    fn mudar(&mut self, i: usize) {
        if let Some(h) = self.host() {
            h.was_hidden(1);
        }
        self.atual = i.min(self.abas.len().saturating_sub(1));
        if let Some(h) = self.host() {
            h.was_hidden(0);
            h.set_focus(1);
        }
        let url = self.aba().map(|a| a.p.borrow().url.clone()).unwrap_or_default();
        self.barra = mostrar(&url);
        self.barra_foco = false;
    }

    fn fechar(&mut self, i: usize) {
        if i >= self.abas.len() {
            return;
        }
        let a = self.abas.remove(i);
        let b = a.p.borrow().browser.clone();
        if let Some(h) = b.and_then(|b| b.host()) {
            h.close_browser(1);
        }
        if self.abas.is_empty() {
            self.abrir(INICIO);
        } else {
            self.mudar(self.atual.min(self.abas.len() - 1));
        }
    }

    fn carregar(&mut self, url: &str) {
        let url = endereco(url);
        if let Some(f) = self.browser().and_then(|b| b.main_frame()) {
            f.load_url(Some(&url.as_str().into()));
        }
        if let Some(a) = self.aba() {
            a.p.borrow_mut().url = url.clone();
        }
        self.barra = mostrar(&url);
        if let Some(h) = self.host() {
            h.set_focus(1);
        }
    }

    /// Atalhos do navegador (do teclado ou dos menus). Devolve se tratou.
    fn atalho(&mut self, combo: &str) -> bool {
        let b = self.browser();
        let frame = || b.as_ref().and_then(|b| b.focused_frame().or_else(|| b.main_frame()));
        match combo {
            "Ctrl+T" => self.abrir(INICIO),
            "Ctrl+W" => self.fechar(self.atual),
            "Ctrl+L" | "F6" => self.focar_barra = true,
            "Ctrl+R" | "F5" => {
                if let Some(b) = &b {
                    b.reload();
                }
            }
            "Ctrl+Shift+R" => {
                if let Some(b) = &b {
                    b.reload_ignore_cache();
                }
            }
            "Alt+Left" => {
                if let Some(b) = &b {
                    b.go_back();
                }
            }
            "Alt+Right" => {
                if let Some(b) = &b {
                    b.go_forward();
                }
            }
            "Ctrl+Tab" if !self.abas.is_empty() => self.mudar((self.atual + 1) % self.abas.len()),
            "Ctrl+Shift+Tab" if !self.abas.is_empty() => self.mudar((self.atual + self.abas.len() - 1) % self.abas.len()),
            "Ctrl+=" | "Ctrl++" | "Ctrl+Shift+=" | "Ctrl+plus" | "Ctrl+equal" => self.zoom(0.5),
            "Ctrl+-" | "Ctrl+minus" => self.zoom(-0.5),
            "Ctrl+0" => {
                if let Some(h) = self.host() {
                    h.set_zoom_level(0.0);
                }
            }
            "Ctrl+C" => frame().map(|f| f.copy()).unwrap_or(()),
            "Ctrl+X" => frame().map(|f| f.cut()).unwrap_or(()),
            "Ctrl+V" => frame().map(|f| f.paste()).unwrap_or(()),
            "Ctrl+A" => frame().map(|f| f.select_all()).unwrap_or(()),
            "Ctrl+Z" => frame().map(|f| f.undo()).unwrap_or(()),
            "Ctrl+Shift+Z" | "Ctrl+Y" => frame().map(|f| f.redo()).unwrap_or(()),
            _ => return false,
        }
        true
    }

    fn zoom(&self, d: f64) {
        if let Some(h) = self.host() {
            h.set_zoom_level((h.zoom_level() + d).clamp(-4.0, 6.0));
        }
    }

    /// Endereços vindos de outra chamada do glove-web (o segundo abre aqui).
    fn ler_servidor(&mut self) {
        let mut novos = Vec::new();
        if let Some(l) = &self.servidor {
            while let Ok((mut c, _)) = l.accept() {
                let _ = c.set_read_timeout(Some(Duration::from_millis(200)));
                let mut t = String::new();
                let _ = c.read_to_string(&mut t);
                novos.extend(t.lines().map(str::to_string));
            }
        }
        for u in novos {
            self.abrir(if u.trim().is_empty() { INICIO } else { &u });
        }
    }

    /// A barra de cima: voltar/avançar, recarregar, o endereço e a aba nova.
    fn barra(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let modern = ui.theme.modern;
        if modern {
            ui.c.fill_rect(0.0, 0.0, w, BARRA_H, &solid(ui.theme.body_a));
        } else {
            ui.c.fill_rect(0.0, 0.0, w, BARRA_H, &vgrad(0.0, BARRA_H, &[(0.0, rgba(236, 236, 236, 255)), (1.0, rgba(212, 212, 216, 255))]));
        }
        ui.c.hline(0.0, BARRA_H - 1.0, w, rgba(150, 150, 156, 255));
        let cy = (BARRA_H - SEGMENT_H) / 2.0;
        let (back, fwd, loading) = self.aba().map(|a| {
            let p = a.p.borrow();
            (p.back, p.forward, p.loading)
        }).unwrap_or_default();
        match ui.segmented_in(Rect::new(10.0, cy, 2.0 * SEGMENT_W, SEGMENT_H), &[Glyph::Back, Glyph::Forward], None, &[back, fwd]) {
            Some(0) => {
                self.atalho("Alt+Left");
            }
            Some(_) => {
                self.atalho("Alt+Right");
            }
            None => {}
        }
        // Recarregar (ou parar, carregando).
        let rx = 10.0 + 2.0 * SEGMENT_W + 8.0;
        let glyph = if loading { Glyph::Minus } else { Glyph::RotateRight };
        if ui.segmented_in(Rect::new(rx, cy, SEGMENT_W, SEGMENT_H), &[glyph], None, &[]).is_some() {
            if let Some(b) = self.browser() {
                if loading { b.stop_load() } else { b.reload() }
            }
        }
        // Aba nova, à direita.
        let nx = w - 10.0 - SEGMENT_W;
        if ui.segmented_in(Rect::new(nx, cy, SEGMENT_W, SEGMENT_H), &[Glyph::Plus], None, &[]).is_some() {
            self.abrir(INICIO);
            self.focar_barra = true;
        }
        // O endereço no meio.
        let fx = rx + SEGMENT_W + 12.0;
        let fr = Rect::new(fx, (BARRA_H - 24.0) / 2.0, (nx - 12.0 - fx).max(80.0), 24.0);
        let autofocus = std::mem::take(&mut self.focar_barra);
        if autofocus {
            if let Some(h) = self.host() {
                h.set_focus(0);
            }
            // Ctrl+L: o campo começa vazio (digitar já troca o endereço); Esc devolve.
            self.barra.clear();
        }
        let (_, enter) = ui.text_field_in(fr, &mut self.barra, autofocus);
        self.barra_foco = ui.has_focus();
        if self.barra_foco && ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            let url = self.aba().map(|a| a.p.borrow().url.clone()).unwrap_or_default();
            self.barra = mostrar(&url);
            if let Some(h) = self.host() {
                h.set_focus(1);
            }
        }
        // Fora de foco, a barra mostra o endereço da página (que muda ao seguir links).
        if !self.barra_foco && !enter {
            let url = self.aba().map(|a| a.p.borrow().url.clone()).unwrap_or_default();
            if !url.is_empty() {
                self.barra = mostrar(&url);
            }
        }
        if enter {
            let t = self.barra.clone();
            self.carregar(&t);
            self.barra_foco = false;
        }
        // Carregando: uma faixa fina embaixo da barra.
        if loading {
            let t = (self.ativo.elapsed().as_millis() % 1200) as f32 / 1200.0;
            let bw = w * 0.25;
            ui.c.fill_rect(-bw + t * (w + bw), BARRA_H - 2.0, bw, 2.0, &solid(ui.theme.accent));
        }
    }

    /// Copia a imagem nova da página (se houver) para os canvas da aba.
    fn atualizar_tela(&mut self) {
        let escala = self.escala;
        let Some(a) = self.abas.get_mut(self.atual) else { return };
        let mut p = a.p.borrow_mut();
        if !p.dirty {
            return;
        }
        p.dirty = false;
        if p.fw > 0 && p.fh > 0 && p.frame.len() == (p.fw * p.fh * 4) as usize {
            let need = (p.fw as f32 / escala, p.fh as f32 / escala);
            let ok = a.tela.as_ref().is_some_and(|c| c.px_width() == p.fw as u32 && c.px_height() == p.fh as u32);
            if !ok {
                a.tela = Canvas::new(need.0, need.1, escala).filter(|c| c.px_width() == p.fw as u32 && c.px_height() == p.fh as u32);
            }
            if let Some(c) = &mut a.tela {
                c.pixels_mut().copy_from_slice(&p.frame);
            }
        }
        a.popup = None;
        if p.popup.is_some() && p.pw > 0 && p.ph > 0 && p.popup_frame.len() == (p.pw * p.ph * 4) as usize {
            if let Some(mut c) = Canvas::new(p.pw as f32 / escala, p.ph as f32 / escala, escala).filter(|c| c.px_width() == p.pw as u32 && c.px_height() == p.ph as u32) {
                c.pixels_mut().copy_from_slice(&p.popup_frame);
                a.popup = Some(c);
            }
        }
    }

    /// O mouse e o teclado vão para a página (o que não é da barra).
    fn entrada(&mut self, ui: &mut Ui, area: Rect) {
        let Some(h) = self.host() else { return };
        let (px, py) = ui.pointer();
        let mods = ui.mods().clone();
        let flags = (if mods.shift { 1 << 1 } else { 0 }) | (if mods.ctrl { 1 << 2 } else { 0 }) | (if mods.alt { 1 << 3 } else { 0 }) | (if ui.mouse_down() { 1 << 4 } else { 0 });
        let ev = cef::MouseEvent { x: (px - area.x).round() as i32, y: (py - area.y).round() as i32, modifiers: flags };
        let inside = ui.pointer_inside() && area.contains((px, py));
        if inside || self.apertado {
            if ui.moved() {
                h.send_mouse_move_event(Some(&ev), 0);
                self.ativo = Instant::now();
            }
            self.dentro = true;
        } else if self.dentro {
            h.send_mouse_move_event(Some(&ev), 1);
            self.dentro = false;
        }
        if inside && ui.mouse_pressed() {
            h.set_focus(1);
            self.apertado = true;
            h.send_mouse_click_event(Some(&ev), cef::MouseButtonType::LEFT, 0, ui.click_count().max(1) as i32);
            self.ativo = Instant::now();
        }
        if self.apertado && ui.mouse_released() {
            self.apertado = false;
            h.send_mouse_click_event(Some(&ev), cef::MouseButtonType::LEFT, 1, ui.click_count().max(1) as i32);
        }
        if ui.right_clicked(area) {
            h.set_focus(1);
            h.send_mouse_click_event(Some(&ev), cef::MouseButtonType::RIGHT, 0, 1);
            h.send_mouse_click_event(Some(&ev), cef::MouseButtonType::RIGHT, 1, 1);
        }
        let sc = ui.scroll_in(area);
        if sc != 0.0 {
            // A roda vem em pixels (para baixo = positivo); o motor quer o contrário e maior.
            h.send_mouse_wheel_event(Some(&ev), 0, (-sc * 4.0).round() as i32);
            self.ativo = Instant::now();
        }
        // Teclado: só com a barra de endereço fora de foco.
        if self.barra_foco {
            return;
        }
        let keys = ui.keys().to_vec();
        let mut handled_text = false;
        for k in keys {
            use glove_ui::ui::Key;
            if self.atalho(&k.combo) {
                handled_text = true;
                continue;
            }
            let vk = match k.key {
                Key::Backspace => 8,
                Key::Tab => 9,
                Key::Enter => 13,
                Key::Escape => 27,
                Key::PageUp => 33,
                Key::PageDown => 34,
                Key::End => 35,
                Key::Home => 36,
                Key::Left => 37,
                Key::Up => 38,
                Key::Right => 39,
                Key::Down => 40,
                Key::Delete => 46,
                Key::Other => 0,
            };
            if vk == 0 {
                continue;
            }
            let mut e = cef::KeyEvent { size: std::mem::size_of::<cef::sys::cef_key_event_t>(), type_: cef::KeyEventType::RAWKEYDOWN, modifiers: flags, windows_key_code: vk, ..Default::default() };
            h.send_key_event(Some(&e));
            if vk == 13 || vk == 9 || vk == 8 {
                e.type_ = cef::KeyEventType::CHAR;
                e.character = if vk == 8 { 8 } else if vk == 9 { 9 } else { 13 };
                e.unmodified_character = e.character;
                h.send_key_event(Some(&e));
            }
            e.type_ = cef::KeyEventType::KEYUP;
            h.send_key_event(Some(&e));
            self.ativo = Instant::now();
        }
        if !handled_text {
            let text = ui.typed().to_string();
            for ch in text.chars().filter(|c| !c.is_control()) {
                let mut buf = [0u16; 2];
                for u in ch.encode_utf16(&mut buf).iter() {
                    let e = cef::KeyEvent { size: std::mem::size_of::<cef::sys::cef_key_event_t>(), type_: cef::KeyEventType::CHAR, modifiers: flags & !(1 << 4), windows_key_code: *u as i32, character: *u, unmodified_character: *u, ..Default::default() };
                    h.send_key_event(Some(&e));
                }
                self.ativo = Instant::now();
            }
        }
    }

    /// O menu de contexto que a página pediu (link, seleção, campo).
    fn menu_contexto(&mut self, ui: &mut Ui, area: Rect) {
        let Some(c) = self.aba().and_then(|a| a.p.borrow_mut().ctx.take()) else { return };
        let mut itens = Vec::new();
        if !c.link.is_empty() {
            itens.push(MenuItem::new("ctx-link-aba", tr!("Abrir Link em Nova Aba"), ""));
            itens.push(MenuItem::new("ctx-link-copiar", tr!("Copiar Link"), ""));
            itens.push(MenuItem::Separator);
        }
        if !c.image.is_empty() {
            itens.push(MenuItem::new("ctx-img-aba", tr!("Abrir Imagem em Nova Aba"), ""));
            itens.push(MenuItem::Separator);
        }
        if c.editable {
            itens.push(MenuItem::new("ctx-recortar", tr!("Recortar"), ""));
        }
        if c.editable || !c.selection.is_empty() {
            itens.push(MenuItem::new("ctx-copiar", tr!("Copiar"), ""));
        }
        if c.editable {
            itens.push(MenuItem::new("ctx-colar", tr!("Colar"), ""));
        }
        if !c.selection.is_empty() {
            itens.push(MenuItem::new("ctx-buscar", &tr!("Buscar “{}”", c.selection.chars().take(24).collect::<String>()), ""));
        }
        if itens.last().is_some_and(|i| !matches!(i, MenuItem::Separator)) {
            itens.push(MenuItem::Separator);
        }
        let (back, fwd) = self.aba().map(|a| (a.p.borrow().back, a.p.borrow().forward)).unwrap_or_default();
        itens.push(MenuItem::new("ctx-voltar", tr!("Voltar"), "").enabled(back));
        itens.push(MenuItem::new("ctx-avancar", tr!("Avançar"), "").enabled(fwd));
        itens.push(MenuItem::new("ctx-recarregar", tr!("Recarregar"), ""));
        self.ctx_dados = Some(c.clone());
        ui.context_menu((area.x + c.x as f32, area.y + c.y as f32), &itens);
    }
}

impl Navegador {
    fn acao_contexto(&mut self, id: &str) {
        let Some(c) = self.ctx_dados.clone() else { return };
        let b = self.browser();
        let frame = || b.as_ref().and_then(|b| b.focused_frame().or_else(|| b.main_frame()));
        match id {
            "ctx-link-aba" => self.abrir(&c.link),
            "ctx-img-aba" => self.abrir(&c.image),
            "ctx-link-copiar" => copiar_texto(&c.link),
            "ctx-recortar" => frame().map(|f| f.cut()).unwrap_or(()),
            "ctx-copiar" => frame().map(|f| f.copy()).unwrap_or(()),
            "ctx-colar" => frame().map(|f| f.paste()).unwrap_or(()),
            "ctx-buscar" => {
                let u = endereco(&c.selection);
                let u = if u.starts_with(BUSCA) { u } else { format!("{BUSCA}{}", c.selection.replace(' ', "+")) };
                self.abrir(&u);
            }
            "ctx-voltar" => {
                self.atalho("Alt+Left");
            }
            "ctx-avancar" => {
                self.atalho("Alt+Right");
            }
            "ctx-recarregar" => {
                self.atalho("Ctrl+R");
            }
            _ => {}
        }
    }
}

/// Texto para a área de transferência (pelo wl-copy, se houver).
fn copiar_texto(t: &str) {
    if let Ok(mut c) = std::process::Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn() {
        if let Some(mut i) = c.stdin.take() {
            let _ = i.write_all(t.as_bytes());
        }
        let _ = c.wait();
    }
}

impl App for Navegador {
    fn tick(&self) -> Option<Duration> {
        // O motor pede quando quer trabalhar; com algo acontecendo, quadros a ~60/s.
        let pedido = motor::BOMBA.with(|b| b.get()) as u64;
        let mexendo = self.ativo.elapsed() < Duration::from_secs(2) || self.abas.iter().any(|a| a.p.borrow().loading);
        let ms = if mexendo { 16 } else { pedido.clamp(16, 120) };
        Some(Duration::from_millis(ms))
    }

    fn on_tick(&mut self) {
        cef::do_message_loop_work();
        let novas = motor::NOVAS_ABAS.with(|n| std::mem::take(&mut *n.borrow_mut()));
        for u in novas {
            self.abrir(&u);
        }
        if let Some(a) = motor::AVISOS.with(|a| a.borrow_mut().pop()) {
            self.aviso = Some((a, Instant::now()));
        }
        self.ler_servidor();
    }

    fn close_requested(&mut self) -> bool {
        for a in &self.abas {
            let b = a.p.borrow().browser.clone();
            if let Some(h) = b.and_then(|b| b.host()) {
                h.close_browser(1);
            }
        }
        true
    }

    fn paints_background(&self) -> bool {
        true
    }

    fn menus(&self) -> Vec<Menu> {
        vec![
            Menu::new(tr!("Arquivo"), vec![
                MenuItem::new("nova-aba", tr!("Nova Aba"), "Ctrl+T"),
                MenuItem::new("abrir-local", tr!("Abrir Endereço…"), "Ctrl+L"),
                MenuItem::Separator,
                MenuItem::new("fechar-aba", tr!("Fechar Aba"), "Ctrl+W"),
            ]),
            Menu::new(tr!("Editar"), vec![
                MenuItem::new("desfazer", tr!("Desfazer"), "Ctrl+Z"),
                MenuItem::new("refazer", tr!("Refazer"), "Ctrl+Shift+Z"),
                MenuItem::Separator,
                MenuItem::new("recortar", tr!("Recortar"), "Ctrl+X"),
                MenuItem::new("copiar", tr!("Copiar"), "Ctrl+C"),
                MenuItem::new("colar", tr!("Colar"), "Ctrl+V"),
                MenuItem::new("tudo", tr!("Selecionar Tudo"), "Ctrl+A"),
            ]),
            Menu::new(tr!("Visualizar"), vec![
                MenuItem::new("recarregar", tr!("Recarregar a Página"), "Ctrl+R"),
                MenuItem::Separator,
                MenuItem::new("maior", tr!("Aumentar"), "Ctrl+="),
                MenuItem::new("menor", tr!("Diminuir"), "Ctrl+-"),
                MenuItem::new("real", tr!("Tamanho Real"), "Ctrl+0"),
            ]),
            Menu::new(tr!("Histórico"), vec![
                MenuItem::new("voltar", tr!("Voltar"), "Alt+Left"),
                MenuItem::new("avancar", tr!("Avançar"), "Alt+Right"),
                MenuItem::Separator,
                MenuItem::new("inicio", tr!("Início"), ""),
            ]),
        ]
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.escala = ui.c.scale;
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "inicio" => self.carregar(INICIO),
                id if id.starts_with("ctx-") => self.acao_contexto(id),
                _ => {}
            }
        }
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(Color::WHITE));
        self.barra(ui);
        // Abas (só com mais de uma, como no Safari).
        let mut top = BARRA_H;
        if self.abas.len() > 1 {
            let titulos: Vec<String> = self.abas.iter().map(|a| {
                let p = a.p.borrow();
                if p.title.is_empty() { mostrar(&p.url) } else { p.title.clone() }
            }).collect();
            match ui.tab_strip(Rect::new(0.0, top, w, TAB_STRIP_H), &titulos, self.atual) {
                Some(TabAction::Select(i)) => self.mudar(i),
                Some(TabAction::Close(i)) => self.fechar(i),
                Some(TabAction::New) => {
                    self.abrir(INICIO);
                    self.focar_barra = true;
                }
                None => {}
            }
            top += TAB_STRIP_H;
        }
        let status = self.aviso.as_ref().filter(|(_, t)| t.elapsed() < Duration::from_secs(5)).map(|(m, _)| m.clone()).or_else(|| {
            self.aba().map(|a| a.p.borrow().status.clone()).filter(|s| !s.is_empty())
        });
        let area = Rect::new(0.0, top, w, (h - top).max(1.0));
        // O tamanho da página acompanha a janela.
        if (area.w, area.h) != (self.area.w, self.area.h) || self.aba().is_some_and(|a| a.p.borrow().scale != self.escala) {
            for a in &self.abas {
                let mut p = a.p.borrow_mut();
                p.w = area.w as i32;
                p.h = area.h as i32;
                p.scale = self.escala;
            }
            self.area = area;
            for a in &self.abas {
                let b = a.p.borrow().browser.clone();
                if let Some(hh) = b.and_then(|b| b.host()) {
                    hh.notify_screen_info_changed();
                    hh.was_resized();
                }
            }
        }
        self.area = area;
        self.atualizar_tela();
        if let Some(a) = self.aba() {
            if let Some(c) = &a.tela {
                ui.c.draw_canvas(c, area.x, area.y);
            }
            let pop = a.p.borrow().popup.clone();
            if let (Some(c), Some(r)) = (&a.popup, pop) {
                ui.c.draw_canvas(c, area.x + r.x as f32, area.y + r.y as f32);
            }
        }
        // O link sob o mouse (ou um aviso), num balãozinho no canto de baixo.
        if let Some(t) = status {
            let t = ui.fonts.ellipsize(&t, 11.0, false, w * 0.6);
            let tw = ui.fonts.measure(&t, 11.0, false) + 14.0;
            let y = h - STATUS_H;
            ui.c.fill(rrect4(0.0, y, tw, STATUS_H, [0.0, 5.0, 0.0, 0.0]), &solid(rgba(240, 240, 242, 245)));
            ui.c.stroke(rrect4(0.5, y + 0.5, tw - 1.0, STATUS_H, [0.0, 5.0, 0.0, 0.0]), &solid(rgba(170, 170, 176, 255)), 1.0);
            ui.fonts.draw(ui.c, &t, 7.0, y + 14.0, 11.0, false, rgba(40, 40, 44, 255));
        }
        // O título da janela segue a página.
        let titulo = self.aba().map(|a| a.p.borrow().title.clone()).unwrap_or_default();
        ui.set_title(if titulo.is_empty() { tr!("Navegador") } else { &titulo });
        // alert()/confirm() da página: alerta do Glove (janela própria).
        let dialog = self.aba().and_then(|a| a.p.borrow().dialog.clone());
        if let Some((confirm, msg, cb)) = dialog {
            let botoes: Vec<&str> = if confirm { vec![tr!("Cancelar"), tr!("OK")] } else { vec![tr!("OK")] };
            let url = self.aba().map(|a| mostrar(&a.p.borrow().url)).unwrap_or_default();
            if let Some(i) = ui.alert(glove_ui::AlertKind::Note, &tr!("A página {} diz:", url.split('/').next().unwrap_or("")), &msg, &botoes) {
                cb.cont(if confirm { (i == 1) as i32 } else { 1 }, None);
                if let Some(a) = self.aba() {
                    a.p.borrow_mut().dialog = None;
                }
            }
        }
        if !self.barra_foco || ui.mouse_pressed() {
            self.entrada(ui, area);
        }
        self.menu_contexto(ui, area);
    }
}

fn main() -> std::process::ExitCode {
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let Some(cmd) = args.as_cmd_line() else { return 1.into() };
    let is_browser = cmd.has_switch(Some(&cef::CefString::from("type"))) != 1;
    let mut app = motor::app();
    let ret = cef::execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    if !is_browser {
        // Processo da página, da rede etc.: o motor já fez o trabalho dele.
        return (ret.max(0) as u8).into();
    }
    // Endereços pedidos (o resto da linha de comando que não é opção).
    let urls: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with("--")).map(|a| endereco(&a)).collect();
    // Já aberto: manda os endereços para ele e sai.
    let sock = socket_path();
    if let Ok(mut c) = UnixStream::connect(&sock) {
        let text = if urls.is_empty() { "\n".to_string() } else { urls.join("\n") };
        let _ = c.write_all(text.as_bytes());
        return 0.into();
    }
    let _ = std::fs::remove_file(&sock);
    let servidor = UnixListener::bind(&sock).ok().inspect(|l| {
        let _ = l.set_nonblocking(true);
    });

    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let dados = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share")).join("glove-web");
    let _ = std::fs::create_dir_all(&dados);
    let perfil = dados.join("Perfil");
    let idioma = if glove_config::idioma().starts_with("en") { "en-US" } else { "pt-BR" };
    let settings = cef::Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: (std::env::var_os("GLOVE_WEB_SEM_SANDBOX").is_some()) as i32,
        root_cache_path: dados.to_string_lossy().as_ref().into(),
        cache_path: perfil.to_string_lossy().as_ref().into(),
        persist_session_cookies: 1,
        locale: idioma.into(),
        accept_language_list: if idioma == "pt-BR" { "pt-BR,pt,en-US,en".into() } else { "en-US,en".into() },
        log_severity: cef::LogSeverity::ERROR,
        background_color: 0xffff_ffff,
        ..Default::default()
    };
    let dbg = std::env::var_os("GLOVE_WEB_DEBUG").is_some();
    if dbg {
        eprintln!("glove-web: iniciando o motor");
    }
    if cef::initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()) != 1 {
        eprintln!("glove-web: o motor do Chromium não iniciou");
        return 1.into();
    }
    let mut nav = Navegador { abas: Vec::new(), atual: 0, barra: String::new(), barra_foco: false, focar_barra: false, area: Rect::new(0.0, BARRA_H, 1100.0, 700.0), escala: 1.0, dentro: false, apertado: false, aviso: None, servidor, ativo: Instant::now(), ctx_dados: None };
    if urls.is_empty() {
        nav.abrir(INICIO);
    } else {
        for u in &urls {
            nav.abrir(u);
        }
    }
    let opts = WindowOptions { title: tr!("Navegador").into(), app_id: "glove.Navegador".into(), size: (1100, 760), fixed: false, min_size: Some((520, 360)) };
    if dbg {
        eprintln!("glove-web: motor pronto, abrindo a janela");
    }
    let r = run(opts, nav);
    if dbg {
        eprintln!("glove-web: janela fechada: {:?}", r.as_ref().err());
    }
    // Fecha o motor com calma (as abas já pediram para fechar).
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(300) {
        cef::do_message_loop_work();
        std::thread::sleep(Duration::from_millis(10));
    }
    cef::shutdown();
    let _ = std::fs::remove_file(&sock);
    if r.is_err() { 1.into() } else { 0.into() }
}
