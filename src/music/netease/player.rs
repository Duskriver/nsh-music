//! Native audio player for NetEase Cloud Music using rodio and audio sample tapping.

use std::{
    io::Cursor,
    num::{NonZeroU16, NonZeroU32},
    sync::{Arc, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use log::warn;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player as RodioPlayer, Source, source::SeekError};

use super::client::NetEaseClient;
use crate::music::model::{EngineError, ErrorCode, Player, Repeat, Track};

static AUDIO_TAP: OnceLock<tokio::sync::broadcast::Sender<f32>> = OnceLock::new();

/// Returns the global broadcast channel sender for audio samples.
pub fn get_audio_tap() -> &'static tokio::sync::broadcast::Sender<f32> {
    AUDIO_TAP.get_or_init(|| {
        let (tx, _rx) = tokio::sync::broadcast::channel(8192);
        tx
    })
}

fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// A wrapper around a rodio Source that copies mono samples to the audio tap.
struct MeteredSource<S> {
    inner: S,
    channels: NonZeroU16,
    sample_rate: NonZeroU32,
    channel_idx: u16,
    left_sample: Option<f32>,
}

fn decode_audio(bytes: Vec<u8>) -> Result<MeteredSource<Decoder<Cursor<Vec<u8>>>>, EngineError> {
    let decoder = Decoder::builder()
        .with_byte_len(bytes.len() as u64)
        .with_seekable(true)
        .with_data(Cursor::new(bytes))
        .build()
        .map_err(|e| EngineError::new(ErrorCode::Shape, format!("音频解码失败: {e}")))?;
    Ok(MeteredSource {
        channels: decoder.channels(),
        sample_rate: decoder.sample_rate(),
        inner: decoder,
        channel_idx: 0,
        left_sample: None,
    })
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
        // from before the jump instead of mixing it with the new position.
        self.left_sample = None;
        Ok(())
    }
}

