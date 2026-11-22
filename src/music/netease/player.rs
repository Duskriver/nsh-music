//! Native audio player for NetEase Cloud Music using rodio and audio sample tapping.

use std::{
    io::{Read, Seek},
    num::{NonZeroU16, NonZeroU32},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use log::warn;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player as RodioPlayer, Source, source::SeekError};

use super::client::{NetEaseClient, StreamCancelHandle};
use crate::music::model::{AudioQuality, EngineError, ErrorCode, Player, Repeat, Track};

/// Number of mono PCM samples per audio-tap broadcast batch (aligned with
/// `audio::analysis::HOP` = 400 samples ≈ 8.3 ms at 48 kHz).
pub const AUDIO_TAP_CHUNK: usize = 400;

/// A fixed-size batch of mono PCM samples sent from the audio playback thread
/// to the spectrum analyzer without per-sample locking or heap allocations.
#[derive(Clone)]
pub struct AudioChunk(pub [f32; AUDIO_TAP_CHUNK]);

static AUDIO_TAP: OnceLock<tokio::sync::broadcast::Sender<AudioChunk>> = OnceLock::new();

/// Maximum audio payload limit for streaming downloads (512 MB).
/// NetEase audio tracks (especially high-bitrate 320kbps MP3s and Hi-Res FLACs)
/// easily exceed ureq's default 10 MB body limit.
const MAX_AUDIO_BYTES: u64 = 512 * 1024 * 1024;

/// Returns the global broadcast channel sender for batched mono audio samples.
pub fn get_audio_tap() -> &'static tokio::sync::broadcast::Sender<AudioChunk> {
    AUDIO_TAP.get_or_init(|| {
        // 32 chunks * 400 samples = 12,800 samples (~267 ms at 48 kHz).
        let (tx, _rx) = tokio::sync::broadcast::channel(32);
        tx
    })
}

fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// A wrapper around a rodio Source that copies batched mono samples to the audio tap.
pub struct MeteredSource<S> {
    inner: S,
    channels: NonZeroU16,
    sample_rate: NonZeroU32,
    channel_idx: u16,
    left_sample: Option<f32>,
    tap: &'static tokio::sync::broadcast::Sender<AudioChunk>,
    tap_buf: [f32; AUDIO_TAP_CHUNK],
    tap_len: usize,
}

fn decode_stream<R: Read + Seek + Send + Sync + 'static>(
    reader: R,
    byte_len: Option<u64>,
    hint: Option<&str>,
    mime_type: Option<&str>,
) -> Result<MeteredSource<Decoder<R>>, EngineError> {
    let mut builder = Decoder::builder().with_data(reader);
    if let Some(len) = byte_len {
        builder = builder.with_byte_len(len).with_seekable(true);
    }
    if let Some(ext) = hint {
        builder = builder.with_hint(ext);
    }
    if let Some(mime) = mime_type {
        builder = builder.with_mime_type(mime);
    }
    let decoder = builder
        .build()
        .map_err(|e| EngineError::new(ErrorCode::Shape, format!("音频解码失败: {e}")))?;
    Ok(MeteredSource {
        channels: decoder.channels(),
        sample_rate: decoder.sample_rate(),
        inner: decoder,
        channel_idx: 0,
        left_sample: None,
        tap: get_audio_tap(),
        tap_buf: [0.0; AUDIO_TAP_CHUNK],
        tap_len: 0,
    })
}

#[cfg(test)]
fn decode_audio(bytes: Vec<u8>) -> Result<MeteredSource<Decoder<std::io::Cursor<Vec<u8>>>>, EngineError> {
    let len = bytes.len() as u64;
    decode_stream(std::io::Cursor::new(bytes), Some(len), None, None)
}

impl<S: Source<Item = f32>> Source for MeteredSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> NonZeroU16 {
        self.channels
    }
    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(position)?;
        // The decoder preserves the active channel. Discard any half-frame
        // and buffered samples from before the jump.
        self.left_sample = None;
        self.channel_idx = 0;
        self.tap_len = 0;
        Ok(())
    }
}

impl<S: Source<Item = f32>> MeteredSource<S> {
    #[inline]
    fn push_mono(&mut self, mono: f32) {
        self.tap_buf[self.tap_len] = mono;
        self.tap_len += 1;
        if self.tap_len == AUDIO_TAP_CHUNK {
            let _ = self.tap.send(AudioChunk(self.tap_buf));
            self.tap_len = 0;
        }
    }
}

