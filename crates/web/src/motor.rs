//! O motor: o Chromium (CEF) sem janela própria. Cada aba é um navegador que desenha a
//! página numa imagem (`on_paint`); o app copia essa imagem para a janela do glove-ui e
//! manda para o motor o mouse e o teclado. Tudo roda na linha principal: o CEF trabalha
//! quando o app chama `cef::do_message_loop_work()` (a cada tique do glove-ui).

use std::cell::{Cell, RefCell};
use std::rc::Rc as StdRc;

use cef::*;

/// O que uma aba sabe da página (escrito pelo motor, lido pelo app).
pub struct Pagina {
    /// Área da página na janela, em pontos, e a escala da tela.
    pub w: i32,
    pub h: i32,
    pub scale: f32,
    /// A imagem da página (RGBA pré-multiplicado, em pixels) e o tamanho dela.
    pub frame: Vec<u8>,
    pub fw: i32,
    pub fh: i32,
    /// Lista suspensa da página (<select>), desenhada por cima: onde e a imagem.
    pub popup: Option<cef::Rect>,
    pub popup_frame: Vec<u8>,
    pub pw: i32,
    pub ph: i32,
    /// Chegou imagem nova desde o último quadro.
    pub dirty: bool,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
    pub browser: Option<cef::Browser>,
    /// Menu de contexto pedido pela página (botão direito).
    pub ctx: Option<Contexto>,
    /// Diálogo do JavaScript esperando resposta (alert/confirm).
    pub dialog: Option<(bool, String, JsdialogCallback)>,
    /// Texto da dica (title de um link ou botão).
    pub tooltip: String,
    /// Endereço do link sob o mouse (mostrado embaixo, como nos navegadores).
    pub status: String,
}

impl Pagina {
    pub fn new(scale: f32) -> Self {
        Self { w: 800, h: 600, scale, frame: Vec::new(), fw: 0, fh: 0, popup: None, popup_frame: Vec::new(), pw: 0, ph: 0, dirty: true, title: String::new(), url: String::new(), loading: true, back: false, forward: false, browser: None, ctx: None, dialog: None, tooltip: String::new(), status: String::new() }
    }
}

/// O que o botão direito pegou na página.
#[derive(Clone, Debug, Default)]
pub struct Contexto {
    pub x: i32,
    pub y: i32,
    pub link: String,
    pub selection: String,
    pub editable: bool,
    pub image: String,
}

pub type Shared = StdRc<RefCell<Pagina>>;

thread_local! {
    /// Endereços que pediram aba nova (target=_blank, window.open).
    pub static NOVAS_ABAS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Quando o motor quer trabalhar de novo (ms; o CEF avisa).
    pub static BOMBA: Cell<i64> = const { Cell::new(0) };
    /// Avisos para a barra de baixo (downloads).
    pub static AVISOS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn s(v: Option<&CefString>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

/// BGRA (do motor) para RGBA (do glove-paint), os dois pré-multiplicados.
fn bgra_to_rgba(src: &[u8], dst: &mut Vec<u8>) {
    dst.resize(src.len(), 0);
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = s[3];
    }
}

// ---------------------------------------------------------------------------
// App (opções do motor) e o processo do navegador
// ---------------------------------------------------------------------------

wrap_app! {
    pub struct MotorApp;

    impl App {
        fn on_before_command_line_processing(&self, process_type: Option<&CefString>, command_line: Option<&mut CommandLine>) {
            // Só no processo principal (os outros herdam o que precisam).
            if process_type.is_some_and(|t| !t.to_string().is_empty()) {
                return;
            }
            let Some(cl) = command_line else { return };
            let sw = |cl: &mut CommandLine, n: &str| cl.append_switch(Some(&n.into()));
            let val = |cl: &mut CommandLine, n: &str, v: &str| cl.append_switch_with_value(Some(&n.into()), Some(&v.into()));
            // Leve para máquinas fracas: sem o processo da placa de vídeo (a página é
            // desenhada pelo processador e copiada para a janela); poucos processos de
            // página; sem o cache de voltar (páginas inteiras guardadas na memória).
            if std::env::var_os("GLOVE_WEB_GPU").is_none() {
                sw(cl, "disable-gpu");
                sw(cl, "disable-gpu-compositing");
            }
            val(cl, "renderer-process-limit", "4");
            sw(cl, "process-per-site");
            val(cl, "disable-features", "BackForwardCache,Translate,MediaRouter,OptimizationHints,OptimizationGuideModelDownloading,InterestFeedContentSuggestions");
            sw(cl, "disable-background-networking");
            sw(cl, "disable-component-update");
            sw(cl, "no-first-run");
            sw(cl, "no-default-browser-check");
            sw(cl, "disable-default-apps");
            // Sem X11 à vista: o motor não precisa de tela nenhuma.
            if std::env::var_os("DISPLAY").is_none() {
                val(cl, "ozone-platform", "headless");
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Processo::new())
        }
    }
}

wrap_browser_process_handler! {
    pub struct Processo;

    impl BrowserProcessHandler {
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            BOMBA.with(|b| b.set(delay_ms.max(0)));
        }
    }
}

