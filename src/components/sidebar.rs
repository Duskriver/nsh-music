//! The sidebar: the main pages, the user's library and the account. On
//! narrower desktop windows it collapses to an icon rail — just the icons and
//! library covers, no labels.

use nsh_base::{ButtonElement, button, theme};
use nsh_core::{element::Position, prelude::*};

use crate::{
    spotify::Item,
    store::{Route, Store},
    ui::{Icon, cover, icon, pill_button, theme as tokens},
};

/// Whether the sidebar is an icon rail (a narrow desktop window).
fn rail() -> bool {
    window().width.0 < 1100.0
}

fn nav_item(store: Store, glyph: Icon, label: &'static str, route: Route) -> ButtonElement {
    let selected = {
        let route = route.clone();
        move || store.route() == route
    };
    button()
        .accessibility_label(label)
        .flex_row()
        .items_center()
        .gap_4()
        .h_10()
        .rounded(move || theme().radius)
        .style_with(move |s| if rail() { s.justify_center().px_0() } else { s.px_3() })
        .text_color(move || if selected() { theme().foreground } else { theme().muted_foreground })
        .hover(|s| s.text_color(move || theme().foreground))
        .active(|s| s.opacity(0.7))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(move || store.open_and_close_drawer(route.clone()))
        .child(icon(glyph, 24.0))
        .child(text(label).text_base().font_weight(FontWeight::BOLD).style_with(|s| if rail() { s.hidden() } else { s }))
}

fn library_row(store: Store, at: Position) -> impl IntoElement {
    let item = computed(move || store.library.with(|l| l.get(at.index()).cloned()));
    let field = move |f: fn(&Item) -> String| move || item.with(|i| i.as_ref().map(f).unwrap_or_default());
    let selected = move || item.with(|i| i.as_ref().is_some_and(|i| store.route() == Route::Collection(i.uri.clone())));
    let playing_here = move || {
        item.with(|i| {
            i.as_ref().is_some_and(|i| {
                let id = i.uri.rsplit(':').next().unwrap_or_default().to_owned();
                store.player.with(|p| p.playing && !id.is_empty() && p.context_href.ends_with(&id))
            })
        })
    };
    let glyph = move || match item.with(|i| i.as_ref().map(|i| i.kind.clone())).as_deref() {
        Some("artist") => Icon::User,
        Some("album") => Icon::Disc,
        Some("collection") => Icon::HeartFilled,
        _ => Icon::Music,
    };
    button()
        .accessibility_label(move || item.with(|i| i.as_ref().map(|i| SharedString::from(i.name.clone()))))
        .flex_row()
        .items_center()
        .gap_3()
        .p_2()
        .rounded(move || theme().radius)
        .style_with(move |s| if rail() { s.justify_center() } else { s })
        .style_with(move |s| if selected() { s.bg(move || theme().sidebar_accent) } else { s })
        .hover(|s| s.bg(move || theme().sidebar_accent))
        .active(|s| s.opacity(0.75))
        .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
        .cursor_pointer()
        .on_press(move || {
            if let Some(item) = item.get_untracked() {
                store.open_and_close_drawer(Route::Collection(item.uri));
            }
        })
        .child(dynamic(move || {
            let round = item.with(|i| i.as_ref().is_some_and(|i| i.kind == "artist"));
            let glyph = glyph();
            untracked(|| cover(field(|i| i.image.clone()), Some(48.0), round, glyph))
        }))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap_0p5()
                .style_with(|s| if rail() { s.hidden() } else { s })
                .child(
                    text(field(|i| i.name.clone()))
                        .number_of_lines(1)
                        .text_base()
                        .text_color(move || if playing_here() { theme().primary } else { theme().foreground }),
                )
                .child(
                    text(field(|i| i.subtitle.clone()))
                        .number_of_lines(1)
                        .text_sm()
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(
            view()
                .text_color(move || theme().primary)
                .style_with(move |s| if playing_here() && !rail() { s } else { s.hidden() })
                .child(icon(Icon::Volume, 16.0)),
        )
}

fn library(store: Store) -> View {
    let signed_in = move || store.signed_in.get() == Some(true);
    view()
        .flex_1()
        .min_h(px(0.0))
        .rounded(move || tokens::radius_panel())
        .bg(move || theme().sidebar)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .h_14()
                .style_with(|s| if rail() { s.justify_center().px_0() } else { s.px_5() })
                .text_color(move || theme().muted_foreground)
                .child(icon(Icon::Library, 24.0))
                .child(text("Your Library").text_base().font_weight(FontWeight::BOLD).style_with(|s| if rail() { s.hidden() } else { s })),
        )
        .child(
            when(signed_in, move || {
                virtual_list(move || store.library.with(Vec::len), move |at| library_row(store, at))
                    .keys(move || store.library.with(|l| l.iter().map(|i| i.uri.clone()).collect::<Vec<_>>()))
                    .estimated_item_size(px(64.0))
                    .content_padding(px(8.0))
                    .empty(
                        text(move || match store.library_error.get() {
                            Some(error) => format!("Your library didn’t load: {error}").into(),
                            None => SharedString::from("Loading your library…"),
                        })
                        .px_5()
                        .text_sm()
                        .text_color(move || theme().muted_foreground)
                        .style_with(|s| if rail() { s.hidden() } else { s }),
                    )
                    .flex_1()
            })
            .otherwise(move || {
                // Signed-out: a full card, or just a login button on the rail.
                dynamic(move || {
                    if rail() {
                        untracked(|| {
                            button()
                                .accessibility_label("Log in")
                                .m_2()
                                .size_12()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(move || theme().primary)
                                .text_color(move || theme().primary_foreground)
                                .active(|s| s.opacity(0.7))
                                .cursor_pointer()
                                .on_press(move || store.sign_in())
                                .child(icon(Icon::LogIn, 20.0))
                                .into_element()
                        })
                    } else {
                        untracked(|| {
                            view()
                                .m_2()
                                .p_4()
                                .gap_3()
                                .rounded(move || tokens::radius_card())
                                .bg(move || theme().secondary)
                                .child(text("Your playlists live here").text_base().font_weight(FontWeight::BOLD))
                                .child(
                                    text("Log in with your Spotify account, free or Premium, to see your library and play music.")
                                        .text_sm()
                                        .text_color(move || theme().muted_foreground),
                                )
                                .child(view().flex_row().child(pill_button("Log in", true, move || store.sign_in())))
                                .into_element()
                        })
                    }
                })
            }),
        )
}

