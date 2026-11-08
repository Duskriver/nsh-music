//! The app's state: the session with the web player, what it reported, the
//! user's library and the view's history. One store, made by the root and
//! handed down as context; its methods are the only writers.

use std::{future::Future, time::Duration};

use nsh_core::prelude::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::spotify::{Event, Item, Me, PlayerState, Session, Status, demo};

/// A page of the main view.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Home,
    Search,
    /// A playlist, album, artist or Liked Songs, by URI.
    Collection(String),
}

#[derive(Debug, Clone, PartialEq)]
struct History {
    routes: Vec<Route>,
    at: usize,
}

/// The store; `Copy`, as the signals it holds.
#[derive(Clone, Copy)]
pub struct Store {
    pub status: Signal<Status>,
    /// Whether a Spotify account is signed in; `None` until the session knows.
    pub signed_in: Signal<Option<bool>>,
    pub player: Signal<PlayerState>,
    pub library: Signal<Vec<Item>>,
    pub library_error: Signal<Option<SharedString>>,
    pub me: Signal<Option<Me>>,
    /// A short message over the player bar (an error, a hint).
    pub notice: Signal<Option<SharedString>>,
    /// Counts the moments data may load anew (the player ready, an account
    /// signed in or out): pages load again when it changes.
    pub epoch: Signal<u32>,
    /// The search field's text, kept across visits to the page.
    pub query: Signal<String>,
    /// The position the user is dragging the progress to.
    pub seeking: Signal<Option<f64>>,
    /// The volume the user is dragging to.
    pub volume_draft: Signal<Option<f64>>,
    /// Whether the full-screen Now Playing view is up.
    pub now_playing: Signal<bool>,
    /// Whether the About panel is up.
    pub about: Signal<bool>,
    /// Whether the library drawer is open (narrow layouts only).
    pub drawer: Signal<bool>,
    history: Signal<History>,
    session: Stored<Option<Session>>,
    notice_task: Stored<Option<Spawned<()>>>,
}

pub fn store() -> Store {
    use_context::<Store>().expect("the root provides the store")
}

impl Store {
    /// The store, on the real web player, or the demo's data when
    /// `NSHTIFY_DEMO` is set.
    pub fn new() -> Store {
        let demo = std::env::var_os("NSHTIFY_DEMO").is_some_and(|v| v != "0");
        let (session, events) = if demo { demo::session() } else { Session::start() };
        Store::with_session(session, events)
    }

    /// The store on `session`, hearing its `events`.
    pub fn with_session(session: Session, mut events: tokio::sync::mpsc::UnboundedReceiver<Event>) -> Store {
        let store = Store {
            status: signal(Status::Starting),
            signed_in: signal(None),
            player: signal(PlayerState::default()),
            library: signal(Vec::new()),
            library_error: signal(None),
            me: signal(None),
            notice: signal(None),
            epoch: signal(0),
            query: signal(String::new()),
            seeking: signal(None),
            volume_draft: signal(None),
            now_playing: signal(false),
            about: signal(false),
            drawer: signal(false),
            history: signal(History { routes: vec![Route::Home], at: 0 }),
            session: stored(Some(session)),
            notice_task: stored(None),
        };
        // A screenshot aid: open a view at start (NSHTIFY_SHOT=nowplaying|about).
        match std::env::var("NSHTIFY_SHOT").as_deref() {
            Ok("nowplaying") => store.now_playing.set(true),
            Ok("about") => store.about.set(true),
            _ => {}
        }
        spawn(async move {
            while let Some(event) = events.recv().await {
                store.apply(event);
            }
        });
        // The library and the account, whenever data may load anew.
        effect(move || {
            let _ = store.epoch.get();
            let signed_in = store.signed_in.get() == Some(true);
            let ready = store.status.with(|s| *s == Status::Ready);
            untracked(|| store.load_library(signed_in && ready));
        });
        store
    }

    fn apply(&self, event: Event) {
        match event {
            Event::Status(status) => {
                let ready = status == Status::Ready;
                self.status.set(status);
                if ready {
                    self.epoch.update(|e| *e += 1);
                }
            }
            Event::Player(state) => {
                self.player.set(state);
            }
            Event::SignedIn(now) => {
                if self.signed_in.get_untracked() != Some(now) {
                    self.signed_in.set(Some(now));
                    self.epoch.update(|e| *e += 1);
                }
            }
            Event::Auth => {
                // The first signed request, or a refreshed token: retry what
                // failed for want of one.
                if self.library_error.get_untracked().is_some() {
                    self.epoch.update(|e| *e += 1);
                }
            }
        }
    }

    fn load_library(&self, signed_in: bool) {
        let store = *self;
        if !signed_in {
            store.library.set(Vec::new());
            store.me.set(None);
            return;
        }
        spawn(async move {
            match store.load::<crate::spotify::Library>("library", json!([])).await {
                Ok(library) => {
                    store.library_error.set(None);
                    store.library.set(library.items);
                }
                Err(error) => store.library_error.set(Some(error.into())),
            }
            if let Ok(me) = store.load::<Me>("me", json!([])).await {
                store.me.set(Some(me));
            }
        });
    }

