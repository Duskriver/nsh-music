//! Cards of items, in rows that scroll sideways.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::prelude::*;

use crate::{
    spotify::Item,
    store::{Route, Store},
    ui::{Icon, cover, section_title, theme as tokens},
};

const CARD_WIDTH: f32 = 180.0;

fn fallback(kind: &str) -> Icon {
    match kind {
        "artist" => Icon::User,
        "album" => Icon::Disc,
        _ => Icon::Music,
    }
}

/// Opens an item's page, or plays it when it has none (a track).
pub fn activate(store: Store, item: &Item) {
    if item.opens() {
        store.open(Route::Collection(item.uri.clone()));
    } else {
        store.play_item(item);
    }
}

/// One card: the picture, the name and what it is.
pub fn card(store: Store, item: Item) -> ButtonElement {
    let round = item.kind == "artist";
    let image = item.image.clone();
    let label = format!("{} {}", item.name, item.subtitle);
    let kind = item.kind.clone();
    let name = item.name.clone();
    let subtitle = item.subtitle.clone();
    button()
        .accessibility_label(label)
        .w(px(CARD_WIDTH))
        .flex_shrink_0()
        .p_3()
        .gap_2()
        .rounded(move || tokens::radius_card())
        .hover(|s| s.bg(move || theme().card))
        .active(|s| s.opacity(0.8))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(move || activate(store, &item))
        .child(cover(move || image.clone(), None, round, fallback(&kind)))
        .child(
            view()
                .gap_1()
                .child(
                    text(name)
                        .number_of_lines(1)
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || theme().foreground),
                )
                .child(
                    text(subtitle)
                        .number_of_lines(2)
                        .text_sm()
                        .text_color(move || theme().muted_foreground),
                ),
        )
}

/// A titled row of cards that scrolls sideways.
pub fn card_row(store: Store, title: impl Into<SharedString>, items: Vec<Item>) -> View {
    view()
        .gap_2()
        .child(section_title(title.into()).px_3())
        .child(
            scroll()
                .horizontal()
                .scrollbar(false)
                .child(view().flex_row().children(items.into_iter().map(move |item| card(store, item)))),
        )
}

/// Cards that wrap into a grid (an artist's releases).
pub fn card_grid(store: Store, items: Vec<Item>) -> View {
    view().flex_row().flex_wrap().children(items.into_iter().map(move |item| card(store, item)))
}