fn account(store: Store) -> View {
    let name = move || store.me.with(|m| m.as_ref().map(|m| m.name.clone()).unwrap_or_default());
    let image = move || store.me.with(|m| m.as_ref().map(|m| m.image.clone()).unwrap_or_default());
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .h_14()
        .rounded(move || tokens::radius_panel())
        .bg(move || theme().sidebar)
        .style_with(|s| if rail() { s.justify_center().px_0() } else { s.px_4() })
        .style_with(move |s| if store.signed_in.get() == Some(true) { s } else { s.hidden() })
        .child(cover(image, Some(30.0), true, Icon::User))
        .child(
            text(move || {
                let name = name();
                if name.is_empty() { "Signed in".to_owned() } else { name }
            })
            .flex_1()
            .number_of_lines(1)
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .style_with(|s| if rail() { s.hidden() } else { s }),
        )
        .child(
            button()
                .accessibility_label("Log out")
                .size_8()
                .items_center()
                .justify_center()
                .rounded_full()
                .style_with(|s| if rail() { s.hidden() } else { s })
                .text_color(move || theme().muted_foreground)
                .hover(|s| s.text_color(move || theme().foreground))
                .active(|s| s.opacity(0.6))
                .focus(|s| s.outline(px(2.0)).outline_color(move || theme().ring))
                .cursor_pointer()
                .on_press(move || store.sign_out())
                .child(icon(Icon::LogOut, 18.0)),
        )
}

fn footer(store: Store) -> View {
    let link = |label: &'static str, on_press: fn(Store)| {
        view()
            .role(AccessibilityRole::Button)
            .label(label)
            .cursor_pointer()
            .text_color(move || theme().muted_foreground)
            .hover(|s| s.text_color(move || theme().foreground))
            .active(|s| s.opacity(0.6))
            .on_press(move || on_press(store))
            .child(text(label).text_xs().font_weight(FontWeight::MEDIUM))
    };
    view()
        .px_4()
        .py_3()
        .gap_2()
        // Hidden on the rail, where there is no room for text links.
        .style_with(|s| if rail() { s.hidden() } else { s })
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_x_4()
                .gap_y_1()
                .child(link("Get Premium", |s| s.open_premium()))
                .child(link("spotify.com", |s| s.open_external("https://www.spotify.com/")))
                .child(link("About", |s| s.about.set(true))),
        )
        .child(
            view()
                .role(AccessibilityRole::Button)
                .label("Built with NSH")
                .cursor_pointer()
                .on_press(move || store.open_external("https://nsh.dev"))
                .child(
                    text("Built with NSH — native UI in Rust")
                        .text_xs()
                        .text_color(move || theme().muted_foreground.opacity(0.7))
                        .hover(|s| s.text_color(move || theme().muted_foreground)),
                ),
        )
}

pub fn sidebar(store: Store) -> View {
    view()
        // Full on wide windows, an icon rail on narrow ones.
        .w(move || if rail() { px(72.0) } else { px(300.0) })
        .flex_shrink_0()
        .gap_2()
        .text_color(move || theme().sidebar_foreground)
        .child(
            view()
                .rounded(move || tokens::radius_panel())
                .bg(move || theme().sidebar)
                .style_with(|s| if rail() { s.p_1() } else { s.p_2() })
                .child(nav_item(store, Icon::Home, "Home", Route::Home))
                .child(nav_item(store, Icon::Search, "Search", Route::Search)),
        )
        .child(library(store))
        .child(account(store))
        .child(footer(store))
}
