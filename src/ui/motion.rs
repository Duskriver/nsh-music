//! The app's motion: springs and timings used across the UI, and small
//! helpers that bring content in. These run on the platform's animation graph,
//! not on signals written per frame.

use nsh_core::{
    animation::{Curve, EaseMode, Easing},
    element::{Spring, Timing},
    prelude::*,
};

/// A press sinking under the finger.
pub const PRESS: Spring = Spring { stiffness: 520.0, damping: 34.0, mass: 1.0, velocity: None, frame_rate: None };

/// A knob or panel settling into place.
pub const SETTLE: Spring = Spring { stiffness: 300.0, damping: 28.0, mass: 1.0, velocity: None, frame_rate: None };

/// Content coming in.
pub const ENTER: Timing = Timing::new(460.0, Easing::Curve(Curve::Cubic, EaseMode::Out));

/// Fades and lifts `child` in when it mounts; `index` staggers siblings.
pub fn appear(index: u32, child: impl IntoElement) -> View {
    let shown = animated_value(0.0);
    shown.timing_with(1.0, Timing { delay_ms: (index.min(10) as f32) * 45.0, ..ENTER });
    view()
        .bind_opacity(&shown)
        .bind_translate_y(&shown.interpolate(&[0.0, 1.0], &[18.0, 0.0]))
        .child(child)
}

/// Fades and rises a full page in (it fills its parent).
pub fn appear_page(child: impl IntoElement) -> View {
    let shown = animated_value(0.0);
    shown.timing_with(1.0, ENTER);
    view()
        .flex_1()
        .min_h(px(0.0))
        .bind_opacity(&shown)
        .bind_translate_y(&shown.interpolate(&[0.0, 1.0], &[10.0, 0.0]))
        .child(child)
}

/// Scales `child` on press, with [`PRESS`]; returns the view and the value to
/// drive from `on_press_down` / `on_press_up`.
pub fn press_scale() -> (nsh_core::element::AnimatedValue, nsh_core::element::AnimatedValue) {
    let v = animated_value(1.0);
    (v, v)
}
