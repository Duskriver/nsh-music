//! The live audio meter reads PCM samples from the native player and publishes
//! loudness, a 16-band spectrum, and a beat pulse for the music visuals.

use std::{cell::RefCell, rc::Rc, time::Duration};

use nsh_core::prelude::*;
use tokio::sync::{oneshot, watch};

/// One moment of the music, every value from 0 to 1. Quiet and loud tracks
/// move alike: each value is measured against what was loud a few seconds
/// before.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct AudioFrame {
    /// Loudness (smoothed RMS).
    pub level: f64,
    /// Bass, 20–250 Hz.
    pub bass: f64,
    /// Mids, 250 Hz–4 kHz.
    pub mid: f64,
    /// Treble, 4–16 kHz.
    pub treble: f64,
    /// A pulse: 1 on a beat (an onset in the bass), falling to 0 over about
    /// 200 ms.
    pub beat: f64,
    /// The spectrum: 16 bands, log-spaced from 40 Hz to 16 kHz, lowest
    /// first.
    pub bands: Vec<f64>,
}

impl AudioFrame {
    /// Silence: every value 0.
    pub fn zero() -> Self {
        AudioFrame { bands: vec![0.0; BANDS], ..AudioFrame::default() }
    }
}

/// The number of spectrum bands in a frame.
pub const BANDS: usize = 16;

#[derive(Default)]
struct Inner {
    next: u64,
    listeners: Vec<(u64, Callback<AudioFrame>)>,
    stop: Option<oneshot::Sender<()>>,
}

/// A live meter of the music nsh-music plays: loudness, bass, mids, treble, a
/// spectrum and a beat pulse, about 60 frames a second while sound plays.
#[class]
pub struct AudioMeter {
    inner: Rc<RefCell<Inner>>,
}

impl Default for AudioMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[class]
impl AudioMeter {
    /// Makes a meter; it records nothing until `run()`.
    pub fn new() -> Self {
        AudioMeter { inner: Rc::new(RefCell::new(Inner::default())) }
    }

    /// Whether the native PCM tap is supported.
    #[getter]
    pub fn available(&self) -> bool {
        true
    }

    /// Calls `listener` with each frame (at most about 60 a second; one
    /// zero frame after a second of silence, then none until sound
    /// returns). Call `run()` once to start.
    pub fn on_frame(&self, #[args(frame)] listener: Callback<AudioFrame>) -> Stop {
        let inner = self.inner.clone();
        let id = {
            let mut w = inner.borrow_mut();
            let id = w.next;
            w.next += 1;
            w.listeners.push((id, listener));
            id
        };
        Stop::new(move || inner.borrow_mut().listeners.retain(|(i, _)| *i != id))
    }

    /// Starts analyzing samples and delivers frames to the `onFrame` listeners;
    /// resolves when the meter stops.
    /// Call it once.
    pub async fn run(this: Instance<Self>) {
        let inner = this.with(|s| s.inner.clone());
        // Disposing the meter drops the stop sender and closes the frame channel.
        if inner.borrow().stop.is_some() {
            return;
        }
        let (stop, mut stopped) = oneshot::channel();
        inner.borrow_mut().stop = Some(stop);
        drop(this);

        let (tx, mut rx) = watch::channel(None);
        if let Err(error) = std::thread::Builder::new()
            .name("nsh-music-meter".into())
            .spawn(move || meter_samples(tx))
        {
            log::warn!("could not start the audio meter: {error}");
            inner.borrow_mut().stop = None;
            return;
        }

        loop {
            tokio::select! {
                _ = &mut stopped => break,
                changed = rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
            if let Some(frame) = rx.borrow_and_update().as_ref() {
                for (_, cb) in &inner.borrow().listeners {
                    cb.post(frame.clone());
                }
            }
        }
    }

    /// Stops analyzing samples; `run()` resolves. Disposing the meter does the same.
    pub fn stop(&self) {

    }
}

impl Drop for AudioMeter {
    fn drop(&mut self) {
        self.stop();
    }
}

fn meter_samples(tx: watch::Sender<Option<AudioFrame>>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_time().build() else {
        return;
    };
    runtime.block_on(async {
        use tokio::sync::broadcast::error::RecvError;

        let mut samples = crate::music::netease::get_audio_tap().subscribe();
        let mut analyzer = analysis::Analyzer::new();
        let mut emitter = analysis::Emitter::new();
        loop {
            let sample = tokio::select! {
                _ = tx.closed() => break,
                sample = tokio::time::timeout(Duration::from_secs(1), samples.recv()) => sample,
            };
            match sample {
                Ok(Ok(sample)) => analyzer.push(&[sample], 1, |levels| {
                    if let Some(frame) = emitter.step(levels) {
                        tx.send_replace(Some(frame));
                    }
                }),
                Ok(Err(RecvError::Lagged(_))) => continue,
                Ok(Err(RecvError::Closed)) => break,
                Err(_) => {
                    if let Some(frame) = emitter.silence() {
                        tx.send_replace(Some(frame));
                    }
                }
            }
        }
    });
}

/// The analysis: mono samples in, smoothed and auto-gained frames out.
pub mod analysis {
    use std::{collections::VecDeque, sync::Arc};

