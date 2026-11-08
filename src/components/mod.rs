//! What the pages share.

mod about;
mod cards;
mod player_bar;
mod sidebar;
mod track_row;

pub use about::about;
pub use cards::{card_grid, card_row};
pub use player_bar::player_bar;
pub use sidebar::sidebar;
pub use track_row::{track_header, track_row};

use nsh_base::theme;
use nsh_core::prelude::*;

/// A page's message in place of its content: loading, empty, an error.
pub fn page_message(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> View {
    let detail: SharedString = detail.into();
    view()
        .pt_16()
        .px_6()
        .gap_2()
        .items_center()
        .child(text(title.into()).text_lg().font_weight(FontWeight::BOLD).text_color(move || theme().foreground))
        .child((!detail.is_empty()).then(|| {
            text(detail)
                .text_sm()
                .text_center()
                .max_w(px(420.0))
                .text_color(move || theme().muted_foreground)
        }))
}