impl<S: Source<Item = f32>> Iterator for MeteredSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let ch = self.channels.get();
        // Fast path: skip mixing and broadcast locking when no visualizer is listening.
        if self.tap.receiver_count() == 0 {
            if self.tap_len > 0 || self.left_sample.is_some() {
                self.tap_len = 0;
                self.left_sample = None;
            }
            if ch > 1 {
                self.channel_idx = (self.channel_idx + 1) % ch;
            }
            return Some(sample);
        }
        if ch == 1 {
            self.push_mono(sample);
        } else if ch == 2 {
            if self.channel_idx == 0 {
                self.left_sample = Some(sample);
                self.channel_idx = 1;
            } else {
                if let Some(left) = self.left_sample.take() {
                    self.push_mono((left + sample) * 0.5);
                }
                self.channel_idx = 0;
            }
        }
        Some(sample)
    }
}

struct StreamState {
    _handle: MixerDeviceSink,
    player: RodioPlayer,
}

/// Prepared progressive audio source ready to be appended to `rodio::Player`.
pub struct PreparedAudio {
    /// Decoded and metered audio stream.
    pub source: Option<MeteredSource<Decoder<super::client::ProgressiveStreamReader>>>,
    /// Handle to abort background download on track switch or stop.
    pub cancel: Option<StreamCancelHandle>,
}

/// Result of a periodic player tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickAction {
    /// Nothing changed.
    None,
    /// Position/state changed without changing tracks.
    Changed,
    /// Current track ended under `Repeat::Track` and should restart.
    ReplayCurrent,
    /// Current track ended and should advance to the next track.
    Ended,
}

/// Decision produced when initiating a track transition.
#[derive(Debug, Clone)]
pub enum SwitchPlan {
    /// Playback paused (e.g. reached the end of queue with repeat off).
    Paused,
    /// A track was selected and is ready to be loaded in background.
    Load {
        load_id: u64,
        track: Track,
        context_href: String,
        attempts: usize,
        liked_needs_refresh: bool,
    },
}

/// Converts a `netease:` context URI into its canonical href path.
pub fn context_href_of(context_uri: &str) -> String {
    if context_uri == "netease:collection:tracks" {
        "/collection/tracks".to_string()
    } else if let Some(id) = context_uri.strip_prefix("netease:playlist:") {
        format!("/playlist/{id}")
    } else if let Some(id) = context_uri.strip_prefix("netease:album:") {
        format!("/album/{id}")
    } else if let Some(id) = context_uri.strip_prefix("netease:artist:") {
        format!("/artist/{id}")
    } else {
        context_uri.to_string()
    }
}

fn open_and_decode(client: &NetEaseClient, url: &str) -> Result<PreparedAudio, EngineError> {
    let stream = client.open_audio_stream(url, MAX_AUDIO_BYTES)?;
    let cancel = stream.cancel.clone();
    match decode_stream(
        stream.reader,
        stream.byte_len,
        stream.hint.as_deref(),
        stream.mime_type.as_deref(),
    ) {
        Ok(source) => Ok(PreparedAudio {
            source: Some(source),
            cancel: Some(cancel),
        }),
        Err(e) => {
            cancel.cancel();
            Err(e)
        }
    }
}

