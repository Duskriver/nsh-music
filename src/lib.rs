//! Spotish's Rust crate: the engine that runs and drives Spotify's own web
//! player (`spotify`), exported to the TSX UI as the `Spotify` class
//! (`exports`). The screens are TSX (`App.tsx`); this crate ships in the
//! binary and does the work TSX cannot: processes, sockets and the browser.

pub mod exports;
pub mod spotify;

use nsh_core::prelude::*;

pub use exports::Spotify;

/// The crate's UI set: what `App.tsx` imports from `spotish/native`.
#[native_module("spotish/native")]
fn native(registry: &mut Registry) {
    registry.class::<Spotify>();
}
