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
    Rounded=crate::cantos::Rounded,
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
    /// Programa dos cantos de baixo arredondados (Moderno); `Some(None)` = não compilou.
    cantos: Option<Option<GlesTexProgram>>,
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
// Lado do Dock: 0 embaixo, 1 à esquerda, 2 à direita. Nas laterais a conta é a mesma, num
// espaço girado em que o Dock fica embaixo (fr e sl já chegam nesse espaço).
uniform float dir;

void main() {
    vec2 s = v_coords * bx;
    vec2 p = s;
    if (dir > 1.5) {
        p = vec2(s.y, s.x);
    } else if (dir > 0.5) {
        p = vec2(s.y, bx.x - s.x);
    }
    float bottom = sl.y + sl.w;
    // Duas etapas, como no Mac. anim.x: a parte de baixo da janela estica até o ícone e
    // afina no funil (a lâmpada), com o topo parado. anim.y: o topo desce até o ícone e a
    // janela escorre pelo funil para dentro dele.
    float bot = mix(fr.y + fr.w, bottom, anim.x);
    float top = mix(fr.y, bottom, anim.y);
    float hh = max(bot - top, 0.5);
    // Linha da janela neste ponto.
    float v = (p.y - top) / hh;
    // Borda do funil nesta altura: da janela até o ícone, em "S".
    float f = smoothstep(0.0, 1.0, (p.y - fr.y) / max(sl.y - fr.y, 1.0));
    float l = mix(fr.x, mix(fr.x, sl.x, f), anim.x);
    float wd = mix(fr.z, mix(fr.z, sl.z, f), anim.x);
    float u = (p.x - l) / max(wd, 0.5);
    // Bordas suaves (meio ponto de cada lado) e nada abaixo do ícone.
    float edge = clamp(min(p.x - l, l + wd - p.x) + 0.5, 0.0, 1.0)
        * clamp(min(p.y - top, bot - p.y) + 0.5, 0.0, 1.0)
        * clamp(bottom - p.y + 0.5, 0.0, 1.0);
    if (edge <= 0.0 || u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0) {
        gl_FragColor = vec4(0.0);
        return;
    }
    // De volta ao espaço da janela para pegar o pixel certo dela.
    vec2 uv = vec2(u, v);
    if (dir > 1.5) {
        uv = vec2(v, u);
    } else if (dir > 0.5) {
        uv = vec2(1.0 - v, u);
    }
    gl_FragColor = texture2D(tex, uv) * alpha * edge;
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

/// Um pedaço (faixa horizontal) de uma textura, em `loc` com o tamanho lógico `size`:
/// `crop` = (começo, altura) da faixa em pontos lógicos da textura; None = inteira.
fn mem_part(renderer: &mut GlesRenderer, loc: (f32, f32), tex: &Tex, scale: f64, alpha: f32, crop: Option<(f32, f32)>, size: (f32, f32)) -> Option<GloveElement> {
    let buf = tex.upload(renderer)?;
    let k = tex.px.1 as f64 / tex.size.h.max(1) as f64;
    let src = match crop {
        Some((y0, h)) => Rectangle::new((0.0, y0 as f64 * k).into(), (tex.px.0 as f64, (h as f64 * k).max(1.0)).into()),
        None => Rectangle::from_size((tex.px.0 as f64, tex.px.1 as f64).into()),
    };
    let p = Point::<f64, Logical>::from((loc.0 as f64, loc.1 as f64)).to_physical(scale);
    let size = smithay::utils::Size::<i32, Logical>::from((size.0.round().max(1.0) as i32, size.1.round().max(1.0) as i32));
    Some(GloveElement::Texture(TextureRenderElement::from_texture_buffer((p.x.round(), p.y.round()), &buf, Some(alpha), Some(src), Some(size), Kind::Unspecified)))
}

/// Textura esticada para um retângulo lógico (os ícones do Dock na ampliação), com a
/// transparência (o Dock de um monitor sem o foco).
fn mem_scaled_alpha(renderer: &mut GlesRenderer, loc: (f64, f64), size: (f64, f64), tex: &Tex, scale: f64, alpha: Option<f32>) -> Option<GloveElement> {
    let buf = tex.upload(renderer)?;
    let src = Rectangle::from_size((tex.px.0 as f64, tex.px.1 as f64).into());
    let p = Point::<f64, Logical>::from(loc).to_physical(scale);
    let size = smithay::utils::Size::<i32, Logical>::from((size.0.round().max(1.0) as i32, size.1.round().max(1.0) as i32));
    Some(GloveElement::Texture(TextureRenderElement::from_texture_buffer((p.x.round(), p.y.round()), &buf, alpha, Some(src), Some(size), Kind::Unspecified)))
}

impl Glove {
    /// O Dock montado na GPU, da frente para o fundo: nome, indicadores, ícones,
    /// divisórias e a prateleira.
    fn dock_elements(&self, renderer: &mut GlesRenderer, sc: f64, out: &mut Vec<GloveElement>) {
        let principal = self.primary().and_then(|o| self.space.output_geometry(o));
        // Ocultando automaticamente: só no monitor em que foi revelado.
        if !self.dock_shown_on(principal) {
            return;
        }
        let alpha = self.dock_alpha(principal);
        match self.still_dock_for(principal) {
            Some(l) => self.dock_elements_at(renderer, sc, self.chrome.dock_pos_layout(l, self.chrome.out_size), true, alpha, out),
            None => self.dock_elements_at(renderer, sc, self.chrome.dock_pos(), false, alpha, out),
        }
    }

    /// O desenho parado do Dock para o monitor `area`, se outro monitor estiver ampliando
    /// o dele (None = o Dock de sempre).
    pub fn still_dock_for(&self, area: Option<Rectangle<i32, Logical>>) -> Option<&glove_paint::chrome::DockLayout> {
        let l = self.chrome.dock_still_layout.as_ref()?;
        (self.chrome.dock_zoom_area.is_some() && self.chrome.dock_zoom_area != area).then_some(l)
    }

    /// O Dock com o canto em `o` (no principal, a posição dele; num monitor extra, a da
    /// cópia, relativa a esse monitor). `parado`: o desenho sem a ampliação.
    fn dock_elements_at(&self, renderer: &mut GlesRenderer, sc: f64, o: Point<i32, Logical>, parado: bool, alpha: Option<f32>, out: &mut Vec<GloveElement>) {
        let c = &self.chrome;
        let g = if parado { &c.dock_still } else { &c.dock_gpu };
        let Some(scene) = &g.scene else { return };
        let at = |x: f32, y: f32| Point::<i32, Logical>::from((o.x + x.round() as i32, o.y + y.round() as i32));
        if let (Some((_, x, y)), Some(t)) = (scene.label, g.label()) {
            out.extend(mem_alpha(renderer, at(x, y), t, sc, alpha));
        }
        for (&(x, y), t) in scene.indicators.iter().zip(g.inds()) {
            out.extend(mem_alpha(renderer, at(x, y), t, sc, alpha));
        }
        for (t, x, y, w, h) in &g.icon_places {
            out.extend(mem_scaled_alpha(renderer, (o.x as f64 + x, o.y as f64 + y), (*w, *h), t, sc, alpha));
        }
        for (&(x, y), t) in scene.separators.iter().zip(g.seps()) {
            out.extend(mem_alpha(renderer, at(x, y), t, sc, alpha));
        }
        if let Some(t) = g.shelf() {
            out.extend(mem_alpha(renderer, at(scene.shelf.0, scene.shelf.1), t, sc, alpha));
        }
    }

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
        // Menus (de contexto, do Dock) abertos neste monitor, por cima das janelas.
        if let Some(m) = &self.chrome.menu {
            for lv in m.levels.iter().rev() {
                let r = Rectangle::new(lv.buffer_pos(), (lv.size.w + 2 * glove_paint::chrome::MENU_SHADOW, lv.size.h + glove_paint::chrome::MENU_SHADOW).into());
                if let Some(b) = lv.buffer.as_ref().filter(|_| r.overlaps(area)) {
                    out.extend(mem(renderer, lv.buffer_pos() - origin, b, sc));
                }
            }
        }
        if cursor {
            self.region_elements_at(sc, origin, (area.size.w, area.size.h), &mut out);
        }
        // Bloqueada ou encerrando: só o papel de parede (nada das janelas).
        let covered = self.lock.is_some() || self.greeter || self.ending.as_ref().is_some_and(|e| e.screen_since.is_some()) || self.startup.as_ref().is_some_and(|s| s.covering());
        // Os painéis da barra abertos neste monitor (Busca Rápida, central de controle,
        // volume), por cima de tudo menos dos menus.
        let aqui = |p: Point<i32, Logical>| area.contains(p);
        if self.search.open {
            if let Some(t) = &self.search.tex {
                let o = self.search_origin();
                if aqui(o) {
                    let (mx, my) = glove_paint::chrome::quick_search_margin();
                    out.extend(mem(renderer, (o.x - mx as i32 - origin.x, o.y - my as i32 - origin.y).into(), t, sc));
                }
            }
        }
        if let Some(cp) = self.control.as_ref().filter(|c| aqui(c.origin)) {
            if let Some(t) = &cp.tex {
                out.extend(mem(renderer, (cp.origin.x - glove_paint::chrome::MENU_SHADOW - origin.x, cp.origin.y - origin.y).into(), t, sc));
            }
        }
        if let Some(s) = self.extras.slider.as_ref().filter(|s| aqui(s.origin)) {
            if let Some(t) = &s.tex {
                out.extend(mem(renderer, (s.origin.x - glove_paint::chrome::MENU_SHADOW - origin.x, s.origin.y - origin.y).into(), t, sc));
            }
        }
        // A barra de menu e o Dock deste monitor (as cópias dos do principal), por cima
        // das janelas; somem com uma janela em tela cheia aqui.
        if !covered && !self.area_fullscreen(area) {
            if let Some(t) = self.chrome.extra_bars.iter().find(|b| b.area == area).and_then(|b| b.tex.as_ref()) {
                out.extend(mem_alpha(renderer, (0, 0).into(), t, sc, self.bar_alpha(Some(area))));
            }
            if self.grade.alpha <= 0.0 && self.dock_shown_on(Some(area)) {
                let alpha = self.dock_alpha(Some(area));
                match self.still_dock_for(Some(area)) {
                    Some(l) if self.config.monitor.dock_all => {
                        let o = self.chrome.dock_pos_layout(l, (area.size.w, area.size.h));
                        self.dock_elements_at(renderer, sc, o, true, alpha, &mut out);
                    }
                    _ => {
                        if let Some(o) = self.extra_dock_origin(area) {
                            self.dock_elements_at(renderer, sc, o - origin, false, alpha, &mut out);
                        }
                    }
                }
            }
        }
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
                // Os popups (listas suspensas) podem passar das bordas da janela.
                let mut with_popups = w.bbox_with_popups();
                with_popups.loc += geo.loc - w.geometry().loc;
                if !framed.overlaps(area) && !with_popups.overlaps(area) {
                    continue;
                }
                out.extend(self.window_elements(renderer, w, geo.loc - origin, sc));
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
        // Bloqueada com o mouse nesta tela: o painel de senha aparece aqui.
        if let Some(t) = self.lock.as_ref().and_then(|l| l.tex.as_ref()).filter(|_| self.lock_output_rect() == area) {
            let (x, y) = self.lock_panel_pos();
            let m = glove_paint::chrome::LOCK_SHADOW as i32;
            out.extend(mem(renderer, (x - m - origin.x, y - m - origin.y).into(), t, sc));
        }
        // O papel desta tela (o dela, o pedaço do panorama ou o da principal esticado).
        let own = self.space.outputs().find(|o| self.space.output_geometry(o) == Some(area)).and_then(|o| self.wallpaper_of(&o.name()));
        if let Some(t) = own.or(self.chrome.wallpaper.as_ref()) {
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
    /// As superfícies de uma janela cuja área (geometria) começa em `loc` (pontos, na
    /// tela). No Moderno, as que encostam nos cantos de baixo saem arredondadas.
    fn window_elements(&self, renderer: &mut GlesRenderer, w: &smithay::desktop::Window, loc: Point<i32, Logical>, sc: f64) -> Vec<GloveElement> {
        let render_loc = (loc - w.geometry().loc).to_physical_precise_round(sc);
        // Moderno: os cantos de baixo (os de cima vêm da barra de título) ou os quatro
        // (janela sem barra). Clássico: só as janelas sem barra, nos cantos de cima (como
        // a barra de título clássica; embaixo ela é reta).
        let bare = !self.greeter && self.bare_window(w);
        let cheia = crate::shell::is_fullscreen(w);
        let modo = if cheia {
            None
        } else if self.theme.modern {
            Some((if bare { crate::cantos::Cantos::Todos } else { crate::cantos::Cantos::Baixo }, crate::cantos::RAIO))
        } else if bare && self.pede_moldura(w) {
            // A janela inteira de um app com moldura própria (a Jangada): os quatro cantos.
            Some((crate::cantos::Cantos::Todos, 6.0))
        } else if bare && w.toplevel().is_some_and(|t| t.parent().is_none()) {
            Some((crate::cantos::Cantos::Cima, 6.0))
        } else {
            None
        };
        let rounder = if modo.is_some() {
            let mut cache = self.effects.borrow_mut();
            cache.cantos.get_or_insert_with(|| crate::cantos::compile(renderer)).clone()
        } else {
            None
        };
        let (Some(program), Some((cantos, raio))) = (rounder, modo) else {
            return w.render_elements::<GloveElement>(renderer, render_loc, Scale::from(sc), 1.0);
        };
        let win = Rectangle::new(loc.to_physical_precise_round(sc), w.geometry().size.to_physical_precise_round(sc));
        w.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(renderer, render_loc, Scale::from(sc), 1.0)
            .into_iter()
            .map(|e| match crate::cantos::Rounded::wrap(e, &program, win, sc, cantos, raio) {
                Ok(r) => GloveElement::Rounded(r),
                Err(e) => GloveElement::Surface(e),
            })
            .collect()
    }

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
        // Grade de apps: por cima de tudo (menos do cursor e do Dock), sobre o papel de
        // parede desfocado. Aberta de todo, nada mais aparece embaixo.
        let g = &self.grade;
        if g.alpha > 0.0 {
            let a = Some(g.alpha);
            self.dock_elements(renderer, sc, &mut out);
            if let Some((_, t)) = &g.field_tex {
                out.extend(mem_alpha(renderer, self.grade_field_pos(), t, sc, a));
            }
            if let Some((_, t)) = &g.dots_tex {
                out.extend(mem_alpha(renderer, self.grade_dots_pos(), t, sc, a));
            }
            let pos = self.grade_grid_pos();
            let (dx_new, dx_old) = self.grade_slide_offsets().unwrap_or((0, 0));
            if let Some(fg) = g.fold_gpu.as_ref().filter(|_| g.fold_gpu_on) {
                // Pasta abrindo ou fechando: as camadas prontas, movidas e recortadas.
                use crate::grade::FoldPart;
                for (part, at, crop, size, alpha) in self.fold_gpu_parts() {
                    let t = match part {
                        FoldPart::Rest => &fg.rest,
                        FoldPart::Tile => match &fg.tile {
                            Some((t, _)) => t,
                            None => continue,
                        },
                        FoldPart::Body => &fg.body,
                        FoldPart::Bottom => &fg.bottom,
                        FoldPart::Glow => &fg.glow,
                    };
                    out.extend(mem_part(renderer, at, t, sc, alpha * g.alpha, crop, size));
                }
            } else if let Some((_, t)) = &g.grid_tex {
                out.extend(mem_alpha(renderer, pos + Point::from((dx_new, 0)), t, sc, a));
            }
            if let Some((t, _, _)) = g.slide.as_ref().filter(|_| dx_old != 0 || dx_new != 0) {
                out.extend(mem_alpha(renderer, pos + Point::from((dx_old, 0)), t, sc, a));
            }
            if let Some((_, t)) = &g.bg {
                out.extend(mem_alpha(renderer, (0, 0).into(), t, sc, a));
            }
            if g.alpha >= 1.0 && g.bg.is_some() {
                return out;
            }
        }
        // Painel de Widgets: por cima de tudo (menos do cursor), sobre a tela escurecida.
        let dash = &self.dashboard;
        if dash.alpha > 0.0 {
            let a = Some(dash.alpha);
            if let Some((_, t)) = &dash.button_tex {
                out.extend(mem_alpha(renderer, self.dash_button_pos(), t, sc, a));
            }
            if let (true, Some((_, t))) = (dash.bar, &dash.close_tex) {
                for w in dash.widgets.iter().rev() {
                    let c = glove_paint::widgets::CLOSE as i32;
                    out.extend(mem_alpha(renderer, (w.x - c / 2 + 2, w.y - c / 2 + 2).into(), t, sc, a));
                }
            }
            if let (true, Some((_, t))) = (dash.bar, &dash.bar_tex) {
                let (_, sh) = self.output_size();
                out.extend(mem_alpha(renderer, (0, sh - glove_paint::widgets::BAR_H as i32).into(), t, sc, a));
            }
            let m = glove_paint::widgets::M as i32;
            for w in dash.widgets.iter().rev() {
                if let Some((_, t)) = &w.tex {
                    out.extend(mem_alpha(renderer, (w.x - m, w.y - m).into(), t, sc, a));
                }
            }
            if let Some(d) = &dash.dim {
                out.push(GloveElement::Solid(SolidColorRenderElement::from_buffer(d, (0, 0), sc, 1.0, Kind::Unspecified)));
            }
        }
        // Ícones da mesa sendo arrastados: translúcidos, seguindo o mouse.
        if let Some(off) = self.desk_drag_offset() {
            for p in self.desk.drag.as_ref().map(|d| d.paths.as_slice()).unwrap_or_default() {
                if let (Some(i), Some((_, t))) = (self.desk.index_of(p), self.desk.texs.get(p)) {
                    out.extend(mem_alpha(renderer, self.desk.rects[i].loc + off, t, sc, Some(0.6)));
                }
            }
        }
        // Escolhendo a área de uma captura: a película fica por cima de tudo (menos do cursor),
        // e, logo atrás dela, a tela congelada do momento em que a seleção começou.
        self.region_elements(sc, &mut out);
        if self.region.is_some() {
            if let Some(f) = &self.freeze {
                out.extend(mem(renderer, (0, 0).into(), &f.tex, sc));
            }
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
        // As janelas do app (Alt+Tab): as miniaturas por cima do fundo da faixa.
        if let (Some(s), Some((x, y))) = (&self.win_switcher, self.win_switcher_pos()) {
            for (w, sl) in s.windows.iter().zip(&s.slots) {
                let r = Rectangle::new((x as f64 + sl[0] as f64, y as f64 + sl[1] as f64).into(), (sl[2] as f64, sl[3] as f64).into());
                self.push_scaled_window(renderer, w, r, sc, &mut out);
            }
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
            // O painel só vai no principal se o mouse estiver nele (senão vai no extra).
            let on_primary = self.primary().and_then(|o| self.space.output_geometry(o)).is_none_or(|g| g == self.lock_output_rect());
            if let Some(t) = l.tex.as_ref().filter(|_| on_primary) {
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

        // Central de avisos e os balões.
        if let Some((_, t)) = self.avisos.center.as_ref().and_then(|c| c.tex.as_ref()) {
            out.extend(mem(renderer, self.center_origin(), t, sc));
        }
        for (p, t, a) in self.bubble_draws() {
            out.extend(mem_alpha(renderer, p, t, sc, Some(a)));
        }
        // Aviso da troca de mesa virtual.
        if let (Some((_, t)), Some(p)) = (&self.mesas.hud, self.desk_hud_pos()) {
            out.extend(mem_alpha(renderer, p, t, sc, Some(self.mesas.hud_alpha)));
        }
        // Aviso de volume/brilho por cima de tudo (menos o cursor).
        if let Some(t) = self.extras.osd.as_ref().and_then(|o| o.tex.as_ref()) {
            out.extend(mem(renderer, self.osd_pos(), t, sc));
        }
        if self.search.open {
            if let Some(t) = &self.search.tex {
                let o = self.search_origin();
                let (mx, my) = glove_paint::chrome::quick_search_margin();
                out.extend(mem(renderer, (o.x - mx as i32, o.y - my as i32).into(), t, sc));
            }
        }
        if let Some(cp) = &self.control {
            if let Some(t) = &cp.tex {
                out.extend(mem(renderer, (cp.origin.x - glove_paint::chrome::MENU_SHADOW, cp.origin.y).into(), t, sc));
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
        let covered = self.fullscreen_cover();
        if let Some(b) = c.menubar.as_ref().filter(|_| !covered) {
            // Em tela cheia, a barra desce do alto (e sobe ao sair).
            let y = if c.fs { -(((1.0 - c.peek) * glove_paint::theme::MENUBAR_H as f32).round() as i32) } else { 0 };
            let principal = self.primary().and_then(|o| self.space.output_geometry(o));
            out.extend(mem_alpha(renderer, (0, y).into(), b, sc, self.bar_alpha(principal)));
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
        // Miniaturas vivas das minimizadas, com o ícone do app no canto. Ficam no Dock do
        // principal: com o Dock que some sozinho revelado em outro monitor, não sobem aqui.
        let dock_aqui = self.dock_shown_on(self.primary().and_then(|o| self.space.output_geometry(o)));
        for (w, (x, y), t) in &self.badge_places {
            if dock_aqui && !self.min_animating(w) {
                out.extend(mem(renderer, (*x, *y).into(), t, sc));
            }
        }
        for (w, _) in &self.minimized {
            if !dock_aqui || self.min_animating(w) {
                continue;
            }
            if let Some(r) = self.thumbnail_rect(w) {
                self.push_scaled_window(renderer, w, r, sc, &mut out);
            }
        }
        if !covered && self.grade.alpha <= 0.0 {
            self.dock_elements(renderer, sc, &mut out);
        }

        if self.expose.is_some() {
            self.expose_elements(renderer, sc, &mut out);
        } else {
        // Trocando de mesa virtual: as janelas novas entram de lado e as antigas saem.
        let slide = self.desk_slide_offsets();
        let (dx_in, dx_out) = slide.unwrap_or((0, 0));
        let outgoing: Vec<(&smithay::desktop::Window, Point<i32, Logical>)> = match (&self.mesas.slide, slide) {
            (Some(s), Some(_)) => s.out.iter().rev().map(|(w, l)| (w, *l + Point::from((dx_out, 0)))).collect(),
            _ => Vec::new(),
        };
        let incoming = self.space.elements().rev().filter_map(|w| self.space.element_location(w).map(|l| (w, l + Point::from((dx_in, 0)))));
        for (w, loc) in incoming.collect::<Vec<_>>().into_iter().chain(outgoing) {
            // Voltando do Dock: desenhada pela animação, acima.
            if self.min_animating(w) {
                continue;
            }
            out.extend(self.window_elements(renderer, w, loc, sc));
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

        // Mesa: o retângulo de escolher por cima dos ícones.
        if let (Some((a, b)), Some((_, t))) = (self.desk.rubber, &self.desk.rubber_tex) {
            out.extend(mem(renderer, (a.x.min(b.x).round() as i32, a.y.min(b.y).round() as i32).into(), t, sc));
        }
        for (it, r) in self.desk.items.iter().zip(&self.desk.rects) {
            if let Some((_, t)) = self.desk.texs.get(&it.path) {
                out.extend(mem(renderer, r.loc, t, sc));
            }
        }
        // O papel da tela principal (o próprio dela, animado ou não).
        let own = self.primary().and_then(|o| self.wallpaper_of(&o.name()));
        if let Some(b) = own.or(c.wallpaper.as_ref()) {
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
        // A janela arrastada fica por cima de tudo, até da faixa.
        let dragged = e.drag.filter(|d| d.3).map(|d| d.0);
        if let Some(i) = dragged {
            self.push_scaled_window(renderer, &e.items[i].window, e.shown(i), sc, out);
        }
        // Faixa das mesas virtuais: em cada miniatura, as janelas da mesa sobre o fundo.
        let rects = self.expose_desk_rects();
        if !rects.is_empty() {
            let (sw, _) = self.output_size();
            for (d, r) in rects.iter().enumerate() {
                let k = r.size.w / sw as f64;
                for (w, loc) in self.desk_windows(d).into_iter().rev() {
                    let top = crate::shell::deco_top(&w) as f64;
                    let size = w.geometry().size;
                    let fr = Rectangle::new((r.loc.x + loc.x as f64 * k, r.loc.y + (loc.y as f64 - top) * k).into(), (size.w as f64 * k, (size.h as f64 + top) * k).into());
                    self.push_scaled_window(renderer, &w, fr, sc, out);
                }
                if let Some(b) = &self.chrome.wallpaper {
                    self.push_scaled_tex(renderer, b, *r, sc, out);
                }
            }
            if let Some((_, o, t)) = &e.strip {
                out.extend(mem(renderer, *o, t, sc));
            }
        }
        for w in self.space.elements().rev() {
            let Some(i) = e.item_of(w) else { continue };
            if Some(i) == dragged {
                continue;
            }
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
                out.extend(self.window_elements(renderer, w, loc, sc));
                if let Some(deco) = w.user_data().get::<RefCell<Deco>>() {
                    if let Some(t) = &deco.borrow().title {
                        out.extend(mem(renderer, (loc.x, loc.y - TITLE_H).into(), t, sc));
                    }
                }
            }
        }
    }

    /// Desenha a textura encolhida (ou esticada) para caber em `r`.
    fn push_scaled_tex(&self, renderer: &mut GlesRenderer, t: &Tex, r: Rectangle<f64, Logical>, sc: f64, out: &mut Vec<GloveElement>) {
        use smithay::backend::renderer::element::Kind;
        if t.size.w <= 0 {
            return;
        }
        let s = r.size.w / t.size.w as f64;
        let o = Point::<f64, Logical>::from((r.loc.x, r.loc.y)).to_physical(sc).to_i32_round();
        if let Some(buf) = t.upload(renderer) {
            let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
            let el = TextureRenderElement::from_texture_buffer(o.to_f64(), &buf, None, Some(src), Some(t.size), Kind::Unspecified);
            out.push(GloveElement::ScaledTexture(RescaleRenderElement::from_element(el, o, s)));
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
                UniformName::new("dir", UniformType::_1f),
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
        // Duas fases (p é o tempo, sem suavizar): primeiro o funil, só a parte de baixo
        // (~100 ms, o topo parado), depois a janela é sugada para o ícone, acelerando.
        let cut = crate::minimize::GENIE_FUNNEL_MS as f64 / crate::minimize::GENIE_MS as f64;
        let bend = smooth(p / cut);
        let q = ((p - cut) / (1.0 - cut)).clamp(0.0, 1.0);
        let slide = q * q * (3.0 - 2.0 * q);
        // Área que a animação ocupa: da janela até o ícone.
        let bb = frame.merge(slot);
        let bb = Rectangle::new((bb.loc.x.floor() - 2.0, bb.loc.y.floor() - 2.0).into(), (bb.size.w.ceil() + 4.0, bb.size.h.ceil() + 4.0).into());
        let rel = |r: Rectangle<f64, Logical>| ((r.loc.x - bb.loc.x) as f32, (r.loc.y - bb.loc.y) as f32, r.size.w as f32, r.size.h as f32);
        // Dock nas laterais: a janela e o ícone no espaço girado em que o Dock fica embaixo
        // (à direita, x e y trocados; à esquerda, também espelhado).
        let bw = bb.size.w as f32;
        let (dir, girar): (f32, Box<dyn Fn((f32, f32, f32, f32)) -> (f32, f32, f32, f32)>) = match self.chrome.dock_side {
            glove_paint::chrome::DockSide::Bottom => (0.0, Box::new(|r| r)),
            glove_paint::chrome::DockSide::Left => (1.0, Box::new(move |(x, y, w, h)| (y, bw - (x + w), h, w))),
            glove_paint::chrome::DockSide::Right => (2.0, Box::new(|(x, y, w, h)| (y, x, h, w))),
        };
        let (fx, fy, fw, fh) = girar(rel(frame));
        let (sx, sy, sw, sh) = girar(rel(slot));
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
            Uniform::new("dir", dir),
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
        // Na borda de uma janela (ou redimensionando): a seta de duas pontas, no
        // lugar do cursor que o app tiver pedido.
        if let Some(t) = self.resize_cursor().and_then(|i| self.resize_cursors[i].as_ref()) {
            let k = self.config.access.cursor_size.clamp(1.0, 4.0) as f64;
            let half = glove_paint::chrome::RESIZE_CURSOR_SIZE as f64 * k / 2.0;
            let loc = Point::<f64, Logical>::from((pos.x - half, pos.y - half));
            if let Some(buf) = t.upload(renderer) {
                let src = Rectangle::from_size((t.px.0 as f64, t.px.1 as f64).into());
                out.push(GloveElement::Texture(TextureRenderElement::from_texture_buffer(loc.to_physical(sc), &buf, None, Some(src), Some(t.size), Kind::Cursor)));
                return;
            }
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
            CursorImageStatus::Named(icon) => {
                use smithay::input::pointer::CursorIcon;
                let (tex, (hx, hy)) = match icon {
                    CursorIcon::Pointer if self.hand_tex.is_some() => (&self.hand_tex, glove_paint::chrome::HAND_HOTSPOT),
                    CursorIcon::Text | CursorIcon::VerticalText if self.ibeam_tex.is_some() => (&self.ibeam_tex, glove_paint::chrome::IBEAM_HOTSPOT),
                    _ => (&self.cursor_tex, glove_paint::chrome::CURSOR_HOTSPOT),
                };
                if let Some(t) = tex {
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
    let t0 = std::time::Instant::now();
    state.prepare_chrome();
    if crate::prof::on() {
        crate::prof::prep(t0.elapsed());
    }

    // Depuração (janela dentro de outro desktop): as setas só são preparadas na
    // sessão de verdade; com GLOVE_SHOT_CURSOR, aqui também, para a captura.
    if std::env::var_os("GLOVE_SHOT_CURSOR").is_some() {
        state.prepare_cursor();
    }
    let age = backend.buffer_age().unwrap_or(0);
    let damage = {
        let (renderer, mut fb) = match backend.bind() {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("bind falhou: {e:?}");
                return;
            }
        };
        let t1 = std::time::Instant::now();
        let elements = state.build_elements(renderer, false);
        let t2 = std::time::Instant::now();
        let r = damage_tracker.render_output(renderer, &mut fb, age, &elements, [0.0, 0.0, 0.0, 1.0]);
        if crate::prof::on() {
            crate::prof::build(t2 - t1);
            crate::prof::render(t2.elapsed());
        }
        match r {
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
    // Captura de área pedida: congela a tela (sem a película) e começa a seleção.
    if state.region_pending.is_some() {
        let frozen = grab(state, renderer).ok().and_then(|(pix, sc)| {
            let (lw, lh) = state.output_size();
            let mut c = glove_paint::draw::Canvas::new(lw as f32, lh as f32, sc as f32)?;
            c.pm.draw_pixmap(0, 0, pix.as_ref(), &glove_paint::tiny_skia::PixmapPaint::default(), glove_paint::tiny_skia::Transform::identity(), None);
            Some(crate::region::Frozen { pix, tex: crate::shell::to_tex(c), scale: sc })
        });
        state.begin_region(frozen);
    }
    // Gravação em andamento: manda a tela deste quadro para o ffmpeg.
    crate::gravar::quadro(state, renderer);
    if let Some(path) = state.screenshot_request.take() {
        let crop = state.screenshot_crop.take();
        match screenshot(state, renderer, &path, crop) {
            Ok(()) => {
                tracing::info!("captura salva em {}", path.display());
                // Já vai para a área de transferência: é só colar (Ctrl+V).
                state.copy_file_to_clipboard(&path);
                if std::mem::take(&mut state.open_after_shot) {
                    // Abre pronta para desenhar por cima.
                    state.spawn(&format!("glove-view --draw '{}'", path.display().to_string().replace('\'', "'\\''")));
                }
            }
            Err(e) => tracing::warn!("captura falhou: {e}"),
        }
    }

    let time = state.start_time.elapsed();
    for w in state.space.elements() {
        w.send_frame(output, time, Some(Duration::ZERO), |_, _| Some(output.clone()));
    }
    // Menus, dicas e listas de apps X11 (override-redirect): o Xwayland só manda o
    // próximo quadro depois deste aviso; sem ele, o menu ficava no primeiro (um
    // retângulo vazio, como nos menus do Arduino IDE e de outros apps Java).
    for w in &state.x11_overlays {
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

/// A tela principal inteira, como está agora (sem o cursor), em pixels reais.
pub(crate) fn grab(state: &Glove, renderer: &mut GlesRenderer) -> Result<(glove_paint::Pixmap, f64), Box<dyn std::error::Error>> {
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
    Ok((pm, sc))
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
    // Depuração: GLOVE_SHOT_CURSOR=1 põe o cursor na captura (para conferir as setas).
    let elements = state.build_elements(renderer, std::env::var_os("GLOVE_SHOT_CURSOR").is_some());
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
