//! NetEase Cloud Music session runner and command dispatcher.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex, mpsc,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use log::warn;
use tokio::sync::{oneshot, watch};

use super::{
    client::{NetEaseClient, QrPollResult},
    player::{PlayerManager, PreparedAudio, SwitchPlan, TickAction, context_href_of, prepare_track_audio},
};
use crate::music::{
    command::{Command, CommandOutput},
    model::{CoverColor, EngineError, ErrorCode, Health, Player, Track},
    paths::Paths,
    session::{Answer, Request, Session, Snapshot},
};

const QR_POLL_INTERVAL: Duration = Duration::from_millis(1500);
const QR_TIMEOUT: Duration = Duration::from_secs(180);
const DATA_WORKERS: usize = 4;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct PoolInner {
    queue: Mutex<(VecDeque<Job>, bool)>,
    cvar: Condvar,
}

/// Lightweight fixed-size worker pool for NetEase API data fetches and prefetches,
/// avoiding per-request OS thread creation and bounding concurrent requests.
struct TaskPool {
    inner: Arc<PoolInner>,
}

impl TaskPool {
    fn new(workers: usize) -> Self {
        let inner = Arc::new(PoolInner {
            queue: Mutex::new((VecDeque::new(), false)),
            cvar: Condvar::new(),
        });
        for i in 0..workers {
            let worker = Arc::clone(&inner);
            let _ = std::thread::Builder::new()
                .name(format!("netease-worker-{i}"))
                .spawn(move || loop {
                    let job = {
                        let mut guard = worker.queue.lock().unwrap();
                        while guard.0.is_empty() && !guard.1 {
                            guard = worker.cvar.wait(guard).unwrap();
                        }
                        if guard.1 && guard.0.is_empty() {
                            return;
                        }
                        guard.0.pop_front()
                    };
                    if let Some(job) = job {
                        job();
                    }
                });
        }
        TaskPool { inner }
    }

    fn execute(&self, job: impl FnOnce() + Send + 'static) {
        let mut guard = self.inner.queue.lock().unwrap();
        if guard.1 {
            return;
        }
        guard.0.push_back(Box::new(job));
        self.inner.cvar.notify_one();
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        let mut guard = self.inner.queue.lock().unwrap();
        guard.1 = true;
        self.inner.cvar.notify_all();
    }
}

/// Starts the NetEase Cloud Music native session.
pub fn start(paths: Paths) -> (Session, watch::Receiver<Snapshot>, JoinHandle<()>) {
    let (requests, queue) = mpsc::channel();
    let wake_tx = requests.clone();
    let (events_tx, events_rx) = mpsc::channel();
    let client = Arc::new(NetEaseClient::new(&paths.data));
    let signed_in = client.is_signed_in();
    let initial_snapshot = Snapshot {
        player: Player::default(),
        health: Health::ready(),
        signed_in: Some(signed_in),
        auth: 0,
    };

    let (out, state) = watch::channel(initial_snapshot.clone());

    let thread = std::thread::Builder::new()
        .name("netease-session".into())
        .spawn(move || {
            let mut runner = NetEaseRunner {
                queue,
                wake_tx,
                events_tx,
                events_rx,
                out,
                client,
                pool: TaskPool::new(DATA_WORKERS),
                player: None, // initialized in thread
                snapshot: initial_snapshot,
                last_signed_in: signed_in,
                signing_in: None,
                active_load: None,
            };
            runner.run();
        })
        .expect("the netease session thread starts");

    (Session::from_sender(requests), state, thread)
}

struct SigningInState {
    unikey: String,
    started_at: Instant,
    last_poll: Instant,
}

struct ActiveLoad {
    load_id: u64,
    reply: Option<oneshot::Sender<Answer>>,
    auto_advance: bool,
    attempts: usize,
    context_href: String,
}