impl<S: Source<Item = f32>> Iterator for MeteredSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let tap = get_audio_tap();
        let ch = self.channels.get();
        if ch == 1 {
            let _ = tap.send(sample);
        } else if ch == 2 {
            if self.channel_idx == 0 {
                self.left_sample = Some(sample);
                self.channel_idx = 1;
            } else {
                if let Some(left) = self.left_sample.take() {
                    let _ = tap.send((left + sample) * 0.5);
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

/// Native player manager.
pub struct PlayerManager {
    client: Arc<NetEaseClient>,
    stream_state: Option<StreamState>,
    queue: Vec<Track>,
    queue_index: usize,
    current_track: Option<Track>,
    context_href: String,
    playing: bool,
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
            queue: Vec::new(),
            queue_index: 0,
            current_track: None,
            context_href: String::new(),
            playing: false,
            volume: 1.0,
            muted: false,
            shuffle: false,
            repeat: Repeat::Off,
            liked: false,
            position_ms: 0.0,
            play_started_at: None,
        }
    }

    /// Current playback position in milliseconds.
    pub fn position_ms(&self) -> f64 {
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
            volume: self.volume,
            muted: self.muted,
            liked: self.liked,
            can_next,
            can_previous,
        }
    }

    /// Plays a given track now.
    pub fn play_track(&mut self, track: Track, context_href: String) -> Result<(), EngineError> {
        let stream_url = self.client.stream_url(&track.uri)?;

        // Download audio bytes
        let resp = ureq::get(&stream_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15")
            .header("Referer", "https://music.163.com/")
            .call()
            .map_err(|e| EngineError::new(ErrorCode::NotFound, format!("failed to stream audio: {e}")))?;
        let bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("failed to read audio body: {e}")))?;

        if let Some(ref mut st) = self.stream_state {
            let metered = decode_audio(bytes)?;
            st.player.stop();
            st.player.append(metered);
            st.player.set_volume(if self.muted { 0.0 } else { self.volume as f32 });
            st.player.play();
        }

        self.current_track = Some(track);
        self.context_href = context_href;
        self.playing = true;
        self.position_ms = 0.0;
        self.play_started_at = Some(Instant::now());
        Ok(())
    }

    /// Plays a context (playlist, album, artist, liked songs), starting from `track_uri`.
    pub fn play_context(
        &mut self,
        context_uri: &str,
        track_uri: Option<&str>,
        tracks: Vec<Track>,
    ) -> Result<(), EngineError> {
        if tracks.is_empty() {
            return Err(EngineError::new(ErrorCode::NotFound, "列表为空，没有可播放的歌曲"));
        }

        let context_href = if context_uri == "netease:collection:tracks" {
            "/collection/tracks".to_string()
        } else if let Some(id) = context_uri.strip_prefix("netease:playlist:") {
            format!("/playlist/{id}")
        } else if let Some(id) = context_uri.strip_prefix("netease:album:") {
            format!("/album/{id}")
        } else if let Some(id) = context_uri.strip_prefix("netease:artist:") {
            format!("/artist/{id}")
        } else {
            context_uri.to_string()
        };

        let start_index = if let Some(t_uri) = track_uri {
            tracks.iter().position(|t| t.uri == t_uri).unwrap_or(0)
        } else {
            0
        };

        self.queue = tracks;
        self.queue_index = start_index;

        if track_uri.is_some() {
            let track_to_play = self.queue[start_index].clone();
            self.play_track(track_to_play, context_href)
        } else {
            // Find the first playable track
            let has_marked_playable = self.queue.iter().skip(start_index).any(|t| t.playable);
            let mut last_err = None;
            for idx in start_index..self.queue.len() {
                if has_marked_playable && !self.queue[idx].playable {
                    continue;
                }
                self.queue_index = idx;
                let track = self.queue[idx].clone();
                match self.play_track(track, context_href.clone()) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        warn!("skip unplayable track: {e}");
                        let stop_retry = e.code == ErrorCode::Internal || e.code == ErrorCode::SignedOut;
                        last_err = Some(e);
                        if stop_retry {
                            break;
                        }
                    }
                }
            }
            Err(last_err.unwrap_or_else(|| EngineError::new(ErrorCode::NotFound, "没有可播放的歌曲（可能需要 VIP）")))
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
        if self.current_track.is_none() {
            if let Some(first) = self.queue.first().cloned() {
                return self.play_track(first, self.context_href.clone());
            }
            return Ok(());
        }
        if let Some(ref st) = self.stream_state {
            st.player.play();
        }
        self.playing = true;
        self.play_started_at = Some(Instant::now());
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

    /// Skips to next track.
    pub fn next(&mut self) -> Result<(), EngineError> {
        if self.queue.is_empty() {
            return Ok(());
        }
        let len = self.queue.len();
        let start_next = if self.shuffle {
            (self.queue_index + 1 + (now_millis() as usize % (len.max(2) - 1))) % len
        } else if self.queue_index + 1 < len {
            self.queue_index + 1
        } else if self.repeat == Repeat::Context {
            0
        } else {
            return self.pause();
        };

        // Skip unplayable tracks, but stop immediately on risk-control or session errors.
        let has_marked_playable = self.queue.iter().any(|t| t.playable);
        let mut idx = start_next;
        let mut attempts = 0;
        let mut last_err = None;
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
            match self.play_track(track, self.context_href.clone()) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("failed to play track {} in queue: {e}", self.queue[idx].name);
                    let stop_retry = e.code == ErrorCode::Internal || e.code == ErrorCode::SignedOut;
                    last_err = Some(e);
                    if stop_retry {
                        break;
                    }
                    idx = (idx + 1) % len;
                    attempts += 1;
                    if idx == 0 && self.repeat != Repeat::Context && !self.shuffle {
                        break;
                    }
                }
            }
        }
        self.pause()?;
        Err(last_err.unwrap_or_else(|| EngineError::new(ErrorCode::NotFound, "列表中没有可播放的歌曲（可能需要 VIP）")))
    }

    /// Goes to previous track or rewinds to beginning.
    pub fn previous(&mut self) -> Result<(), EngineError> {
        if self.position_ms() > 3000.0 {
            return self.seek(0.0);
        }
        if self.queue.is_empty() {
            return Ok(());
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
        self.play_track(track, self.context_href.clone())
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
        Ok(())
    }

    /// Sets repeat mode.
    pub fn set_repeat(&mut self, mode: Repeat) -> Result<(), EngineError> {
        self.repeat = mode;
        Ok(())
    }

    /// Marks the current track as liked / unliked.
    pub fn set_liked(&mut self, on: bool) -> Result<(), EngineError> {
        self.liked = on;
        if let Some(ref t) = self.current_track {
            self.client.set_liked(&t.uri, on)?;
        }
        Ok(())
    }

    /// Ticks the player state, advancing track if finished.
    pub fn tick(&mut self) -> bool {
        if !self.playing {
            return false;
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
                let _ = self.seek(0.0);
                return true;
            }
            let _ = self.next();
            return true;
        }
        false
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
}
