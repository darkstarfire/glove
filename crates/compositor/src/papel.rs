//! Papel de parede de cada monitor.
//!
//! - "igual": o mesmo papel em cada monitor (cada um no tamanho dele);
//! - "por_tela": um papel para cada monitor (pelo nome do conector);
//! - "panorama": uma imagem só cobrindo todos os monitores, pela posição deles na
//!   mesa (lado a lado, um em cima do outro, em L): cada um mostra o seu pedaço.
//!
//! O ajuste da imagem (preencher, ajustar, esticar, centralizar, lado a lado) vale
//! nos três. Papel animado (vídeo ou GIF): o ffmpeg decodifica no ritmo pedido e
//! manda os quadros por um cano; o Glove troca a textura a cada quadro. Ele para
//! (SIGSTOP) com a tela bloqueada ou apagada, com app em tela cheia, na bateria e
//! com "Reduzir efeitos", conforme as Preferências.
//!
//! `chrome.wallpaper` continua sendo a imagem parada da tela principal (bloqueio,
//! login, Exposé); a mesa de cada tela desenha a textura daqui.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_paint::draw::Canvas;
use glove_paint::wallpaper::{self, Fit, Span};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// Uma tela: nome do conector, retângulo lógico na mesa e escala.
#[derive(Clone, Debug, PartialEq)]
struct Screen {
    name: String,
    rect: (i32, i32, i32, i32),
    scale: f64,
}

/// O que um monitor mostra.
#[derive(Clone, Debug, PartialEq)]
struct Want {
    style: String,
    fit: Fit,
    span: Option<Span>,
    /// Animado: a fonte (vídeo) compartilhada com outras telas (panorama/igual).
    anim: Option<String>,
}

/// Um papel animado tocando: o ffmpeg, o último quadro e quem o mostra.
struct Anim {
    child: Child,
    /// (número do quadro, pixels RGBA) do último quadro que chegou.
    frame: Arc<Mutex<(u64, Vec<u8>)>>,
    seen: u64,
    /// Tamanho do quadro em pixels.
    w: u32,
    h: u32,
    paused: bool,
}

impl Drop for Anim {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct Papel {
    /// Por tela: o que foi pedido (para refazer só quando muda) e a textura.
    wants: HashMap<String, (u64, Want)>,
    pub texs: HashMap<String, Tex>,
    /// Animados tocando, pela chave da fonte.
    anims: HashMap<String, Anim>,
    ticking: bool,
}

fn hash_of<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut k = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut k);
    k.finish()
}

/// Começa o ffmpeg tocando `path` em laço, já no tamanho w×h (cobrindo, sem
/// distorcer) e no ritmo `fps`; os quadros chegam numa thread.
fn start_anim(path: &str, w: u32, h: u32, fps: u32) -> Option<Anim> {
    let vf = format!("fps={fps},scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},format=rgb24");
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-re", "-stream_loop", "-1", "-i", path, "-an", "-vf", &vf, "-pix_fmt", "rgba", "-f", "rawvideo", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let frame: Arc<Mutex<(u64, Vec<u8>)>> = Arc::new(Mutex::new((0, Vec::new())));
    let shared = frame.clone();
    let size = (w * h * 4) as usize;
    std::thread::spawn(move || {
        let mut buf = vec![0u8; size];
        let mut n = 0u64;
        while out.read_exact(&mut buf).is_ok() {
            n += 1;
            let mut f = shared.lock().unwrap();
            f.0 = n;
            std::mem::swap(&mut f.1, &mut buf);
            if buf.len() != size {
                buf = vec![0u8; size];
            }
        }
    });
    Some(Anim { child, frame, seen: 0, w, h, paused: false })
}

/// Textura de um pedaço (x, y, w, h em pixels do quadro) de um quadro RGBA,
/// mostrada no tamanho lógico `show` da tela.
fn frame_tex(data: &[u8], fw: u32, part: (u32, u32, u32, u32), show: (i32, i32)) -> Option<Tex> {
    let (x, y, w, h) = part;
    let mut c = Canvas::new(w as f32, h as f32, 1.0)?;
    let dst = c.pm.data_mut();
    for row in 0..h as usize {
        let si = ((y as usize + row) * fw as usize + x as usize) * 4;
        let di = row * w as usize * 4;
        let n = w as usize * 4;
        if si + n <= data.len() {
            dst[di..di + n].copy_from_slice(&data[si..si + n]);
        }
    }
    let mut t = to_tex(c);
    t.size = show.into();
    Some(t)
}

