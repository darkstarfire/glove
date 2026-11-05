//! Monta a lista de elementos do quadro (da frente para o fundo) e desenha.
//! O rastreador de danos só redesenha os pedaços que mudaram.

use std::cell::RefCell;
use std::time::Duration;

use glove_paint::theme::TITLE_H;
use smithay::{
    backend::allocator::Fourcc,
    backend::renderer::{
        damage::OutputDamageTracker,
        gles::GlesTexture,
        Bind, ExportMem, Offscreen,
        element::{
            texture::TextureRenderElement,
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
            solid::{SolidColorRenderElement},
            utils::RescaleRenderElement,
            AsRenderElements, Kind,
        },
        gles::GlesRenderer,
    },
    render_elements,
    utils::{Logical, Point, Rectangle, Scale, Transform},
};

use crate::{shell::{Deco, Tex}, state::Glove, Backend, CalloopData, WinitBackend};

render_elements! {
    pub GloveElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Texture=TextureRenderElement<GlesTexture>,
    Scaled=RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>,
    ScaledTexture=RescaleRenderElement<TextureRenderElement<GlesTexture>>,
    Solid=SolidColorRenderElement,
}

fn mem(renderer: &mut GlesRenderer, loc: Point<i32, Logical>, tex: &Tex, scale: f64) -> Option<GloveElement> {
    mem_alpha(renderer, loc, tex, scale, None)
}

fn mem_alpha(renderer: &mut GlesRenderer, loc: Point<i32, Logical>, tex: &Tex, scale: f64, alpha: Option<f32>) -> Option<GloveElement> {
    let buf = tex.upload(renderer)?;
    // Origem = a textura inteira (em pixels); destino = tamanho lógico.
    let src = Rectangle::from_size((tex.px.0 as f64, tex.px.1 as f64).into());
    Some(GloveElement::Texture(TextureRenderElement::from_texture_buffer(
        loc.to_f64().to_physical(scale),
        &buf,
        alpha,
        Some(src),
        Some(tex.size),
        Kind::Unspecified,
    )))
}

