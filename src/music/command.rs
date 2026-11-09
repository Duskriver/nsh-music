//! Commands shared by the native session and the offline demo.

use serde_json::Value;

use super::model::{EngineError, ErrorCode, Repeat};

/// A request to the music engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Home,
    Search(String),
    Playlist(String),
    Album(String),
    Artist(String),
    LikedSongs,
    Library,
    Me,
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    /// To this position, in milliseconds.
    Seek(f64),
    /// To this volume, 0 to 1.
    SetVolume(f64),
    SetShuffle(bool),
    SetRepeat(Repeat),
    SetLiked(bool),
    SetMuted(bool),
    /// A context, from `track_uri` when given.
    PlayContext {
        uri: String,
        track_uri: Option<String>,
    },
    PlayTrack(String),
    /// The extracted color of a cover image.
    Colors(String),
    /// A track's lyrics, by its URI.
    Lyrics(String),
}

impl Command {
    /// The command name used in diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Command::Home => "home",
            Command::Search(_) => "search",
            Command::Playlist(_) => "playlist",
            Command::Album(_) => "album",
            Command::Artist(_) => "artist",
            Command::LikedSongs => "likedSongs",
            Command::Library => "library",
            Command::Me => "me",
            Command::Toggle => "toggle",
            Command::Play => "play",
            Command::Pause => "pause",
            Command::Next => "next",
            Command::Previous => "previous",
            Command::Seek(_) => "seek",
            Command::SetVolume(_) => "setVolume",
            Command::SetShuffle(_) => "setShuffle",
            Command::SetRepeat(_) => "setRepeat",
            Command::SetLiked(_) => "setLiked",
            Command::SetMuted(_) => "setMuted",
            Command::PlayContext { .. } => "playContext",
            Command::PlayTrack(_) => "playTrack",
            Command::Colors(_) => "colors",
            Command::Lyrics(_) => "lyrics",
        }
    }

}

/// Decodes a data answer into its type, or `E_SHAPE`.
pub fn decode<T: serde::de::DeserializeOwned>(command: &Command, value: Value) -> Result<T, EngineError> {
    serde_json::from_value(value)
        .map_err(|e| EngineError::new(ErrorCode::Shape, format!("{} answered an unknown shape: {e}", command.name())))
}
