//! The session with the web player: a thread that keeps a browser running
//! the player, installs the bridge in it, and answers the app's calls.
//!
//! The app talks to it through [`Session`] (a `Send` handle) and hears from
//! it through the receiver [`Session::start`] returns: the status, the
//! player's state, and when the player's requests are signed.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::sync::{mpsc as async_mpsc, oneshot};

use super::{
    cdp::{Cdp, CdpEvent},
    chrome::{self, Browser, Mode},
    model::PlayerState,
};

const BRIDGE: &str = include_str!("../bridge/bridge.js");

/// Where the session is.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Status {
    /// The browser is starting, or the player loading in it.
    #[default]
    Starting,
    /// The player runs; the app can ask for data and control it.
    Ready,
    /// The sign-in window is open.
    SigningIn,
    /// Something keeps the player from running (no browser, say).
    Failed(String),
}

/// What the session tells the app.
#[derive(Debug, Clone)]
pub enum Event {
    Status(Status),
    /// The player's state changed.
    Player(PlayerState),
    /// Whether a Spotify account is signed in (checked at start and after
    /// signing in or out).
    SignedIn(bool),
    /// The player signed its requests with a new token: data the app asked
    /// for before may now load.
    Auth,
}

type Answer = Result<Value, String>;

enum Request {
    Call { name: String, args: Value, reply: oneshot::Sender<Answer> },
    SignIn,
    CancelSignIn,
    FinishSignIn,
    SignOut,
}

/// A handle to the session; cheap to clone, usable from any thread.
#[derive(Clone)]
pub struct Session {
    requests: mpsc::Sender<Request>,
}

impl Session {
    /// Starts the session with the real web player.
    pub fn start() -> (Session, async_mpsc::UnboundedReceiver<Event>) {
        let (requests, queue) = mpsc::channel();
        let (events, heard) = async_mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("spotish-session".into())
            .spawn(move || run(queue, events))
            .expect("the session thread starts");
        (Session { requests }, heard)
    }

    /// A session that answers from `answer` with no browser (tests, and
    /// `SPOTISH_DEMO=1`): ready at once, signed in.
    pub fn fake(
        answer: impl Fn(&str, &Value) -> Answer + Send + 'static,
        player: PlayerState,
    ) -> (Session, async_mpsc::UnboundedReceiver<Event>) {
        let (requests, queue) = mpsc::channel::<Request>();
        let (events, heard) = async_mpsc::unbounded_channel();
        let _ = events.send(Event::Status(Status::Ready));
        let _ = events.send(Event::SignedIn(true));
        let _ = events.send(Event::Player(player.clone()));
        std::thread::Builder::new()
            .name("spotish-fake-session".into())
            .spawn(move || {
                let mut player = player;
                while let Ok(request) = queue.recv() {
                    if let Request::Call { name, args, reply } = request {
                        let result = answer(&name, &args);
                        if fake_control(&mut player, &name, &args) {
                            let _ = events.send(Event::Player(player.clone()));
                        }
                        let _ = reply.send(result);
                    }
                }
            })
            .expect("the session thread starts");
        (Session { requests }, heard)
    }

    /// Calls the bridge's `name` (a query or a control) with `args`.
    pub fn call(&self, name: &str, args: Value) -> oneshot::Receiver<Answer> {
        let (reply, answer) = oneshot::channel();
        let request = Request::Call { name: name.to_owned(), args, reply };
        if let Err(mpsc::SendError(Request::Call { reply, .. })) = self.requests.send(request) {
            let _ = reply.send(Err("the session ended".into()));
        }
        answer
    }

    /// Opens the sign-in window (the player stops until it closes).
    pub fn sign_in(&self) {
        let _ = self.requests.send(Request::SignIn);
    }

    /// Closes the sign-in window without signing in.
    pub fn cancel_sign_in(&self) {
        let _ = self.requests.send(Request::CancelSignIn);
    }

    /// The user says they have finished logging in: close the window and go
    /// back to the player, which picks up the new session.
    pub fn finish_sign_in(&self) {
        let _ = self.requests.send(Request::FinishSignIn);
    }

    /// Signs the player out of Spotify.
    pub fn sign_out(&self) {
        let _ = self.requests.send(Request::SignOut);
    }
}

/// What a fake session's player does on a control.
fn fake_control(player: &mut PlayerState, name: &str, args: &Value) -> bool {
    match name {
        "toggle" => player.playing = !player.playing,
        "play" => player.playing = true,
        "pause" => player.playing = false,
        "shuffle" => player.shuffle = !player.shuffle,
        "like" => player.liked = !player.liked,
        "seek" => player.position_ms = args[0].as_u64().unwrap_or(0),
        "volume" => player.volume = args[0].as_f64().unwrap_or(1.0) as f32,
        "repeat" => {
            use super::model::Repeat;
            player.repeat = match player.repeat {
                Repeat::Off => Repeat::Context,
                Repeat::Context => Repeat::Track,
                Repeat::Track => Repeat::Off,
            }
        }
        _ => return false,
    }
    true
}