    use rustfft::{Fft, FftPlanner, num_complex::Complex};

    use super::{AudioFrame, BANDS};

    /// The rate the meter records at.
    pub const SAMPLE_RATE: f32 = 48_000.0;
    /// FFT window length (21 ms at 48 kHz).
    pub const FFT_SIZE: usize = 1024;
    /// Samples between two analyses: 120 a second, every other one sent
    /// (60 a second).
    pub const HOP: usize = 400;

    /// Below this window RMS (-80 dBFS) it is silence.
    const GATE: f32 = 1e-4;
    /// The dynamic range a value spans, in dB below its reference.
    const RANGE_DB: f32 = 36.0;
    const LEVEL_RANGE_DB: f32 = 30.0;
    /// How far below the loudest band a band's reference may fall: a band
    /// this much quieter than the rest is not raised to full scale.
    const SPREAD_DB: f32 = 12.0;
    /// The lowest a reference goes, so faint noise is not raised to full
    /// scale. The bands' is low: the tilt takes up to 14 dB off the bass.
    const FLOOR_DB: f32 = -80.0;
    /// The lowest loudness reference: a track at -60 dBFS RMS fills it.
    const LEVEL_FLOOR_DB: f32 = -60.0;
    /// How fast references fall (the auto-gain's release), dB per second.
    const DECAY_DB_PER_S: f32 = 4.0;
    /// Smoothing time constants, seconds.
    const ATTACK_S: f32 = 0.03;
    const RELEASE_S: f32 = 0.25;
    /// The beat pulse falls from 1 to 0 in this long.
    const BEAT_FALL_S: f32 = 0.2;
    /// No second onset sooner than this.
    const REFRACTORY_S: f32 = 0.12;
    /// The onset threshold: the flux's mean plus this many standard
    /// deviations over the last second, and at least `FLUX_MIN`.
    const FLUX_K: f32 = 1.5;
    const FLUX_MIN: f32 = 1.0;

    const BAND_LOW_HZ: f32 = 40.0;
    const BAND_HIGH_HZ: f32 = 16_000.0;
    /// Bass, mids, treble.
    const GROUPS: [(f32, f32); 3] = [(20.0, 250.0), (250.0, 4_000.0), (4_000.0, 16_000.0)];
    /// The bins the beat detector watches (about 45–190 Hz: kicks, bass).
    const BEAT_BINS: std::ops::Range<usize> = 1..5;

    /// A run of bins and its pink-noise tilt (music's energy falls with
    /// frequency; the tilt evens the bands out).
    #[derive(Clone, Copy)]
    struct Span {
        lo: usize,
        hi: usize,
        tilt: f32,
    }

    impl Span {
        fn new(lo_hz: f32, hi_hz: f32) -> Self {
            let bin = SAMPLE_RATE / FFT_SIZE as f32;
            let lo = ((lo_hz / bin).round() as usize).max(1);
            let hi = ((hi_hz / bin).round() as usize).clamp(lo + 1, FFT_SIZE / 2 + 1);
            let center = (lo_hz * hi_hz).sqrt();
            Span { lo, hi, tilt: (center / 1000.0).sqrt() }
        }

