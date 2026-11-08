//! The home page: Spotify's sections of the day, as rows of cards.

use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::json;

use crate::{
    components::{card_row, page_message},
    spotify::Home,
    store::Store,
};

pub fn home(store: Store) -> View {
    let data = resource(move || Some(store.epoch.get()), move |_, _| store.load::<Home>("home", json!([])));
    view().flex_1().child(scroll().flex_1().child(
        view()
            .pt_16()
            .pb_8()
            .px_3()
            .gap_8()
            .child(
                text(move || {
                    let greeting = data.with(|d| d.as_ref().map(|d| d.greeting.clone()).unwrap_or_default());
                    if greeting.is_empty() { "Home".to_owned() } else { greeting }
                })
                .px_3()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(move || theme().foreground),
            )
            .child(dynamic(move || {
                let home = data.get();
                let error = data.error();
                untracked(move || match (home, error) {
                    (Some(home), _) if !home.sections.is_empty() => view()
                        .gap_8()
                        .children(home.sections.into_iter().map(move |s| card_row(store, s.title, s.items)))
                        .into_element(),
                    (_, Some(error)) => page_message("Home didn’t load", error).into_element(),
                    _ => page_message("Loading", "Fetching what’s new for you…").into_element(),
                })
            })),
    ))
}
