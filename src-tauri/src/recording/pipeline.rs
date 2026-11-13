//! Audio → text, one recording: 30 s windows with 5 s overlap per stream, whisper on the caller's
//! (worker) thread. Pure of capture and HTTP, so a WAV file can be fed through it the same way (D27).
use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub const RATE: usize = 16_000;
const WINDOW: usize = 30 * RATE;
const OVERLAP: usize = 5 * RATE;
/// Quieter than this (RMS ≈ -50 dBFS) is a muted mic or a silent call: whisper invents "Thank you." there.
const SILENCE: f32 = 0.003;
/// A tail shorter than this is a click at Stop, not speech.
const MIN_TAIL: usize = RATE;

pub const SPEAKERS: [&str; 2] = ["me", "others"];

#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    /// Seconds from the start of the recording.
    pub t: f64,
    /// Where the line ends (s): the speaker split matches it against the diarized turns; never stored.
    pub end: f64,
    /// `me`, `others`, or `Speaker N` once the split ran.
    pub speaker: String,
    pub text: String,
}

#[derive(Default)]
struct Stream {
    buf: Vec<f32>,
    /// Sample index (from the start) of `buf[0]`.
    offset: usize,
    /// End (s) of the last line sent: the overlap repeats lines, so anything starting before it is a repeat.
    sent_until: f64,
    /// Whisper sometimes says the same line twice a few seconds apart (seen at a window edge in T53).
    last_text: String,
}

pub struct Pipeline {
    // The state keeps the model alive (it holds the context); dropping the pipeline unloads both.
    state: WhisperState,
    lang: String,
    prompt: String,
    streams: [Stream; 2],
}

impl Pipeline {
    pub fn new(model: &Path, lang: &str, prompt: &str) -> Result<Self, String> {
        // whisper.cpp logs every layer to stderr; route it to the (absent) `log` backend instead.
        whisper_rs::install_logging_hooks();
        let mut cp = WhisperContextParameters::default();
        cp.flash_attn(true);
        let ctx = WhisperContext::new_with_params(model, cp).map_err(|e| e.to_string())?;
        let state = ctx.create_state().map_err(|e| e.to_string())?;
        Ok(Pipeline {
            state,
            lang: lang.to_string(),
            prompt: prompt.to_string(),
            streams: Default::default(),
        })
    }

    /// 16 kHz mono samples of one stream (0 = me, 1 = others); returns the lines of every window it filled.
    pub fn push(&mut self, who: usize, samples: &[f32]) -> Vec<Seg> {
        self.streams[who].buf.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.streams[who].buf.len() >= WINDOW {
            out.extend(self.window(who, WINDOW, false));
            let s = &mut self.streams[who];
            s.buf.drain(..WINDOW - OVERLAP);
            s.offset += WINDOW - OVERLAP;
        }
        out
    }

    /// Stop: the rest of both streams, every line kept.
    pub fn finish(&mut self) -> Vec<Seg> {
        let mut out = Vec::new();
        for who in 0..2 {
            let n = self.streams[who].buf.len();
            if n >= MIN_TAIL {
                out.extend(self.window(who, n, true));
            }
            self.streams[who].buf.clear();
        }
        out
    }

    fn window(&mut self, who: usize, len: usize, last: bool) -> Vec<Seg> {
        let s = &self.streams[who];
        let audio = &s.buf[..len];
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / len as f32).sqrt();
        if rms < SILENCE {
            return Vec::new();
        }
        let base = s.offset as f64 / RATE as f64;
        let p = params(&self.lang, &self.prompt);
        if self.state.full(p, audio).is_err() {
            return Vec::new();
        }
        // Lines starting in the last OVERLAP of a non-final window come again, whole, in the next one.
        let keep_before = base + (len - OVERLAP) as f64 / RATE as f64;
        let s = &mut self.streams[who];
        let mut out = Vec::new();
        for seg in self.state.as_iter() {
            let start = base + seg.start_timestamp() as f64 / 100.0;
            let end = base + seg.end_timestamp() as f64 / 100.0;
            let text = seg.to_str_lossy().map(|t| clean(&t)).unwrap_or_default();
            if text.is_empty() || seg.no_speech_probability() > 0.6 {
                continue;
            }
            if (!last && start >= keep_before) || start < s.sent_until - 0.5 || text == s.last_text
            {
                continue;
            }
            s.sent_until = end;
            s.last_text = text.clone();
            out.push(Seg {
                t: start,
                end,
                speaker: SPEAKERS[who].into(),
                text,
            });
        }
        out
    }

    /// One stretch of audio (≤ 30 s) as one line: the speaker split re-transcribes a line two voices shared.
    pub fn text(&mut self, audio: &[f32]) -> String {
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len().max(1) as f32).sqrt();
        if rms < SILENCE
            || self
                .state
                .full(params(&self.lang, &self.prompt), audio)
                .is_err()
        {
            return String::new();
        }
        let parts: Vec<String> = self
            .state
            .as_iter()
            .filter(|seg| seg.no_speech_probability() <= 0.6)
            .filter_map(|seg| seg.to_str_lossy().ok().map(|t| clean(&t)))
            .filter(|t| !t.is_empty())
            .collect();
        parts.join(" ")
    }
}

