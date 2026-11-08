//! The `Spotify` class exported to TSX: it owns the engine (the session with
//! the web player) and gives TSX a view of the player's state through
//! callbacks, data through async methods, and control through methods.
//!
//! One owner per piece of state (mixed.md): the player's state lives here;
//! TSX mirrors it in signals, updated by the callbacks, and changes it only
//! through these methods.

use std::{cell::RefCell, rc::Rc};

use nsh_core::{element::CallbackArgs, prelude::*};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::spotify::{Event, PlayerState, Session, Status, demo};

/// The player's state, as TSX sees it (one object per change).
#[derive(Debug, Clone, Default, JsObject)]
pub struct Player {
    /// Whether a Spotify account is signed in.
    pub signed_in: bool,
    /// The track's title, empty when nothing is loaded.
    pub title: String,
    /// The track's artists.
    pub artist: String,
    /// The album's name.
    pub album: String,
    /// The album art URL.
    pub artwork: String,
    /// The track's URI (`spotify:track:…`).
    pub track_uri: String,
    /// The path of the context playing (`/playlist/…`), for linking to it.
    pub context_href: String,
    /// Whether it is playing.
    pub playing: bool,
    /// Whether the player can play at all.
    pub can_play: bool,
    /// The playback position, in milliseconds.
    pub position_ms: f64,
    /// The track's length, in milliseconds.
    pub duration_ms: f64,
    /// Whether shuffle is on.
    pub shuffle: bool,
    /// Whether shuffle can be toggled.
    pub shuffle_available: bool,
    /// How it repeats: `"off"`, `"context"` or `"track"`.
    pub repeat: String,
    /// The volume, 0 to 1.
    pub volume: f64,
    /// Whether it is muted.
    pub muted: bool,
    /// Whether the track is saved to Liked Songs.
    pub liked: bool,
    /// Whether there is a next track.
    pub can_next: bool,
    /// Whether there is a previous track.
    pub can_previous: bool,
    /// Whether an ad is playing.
    pub ad: bool,
    /// Whether the account is Premium (free accounts hear ads).
    pub premium: bool,
}

impl From<PlayerState> for Player {
    fn from(p: PlayerState) -> Self {
        Player {
            signed_in: p.signed_in,
            title: p.title,
            artist: p.artist,
            album: p.album,
            artwork: p.artwork,
            track_uri: p.track_uri,
            context_href: p.context_href,
            playing: p.playing,
            can_play: p.can_play,
            position_ms: p.position_ms as f64,
            duration_ms: p.duration_ms as f64,
            shuffle: p.shuffle,
            shuffle_available: p.shuffle_available,
            repeat: match p.repeat {
                crate::spotify::Repeat::Off => "off",
                crate::spotify::Repeat::Context => "context",
                crate::spotify::Repeat::Track => "track",
            }
            .to_owned(),
            volume: p.volume as f64,
            muted: p.muted,
            liked: p.liked,
            can_next: p.can_next,
            can_previous: p.can_previous,
            ad: p.ad,
            premium: p.premium,
        }
    }
}

/// Where the session is, as TSX sees it.
#[derive(Debug, Clone, Default, JsObject)]
pub struct Health {
    /// `"starting"`, `"ready"`, `"signingIn"` or `"failed"`.
    pub status: String,
    /// The reason, when `status` is `"failed"`.
    pub error: Option<String>,
}

fn health_of(status: &Status) -> Health {
    match status {
        Status::Starting => Health { status: "starting".into(), error: None },
        Status::Ready => Health { status: "ready".into(), error: None },
        Status::SigningIn => Health { status: "signingIn".into(), error: None },
        Status::Failed(error) => Health { status: "failed".into(), error: Some(error.clone()) },
    }
}

#[derive(Default)]
struct Listeners<T> {
    next: u64,
    items: Vec<(u64, Callback<T>)>,
}

impl<T: CallbackArgs + Clone + 'static> Listeners<T> {
    fn add(&mut self, cb: Callback<T>) -> u64 {
        let id = self.next;
        self.next += 1;
        self.items.push((id, cb));
        id
    }
    fn remove(&mut self, id: u64) {
        self.items.retain(|(i, _)| *i != id);
    }
    fn post(&self, value: T) {
        for (_, cb) in &self.items {
            cb.post(value.clone());
        }
    }
}