enum RunnerEvent {
    AudioPrepared {
        load_id: u64,
        result: Result<PreparedAudio, EngineError>,
    },
    SingleTrackReady {
        load_id: u64,
        context_href: String,
        result: Result<Track, EngineError>,
    },
    ContextTracksReady {
        load_id: u64,
        uri: String,
        track_uri: Option<String>,
        result: Result<Vec<Track>, EngineError>,
    },
    LikedRefreshed,
    AuthCheck,
}

struct NetEaseRunner {
    queue: mpsc::Receiver<Request>,
    wake_tx: mpsc::Sender<Request>,
    events_tx: mpsc::Sender<RunnerEvent>,
    events_rx: mpsc::Receiver<RunnerEvent>,
    out: watch::Sender<Snapshot>,
    client: Arc<NetEaseClient>,
    pool: TaskPool,
    player: Option<PlayerManager>,
    snapshot: Snapshot,
    last_signed_in: bool,
    signing_in: Option<SigningInState>,
    active_load: Option<ActiveLoad>,
}

impl NetEaseRunner {
    fn run(&mut self) {
        self.player = Some(PlayerManager::new(self.client.clone()));

        loop {
            self.drain_events();

            // Check for incoming requests with a 100ms timeout
            match self.queue.recv_timeout(Duration::from_millis(100)) {
                Ok(Request::Command { command, reply }) => {
                    self.dispatch_command(command, reply);
                }
                Ok(Request::Wake) => {
                    self.drain_events();
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
                    self.supersede_load();
                    if let Some(ref mut p) = self.player {
                        p.cancel_pending_load();
                        let _ = p.pause();
                    }
                    self.client.clear_session();
                    self.snapshot.health = Health::ready();
                    self.publish();
                }
                Ok(Request::Retry) => {
                    self.snapshot.health = Health::ready();
                    self.publish();
                }
                Ok(Request::Shutdown(ack)) => {
                    self.supersede_load();
                    if let Some(ref mut p) = self.player {
                        p.cancel_pending_load();
                        let _ = p.pause();
                    }
                    let _ = ack.send(());
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic tick
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }

            self.drain_events();

            // 1. Tick player
            let tick_action = self
                .player
                .as_mut()
                .map_or(TickAction::None, PlayerManager::tick);
            match tick_action {
                TickAction::None => {}
                TickAction::Changed => self.publish(),
                TickAction::ReplayCurrent => {
                    if let Some(track) = self.player.as_ref().and_then(|p| p.current_track().cloned()) {
                        let href = self
                            .player
                            .as_ref()
                            .map(|p| p.context_href().to_string())
                            .unwrap_or_default();
                        let plan = self
                            .player
                            .as_mut()
                            .unwrap()
                            .begin_track_load(track, href, 0);
                        self.launch_switch(plan, None, false);
                    }
                }
                TickAction::Ended => {
                    if let Some(ref mut p) = self.player {
                        match p.prepare_next() {
                            Ok(plan) => self.launch_switch(plan, None, true),
                            Err(_) => self.publish(),
                        }
                    }
                }
            }

            // 2. Poll QR login if active
            if let Some(ref state) = self.signing_in {
                if state.started_at.elapsed() >= QR_TIMEOUT {
                    self.signing_in = None;
                    self.snapshot.health = Health::failed(EngineError::new(
                        ErrorCode::Timeout,
                        "二维码已过期，请重新点击登录",
                    ));
                    self.publish();
                } else if state.last_poll.elapsed() >= QR_POLL_INTERVAL {
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
                            self.snapshot.health = Health::ready();
                            if self.last_signed_in {
                                // Account switched while already signed in; bump auth explicitly
                                // because `publish()` only bumps on signed-in state transitions.
                                self.snapshot.auth += 1;
                            }
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
                        Err(e) if e.code == ErrorCode::SignedOut => {
                            self.signing_in = None;
                            self.snapshot.health = Health::signing_in_qr("", e.message);
                            self.publish();
                        }
                        Err(e) => {
                            warn!("QR poll error: {e}");
                        }
                    }
                }
            }
        }
    }

