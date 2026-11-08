//! A collection's page: a playlist, album, artist or Liked Songs, with its
//! header, a play button and its tracks in a virtualized list.

use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::json;

use crate::{
    components::{card_grid, page_message, track_header, track_row},
    spotify::{Collection, Track},
    store::Store,
    ui::{Icon, cover, play_button, section_title},
};

fn kind_of(uri: &str) -> &str {
    uri.split(':').nth(1).unwrap_or_default()
}

/// What plays a collection, to tell whether it is the one playing.
fn href_of(uri: &str) -> String {
    let mut parts = uri.split(':').skip(1);
    match (parts.next(), parts.next()) {
        (Some("collection"), _) => "/collection/tracks".to_owned(),
        (Some(kind), Some(id)) => format!("/{kind}/{id}"),
        _ => String::new(),
    }
}

pub fn collection(store: Store, uri: String) -> View {
    let kind = kind_of(&uri).to_owned();
    let query = match kind.as_str() {
        "album" => "album",
        "artist" => "artist",
        "collection" => "likedSongs",
        _ => "playlist",
    };
    let data = {
        let uri = uri.clone();
        resource(move || Some(store.epoch.get()), move |_, _| {
            let args = if query == "likedSongs" { json!([]) } else { json!([uri.clone()]) };
            store.load::<Collection>(query, args)
        })
    };
    view().flex_1().child(dynamic(move || {
        let loaded = data.get();
        let error = data.error();
        let uri = uri.clone();
        untracked(move || match (loaded, error) {
            (Some(c), _) => page(store, uri, c).into_element(),
            (None, Some(error)) => page_message("This page didn’t load", error).into_element(),
            _ => page_message("Loading", "").into_element(),
        })
    }))
}

fn header(store: Store, uri: &str, c: &Collection) -> View {
    let artist = c.header.kind == "artist";
    let liked = c.header.kind == "collection";
    let image = c.header.image.clone();
    let label = match c.header.kind.as_str() {
        "album" => "Album",
        "artist" => "Artist",
        _ => "Playlist",
    };
    let href = href_of(uri);
    let playing_here = computed(move || store.player.with(|p| p.playing && !href.is_empty() && p.context_href.ends_with(&href)));
    let playing_here = move || playing_here.get();
    let play = {
        let uri = uri.to_owned();
        let first = c.tracks.first().map(|t| t.uri.clone());
        move || {
            if playing_here() {
                store.toggle();
            } else if artist {
                // The artist page's tracks have no context of their own here.
                if let Some(first) = &first {
                    store.play_track(first);
                }
            } else {
                store.play_item(&crate::spotify::Item { uri: uri.clone(), kind: "playlist".into(), ..Default::default() });
            }
        }
    };
    view()
        .gap_6()
        .child(
            view()
                .flex_row()
                .items_end()
                .gap_6()
                .pt_16()
                .px_6()
                .child(
                    cover(move || image.clone(), Some(200.0), artist, if liked { Icon::HeartFilled } else { Icon::Music })
                        .shadow_xl(),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap_2()
                        .child(text(label).text_sm().font_weight(FontWeight::MEDIUM))
                        .child(
                            text(c.header.name.clone())
                                .number_of_lines(2)
                                .text_size(px(56.0))
                                .line_height(px(62.0))
                                .font_weight(FontWeight::EXTRA_BOLD)
                                .text_color(move || theme().foreground),
                        )
                        .child((!c.header.description.is_empty()).then(|| {
                            text(c.header.description.clone())
                                .number_of_lines(2)
                                .text_sm()
                                .text_color(move || theme().muted_foreground)
                        }))
                        .child(text(c.header.subtitle.clone()).text_sm().font_weight(FontWeight::SEMIBOLD)),
                ),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .px_6()
                .pb_2()
                .child(play_button(playing_here, 56.0, true, play).disabled(c.tracks.is_empty())),
        )
}

fn page(store: Store, uri: String, c: Collection) -> View {
    let tracks: Signal<Vec<Track>> = signal(c.tracks.clone());
    let artist = c.header.kind == "artist";
    let wide = move || window().width.0 > 1100.0 && !artist;
    let releases = c.releases.clone();
    let head = view()
        .child(header(store, &uri, &c))
        .child(when(move || artist, || section_title("Popular").px_6().pb_2()))
        .child(track_header(wide));
    let foot = view().pb_8().child((!releases.is_empty()).then(move || {
        view()
            .pt_8()
            .px_3()
            .gap_2()
            .child(section_title("Discography").px_3())
            .child(card_grid(store, releases))
    }));
    let context = uri.clone();
    view().flex_1().child(
        virtual_list(move || tracks.with(Vec::len), move |at| {
            let track = computed(move || tracks.with(|t| t.get(at.index()).cloned()));
            let context = context.clone();
            track_row(store, move || track.get(), move || at.index(), wide, move || {
                let Some(t) = track.get_untracked() else { return };
                if artist {
                    store.play_track(&t.uri);
                } else {
                    store.play_at(&context, at.index());
                }
            })
        })
        .keys(move || tracks.with(|t| t.iter().map(|t| format!("{}{}", t.uid, t.uri)).collect::<Vec<_>>()))
        .estimated_item_size(px(56.0))
        .header(head)
        .footer(foot)
        .empty(page_message("Nothing here yet", "").pt_0())
        .flex_1(),
    )
}