/// Resolves the track stream URL and opens a progressive decoder, retrying once if a cached CDN URL failed.
pub fn prepare_track_audio(
    client: &NetEaseClient,
    track_uri: &str,
    need_audio: bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PreparedAudio, EngineError> {
    if is_cancelled() {
        return Err(EngineError::new(ErrorCode::Internal, "音频加载已取消"));
    }
    let stream_url = client.stream_url(track_uri)?;
    if !need_audio {
        return Ok(PreparedAudio {
            source: None,
            cancel: None,
        });
    }
    if is_cancelled() {
        return Err(EngineError::new(ErrorCode::Internal, "音频加载已取消"));
    }
    match open_and_decode(client, &stream_url) {
        Ok(prepared) => Ok(prepared),
        Err(first_err) => {
            // If a cached CDN URL expired or failed, invalidate and resolve a fresh stream URL once.
            client.invalidate_stream_cache(track_uri);
            if is_cancelled() {
                return Err(first_err);
            }
            let fresh_url = client.stream_url(track_uri)?;
            if fresh_url == stream_url || is_cancelled() {
                return Err(first_err);
            }
            open_and_decode(client, &fresh_url)
        }
    }
}

/// Native player manager.
pub struct PlayerManager {
    client: Arc<NetEaseClient>,
    stream_state: Option<StreamState>,
    active_stream_cancel: Option<StreamCancelHandle>,
    queue: Vec<Track>,
    queue_index: usize,
    next_shuffle_index: Option<usize>,
    current_track: Option<Track>,
    context_uri: String,
    context_href: String,
    playing: bool,
    loading: bool,
    load_seq: Arc<AtomicU64>,
    pending_seek_ms: Option<f64>,
    volume: f64,
    muted: bool,
    shuffle: bool,
    repeat: Repeat,
    liked: bool,
    position_ms: f64,
    play_started_at: Option<Instant>,
}

impl PlayerManager {
    /// Creates a new player manager.
    pub fn new(client: Arc<NetEaseClient>) -> Self {
        let stream_state = DeviceSinkBuilder::open_default_sink()
            .ok()
            .map(|handle| {
                let player = RodioPlayer::connect_new(&handle.mixer());
                StreamState {
                    _handle: handle,
                    player,
                }
            });

        if stream_state.is_none() {
            warn!("no audio output device available (running headless)");
        }

        PlayerManager {
            client,
            stream_state,
            active_stream_cancel: None,
            queue: Vec::new(),
            queue_index: 0,
            next_shuffle_index: None,
            current_track: None,
            context_uri: String::new(),
            context_href: String::new(),
            playing: false,
            loading: false,
            load_seq: Arc::new(AtomicU64::new(0)),
            pending_seek_ms: None,
            volume: 1.0,
            muted: false,
            shuffle: false,
            repeat: Repeat::Off,
            liked: false,
            position_ms: 0.0,
            play_started_at: None,
        }
    }

    /// Returns whether an audio output sink is active.
    pub fn has_audio_sink(&self) -> bool {
        self.stream_state.is_some()
    }

    /// Returns the shared monotonic load generation counter for cancellation checks.
    pub fn load_seq(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.load_seq)
    }

    /// Returns the current queue length.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Returns the current track if any.
    pub fn current_track(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    /// Returns the current context href.
    pub fn context_href(&self) -> &str {
        &self.context_href
    }

    /// Looks up a track by URI in the active playback queue without network requests.
    pub fn find_queued_track(&self, uri: &str) -> Option<Track> {
        self.queue.iter().find(|t| t.uri == uri).cloned()
    }

    /// Returns the active queue if `context_uri` matches and `track_uri` (if given) is in it.
    pub fn reusable_queue(&self, context_uri: &str, track_uri: Option<&str>) -> Option<Vec<Track>> {
        if self.context_uri != context_uri || self.queue.is_empty() {
            return None;
        }
        if let Some(t_uri) = track_uri {
            if !self.queue.iter().any(|t| t.uri == t_uri) {
                return None;
            }
        }
        Some(self.queue.clone())
    }

    fn cancel_active_stream(&mut self) {
        if let Some(cancel) = self.active_stream_cancel.take() {
            cancel.cancel();
        }
    }

    /// Cancels any in-flight track load or active audio stream and stops playback.
    pub fn cancel_pending_load(&mut self) -> u64 {
        self.cancel_active_stream();
        if let Some(ref st) = self.stream_state {
            st.player.stop();
        }
        self.loading = false;
        self.pending_seek_ms = None;
        self.load_seq.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Prepares state for an asynchronous metadata + audio load (cold cache path).
    pub fn begin_pending_metadata(&mut self, context_uri: Option<&str>, context_href: String) -> u64 {
        let load_id = self.cancel_pending_load();
        if let Some(uri) = context_uri {
            self.context_uri = uri.to_string();
        } else {
            self.context_uri.clear();
        }
        self.context_href = context_href;
        self.playing = true;
        self.loading = true;
        self.position_ms = 0.0;
        self.play_started_at = None;
        load_id
    }

    /// Current playback position in milliseconds.
    pub fn position_ms(&self) -> f64 {
        if self.loading {
            return self.pending_seek_ms.unwrap_or(0.0);
        }
        if self.playing {
            if let Some(ref st) = self.stream_state {
                return st.player.get_pos().as_millis() as f64;
            }
            if let Some(started) = self.play_started_at {
                let elapsed = started.elapsed().as_millis() as f64;
                return self.position_ms + elapsed;
            }
        }
        self.position_ms
    }

    /// Builds the `Player` state object.
    pub fn state(&self) -> Player {
        let (title, artist, album, artwork, track_uri, duration_ms) =
            if let Some(ref t) = self.current_track {
                (
                    t.name.clone(),
                    t.artists.clone(),
                    t.album.clone(),
                    t.image.clone(),
                    t.uri.clone(),
                    t.duration_ms,
                )
            } else {
                (String::new(), String::new(), String::new(), String::new(), String::new(), 0.0)
            };

        let can_next = self.queue_index + 1 < self.queue.len() || self.repeat == Repeat::Context;
        let can_previous = self.queue_index > 0 || self.position_ms() > 3000.0;

        Player {
            title,
            artist,
            album,
            artwork,
            track_uri,
            context_href: self.context_href.clone(),
            playing: self.playing,
            can_play: true,
            position_ms: self.position_ms().min(duration_ms),
            duration_ms,
            at: now_millis(),
            shuffle: self.shuffle,
            shuffle_available: true,
            repeat: self.repeat,
            quality: self.client.quality(),
            volume: self.volume,
            muted: self.muted,
            liked: self.liked,
            can_next,
            can_previous,
        }
    }

    /// Updates `self.liked` from the in-memory cache; returns true if the value changed.
    pub fn refresh_liked_from_cache(&mut self) -> bool {
        let Some(ref track) = self.current_track else {
            return false;
        };
        let (liked, _) = self.client.cached_track_liked(&track.uri);
        if liked != self.liked {
            self.liked = liked;
            return true;
        }
        false
    }

    /// Immediately updates player UI state, stops old audio, and returns a load ID
    /// for asynchronous stream preparation.
    pub fn begin_track_load(
        &mut self,
        track: Track,
        context_href: String,
        attempts: usize,
    ) -> SwitchPlan {
        let load_id = self.cancel_pending_load();
        let (liked, liked_needs_refresh) = self.client.cached_track_liked(&track.uri);
        self.liked = liked;
        self.current_track = Some(track.clone());
        self.context_href = context_href.clone();
        self.playing = true;
        self.loading = true;
        self.position_ms = 0.0;
        self.play_started_at = None;
        SwitchPlan::Load {
            load_id,
            track,
            context_href,
            attempts,
            liked_needs_refresh,
        }
    }

    /// Starts playing a single standalone track (clears context URI).
    pub fn start_single_track(&mut self, track: Track, context_href: String) -> SwitchPlan {
        self.context_uri.clear();
        self.next_shuffle_index = None;
        self.begin_track_load(track, context_href, 0)
    }

    /// Applies the prepared audio stream if `load_id` is still active.
    pub fn finish_track_load(&mut self, load_id: u64, prepared: PreparedAudio) -> bool {
        if self.load_seq.load(Ordering::Acquire) != load_id {
            if let Some(cancel) = prepared.cancel {
                cancel.cancel();
            }
            return false;
        }
        self.loading = false;
        let seek_ms = self.pending_seek_ms.take();
        if let Some(ref mut st) = self.stream_state {
            st.player.stop();
            if let Some(source) = prepared.source {
                st.player.append(source);
            }
            st.player.set_volume(if self.muted { 0.0 } else { self.volume as f32 });
            if let Some(ms) = seek_ms {
                let dur = Duration::from_millis(ms as u64);
                let _ = st.player.try_seek(dur);
                self.position_ms = ms;
            }
            if self.playing {
                st.player.play();
            } else {
                st.player.pause();
            }
            self.active_stream_cancel = prepared.cancel;
        } else {
            if let Some(cancel) = prepared.cancel {
                cancel.cancel();
            }
            if let Some(ms) = seek_ms {
                self.position_ms = ms;
            }
        }
        self.play_started_at = if self.playing { Some(Instant::now()) } else { None };
        true
    }

    /// Marks an active load as failed and pauses playback if `load_id` is still current.
    pub fn fail_track_load(&mut self, load_id: u64) -> bool {
        if self.load_seq.load(Ordering::Acquire) != load_id {
            return false;
        }
        self.loading = false;
        self.playing = false;
        self.pending_seek_ms = None;
        self.play_started_at = None;
        true
    }

    /// Computes and returns the URI of the next likely playable track for background URL prefetch.
    pub fn peek_upcoming_track_uri(&mut self) -> Option<String> {
        let len = self.queue.len();
        if len <= 1 && self.repeat != Repeat::Context {
            return None;
        }
        if len == 0 {
            return None;
        }
        let has_marked_playable = self.queue.iter().any(|t| t.playable);
        let candidate = if self.shuffle {
            let idx = match self.next_shuffle_index {
                Some(idx) if idx < len && idx != self.queue_index => idx,
                _ => {
                    let picked = (self.queue_index + 1 + (now_millis() as usize % (len.max(2) - 1))) % len;
                    self.next_shuffle_index = Some(picked);
                    picked
                }
            };
            Some(idx)
        } else if self.queue_index + 1 < len {
            Some(self.queue_index + 1)
        } else if self.repeat == Repeat::Context {
            Some(0)
        } else {
            None
        }?;

        let mut idx = candidate;
        for _ in 0..len {
            if !has_marked_playable || self.queue[idx].playable {
                if !self.client.has_fresh_stream_url(&self.queue[idx].uri) {
                    return Some(self.queue[idx].uri.clone());
                }
                return None;
            }
            idx = (idx + 1) % len;
            if idx == 0 && self.repeat != Repeat::Context && !self.shuffle {
                break;
            }
        }
        None
    }

    /// Prepares a context (`playlist`, `album`, `artist`, `liked songs`) for playback.
    pub fn prepare_context(
        &mut self,
        context_uri: &str,
        track_uri: Option<&str>,
        tracks: Vec<Track>,
    ) -> Result<SwitchPlan, EngineError> {
        if tracks.is_empty() {
            return Err(EngineError::new(ErrorCode::NotFound, "列表为空，没有可播放的歌曲"));
        }

        let context_href = context_href_of(context_uri);
        let start_index = if let Some(t_uri) = track_uri {
            tracks.iter().position(|t| t.uri == t_uri).unwrap_or(0)
        } else {
            0
        };

        self.context_uri = context_uri.to_string();
        self.queue = tracks;
        self.queue_index = start_index;
        self.next_shuffle_index = None;

        if track_uri.is_some() {
            let track_to_play = self.queue[start_index].clone();
            Ok(self.begin_track_load(track_to_play, context_href, 0))
        } else {
            let has_marked_playable = self.queue.iter().skip(start_index).any(|t| t.playable);
            let Some(idx) = (start_index..self.queue.len())
                .find(|&i| !has_marked_playable || self.queue[i].playable)
            else {
                return Err(EngineError::new(ErrorCode::NotFound, "没有可播放的歌曲（可能需要 VIP）"));
            };
            self.queue_index = idx;
            let track = self.queue[idx].clone();
            Ok(self.begin_track_load(track, context_href, idx - start_index))
        }
    }

    /// Plays a given track synchronously (used by tests or direct callers).
    pub fn play_track(&mut self, track: Track, context_href: String) -> Result<(), EngineError> {
        let plan = self.start_single_track(track.clone(), context_href);
        let SwitchPlan::Load { load_id, .. } = plan else {
            return Ok(());
        };
        let need_audio = self.has_audio_sink();
        match prepare_track_audio(&self.client, &track.uri, need_audio, &|| false) {
            Ok(prepared) => {
                self.finish_track_load(load_id, prepared);
                Ok(())
            }
            Err(err) => {
                self.fail_track_load(load_id);
                Err(err)
            }
        }
    }

    /// Plays a context synchronously (used by direct callers).
    pub fn play_context(
        &mut self,
        context_uri: &str,
        track_uri: Option<&str>,
        tracks: Vec<Track>,
    ) -> Result<(), EngineError> {
        let plan = self.prepare_context(context_uri, track_uri, tracks)?;
        let SwitchPlan::Load { load_id, mut track, context_href, mut attempts, .. } = plan else {
            return Ok(());
        };
        let need_audio = self.has_audio_sink();
        let mut active_id = load_id;
        loop {
            match prepare_track_audio(&self.client, &track.uri, need_audio, &|| false) {
                Ok(prepared) => {
                    self.finish_track_load(active_id, prepared);
                    return Ok(());
                }
                Err(e) => {
                    warn!("skip unplayable track: {e}");
                    let stop_retry = e.code == ErrorCode::Internal || e.code == ErrorCode::SignedOut || track_uri.is_some();
                    if stop_retry {
                        self.fail_track_load(active_id);
                        return Err(e);
                    }
                    match self.advance_next_after_failure(attempts, context_href.clone()) {
                        Ok(SwitchPlan::Load { load_id: next_id, track: next_track, attempts: next_attempts, .. }) => {
                            active_id = next_id;
                            track = next_track;
                            attempts = next_attempts;
                        }
                        Ok(SwitchPlan::Paused) => return Ok(()),
                        Err(_) => {
                            self.fail_track_load(active_id);
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    /// Toggles play / pause.
    pub fn toggle(&mut self) -> Result<(), EngineError> {
        if self.playing {
            self.pause()
        } else {
            self.play()
        }
    }

    /// Resumes playback.
    pub fn play(&mut self) -> Result<(), EngineError> {
        if self.current_track.is_none() && !self.loading {
            return Ok(());
        }
        if let Some(ref st) = self.stream_state {
            if !self.loading {
                st.player.play();
            }
        }
        self.playing = true;
        if !self.loading {
            self.play_started_at = Some(Instant::now());
        }
        Ok(())
    }

    /// Pauses playback.
    pub fn pause(&mut self) -> Result<(), EngineError> {
        self.position_ms = self.position_ms();
        self.play_started_at = None;
        self.playing = false;
        if let Some(ref st) = self.stream_state {
            st.player.pause();
        }
        Ok(())
    }

    /// Computes and begins loading the next track in the queue.
    pub fn prepare_next(&mut self) -> Result<SwitchPlan, EngineError> {
        if self.queue.is_empty() {
            return Ok(SwitchPlan::Paused);
        }
        let len = self.queue.len();
        let start_next = if self.shuffle {
            self.next_shuffle_index
                .take()
                .filter(|&idx| idx < len && (len == 1 || idx != self.queue_index))
                .unwrap_or_else(|| (self.queue_index + 1 + (now_millis() as usize % (len.max(2) - 1))) % len)
        } else if self.queue_index + 1 < len {
            self.queue_index + 1
        } else if self.repeat == Repeat::Context {
            0
        } else {
            self.pause()?;
            return Ok(SwitchPlan::Paused);
        };

        self.find_next_playable_load(start_next, 0, self.context_href.clone())
    }

    /// Advances to the next candidate track after an unplayable track failed to load.
    pub fn advance_next_after_failure(&mut self, prev_attempts: usize, context_href: String) -> Result<SwitchPlan, EngineError> {
        if self.queue.is_empty() {
            self.pause()?;
            return Err(EngineError::new(ErrorCode::NotFound, "列表中没有可播放的歌曲（可能需要 VIP）"));
        }
        let len = self.queue.len();
        let next_idx = (self.queue_index + 1) % len;
        let next_attempts = prev_attempts + 1;
        if next_attempts >= len || (next_idx == 0 && self.repeat != Repeat::Context && !self.shuffle) {
            self.pause()?;
            return Err(EngineError::new(ErrorCode::NotFound, "列表中没有可播放的歌曲（可能需要 VIP）"));
        }
        self.find_next_playable_load(next_idx, next_attempts, context_href)
    }

    fn find_next_playable_load(
        &mut self,
        start_idx: usize,
        start_attempts: usize,
        context_href: String,
    ) -> Result<SwitchPlan, EngineError> {
        let len = self.queue.len();
        let has_marked_playable = self.queue.iter().any(|t| t.playable);
        let mut idx = start_idx;
        let mut attempts = start_attempts;
        while attempts < len {
            if has_marked_playable && !self.queue[idx].playable {
                idx = (idx + 1) % len;
                attempts += 1;
                if idx == 0 && self.repeat != Repeat::Context && !self.shuffle {
                    break;
                }
                continue;
            }
            self.queue_index = idx;
            let track = self.queue[idx].clone();
            return Ok(self.begin_track_load(track, context_href, attempts));
        }
        self.pause()?;
        Err(EngineError::new(ErrorCode::NotFound, "列表中没有可播放的歌曲（可能需要 VIP）"))
    }

    /// Prepares going to the previous track or rewinding to the beginning.
    pub fn prepare_previous(&mut self) -> Result<SwitchPlan, EngineError> {
        if self.position_ms() > 3000.0 {
            self.seek(0.0)?;
            return Ok(SwitchPlan::Paused);
        }
        if self.queue.is_empty() {
            return Ok(SwitchPlan::Paused);
        }
        let prev_idx = if self.queue_index > 0 {
            self.queue_index - 1
        } else if self.repeat == Repeat::Context {
            self.queue.len() - 1
        } else {
            0
        };

        self.queue_index = prev_idx;
        let track = self.queue[prev_idx].clone();
        Ok(self.begin_track_load(track, self.context_href.clone(), 0))
    }

    /// Seeks to a position in milliseconds.
    pub fn seek(&mut self, ms: f64) -> Result<(), EngineError> {
        let Some(track) = &self.current_track else {
            return Ok(());
        };
        if !ms.is_finite() {
            return Err(EngineError::new(ErrorCode::Shape, "无效的播放位置"));
        }
        let ms = ms.clamp(0.0, track.duration_ms.max(0.0));
        if self.loading {
            self.pending_seek_ms = Some(ms);
            self.position_ms = ms;
            return Ok(());
        }
        let dur = Duration::from_millis(ms as u64);
        if let Some(ref st) = self.stream_state {
            st.player
                .try_seek(dur)
                .map_err(|e| EngineError::new(ErrorCode::Internal, format!("无法调整播放进度: {e}")))?;
        }
        self.position_ms = ms;
        if self.playing {
            self.play_started_at = Some(Instant::now());
        }
        Ok(())
    }

    /// Sets volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f64) -> Result<(), EngineError> {
        let vol = volume.clamp(0.0, 1.0);
        self.volume = vol;
        self.muted = vol == 0.0;
        if let Some(ref st) = self.stream_state {
            st.player.set_volume(if self.muted { 0.0 } else { vol as f32 });
        }
        Ok(())
    }

    /// Sets muted state.
    pub fn set_muted(&mut self, muted: bool) -> Result<(), EngineError> {
        self.muted = muted;
        if let Some(ref st) = self.stream_state {
            st.player.set_volume(if muted { 0.0 } else { self.volume as f32 });
        }
        Ok(())
    }

    /// Sets shuffle mode.
    pub fn set_shuffle(&mut self, on: bool) -> Result<(), EngineError> {
        self.shuffle = on;
        if !on {
            self.next_shuffle_index = None;
        }
        Ok(())
    }

    /// Sets repeat mode.
    pub fn set_repeat(&mut self, mode: Repeat) -> Result<(), EngineError> {
        self.repeat = mode;
        Ok(())
    }

    /// Sets the preferred audio quality and reloads the current track if one is loaded.
    pub fn set_quality(&mut self, quality: AudioQuality) -> Option<SwitchPlan> {
        if !self.client.set_quality(quality) {
            return None;
        }
        let track = self.current_track.clone()?;
        let pos = self.position_ms();
        let was_playing = self.playing;
        let plan = self.begin_track_load(track, self.context_href.clone(), 0);
        self.playing = was_playing;
        if pos > 500.0 {
            self.pending_seek_ms = Some(pos);
            self.position_ms = pos;
        }
        Some(plan)
    }

    /// Marks the current track as liked / unliked.
    pub fn set_liked(&mut self, on: bool) -> Result<(), EngineError> {
        if let Some(ref t) = self.current_track {
            self.client.set_liked(&t.uri, on)?;
        }
        self.liked = on;
        Ok(())
    }

    /// Ticks the player state, reporting whether playback ended or repeated.
    pub fn tick(&mut self) -> TickAction {
        if !self.playing || self.loading {
            return TickAction::None;
        }
        let ended = if let Some(ref st) = self.stream_state {
            st.player.empty()
        } else if let Some(ref t) = self.current_track {
            self.position_ms() >= t.duration_ms && t.duration_ms > 0.0
        } else {
            false
        };

        if ended {
            if self.repeat == Repeat::Track {
                return TickAction::ReplayCurrent;
            }
            return TickAction::Ended;
        }
        TickAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metered_audio_seeks_forward_and_backward() {
        // One second of 8 kHz mono PCM, with a different level in each half.
        let mut wav = Vec::from(&b"RIFF\xA4\x3E\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1F\x00\x00\x80\x3E\x00\x00\x02\x00\x10\x00data\x80\x3E\x00\x00"[..]);
        for sample in [8_192_i16, -8_192] {
            for _ in 0..4_000 {
                wav.extend_from_slice(&sample.to_le_bytes());
            }
        }
        let mut source = decode_audio(wav).unwrap();

        assert!(source.next().unwrap() > 0.0);
        source.try_seek(Duration::from_millis(750)).unwrap();
        assert!(source.next().unwrap() < 0.0);
        source.try_seek(Duration::from_millis(250)).unwrap();
        assert!(source.next().unwrap() > 0.0);
    }

    #[test]
    fn audio_body_reader_allows_tracks_larger_than_default_limit() {
        use std::io::Read;

        let eleven_mb = 11 * 1024 * 1024;

        // Verify that a 10MB limit produces the exact error reported by the user:
        let limited_body = ureq::Body::builder().reader(std::io::repeat(42).take(eleven_mb));
        let err = limited_body
            .into_with_config()
            .limit(10 * 1024 * 1024)
            .read_to_vec()
            .unwrap_err();
        assert!(err.to_string().contains("the response body is larger than request limit: 10485760"));

        // Verify that with MAX_AUDIO_BYTES, large tracks read successfully:
        let audio_body = ureq::Body::builder().reader(std::io::repeat(42).take(eleven_mb));
        let bytes = audio_body
            .into_with_config()
            .limit(MAX_AUDIO_BYTES)
            .read_to_vec()
            .expect("should read audio bodies larger than 10MB");

        assert_eq!(bytes.len(), eleven_mb as usize);
    }

    #[test]
    fn loading_state_defers_seek_prevents_premature_tick_and_reuses_queue() {
        let data = std::env::temp_dir().join(format!("nsh-music-player-loading-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        let client = Arc::new(NetEaseClient::new(&data));
        let mut manager = PlayerManager::new(client);

        let tracks = vec![
            Track {
                uri: "netease:track:1".to_string(),
                name: "One".to_string(),
                duration_ms: 1000.0,
                playable: true,
                ..Track::default()
            },
            Track {
                uri: "netease:track:2".to_string(),
                name: "Two".to_string(),
                duration_ms: 2000.0,
                playable: false,
                ..Track::default()
            },
            Track {
                uri: "netease:track:3".to_string(),
                name: "Three".to_string(),
                duration_ms: 3000.0,
                playable: true,
                ..Track::default()
            },
        ];

        let plan = manager
            .prepare_context("netease:playlist:99", Some("netease:track:1"), tracks)
            .unwrap();
        let SwitchPlan::Load { load_id, track, .. } = plan else {
            panic!("expected SwitchPlan::Load");
        };
        assert_eq!(track.uri, "netease:track:1");
        assert!(manager.loading);
        assert!(manager.playing);
        // While loading, tick must not treat an empty sink or zero progress as ended.
        assert_eq!(manager.tick(), TickAction::None);

        // Seeking while loading records a pending seek instead of failing.
        manager.seek(500.0).unwrap();
        assert_eq!(manager.position_ms(), 500.0);

        // Finishing the load clears loading and preserves pending seek position.
        assert!(manager.finish_track_load(
            load_id,
            PreparedAudio {
                source: None,
                cancel: None,
            }
        ));
        assert!(!manager.loading);
        assert_eq!(manager.pending_seek_ms, None);

        // Reusable queue matches current context without network re-fetch.
        assert!(manager.reusable_queue("netease:playlist:99", Some("netease:track:3")).is_some());
        assert!(manager.reusable_queue("netease:playlist:99", Some("netease:track:999")).is_none());

        // Upcoming track skips unplayable track 2 and picks track 3 for prefetch.
        assert_eq!(manager.peek_upcoming_track_uri(), Some("netease:track:3".to_string()));

        // Next skips unplayable track 2 and immediately selects track 3.
        let next_plan = manager.prepare_next().unwrap();
        let SwitchPlan::Load { track: next_track, .. } = next_plan else {
            panic!("expected next track load");
        };
        assert_eq!(next_track.uri, "netease:track:3");

        let _ = std::fs::remove_dir_all(data);
    }
}
