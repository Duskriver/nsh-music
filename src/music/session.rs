//! The request channel and observable state shared by the native and demo sessions.

use std::{sync::mpsc, thread::JoinHandle, time::Duration};

use serde_json::Value;
use tokio::sync::{oneshot, watch};

use super::{
    command::Command,
    model::{EngineError, ErrorCode, Health, Player},
    paths::Paths,
};

/// What a command answers: the response as JSON, or why not.
pub type Answer = Result<Value, EngineError>;

/// What the session publishes; TSX's listeners hear its changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// The player's state (empty until playback loads).
    pub player: Player,
    /// Where the session is.
    pub health: Health,
    /// Whether a NetEase Cloud Music account is signed in;
    /// `None` while unknown.
    pub signed_in: Option<bool>,
    /// Counts authentication changes: data that failed may now load.
    pub auth: u64,
}

/// What the session is asked.
pub enum Request {
    /// Runs an engine command.
    Command { command: Command, reply: oneshot::Sender<Answer> },
    /// Starts QR sign-in.
    SignIn,
    /// Cancels QR sign-in.
    CancelSignIn,
    /// Signs out of NetEase Cloud Music.
    SignOut,
    /// Tries to start the player again now.
    Retry,
    /// Ends the session thread; `ack` hears when it has.
    Shutdown(mpsc::Sender<()>),
}

/// A handle to the session; cheap to clone, usable from any thread.
#[derive(Clone)]
pub struct Session {
    requests: mpsc::Sender<Request>,
}

impl Session {
    /// Starts the session.
    pub fn start(paths: Paths) -> (Session, watch::Receiver<Snapshot>, JoinHandle<()>) {
        super::netease::start(paths)
    }

    /// Creates a session from a request sender.
    pub fn from_sender(requests: mpsc::Sender<Request>) -> Self {
        Session { requests }
    }

    /// A session that answers from `answer` (tests and
    /// `NSH_MUSIC_DEMO=1`): ready at once, signed in. `control` applies a
    /// control to the player's state, and says whether it changed it.
    pub fn fake(
        answer: impl Fn(&Command) -> Answer + Send + 'static,
        control: impl Fn(&mut Player, &Command) -> bool + Send + 'static,
        player: Player,
    ) -> (Session, watch::Receiver<Snapshot>, JoinHandle<()>) {
        let (requests, queue) = mpsc::channel::<Request>();
        let (out, state) = watch::channel(Snapshot { player, health: Health::ready(), signed_in: Some(true), auth: 0 });
        let thread = std::thread::Builder::new()
            .name("nsh-music-fake-session".into())
            .spawn(move || {
                while let Ok(request) = queue.recv() {
                    match request {
                        Request::Command { command, reply } => {
                            let result = answer(&command);
                            if result.is_ok() {
                                out.send_if_modified(|s| control(&mut s.player, &command));
                            }
                            let _ = reply.send(result);
                        }
                        Request::Shutdown(ack) => {
                            let _ = ack.send(());
                            return;
                        }
                        _ => {}
                    }
                }
            })
            .expect("the session thread starts");
        (Session { requests }, state, thread)
    }

    /// Runs `command` in the session; the receiver hears its answer.
    pub fn command(&self, command: Command) -> oneshot::Receiver<Answer> {
        let (reply, answer) = oneshot::channel();
        if let Err(mpsc::SendError(Request::Command { reply, .. })) =
            self.requests.send(Request::Command { command, reply })
        {
            let _ = reply.send(Err(EngineError::new(ErrorCode::Internal, "the engine has stopped")));
        }
        answer
    }

    fn send(&self, request: Request) {
        let _ = self.requests.send(request);
    }

    /// Starts QR sign-in.
    pub fn sign_in(&self) {
        self.send(Request::SignIn);
    }

    /// Cancels QR sign-in.
    pub fn cancel_sign_in(&self) {
        self.send(Request::CancelSignIn);
    }

    /// Signs out of NetEase Cloud Music.
    pub fn sign_out(&self) {
        self.send(Request::SignOut);
    }

    /// Starts the player again now, after a failure.
    pub fn retry(&self) {
        self.send(Request::Retry);
    }

    /// Asks the session to end; waits up to
    /// `timeout`. True when it ended in time.
    pub fn shutdown(&self, timeout: Duration) -> bool {
        let (ack, done) = mpsc::channel();
        if self.requests.send(Request::Shutdown(ack)).is_err() {
            return true;
        }
        done.recv_timeout(timeout).is_ok()
    }
}
