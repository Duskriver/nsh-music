//! Commands shared by the native session and the offline demo.

use super::model::{
    AudioQuality, Collection, CoverColor, EngineError, ErrorCode, Home, Library, Lyrics, Me, Repeat, SearchResults,
};

/// Typed result of an engine command, transferred directly without JSON round-trips.
#[derive(Debug, Clone)]
pub enum CommandOutput {
    /// Control commands that carry no payload.
    Unit,
    /// Home page sections and greeting.
    Home(Home),
    /// Search results.
    Search(SearchResults),
    /// A playlist, album, artist, or Liked Songs collection.
    Collection(Collection),
    /// The user's library items.
    Library(Library),
    /// The signed-in user profile.
    Me(Me),
    /// Extracted cover colors.
    Colors(CoverColor),
    /// Track lyrics.
    Lyrics(Lyrics),
}

impl CommandOutput {
    fn shape_error(command: &Command, expected: &str) -> EngineError {
        EngineError::new(
            ErrorCode::Shape,
            format!("{} answered an unexpected output (expected {expected})", command.name()),
        )
    }

    /// Unwraps a [`Home`] result.
    pub fn into_home(self, command: &Command) -> Result<Home, EngineError> {
        match self {
            CommandOutput::Home(v) => Ok(v),
            _ => Err(Self::shape_error(command, "Home")),
        }
    }

    /// Unwraps a [`SearchResults`] result.
    pub fn into_search(self, command: &Command) -> Result<SearchResults, EngineError> {
        match self {
            CommandOutput::Search(v) => Ok(v),
            _ => Err(Self::shape_error(command, "SearchResults")),
        }
    }

    /// Unwraps a [`Collection`] result.
    pub fn into_collection(self, command: &Command) -> Result<Collection, EngineError> {
        match self {
            CommandOutput::Collection(v) => Ok(v),
            _ => Err(Self::shape_error(command, "Collection")),
        }
    }

    /// Unwraps a [`Library`] result.
    pub fn into_library(self, command: &Command) -> Result<Library, EngineError> {
        match self {
            CommandOutput::Library(v) => Ok(v),
            _ => Err(Self::shape_error(command, "Library")),
        }
    }

    /// Unwraps a [`Me`] result.
    pub fn into_me(self, command: &Command) -> Result<Me, EngineError> {
        match self {
            CommandOutput::Me(v) => Ok(v),
            _ => Err(Self::shape_error(command, "Me")),
        }
    }

    /// Unwraps a [`CoverColor`] result.
    pub fn into_colors(self, command: &Command) -> Result<CoverColor, EngineError> {
        match self {
            CommandOutput::Colors(v) => Ok(v),
            _ => Err(Self::shape_error(command, "CoverColor")),
        }
    }

    /// Unwraps a [`Lyrics`] result.
    pub fn into_lyrics(self, command: &Command) -> Result<Lyrics, EngineError> {
        match self {
            CommandOutput::Lyrics(v) => Ok(v),
            _ => Err(Self::shape_error(command, "Lyrics")),
        }
    }
}

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
    SetQuality(AudioQuality),
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
            Command::SetQuality(_) => "setQuality",
            Command::SetLiked(_) => "setLiked",
            Command::SetMuted(_) => "setMuted",
            Command::PlayContext { .. } => "playContext",
            Command::PlayTrack(_) => "playTrack",
            Command::Colors(_) => "colors",
            Command::Lyrics(_) => "lyrics",
        }
    }

}