impl Glove {
    /// As telas da mesa (a principal primeiro). Espelhando ou dentro de outro
    /// desktop, só a principal.
    fn papel_screens(&self) -> Vec<Screen> {
        let primary = self.primary().cloned();
        let mut v: Vec<Screen> = Vec::new();
        let mirror = self.config.monitor.arrangement == "espelhar" || self.nested;
        let outs: Vec<smithay::output::Output> = self.space.outputs().cloned().collect();
        for o in outs.iter().filter(|o| Some(*o) == primary.as_ref()).chain(outs.iter().filter(|o| Some(*o) != primary.as_ref())) {
            if mirror && Some(o) != primary.as_ref() {
                continue;
            }
            let Some(g) = self.space.output_geometry(o) else { continue };
            v.push(Screen { name: o.name(), rect: (g.loc.x, g.loc.y, g.size.w, g.size.h), scale: o.current_scale().fractional_scale() });
        }
        v
    }

    /// O papel animado deve parar agora?
    fn papel_paused(&self) -> bool {
        let a = &self.config.wallpaper_anim;
        self.lock.is_some()
            || self.idle.screen_off
            || glove_config::reduce_effects()
            || (a.pause_fullscreen && self.fullscreen_top())
            || (a.pause_on_battery && self.extras.battery.as_ref().is_some_and(|b| !b.on_ac))
    }

    /// Prepara o papel de parede de cada tela (chamado a cada quadro; só refaz o
    /// que mudou).
    pub fn prepare_wallpapers(&mut self) {
        let screens = self.papel_screens();
        let Some(first) = screens.first().cloned() else { return };
        let fit = Fit::from_id(&self.config.wallpaper_fit);
        let mode = self.config.wallpaper_mode.as_str();
        let panorama = mode == "panorama" && screens.len() > 1;
        // A área que o panorama cobre: o retângulo de todos os monitores juntos.
        let bb = screens.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, s| (b.0.min(s.rect.0), b.1.min(s.rect.1), b.2.max(s.rect.0 + s.rect.2), b.3.max(s.rect.1 + s.rect.3)));
        let (bw, bh) = ((bb.2 - bb.0).max(1), (bb.3 - bb.1).max(1));
        let ffmpeg = *FFMPEG.get_or_init(wallpaper::has_ffmpeg);
        let anim_cfg = self.config.wallpaper_anim.clone();
        let tgen = self.theme_gen;

        // O que cada tela quer.
        let mut wants: Vec<(Screen, Want)> = Vec::new();
        for s in &screens {
            let style = if panorama { self.config.wallpaper.clone() } else { self.config.wallpaper_for(&s.name).to_string() };
            let span = panorama.then(|| Span { total_w: bw as f32, total_h: bh as f32, x: (s.rect.0 - bb.0) as f32, y: (s.rect.1 - bb.1) as f32 });
            let anim = (ffmpeg && wallpaper::is_animated(&style)).then(|| {
                if panorama {
                    format!("panorama|{style}")
                } else if mode == "por_tela" {
                    format!("{}|{style}", s.name)
                } else {
                    format!("igual|{style}")
                }
            });
            wants.push((s.clone(), Want { style, fit, span, anim }));
        }

        // Animados: começa os que faltam e para os que ninguém mais usa.
        let needed: Vec<String> = wants.iter().filter_map(|(_, w)| w.anim.clone()).collect();
        self.papel.anims.retain(|k, _| needed.contains(k));
        let light = if anim_cfg.light { 0.5 } else { 1.0 };
        let fps = anim_cfg.fps.clamp(5, 60);
        for (s, w) in &wants {
            let Some(key) = &w.anim else { continue };
            if self.papel.anims.contains_key(key) {
                continue;
            }
            // Tamanho do quadro: a área inteira (panorama) ou a tela, em pixels, com teto.
            let (lw, lh, sc) = if key.starts_with("panorama|") { (bw, bh, first.scale) } else { (s.rect.2, s.rect.3, s.scale) };
            let mut fw = (lw as f64 * sc * light).round().max(16.0);
            let mut fh = (lh as f64 * sc * light).round().max(16.0);
            let cap = 3840.0_f64;
            if fw > cap {
                fh = (fh * cap / fw).round();
                fw = cap;
            }
            let (fw, fh) = ((fw as u32) & !1, (fh as u32) & !1);
            if let Some(a) = start_anim(&w.style, fw, fh, fps) {
                tracing::info!("papel animado {} em {fw}x{fh} a {fps} q/s", w.style);
                self.papel.anims.insert(key.clone(), a);
            }
        }
        // Parar e continuar (o ffmpeg fica parado no quadro atual).
        let pause = self.papel_paused();
        for a in self.papel.anims.values_mut() {
            if a.paused != pause {
                a.paused = pause;
                unsafe {
                    libc::kill(a.child.id() as i32, if pause { libc::SIGSTOP } else { libc::SIGCONT });
                }
            }
        }