        /// The span's amplitude in dBFS (RMS of its bins, tilted).
        fn db(&self, mags: &[f32]) -> f32 {
            let bins = &mags[self.lo..self.hi];
            let power = bins.iter().map(|m| m * m).sum::<f32>() / bins.len() as f32;
            db(power.sqrt() * self.tilt)
        }
    }

    fn db(amplitude: f32) -> f32 {
        20.0 * amplitude.max(1e-12).log10()
    }

    /// A value measured against a slowly falling reference (the auto-gain).
    #[derive(Clone, Copy)]
    struct Gain {
        reference: f32,
    }

    impl Gain {
        const fn new() -> Self {
            Gain { reference: FLOOR_DB }
        }

        /// Lowers the reference by `decay` dB, raises it to `db`, and
        /// gives `db` on a 0..1 scale `range` dB below `max(reference,
        /// least)`.
        fn measure(&mut self, db: f32, decay: f32, least: f32, range: f32) -> f32 {
            self.reference = (self.reference - decay).max(db).max(FLOOR_DB);
            let top = self.reference.max(least);
            ((db - (top - range)) / range).clamp(0.0, 1.0)
        }
    }

    /// Fast attack, slow release.
    #[derive(Clone, Copy)]
    struct Smooth {
        attack: f32,
        release: f32,
    }

    impl Smooth {
        fn toward(&self, current: f32, target: f32) -> f32 {
            let k = if target > current { self.attack } else { self.release };
            current + (target - current) * k
        }
    }

    /// One analysis step's result.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Levels {
        pub level: f32,
        pub bass: f32,
        pub mid: f32,
        pub treble: f32,
        pub beat: f32,
        pub bands: [f32; BANDS],
        /// The window held sound (above the gate).
        pub loud: bool,
        /// A beat started in this step.
        pub onset: bool,
    }

    impl Levels {
        pub fn frame(&self) -> AudioFrame {
            let r = |v: f32| (f64::from(v) * 1000.0).round() / 1000.0;
            AudioFrame {
                level: r(self.level),
                bass: r(self.bass),
                mid: r(self.mid),
                treble: r(self.treble),
                beat: r(self.beat),
                bands: self.bands.iter().map(|v| r(*v)).collect(),
            }
        }
    }

    /// The meter's analysis: feed it samples, hear a [`Levels`] every
    /// [`HOP`] samples.
    pub struct Analyzer {
        fft: Arc<dyn Fft<f32>>,
        window: Vec<f32>,
        /// Hann's sum, halved: a full-scale sine's bin reads 1.
        norm: f32,
        samples: Vec<f32>,
        buf: Vec<Complex<f32>>,
        scratch: Vec<Complex<f32>>,
        mags: Vec<f32>,
        bands: [Span; BANDS],
        groups: [Span; 3],
        band_gain: [Gain; BANDS],
        group_gain: [Gain; 3],
        level_gain: Gain,
        /// The loudest band's and group's reference (all bands' auto-gain).
        band_top: Gain,
        group_top: Gain,
        decay: f32,
        smooth: Smooth,
        out: Levels,
        prev_beat: Vec<f32>,
        flux: VecDeque<f32>,
        since_onset: usize,
    }

