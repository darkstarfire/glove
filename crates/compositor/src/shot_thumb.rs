//! The thumbnail of a screenshot in the corner of the screen, as on the Mac: after a
//! capture it slides in at the bottom right of the monitor where it was taken and stays a
//! few seconds (waiting while the mouse is on it). A click opens the picture in the
//! Viewer, ready to draw on it; dragging it drops the picture into any window (a chat,
//! the Files, the desktop). The picture is already saved and copied when it shows.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use glove_paint::{
    draw::{rrect, solid, Canvas, ShadowSpec},
    tiny_skia, Pixmap,
};
use smithay::{
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    shell::{to_tex, Tex},
    state::Glove,
};

/// How long it stays (counted again after the mouse leaves it).
const STAY: Duration = Duration::from_secs(6);
/// Sliding in and out.
const SLIDE: Duration = Duration::from_millis(260);
/// The biggest the picture gets in the corner (points).
const MAX_W: f32 = 210.0;
const MAX_H: f32 = 150.0;
/// The white frame around the picture and the shadow margin.
const FRAME: f32 = 4.0;
const SHADOW: f32 = 14.0;
/// Distance from the corner of the screen.
const MARGIN: i32 = 18;
/// How far the mouse moves with the button down before it is a drag.
const DRAG_START: f64 = 5.0;

pub struct ShotThumb {
    pub path: PathBuf,
    /// The monitor where it shows.
    pub area: Rectangle<i32, Logical>,
    /// The picture, already reduced (physical pixels at `scale`).
    pic: Pixmap,
    scale: f64,
    /// Size of the frame with the picture (points, without the shadow).
    pub size: (i32, i32),
    pub tex: Option<Tex>,
    shown: Instant,
    /// Since when the mouse is away (it waits while the mouse is on it).
    away_since: Instant,
    pub hover: bool,
    /// The button went down on it (where), to tell a click from a drag.
    pub press: Option<Point<f64, Logical>>,
    /// Being dragged into a window: drawn under the mouse.
    pub dragging: bool,
    leaving: Option<Instant>,
}

/// The picture reduced to fit the corner (keeping its proportion), in physical pixels.
fn reduce(src: &Pixmap, src_scale: f64, scale: f64) -> Option<(Pixmap, (f32, f32))> {
    let (lw, lh) = (src.width() as f32 / src_scale as f32, src.height() as f32 / src_scale as f32);
    let k = (MAX_W / lw).min(MAX_H / lh).min(1.0);
    let (w, h) = ((lw * k).max(8.0), (lh * k).max(8.0));
    let (pw, ph) = ((w * scale as f32).round().max(1.0) as u32, (h * scale as f32).round().max(1.0) as u32);
    let mut out = Pixmap::new(pw, ph)?;
    // Halves first, then the rest (a big screenshot reduced at once looks grainy).
    let mut cur: Option<Pixmap> = None;
    loop {
        let s = cur.as_ref().unwrap_or(src);
        if s.width() / 2 >= pw && s.height() / 2 >= ph {
            let mut half = Pixmap::new(s.width() / 2, s.height() / 2)?;
            let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
            half.draw_pixmap(0, 0, s.as_ref(), &paint, tiny_skia::Transform::from_scale(0.5, 0.5), None);
            cur = Some(half);
            continue;
        }
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
        out.draw_pixmap(0, 0, s.as_ref(), &paint, tiny_skia::Transform::from_scale(pw as f32 / s.width() as f32, ph as f32 / s.height() as f32), None);
        break;
    }
    Some((out, (w, h)))
}

impl ShotThumb {
    /// The frame with the picture and its shadow.
    fn draw(&self) -> Option<Canvas> {
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let m = SHADOW;
        let sc = self.scale as f32;
        let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, sc)?;
        let s = ShadowSpec { blur: 10.0, offset_y: 3.0, radius: 6.0, alpha: 0.4 };
        if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &s, sc) {
            c.draw_canvas(&sh, 0.0, 0.0);
        }
        c.fill(rrect(m, m, w, h, 6.0), &solid(glove_paint::Color::WHITE));
        let (px, py) = (((m + FRAME) * sc).round() as i32, ((m + FRAME) * sc).round() as i32);
        c.pm.draw_pixmap(px, py, self.pic.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        c.stroke(rrect(m + 0.5, m + 0.5, w - 1.0, h - 1.0, 6.0), &solid(glove_paint::draw::rgba(0, 0, 0, 40)), 1.0 / sc);
        Some(c)
    }

    /// How far it is slid out (0 = in place, 1 = out of the screen).
    fn out(&self) -> f32 {
        let e = |t: f32| t * t * (3.0 - 2.0 * t);
        if let Some(l) = self.leaving {
            return e((l.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).min(1.0));
        }
        1.0 - e((self.shown.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).min(1.0))
    }

    /// The frame on the screen (global points, without the shadow).
    pub fn rect(&self) -> Rectangle<i32, Logical> {
        let (w, h) = self.size;
        let x = self.area.loc.x + self.area.size.w - MARGIN - w;
        let y = self.area.loc.y + self.area.size.h - MARGIN - h;
        let slide = (self.out() * (w + MARGIN + SHADOW as i32) as f32).round() as i32;
        Rectangle::new((x + slide, y).into(), (w, h).into())
    }

    fn animating(&self) -> bool {
        self.leaving.is_some() || self.shown.elapsed() < SLIDE
    }
}

