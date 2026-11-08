//! The window's layout: a desktop workspace (sidebar + main + player bar) that
//! becomes a phone layout (full-width pages, a bottom tab bar and a mini
//! player) as the window narrows. Over both sit the full-screen Now Playing
//! view, the About panel, the sign-in prompt and notices.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{about, player_bar, sidebar},
    pages,
    spotify::Status,
    store::{Route, Store},
    ui::{Icon, cover, history_button, icon, motion, pill_button, play_button, theme as tokens},
};

/// The width below which the phone layout is used.
const NARROW: f32 = 560.0;

fn premium_pill(store: Store) -> View {
    // Shown until we know the account is Premium: the honest upgrade nudge.
    let show = move || store.signed_in.get() == Some(true) && !store.player.with(|p| p.premium);
    view()
        .style_with(move |s| if show() { s } else { s.hidden() })
        .child(pill_button("Explore Premium", false, move || store.open_premium()))
}

fn top_bar(store: Store, wide: bool) -> View {
    view()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h_16()
        .px_4()
        .flex_row()
        .items_center()
        .gap_2()
        .when(wide, |v| {
            v.child(history_button(Icon::ChevronLeft, "Go back", move || store.can_go_back(), move || store.back()))
                .child(history_button(Icon::ChevronRight, "Go forward", move || store.can_go_forward(), move || store.forward()))
        })
        .child(view().flex_1())
        .child(premium_pill(store))
        .child(
            view()
                .style_with(move |s| if store.signed_in.get() == Some(false) { s } else { s.hidden() })
                .child(pill_button("Log in", true, move || store.sign_in())),
        )
}

fn status_line(store: Store) -> View {
    let message = move || match store.status.get() {
        Status::Starting => Some(("Starting the Spotify web player…".to_owned(), false)),
        Status::Failed(error) => Some((error, true)),
        Status::Ready | Status::SigningIn => None,
    };
    view()
        .absolute()
        .top_16()
        .left_4()
        .right_4()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .rounded(move || theme().radius)
        .bg(move || theme().popover)
        .shadow_md()
        .text_color(move || if message().is_some_and(|m| m.1) { theme().destructive } else { theme().muted_foreground })
        .style_with(move |s| if message().is_some() { s } else { s.hidden() })
        .child(icon(Icon::Alert, 16.0))
        .child(text(move || message().map(|m| m.0).unwrap_or_default()).flex_1().text_sm())
}

/// The current page, animated in on each route change.
fn main_page(store: Store) -> View {
    view().flex_1().min_h(px(0.0)).child(dynamic(move || {
        let route = store.route();
        untracked(move || {
            let page = match route.clone() {
                Route::Home => pages::home(store).into_element(),
                Route::Search => pages::search(store).into_element(),
                Route::Collection(uri) => pages::collection(store, uri).into_element(),
            };
            motion::appear_page(page).key(format!("{route:?}"))
        })
    }))
}

fn main_view(store: Store, wide: bool) -> View {
    view()
        .flex_1()
        .min_w(px(0.0))
        .when(wide, |v| v.rounded(move || tokens::radius_panel()))
        .overflow_hidden()
        .bg(move || theme().background)
        .child(main_page(store))
        .child(top_bar(store, wide))
        .child(status_line(store))
}

// --- The phone layout -------------------------------------------------------

fn tab(store: Store, glyph: Icon, label: &'static str, route: Route) -> impl IntoElement {
    let selected = {
        let route = route.clone();
        move || !store.drawer.get() && store.route() == route
    };
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_1()
        .py_2()
        .role(AccessibilityRole::Tab)
        .label(label)
        .cursor_pointer()
        .text_color(move || if selected() { theme().foreground } else { theme().muted_foreground })
        .active(|s| s.opacity(0.6))
        .on_press(move || store.open_and_close_drawer(route.clone()))
        .child(icon(glyph, 24.0))
        .child(text(label).text_xs().font_weight(FontWeight::MEDIUM))
}

fn library_tab(store: Store) -> impl IntoElement {
    let selected = move || store.drawer.get();
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_1()
        .py_2()
        .role(AccessibilityRole::Tab)
        .label("Your Library")
        .cursor_pointer()
        .text_color(move || if selected() { theme().foreground } else { theme().muted_foreground })
        .active(|s| s.opacity(0.6))
        .on_press(move || store.drawer.update(|d| *d = !*d))
        .child(icon(Icon::Library, 24.0))
        .child(text("Library").text_xs().font_weight(FontWeight::MEDIUM))
}

fn mini_player(store: Store) -> View {
    let field = move |f: fn(&crate::spotify::PlayerState) -> String| move || store.player.with(f);
    view()
        .mx_2()
        .mb_2()
        .h_14()
        .flex_row()
        .items_center()
        .gap_3()
        .px_2()
        .rounded(move || tokens::radius_card())
        .bg(move || theme().card)
        .style_with(move |s| if store.player.with(|p| p.has_track()) { s } else { s.hidden() })
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .on_press(move || store.now_playing.set(true))
                .child(cover(field(|p| p.artwork.clone()), Some(44.0), false, Icon::Music).rounded(px(4.0)))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(text(field(|p| p.title.clone())).number_of_lines(1).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(move || theme().foreground))
                        .child(text(move || store.player.with(|p| if p.ad { "Advertisement".to_owned() } else { p.artist.clone() })).number_of_lines(1).text_xs().text_color(move || theme().muted_foreground)),
                ),
        )
        .child(play_button(move || store.player.with(|p| p.playing), 36.0, false, move || store.toggle()))
}