    /// Calls the bridge's `name` and decodes its answer.
    pub fn load<T: DeserializeOwned>(&self, name: &str, args: Value) -> impl Future<Output = Result<T, String>> + use<T> {
        let answer = self.session.with(|s| s.as_ref().map(|s| s.call(name, args)));
        async move {
            let value = answer.ok_or("no session")?.await.map_err(|_| "the session ended".to_owned())??;
            serde_json::from_value(value).map_err(|e| e.to_string())
        }
    }

    /// Presses one of the player's controls; a failure shows as a notice.
    pub fn control(&self, name: &'static str, args: Value) {
        if !self.can_play() {
            return;
        }
        let store = *self;
        let answer = store.load::<Value>(name, args);
        spawn(async move {
            if let Err(error) = answer.await {
                log::warn!("{name}: {error}");
                store.notify(format!("Couldn’t {}: {error}", describe(name)));
            }
        });
    }

    /// Plays or pauses, showing the change before the player confirms it.
    pub fn toggle(&self) {
        if !self.can_play() {
            return;
        }
        self.player.update(|p| p.playing = !p.playing);
        self.control("toggle", json!([]));
    }

    /// Plays an item: a track on its own, a collection from its start.
    pub fn play_item(&self, item: &Item) {
        if item.kind == "track" {
            self.play("playTrack", json!([item.uri]));
        } else {
            self.play("playContext", json!([item.uri, null]));
        }
    }

    /// Plays the track at `index` of the collection `uri`.
    pub fn play_at(&self, uri: &str, index: usize) {
        self.play("playContext", json!([uri, index]));
    }

    /// Plays one track (and its radio after it).
    pub fn play_track(&self, uri: &str) {
        self.play("playTrack", json!([uri]));
    }

    fn play(&self, name: &'static str, args: Value) {
        if !self.can_play() {
            return;
        }
        self.notify("Starting playback…");
        let store = *self;
        let answer = store.load::<Value>(name, args);
        spawn(async move {
            match answer.await {
                Ok(_) => store.notice.set(None),
                Err(error) => store.notify(format!("Couldn’t play that: {error}")),
            }
        });
    }

    /// Whether the player can play; when it cannot, says why.
    fn can_play(&self) -> bool {
        if self.status.get_untracked() != Status::Ready {
            self.notify("The player is still starting.");
            return false;
        }
        if self.signed_in.get_untracked() != Some(true) {
            self.notify("Log in to Spotify to play music.");
            return false;
        }
        true
    }

    /// Shows `message` over the player bar for a few seconds.
    pub fn notify(&self, message: impl Into<SharedString>) {
        let store = *self;
        store.notice.set(Some(message.into()));
        if let Some(task) = store.notice_task.replace(None) {
            task.cancel();
        }
        let task = spawn(async move {
            timer(Duration::from_secs(5)).await;
            store.notice.set(None);
        });
        store.notice_task.replace(Some(task));
    }

    pub fn sign_in(&self) {
        self.session.with(|s| s.as_ref().map(Session::sign_in));
    }

    pub fn cancel_sign_in(&self) {
        self.session.with(|s| s.as_ref().map(Session::cancel_sign_in));
    }

    /// The user finished logging in: close the window and resume the player.
    pub fn finish_sign_in(&self) {
        self.session.with(|s| s.as_ref().map(Session::finish_sign_in));
    }

    pub fn sign_out(&self) {
        self.session.with(|s| s.as_ref().map(Session::sign_out));
    }

    // --- The view's history ---------------------------------------------

    /// The page shown (tracked where it is read).
    pub fn route(&self) -> Route {
        self.history.with(|h| h.routes[h.at].clone())
    }

    /// Shows `route`, after the page shown; the pages ahead are dropped.
    pub fn open(&self, route: Route) {
        self.history.update(|h| {
            if h.routes[h.at] == route {
                return;
            }
            h.routes.truncate(h.at + 1);
            h.routes.push(route);
            h.at += 1;
        });
    }

    pub fn can_go_back(&self) -> bool {
        self.history.with(|h| h.at > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        self.history.with(|h| h.at + 1 < h.routes.len())
    }

    pub fn back(&self) {
        self.history.update(|h| h.at = h.at.saturating_sub(1));
    }

    pub fn forward(&self) {
        self.history.update(|h| h.at = (h.at + 1).min(h.routes.len() - 1));
    }

    /// Opens a page, and on a narrow layout closes the library drawer.
    pub fn open_and_close_drawer(&self, route: Route) {
        self.open(route);
        self.drawer.set(false);
    }

    // --- Premium and the web ---------------------------------------------

    /// Opens Spotify's Premium page in the system browser. Nshtify never
    /// removes Spotify's ads; Premium is how a listener does.
    pub fn open_premium(&self) {
        let _ = open_url("https://www.spotify.com/premium/");
    }

    /// Opens an external page in the system browser.
    pub fn open_external(&self, url: &str) {
        let _ = open_url(url.to_owned());
    }
}

fn describe(control: &str) -> &str {
    match control {
        "toggle" | "play" | "pause" => "play or pause",
        "next" => "skip",
        "previous" => "go back",
        "shuffle" => "change shuffle",
        "repeat" => "change repeat",
        "like" => "save the track",
        "seek" => "seek",
        "volume" | "mute" => "change the volume",
        other => other,
    }
}
