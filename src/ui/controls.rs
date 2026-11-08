//! The app's styled controls, each wrapping a base component once.

use nsh_base::{SliderElement, ButtonElement, SliderValue, button, slider, theme};
use nsh_core::prelude::*;

use super::{
    icons::{Icon, icon, icon_with},
    theme as tokens,
};

/// A borderless icon button: muted, brighter on hover, primary while `on`.
pub fn icon_button(
    glyph: impl Fn() -> Icon + 'static,
    label: impl Fn() -> &'static str + 'static,
    size: f32,
    on: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    button()
        .accessibility_label(move || Some(SharedString::from(label())))
        .items_center()
        .justify_center()
        .size(px(size + 12.0))
        .rounded_full()
        .text_color(move || if on() { theme().primary } else { theme().muted_foreground })
        .hover(|s| s.opacity(0.85))
        .active(|s| s.opacity(0.6).scale(0.94))
        .disabled_style(|s| s.opacity(0.35))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(on_press)
        .child(icon_with(glyph, size))
}

/// The round play / pause button, `size` across.
pub fn play_button(
    playing: impl Fn() -> bool + Copy + 'static,
    size: f32,
    filled_with_primary: bool,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let glyph = move || if playing() { Icon::Pause } else { Icon::Play };
    button()
        .accessibility_label(move || Some(SharedString::from(if playing() { "Pause" } else { "Play" })))
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(move || if filled_with_primary { theme().primary } else { tokens::control() })
        .text_color(move || if filled_with_primary { theme().primary_foreground } else { tokens::control_foreground() })
        .hover(|s| s.scale(1.04))
        .active(|s| s.scale(0.95).opacity(0.85))
        .disabled_style(|s| s.opacity(0.4))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(on_press)
        .child(icon_with(glyph, size * 0.42))
}

/// A pill-shaped button with a label: `primary` filled, else outlined.
pub fn pill_button(label: impl IntoReactive<SharedString>, primary: bool, on_press: impl Fn() + 'static) -> ButtonElement {
    button()
        .h_9()
        .px_5()
        .items_center()
        .justify_center()
        .rounded_full()
        .when(primary, |b| b.bg(move || theme().foreground))
        .when(!primary, |b| b.border_1().border_color(move || theme().muted_foreground))
        .hover(|s| s.scale(1.03))
        .active(|s| s.opacity(0.7))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(on_press)
        .child(
            text(label)
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(move || if primary { theme().background } else { theme().foreground }),
        )
}

/// A round button with a chevron: the main view's back and forward.
pub fn history_button(glyph: Icon, label: &'static str, enabled: impl Fn() -> bool + 'static, on_press: impl Fn() + 'static) -> ButtonElement {
    button()
        .accessibility_label(label)
        .disabled(move || !enabled())
        .size_8()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(move || tokens::shell().opacity(0.7))
        .text_color(move || theme().foreground)
        .hover(|s| s.opacity(0.85))
        .active(|s| s.opacity(0.6))
        .disabled_style(|s| s.opacity(0.4))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(on_press)
        .child(icon(glyph, 18.0))
}

/// An image of an item, `size` square (`round` for an artist), with a glyph
/// on a muted square while there is no picture.
pub fn cover(url: impl Fn() -> String + 'static, size: Option<f32>, round: bool, fallback: Icon) -> View {
    let url = computed(url);
    let radius = move || if round { px(9999.0) } else { px(6.0) };
    view()
        .when_some(size, |v, size| v.size(px(size)))
        .when(size.is_none(), |v| v.w_full().aspect_ratio(1.0))
        .flex_shrink_0()
        .rounded(radius)
        .overflow_hidden()
        .bg(move || theme().secondary)
        .items_center()
        .justify_center()
        .text_color(move || theme().muted_foreground)
        .child(icon(fallback, size.map_or(40.0, |s| (s * 0.4).max(16.0))))
        // Made once there is a picture: an image never given an empty source.
        .child(when(
            move || !url.with(String::is_empty),
            move || image(move || url.get()).absolute().inset_0().object_fit(ResizeMode::Cover),
        ))
}

/// A thin bar slider (the progress and the volume): the range in the text
/// color, primary while hovered, a thumb shown on hover.
pub fn bar_slider(
    label: &'static str,
    value: impl Fn() -> f64 + 'static,
    max: impl Fn() -> f64 + 'static,
    on_change: impl Fn(f64) + 'static,
    on_change_end: impl Fn(f64) + 'static,
) -> SliderElement {
    slider()
        .accessibility_label(label)
        .value(move || Some(SliderValue::Single(value())))
        .min(0.0)
        .max(max)
        .step(0.0)
        .track_class("h-4 justify-center group")
        .indicator_class("h-1 rounded-full bg-muted")
        .range_class("rounded-full bg-foreground group-hover:bg-primary")
        .thumb_class("size-3 rounded-full bg-foreground opacity-0 group-hover:opacity-100 active:opacity-100 focus:opacity-100")
        .on_change(move |v: &SliderValue| {
            if let SliderValue::Single(v) = v {
                on_change(*v)
            }
        })
        .on_change_end(move |v: &SliderValue| {
            if let SliderValue::Single(v) = v {
                on_change_end(*v)
            }
        })
}

/// A section's title.
pub fn section_title(title: impl IntoReactive<SharedString>) -> Text {
    text(title)
        .text_xl()
        .font_weight(FontWeight::BOLD)
        .text_color(move || theme().foreground)
        .role(AccessibilityRole::Header)
}
