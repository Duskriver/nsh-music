//! What the bridge answers, as the app shows it: items (playlists, albums,
//! artists, shows), tracks, pages of them and the player's state. Plain data,
//! decoded from the bridge's JSON (`bridge/bridge.js` makes these shapes).

use serde::Deserialize;

/// A playlist, album, artist, show, Liked Songs or a track shown as a card.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub uri: String,
    /// `playlist`, `album`, `artist`, `show`, `collection` or `track`.
    pub kind: String,
    pub name: String,
    pub subtitle: String,
    pub image: String,
}

impl Item {
    /// Whether the app has a page for it (a track has none: it plays).
    pub fn opens(&self) -> bool {
        matches!(self.kind.as_str(), "playlist" | "album" | "artist" | "collection")
    }
}

/// A track of a list.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
    pub uri: String,
    pub uid: String,
    pub name: String,
    pub artists: String,
    pub album: String,
    pub album_uri: String,
    pub image: String,
    pub duration_ms: u64,
    pub explicit: bool,
    pub playable: bool,
}

/// The header of a collection's page.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Header {
    pub uri: String,
    pub kind: String,
    pub name: String,
    pub subtitle: String,
    pub image: String,
    pub description: String,
}

/// A playlist, album, artist or Liked Songs: its header, tracks and, for an
/// artist, releases.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Collection {
    pub header: Header,
    pub tracks: Vec<Track>,
    pub releases: Vec<Item>,
    pub total: u32,
}

/// A titled row of cards on the home page.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

/// The home page.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Home {
    pub greeting: String,
    pub sections: Vec<Section>,
}

/// What a search found.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Item>,
    pub albums: Vec<Item>,
    pub playlists: Vec<Item>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.artists.is_empty() && self.albums.is_empty() && self.playlists.is_empty()
    }
}

/// The signed-in user's library.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Library {
    pub items: Vec<Item>,
}

/// The signed-in user.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Me {
    pub name: String,
    pub username: String,
    pub image: String,
}

/// How the player repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    Context,
    Track,
}

/// The web player's state, as its own controls show it.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlayerState {
    pub signed_in: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork: String,
    pub track_uri: String,
    pub context_href: String,
    pub playing: bool,
    pub can_play: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: bool,
    pub shuffle_available: bool,
    pub repeat: Repeat,
    pub volume: f32,
    pub muted: bool,
    pub liked: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub ad: bool,
    /// Whether the signed-in account is Premium (free accounts hear ads).
    pub premium: bool,
}

impl PlayerState {
    /// Whether a track (or an ad) is loaded in the player.
    pub fn has_track(&self) -> bool {
        !self.title.is_empty()
    }
}

/// `m:ss` (or `h:mm:ss`) of a duration.
pub fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    let (h, m, s) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(61_000), "1:01");
        assert_eq!(clock(3_725_000), "1:02:05");
    }

    #[test]
    fn decodes_the_bridges_player_state() {
        let state: PlayerState = serde_json::from_str(
            r#"{"signedIn":true,"title":"Airbag","artist":"Radiohead","playing":true,"positionMs":1200,"durationMs":287880,"repeat":"track","volume":0.5}"#,
        )
        .unwrap();
        assert!(state.playing && state.signed_in);
        assert_eq!(state.repeat, Repeat::Track);
        assert_eq!(state.duration_ms, 287_880);
    }
}
