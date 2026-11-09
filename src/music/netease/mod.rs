//! NetEase Cloud Music pure-native client and playback engine.

pub mod client;
pub mod crypto;
pub mod player;
pub mod session;

pub use client::NetEaseClient;
pub use player::{PlayerManager, get_audio_tap};
pub use session::start;
