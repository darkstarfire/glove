//! Cantos de baixo arredondados nas janelas da aparência Moderno (os de cima já são
//! arredondados pela barra de título). Vale para qualquer app, inclusive os de fora:
//! cada superfície da janela que encosta num dos cantos de baixo é desenhada com um
//! programa de textura que apaga (com borda suave) o que fica fora do arco.

use smithay::{
    backend::renderer::{
        element::{surface::WaylandSurfaceRenderElement, Element, Id, Kind, RenderElement, UnderlyingStorage},
        gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, Uniform, UniformName, UniformType},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{Buffer as BufferCoords, Physical, Rectangle, Scale, Transform},
};

/// Raio dos cantos (o mesmo dos de cima da barra de título no Moderno), em pontos.
pub const RAIO: f64 = 10.0;

/// Programa de textura: a cor da superfície vezes a cobertura do canto.
pub const SHADER: &str = r#"#version 100

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

// A janela (x, y, largura, altura) em pixels, relativa a esta superfície; o tamanho
// da superfície na tela; o raio em pixels.
uniform vec4 win;
uniform vec2 size;
uniform float radius;
// 0 = só os de baixo; 1 = os quatro (janela sem barra de título, como os alertas);
// 2 = só os de cima (janela sem barra no tema clássico, como a Jangada).
uniform float top;

void main() {
    vec4 color = texture2D(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif
    vec2 p = v_coords * size;
    float k = 1.0;
    float by = win.y + win.w - radius;
    float ty = win.y + radius;
    float lx = win.x + radius;
    float rx = win.x + win.z - radius;
    float cy = -1.0;
    if (top < 1.5 && p.y > by) {
        cy = by;
    } else if (top > 0.5 && p.y < ty) {
        cy = ty;
    }
    if (cy >= 0.0) {
        if (p.x < lx) {
            k = clamp(radius - distance(p, vec2(lx, cy)) + 0.5, 0.0, 1.0);
        } else if (p.x > rx) {
            k = clamp(radius - distance(p, vec2(rx, cy)) + 0.5, 0.0, 1.0);
        }
    }
    color = color * alpha * k;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
"#;

pub fn compile(renderer: &mut GlesRenderer) -> Option<GlesTexProgram> {
    let names = [UniformName::new("win", UniformType::_4f), UniformName::new("size", UniformType::_2f), UniformName::new("radius", UniformType::_1f), UniformName::new("top", UniformType::_1f)];
    match renderer.compile_custom_texture_shader(SHADER, &names) {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("cantos arredondados não compilaram: {e:?}");
            None
        }
    }
}

/// Quais cantos arredondar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cantos {
    /// Os de baixo (a barra de título do Glove já arredonda os de cima).
    Baixo,
    /// Os quatro (janela sem barra, no Moderno).
    Todos,
    /// Só os de cima (janela sem barra no tema clássico: embaixo ele é reto).
    Cima,
}

/// Uma superfície da janela desenhada com os cantos de baixo dela recortados.
pub struct Rounded {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    /// A janela inteira, em pixels da tela.
    win: Rectangle<i32, Physical>,
    radius: f64,
    cantos: Cantos,
}

impl Rounded {
    /// Envolve `inner` se ela encosta num canto de baixo de `win` (e fica dentro da
    /// janela: um menu que sai para fora não é recortado). Senão devolve a própria.
    pub fn wrap(inner: WaylandSurfaceRenderElement<GlesRenderer>, program: &GlesTexProgram, win: Rectangle<i32, Physical>, scale: f64, cantos: Cantos, raio: f64) -> Result<Self, WaylandSurfaceRenderElement<GlesRenderer>> {
        let g = inner.geometry(Scale::from(scale));
        let radius = (raio * scale).round();
        let inside = g.loc.x >= win.loc.x - 1 && g.loc.y >= win.loc.y - 1 && g.loc.x + g.size.w <= win.loc.x + win.size.w + 1 && g.loc.y + g.size.h <= win.loc.y + win.size.h + 1;
        // A superfície que contém a janela inteira (as do GTK guardam em volta uma margem
        // transparente para redimensionar): os cantos são os da janela, dentro dela.
        let contains = g.loc.x <= win.loc.x && g.loc.y <= win.loc.y && g.loc.x + g.size.w >= win.loc.x + win.size.w && g.loc.y + g.size.h >= win.loc.y + win.size.h;
        let corners = quadrados(win.loc.x, win.loc.y, win.size.w, win.size.h, radius as i32, cantos);
        if !(inside || contains) || !corners.iter().any(|c| c.overlaps(g)) {
            return Err(inner);
        }
        Ok(Self { inner, program: program.clone(), win, radius, cantos })
    }

    /// Os quadrados dos cantos, relativos à superfície.
    fn corners_local(&self, scale: Scale<f64>) -> Vec<Rectangle<i32, Physical>> {
        let g = self.inner.geometry(scale);
        let r = self.radius.ceil() as i32;
        quadrados(self.win.loc.x - g.loc.x, self.win.loc.y - g.loc.y, self.win.size.w, self.win.size.h, r, self.cantos)
    }
}

/// Os quadrados dos cantos arredondados de uma janela (x, y, largura, altura).
fn quadrados(x: i32, y: i32, w: i32, h: i32, r: i32, cantos: Cantos) -> Vec<Rectangle<i32, Physical>> {
    let mut v = Vec::new();
    if cantos != Cantos::Cima {
        v.push(Rectangle::new((x, y + h - r).into(), (r, r).into()));
        v.push(Rectangle::new((x + w - r, y + h - r).into(), (r, r).into()));
    }
    if cantos != Cantos::Baixo {
        v.push(Rectangle::new((x, y).into(), (r, r).into()));
        v.push(Rectangle::new((x + w - r, y).into(), (r, r).into()));
    }
    v
}

impl Element for Rounded {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn src(&self) -> Rectangle<f64, BufferCoords> {
        self.inner.src()
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn location(&self, scale: Scale<f64>) -> smithay::utils::Point<i32, Physical> {
        self.inner.location(scale)
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    /// Os cantos não são mais opacos: o que está atrás aparece neles.
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let corners = self.corners_local(scale);
        self.inner.opaque_regions(scale).into_iter().flat_map(|r| r.subtract_rects(corners.iter().copied())).collect()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for Rounded {
    fn draw(&self, frame: &mut GlesFrame<'_, '_>, src: Rectangle<f64, BufferCoords>, dst: Rectangle<i32, Physical>, damage: &[Rectangle<i32, Physical>], opaque_regions: &[Rectangle<i32, Physical>]) -> Result<(), GlesError> {
        // A janela relativa ao lugar onde a superfície é desenhada.
        let win = [(self.win.loc.x - dst.loc.x) as f32, (self.win.loc.y - dst.loc.y) as f32, self.win.size.w as f32, self.win.size.h as f32];
        frame.override_default_tex_program(
            self.program.clone(),
            vec![
                Uniform::new("win", win),
                Uniform::new("size", [dst.size.w as f32, dst.size.h as f32]),
                Uniform::new("radius", self.radius as f32),
                Uniform::new("top", match self.cantos {
                    Cantos::Baixo => 0.0f32,
                    Cantos::Todos => 1.0,
                    Cantos::Cima => 2.0,
                }),
            ],
        );
        let r = RenderElement::<GlesRenderer>::draw(&self.inner, frame, src, dst, damage, opaque_regions);
        frame.clear_tex_program_override();
        r
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        // Sempre pelo programa (nunca direto num plano da placa: os cantos sumiriam).
        None
    }
}
