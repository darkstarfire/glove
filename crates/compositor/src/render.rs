//! Monta a lista de elementos do quadro (da frente para o fundo) e desenha.
//! O rastreador de danos só redesenha os pedaços que mudaram.

use std::cell::RefCell;
use std::time::Duration;

use glove_paint::theme::TITLE_H;
use smithay::{
    backend::allocator::Fourcc,
    backend::renderer::{
        damage::OutputDamageTracker,
        element::Id,
        gles::{element::TextureShaderElement, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType},
        Renderer,
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
    Shader=TextureShaderElement,
}

/// Efeitos do Acesso Universal: escala de cinza, contraste e cores invertidas,
/// aplicados à tela inteira já desenhada (num passo extra).
const EFFECT_SHADER: &str = r#"#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision mediump float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

uniform float invert;
uniform float gray;
uniform float contrast;

void main() {
    vec3 c = texture2D(tex, v_coords).rgb;
    if (gray > 0.5) {
        float l = dot(c, vec3(0.299, 0.587, 0.114));
        c = vec3(l);
    }
    c = clamp((c - 0.5) * (1.0 + contrast * 1.5) + 0.5, 0.0, 1.0);
    if (invert > 0.5) {
        c = vec3(1.0) - c;
    }
    gl_FragColor = vec4(c, 1.0) * alpha;
}
"#;

/// Textura fora da tela e o programa do efeito (guardados entre quadros).
#[derive(Default)]
pub struct EffectCache {
    tex: Option<(GlesTexture, (i32, i32))>,
    program: Option<GlesTexProgram>,
    /// Programa da Lâmpada Mágica.
    genie: Option<GlesTexProgram>,
}

/// Lâmpada Mágica: para cada ponto da área da animação, de onde ele vem na foto
/// da janela. `fr`/`sl` = janela e ícone (x, y, largura, altura, em pontos da área
/// `bx`); `anim` = (quanto os lados já curvaram, quanto a janela já escorreu).
const GENIE_SHADER: &str = r#"#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

uniform vec4 fr;
uniform vec4 sl;
uniform vec2 anim;
uniform vec2 bx;

void main() {
    vec2 p = v_coords * bx;
    float bottom = sl.y + sl.w;
    float top = fr.y + anim.y * (bottom - fr.y);
    // Linha da janela neste ponto (ela só escorre: a altura não muda).
    float v = (p.y - top) / fr.w;
    // Borda do funil nesta altura: da janela até o ícone, em "S".
    float f = smoothstep(0.0, 1.0, (p.y - fr.y) / max(sl.y - fr.y, 1.0));
    float l = mix(fr.x, mix(fr.x, sl.x, f), anim.x);
    float wd = mix(fr.z, mix(fr.z, sl.z, f), anim.x);
    float u = (p.x - l) / max(wd, 0.5);
    // Bordas suaves (meio ponto de cada lado) e nada abaixo do ícone.
    float edge = clamp(min(p.x - l, l + wd - p.x) + 0.5, 0.0, 1.0)
        * clamp(min(p.y - top, top + fr.w - p.y) + 0.5, 0.0, 1.0)
        * clamp(bottom - p.y + 0.5, 0.0, 1.0);
    if (edge <= 0.0 || u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0) {
        gl_FragColor = vec4(0.0);
        return;
    }
    gl_FragColor = texture2D(tex, vec2(u, v)) * alpha * edge;
}
"#;

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
    /// Um monitor extra (mesa estendida): o cursor, as janelas que passam por
    /// ele e o papel de parede. Barra de menu e Dock só no principal.
    pub fn build_extra_elements(&self, renderer: &mut GlesRenderer, area: Rectangle<i32, Logical>, sc: f64) -> Vec<GloveElement> {
        self.build_area_elements(renderer, area, sc, true)
    }

    /// O que aparece na área (de um monitor extra): `cursor` = com o mouse e a película
    /// da captura de área (a foto de uma área vai sem eles).
    pub fn build_area_elements(&self, renderer: &mut GlesRenderer, area: Rectangle<i32, Logical>, sc: f64, cursor: bool) -> Vec<GloveElement> {
        let mut out = Vec::new();
        let origin = area.loc;
        if self.saver.is_some() || self.idle.screen_off {
            return out;
        }
        if cursor && area.to_f64().contains(self.pointer_pos) {
            self.cursor_elements_at(renderer, sc, origin, &mut out);
        }
        if cursor {
            self.region_elements_at(sc, origin, (area.size.w, area.size.h), &mut out);
        }
        // Bloqueada ou encerrando: só o papel de parede (nada das janelas).
        let covered = self.lock.is_some() || self.greeter || self.ending.as_ref().is_some_and(|e| e.screen_since.is_some()) || self.startup.as_ref().is_some_and(|s| s.covering());
        if !covered {
            for w in self.x11_overlays.iter().rev() {
                let Some(x) = w.x11_surface() else { continue };
                let g = x.geometry();
                if g.overlaps(area) {
                    out.extend(w.render_elements::<GloveElement>(renderer, (g.loc - origin).to_physical_precise_round(sc), Scale::from(sc), 1.0));
                }
            }
            for w in self.space.elements().rev() {
                if self.min_animating(w) {
                    continue;
                }
                let Some(geo) = self.space.element_geometry(w) else { continue };
                let framed = Rectangle::new((geo.loc.x - 30, geo.loc.y - TITLE_H - 30).into(), (geo.size.w + 60, geo.size.h + TITLE_H + 60).into());
                if !framed.overlaps(area) {
                    continue;
                }
                let render_loc = geo.loc - w.geometry().loc - origin;
                out.extend(w.render_elements::<GloveElement>(renderer, render_loc.to_physical_precise_round(sc), Scale::from(sc), 1.0));
                if let Some(deco) = w.user_data().get::<RefCell<Deco>>() {
                    let d = deco.borrow();
                    let top = (geo.loc.x - origin.x, geo.loc.y - TITLE_H - origin.y);
                    if let Some(t) = &d.title {
                        out.extend(mem(renderer, top.into(), t, sc));
                    }
                    for (sx, sy, s) in &d.shadows {
                        out.extend(mem(renderer, (top.0 + sx, top.1 + sy).into(), s, sc));
                    }
                }
            }
        }
        // Papel de parede esticado para esta tela.
        if let Some(t) = &self.chrome.wallpaper {
            if let Some(buf) = t.upload(renderer) {
                let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
                out.push(GloveElement::Texture(TextureRenderElement::from_texture_buffer((0.0, 0.0), &buf, None, Some(src), Some(area.size), Kind::Unspecified)));
            }
        }
        out
    }

    /// Elementos do quadro, da frente para o fundo. `cursor`: desenhar o cursor
    /// (na sessão própria; dentro de outro desktop o cursor é o dele).
    pub fn build_elements(&self, renderer: &mut GlesRenderer, cursor: bool) -> Vec<GloveElement> {
        let scene = self.build_scene(renderer, cursor);
        if !self.config.access.screen_effects() || self.greeter {
            return scene;
        }
        match self.apply_effects(renderer, &scene) {
            Some(e) => vec![e],
            None => scene,
        }
    }

    /// Acesso Universal: desenha a cena numa textura e devolve um elemento só,
    /// com o zoom (um recorte em volta do mouse esticado na tela) e o efeito de cor.
    fn apply_effects(&self, renderer: &mut GlesRenderer, scene: &[GloveElement]) -> Option<GloveElement> {
        let a = &self.config.access;
        let (lw, lh) = self.output_size();
        let sc = self.scale();
        let (w, h) = ((lw as f64 * sc).round() as i32, (lh as f64 * sc).round() as i32);
        let mut cache = self.effects.borrow_mut();
        if cache.tex.as_ref().map(|t| t.1) != Some((w, h)) {
            let t: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (w, h).into()).ok()?;
            cache.tex = Some((t, (w, h)));
        }
        if cache.program.is_none() {
            let names = [UniformName::new("invert", UniformType::_1f), UniformName::new("gray", UniformType::_1f), UniformName::new("contrast", UniformType::_1f)];
            match renderer.compile_custom_texture_shader(EFFECT_SHADER, &names) {
                Ok(p) => cache.program = Some(p),
                Err(e) => {
                    tracing::warn!("efeito de tela não compilou: {e:?}");
                    return None;
                }
            }
        }
        let (mut tex, _) = cache.tex.clone()?;
        {
            let mut fb = renderer.bind(&mut tex).ok()?;
            let mut dt = OutputDamageTracker::new((w, h), sc, Transform::Normal);
            dt.render_output(renderer, &mut fb, 0, scene, [0.0, 0.0, 0.0, 1.0]).ok()?;
        }
        // Zoom: recorte do tamanho da tela ÷ nível, centrado no mouse (sem sair da tela).
        let z = if a.zoom { a.zoom_level.clamp(1.0, 8.0) as f64 } else { 1.0 };
        let (vw, vh) = (w as f64 / z, h as f64 / z);
        let (px, py) = (self.pointer_pos.x * sc, self.pointer_pos.y * sc);
        // O ponto sob o mouse fica sob o mouse (como no zoom "acompanhar o ponteiro").
        let x0 = (px - px / z).clamp(0.0, w as f64 - vw);
        let y0 = (py - py / z).clamp(0.0, h as f64 - vh);
        let src = Rectangle::new((x0, y0).into(), (vw, vh).into());
        let el = TextureRenderElement::from_static_texture(Id::new(), renderer.context_id(), (0.0, 0.0), tex, 1, Transform::Normal, None, Some(src), Some((lw, lh).into()), None, Kind::Unspecified);
        let on = |b: bool| if b { 1.0f32 } else { 0.0 };
        let uniforms = vec![Uniform::new("invert", on(a.invert)), Uniform::new("gray", on(a.grayscale)), Uniform::new("contrast", a.contrast.clamp(0.0, 1.0))];
        Some(GloveElement::Shader(TextureShaderElement::new(el, cache.program.clone()?, uniforms)))
    }

    /// Elementos da cena, da frente para o fundo (sem os efeitos de tela).
    fn build_scene(&self, renderer: &mut GlesRenderer, cursor: bool) -> Vec<GloveElement> {
        let mut out = Vec::new();
        let c = &self.chrome;
        let sc = self.scale();

        // Protetor de tela: cobre tudo (e esconde o cursor); nada por baixo precisa ser desenhado.
        if let Some(t) = self.saver.as_ref().and_then(|s| s.tex.as_ref()) {
            out.extend(mem(renderer, (0, 0).into(), t, sc));
            return out;
        }

        if cursor {
            self.cursor_elements(renderer, sc, &mut out);
        }
        // Escolhendo a área de uma captura: a película fica por cima de tudo (menos do cursor).
        self.region_elements(sc, &mut out);

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
        // Menus e dicas de apps X11: por cima das janelas, na posição que o app escolheu.
        for w in self.x11_overlays.iter().rev() {
            let Some(x) = w.x11_surface() else { continue };
            let loc = x.geometry().loc;
            out.extend(w.render_elements::<GloveElement>(renderer, loc.to_physical_precise_round(sc), Scale::from(sc), 1.0));
        }
        // Janelas indo para o Dock ou voltando: por cima de tudo isso.
        for a in &self.min_anims {
            if let Some((frame, slot, p)) = self.genie_params(&a.window) {
                self.push_genie_window(renderer, &a.window, frame, slot, p, sc, &mut out);
            } else if let Some(r) = self.min_anim_rect(&a.window) {
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

    /// Efeito Lâmpada Mágica: a foto da janela deformada pela placa de vídeo. Os lados
    /// se curvam num funil até o ícone e a janela escorre por ele (`p`: 0 = na tela,
    /// 1 = no Dock). Cada pixel acha de onde vem na janela (curvas lisas, sem degraus).
    fn push_genie_window(&self, renderer: &mut GlesRenderer, w: &smithay::desktop::Window, frame: Rectangle<f64, Logical>, slot: Rectangle<f64, Logical>, p: f64, sc: f64, out: &mut Vec<GloveElement>) {
        use smithay::backend::renderer::element::Kind;
        let Some(a) = self.min_anims.iter().find(|a| &a.window == w) else { return };
        // A foto da janela (barra de título + conteúdo), uma vez só por animação.
        if a.snapshot.borrow().is_none() {
            let (tw, th) = ((frame.size.w * sc).round() as i32, (frame.size.h * sc).round() as i32);
            if tw <= 0 || th <= 0 {
                return;
            }
            let Ok(mut tex) = renderer.create_buffer(Fourcc::Abgr8888, (tw, th).into()) else { return };
            let mut els: Vec<GloveElement> = Vec::new();
            let deco = crate::shell::deco_top(w);
            let geo = w.geometry().loc;
            let origin = Point::<i32, Logical>::from((-geo.x, deco - geo.y)).to_physical_precise_round(sc);
            els.extend(w.render_elements::<GloveElement>(renderer, origin, Scale::from(sc), 1.0));
            if let Some(deco_cell) = w.user_data().get::<RefCell<Deco>>() {
                if let Some(t) = &deco_cell.borrow().title {
                    els.extend(mem(renderer, (0, 0).into(), t, sc));
                }
            }
            {
                let Ok(mut fb) = renderer.bind(&mut tex) else { return };
                let mut dt = OutputDamageTracker::new((tw, th), sc, Transform::Normal);
                if dt.render_output(renderer, &mut fb, 0, &els, [0.0, 0.0, 0.0, 0.0]).is_err() {
                    return;
                }
            }
            *a.snapshot.borrow_mut() = Some(tex);
        }
        let Some(tex) = a.snapshot.borrow().clone() else { return };
        let mut cache = self.effects.borrow_mut();
        if cache.genie.is_none() {
            let names = [
                UniformName::new("fr", UniformType::_4f),
                UniformName::new("sl", UniformType::_4f),
                UniformName::new("anim", UniformType::_2f),
                UniformName::new("bx", UniformType::_2f),
            ];
            match renderer.compile_custom_texture_shader(GENIE_SHADER, &names) {
                Ok(pr) => cache.genie = Some(pr),
                Err(e) => {
                    tracing::warn!("Lâmpada Mágica não compilou: {e:?}");
                    return;
                }
            }
        }
        let Some(program) = cache.genie.clone() else { return };
        drop(cache);
        let smooth = |t: f64| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let bend = smooth(p / 0.45);
        let slide = smooth((p - 0.3) / 0.7);
        // Área que a animação ocupa: da janela até o ícone.
        let bb = frame.merge(slot);
        let bb = Rectangle::new((bb.loc.x.floor() - 2.0, bb.loc.y.floor() - 2.0).into(), (bb.size.w.ceil() + 4.0, bb.size.h.ceil() + 4.0).into());
        let rel = |r: Rectangle<f64, Logical>| ((r.loc.x - bb.loc.x) as f32, (r.loc.y - bb.loc.y) as f32, r.size.w as f32, r.size.h as f32);
        let (fx, fy, fw, fh) = rel(frame);
        let (sx, sy, sw, sh) = rel(slot);
        let size = smithay::backend::renderer::Texture::size(&tex);
        let src = Rectangle::from_size((size.w as f64, size.h as f64).into());
        let el = TextureRenderElement::from_static_texture(
            Id::new(),
            renderer.context_id(),
            bb.loc.to_physical(sc),
            tex,
            1,
            Transform::Normal,
            None,
            Some(src),
            Some(smithay::utils::Size::from((bb.size.w.round() as i32, bb.size.h.round() as i32))),
            None,
            Kind::Unspecified,
        );
        let uniforms = vec![
            Uniform::new("fr", [fx, fy, fw, fh]),
            Uniform::new("sl", [sx, sy, sw, sh]),
            Uniform::new("anim", [bend as f32, slide as f32]),
            Uniform::new("bx", [bb.size.w as f32, bb.size.h as f32]),
        ];
        out.push(GloveElement::Shader(TextureShaderElement::new(el, program, uniforms)));
    }

    fn cursor_elements(&self, renderer: &mut GlesRenderer, sc: f64, out: &mut Vec<GloveElement>) {
        self.cursor_elements_at(renderer, sc, Point::from((0, 0)), out);
    }

    /// Cursor numa tela que começa em `origin` (lógico).
    fn cursor_elements_at(&self, renderer: &mut GlesRenderer, sc: f64, origin: Point<i32, Logical>, out: &mut Vec<GloveElement>) {
        use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
        let pos = self.pointer_pos - origin.to_f64();
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
        // Arrastando entre apps: o ícone do app, colado ao cursor (antes dele na lista = por cima).
        if let Some(icon) = self.dnd_icon.as_ref().filter(|i| smithay::reexports::wayland_server::Resource::is_alive(*i)) {
            // Um pouco abaixo e à direita da ponta da seta.
            let loc = (pos + Point::<f64, Logical>::from((6.0, 6.0))).to_physical(sc).to_i32_round();
            out.extend(render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<GlesRenderer>>(renderer, icon, loc, sc, 0.9, Kind::Cursor).into_iter().map(GloveElement::Surface));
        }
        match &self.cursor_status {
            CursorImageStatus::Hidden => {}
            CursorImageStatus::Surface(surface) => {
                let hotspot = smithay::wayland::compositor::with_states(surface, |s| {
                    s.data_map.get::<CursorImageSurfaceData>().map(|d| d.lock().unwrap().hotspot).unwrap_or_default()
                });
                // Cursor maior (Acesso Universal): o dos apps também cresce.
                let k = self.config.access.cursor_size.clamp(1.0, 4.0) as f64;
                let loc = (pos - hotspot.to_f64().upscale(k)).to_physical(sc).to_i32_round();
                out.extend(
                    render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<GlesRenderer>>(renderer, surface, loc, sc * k, 1.0, Kind::Cursor)
                        .into_iter()
                        .map(GloveElement::Surface),
                );
            }
            CursorImageStatus::Named(_) => {
                if let Some(t) = &self.cursor_tex {
                    let (hx, hy) = glove_paint::chrome::CURSOR_HOTSPOT;
                    let k = self.config.access.cursor_size.clamp(1.0, 4.0) as f64;
                    let loc = Point::<f64, Logical>::from((pos.x - hx as f64 * k, pos.y - hy as f64 * k));
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
    if let Some((path, area, sc)) = state.extra_shot.take() {
        match extra_screenshot(state, renderer, &path, area, sc) {
            Ok(()) => tracing::info!("captura do monitor extra em {}", path.display()),
            Err(e) => tracing::warn!("captura do monitor extra falhou: {e}"),
        }
    }
    if let Some(path) = state.screenshot_request.take() {
        let crop = state.screenshot_crop.take();
        match screenshot(state, renderer, &path, crop) {
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
    if let Some(s) = &state.dnd_icon {
        smithay::desktop::utils::send_frames_surface_tree(s, output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
    }
}

/// Desenha a tela inteira numa textura fora da tela e salva como PNG.
/// Depuração: desenha o que um monitor extra (na área dada) mostraria e salva em PNG.
fn extra_screenshot(state: &Glove, renderer: &mut GlesRenderer, path: &std::path::Path, area: Rectangle<i32, Logical>, sc: f64) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = ((area.size.w as f64 * sc).round() as i32, (area.size.h as f64 * sc).round() as i32);
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (w, h).into())?;
    let elements = state.build_extra_elements(renderer, area, sc);
    let mut fb = renderer.bind(&mut tex)?;
    let mut dt = OutputDamageTracker::new((w, h), sc, Transform::Normal);
    dt.render_output(renderer, &mut fb, 0, &elements, [0.0, 0.0, 0.0, 1.0])?;
    let mapping = renderer.copy_framebuffer(&fb, Rectangle::from_size((w, h).into()), Fourcc::Abgr8888)?;
    let data = renderer.map_texture(&mapping)?.to_vec();
    let size = glove_paint::tiny_skia::IntSize::from_wh(w as u32, h as u32).ok_or("tamanho inválido")?;
    let pm = glove_paint::Pixmap::from_vec(data, size).ok_or("dados inválidos")?;
    pm.save_png(path)?;
    Ok(())
}

fn screenshot(state: &Glove, renderer: &mut GlesRenderer, path: &std::path::Path, crop: Option<Rectangle<i32, Logical>>) -> Result<(), Box<dyn std::error::Error>> {
    let (lw, lh) = state.output_size();
    // Área fora do monitor principal (outro monitor): desenha só ela, na escala dele.
    if let Some(c) = crop.filter(|c| !Rectangle::from_size((lw, lh).into()).contains_rect(*c)) {
        let center = c.loc.to_f64() + c.size.to_f64().downscale(2.0).to_point();
        let out = state.space.outputs().find(|o| state.space.output_geometry(o).is_some_and(|g| g.to_f64().contains(center))).cloned();
        let (geo, sc) = match &out {
            Some(o) => (state.space.output_geometry(o).unwrap_or(c), o.current_scale().fractional_scale()),
            None => (c, state.scale()),
        };
        let Some(area) = c.intersection(geo) else { return Err("área fora dos monitores".into()) };
        let (w, h) = ((area.size.w as f64 * sc).round() as i32, (area.size.h as f64 * sc).round() as i32);
        let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (w, h).into())?;
        let elements = state.build_area_elements(renderer, area, sc, false);
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
        return Ok(());
    }
    let sc = state.scale();
    let (w, h) = ((lw as f64 * sc).round() as i32, (lh as f64 * sc).round() as i32);
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (w, h).into())?;
    let elements = state.build_elements(renderer, false);
    let mut fb = renderer.bind(&mut tex)?;
    let mut dt = OutputDamageTracker::new((w, h), sc, Transform::Normal);
    dt.render_output(renderer, &mut fb, 0, &elements, [0.0, 0.0, 0.0, 1.0])?;
    // Área escolhida (em pixels reais, dentro da tela) ou a tela inteira.
    let area = match crop {
        Some(c) => {
            let x0 = ((c.loc.x as f64 * sc).round() as i32).clamp(0, w - 1);
            let y0 = ((c.loc.y as f64 * sc).round() as i32).clamp(0, h - 1);
            let x1 = (((c.loc.x + c.size.w) as f64 * sc).round() as i32).clamp(x0 + 1, w);
            let y1 = (((c.loc.y + c.size.h) as f64 * sc).round() as i32).clamp(y0 + 1, h);
            Rectangle::new((x0, y0).into(), (x1 - x0, y1 - y0).into())
        }
        None => Rectangle::from_size((w, h).into()),
    };
    let mapping = renderer.copy_framebuffer(&fb, area, Fourcc::Abgr8888)?;
    let data = renderer.map_texture(&mapping)?.to_vec();
    let size = glove_paint::tiny_skia::IntSize::from_wh(area.size.w as u32, area.size.h as u32).ok_or("tamanho inválido")?;
    let pm = glove_paint::Pixmap::from_vec(data, size).ok_or("dados inválidos")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    pm.save_png(path)?;
    Ok(())
}