#[derive(Default)]
struct Inner {
    player: Player,
    health: Health,
    signed_in: Option<bool>,
    on_player: Listeners<Player>,
    on_health: Listeners<Health>,
    on_signed_in: Listeners<bool>,
    on_auth: Listeners<()>,
    rx: Option<UnboundedReceiver<Event>>,
}

/// The engine, exported to TSX as `new Spotify()`.
#[class]
pub struct Spotify {
    session: Session,
    inner: Rc<RefCell<Inner>>,
}

#[class]
impl Spotify {
    /// Starts the session with the real web player, or the recorded demo
    /// data when `SPOTISH_DEMO` is set.
    pub fn new() -> Self {
        let demo = std::env::var_os("SPOTISH_DEMO").is_some_and(|v| v != "0");
        let (session, events) = if demo { demo::session() } else { Session::start() };
        let inner = Rc::new(RefCell::new(Inner { rx: Some(events), ..Inner::default() }));
        inner.borrow_mut().health = Health { status: "starting".into(), error: None };
        Spotify { session, inner }
    }

    /// Begins delivering the player's state to the listeners. Call it once,
    /// after subscribing; it resolves when the session ends.
    pub async fn start(this: Instance<Self>) {
        let inner = this.with(|s| s.inner.clone());
        let Some(mut rx) = inner.borrow_mut().rx.take() else { return };
        while let Some(event) = rx.recv().await {
            let mut w = inner.borrow_mut();
            match event {
                Event::Status(status) => {
                    let health = health_of(&status);
                    w.health = health.clone();
                    w.on_health.post(health);
                }
                Event::Player(state) => {
                    let player = Player::from(state);
                    w.player = player.clone();
                    w.on_player.post(player);
                }
                Event::SignedIn(now) => {
                    w.signed_in = Some(now);
                    w.on_signed_in.post(now);
                }
                Event::Auth => w.on_auth.post(()),
            }
        }
    }

    /// The player's current state.
    #[getter]
    pub fn player(&self) -> Player {
        self.inner.borrow().player.clone()
    }

    /// Where the session is.
    #[getter]
    pub fn health(&self) -> Health {
        self.inner.borrow().health.clone()
    }

    /// Whether an account is signed in, or null until it is known.
    #[getter]
    pub fn signed_in(&self) -> Option<bool> {
        self.inner.borrow().signed_in
    }

    /// Calls `listener` with the player's state after every change.
    pub fn on_player(&self, #[args(player)] listener: Callback<Player>) -> Stop {
        let inner = self.inner.clone();
        let id = inner.borrow_mut().on_player.add(listener);
        Stop::new(move || inner.borrow_mut().on_player.remove(id))
    }

    /// Calls `listener` when the session's status changes.
    pub fn on_health(&self, #[args(health)] listener: Callback<Health>) -> Stop {
        let inner = self.inner.clone();
        let id = inner.borrow_mut().on_health.add(listener);
        Stop::new(move || inner.borrow_mut().on_health.remove(id))
    }

    /// Calls `listener` when the signed-in account changes.
    pub fn on_signed_in(&self, #[args(signed_in)] listener: Callback<bool>) -> Stop {
        let inner = self.inner.clone();
        let id = inner.borrow_mut().on_signed_in.add(listener);
        Stop::new(move || inner.borrow_mut().on_signed_in.remove(id))
    }

    /// Calls `listener` when the player signs a new token (data that failed
    /// before may now load).
    pub fn on_auth(&self, listener: Callback<()>) -> Stop {
        let inner = self.inner.clone();
        let id = inner.borrow_mut().on_auth.add(listener);
        Stop::new(move || inner.borrow_mut().on_auth.remove(id))
    }

    // --- Data (the bridge's JSON, straight through) ----------------------

    /// The home page.
    pub async fn home(this: Instance<Self>) -> Result<Value, String> {
        query(this, "home", json!([])).await
    }

    /// What a search for `term` finds.
    pub async fn search(this: Instance<Self>, term: String) -> Result<Value, String> {
        query(this, "search", json!([term])).await
    }

    /// A playlist by URI.
    pub async fn playlist(this: Instance<Self>, uri: String) -> Result<Value, String> {
        query(this, "playlist", json!([uri])).await
    }

