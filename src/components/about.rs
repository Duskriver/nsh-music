//! The About panel: the Spotify disclaimer first, then a short word on NSH,
//! with links. Reached from the sidebar.

use nsh_base::{button, theme};
use nsh_core::prelude::*;

use crate::{
    store::Store,
    ui::{Icon, icon, pill_button, theme as tokens},
};

const REPO: &str = "https://github.com/huacnlee/nshtify";
const NSH: &str = "https://nsh.dev";
const TERMS: &str = "https://www.spotify.com/legal/end-user-agreement/";
const SPOTIFY: &str = "https://www.spotify.com/";

/// A small inline link.
fn link(store: Store, label: &'static str, url: &'static str) -> impl IntoElement {
    text(label)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(move || theme().primary)
        .role(AccessibilityRole::Link)
        .hover(|s| s.underline())
        .cursor_pointer()
        .on_press(move || store.open_external(url))
}

fn paragraph(body: &'static str) -> impl IntoElement {
    text(body).text_sm().line_height(px(21.0)).text_color(move || theme().muted_foreground)
}

pub fn about(store: Store) -> impl IntoElement {
    view()
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .p_4()
        .bg(move || tokens::shell().opacity(0.82))
        .style_with(move |s| if store.about.get() { s } else { s.hidden() })
        // A press on the backdrop closes it; the panel keeps its own presses.
        .on_press(move || store.about.set(false))
        .child(
            // The rounded panel clips its scroll, so the corners stay round.
            view()
                .w(px(440.0))
                .max_w(px(440.0))
                .max_h(px(640.0))
                .rounded(move || tokens::radius_panel())
                .overflow_hidden()
                .bg(move || theme().popover)
                .shadow_2xl()
                .on_press(|| {})
                .child(scroll().flex_1().child(
                    view()
                        .p_6()
                        .gap_5()
                        .child(header(store))
                        .child(disclaimer(store))
                        .child(premium(store))
                        .child(built_with_nsh(store))
                        .child(links(store)),
                )),
        )
}

fn header(store: Store) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(view().text_color(move || theme().primary).child(icon(Icon::Music, 26.0)))
                .child(
                    view()
                        .child(text("Spotish").text_xl().font_weight(FontWeight::EXTRA_BOLD).text_color(move || theme().foreground))
                        .child(text("A native player for Spotify").text_xs().text_color(move || theme().muted_foreground)),
                ),
        )
        .child(
            button()
                .accessibility_label("Close")
                .size_8()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_color(move || theme().muted_foreground)
                .hover(|s| s.text_color(move || theme().foreground).bg(move || tokens::row_hover()))
                .active(|s| s.opacity(0.6))
                .cursor_pointer()
                .on_press(move || store.about.set(false))
                .child(icon(Icon::Close, 18.0)),
        )
}

/// The Spotify disclaimer — first, and the most important.
fn disclaimer(store: Store) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            text("Not affiliated with Spotify")
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(move || theme().foreground),
        )
        .child(paragraph(
            "Spotish is an unofficial, personal project, not affiliated with, endorsed by, or sponsored by Spotify. It \
             plays through Spotify’s own web player, so a Spotify account is needed and Spotify’s terms apply. Spotify, \
             its logo and all music, artwork and trademarks belong to Spotify and the rights holders.",
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_x_4()
                .gap_y_1()
                .child(link(store, "Spotify Terms", TERMS))
                .child(link(store, "spotify.com", SPOTIFY)),
        )
}

fn premium(store: Store) -> impl IntoElement {
    view()
        .gap_2()
        .p_4()
        .rounded(move || tokens::radius_card())
        .bg(move || theme().secondary)
        .child(text("Keeping the ads? Spotify Premium removes them.").text_sm().font_weight(FontWeight::SEMIBOLD).text_color(move || theme().foreground))
        .child(paragraph("Spotish never blocks Spotify’s ads. Premium removes them, plays any track on demand, and supports artists."))
        .child(view().flex_row().pt_1().child(pill_button("Get Spotify Premium", true, move || store.open_premium())))
}

/// A short word on NSH, with a link.
fn built_with_nsh(store: Store) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_baseline()
        .gap_x_1()
        .child(text("Built with").text_sm().text_color(move || theme().muted_foreground))
        .child(link(store, "NSH", NSH))
        .child(paragraph("— a Rust framework for native UIs on desktop, mobile, web and the terminal. This window is all native."))
}

fn links(store: Store) -> impl IntoElement {
    let row = move |label: &'static str, url: &'static str| {
        button()
            .flex_row()
            .items_center()
            .justify_between()
            .h_11()
            .px_4()
            .rounded(move || theme().radius)
            .bg(move || theme().secondary)
            .hover(|s| s.bg(move || tokens::row_hover()))
            .active(|s| s.opacity(0.7))
            .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
            .cursor_pointer()
            .on_press(move || store.open_external(url))
            .child(text(label).text_sm().font_weight(FontWeight::MEDIUM).text_color(move || theme().foreground))
            .child(view().text_color(move || theme().muted_foreground).child(icon(Icon::ChevronRight, 16.0)))
    };
    view().gap_2().child(row("Project on GitHub", REPO)).child(row("Learn more about NSH", NSH))
}
