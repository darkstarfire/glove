//! Papel de parede de cada monitor.
//!
//! - "same": o mesmo papel em cada monitor (cada um no tamanho dele);
//! - "per-display": um papel para cada monitor (pelo nome do conector);
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
    /// Tocando a cópia já reduzida (o cache), e não o vídeo original.
    from_cache: bool,
    /// Cache deste vídeo neste tamanho e ritmo (pronto ou sendo feito).
    cache: std::path::PathBuf,
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
    /// The wallpaper whose copy for the login screen was kept last (style and size).
    stored: Option<u64>,
    /// Login screen: whose wallpaper is shown (the user chosen in the list) and the copy of
    /// each monitor (checked again when the user changes, or every few seconds).
    greeter: Option<(String, std::time::Instant, Vec<(String, String)>)>,
}

/// The last one who entered (the login screen keeps their login in its cache folder).
fn greeter_last_login() -> Option<String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    Some(std::fs::read_to_string(home.join(".cache/glove-login-user")).ok()?.trim().to_string()).filter(|l| !l.is_empty())
}

/// The wallpaper softly blurred for the login screen, at the full size of the screen.
fn greeter_blurred(theme: &glove_paint::theme::Theme, style: &str, w: f32, h: f32, scale: f32) -> Option<glove_paint::draw::Canvas> {
    let small = glove_paint::screens::blurred_wallpaper(theme, style, w, h, 2, 0.0)?;
    let mut c = glove_paint::draw::Canvas::new(w, h, scale)?;
    let t = glove_paint::tiny_skia::Transform::from_scale(w * scale / small.width() as f32, h * scale / small.height() as f32);
    let paint = glove_paint::tiny_skia::PixmapPaint { quality: glove_paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    c.draw_pixmap_t(small.as_ref(), &paint, t);
    Some(c)
}

fn hash_of<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut k = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut k);
    k.finish()
}

/// Começa o ffmpeg tocando `path` em laço, já no tamanho w×h (cobrindo, sem
/// distorcer) e no ritmo `fps`; os quadros chegam numa thread.
/// O filtro que põe o vídeo no tamanho w×h (cobrindo, sem distorcer) e no ritmo.
fn fit_filter(w: u32, h: u32, fps: u32) -> String {
    format!("fps={fps},scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}")
}

/// Onde fica a cópia reduzida de um vídeo para um tamanho e ritmo (~/.cache/glove/wallpaper).
fn cache_path(path: &str, w: u32, h: u32, fps: u32) -> Option<std::path::PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let dir = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| home.join(".cache")).join("glove/wallpaper");
    Some(dir.join(format!("{:016x}-{w}x{h}-{fps}.mp4", hash_of(&(path, meta.len(), meta.modified().ok())))))
}

/// Faz a cópia reduzida numa thread (com prioridade baixa): o vídeo já no tamanho
/// da tela e no ritmo escolhido, em alta qualidade. Tocar essa cópia gasta uma
/// fração do processador (medido: 32% → 6% de um núcleo, num vídeo 720p a 24 q/s).
fn make_cache(src: &str, out: &std::path::Path, w: u32, h: u32, fps: u32) {
    // Já está sendo feita (o papel recomeçou no meio): não faz de novo.
    static DOING: Mutex<Vec<std::path::PathBuf>> = Mutex::new(Vec::new());
    {
        let mut d = DOING.lock().unwrap();
        if d.iter().any(|p| p == out) {
            return;
        }
        d.push(out.to_path_buf());
    }
    let (src, out) = (src.to_string(), out.to_path_buf());
    std::thread::spawn(move || {
        let done = |o: &std::path::Path| DOING.lock().unwrap().retain(|p| p != o);
        let Some(dir) = out.parent() else {
            done(&out);
            return;
        };
        let _ = std::fs::create_dir_all(dir);
        // The cache folder up to Glove 0.15.1 (the copies are made again here).
        if let Some(parent) = dir.parent() {
            let _ = std::fs::remove_dir_all(parent.join("papel"));
        }
        let tmp = out.with_extension("partial.mp4");
        let ok = Command::new("nice")
            .args(["-n", "15", "ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-nostdin", "-hwaccel", "auto", "-i", &src, "-an", "-vf", &fit_filter(w, h, fps)])
            .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "17", "-pix_fmt", "yuv420p", "-g", &(fps * 2).to_string()])
            .arg(&tmp)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            let _ = std::fs::rename(&tmp, &out);
            tracing::info!("papel animado: cópia reduzida pronta em {}", out.display());
            prune_cache(dir);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
        done(&out);
    });
}