impl Glove {
    /// Elementos do quadro, da frente para o fundo. `cursor`: desenhar o cursor
    /// (na sessão própria; dentro de outro desktop o cursor é o dele).
    pub fn build_elements(&self, renderer: &mut GlesRenderer, cursor: bool) -> Vec<GloveElement> {
        let mut out = Vec::new();
        let c = &self.chrome;
        let sc = self.scale();

        if cursor {
            self.cursor_elements(renderer, sc, &mut out);
        }

        // Tela de login: só as janelas (o app de login) sobre o papel de parede.
        if self.greeter {
            if !self.idle.screen_off {
                for w in self.space.elements().rev() {
                    let Some(loc) = self.space.element_location(w) else { continue };
                    let render_loc = loc - w.geometry().loc;
                    out.extend(w.render_elements::<GloveElement>(renderer, render_loc.to_physical_precise_round(sc), Scale::from(sc), 1.0));
                }
                if let Some(b) = &c.wallpaper {
                    out.extend(mem(renderer, (0, 0).into(), b, sc));
                }
            }
            return out;
        }

        // Troca de apps por cima de tudo (menos o cursor).
        if let (Some(s), Some((x, y))) = (&self.switcher, self.switcher_pos()) {
            if let Some(t) = &s.tex {
                out.extend(mem(renderer, (x, y).into(), t, sc));
            }
        }
        // Tela apagada: nada (na sessão própria a tela nem recebe quadros).
        if self.idle.screen_off {
            return out;
        }
        // Bloqueada: só o painel de senha sobre o papel de parede; nenhuma janela.
        if let Some(l) = &self.lock {
            if let Some(t) = &l.tex {
                let (x, y) = self.lock_panel_pos();
                let m = glove_paint::chrome::LOCK_SHADOW as i32;
                out.extend(mem(renderer, (x - m, y - m).into(), t, sc));
            }
            if let Some(b) = &c.wallpaper {
                out.extend(mem(renderer, (0, 0).into(), b, sc));
            }
            return out;
        }

        // Iniciando sessão: o painel por cima; enquanto cobre, só o papel de parede embaixo.
        if let Some(s) = &self.startup {
            if let Some(t) = &s.tex {
                let (x, y) = self.startup_pos();
                let m = glove_paint::chrome::STARTUP_SHADOW as i32;
                out.extend(mem_alpha(renderer, (x - m, y - m).into(), t, sc, Some(s.alpha())));
            }
            if s.covering() {
                if let Some(b) = &c.wallpaper {
                    out.extend(mem(renderer, (0, 0).into(), b, sc));
                }
                return out;
            }
        }

        // Tela de encerramento: cobre tudo (menos o cursor).
        if let Some(e) = self.ending.as_ref().filter(|e| e.screen_since.is_some()) {
            let (w, h) = c.out_size;
            let (cx, cy) = glove_paint::chrome::ending_spinner_center(w as f32, h as f32);
            let half = glove_paint::chrome::ENDING_SPINNER / 2.0;
            if let Some(t) = &e.spinner {
                out.extend(mem(renderer, ((cx - half).round() as i32, (cy - half).round() as i32).into(), t, sc));
            }
            if let Some(t) = &e.bg {
                out.extend(mem(renderer, (0, 0).into(), t, sc));
            }
            return out;
        }

        // Aviso de volume/brilho por cima de tudo (menos o cursor).
        if let Some(t) = self.extras.osd.as_ref().and_then(|o| o.tex.as_ref()) {
            out.extend(mem(renderer, self.osd_pos(), t, sc));
        }
        if self.search.open {
            if let Some(t) = &self.search.tex {
                let o = self.search_origin();
                out.extend(mem(renderer, (o.x - glove_paint::chrome::MENU_SHADOW, o.y).into(), t, sc));
            }
        }
        if let Some(s) = &self.extras.slider {
            if let Some(t) = &s.tex {
                out.extend(mem(renderer, (s.origin.x - glove_paint::chrome::MENU_SHADOW, s.origin.y).into(), t, sc));
            }
        }
        if let Some(m) = &c.menu {
            // Submenus por cima do menu que os abriu.
            for lv in m.levels.iter().rev() {
                if let Some(b) = &lv.buffer {
                    out.extend(mem(renderer, lv.buffer_pos(), b, sc));
                }
            }
        }
        if let Some(fq) = &self.force_quit {
            if let Some(t) = &fq.tex {
                let m = glove_paint::chrome::FQ_SHADOW as i32;
                out.extend(mem(renderer, (fq.origin.x - m, fq.origin.y - m).into(), t, sc));
            }
        }
        if let Some(b) = &c.menubar {
            out.extend(mem(renderer, (0, 0).into(), b, sc));
        }
        // Janelas indo para o Dock ou voltando: por cima de tudo isso.
        for a in &self.min_anims {
            if let Some(r) = self.min_anim_rect(&a.window) {
                self.push_scaled_window(renderer, &a.window, r, sc, &mut out);
            }
        }
        // Miniaturas vivas das minimizadas, com o ícone do app no canto.
        for (w, (x, y), t) in &self.badge_places {
            if !self.min_animating(w) {
                out.extend(mem(renderer, (*x, *y).into(), t, sc));
            }
        }
        for (w, _) in &self.minimized {
            if self.min_animating(w) {
                continue;
            }
            if let Some(r) = self.thumbnail_rect(w) {
                self.push_scaled_window(renderer, w, r, sc, &mut out);
            }
        }
        if let Some(b) = &c.dock {
            out.extend(mem(renderer, c.dock_pos(), b, sc));
        }

        if self.expose.is_some() {
            self.expose_elements(renderer, sc, &mut out);
        } else {
        for w in self.space.elements().rev() {
            // Voltando do Dock: desenhada pela animação, acima.
            if self.min_animating(w) {
                continue;
            }
            let Some(loc) = self.space.element_location(w) else { continue };
            let render_loc = loc - w.geometry().loc;
            out.extend(w.render_elements::<GloveElement>(renderer, render_loc.to_physical_precise_round(sc), Scale::from(sc), 1.0));
            if let Some(deco) = w.user_data().get::<RefCell<Deco>>() {
                let d = deco.borrow();
                let top = (loc.x, loc.y - TITLE_H);
                if let Some(t) = &d.title {
                    out.extend(mem(renderer, top.into(), t, sc));
                }
                for (sx, sy, s) in &d.shadows {
                    out.extend(mem(renderer, (top.0 + sx, top.1 + sy).into(), s, sc));
                }
            }
        }
        }

        for (i, b) in c.desk_texs.iter().enumerate() {
            out.extend(mem(renderer, self.desk_icon_rect(i).loc, b, sc));
        }
        if let Some(b) = &c.wallpaper {
            out.extend(mem(renderer, (0, 0).into(), b, sc));
        }
        out
    }
}

