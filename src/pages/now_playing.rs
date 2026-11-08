//! The full-screen Now Playing view: an aurora tinted by the track, the big
//! album art, and the transport. It is NSH showing a live shader, animation
//! and native layout behind one coherent Spotify-style screen.

use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::json;

use crate::{
    spotify::{Repeat, clock},
    store::Store,
    ui::{Icon, bar_slider, cover, icon_button, motion, play_button, shaders},
};

pub fn now_playing(store: Store) -> impl IntoElement {
    let field = move |f: fn(&crate::spotify::PlayerState) -> String| move || store.player.with(f);
    let colors = move || shaders::colors_for(&store.player.with(|p| p.track_uri.clone()));
    // The art breathes while playing.
    let float = animated_value(0.0);
    effect(move || {
        let playing = store.player.with(|p| p.playing);
        if playing {
            float.timing_with(1.0, nsh_core::element::Timing { delay_ms: 0.0, ..motion::ENTER });
        } else {
            float.timing(0.0, 600.0, nsh_core::animation::Easing::Linear);
        }
    });

    let position = move || store.seeking.get().unwrap_or_else(|| store.player.with(|p| p.position_ms as f64));
    let duration = move || store.player.with(|p| p.duration_ms.max(1) as f64);

    let screen = view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_8()
        .p_8()
        .child(
            // The album art, lifted on a soft shadow, floating a little.
            view()
                .bind_translate_y(&float.interpolate(&[0.0, 1.0], &[0.0, -10.0]))
                .child(
                    cover(field(|p| p.artwork.clone()), None, false, Icon::Music)
                        .w(px(360.0))
                        .max_w(px(360.0))
                        .rounded(px(16.0))
                        .shadow_2xl(),
                ),
        )
        .child(
            view()
                .w_full()
                .max_w(px(520.0))
                .gap_6()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_4()
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(0.0))
                                .gap_1()
                                .child(
                                    text(move || {
                                        let t = store.player.with(|p| p.title.clone());
                                        if t.is_empty() { "Nothing playing".to_owned() } else { t }
                                    })
                                    .number_of_lines(2)
                                    .text_3xl()
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(move || theme().foreground),
                                )
                                .child(
                                    text(move || store.player.with(|p| if p.ad { "Advertisement".to_owned() } else { p.artist.clone() }))
                                        .number_of_lines(1)
                                        .text_base()
                                        .text_color(move || theme().muted_foreground),
                                ),
                        )
                        .child(icon_button(
                            move || if store.player.with(|p| p.liked) { Icon::HeartFilled } else { Icon::Heart },
                            || "Save to Liked Songs",
                            24.0,
                            move || store.player.with(|p| p.liked),
                            move || store.control("like", json!([])),
                        )),
                )
                .child(
                    view()
                        .gap_1()
                        .child(
                            bar_slider(
                                "Playback position",
                                position,
                                duration,
                                move |v| store.seeking.set(Some(v)),
                                move |v| {
                                    store.player.update(|p| p.position_ms = v as u64);
                                    store.seeking.set(None);
                                    store.control("seek", json!([v]));
                                },
                            )
                            .disabled(move || !store.player.with(|p| p.has_track())),
                        )
                        .child(
                            view()
                                .flex_row()
                                .justify_between()
                                .child(text(move || clock(position() as u64)).text_xs().tabular_nums().text_color(move || theme().muted_foreground))
                                .child(text(move || clock(duration() as u64)).text_xs().tabular_nums().text_color(move || theme().muted_foreground)),
                        ),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_6()
                        .child(icon_button(|| Icon::Shuffle, || "Shuffle", 22.0, move || store.player.with(|p| p.shuffle), move || store.control("shuffle", json!([]))))
                        .child(icon_button(|| Icon::SkipBack, || "Previous", 26.0, || false, move || store.control("previous", json!([]))))
                        .child(play_button(move || store.player.with(|p| p.playing), 68.0, false, move || store.toggle()))
                        .child(icon_button(|| Icon::SkipForward, || "Next", 26.0, || false, move || store.control("next", json!([]))))
                        .child(icon_button(
                            move || if store.player.with(|p| p.repeat == Repeat::Track) { Icon::RepeatOne } else { Icon::Repeat },
                            || "Repeat",
                            22.0,
                            move || store.player.with(|p| p.repeat != Repeat::Off),
                            move || store.control("repeat", json!([])),
                        )),
                ),
        );

    // The aurora fills the view, with the screen over it; a chevron closes it.
    view()
        .flex_1()
        .child(shaders::ambient(colors, screen))
        .child(
            icon_button(|| Icon::ChevronDown, || "Close", 24.0, || false, move || store.now_playing.set(false))
                .absolute()
                .top_4()
                .left_4(),
        )
}