        // Texturas.
        let mut changed_primary = false;
        for (s, w) in &wants {
            let key = hash_of(&(&w.style, w.fit.id(), w.span.map(|p| (p.total_w as i32, p.total_h as i32, p.x as i32, p.y as i32)), s.rect.2, s.rect.3, (s.scale * 100.0) as i32, tgen, w.anim.is_some()));
            if let Some(akey) = &w.anim {
                // Animado: um quadro novo vira textura (o pedaço desta tela, no panorama).
                let Some(a) = self.papel.anims.get_mut(akey) else { continue };
                let f = a.frame.lock().unwrap();
                if f.0 == 0 || (f.0 == a.seen && self.papel.texs.contains_key(&s.name) && self.papel.wants.get(&s.name).is_some_and(|x| x.0 == key)) {
                    drop(f);
                    if !self.papel.texs.contains_key(&s.name) {
                        // Enquanto o primeiro quadro não chega: a imagem parada.
                        let sc = s.scale as f32;
                        if let Some(c) = wallpaper::render_part(&self.theme, &w.style, s.rect.2 as f32, s.rect.3 as f32, sc, w.fit, w.span) {
                            self.papel.texs.insert(s.name.clone(), to_tex(c));
                            changed_primary |= s.name == first.name;
                        }
                    }
                    continue;
                }
                let part = match w.span {
                    Some(p) => {
                        let kx = a.w as f32 / p.total_w;
                        let ky = a.h as f32 / p.total_h;
                        let x = (p.x * kx).round() as u32;
                        let y = (p.y * ky).round() as u32;
                        let pw = ((s.rect.2 as f32 * kx).round() as u32).min(a.w.saturating_sub(x)).max(1);
                        let ph = ((s.rect.3 as f32 * ky).round() as u32).min(a.h.saturating_sub(y)).max(1);
                        (x, y, pw, ph)
                    }
                    None => (0, 0, a.w, a.h),
                };
                if let Some(t) = frame_tex(&f.1, a.w, part, (s.rect.2, s.rect.3)) {
                    self.papel.texs.insert(s.name.clone(), t);
                    changed_primary |= s.name == first.name;
                }
                drop(f);
                self.papel.wants.insert(s.name.clone(), (key, w.clone()));
                continue;
            }
            if self.papel.wants.get(&s.name).is_some_and(|x| x.0 == key) && self.papel.texs.contains_key(&s.name) {
                continue;
            }
            if let Some(c) = wallpaper::render_part(&self.theme, &w.style, s.rect.2 as f32, s.rect.3 as f32, s.scale as f32, w.fit, w.span) {
                self.papel.texs.insert(s.name.clone(), to_tex(c));
                self.papel.wants.insert(s.name.clone(), (key, w.clone()));
                changed_primary |= s.name == first.name;
            }
        }
        // Quadros já mostrados (depois de todas as telas daquele vídeo).
        for a in self.papel.anims.values_mut() {
            a.seen = a.frame.lock().unwrap().0;
        }
        // Telas que saíram.
        let names: Vec<String> = screens.iter().map(|s| s.name.clone()).collect();
        self.papel.texs.retain(|k, _| names.contains(k));
        self.papel.wants.retain(|k, _| names.contains(k));
        if changed_primary {
            self.dirty = true;
        }
        if !self.papel.anims.is_empty() && !pause {
            self.papel_tick(fps);
        }
    }

    /// A textura do papel de uma tela.
    pub fn wallpaper_of(&self, output: &str) -> Option<&Tex> {
        self.papel.texs.get(output)
    }

    /// Acorda o desenho no ritmo do vídeo enquanto houver papel animado tocando.
    fn papel_tick(&mut self, fps: u32) {
        if self.papel.ticking {
            return;
        }
        self.papel.ticking = true;
        let every = Duration::from_millis((1000 / fps.max(1)) as u64);
        let _ = self.loop_handle.insert_source(Timer::from_duration(every), move |_, _, data| {
            let st = &mut data.state;
            if st.papel.anims.is_empty() || st.papel.anims.values().all(|a| a.paused) {
                st.papel.ticking = false;
                return TimeoutAction::Drop;
            }
            let ready = st.papel.anims.values().any(|a| a.frame.lock().unwrap().0 != a.seen);
            if ready {
                st.dirty = true;
            }
            TimeoutAction::ToDuration(every)
        });
    }
}

/// O ffmpeg existe? (Pergunta uma vez por sessão.)
static FFMPEG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