    fn send_event(&self, event: RunnerEvent) {
        let _ = self.events_tx.send(event);
        let _ = self.wake_tx.send(Request::Wake);
    }

    fn supersede_load(&mut self) {
        if let Some(mut prev) = self.active_load.take() {
            if let Some(reply) = prev.reply.take() {
                let _ = reply.send(Ok(CommandOutput::Unit));
            }
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: RunnerEvent) {
        match event {
            RunnerEvent::AudioPrepared { load_id, result } => {
                if self.active_load.as_ref().is_none_or(|a| a.load_id != load_id) {
                    if let Ok(prepared) = result {
                        if let Some(cancel) = prepared.cancel {
                            cancel.cancel();
                        }
                    }
                    return;
                }
                let mut active = self.active_load.take().unwrap();
                match result {
                    Ok(prepared) => {
                        if let Some(ref mut p) = self.player {
                            p.finish_track_load(load_id, prepared);
                        }
                        if let Some(reply) = active.reply.take() {
                            let _ = reply.send(Ok(CommandOutput::Unit));
                        }
                        self.spawn_prefetch();
                        self.publish();
                    }
                    Err(err) => {
                        let stop_retry = err.code == ErrorCode::Internal || err.code == ErrorCode::SignedOut;
                        if active.auto_advance && !stop_retry {
                            if let Some(ref mut p) = self.player {
                                match p.advance_next_after_failure(active.attempts, active.context_href.clone()) {
                                    Ok(plan) => {
                                        self.launch_switch(plan, active.reply.take(), true);
                                        return;
                                    }
                                    Err(_) => {
                                        p.fail_track_load(load_id);
                                    }
                                }
                            }
                        } else if let Some(ref mut p) = self.player {
                            p.fail_track_load(load_id);
                        }
                        if let Some(reply) = active.reply.take() {
                            let _ = reply.send(Err(err));
                        }
                        self.publish();
                    }
                }
            }
            RunnerEvent::SingleTrackReady { load_id, context_href, result } => {
                if self.active_load.as_ref().is_none_or(|a| a.load_id != load_id) {
                    return;
                }
                let mut active = self.active_load.take().unwrap();
                match result {
                    Ok(track) => {
                        if let Some(ref mut p) = self.player {
                            let plan = p.start_single_track(track, context_href);
                            self.launch_switch(plan, active.reply.take(), false);
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut p) = self.player {
                            p.fail_track_load(load_id);
                        }
                        if let Some(reply) = active.reply.take() {
                            let _ = reply.send(Err(err));
                        }
                        self.publish();
                    }
                }
            }
            RunnerEvent::ContextTracksReady { load_id, uri, track_uri, result } => {
                if self.active_load.as_ref().is_none_or(|a| a.load_id != load_id) {
                    return;
                }
                let mut active = self.active_load.take().unwrap();
                match result {
                    Ok(tracks) => {
                        if let Some(ref mut p) = self.player {
                            match p.prepare_context(&uri, track_uri.as_deref(), tracks) {
                                Ok(plan) => {
                                    self.launch_switch(plan, active.reply.take(), track_uri.is_none());
                                }
                                Err(err) => {
                                    p.fail_track_load(load_id);
                                    if let Some(reply) = active.reply.take() {
                                        let _ = reply.send(Err(err));
                                    }
                                    self.publish();
                                }
                            }
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut p) = self.player {
                            p.fail_track_load(load_id);
                        }
                        if let Some(reply) = active.reply.take() {
                            let _ = reply.send(Err(err));
                        }
                        self.publish();
                    }
                }
            }
            RunnerEvent::LikedRefreshed => {
                if let Some(ref mut p) = self.player {
                    if p.refresh_liked_from_cache() {
                        self.publish();
                    }
                }
            }
            RunnerEvent::AuthCheck => {
                if self.client.is_signed_in() != self.last_signed_in {
                    self.publish();
                }
            }
        }
    }

    fn spawn_prefetch(&mut self) {
        let Some(ref mut p) = self.player else {
            return;
        };
        let Some(next_uri) = p.peek_upcoming_track_uri() else {
            return;
        };
        let client = Arc::clone(&self.client);
        self.pool.execute(move || {
            let _ = client.stream_url(&next_uri);
        });
    }

    fn launch_switch(&mut self, plan: SwitchPlan, reply: Option<oneshot::Sender<Answer>>, auto_advance: bool) {
        match plan {
            SwitchPlan::Paused => {
                self.supersede_load();
                if let Some(r) = reply {
                    let _ = r.send(Ok(CommandOutput::Unit));
                }
                self.publish();
            }
            SwitchPlan::Load {
                load_id,
                track,
                context_href,
                attempts,
                liked_needs_refresh,
            } => {
                self.supersede_load();
                self.active_load = Some(ActiveLoad {
                    load_id,
                    reply,
                    auto_advance,
                    attempts,
                    context_href,
                });

                if liked_needs_refresh {
                    let events_tx = self.events_tx.clone();
                    let wake_tx = self.wake_tx.clone();
                    self.client.try_start_liked_refresh(move || {
                        let _ = events_tx.send(RunnerEvent::LikedRefreshed);
                        let _ = wake_tx.send(Request::Wake);
                    });
                }

                self.spawn_prefetch();
                self.publish();

                let Some(ref p) = self.player else {
                    return;
                };
                let need_audio = p.has_audio_sink();
                let load_seq: Arc<AtomicU64> = p.load_seq();
                let client = Arc::clone(&self.client);
                let events_tx = self.events_tx.clone();
                let wake_tx = self.wake_tx.clone();
                let track_uri = track.uri;

                if std::thread::Builder::new()
                    .name("netease-track-load".into())
                    .spawn(move || {
                        let result = prepare_track_audio(&client, &track_uri, need_audio, &|| {
                            load_seq.load(Ordering::Acquire) != load_id
                        });
                        let _ = events_tx.send(RunnerEvent::AudioPrepared { load_id, result });
                        let _ = wake_tx.send(Request::Wake);
                    })
                    .is_err()
                {
                    self.send_event(RunnerEvent::AudioPrepared {
                        load_id,
                        result: Err(EngineError::new(ErrorCode::Internal, "无法启动音频加载线程")),
                    });
                }
            }
        }
    }

    fn publish(&mut self) {
        let now_signed_in = self.client.is_signed_in();
        if now_signed_in != self.last_signed_in {
            self.last_signed_in = now_signed_in;
            self.snapshot.auth += 1;
        }
        self.snapshot.signed_in = Some(now_signed_in);
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
                let now = Instant::now();
                self.signing_in = Some(SigningInState {
                    unikey,
                    started_at: now,
                    last_poll: now,
                });
            }
            Err(e) => {
                self.snapshot.health = Health::failed(e);
            }
        }
    }

    fn dispatch_command(&mut self, cmd: Command, reply: oneshot::Sender<Answer>) {
        match cmd {
            Command::Colors(_) => {
                let _ = reply.send(Ok(CommandOutput::Colors(CoverColor::default())));
            }
            Command::Home
            | Command::Search(_)
            | Command::Playlist(_)
            | Command::Album(_)
            | Command::Artist(_)
            | Command::LikedSongs
            | Command::Library
            | Command::Me
            | Command::Lyrics(_) => {
                let client = Arc::clone(&self.client);
                let was_signed_in = self.last_signed_in;
                let events_tx = self.events_tx.clone();
                let wake_tx = self.wake_tx.clone();
                self.pool.execute(move || {
                    let answer = run_data_command(&client, cmd);
                    let _ = reply.send(answer);
                    if client.is_signed_in() != was_signed_in {
                        let _ = events_tx.send(RunnerEvent::AuthCheck);
                        let _ = wake_tx.send(Request::Wake);
                    }
                });
            }
            Command::Toggle => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), PlayerManager::toggle)
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::Play => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), PlayerManager::play)
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::Pause => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), PlayerManager::pause)
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::Next => {
                let Some(ref mut p) = self.player else {
                    let _ = reply.send(Ok(CommandOutput::Unit));
                    return;
                };
                match p.prepare_next() {
                    Ok(plan) => self.launch_switch(plan, Some(reply), true),
                    Err(err) => {
                        let _ = reply.send(Err(err));
                        self.publish();
                    }
                }
            }
            Command::Previous => {
                let Some(ref mut p) = self.player else {
                    let _ = reply.send(Ok(CommandOutput::Unit));
                    return;
                };
                match p.prepare_previous() {
                    Ok(plan) => self.launch_switch(plan, Some(reply), false),
                    Err(err) => {
                        let _ = reply.send(Err(err));
                        self.publish();
                    }
                }
            }
            Command::Seek(ms) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.seek(ms))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::SetVolume(vol) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.set_volume(vol))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::SetMuted(m) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.set_muted(m))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::SetShuffle(on) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.set_shuffle(on))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::SetRepeat(mode) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.set_repeat(mode))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::SetQuality(quality) => {
                let plan = self.player.as_mut().and_then(|p| p.set_quality(quality));
                if let Some(plan) = plan {
                    self.launch_switch(plan, Some(reply), false);
                } else {
                    let _ = reply.send(Ok(CommandOutput::Unit));
                    self.publish();
                }
            }
            Command::SetLiked(on) => {
                let res = self
                    .player
                    .as_mut()
                    .map_or(Ok(()), |p| p.set_liked(on))
                    .map(|()| CommandOutput::Unit);
                let _ = reply.send(res);
                self.publish();
            }
            Command::PlayTrack(uri) => {
                let raw_id = uri.strip_prefix("netease:track:").unwrap_or(&uri).to_string();
                let context_href = format!("/track/{raw_id}");
                let cached = self
                    .player
                    .as_ref()
                    .and_then(|p| p.find_queued_track(&uri))
                    .or_else(|| self.client.cached_track(&uri));

                if let Some(track) = cached {
                    if let Some(ref mut p) = self.player {
                        let plan = p.start_single_track(track, context_href);
                        self.launch_switch(plan, Some(reply), false);
                    } else {
                        let _ = reply.send(Ok(CommandOutput::Unit));
                    }
                    return;
                }

                self.supersede_load();
                let Some(ref mut p) = self.player else {
                    let _ = reply.send(Ok(CommandOutput::Unit));
                    return;
                };
                let load_id = p.begin_pending_metadata(None, context_href.clone());
                let load_seq = p.load_seq();
                self.active_load = Some(ActiveLoad {
                    load_id,
                    reply: Some(reply),
                    auto_advance: false,
                    attempts: 0,
                    context_href: context_href.clone(),
                });
                self.publish();

                let client = Arc::clone(&self.client);
                let events_tx = self.events_tx.clone();
                let wake_tx = self.wake_tx.clone();
                let _ = std::thread::Builder::new()
                    .name("netease-track-meta".into())
                    .spawn(move || {
                        if load_seq.load(Ordering::Acquire) != load_id {
                            return;
                        }
                        let result = client.song_details(&[raw_id]).and_then(|tracks| {
                            tracks
                                .into_iter()
                                .next()
                                .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "未找到歌曲"))
                        });
                        let _ = events_tx.send(RunnerEvent::SingleTrackReady {
                            load_id,
                            context_href,
                            result,
                        });
                        let _ = wake_tx.send(Request::Wake);
                    });
            }
            Command::PlayContext { uri, track_uri } => {
                let cached_tracks = self
                    .client
                    .cached_collection_tracks(&uri)
                    .or_else(|| {
                        self.player
                            .as_ref()
                            .and_then(|p| p.reusable_queue(&uri, track_uri.as_deref()))
                    });

                if let Some(tracks) = cached_tracks {
                    if let Some(ref mut p) = self.player {
                        match p.prepare_context(&uri, track_uri.as_deref(), tracks) {
                            Ok(plan) => self.launch_switch(plan, Some(reply), track_uri.is_none()),
                            Err(err) => {
                                let _ = reply.send(Err(err));
                                self.publish();
                            }
                        }
                    } else {
                        let _ = reply.send(Ok(CommandOutput::Unit));
                    }
                    return;
                }

                self.supersede_load();
                let Some(ref mut p) = self.player else {
                    let _ = reply.send(Ok(CommandOutput::Unit));
                    return;
                };
                let context_href = context_href_of(&uri);
                let load_id = p.begin_pending_metadata(Some(&uri), context_href.clone());
                let load_seq = p.load_seq();
                self.active_load = Some(ActiveLoad {
                    load_id,
                    reply: Some(reply),
                    auto_advance: track_uri.is_none(),
                    attempts: 0,
                    context_href,
                });
                self.publish();

                let client = Arc::clone(&self.client);
                let events_tx = self.events_tx.clone();
                let wake_tx = self.wake_tx.clone();
                let _ = std::thread::Builder::new()
                    .name("netease-context-meta".into())
                    .spawn(move || {
                        if load_seq.load(Ordering::Acquire) != load_id {
                            return;
                        }
                        let result = client.collection_tracks_for_playback(&uri);
                        let _ = events_tx.send(RunnerEvent::ContextTracksReady {
                            load_id,
                            uri,
                            track_uri,
                            result,
                        });
                        let _ = wake_tx.send(Request::Wake);
                    });
            }
        }
    }
}

