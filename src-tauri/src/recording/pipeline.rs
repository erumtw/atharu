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
    pub speaker: &'static str,
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
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_language(Some(&self.lang));
        // A glossary, not context: each window starts clean, so one bad window can't poison the next.
        p.set_initial_prompt(&self.prompt);
        p.set_no_context(true);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_special(false);
        p.set_print_timestamps(false);
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
                speaker: SPEAKERS[who],
                text,
            });
        }
        out
    }
}

/// The prompt's own words echoed at a window start (`Glossary: …`, a made-up `word:`, a leading `-`) are not speech.
fn clean(t: &str) -> String {
    // Any one-word `label:` whisper made up (`Glossary:`, and on Thai `เมื่อ:` / `กลับ:`), same rule as on read.
    let t = atharu::domain::echo::strip_label(t.trim());
    t.trim_start_matches(['-', ':', ' ']).trim().to_string()
}

/// 48 kHz (the mic, whatever SCK is asked for) or any other rate → 16 kHz, box-averaged.
// ponytail: box filter, no proper low-pass; fine for speech into whisper, swap for a polyphase filter if music matters.
pub fn to_16k(x: &[f32], rate: f64) -> Vec<f32> {
    if (rate - RATE as f64).abs() < 1.0 || rate <= 0.0 {
        return x.to_vec();
    }
    let step = rate / RATE as f64;
    let n = (x.len() as f64 / step) as usize;
    (0..n)
        .map(|i| {
            let (a, b) = (
                (i as f64 * step) as usize,
                (((i + 1) as f64 * step) as usize).min(x.len()),
            );
            let w = &x[a..b.max(a + 1).min(x.len())];
            w.iter().sum::<f32>() / w.len().max(1) as f32
        })
        .collect()
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
            to_16k(&x, 48_000.0),
            (0..16).map(|i| i as f32).collect::<Vec<_>>()
        );
        assert_eq!(to_16k(&[1.0, 2.0], 16_000.0), vec![1.0, 2.0]);
        assert_eq!(level(&[0.0; 160]), 0);
        assert_eq!(level(&[0.5; 160]), 6);
        assert_eq!(level(&[0.012; 160]), 3); // -38 dBFS
        assert_eq!(clean(" Glossary: Great. So"), "Great. So");
        assert_eq!(clean("-แต่ว่า"), "แต่ว่า");
    }
}
