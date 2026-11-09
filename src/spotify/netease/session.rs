//! NetEase Cloud Music session runner and command dispatcher.

use std::{
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use log::warn;
use serde_json::{Value, json};
use tokio::sync::watch;

use super::{
    client::{NetEaseClient, QrPollResult},
    player::PlayerManager,
};
use crate::spotify::{
    bridge::Command,
    model::{EngineError, ErrorCode, Health, Player},
    paths::Paths,
    session::{Answer, Request, Session, Snapshot},
};

/// Starts the NetEase Cloud Music native session.
pub fn start(paths: Paths) -> (Session, watch::Receiver<Snapshot>, JoinHandle<()>) {
    let (requests, queue) = mpsc::channel();
    let client = Arc::new(NetEaseClient::new(&paths.data));
    let initial_signed_in = if client.is_signed_in() {
        Some(true)
    } else {
        Some(false)
    };

    let initial_snapshot = Snapshot {
        player: Player::default(),
        health: Health::ready(),
        signed_in: initial_signed_in,
        auth: 0,
    };

    let (out, state) = watch::channel(initial_snapshot.clone());

    let thread = std::thread::Builder::new()
        .name("netease-session".into())
        .spawn(move || {
            let mut runner = NetEaseRunner {
                queue,
                out,
                client,
                player: None, // initialized in thread
                snapshot: initial_snapshot,
                signing_in: None,
            };
            runner.run();
        })
        .expect("the netease session thread starts");

    (Session::from_sender(requests), state, thread)
}

struct SigningInState {
    unikey: String,
    last_poll: Instant,
}

struct NetEaseRunner {
    queue: mpsc::Receiver<Request>,
    out: watch::Sender<Snapshot>,
    client: Arc<NetEaseClient>,
    player: Option<PlayerManager>,
    snapshot: Snapshot,
    signing_in: Option<SigningInState>,
}

impl NetEaseRunner {
    fn run(&mut self) {
        self.player = Some(PlayerManager::new(self.client.clone()));

        loop {
            // Check for incoming requests with a 100ms timeout
            match self.queue.recv_timeout(Duration::from_millis(100)) {
                Ok(Request::Command { command, reply }) => {
                    let result = self.handle_command(command);
                    let _ = reply.send(result);
                    self.publish();
                }
                Ok(Request::SignIn) => {
                    self.start_sign_in();
                    self.publish();
                }
                Ok(Request::CancelSignIn) => {
                    self.signing_in = None;
                    self.snapshot.health = Health::ready();
                    self.publish();
                }
                Ok(Request::SignOut) => {
                    self.client.clear_session();
                    if let Some(ref mut p) = self.player {
                        let _ = p.pause();
                    }
                    self.snapshot.signed_in = Some(false);
                    self.snapshot.health = Health::ready();
                    self.snapshot.auth += 1;
                    self.publish();
                }
                Ok(Request::Retry) => {
                    self.snapshot.health = Health::ready();
                    self.publish();
                }
                Ok(Request::SetBridge(_)) => {}
                Ok(Request::Shutdown(ack)) => {
                    if let Some(ref mut p) = self.player {
                        let _ = p.pause();
                    }
                    let _ = ack.send(());
                    break;
                }
                Ok(Request::Evaluated { .. }) | Ok(Request::Cookies(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic tick
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }

            // 1. Tick player
            let mut player_changed = false;
            if let Some(ref mut p) = self.player {
                player_changed = p.tick();
            }

            // 2. Poll QR login if active
            if let Some(ref state) = self.signing_in {
                if state.last_poll.elapsed() >= Duration::from_millis(1500) {
                    let unikey = state.unikey.clone();
                    self.signing_in.as_mut().unwrap().last_poll = Instant::now();

                    match self.client.poll_qr_login(&unikey) {
                        Ok(QrPollResult::WaitingScan) => {
                            self.snapshot.health.prompt = "请使用网易云音乐手机 App 扫码登录".to_string();
                            self.publish();
                        }
                        Ok(QrPollResult::ScannedWaitingConfirm) => {
                            self.snapshot.health.prompt = "已扫码，请在手机上确认登录".to_string();
                            self.publish();
                        }
                        Ok(QrPollResult::Success) => {
                            self.signing_in = None;
                            self.snapshot.signed_in = Some(true);
                            self.snapshot.health = Health::ready();
                            self.snapshot.auth += 1;
                            self.publish();
                        }
                        Ok(QrPollResult::Expired) => {
                            self.signing_in = None;
                            self.snapshot.health = Health::failed(EngineError::new(
                                ErrorCode::Timeout,
                                "二维码已过期，请重新点击登录",
                            ));
                            self.publish();
                        }
                        Err(e) => {
                            warn!("QR poll error: {e}");
                        }
                    }
                }
            }

            if player_changed {
                self.publish();
            }
        }
    }

    fn publish(&mut self) {
        if let Some(ref p) = self.player {
            self.snapshot.player = p.state();
        }
        let _ = self.out.send(self.snapshot.clone());
    }

    fn start_sign_in(&mut self) {
        match self.client.start_qr_login() {
            Ok((unikey, qr_image)) => {
                self.snapshot.health = Health::signing_in_qr(
                    qr_image,
                    "请使用网易云音乐手机 App 扫码并在手机上确认登录",
                );
                self.signing_in = Some(SigningInState {
                    unikey,
                    last_poll: Instant::now(),
                });
            }
            Err(e) => {
                self.snapshot.health = Health::failed(e);
            }
        }
    }

    fn handle_command(&mut self, cmd: Command) -> Answer {
        match cmd {
            Command::Home => {
                let home = self.client.home()?;
                Ok(serde_json::to_value(home).unwrap_or(Value::Null))
            }
            Command::Search(q) => {
                let res = self.client.search(&q)?;
                Ok(serde_json::to_value(res).unwrap_or(Value::Null))
            }
            Command::Playlist(uri) => {
                let p = self.client.playlist(&uri)?;
                Ok(serde_json::to_value(p).unwrap_or(Value::Null))
            }
            Command::Album(uri) => {
                let a = self.client.album(&uri)?;
                Ok(serde_json::to_value(a).unwrap_or(Value::Null))
            }
            Command::Artist(uri) => {
                let ar = self.client.artist(&uri)?;
                Ok(serde_json::to_value(ar).unwrap_or(Value::Null))
            }
            Command::LikedSongs => {
                let l = self.client.liked_songs()?;
                Ok(serde_json::to_value(l).unwrap_or(Value::Null))
            }
            Command::Library => {
                let lib = self.client.library()?;
                Ok(serde_json::to_value(lib).unwrap_or(Value::Null))
            }
            Command::Me => {
                let me = self.client.me()?;
                Ok(serde_json::to_value(me).unwrap_or(Value::Null))
            }
            Command::Colors(_) => {
                // Return default cover colors
                Ok(json!({ "hex": "", "palette": [] }))
            }
            Command::Lyrics(uri) => {
                let l = self.client.lyrics(&uri)?;
                Ok(serde_json::to_value(l).unwrap_or(Value::Null))
            }
            Command::Toggle => {
                if let Some(ref mut p) = self.player {
                    p.toggle()?;
                }
                Ok(Value::Null)
            }
            Command::Play => {
                if let Some(ref mut p) = self.player {
                    p.play()?;
                }
                Ok(Value::Null)
            }
            Command::Pause => {
                if let Some(ref mut p) = self.player {
                    p.pause()?;
                }
                Ok(Value::Null)
            }
            Command::Next => {
                if let Some(ref mut p) = self.player {
                    p.next()?;
                }
                Ok(Value::Null)
            }
            Command::Previous => {
                if let Some(ref mut p) = self.player {
                    p.previous()?;
                }
                Ok(Value::Null)
            }
            Command::Seek(ms) => {
                if let Some(ref mut p) = self.player {
                    p.seek(ms)?;
                }
                Ok(Value::Null)
            }
            Command::SetVolume(vol) => {
                if let Some(ref mut p) = self.player {
                    p.set_volume(vol)?;
                }
                Ok(Value::Null)
            }
            Command::SetMuted(m) => {
                if let Some(ref mut p) = self.player {
                    p.set_muted(m)?;
                }
                Ok(Value::Null)
            }
            Command::SetShuffle(on) => {
                if let Some(ref mut p) = self.player {
                    p.set_shuffle(on)?;
                }
                Ok(Value::Null)
            }
            Command::SetRepeat(mode) => {
                if let Some(ref mut p) = self.player {
                    p.set_repeat(mode)?;
                }
                Ok(Value::Null)
            }
            Command::SetLiked(on) => {
                if let Some(ref mut p) = self.player {
                    p.set_liked(on)?;
                }
                Ok(Value::Null)
            }
            Command::PlayTrack(uri) => {
                let raw_id = uri.strip_prefix("spotify:track:").unwrap_or(&uri);
                let tracks = self.client.song_details(&[raw_id.to_string()])?;
                let track = tracks
                    .into_iter()
                    .next()
                    .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "未找到歌曲"))?;
                if let Some(ref mut p) = self.player {
                    p.play_track(track, format!("/track/{raw_id}"))?;
                }
                Ok(Value::Null)
            }
            Command::PlayContext { uri, track_uri } => {
                let tracks = if uri == "spotify:collection:tracks" {
                    self.client.liked_songs()?.tracks
                } else if uri == "spotify:playlist:daily" {
                    self.client.playlist("spotify:playlist:daily")?.tracks
                } else if uri.starts_with("spotify:playlist:") {
                    self.client.playlist(&uri)?.tracks
                } else if uri.starts_with("spotify:album:") {
                    self.client.album(&uri)?.tracks
                } else if uri.starts_with("spotify:artist:") {
                    self.client.artist(&uri)?.tracks
                } else {
                    Vec::new()
                };

                if let Some(ref mut p) = self.player {
                    p.play_context(&uri, track_uri.as_deref(), tracks)?;
                }
                Ok(Value::Null)
            }
        }
    }
}
