use crate::state::Glove;
use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
        GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab,
        PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<Glove>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
    /// Menor `y` permitido (a barra de título não passa por baixo da barra de menu).
    pub min_y: i32,
    /// Já começou a arrastar de verdade (passou de alguns pontos).
    started: bool,
    /// Tamanho da janela agora (muda ao sair do zoom ou ao encolher para outro monitor).
    size: Option<Size<i32, Logical>>,
    /// Monitor em que o tamanho já foi conferido.
    checked: Option<String>,
}

impl MoveSurfaceGrab {
    pub fn new(start_data: PointerGrabStartData<Glove>, window: Window, initial_window_location: Point<i32, Logical>, min_y: i32) -> Self {
        Self { start_data, window, initial_window_location, min_y, started: false, size: None, checked: None }
    }

    /// A janela mudou de tamanho no meio do arrasto: reposiciona para o ponto
    /// agarrado continuar sob o mouse (na mesma proporção da largura).
    fn regrip(&mut self, at: Point<f64, Logical>, old: Size<i32, Logical>, new: Size<i32, Logical>) {
        let loc = self.initial_window_location.to_f64() + (at - self.start_data.location);
        let frac = ((at.x - loc.x) / old.w.max(1) as f64).clamp(0.05, 0.95);
        let off_y = (at.y - loc.y).min(new.h as f64);
        self.initial_window_location = Point::from(((at.x - frac * new.w as f64).round() as i32, (at.y - off_y).round() as i32));
        self.start_data.location = at;
    }
}

impl PointerGrab<Glove> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        if !self.started && (delta.x.abs() > 3.0 || delta.y.abs() > 3.0) {
            self.started = true;
            let cur = data.space.element_geometry(&self.window).map(|g| g.size).unwrap_or_default();
            self.size = Some(cur);
            self.checked = data.space.output_under(self.start_data.location).next().map(|o| o.name());
            // Em zoom: volta ao tamanho de antes e continua arrastando.
            if let Some(old) = data.unzoom_for_drag(&self.window) {
                self.regrip(event.location, cur, old);
                self.size = Some(old);
            }
        }
        if self.started {
            // Entrou num monitor em que a janela não cabe: encolhe para 80% dele.
            let under = data.space.output_under(event.location).next().cloned();
            if let Some(o) = under {
                if self.checked.as_deref() != Some(o.name().as_str()) {
                    self.checked = Some(o.name());
                    if let (Some(og), Some(cur)) = (data.space.output_geometry(&o), self.size) {
                        let top = crate::shell::deco_top(&self.window);
                        let (mw, mh) = ((og.size.w as f64 * 0.8) as i32, (og.size.h as f64 * 0.8) as i32 - top);
                        if cur.w > og.size.w || cur.h + top > og.size.h {
                            let new = Size::from((cur.w.min(mw), cur.h.min(mh)));
                            data.resize_for_drag(&self.window, new);
                            self.regrip(event.location, cur, new);
                            self.size = Some(new);
                        }
                    }
                }
            }
        }
        let delta = event.location - self.start_data.location;
        let mut new_location = (self.initial_window_location.to_f64() + delta).to_i32_round::<i32>();
        new_location.y = new_location.y.max(self.min_y);
        data.space.map_element(self.window.clone(), new_location, true);
        data.sync_x11(&self.window);
        data.dirty = true;
    }

    fn relative_motion(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        // The button is a button code as defined in the
        // Linux kernel's linux/input-event-codes.h header file, e.g. BTN_LEFT.
        const BTN_LEFT: u32 = 0x110;

        if !handle.current_pressed().contains(&BTN_LEFT) {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn frame(&mut self, data: &mut Glove, handle: &mut PointerInnerHandle<'_, Glove>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event)
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event)
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event)
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event)
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event)
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event)
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event)
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut Glove,
        handle: &mut PointerInnerHandle<'_, Glove>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event)
    }

    fn start_data(&self) -> &PointerGrabStartData<Glove> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut Glove) {}
}