    impl Default for Analyzer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Analyzer {
        pub fn new() -> Self {
            let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
            let window: Vec<f32> = (0..FFT_SIZE)
                .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
                .collect();
            let norm = window.iter().sum::<f32>() / 2.0;
            let ratio = (BAND_HIGH_HZ / BAND_LOW_HZ).powf(1.0 / BANDS as f32);
            let bands = std::array::from_fn(|i| {
                let lo = BAND_LOW_HZ * ratio.powi(i as i32);
                Span::new(lo, lo * ratio)
            });
            let groups = GROUPS.map(|(lo, hi)| Span::new(lo, hi));
            let hop_s = HOP as f32 / SAMPLE_RATE;
            let coefficient = |tau: f32| 1.0 - (-hop_s / tau).exp();
            let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
            Analyzer {
                fft,
                window,
                norm,
                samples: Vec::with_capacity(FFT_SIZE + HOP * 8),
                buf: vec![Complex::default(); FFT_SIZE],
                scratch,
                mags: vec![0.0; FFT_SIZE / 2 + 1],
                bands,
                groups,
                band_gain: [Gain::new(); BANDS],
                group_gain: [Gain::new(); 3],
                level_gain: Gain::new(),
                band_top: Gain::new(),
                group_top: Gain::new(),
                decay: DECAY_DB_PER_S * hop_s,
                smooth: Smooth { attack: coefficient(ATTACK_S), release: coefficient(RELEASE_S) },
                out: Levels { level: 0.0, bass: 0.0, mid: 0.0, treble: 0.0, beat: 0.0, bands: [0.0; BANDS], loud: false, onset: false },
                prev_beat: vec![0.0; BEAT_BINS.len()],
                flux: VecDeque::with_capacity(Self::FLUX_HISTORY),
                since_onset: usize::MAX,
            }
        }

        /// One second of flux values.
        const FLUX_HISTORY: usize = (SAMPLE_RATE as usize) / HOP;

        /// Feeds interleaved samples of `channels` channels (mixed to
        /// mono); calls `step` after each hop.
        pub fn push(&mut self, interleaved: &[f32], channels: usize, mut step: impl FnMut(&Levels)) {
            let channels = channels.max(1);
            let scale = 1.0 / channels as f32;
            for chunk in interleaved.chunks_exact(channels) {
                let s: f32 = chunk.iter().sum::<f32>() * scale;
                self.samples.push(if s.is_finite() { s } else { 0.0 });
                if self.samples.len() == FFT_SIZE {
                    self.analyze();
                    step(&self.out);
                    self.samples.drain(..HOP);
                }
            }
        }

        fn analyze(&mut self) {
            let x = &self.samples[..FFT_SIZE];
            let rms = (x.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
            let loud = rms >= GATE;

            for ((b, s), w) in self.buf.iter_mut().zip(x).zip(&self.window) {
                *b = Complex::new(s * w, 0.0);
            }
            self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);
            for (m, b) in self.mags.iter_mut().zip(&self.buf) {
                *m = b.norm() / self.norm;
            }

            // Spectrum and groups, each against its own reference but never
            // more than SPREAD below the loudest one's.
            let decay = self.decay;
            let mut band_db = [0.0f32; BANDS];
            for (d, span) in band_db.iter_mut().zip(&self.bands) {
                *d = span.db(&self.mags);
            }
            let group_db = self.groups.map(|span| span.db(&self.mags));
            let loudest = |dbs: &[f32]| dbs.iter().copied().fold(f32::MIN, f32::max);
            self.band_top.measure(loudest(&band_db), decay, FLOOR_DB, RANGE_DB);
            self.group_top.measure(loudest(&group_db), decay, FLOOR_DB, RANGE_DB);
            let band_least = self.band_top.reference - SPREAD_DB;
            let group_least = self.group_top.reference - SPREAD_DB;
            let mut bands = [0.0f32; BANDS];
            for ((v, gain), d) in bands.iter_mut().zip(&mut self.band_gain).zip(band_db) {
                *v = gain.measure(d, decay, band_least, RANGE_DB);
            }
            let mut groups = [0.0f32; 3];
            for ((v, gain), d) in groups.iter_mut().zip(&mut self.group_gain).zip(group_db) {
                *v = gain.measure(d, decay, group_least, RANGE_DB);
            }
            let level = self.level_gain.measure(db(rms), decay, LEVEL_FLOOR_DB, LEVEL_RANGE_DB);

            // Spectral flux on the bass bins (log magnitudes, so it is the
            // rise in dB whatever the volume), against an adaptive
            // threshold.
            let mut flux = 0.0;
            for (prev, m) in self.prev_beat.iter_mut().zip(&self.mags[BEAT_BINS]) {
                let now = (1.0 + m * 1e4).ln();
                flux += (now - *prev).max(0.0);
                *prev = now;
            }
            let n = self.flux.len().max(1) as f32;
            let mean = self.flux.iter().sum::<f32>() / n;
            let var = self.flux.iter().map(|f| (f - mean) * (f - mean)).sum::<f32>() / n;
            let threshold = (mean + FLUX_K * var.sqrt()).max(FLUX_MIN);
            if self.flux.len() == Self::FLUX_HISTORY {
                self.flux.pop_front();
            }
            self.flux.push_back(flux);
            let hop_s = HOP as f32 / SAMPLE_RATE;
            let refractory = (REFRACTORY_S / hop_s).ceil() as usize;
            self.since_onset = self.since_onset.saturating_add(1);
            let onset = loud && flux > threshold && self.since_onset >= refractory;
            if onset {
                self.since_onset = 0;
            }

            let s = self.smooth;
            let o = &mut self.out;
            let gate = |v: f32| if loud { v } else { 0.0 };
            o.level = s.toward(o.level, gate(level));
            o.bass = s.toward(o.bass, gate(groups[0]));
            o.mid = s.toward(o.mid, gate(groups[1]));
            o.treble = s.toward(o.treble, gate(groups[2]));
            for (v, t) in o.bands.iter_mut().zip(bands) {
                *v = s.toward(*v, gate(t));
            }
            o.beat = if onset { 1.0 } else { (o.beat - hop_s / BEAT_FALL_S).max(0.0) };
            o.loud = loud;
            o.onset = onset;
        }
    }