pub fn app() -> cef::App {
    MotorApp::new()
}

// ---------------------------------------------------------------------------
// Uma aba: os handlers do cliente, todos com a mesma página
// ---------------------------------------------------------------------------

wrap_render_handler! {
    pub struct Desenho {
        p: Shared,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut cef::Rect>) {
            if let (Some(r), Ok(p)) = (rect, self.p.try_borrow()) {
                r.x = 0;
                r.y = 0;
                r.width = p.w.max(1);
                r.height = p.h.max(1);
            }
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            let (Some(info), Ok(p)) = (info, self.p.try_borrow()) else { return 0 };
            info.device_scale_factor = p.scale;
            1
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            if let Ok(mut p) = self.p.try_borrow_mut() {
                if show == 0 {
                    p.popup = None;
                    p.popup_frame.clear();
                }
                p.dirty = true;
            }
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&cef::Rect>) {
            if let (Some(r), Ok(mut p)) = (rect, self.p.try_borrow_mut()) {
                p.popup = Some(r.clone());
            }
        }

        fn on_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, _dirty_rects: Option<&[cef::Rect]>, buffer: *const u8, width: ::std::os::raw::c_int, height: ::std::os::raw::c_int) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let Ok(mut p) = self.p.try_borrow_mut() else { return };
            // Segurança: o motor garante o buffer (largura × altura × 4) durante a chamada.
            let src = unsafe { std::slice::from_raw_parts(buffer, (width * height * 4) as usize) };
            if type_ == PaintElementType::POPUP {
                let mut f = std::mem::take(&mut p.popup_frame);
                bgra_to_rgba(src, &mut f);
                p.popup_frame = f;
                p.pw = width;
                p.ph = height;
            } else {
                let mut f = std::mem::take(&mut p.frame);
                bgra_to_rgba(src, &mut f);
                p.frame = f;
                p.fw = width;
                p.fh = height;
            }
            p.dirty = true;
        }
    }
}

wrap_display_handler! {
    pub struct Exibicao {
        p: Shared,
    }

    impl DisplayHandler {
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_some_and(|f| f.is_main() != 0) {
                if let Ok(mut p) = self.p.try_borrow_mut() {
                    p.url = s(url);
                    p.dirty = true;
                }
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.title = s(title);
                p.dirty = true;
            }
        }

        fn on_tooltip(&self, _browser: Option<&mut Browser>, text: Option<&mut CefString>) -> ::std::os::raw::c_int {
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.tooltip = text.map(|t| t.to_string()).unwrap_or_default();
                p.dirty = true;
            }
            1
        }

        fn on_status_message(&self, _browser: Option<&mut Browser>, value: Option<&CefString>) {
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.status = s(value);
                p.dirty = true;
            }
        }
    }
}

wrap_load_handler! {
    pub struct Carga {
        p: Shared,
    }

    impl LoadHandler {
        fn on_loading_state_change(&self, _browser: Option<&mut Browser>, is_loading: ::std::os::raw::c_int, can_go_back: ::std::os::raw::c_int, can_go_forward: ::std::os::raw::c_int) {
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.loading = is_loading != 0;
                p.back = can_go_back != 0;
                p.forward = can_go_forward != 0;
                p.dirty = true;
            }
        }
    }
}

wrap_life_span_handler! {
    pub struct Vida {
        p: Shared,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let (Some(b), Ok(mut p)) = (browser, self.p.try_borrow_mut()) {
                p.browser = Some(b.clone());
            }
        }

        /// Janela nova pedida pela página: vira uma aba.
        fn on_before_popup(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, _popup_id: ::std::os::raw::c_int, target_url: Option<&CefString>, _target_frame_name: Option<&CefString>, _target_disposition: WindowOpenDisposition, _user_gesture: ::std::os::raw::c_int, _popup_features: Option<&PopupFeatures>, _window_info: Option<&mut WindowInfo>, _client: Option<&mut Option<Client>>, _settings: Option<&mut BrowserSettings>, _extra_info: Option<&mut Option<DictionaryValue>>, _no_javascript_access: Option<&mut ::std::os::raw::c_int>) -> ::std::os::raw::c_int {
            let url = s(target_url);
            if !url.is_empty() {
                NOVAS_ABAS.with(|n| n.borrow_mut().push(url));
            }
            1
        }
    }
}

wrap_context_menu_handler! {
    pub struct MenuPagina {
        p: Shared,
    }

    impl ContextMenuHandler {
        /// O menu do motor fica vazio: o Glove mostra o dele (solto, por cima de tudo).
        fn on_before_context_menu(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, params: Option<&mut ContextMenuParams>, model: Option<&mut MenuModel>) {
            if let Some(m) = model {
                m.clear();
            }
            let Some(params) = params else { return };
            let st = |u: CefStringUserfree| CefString::from(&u).to_string();
            let c = Contexto {
                x: params.xcoord(),
                y: params.ycoord(),
                link: st(params.link_url()),
                selection: st(params.selection_text()),
                editable: params.is_editable() != 0,
                image: if params.has_image_contents() != 0 { st(params.source_url()) } else { String::new() },
            };
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.ctx = Some(c);
                p.dirty = true;
            }
        }
    }
}

