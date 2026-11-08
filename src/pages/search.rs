//! The search page: a field, then songs, artists, albums and playlists.

use std::time::Duration;

use nsh_base::{input, theme};
use nsh_core::prelude::*;
use serde_json::json;

use crate::{
    components::{card_row, page_message, track_header, track_row},
    spotify::SearchResults,
    store::Store,
    ui::{Icon, icon, section_title},
};

pub fn search(store: Store) -> View {
    let results = signal(None::<Result<SearchResults, String>>);
    let searching = signal(false);
    // A search per pause in typing; a newer one cancels the last.
    effect(move || {
        let query = store.query.get().trim().to_owned();
        let _ = store.epoch.get();
        if query.is_empty() {
            results.set(None);
            searching.set(false);
            return;
        }
        searching.set(true);
        spawn(async move {
            timer(Duration::from_millis(280)).await;
            let answer = store.load::<SearchResults>("search", json!([query])).await;
            results.set(Some(answer));
            searching.set(false);
        });
    });
    let wide = move || window().width.0 > 1100.0;
    view()
        .flex_1()
        .child(
            view().pt_16().px_6().pb_4().child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .h_12()
                    .px_4()
                    .max_w(px(480.0))
                    .rounded_full()
                    .bg(move || theme().secondary)
                    .border_1()
                    .border_color(move || theme().secondary)
                    .focus_within(|s| s.border_color(move || theme().foreground))
                    .text_color(move || theme().muted_foreground)
                    .child(icon(Icon::Search, 20.0))
                    .child(
                        input()
                            .value(move || store.query.get())
                            .on_change(move |text: &str| store.query.set(text.to_owned()))
                            .placeholder("What do you want to play?")
                            .placeholder_color(|| color(theme().muted_foreground))
                            .caret_color(|| color(theme().primary))
                            .auto_focus(true)
                            .flex_1()
                            .text_base()
                            .text_color(move || theme().foreground),
                    ),
            ),
        )
        .child(
            scroll().flex_1().child(dynamic(move || {
                let answer = results.get();
                let query_empty = store.query.with(|q| q.trim().is_empty());
                untracked(move || match answer {
                    _ if query_empty => {
                        page_message("Search Spotify", "Find songs, artists, albums and playlists.").into_element()
                    }
                    None => page_message("Searching", "").into_element(),
                    Some(Err(error)) => page_message("Search didn’t work", error).into_element(),
                    Some(Ok(found)) if found.is_empty() => {
                        page_message("No results", "Check the spelling, or try fewer words.").into_element()
                    }
                    Some(Ok(found)) => {
                        let tracks = found.tracks.clone();
                        view()
                            .pb_8()
                            .gap_8()
                            .child(when(move || !tracks.is_empty(), {
                                let tracks = found.tracks.clone();
                                move || {
                                    let shown = tracks.clone();
                                    view()
                                        .gap_2()
                                        .child(section_title("Songs").px_6())
                                        .child(track_header(wide))
                                        .children(shown.into_iter().enumerate().map(move |(i, t)| {
                                            let uri = t.uri.clone();
                                            let t = Some(t);
                                            track_row(store, move || t.clone(), move || i, wide, move || store.play_track(&uri))
                                        }))
                                }
                            }))
                            .child((!found.artists.is_empty()).then(|| card_row(store, "Artists", found.artists.clone()).px_3()))
                            .child((!found.albums.is_empty()).then(|| card_row(store, "Albums", found.albums.clone()).px_3()))
                            .child(
                                (!found.playlists.is_empty())
                                    .then(|| card_row(store, "Playlists", found.playlists.clone()).px_3()),
                            )
                            .into_element()
                    }
                })
            })),
        )
        .child(
            text("Searching…")
                .absolute()
                .top_4()
                .right_6()
                .text_xs()
                .text_color(move || theme().muted_foreground)
                .style_with(move |s| if searching.get() { s } else { s.hidden() }),
        )
}