    /// Picks which steps reach JS: every other one (60 a second), and,
    /// after a second without sound, one zero frame and then none until
    /// sound returns.
    pub struct Emitter {
        step: usize,
        quiet: usize,
        paused: bool,
    }

    impl Default for Emitter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Emitter {
        /// Steps of silence before pausing (1 s).
        const QUIET_STEPS: usize = (SAMPLE_RATE as usize) / HOP;

        pub fn new() -> Self {
            // Paused until the first sound: nothing to say before it.
            Emitter { step: 0, quiet: 0, paused: true }
        }

        /// What to send for this step, if anything.
        pub fn step(&mut self, levels: &Levels) -> Option<AudioFrame> {
            if levels.loud {
                self.quiet = 0;
                self.paused = false;
            } else {
                self.quiet += 1;
                if self.quiet >= Self::QUIET_STEPS {
                    return self.silence();
                }
            }
            if self.paused {
                return None;
            }
            self.step += 1;
            // An onset is always sent, so no beat is lost to the halving.
            (self.step.is_multiple_of(2) || levels.onset).then(|| levels.frame())
        }

        /// No samples arrive (a corked or suspended stream): the same as
        /// silence. One zero frame, if not yet sent.
        pub fn silence(&mut self) -> Option<AudioFrame> {
            if self.paused {
                return None;
            }
            self.paused = true;
            Some(AudioFrame::zero())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::analysis::*;

    fn run(signal: impl Fn(usize) -> f32, seconds: f32) -> Vec<Levels> {
        let n = (SAMPLE_RATE * seconds) as usize;
        let stereo: Vec<f32> = (0..n).flat_map(|i| {
            let s = signal(i);
            [s, s]
        }).collect();
        let mut a = Analyzer::new();
        let mut out = Vec::new();
        // Feed stereo audio in chunks.
        for chunk in stereo.chunks(960) {
            a.push(chunk, 2, |l| out.push(*l));
        }
        out
    }

    fn sine(hz: f32, amp: f32) -> impl Fn(usize) -> f32 {
        move |i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / SAMPLE_RATE).sin()
    }

    #[test]
    fn steps_120_times_a_second() {
        let steps = run(|_| 0.0, 1.0).len();
        assert!((117..=120).contains(&steps), "{steps}");
    }

    #[test]
    fn silence_is_zero() {
        for l in run(|_| 0.0, 1.0) {
            assert!(!l.loud);
            assert_eq!(l.frame(), crate::audio::AudioFrame::zero());
        }
    }

