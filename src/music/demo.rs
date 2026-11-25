//! An offline native session with sample data.

use std::thread::JoinHandle;

use tokio::sync::watch;

use super::{
    command::{Command, CommandOutput},
    model::{Collection, CoverColor, Header, Home, Item, Library, LyricLine, Lyrics, Me, PlayMode, Player, SearchResults, Track},
    session::{Answer, Session, Snapshot},
};

const HOME: &str = include_str!("fixtures/home.json");
const SEARCH: &str = include_str!("fixtures/search.json");
const PLAYLIST: &str = include_str!("fixtures/playlist.json");
const ALBUM: &str = include_str!("fixtures/album.json");
const ARTIST: &str = include_str!("fixtures/artist.json");

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).expect("a fixture is valid JSON")
}

/// Made-up lyrics, timed every four seconds, for any track.
fn demo_lyrics() -> Lyrics {
    const WORDS: [&str; 12] = [
        "♪",
        "Streetlights hum a quiet tune",
        "Paper boats along the river",
        "We were waiting for the noon",
        "♪",
        "Every window holds a summer",
        "Every summer holds a name",
        "Carry me across the harbor",
        "I will never be the same",
        "♪",
        "Streetlights hum a quiet tune",
        "And the night lets us go home",
    ];
    let lines = WORDS
        .iter()
        .enumerate()
        .map(|(i, words)| LyricLine { start_ms: i as f64 * 4000.0, words: (*words).to_string() })
        .collect();
    Lyrics { synced: true, lines, provider: "Demo".to_string() }
}

/// The sample response for `command`.
pub fn answer(command: &Command) -> Answer {
    match command {
        Command::Home => Ok(CommandOutput::Home(parse(HOME))),
        Command::Search(term) => {
            let term = term.trim().to_lowercase();
            if term.is_empty() || term == "nothing at all" {
                Ok(CommandOutput::Search(SearchResults::default()))
            } else {
                Ok(CommandOutput::Search(parse(SEARCH)))
            }
        }
        Command::Me => Ok(CommandOutput::Me(Me {
            id: "demo".to_string(),
            name: "Demo Listener".to_string(),
            username: "demo".to_string(),
            image: String::new(),
        })),
        Command::Colors(_) => Ok(CommandOutput::Colors(CoverColor::default())),
        Command::Lyrics(_) => Ok(CommandOutput::Lyrics(demo_lyrics())),
        Command::Library => {
            let home: Home = parse(HOME);
            let mut items = vec![Item {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "Liked Songs".to_string(),
                subtitle: "2 songs".to_string(),
                image: String::new(),
            }];
            for section in home.sections {
                for item in section.items {
                    if matches!(item.kind.as_str(), "playlist" | "album" | "artist") && items.len() < 14 {
                        items.push(item);
                    }
                }
            }
            Ok(CommandOutput::Library(Library { items }))
        }
        Command::Playlist(_) => Ok(CommandOutput::Collection(parse(PLAYLIST))),
        Command::Album(_) => Ok(CommandOutput::Collection(parse(ALBUM))),
        Command::Artist(_) => Ok(CommandOutput::Collection(parse(ARTIST))),
        Command::LikedSongs => {
            let mut album: Collection = parse(ALBUM);
            album.header = Header {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "Liked Songs".to_string(),
                subtitle: "2 songs".to_string(),
                image: String::new(),
                description: String::new(),
            };
            Ok(CommandOutput::Collection(album))
        }
        _ => Ok(CommandOutput::Unit),
    }
}

/// What a control does to the demo's player; true when it changed it.
pub fn control(player: &mut Player, command: &Command) -> bool {
    let before = player.clone();
    match command {
        Command::Toggle => player.playing = !player.playing,
        Command::Play => player.playing = true,
        Command::Pause => player.playing = false,
        Command::Seek(ms) => player.position_ms = ms.clamp(0.0, player.duration_ms),
        Command::SetVolume(v) => player.volume = v.clamp(0.0, 1.0),
        Command::SetPlayMode(mode) => player.play_mode = *mode,
        Command::SetQuality(quality) => player.quality = *quality,
        Command::SetLiked(on) => player.liked = *on,
        Command::SetMuted(on) => player.muted = *on,
        Command::PlayContext { .. } | Command::PlayTrack(_) => player.playing = true,
        _ => {}
    }
    if *player != before {
        player.at = now_ms();
        true
    } else {
        false
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// The demo's player: the first track of the sample album, paused.
pub fn player() -> Player {
    let album: Collection = parse(ALBUM);
    let track: Track = album.tracks.into_iter().next().unwrap_or_default();
    Player {
        title: track.name,
        artist: track.artists,
        album: track.album,
        artwork: track.image,
        track_uri: track.uri,
        context_href: "/album/paperlanterns".into(),
        playing: false,
        can_play: true,
        position_ms: 64_000.0,
        duration_ms: track.duration_ms,
        at: now_ms(),
        play_mode: PlayMode::RepeatAll,
        volume: 0.7,
        can_next: true,
        can_previous: true,
        ..Player::default()
    }
}

/// A session on the demo's data.
pub fn session() -> (Session, watch::Receiver<Snapshot>, JoinHandle<()>) {
    Session::fake(answer, control, player())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::model::{Collection, Home, Library, Me, SearchResults};

    #[test]
    fn every_answer_has_its_type() {
        let home: Home = answer(&Command::Home).unwrap().into_home(&Command::Home).unwrap();
        assert!(!home.sections.is_empty());
        let found: SearchResults = answer(&Command::Search("kites".into()))
            .unwrap()
            .into_search(&Command::Search("kites".into()))
            .unwrap();
        assert!(!found.is_empty());
        let none: SearchResults = answer(&Command::Search("nothing at all".into()))
            .unwrap()
            .into_search(&Command::Search("nothing at all".into()))
            .unwrap();
        assert!(none.is_empty());
        let me: Me = answer(&Command::Me).unwrap().into_me(&Command::Me).unwrap();
        assert_eq!(me.id, "demo");
        let library: Library = answer(&Command::Library).unwrap().into_library(&Command::Library).unwrap();
        assert!(library.items.len() > 1 && library.items[0].opens());
        for c in [
            Command::Playlist("p".into()),
            Command::Album("a".into()),
            Command::Artist("a".into()),
            Command::LikedSongs,
        ] {
            let page: Collection = answer(&c).unwrap().into_collection(&c).unwrap();
            assert!(!page.tracks.is_empty(), "{c:?}");
        }
    }

    #[test]
    fn controls_set_the_target_state() {
        let mut p = player();
        assert!(control(&mut p, &Command::SetLiked(true)));
        assert!(!control(&mut p, &Command::SetLiked(true)), "idempotent");
        assert!(control(&mut p, &Command::SetVolume(2.0)));
        assert_eq!(p.volume, 1.0);
        assert!(control(&mut p, &Command::Seek(1e12)));
        assert_eq!(p.position_ms, p.duration_ms);
        assert!(!control(&mut p, &Command::Next));
    }
}