fn phone(store: Store) -> View {
    view()
        .flex_1()
        .child(main_view(store, false))
        .child(mini_player(store))
        .child(
            view()
                .flex_row()
                .items_stretch()
                .pt_1()
                .px_2()
                .pb_safe_area_plus(px(6.0))
                .border_t_1()
                .border_color(move || theme().border)
                .bg(move || tokens::player())
                .child(tab(store, Icon::Home, "Home", Route::Home))
                .child(tab(store, Icon::Search, "Search", Route::Search))
                .child(library_tab(store)),
        )
}

/// The library as a drawer that slides in on the phone layout.
fn drawer(store: Store) -> View {
    view()
        .absolute()
        .inset_0()
        .flex_row()
        .bg(move || tokens::shell().opacity(0.6))
        .style_with(move |s| if store.drawer.get() { s } else { s.hidden() })
        .on_press(move || store.drawer.set(false))
        .child(
            view()
                .w(px(320.0))
                .max_w(px(320.0))
                .flex_1()
                .p_2()
                .bg(move || tokens::shell())
                .on_press(|| {})
                .child(sidebar(store)),
        )
}

// --- The desktop layout -----------------------------------------------------

fn desktop(store: Store) -> View {
    view()
        .flex_1()
        .child(
            view()
                .flex_1()
                .min_h(px(0.0))
                .flex_row()
                .gap_2()
                .p_2()
                .pb_0()
                .child(sidebar(store))
                .child(main_view(store, true)),
        )
        .child(player_bar(store))
}

// --- Overlays ---------------------------------------------------------------

fn notice(store: Store) -> View {
    view()
        .absolute()
        .bottom(px(96.0))
        .left_0()
        .right_0()
        .items_center()
        .style_with(move |s| if store.notice.with(Option::is_some) { s } else { s.hidden() })
        .child(
            text(move || store.notice.get().unwrap_or_default())
                .px_4()
                .py_2()
                .rounded(move || theme().radius)
                .bg(move || theme().foreground)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().background)
                .shadow_lg(),
        )
}

fn signing_in(store: Store) -> View {
    view()
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .bg(move || tokens::shell().opacity(0.85))
        .style_with(move |s| if store.status.get() == Status::SigningIn { s } else { s.hidden() })
        .child(
            view()
                .w(px(400.0))
                .max_w(px(400.0))
                .p_8()
                .gap_4()
                .items_center()
                .rounded(move || tokens::radius_panel())
                .bg(move || theme().popover)
                .shadow_2xl()
                .child(view().text_color(move || theme().primary).child(icon(Icon::LogIn, 36.0)))
                .child(text("Log in to Spotify").text_xl().font_weight(FontWeight::BOLD).text_color(move || theme().foreground))
                .child(
                    text("A Spotify window has opened. Log in there — your account, or Google, Apple or Facebook — then come back and press “I’m logged in”. Spotish takes over playback from the window.")
                        .text_sm()
                        .text_center()
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    view()
                        .flex_row()
                        .gap_3()
                        .child(pill_button("Cancel", false, move || store.cancel_sign_in()))
                        .child(pill_button("I’m logged in", true, move || store.finish_sign_in())),
                ),
        )
}

/// The full-screen Now Playing view, brought up over everything.
fn now_playing_overlay(store: Store) -> View {
    // The overlay fills the window (a sized box), fades in, and holds the
    // Now Playing page, which fills it with flex.
    let shown = animated_value(0.0);
    effect(move || {
        if store.now_playing.get() {
            shown.timing_with(1.0, motion::ENTER);
        } else {
            shown.set(0.0);
        }
    });
    view()
        .absolute()
        .inset_0()
        .bg(move || theme().background)
        .bind_opacity(&shown)
        .style_with(move |s| if store.now_playing.get() { s } else { s.hidden() })
        .child(when(move || store.now_playing.get(), move || pages::now_playing(store)))
}

pub fn shell(store: Store) -> View {
    // The keys of a music player.
    on_key("space", move |_| store.toggle());
    on_key("mod+right", move |_| store.control("next", serde_json::json!([])));
    on_key("mod+left", move |_| store.control("previous", serde_json::json!([])));
    on_key("escape", move |_| {
        if store.now_playing.get_untracked() {
            store.now_playing.set(false);
        } else if store.about.get_untracked() {
            store.about.set(false);
        } else if store.drawer.get_untracked() {
            store.drawer.set(false);
        }
    });
    // The phone layout is for mobile devices, or a desktop window made very
    // narrow; desktop windows otherwise keep the desktop layout.
    let phone = move || !is_desktop() || window().width.0 < NARROW;
    view()
        .flex_1()
        .bg(move || tokens::shell())
        .child(dynamic(move || {
            let phone = phone();
            untracked(move || if phone { self::phone(store).key("phone") } else { desktop(store).key("desktop") })
        }))
        .child(when(move || phone(), move || drawer(store)))
        .child(now_playing_overlay(store))
        .child(about(store))
        .child(signing_in(store))
        .child(notice(store))
}
