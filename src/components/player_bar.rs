//! The player bar: what plays, the controls and the volume.

use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::json;

use crate::{
    spotify::{Repeat, clock},
    store::{Route, Store},
    ui::{Icon, bar_slider, cover, icon_button, play_button, theme as tokens},
};

fn now_playing(store: Store) -> View {
    let field = move |f: fn(&crate::spotify::PlayerState) -> String| move || store.player.with(f);
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .w(px(300.0))
        .min_w(px(0.0))
        .child(
            // The art opens the full-screen Now Playing view.
            view()
                .cursor_pointer()
                .rounded(px(6.0))
                .hover(|s| s.opacity(0.85))
                .active(|s| s.scale(0.96))
                .role(AccessibilityRole::Button)
                .label("Open the now playing view")
                .on_press(move || {
                    if store.player.with_untracked(|p| p.has_track()) {
                        store.now_playing.set(true)
                    }
                })
                .child(cover(field(|p| p.artwork.clone()), Some(56.0), false, Icon::Music)),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap_0p5()
                .cursor_pointer()
                // The title opens what plays, when it is a page of the app's.
                .on_press(move || {
                    let href = store.player.with_untracked(|p| p.context_href.clone());
                    let mut parts = href.trim_start_matches('/').split('/');
                    if let (Some(kind @ ("playlist" | "album" | "artist")), Some(id)) = (parts.next(), parts.next()) {
                        store.open(Route::Collection(format!("spotify:{kind}:{id}")));
                    }
                })
                .child(
                    text(move || {
                        let title = store.player.with(|p| p.title.clone());
                        if title.is_empty() { "Nothing playing".to_owned() } else { title }
                    })
                    .number_of_lines(1)
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(move || theme().foreground),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            text(move || {
                                store.player.with(|p| if p.ad { "Advertisement".to_owned() } else { p.artist.clone() })
                            })
                            .number_of_lines(1)
                            .text_xs()
                            .text_color(move || theme().muted_foreground),
                        )
                        // During an ad, an honest nudge to Premium (Nshtify
                        // never skips or blocks the ad).
                        .child(when(
                            move || store.player.with(|p| p.ad),
                            move || {
                                text("Get Premium")
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(move || theme().primary)
                                    .role(AccessibilityRole::Button)
                                    .on_press(move || store.open_premium())
                            },
                        )),
                ),
        )
        .child(
            icon_button(
                move || if store.player.with(|p| p.liked) { Icon::HeartFilled } else { Icon::Heart },
                move || if store.player.with(|p| p.liked) { "Remove from Liked Songs" } else { "Save to Liked Songs" },
                18.0,
                move || store.player.with(|p| p.liked),
                move || store.control("like", json!([])),
            )
            .disabled(move || !store.player.with(|p| p.has_track() && !p.ad)),
        )
}

fn transport(store: Store) -> View {
    let has_track = move || store.player.with(|p| p.has_track());
    let position = move || {
        store.seeking.get().unwrap_or_else(|| store.player.with(|p| p.position_ms as f64))
    };
    let duration = move || store.player.with(|p| p.duration_ms.max(1) as f64);
    view()
        .flex_1()
        .max_w(px(720.0))
        .items_center()
        .gap_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_4()
                .child(
                    icon_button(
                        || Icon::Shuffle,
                        || "Shuffle",
                        18.0,
                        move || store.player.with(|p| p.shuffle),
                        move || store.control("shuffle", json!([])),
                    )
                    .disabled(move || !store.player.with(|p| p.shuffle_available)),
                )
                .child(
                    icon_button(|| Icon::SkipBack, || "Previous", 18.0, || false, move || store.control("previous", json!([])))
                        .disabled(move || !store.player.with(|p| p.can_previous)),
                )
                .child(play_button(move || store.player.with(|p| p.playing), 36.0, false, move || store.toggle()).disabled(
                    move || !has_track() && store.player.with(|p| !p.can_play),
                ))
                .child(
                    icon_button(|| Icon::SkipForward, || "Next", 18.0, || false, move || store.control("next", json!([])))
                        .disabled(move || !store.player.with(|p| p.can_next)),
                )
                .child(icon_button(
                    move || if store.player.with(|p| p.repeat == Repeat::Track) { Icon::RepeatOne } else { Icon::Repeat },
                    move || match store.player.with(|p| p.repeat) {
                        Repeat::Off => "Repeat",
                        Repeat::Context => "Repeat one",
                        Repeat::Track => "Don’t repeat",
                    },
                    18.0,
                    move || store.player.with(|p| p.repeat != Repeat::Off),
                    move || store.control("repeat", json!([])),
                )),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .w_full()
                .child(
                    text(move || clock(position() as u64))
                        .w_10()
                        .text_right()
                        .text_xs()
                        .tabular_nums()
                        .text_color(move || theme().muted_foreground),
                )
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
                    .disabled(move || !has_track())
                    .flex_1(),
                )
                .child(
                    text(move || if has_track() { clock(duration() as u64) } else { clock(0) })
                        .w_10()
                        .text_xs()
                        .tabular_nums()
                        .text_color(move || theme().muted_foreground),
                ),
        )
}

fn volume(store: Store) -> View {
    let level = move || store.volume_draft.get().unwrap_or_else(|| store.player.with(|p| if p.muted { 0.0 } else { p.volume as f64 }));
    view()
        .w(px(300.0))
        .flex_row()
        .items_center()
        .justify_end()
        .gap_1()
        .child(icon_button(
            move || match level() {
                v if v <= 0.0 => Icon::VolumeMute,
                v if v < 0.5 => Icon::VolumeLow,
                _ => Icon::Volume,
            },
            move || if store.player.with(|p| p.muted) { "Unmute" } else { "Mute" },
            18.0,
            || false,
            move || store.control("mute", json!([])),
        ))
        .child(
            bar_slider(
                "Volume",
                level,
                || 1.0,
                move |v| store.volume_draft.set(Some(v)),
                move |v| {
                    store.player.update(|p| p.volume = v as f32);
                    store.volume_draft.set(None);
                    store.control("volume", json!([v]));
                },
            )
            .w(px(110.0)),
        )
}

pub fn player_bar(store: Store) -> View {
    view()
        .h(px(76.0))
        .flex_shrink_0()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .px_2()
        .bg(move || tokens::player())
        .child(now_playing(store))
        .child(transport(store))
        .child(volume(store))
}