fn run(queue: mpsc::Receiver<Request>, events: async_mpsc::UnboundedSender<Event>) {
    let Some(binary) = chrome::find_browser() else {
        let _ = events.send(Event::Status(Status::Failed(
            "Spotish plays through Chromium or Google Chrome, and neither is installed. Install one, or set SPOTISH_BROWSER to its path.".into(),
        )));
        // Answer calls until the app goes, so nothing waits forever.
        while let Ok(request) = queue.recv() {
            if let Request::Call { reply, .. } = request {
                let _ = reply.send(Err("no browser".into()));
            }
        }
        return;
    };
    let profile = chrome::profile_dir();
    let mut mode = Mode::Headless;
    loop {
        let next = match mode {
            Mode::Headless => run_player(&binary, &profile, &queue, &events),
            Mode::SignIn => run_sign_in(&binary, &profile, &queue, &events),
        };
        match next {
            Some(mode_next) => mode = mode_next,
            None => return, // the app went away
        }
    }
}

/// Runs the headless player and answers the app's calls, until it must restart
/// or switch to signing in. Returns the next mode, or `None` when the app has
/// gone. The browser and its connection are dropped as this returns.
fn run_player(
    binary: &std::path::Path,
    profile: &std::path::Path,
    queue: &mpsc::Receiver<Request>,
    events: &async_mpsc::UnboundedSender<Event>,
) -> Option<Mode> {
    let send = |event: Event| {
        let _ = events.send(event);
    };
    // A reused profile (SPOTISH_PROFILE) that another browser still holds can't
    // be opened: say so plainly rather than failing to start.
    if std::env::var_os("SPOTISH_PROFILE").is_some() && chrome::profile_locked(profile) {
        send(Event::Status(Status::Failed(
            "The browser profile Spotish reuses (SPOTISH_PROFILE) is open in another browser. Close that browser, then reopen Spotish.".into(),
        )));
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            match queue.recv_timeout(Duration::from_millis(200)) {
                Ok(Request::Call { reply, .. }) => {
                    let _ = reply.send(Err("the profile is in use by another browser".into()));
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
        return Some(Mode::Headless);
    }
    log::info!("player: starting (headless)");
    chrome::adopt_widevine(profile);
    let widevine_at_start = chrome::has_widevine(profile);
    send(Event::Status(Status::Starting));
    let (cdp_events, heard) = mpsc::channel();
    let started = Browser::start(binary, profile, Mode::Headless).and_then(|browser| {
        let cdp = Arc::new(Cdp::connect(&browser.page_url()?, cdp_events)?);
        prepare_player(&cdp, &browser)?;
        Ok((browser, cdp))
    });
    let (mut browser, cdp) = match started {
        Ok(started) => started,
        Err(error) => {
            log::error!("the player did not start: {error}");
            send(Event::Status(Status::Failed(format!("The player did not start: {error}"))));
            // Wait a little before trying again, answering calls meanwhile.
            let until = Instant::now() + Duration::from_secs(3);
            while Instant::now() < until {
                match queue.recv_timeout(Duration::from_millis(200)) {
                    Ok(Request::Call { reply, .. }) => {
                        let _ = reply.send(Err("the player is not running".into()));
                    }
                    Ok(Request::SignIn) => return Some(Mode::SignIn),
                    Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                }
            }
            return Some(Mode::Headless);
        }
    };

    let mut ready = false;
    let mut signed_in = None::<bool>;
    let mut checked = Instant::now() - Duration::from_secs(60);
    let mut playing = false;
    loop {
        match queue.recv_timeout(Duration::from_millis(100)) {
            Ok(Request::Call { name, args, reply }) => {
                let cdp = cdp.clone();
                std::thread::spawn(move || {
                    let _ = reply.send(call(&cdp, &name, &args));
                });
            }
            Ok(Request::SignIn) => {
                // Flush the player's cookies before handing off to sign-in.
                let _ = cdp.call("Browser.close", json!({}), Duration::from_secs(8));
                browser.wait_for_exit(Duration::from_secs(8));
                return Some(Mode::SignIn);
            }
            Ok(Request::SignOut) => {
                let _ = cdp.call("Network.clearBrowserCookies", json!({}), Duration::from_secs(5));
                let _ = cdp.call(
                    "Storage.clearDataForOrigin",
                    json!({ "origin": "https://open.spotify.com", "storageTypes": "all" }),
                    Duration::from_secs(5),
                );
                // Close cleanly so the cleared cookies are written to disk.
                let _ = cdp.call("Browser.close", json!({}), Duration::from_secs(8));
                browser.wait_for_exit(Duration::from_secs(8));
                send(Event::SignedIn(false));
                return Some(Mode::Headless);
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        }

        while let Ok(CdpEvent { method, params }) = heard.try_recv() {
            match method.as_str() {
                "Runtime.bindingCalled" if params["name"] == "spotishEvent" => {
                    let payload = params["payload"].as_str().unwrap_or("{}");
                    let message: Value = serde_json::from_str(payload).unwrap_or_default();
                    match message["type"].as_str() {
                        Some("player") => {
                            if let Ok(state) = serde_json::from_value::<PlayerState>(message["state"].clone()) {
                                playing = state.playing;
                                send(Event::Player(state));
                            }
                            if !ready {
                                ready = true;
                                send(Event::Status(Status::Ready));
                            }
                        }
                        Some("auth") => send(Event::Auth),
                        _ => {}
                    }
                }
                "Inspector.targetCrashed" | "spotish.closed" => {
                    log::warn!("the player's page went away ({method}); starting it again");
                    return Some(Mode::Headless);
                }
                _ => {}
            }
        }

        if !browser.running() {
            log::warn!("the player browser exited; starting it again");
            return Some(Mode::Headless);
        }

        if checked.elapsed() >= Duration::from_secs(8) {
            checked = Instant::now();
            let now = has_session_cookie(&cdp);
            if signed_in != Some(now) {
                signed_in = Some(now);
                send(Event::SignedIn(now));
            }
            // Chromium downloaded Widevine for a new profile: it loads it at
            // the next start, so restart once it is there.
            if !widevine_at_start && !playing && chrome::has_widevine(profile) {
                log::info!("Widevine is installed; starting the player again to load it");
                return Some(Mode::Headless);
            }
        }
    }
}

/// Runs the sign-in window: an ordinary browser window, with no automation and
/// no bridge, so Spotify's own and third-party (Google, Apple, Facebook)
/// sign-in work. The user signs in and then closes the window; a window the user
/// closes is a clean Chromium quit, which flushes the new session's cookies to
/// disk (a killed browser does not), so the login survives a restart.
fn run_sign_in(
    binary: &std::path::Path,
    profile: &std::path::Path,
    queue: &mpsc::Receiver<Request>,
    events: &async_mpsc::UnboundedSender<Event>,
) -> Option<Mode> {
    let send = |event: Event| {
        let _ = events.send(event);
    };
    log::info!("sign-in: opening the login window");
    send(Event::Status(Status::SigningIn));
    let mut browser = match Browser::start(binary, profile, Mode::SignIn) {
        Ok(browser) => browser,
        Err(error) => {
            log::error!("the sign-in window did not open: {error}");
            send(Event::Status(Status::Failed(format!("The sign-in window did not open: {error}"))));
            return Some(Mode::Headless);
        }
    };
    loop {
        match queue.recv_timeout(Duration::from_millis(150)) {
            Ok(Request::Call { reply, .. }) => {
                let _ = reply.send(Err("signing in".into()));
            }
            // Cancel: drop the window and go back without a new session.
            Ok(Request::CancelSignIn) => return Some(Mode::Headless),
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        }
        if !browser.running() {
            // The user closed the window — Chromium flushed the session on its
            // clean exit; go back to the player, which reads it from disk.
            return Some(Mode::Headless);
        }
    }
}

/// Sets the headless player's page up — the bridge before the player's scripts,
/// the binding it posts through, a desktop user agent — then loads the player.
fn prepare_player(cdp: &Cdp, browser: &Browser) -> Result<(), String> {
    let t = Duration::from_secs(10);
    cdp.call("Page.enable", json!({}), t)?;
    cdp.call("Runtime.enable", json!({}), t)?;
    cdp.call("Runtime.addBinding", json!({ "name": "spotishEvent" }), t)?;
    cdp.call("Page.addScriptToEvaluateOnNewDocument", json!({ "source": BRIDGE }), t)?;
    if let Some(agent) = browser.user_agent() {
        cdp.call("Emulation.setUserAgentOverride", json!({ "userAgent": agent }), t)?;
    }
    cdp.call("Page.navigate", json!({ "url": chrome::PLAYER_URL }), t)?;
    Ok(())
}

fn has_session_cookie(cdp: &Cdp) -> bool {
    cdp.call(
        "Network.getCookies",
        json!({ "urls": ["https://open.spotify.com/", "https://accounts.spotify.com/"] }),
        Duration::from_secs(5),
    )
    .ok()
    .and_then(|v| v["cookies"].as_array().cloned())
    .is_some_and(|cookies| cookies.iter().any(|c| c["name"] == "sp_dc" && !c["value"].as_str().unwrap_or("").is_empty()))
}

fn call(cdp: &Cdp, name: &str, args: &Value) -> Answer {
    let args = if args.is_array() { args.clone() } else { json!([args]) };
    let expression = format!(
        "window.__spotish ? window.__spotish.call({}, {}) : Promise.reject(new Error('the player is still loading'))",
        serde_json::to_string(name).unwrap(),
        args
    );
    let value = cdp.evaluate(&expression, Duration::from_secs(40))?;
    let text = value.as_str().ok_or("the bridge answered nothing")?;
    let mut answer: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if answer["ok"].as_bool() == Some(true) {
        Ok(answer["value"].take())
    } else {
        Err(answer["error"].as_str().unwrap_or("the call failed").to_owned())
    }
}
