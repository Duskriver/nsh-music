//! Nshtify: a native client for Spotify, written with NSH.
//!
//! Spotify's own web player does the playing, in a headless Chromium the
//! app runs and drives over the DevTools protocol (`spotify`); everything
//! the user sees is the app's own native UI (`shell`, `pages`,
//! `components`), drawn by NSH on the platform's views.

mod components;
mod pages;
mod shell;
pub mod spotify;
pub mod store;
mod ui;

use nsh_base::theme;
use nsh_core::{element::Root, platform::Appearance, prelude::*};

/// The app's UI set (`nsh.json` `"rust"`).
#[native_module("nshtify/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// The app's root: the store, and the shell on the window's background.
#[component]
pub fn App() -> impl IntoElement {
    let store = store::Store::new();
    provide_context(store);
    view()
        .flex_1()
        .bg(move || ui::theme::shell())
        .text_color(move || theme().foreground)
        .child(shell::shell(store))
}

/// The root with the app's theme and appearance, as `nsh run` mounts it
/// (`nsh.json` `"theme"`, `"appearance"`): for `cargo run` and the tests.
pub fn root() -> Root {
    Root::of::<App>()
        .tokens(nsh_base::Theme::from_json(include_str!("../theme.json")).expect("theme.json").into_tokens())
        .appearance(Appearance::Dark)
}