    #[test]
    fn a_low_sine_is_bass() {
        let last = *run(sine(60.0, 0.5), 1.0).last().unwrap();
        assert!(last.bass > 0.8, "{last:?}");
        assert!(last.mid < 0.2 && last.treble < 0.1, "{last:?}");
        assert!(last.bands[0] > 0.8 || last.bands[1] > 0.8, "{last:?}");
        assert!(last.bands[8..].iter().all(|b| *b < 0.1), "{last:?}");
        assert!(last.level > 0.8, "{last:?}");
    }

    #[test]
    fn a_high_sine_is_treble() {
        let last = *run(sine(5_000.0, 0.5), 1.0).last().unwrap();
        assert!(last.treble > 0.8, "{last:?}");
        assert!(last.bass < 0.1 && last.mid < 0.2, "{last:?}");
        // 5 kHz: band 11 (≈4.4–6.0 kHz).
        let top = (0..16).max_by(|a, b| last.bands[*a].total_cmp(&last.bands[*b])).unwrap();
        assert!((10..=12).contains(&top), "{last:?}");
        assert!(last.bands[..6].iter().all(|b| *b < 0.1), "{last:?}");
    }

    #[test]
    fn a_quiet_track_moves_like_a_loud_one() {
        let loud = *run(sine(60.0, 0.5), 1.0).last().unwrap();
        let quiet = *run(sine(60.0, 0.005), 1.0).last().unwrap();
        assert!((loud.bass - quiet.bass).abs() < 0.05, "{loud:?} {quiet:?}");
        assert!((loud.level - quiet.level).abs() < 0.05, "{loud:?} {quiet:?}");
    }

    #[test]
    fn values_rise_fast_and_fall_slowly() {
        // 0.5 s of tone, then silence.
        let steps = run(|i| if i < 24_000 { sine(60.0, 0.5)(i) } else { 0.0 }, 1.0);
        // ~30 ms in (attack): mostly there.
        assert!(steps[4].bass > 0.5, "{:?}", steps[4]);
        // ~100 ms after it ends (release ~250 ms): still well above 0.
        let after = steps.iter().position(|l| !l.loud).unwrap();
        assert!(steps[after].bass > 0.5, "{:?}", steps[after]);
        assert!(steps.last().unwrap().bass < 0.2);
    }

    #[test]
    fn clicks_are_beats() {
        // A click every 500 ms for 4 s over a soft hum (so it is never
        // silent between them).
        let hum = sine(440.0, 0.01);
        let steps = run(|i| hum(i) + if i % 24_000 == 1_000 { 0.9 } else { 0.0 }, 4.0);
        let onsets: Vec<usize> = steps.iter().enumerate().filter(|(_, l)| l.onset).map(|(i, _)| i).collect();
        assert_eq!(onsets.len(), 8, "{onsets:?}");
        // One per click, ~60 steps apart.
        for pair in onsets.windows(2) {
            assert!((58..=62).contains(&(pair[1] - pair[0])), "{onsets:?}");
        }
        // The pulse: 1 on the onset, gone 250 ms later.
        let first = onsets[0];
        assert_eq!(steps[first].beat, 1.0);
        assert!(steps[first + 12].beat > 0.3);
        assert_eq!(steps[first + 30].beat, 0.0);
    }

    #[test]
    fn a_steady_tone_has_no_beats() {
        let onsets = run(sine(60.0, 0.5), 3.0).iter().filter(|l| l.onset).count();
        assert!(onsets <= 1, "{onsets}");
    }

    #[test]
    fn the_emitter_halves_and_pauses_on_silence() {
        let mut e = Emitter::new();
        let tone = run(sine(200.0, 0.3), 1.0);
        let sent = tone.iter().filter(|l| e.step(l).is_some()).count();
        assert!((55..=62).contains(&sent), "{sent}");
        let quiet = run(|_| 0.0, 2.0);
        let frames: Vec<_> = quiet.iter().filter_map(|l| e.step(l)).collect();
        // A second's fade, then one zero frame, then nothing.
        assert!(frames.len() <= 62, "{}", frames.len());
        assert_eq!(frames.last(), Some(&crate::audio::AudioFrame::zero()));
        assert_eq!(e.silence(), None);
        // Before any sound, nothing at all.
        let mut fresh = Emitter::new();
        assert!(quiet.iter().all(|l| fresh.step(l).is_none()));
    }
}