impl Glove {
    /// A capture was saved: it goes to the clipboard, and then either opens in the Viewer
    /// (the Capture app asked for that) or shows as a thumbnail in the corner.
    /// `rect`: where it was taken (for the monitor); `pic`: the picture, if at hand.
    pub fn shot_saved(&mut self, path: &Path, rect: Option<Rectangle<i32, Logical>>, pic: Option<(Pixmap, f64)>, open: bool) {
        self.copy_file_to_clipboard(path);
        if open {
            self.spawn(&format!("glove-view --draw '{}'", path.display().to_string().replace('\'', "'\\''")));
            return;
        }
        let area = rect
            .and_then(|r| self.output_rect_at((r.loc.x as f64 + r.size.w as f64 / 2.0, r.loc.y as f64 + r.size.h as f64 / 2.0).into()))
            .or_else(|| self.primary().and_then(|o| self.space.output_geometry(o)));
        let Some(area) = area else { return };
        let (src, src_scale) = match pic {
            Some(p) => p,
            None => match Pixmap::load_png(path) {
                Ok(p) => (p, self.scale()),
                Err(_) => return,
            },
        };
        let scale = self.scale();
        let Some((pic, (w, h))) = reduce(&src, src_scale, scale) else { return };
        let size = ((w + 2.0 * FRAME).round() as i32, (h + 2.0 * FRAME).round() as i32);
        let now = Instant::now();
        let mut t = ShotThumb { path: path.to_path_buf(), area, pic, scale, size, tex: None, shown: now, away_since: now, hover: false, press: None, dragging: false, leaving: None };
        t.tex = t.draw().map(to_tex);
        let had = self.shot_thumb.replace(t).is_some();
        self.dirty = true;
        if !had {
            self.shot_thumb_timer();
        }
    }

    /// Moves it along (sliding, time to go) while it exists.
    fn shot_thumb_timer(&mut self) {
        self.loop_handle
            .insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, data| {
                let st = &mut data.state;
                let Some(t) = &mut st.shot_thumb else { return TimeoutAction::Drop };
                if t.dragging {
                    return TimeoutAction::ToDuration(Duration::from_millis(100));
                }
                if t.hover || t.press.is_some() {
                    t.away_since = Instant::now();
                }
                if t.leaving.is_none() && t.away_since.elapsed() >= STAY && t.shown.elapsed() >= STAY {
                    t.leaving = Some(Instant::now());
                }
                if t.leaving.is_some_and(|l| l.elapsed() >= SLIDE) {
                    st.shot_thumb = None;
                    st.dirty = true;
                    return TimeoutAction::Drop;
                }
                let anim = t.animating();
                if anim {
                    st.dirty = true;
                }
                TimeoutAction::ToDuration(Duration::from_millis(if anim { 16 } else { 100 }))
            })
            .ok();
    }

    /// Is the point on the thumbnail?
    pub fn shot_thumb_at(&self, pos: Point<f64, Logical>) -> bool {
        self.shot_thumb.as_ref().is_some_and(|t| !t.dragging && t.leaving.is_none() && t.rect().to_f64().contains(pos))
    }

    /// The mouse moved: hover, and the start of a drag (then the system drag takes over).
    pub fn shot_thumb_motion(&mut self, pos: Point<f64, Logical>) -> bool {
        let over = self.shot_thumb_at(pos);
        let Some(t) = &mut self.shot_thumb else { return false };
        t.hover = over;
        if let Some(p) = t.press.filter(|_| !t.dragging) {
            if (pos.x - p.x).hypot(pos.y - p.y) >= DRAG_START {
                t.dragging = true;
                t.press = None;
                let path = t.path.clone();
                self.desk_start_drag(vec![path], p);
                if let Some(d) = &mut self.desk.drag {
                    d.from_shot = true;
                }
                self.dirty = true;
                return true;
            }
            return true;
        }
        over
    }

    /// The button on the thumbnail: down marks, up (without dragging) opens the Viewer.
    pub fn shot_thumb_button(&mut self, pressed: bool, pos: Point<f64, Logical>) -> bool {
        if pressed {
            if !self.shot_thumb_at(pos) {
                return false;
            }
            if let Some(t) = &mut self.shot_thumb {
                t.press = Some(pos);
            }
            return true;
        }
        let Some(t) = &mut self.shot_thumb else { return false };
        if t.press.take().is_none() {
            return false;
        }
        let path = t.path.clone();
        self.shot_thumb = None;
        self.dirty = true;
        self.spawn(&format!("glove-view --draw '{}'", path.display().to_string().replace('\'', "'\\''")));
        true
    }

    /// The drag of the thumbnail ended (dropped or not): it is gone.
    pub fn shot_thumb_drag_ended(&mut self) {
        if self.shot_thumb.as_ref().is_some_and(|t| t.dragging) {
            self.shot_thumb = None;
            self.dirty = true;
        }
    }

    /// Where to draw it on the monitor `area` (relative to it), if it shows there: in
    /// its corner, or under the mouse while dragged.
    pub fn shot_thumb_place(&self, area: Rectangle<i32, Logical>) -> Option<(Point<i32, Logical>, &Tex)> {
        let t = self.shot_thumb.as_ref()?;
        let tex = t.tex.as_ref()?;
        let m = SHADOW as i32;
        if t.dragging {
            if !area.to_f64().contains(self.pointer_pos) {
                return None;
            }
            let (w, h) = t.size;
            let p = self.pointer_pos;
            return Some(((p.x.round() as i32 - w / 2 - m - area.loc.x, p.y.round() as i32 - h / 2 - m - area.loc.y).into(), tex));
        }
        if t.area != area {
            return None;
        }
        let r = t.rect();
        Some(((r.loc.x - m - area.loc.x, r.loc.y - m - area.loc.y).into(), tex))
    }
}
