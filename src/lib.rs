//! Native NetEase Cloud Music playback and platform integrations for the TSX app.

pub mod audio;
pub mod exports;
pub mod media;
pub mod music;
pub mod theme;
pub mod version;

use nsh_core::prelude::*;

pub use audio::AudioMeter;
pub use exports::MusicEngine;
pub use media::MediaControls;
pub use theme::SystemTheme;
pub use version::Version;

/// The most the decoded images (covers, avatars) keep in memory. NSH decodes
/// each at the size it is shown and keeps the downloads on disk, but holds
/// the images that left the screen until its budget (a sixteenth of the
/// machine's memory, up to 256 MiB) fills. A small one keeps little more
/// than what is shown: an image shown again is decoded again from the disk
/// cache.
const IMAGE_MEMORY: usize = 32 << 20;

/// The crate's UI set: what `App.tsx` imports from `nsh-music/native`.
#[native_module("nsh-music/native")]
fn native(registry: &mut Registry) {
    nsh_core::image::budget().set_limit(IMAGE_MEMORY.min(nsh_core::image::default_budget()));
    registry
        .class::<MusicEngine>()
        .class::<MediaControls>()
        .class::<SystemTheme>()
        .class::<AudioMeter>()
        .module::<Version>();
}