fn run_data_command(client: &NetEaseClient, cmd: Command) -> Answer {
    match cmd {
        Command::Home => Ok(CommandOutput::Home(client.home()?)),
        Command::Search(q) => Ok(CommandOutput::Search(client.search(&q)?)),
        Command::Playlist(uri) => Ok(CommandOutput::Collection(client.playlist(&uri)?)),
        Command::Album(uri) => Ok(CommandOutput::Collection(client.album(&uri)?)),
        Command::Artist(uri) => Ok(CommandOutput::Collection(client.artist(&uri)?)),
        Command::LikedSongs => Ok(CommandOutput::Collection(client.liked_songs()?)),
        Command::Library => Ok(CommandOutput::Library(client.library()?)),
        Command::Me => Ok(CommandOutput::Me(client.me()?)),
        Command::Lyrics(uri) => Ok(CommandOutput::Lyrics(client.lyrics(&uri)?)),
        _ => Ok(CommandOutput::Unit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishing_reflects_a_session_cleared_by_the_client() {
        let data = std::env::temp_dir().join(format!("nsh-music-netease-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        let client = Arc::new(NetEaseClient::new(&data));
        client.save_cookie_str("MUSIC_U=fixture");
        let snapshot = Snapshot {
            player: Player::default(),
            health: Health::ready(),
            signed_in: Some(client.is_signed_in()),
            auth: 0,
        };
        let (out, state) = watch::channel(snapshot.clone());
        let (wake_tx, queue) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let mut runner = NetEaseRunner {
            queue,
            wake_tx,
            events_tx,
            events_rx,
            out,
            client: client.clone(),
            pool: TaskPool::new(1),
            player: None,
            snapshot,
            last_signed_in: true,
            signing_in: None,
            active_load: None,
        };
        assert_eq!(state.borrow().signed_in, Some(true));

        client.clear_session();
        runner.publish();
        let _ = std::fs::remove_dir_all(data);

        assert_eq!(state.borrow().signed_in, Some(false));
        assert_eq!(state.borrow().auth, 1);
    }
}