impl Glove {
    /// Janelas no Exposé: encolhidas e movidas conforme a animação; fundo escurecido.
    fn expose_elements(&self, renderer: &mut GlesRenderer, sc: f64, out: &mut Vec<GloveElement>) {
        use smithay::backend::renderer::element::Kind;
        let Some(e) = &self.expose else { return };
        // Título da janela sob o mouse, no meio dela.
        if let (Some((i, t)), Some(_)) = (&e.label, e.hover) {
            let r = e.current(*i);
            let x = (r.loc.x + (r.size.w - t.size.w as f64) / 2.0).round() as i32;
            let y = (r.loc.y + r.size.h / 2.0 - t.size.h as f64 / 2.0).round() as i32;
            out.extend(mem(renderer, (x, y).into(), t, sc));
        }
        for w in self.space.elements().rev() {
            let Some(i) = e.item_of(w) else { continue };
            self.push_scaled_window(renderer, w, e.current(i), sc, out);
        }
        // Fundo escurecido (menos no modo mesa); no modo app, as outras janelas ficam embaixo dele.
        if e.mode != crate::expose::ExposeMode::Desktop {
            out.push(GloveElement::Solid(SolidColorRenderElement::from_buffer(&e.dim, (0, 0), sc, 1.0, Kind::Unspecified)));
            for w in self.space.elements().rev() {
                if e.item_of(w).is_some() {
                    continue;
                }
                let Some(loc) = self.space.element_location(w) else { continue };
                let render_loc = loc - w.geometry().loc;
                out.extend(w.render_elements::<GloveElement>(renderer, render_loc.to_physical_precise_round(sc), Scale::from(sc), 1.0));
                if let Some(deco) = w.user_data().get::<RefCell<Deco>>() {
                    if let Some(t) = &deco.borrow().title {
                        out.extend(mem(renderer, (loc.x, loc.y - TITLE_H).into(), t, sc));
                    }
                }
            }
        }
    }

    /// Desenha a janela (com a barra de título) encolhida para caber em `r`
    /// (a moldura inteira, na mesma proporção).
    fn push_scaled_window(&self, renderer: &mut GlesRenderer, w: &smithay::desktop::Window, r: Rectangle<f64, Logical>, sc: f64, out: &mut Vec<GloveElement>) {
        use smithay::backend::renderer::element::Kind;
        let frame_w = w.geometry().size.w as f64;
        if frame_w <= 0.0 {
            return;
        }
        let s = (r.size.w / frame_w).max(0.01);
        let top = crate::shell::deco_top(w) as f64;
        // Conteúdo: desenhado num ponto P e encolhido em volta dele.
        let geo = w.geometry().loc.to_f64();
        let px = r.loc.x - geo.x * s;
        let py = r.loc.y + top * s - geo.y * s;
        let origin = Point::<f64, Logical>::from((px, py)).to_physical(sc).to_i32_round();
        let surf = w.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(renderer, origin, Scale::from(sc), 1.0);
        out.extend(surf.into_iter().map(|el| GloveElement::Scaled(RescaleRenderElement::from_element(el, origin, s))));
        if let Some(deco) = w.user_data().get::<RefCell<Deco>>() {
            if let Some(t) = &deco.borrow().title {
                let o = Point::<f64, Logical>::from((r.loc.x, r.loc.y)).to_physical(sc).to_i32_round();
                if let Some(buf) = t.upload(renderer) {
                    let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
                    let el = TextureRenderElement::from_texture_buffer(o.to_f64(), &buf, None, Some(src), Some(t.size), Kind::Unspecified);
                    out.push(GloveElement::ScaledTexture(RescaleRenderElement::from_element(el, o, s)));
                }
            }
        }
    }

    fn cursor_elements(&self, renderer: &mut GlesRenderer, sc: f64, out: &mut Vec<GloveElement>) {
        use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
        let pos = self.pointer_pos;
        // App sem responder sob o mouse: cursor de espera, girando.
        if self.wait_cursor.shown {
            if let Some(t) = &self.wait_cursor.tex {
                let half = glove_paint::chrome::WAIT_CURSOR_SIZE as f64 / 2.0;
                let loc = Point::<f64, Logical>::from((pos.x - half, pos.y - half));
                if let Some(buf) = t.upload(renderer) {
                    let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
                    out.push(GloveElement::Texture(TextureRenderElement::from_texture_buffer(loc.to_physical(sc), &buf, None, Some(src), Some(t.size), Kind::Cursor)));
                }
                return;
            }
        }
        match &self.cursor_status {
            CursorImageStatus::Hidden => {}
            CursorImageStatus::Surface(surface) => {
                let hotspot = smithay::wayland::compositor::with_states(surface, |s| {
                    s.data_map.get::<CursorImageSurfaceData>().map(|d| d.lock().unwrap().hotspot).unwrap_or_default()
                });
                let loc = (pos - hotspot.to_f64()).to_physical(sc).to_i32_round();
                out.extend(
                    render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<GlesRenderer>>(renderer, surface, loc, sc, 1.0, Kind::Cursor)
                        .into_iter()
                        .map(GloveElement::Surface),
                );
            }
            CursorImageStatus::Named(_) => {
                if let Some(t) = &self.cursor_tex {
                    let (hx, hy) = glove_paint::chrome::CURSOR_HOTSPOT;
                    let loc = Point::<f64, Logical>::from((pos.x - hx as f64, pos.y - hy as f64));
                    if let Some(buf) = t.upload(renderer) {
                        let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
                        out.push(GloveElement::Texture(TextureRenderElement::from_texture_buffer(
                            loc.to_physical(sc),
                            &buf,
                            None,
                            Some(src),
                            Some(t.size),
                            Kind::Cursor,
                        )));
                    }
                }
            }
        }
    }
}