fn params<'a>(lang: &'a str, prompt: &'a str) -> FullParams<'a, 'a> {
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(lang));
    // A glossary, not context: each window starts clean, so one bad window can't poison the next.
    p.set_initial_prompt(prompt);
    p.set_no_context(true);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_special(false);
    p.set_print_timestamps(false);
    p
}

/// The prompt's own words echoed at a window start (`Glossary: …`, a made-up `word:`, a leading `-`) are not speech.
fn clean(t: &str) -> String {
    // Any one-word `label:` whisper made up (`Glossary:`, and on Thai `เมื่อ:` / `กลับ:`), same rule as on read.
    let t = atharu::domain::echo::strip_label(t.trim());
    t.trim_start_matches(['-', ':', ' ']).trim().to_string()
}

/// 48 kHz (the mic, whatever SCK is asked for) or any other rate → 16 kHz, box-averaged. Box i of the whole
/// stream is input `[⌊i·step⌋, ⌊(i+1)·step⌋)`, so a callback's unused tail leads the next one: chunk sizes never
/// change the output (flooring per callback dropped up to step-1 samples each time, so Me drifted against Others).
// ponytail: box filter, no proper low-pass; fine for speech into whisper, swap for a polyphase filter if music matters.
#[derive(Default)]
pub struct To16k {
    rate: f64,
    /// Input not yet covered by a finished box.
    carry: Vec<f32>,
    /// Input samples already dropped from the front of `carry`.
    base: u64,
    /// Boxes emitted so far.
    emitted: u64,
}

impl To16k {
    /// Takes the buffer so a stream already at 16 kHz passes through without a copy.
    pub fn push(&mut self, x: Vec<f32>, rate: f64) -> Vec<f32> {
        // The mic can change rate mid-recording (input device switch): positions of the old rate mean nothing now.
        if rate != self.rate {
            *self = To16k {
                rate,
                ..Default::default()
            };
        }
        // A non-finite rate would loop forever below (NaN step → every box at 0): pass it through instead.
        if !(rate.is_finite() && rate > 0.0) || (rate - RATE as f64).abs() < 1.0 {
            return x;
        }
        self.carry.extend_from_slice(&x);
        let step = rate / RATE as f64;
        let at = |i: u64| (i as f64 * step) as u64;
        let mut out = Vec::with_capacity((x.len() as f64 / step) as usize + 1);
        loop {
            let a = (at(self.emitted) - self.base) as usize;
            // Below 16 kHz a box can be empty: hold the sample instead.
            let b = ((at(self.emitted + 1) - self.base) as usize).max(a + 1);
            let Some(w) = self.carry.get(a..b) else {
                break;
            };
            out.push(w.iter().sum::<f32>() / w.len() as f32);
            self.emitted += 1;
        }
        let used = at(self.emitted) - self.base;
        self.carry.drain(..used as usize);
        self.base += used;
        out
    }
}

/// RMS → 0–6 lit bars: -60 dBFS (nothing) to -12 dBFS (full).
pub fn level(x: &[f32]) -> u8 {
    if x.is_empty() {
        return 0;
    }
    let rms = (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let db = 20.0 * rms.max(1e-9).log10();
    (((db + 60.0) / 48.0 * 6.0).round()).clamp(0.0, 6.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_and_levels() {
        let x: Vec<f32> = (0..48).map(|i| (i / 3) as f32).collect();
        assert_eq!(
            To16k::default().push(x, 48_000.0),
            (0..16).map(|i| i as f32).collect::<Vec<_>>()
        );
        assert_eq!(
            To16k::default().push(vec![1.0, 2.0], 16_000.0),
            vec![1.0, 2.0]
        );
        assert_eq!(level(&[0.0; 160]), 0);
        assert_eq!(level(&[0.5; 160]), 6);
        assert_eq!(level(&[0.012; 160]), 3); // -38 dBFS
        assert_eq!(clean(" Glossary: Great. So"), "Great. So");
        assert_eq!(clean("-แต่ว่า"), "แต่ว่า");
    }

    /// 1024 is not a multiple of 3 (48 kHz) nor of 2.75625 (44.1 kHz): per-callback flooring used to lose samples.
    #[test]
    fn resampling_ignores_chunking() {
        let x: Vec<f32> = (0..144_017).map(|i| (i % 97) as f32).collect();
        // A bogus rate passes through instead of looping forever.
        assert_eq!(To16k::default().push(vec![1.0; 10], f64::NAN).len(), 10);
        for rate in [48_000.0, 44_100.0] {
            let whole = To16k::default().push(x.clone(), rate);
            let mut r = To16k::default();
            let chunked: Vec<f32> = x
                .chunks(1024)
                .flat_map(|c| r.push(c.to_vec(), rate))
                .collect();
            assert_eq!(chunked, whole, "{rate} Hz");
        }
        assert_eq!(To16k::default().push(x, 48_000.0).len(), 144_017 / 3);
        // A rate switch restarts the stream instead of indexing with the old rate's positions.
        let mut r = To16k::default();
        r.push(vec![0.0; 1000], 48_000.0);
        assert_eq!(r.push(vec![0.0; 1000], 16_000.0).len(), 1000);
        assert!(!r.push(vec![0.0; 1000], 44_100.0).is_empty());
    }
}
