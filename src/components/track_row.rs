//! A track's row in a list: its number (or what plays), picture, name,
//! artists, album and length. Rows keep no state: a list's cells are reused
//! for other rows, so everything shown is read from `track()`.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

use crate::{
    spotify::{Track, clock},
    store::Store,
    ui::{Icon, cover, icon, icon_with, theme as tokens},
};

/// The header above a track list's rows.
pub fn track_header(show_album: impl Fn() -> bool + Copy + 'static) -> View {
    let label = |t: &'static str| text(t).text_xs().font_weight(FontWeight::MEDIUM).text_color(move || theme().muted_foreground);
    view()
        .flex_row()
        .items_center()
        .gap_4()
        .h_9()
        .px_4()
        .mx_3()
        .mb_2()
        .border_b_1()
        .border_color(move || theme().border)
        .child(label("#").w_6().text_right())
        .child(label("Title").flex_1())
        .child(label("Album").flex_1().style_with(move |s| if show_album() { s } else { s.hidden() }))
        .child(view().w_12().items_end().text_color(move || theme().muted_foreground).child(icon(Icon::Clock, 16.0)))
}

/// One row; `on_press` plays it.
pub fn track_row(
    store: Store,
    track: impl Fn() -> Option<Track> + 'static,
    number: impl Fn() -> usize + Copy + 'static,
    show_album: impl Fn() -> bool + Copy + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let track = computed(track);
    let track = move || track.get();
    let field = move |f: fn(&Track) -> String| move || track().map(|t| f(&t)).unwrap_or_default();
    let current = move || {
        let uri = track().map(|t| t.uri).unwrap_or_default();
        !uri.is_empty() && store.player.with(|p| p.track_uri == uri)
    };
    let playing = move || current() && store.player.with(|p| p.playing);
    button()
        .accessibility_label(move || track().map(|t| SharedString::from(format!("{} {}", t.name, t.artists))))
        .flex_row()
        .items_center()
        .gap_4()
        .h_14()
        .px_4()
        .mx_3()
        .rounded(move || theme().radius)
        .hover(|s| s.bg(move || tokens::row_hover()))
        .active(|s| s.opacity(0.75))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .style_with(move |s| if track().is_some_and(|t| !t.playable) { s.opacity(0.45) } else { s })
        .on_press(on_press)
        .child(
            view()
                .w_6()
                .items_end()
                .child(
                    text(move || (number() + 1).to_string())
                        .text_sm()
                        .tabular_nums()
                        .text_color(move || theme().muted_foreground)
                        .style_with(move |s| if current() { s.hidden() } else { s }),
                )
                .child(
                    view()
                        .text_color(move || theme().primary)
                        .style_with(move |s| if current() { s } else { s.hidden() })
                        .child(icon_with(move || if playing() { Icon::Volume } else { Icon::Play }, 14.0)),
                ),
        )
        .child(cover(field(|t| t.image.clone()), Some(40.0), false, Icon::Music).rounded(px(4.0)))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap_0p5()
                .child(
                    text(field(|t| t.name.clone()))
                        .number_of_lines(1)
                        .text_base()
                        .text_color(move || if current() { theme().primary } else { theme().foreground }),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .child(
                            text("E")
                                .text_size(px(9.0))
                                .font_weight(FontWeight::BOLD)
                                .px_1()
                                .rounded(px(2.0))
                                .bg(move || theme().muted_foreground)
                                .text_color(move || theme().background)
                                .style_with(move |s| if track().is_some_and(|t| t.explicit) { s } else { s.hidden() }),
                        )
                        .child(
                            text(field(|t| t.artists.clone()))
                                .number_of_lines(1)
                                .text_sm()
                                .flex_1()
                                .text_color(move || theme().muted_foreground),
                        ),
                ),
        )
        .child(
            text(field(|t| t.album.clone()))
                .number_of_lines(1)
                .flex_1()
                .min_w(px(0.0))
                .text_sm()
                .text_color(move || theme().muted_foreground)
                .style_with(move |s| if show_album() { s } else { s.hidden() }),
        )
        .child(
            text(move || track().map(|t| clock(t.duration_ms)).unwrap_or_default())
                .w_12()
                .text_right()
                .text_sm()
                .tabular_nums()
                .text_color(move || theme().muted_foreground),
        )
}