pub fn render_frame(data: &mut CalloopData) {
    let CalloopData { state, backend, .. } = data;
    let Backend::Winit(WinitBackend { backend, output, damage_tracker }) = backend else { return };
    state.dirty = false;
    state.prepare_chrome();

    let age = backend.buffer_age().unwrap_or(0);
    let damage = {
        let (renderer, mut fb) = match backend.bind() {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("bind falhou: {e:?}");
                return;
            }
        };
        let elements = state.build_elements(renderer, false);
        match damage_tracker.render_output(renderer, &mut fb, age, &elements, [0.0, 0.0, 0.0, 1.0]) {
            Ok(r) => r.damage.cloned(),
            Err(e) => {
                tracing::warn!("render falhou: {e:?}");
                None
            }
        }
    };
    if let Some(d) = damage {
        if !d.is_empty() {
            if let Err(e) = backend.submit(Some(&d)) {
                tracing::warn!("submit falhou: {e:?}");
            }
        }
    }

    let output = output.clone();
    after_frame(state, backend.renderer(), &output);
}

/// Depois de cada quadro: captura de tela pedida e aviso de quadro aos apps.
pub fn after_frame(state: &mut Glove, renderer: &mut GlesRenderer, output: &smithay::output::Output) {
    if let Some(path) = state.screenshot_request.take() {
        match screenshot(state, renderer, &path) {
            Ok(()) => {
                tracing::info!("captura salva em {}", path.display());
                if std::mem::take(&mut state.open_after_shot) {
                    state.spawn(&format!("glove-view '{}'", path.display().to_string().replace('\'', "'\\''")));
                }
            }
            Err(e) => tracing::warn!("captura falhou: {e}"),
        }
    }

    let time = state.start_time.elapsed();
    for w in state.space.elements() {
        w.send_frame(output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
    }
    // Minimizadas (miniatura viva no Dock): quadros no máximo 2×/s. Se for cedo,
    // agenda um quadro para quando der (só acontece se algo foi redesenhado).
    if !state.minimized.is_empty() && state.min_commit_pending {
        let since = state.min_frame_last.elapsed();
        if since >= crate::minimize::MINIMIZED_FRAME {
            state.min_frame_last = std::time::Instant::now();
            state.min_commit_pending = false;
            for (w, _) in &state.minimized {
                w.send_frame(output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
            }
        } else if !state.min_frame_timer {
            state.min_frame_timer = true;
            let left = crate::minimize::MINIMIZED_FRAME - since;
            let _ = state.loop_handle.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(left), |_, _, data| {
                data.state.min_frame_timer = false;
                data.state.dirty = true;
                smithay::reexports::calloop::timer::TimeoutAction::Drop
            });
        }
    }
    if let smithay::input::pointer::CursorImageStatus::Surface(s) = &state.cursor_status {
        smithay::desktop::utils::send_frames_surface_tree(s, output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
    }
}

/// Desenha a tela inteira numa textura fora da tela e salva como PNG.
fn screenshot(state: &Glove, renderer: &mut GlesRenderer, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let (lw, lh) = state.output_size();
    let sc = state.scale();
    let (w, h) = ((lw as f64 * sc).round() as i32, (lh as f64 * sc).round() as i32);
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (w, h).into())?;
    let elements = state.build_elements(renderer, false);
    let mut fb = renderer.bind(&mut tex)?;
    let mut dt = OutputDamageTracker::new((w, h), sc, Transform::Normal);
    dt.render_output(renderer, &mut fb, 0, &elements, [0.0, 0.0, 0.0, 1.0])?;
    let mapping = renderer.copy_framebuffer(&fb, Rectangle::from_size((w, h).into()), Fourcc::Abgr8888)?;
    let data = renderer.map_texture(&mapping)?.to_vec();
    let size = glove_paint::tiny_skia::IntSize::from_wh(w as u32, h as u32).ok_or("tamanho inválido")?;
    let pm = glove_paint::Pixmap::from_vec(data, size).ok_or("dados inválidos")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    pm.save_png(path)?;
    Ok(())
}