    /// An album by URI.
    pub async fn album(this: Instance<Self>, uri: String) -> Result<Value, String> {
        query(this, "album", json!([uri])).await
    }

    /// An artist by URI.
    pub async fn artist(this: Instance<Self>, uri: String) -> Result<Value, String> {
        query(this, "artist", json!([uri])).await
    }

    /// The signed-in user's Liked Songs.
    pub async fn liked_songs(this: Instance<Self>) -> Result<Value, String> {
        query(this, "likedSongs", json!([])).await
    }

    /// The signed-in user's library.
    pub async fn library(this: Instance<Self>) -> Result<Value, String> {
        query(this, "library", json!([])).await
    }

    /// The signed-in user.
    pub async fn me(this: Instance<Self>) -> Result<Value, String> {
        query(this, "me", json!([])).await
    }

    // --- Controls --------------------------------------------------------

    /// Plays or pauses.
    pub async fn toggle(this: Instance<Self>) -> Result<(), String> {
        control(this, "toggle", json!([])).await
    }
    /// Skips to the next track.
    pub async fn next(this: Instance<Self>) -> Result<(), String> {
        control(this, "next", json!([])).await
    }
    /// Goes to the previous track.
    pub async fn previous(this: Instance<Self>) -> Result<(), String> {
        control(this, "previous", json!([])).await
    }
    /// Toggles shuffle.
    pub async fn shuffle(this: Instance<Self>) -> Result<(), String> {
        control(this, "shuffle", json!([])).await
    }
    /// Cycles the repeat mode.
    pub async fn repeat(this: Instance<Self>) -> Result<(), String> {
        control(this, "repeat", json!([])).await
    }
    /// Toggles whether the track is saved to Liked Songs.
    pub async fn like(this: Instance<Self>) -> Result<(), String> {
        control(this, "like", json!([])).await
    }
    /// Toggles mute.
    pub async fn mute(this: Instance<Self>) -> Result<(), String> {
        control(this, "mute", json!([])).await
    }
    /// Seeks to `ms` milliseconds.
    pub async fn seek(this: Instance<Self>, ms: f64) -> Result<(), String> {
        control(this, "seek", json!([ms])).await
    }
    /// Sets the volume, 0 to 1.
    pub async fn set_volume(this: Instance<Self>, volume: f64) -> Result<(), String> {
        control(this, "volume", json!([volume])).await
    }
    /// Plays a playlist, album, artist or Liked Songs, optionally from the
    /// track at `index`.
    pub async fn play_context(this: Instance<Self>, uri: String, index: Option<f64>) -> Result<(), String> {
        control(this, "playContext", json!([uri, index])).await
    }
    /// Plays one track (its radio follows it).
    pub async fn play_track(this: Instance<Self>, uri: String) -> Result<(), String> {
        control(this, "playTrack", json!([uri])).await
    }

    // --- The account -----------------------------------------------------

    /// Opens the sign-in window.
    pub fn sign_in(&self) {
        self.session.sign_in();
    }
    /// Closes the sign-in window without signing in.
    pub fn cancel_sign_in(&self) {
        self.session.cancel_sign_in();
    }
    /// The user finished logging in: resume the player.
    pub fn finish_sign_in(&self) {
        self.session.finish_sign_in();
    }
    /// Signs out of Spotify.
    pub fn sign_out(&self) {
        self.session.sign_out();
    }

    // --- Links -----------------------------------------------------------

    /// Opens Spotify Premium in the system browser. Spotish never blocks ads;
    /// Premium is how a listener removes them.
    pub async fn open_premium(this: Instance<Self>) {
        let _ = &this;
        let _ = open_url("https://www.spotify.com/premium/").await;
    }
    /// Opens an external page in the system browser.
    pub async fn open_external(this: Instance<Self>, url: String) {
        let _ = &this;
        let _ = open_url(url).await;
    }
}

/// Runs the bridge's `name` with `args` and returns its JSON.
async fn query(this: Instance<Spotify>, name: &str, args: Value) -> Result<Value, String> {
    let session = this.with(|s| s.session.clone());
    session.call(name, args).await.map_err(|_| "the session ended".to_owned())?
}

/// A control: the answer's value is ignored.
async fn control(this: Instance<Spotify>, name: &str, args: Value) -> Result<(), String> {
    query(this, name, args).await.map(|_| ())
}
