//! The process-wide music session, started on first use and shut down at exit.

use std::{
    sync::{Mutex, OnceLock},
    thread::JoinHandle,
    time::Duration,
};

use tokio::sync::watch;

use super::{
    demo,
    paths::Paths,
    session::{Session, Snapshot},
};

/// The process's engine.
pub struct Engine {
    session: Session,
    state: watch::Receiver<Snapshot>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

static ENGINE: OnceLock<Engine> = OnceLock::new();

/// How long the exit hook waits for the session to end.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(8);

/// The engine, started on first call: NetEase Cloud Music, or the offline
/// demo when `NSH_MUSIC_DEMO` is set (and not `0`).
pub fn engine() -> &'static Engine {
    ENGINE.get_or_init(|| {
        let demo = std::env::var_os("NSH_MUSIC_DEMO")
            .or_else(|| std::env::var_os("SPOTISH_DEMO"))
            .is_some_and(|v| v != "0");
        let (session, state, thread) = if demo { demo::session() } else { Session::start(Paths::platform()) };
        // SAFETY: registering a plain `extern "C" fn` with the C runtime.
        unsafe {
            libc::atexit(shut_down_at_exit);
        }
        Engine { session, state, thread: Mutex::new(Some(thread)) }
    })
}

extern "C" fn shut_down_at_exit() {
    if let Some(engine) = ENGINE.get() {
        engine.shutdown(SHUTDOWN_WAIT);
    }
}

impl Engine {
    /// A handle to ask the session things.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The latest snapshot.
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().clone()
    }

    /// A receiver of every later snapshot.
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.state.clone()
    }

    /// Stops playback and ends the session (once; later calls return at
    /// once). Waits up to `timeout`.
    pub fn shutdown(&self, timeout: Duration) {
        let Some(thread) = self.thread.lock().ok().and_then(|mut t| t.take()) else { return };
        if self.session.shutdown(timeout) {
            let _ = thread.join();
        } else {
            log::warn!("the engine did not stop within {timeout:?}");
        }
    }
}
