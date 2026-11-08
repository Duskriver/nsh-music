//! A session with no browser, answering from data recorded from the web
//! player: what the tests run on, and `SPOTISH_DEMO=1` shows (the look of
//! the app without an account).

use serde_json::{Value, json};

use super::{PlayerState, Repeat, Session, model::Track};

const HOME: &str = include_str!("fixtures/home.json");
const SEARCH: &str = include_str!("fixtures/search.json");
const PLAYLIST: &str = include_str!("fixtures/playlist.json");
const ALBUM: &str = include_str!("fixtures/album.json");
const ARTIST: &str = include_str!("fixtures/artist.json");

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("a fixture is JSON")
}

/// What the demo's bridge answers to `name`.
pub fn answer(name: &str, args: &Value) -> Result<Value, String> {
    let uri = args[0].as_str().unwrap_or_default();
    match name {
        "home" => Ok(parse(HOME)),
        "search" => {
            let term = uri.trim().to_lowercase();
            if term.is_empty() || term == "nothing at all" {
                Ok(json!({ "tracks": [], "artists": [], "albums": [], "playlists": [] }))
            } else {
                Ok(parse(SEARCH))
            }
        }
        "me" => Ok(json!({ "name": "Demo Listener", "username": "demo", "image": "" })),
        "library" => {
            let home = parse(HOME);
            let mut items = vec![json!({
                "uri": "spotify:collection:tracks", "kind": "collection", "name": "Liked Songs",
                "subtitle": "12 songs", "image": ""
            })];
            for section in home["sections"].as_array().into_iter().flatten() {
                for item in section["items"].as_array().into_iter().flatten() {
                    if matches!(item["kind"].as_str(), Some("playlist" | "album" | "artist")) && items.len() < 14 {
                        items.push(item.clone());
                    }
                }
            }
            Ok(json!({ "items": items }))
        }
        "playlist" => Ok(parse(PLAYLIST)),
        "album" => Ok(parse(ALBUM)),
        "artist" => Ok(parse(ARTIST)),
        "likedSongs" => {
            let mut album = parse(ALBUM);
            album["header"] = json!({
                "uri": "spotify:collection:tracks", "kind": "collection", "name": "Liked Songs",
                "subtitle": "12 songs", "image": "", "description": ""
            });
            Ok(album)
        }
        "toggle" | "play" | "pause" | "next" | "previous" | "shuffle" | "repeat" | "like" | "mute" | "seek"
        | "volume" | "playContext" | "playTrack" => Ok(Value::Bool(true)),
        _ => Err(format!("the demo has no {name}")),
    }
}

/// The demo's player: the first track of the recorded album, paused.
pub fn player() -> PlayerState {
    let album: Value = parse(ALBUM);
    let track: Track = serde_json::from_value(album["tracks"][0].clone()).unwrap_or_default();
    PlayerState {
        signed_in: true,
        title: track.name,
        artist: track.artists,
        album: track.album,
        artwork: track.image,
        track_uri: track.uri,
        playing: false,
        can_play: true,
        position_ms: 64_000,
        duration_ms: track.duration_ms,
        shuffle_available: true,
        repeat: Repeat::Off,
        volume: 0.7,
        can_next: true,
        can_previous: true,
        premium: false,
        ..PlayerState::default()
    }
}

/// A session on the demo's data.
pub fn session() -> (Session, tokio::sync::mpsc::UnboundedReceiver<super::Event>) {
    Session::fake(answer, player())
}