wrap_download_handler! {
    pub struct Baixar {
        p: Shared,
    }

    impl DownloadHandler {
        fn can_download(&self, _browser: Option<&mut Browser>, _url: Option<&CefString>, _request_method: Option<&CefString>) -> ::std::os::raw::c_int {
            1
        }

        /// Vai direto para a pasta Downloads (sem perguntar), com um nome livre.
        fn on_before_download(&self, _browser: Option<&mut Browser>, _download_item: Option<&mut DownloadItem>, suggested_name: Option<&CefString>, callback: Option<&mut BeforeDownloadCallback>) -> ::std::os::raw::c_int {
            let Some(cb) = callback else { return 0 };
            let name = s(suggested_name);
            let name = if name.is_empty() { "download".to_string() } else { name.replace('/', "_") };
            let path = livre(&crate::pasta_downloads().join(&name));
            AVISOS.with(|a| a.borrow_mut().push(glove_config::tr!("Baixando {}…", name)));
            cb.cont(Some(&path.to_string_lossy().as_ref().into()), 0);
            1
        }

        fn on_download_updated(&self, _browser: Option<&mut Browser>, download_item: Option<&mut DownloadItem>, _callback: Option<&mut DownloadItemCallback>) {
            let Some(d) = download_item else { return };
            if d.is_complete() != 0 {
                let path = CefString::from(&d.full_path()).to_string();
                let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path);
                AVISOS.with(|a| a.borrow_mut().push(glove_config::tr!("Baixado em Downloads: {}", name)));
                if let Ok(mut p) = self.p.try_borrow_mut() {
                    p.dirty = true;
                }
            }
        }
    }
}

wrap_jsdialog_handler! {
    pub struct Dialogo {
        p: Shared,
    }

    impl JsdialogHandler {
        /// alert() e confirm() viram alertas do Glove (em janela própria); prompt() é recusado.
        fn on_jsdialog(&self, _browser: Option<&mut Browser>, _origin_url: Option<&CefString>, dialog_type: JsdialogType, message_text: Option<&CefString>, _default_prompt_text: Option<&CefString>, callback: Option<&mut JsdialogCallback>, suppress_message: Option<&mut ::std::os::raw::c_int>) -> ::std::os::raw::c_int {
            let Some(cb) = callback else { return 0 };
            if dialog_type == JsdialogType::PROMPT {
                if let Some(sup) = suppress_message {
                    *sup = 1;
                }
                return 0;
            }
            if let Ok(mut p) = self.p.try_borrow_mut() {
                p.dialog = Some((dialog_type == JsdialogType::CONFIRM, s(message_text), cb.clone()));
                p.dirty = true;
                return 1;
            }
            0
        }

        fn on_before_unload_dialog(&self, _browser: Option<&mut Browser>, _message_text: Option<&CefString>, _is_reload: ::std::os::raw::c_int, callback: Option<&mut JsdialogCallback>) -> ::std::os::raw::c_int {
            // Sair da página sempre pode (como fechar a aba).
            if let Some(cb) = callback {
                cb.cont(1, None);
                return 1;
            }
            0
        }
    }
}

wrap_client! {
    pub struct Cliente {
        desenho: RenderHandler,
        exibicao: DisplayHandler,
        carga: LoadHandler,
        vida: LifeSpanHandler,
        menu: ContextMenuHandler,
        baixar: DownloadHandler,
        dialogo: JsdialogHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.desenho.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.exibicao.clone())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.carga.clone())
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.vida.clone())
        }
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.menu.clone())
        }
        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(self.baixar.clone())
        }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> {
            Some(self.dialogo.clone())
        }
    }
}

/// Um nome que ainda não existe ("arquivo (2).zip").
fn livre(path: &std::path::Path) -> std::path::PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..1000).map(|n| path.with_file_name(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or_else(|| path.to_path_buf())
}

/// Abre uma aba nova no motor (o navegador chega depois, em `on_after_created`).
pub fn nova_aba(p: &Shared, url: &str) -> bool {
    let window_info = WindowInfo { windowless_rendering_enabled: 1, runtime_style: RuntimeStyle::ALLOY, ..Default::default() };
    let settings = BrowserSettings { windowless_frame_rate: 60, background_color: 0xffff_ffff, ..Default::default() };
    let mut client = Cliente::new(Desenho::new(p.clone()), Exibicao::new(p.clone()), Carga::new(p.clone()), Vida::new(p.clone()), MenuPagina::new(p.clone()), Baixar::new(p.clone()), Dialogo::new(p.clone()));
    browser_host_create_browser(Some(&window_info), Some(&mut client), Some(&url.into()), Some(&settings), None, None) == 1
}