/// O cache não cresce sem fim: fica com as cópias usadas mais recentemente (até 1 GB).
fn prune_cache(dir: &std::path::Path) {
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "mp4"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.accessed().or_else(|_| m.modified()).ok()?, m.len(), e.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    let mut total = 0u64;
    for (_, len, p) in files {
        total += len;
        if total > 1 << 30 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Começa o ffmpeg tocando em laço: a cópia reduzida (só converte as cores) ou,
/// enquanto ela não fica pronta, o vídeo original reduzido quadro a quadro (aí a
/// placa de vídeo decodifica quando pode: `-hwaccel auto`; sem ela, o processador).
fn start_anim(path: &str, w: u32, h: u32, fps: u32) -> Option<Anim> {
    let cache = cache_path(path, w, h, fps)?;
    let from_cache = cache.exists();
    let (input, vf) = if from_cache { (cache.to_string_lossy().into_owned(), "format=rgb24".to_string()) } else { (path.to_string(), format!("{},format=rgb24", fit_filter(w, h, fps))) };
    if !from_cache {
        make_cache(path, &cache, w, h, fps);
    }
    // Medido (vídeo 720p, tela 640×400 a 24 q/s): o original reduzido a cada quadro
    // gasta 32% de um núcleo (27% com a placa decodificando); a cópia pequena, 6% no
    // processador com uma thread só (com a placa, 11%: trazer cada quadro de volta
    // da placa custa mais do que decodificar um vídeo pequeno).
    let decode: &[&str] = if from_cache { &["-threads", "1"] } else { &["-hwaccel", "auto"] };
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(decode)
        .args(["-re", "-stream_loop", "-1", "-i", &input, "-an", "-vf", &vf, "-pix_fmt", "rgba", "-f", "rawvideo", "-"])
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
    Some(Anim { child, from_cache, cache, frame, seen: 0, w, h, paused: false })
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
        let mirror = self.config.monitor.arrangement == "mirror" || (self.nested && std::env::var_os("GLOVE_FAKE_EXTRA").is_none());
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

    /// Login screen: the user whose wallpaper goes behind (the one the login app says is
    /// chosen, in its window title after an invisible separator; before that, the last
    /// one who entered) and the copy of their wallpaper for each monitor.
    fn greeter_styles(&mut self, screens: &[Screen]) -> Vec<(String, String)> {
        let chosen = self
            .space
            .elements()
            .find_map(|w| crate::shell::window_info(w).0.split_once('\u{2063}').map(|(_, l)| l.trim().to_string()))
            .filter(|l| !l.is_empty())
            .or_else(greeter_last_login);
        let Some(login) = chosen else { return Vec::new() };
        let fresh = self.papel.greeter.as_ref().is_some_and(|(l, t, v)| *l == login && t.elapsed() < std::time::Duration::from_secs(5) && v.len() == screens.len());
        if !fresh {
            let uid = glove_config::login_wallpaper::uid_of(&login);
            let v = screens
                .iter()
                .filter_map(|s| uid.and_then(|u| glove_config::login_wallpaper::of_output(&login, u, &s.name)).map(|p| (s.name.clone(), p.display().to_string())))
                .collect();
            self.papel.greeter = Some((login, std::time::Instant::now(), v));
        }
        self.papel.greeter.as_ref().map(|g| g.2.clone()).unwrap_or_default()
    }

    /// The wallpaper of the monitor `name` as it is shown there (its own, the same or its
    /// piece of the panorama), blurred and `dim` darker, small (the GPU enlarges it to the
    /// monitor), and the size of the monitor: the background of the lock screen.
    pub fn blurred_of(&self, name: &str, dim: f32) -> Option<(glove_paint::Pixmap, (i32, i32))> {
        let screens = self.papel_screens();
        let s = screens.iter().find(|s| s.name == name).or(screens.first())?;
        // A mirrored monitor shows the main one's, at its own size.
        let size = self.space.outputs().find(|o| o.name() == name).and_then(|o| self.space.output_geometry(o)).map(|g| (g.size.w, g.size.h)).unwrap_or((s.rect.2, s.rect.3));
        let fit = Fit::from_id(&self.config.wallpaper_fit);
        let panorama = self.config.wallpaper_mode == "panorama" && screens.len() > 1;
        let bb = screens.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, s| (b.0.min(s.rect.0), b.1.min(s.rect.1), b.2.max(s.rect.0 + s.rect.2), b.3.max(s.rect.1 + s.rect.3)));
        let style = if panorama { self.config.wallpaper.clone() } else { self.config.wallpaper_for(&s.name).to_string() };
        let span = panorama.then(|| Span { total_w: (bb.2 - bb.0).max(1) as f32, total_h: (bb.3 - bb.1).max(1) as f32, x: (s.rect.0 - bb.0) as f32, y: (s.rect.1 - bb.1) as f32 });
        let c = wallpaper::render_part(&self.theme, &style, size.0 as f32, size.1 as f32, 0.25, fit, span)?;
        let mut pm = c.pm;
        glove_paint::screens::blur_box(&mut pm, 5);
        if dim > 0.0 {
            let k = 1.0 - dim.clamp(0.0, 1.0);
            for p in pm.data_mut().chunks_exact_mut(4) {
                for v in &mut p[..3] {
                    *v = (*v as f32 * k) as u8;
                }
            }
        }
        Some((pm, size))
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
        let mode_owned = self.config.wallpaper_mode.clone();
        let mode = mode_owned.as_str();
        let panorama = mode == "panorama" && screens.len() > 1;
        // A área que o panorama cobre: o retângulo de todos os monitores juntos.
        let bb = screens.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, s| (b.0.min(s.rect.0), b.1.min(s.rect.1), b.2.max(s.rect.0 + s.rect.2), b.3.max(s.rect.1 + s.rect.3)));
        let (bw, bh) = ((bb.2 - bb.0).max(1), (bb.3 - bb.1).max(1));
        let ffmpeg = *FFMPEG.get_or_init(wallpaper::has_ffmpeg);
        let anim_cfg = self.config.wallpaper_anim.clone();
        let tgen = self.theme_gen;

        // O que cada tela quer.
        let mut wants: Vec<(Screen, Want)> = Vec::new();
        // The login screen: the wallpaper of the user chosen in the list (the copy their
        // session keeps of each monitor), softly blurred.
        let greeter_styles = if self.greeter { self.greeter_styles(&screens) } else { Vec::new() };
        for s in &screens {
            let style = match greeter_styles.iter().find(|(n, _)| *n == s.name) {
                Some((_, g)) => g.clone(),
                None if self.greeter => self.config.wallpaper.clone(),
                None if panorama => self.config.wallpaper.clone(),
                None => self.config.wallpaper_for(&s.name).to_string(),
            };
            let span = panorama.then(|| Span { total_w: bw as f32, total_h: bh as f32, x: (s.rect.0 - bb.0) as f32, y: (s.rect.1 - bb.1) as f32 });
            let anim = (ffmpeg && wallpaper::is_animated(&style)).then(|| {
                if panorama {
                    format!("panorama|{style}")
                } else if mode == "per-display" {
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
        // A cópia reduzida ficou pronta: troca o original por ela (bem mais leve).
        let ready: Vec<String> = self.papel.anims.iter().filter(|(_, a)| !a.from_cache && a.cache.exists()).map(|(k, _)| k.clone()).collect();
        for key in ready {
            let Some(old) = self.papel.anims.get(&key) else { continue };
            let (w, h) = (old.w, old.h);
            let style = key.split_once('|').map(|x| x.1.to_string()).unwrap_or_default();
            if let Some(a) = start_anim(&style, w, h, fps) {
                self.papel.anims.insert(key, a);
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
            let painted = if self.greeter {
                greeter_blurred(&self.theme, &w.style, s.rect.2 as f32, s.rect.3 as f32, s.scale as f32)
            } else {
                wallpaper::render_part(&self.theme, &w.style, s.rect.2 as f32, s.rect.3 as f32, s.scale as f32, w.fit, w.span)
            };
            if let Some(c) = painted {
                self.papel.texs.insert(s.name.clone(), to_tex(c));
                self.papel.wants.insert(s.name.clone(), (key, w.clone()));
                changed_primary |= s.name == first.name;
            }
        }
        // A copy of the main monitor's wallpaper for the login screen (it cannot read the
        // home folder), when it changes. Not in a test session inside another desktop.
        if !self.nested && !self.greeter {
            // One copy for each monitor (in its shape, with the way the wallpaper is shown
            // there: the same, its own or its piece of the panorama) and the general one
            // (the main monitor's, for older login screens and other monitors).
            let todas: Vec<(String, String, Fit, Option<Span>, i32, i32)> = wants.iter().map(|(s, w)| (s.name.clone(), w.style.clone(), w.fit, w.span, s.rect.2.max(1), s.rect.3.max(1))).collect();
            let key = hash_of(&(todas.iter().map(|t| (&t.0, &t.1, t.2.id(), t.3.map(|p| (p.total_w as i32, p.total_h as i32, p.x as i32, p.y as i32)), t.4, t.5)).collect::<Vec<_>>(), tgen));
            if self.papel.stored != Some(key) {
                self.papel.stored = Some(key);
                let theme = self.theme.clone();
                std::thread::spawn(move || {
                    let Some(login) = glove_config::login_wallpaper::current_login() else { return };
                    for (i, (name, style, fit, span, w, h)) in todas.iter().enumerate() {
                        // Up to 1600 points across: it is shown blurred.
                        let k = (1600.0 / *w as f32).min(1.0);
                        let Some(c) = wallpaper::render_part(&theme, style, *w as f32, *h as f32, k, *fit, *span) else { continue };
                        let Ok(png) = c.pm.encode_png() else { continue };
                        if let Err(e) = glove_config::login_wallpaper::store_output(&login, name, &png) {
                            tracing::warn!("papel para a tela de login: {e}");
                        }
                        if i == 0 {
                            let _ = glove_config::login_wallpaper::store(&login, &png);
                        }
                    }
                });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtro_e_cache() {
        assert_eq!(fit_filter(640, 400, 24), "fps=24,scale=640:400:force_original_aspect_ratio=increase,crop=640:400");
        // A cópia depende do vídeo, do tamanho e do ritmo.
        let f = std::env::temp_dir().join(format!("glove-papel-{}.mp4", std::process::id()));
        std::fs::write(&f, b"x").unwrap();
        let p = f.to_string_lossy();
        let a = cache_path(&p, 640, 400, 24).unwrap();
        assert!(a.to_string_lossy().ends_with("-640x400-24.mp4"));
        assert_ne!(a, cache_path(&p, 1280, 800, 24).unwrap());
        assert_ne!(a, cache_path(&p, 640, 400, 15).unwrap());
        assert!(cache_path("/não/existe.mp4", 640, 400, 24).is_none());
        let _ = std::fs::remove_file(&f);
    }
}
