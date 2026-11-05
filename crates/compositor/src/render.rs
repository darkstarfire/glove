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
}

fn mem(renderer: &mut GlesRenderer, loc: Point<i32, Logical>, tex: &Tex, scale: f64) -> Option<GloveElement> {
    let buf = tex.upload(renderer)?;
    // Origem = a textura inteira (em pixels); destino = tamanho lógico.
    let src = Rectangle::from_size((tex.px.0 as f64, tex.px.1 as f64).into());
    Some(GloveElement::Texture(TextureRenderElement::from_texture_buffer(
        loc.to_f64().to_physical(scale),
        &buf,
        None,
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
        if let Some(b) = &c.dock {
            out.extend(mem(renderer, c.dock_pos(), b, sc));
        }

        for w in self.space.elements().rev() {
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

        if let Some(b) = &c.desk_icon {
            out.extend(mem(renderer, self.desk_icon_rect().loc, b, sc));
        }
        if let Some(b) = &c.wallpaper {
            out.extend(mem(renderer, (0, 0).into(), b, sc));
        }
        out
    }
}

impl Glove {
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
