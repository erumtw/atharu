//! Speaker split (T56; D34, D35, D37): after Stop, the Others audio of one recording is diarized on this Mac
//! and its lines become Speaker 1..N. The audio sits in a temp file only until this pass ends (a crash leaves
//! it for `sweep` at the next launch); voice embeddings live inside one `process` call, never across meetings.
use super::pipeline::{Pipeline, RATE, Seg};
use sherpa_onnx::{OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig};
use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

/// (file, url, sha256): pyannote segmentation 3.0 int8 (MIT, 1.5 MB) + 3D-Speaker CAM++ zh_en (Apache-2.0,
/// 28.3 MB). D35: int8 segmentation was the only one that caught the overlapped line.
const MODELS: [(&str, &str, &str); 2] = [
    (
        "pyannote-segmentation-3-0.int8.onnx",
        "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/main/model.int8.onnx",
        "d582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d",
    ),
    (
        "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
        "aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2",
    ),
];
/// Middle of the band (0.7–0.9) where the probe clip clustered into its 3 voices (D35).
const THRESHOLD: f32 = 0.8;
/// A line whose span holds ≥ 2 speakers for longer than this each is re-transcribed per speaker (D37).
const MIN_TURN: f64 = 1.0;
/// A diarized stretch this long that no live line covers is speech whisper dropped: transcribed on its own.
const MIN_GAP: f64 = 2.0;
const TEMP_PREFIX: &str = "atharu-others-";

/// `<models>/speakers`: `Mac::delete` removes it with the speech model.
pub fn dir(models: &Path) -> PathBuf {
    models.join("speakers")
}

/// Both models on disk and checked; fetches what's missing (curl + shasum like the speech model, no UI:
/// 30 MB arrives while the meeting runs). Blocking; one fetch at a time.
pub fn fetch(dir: &Path) -> bool {
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    MODELS.iter().all(|(file, url, sha)| {
        let path = dir.join(file);
        if path.is_file() {
            return true; // only a checked file is ever renamed to this name
        }
        let part = dir.join(format!("{file}.part"));
        let ok = Command::new("curl")
            .args(["-sSfL", "--retry", "2", "-o"])
            .arg(&part)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
            && atharu::recording::sha256(&part).as_deref() == Some(*sha)
            && std::fs::rename(&part, &path).is_ok();
        if !ok {
            let _ = std::fs::remove_file(&part);
        }
        ok
    })
}

/// Others audio of a crashed or quit run, left behind: deleted at launch.
pub fn sweep() {
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for e in rd.flatten() {
        if e.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The Others stream as 16 kHz mono s16 in a temp file, for one recording.
pub struct Tap {
    path: PathBuf,
    w: BufWriter<File>,
}

impl Tap {
    pub fn new(tid: i64) -> Option<Tap> {
        let path = std::env::temp_dir().join(format!("{TEMP_PREFIX}{tid}.pcm"));
        let f = File::create(&path).ok()?;
        Some(Tap {
            path,
            w: BufWriter::new(f),
        })
    }

    pub fn write(&mut self, x: &[f32]) {
        for v in x {
            let s = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
            // A full disk loses the split, not the transcript: `split` then finds the file short.
            let _ = self.w.write_all(&s.to_le_bytes());
        }
    }

    /// In 1 MB chunks: the whole file as bytes next to its f32 copy was +115 MB per hour at the Stop peak.
    fn read(mut self) -> Vec<f32> {
        let _ = self.w.flush();
        let Ok(mut f) = File::open(&self.path) else {
            return Vec::new();
        };
        let len = f.metadata().map(|m| m.len() as usize).unwrap_or(0);
        let mut out = Vec::with_capacity(len / 2);
        let mut b = vec![0u8; 1 << 20];
        let mut have = 0;
        loop {
            match f.read(&mut b[have..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => have += n,
            }
            let (whole, _) = b[..have].as_chunks::<2>();
            out.extend(
                whole
                    .iter()
                    .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0),
            );
            // An odd byte (a short read mid-sample) carries over to the next chunk.
            let used = whole.len() * 2;
            b.copy_within(used..have, 0);
            have -= used;
        }
        out
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The recording's Others lines (as sent live) → the same lines with speakers, re-transcribing the ones two
/// voices share. `None` = no split (models missing, too little audio): the lines stay "Others".
/// The temp audio is deleted when this returns, whatever happened.
// ponytail: reads the whole Others stream into RAM (≈ 230 MB per hour of meeting) next to the loaded
// whisper model; stream it in chunks if hour-plus meetings run out of memory. Unloading whisper while
// diarizing measured worse (peak +34 MB, +5 s on a 42 s clip: the reload costs more than it frees).
pub fn split(tap: Tap, models: &Path, p: &mut Pipeline, lines: &[Seg]) -> Option<Vec<Seg>> {
    let audio = tap.read();
    if lines.is_empty() || audio.len() < 2 * RATE || !fetch(&dir(models)) {
        return None;
    }
    let turns = diarize(&dir(models), &audio)?;
    let at = |s: f64| ((s.max(0.0) * RATE as f64) as usize).min(audio.len());
    let jobs = plan(lines, &turns);
    let done: Vec<(f64, f64, Option<i32>, String)> = jobs
        .into_iter()
        .filter_map(|(t, end, who, text)| {
            let text = text.unwrap_or_else(|| p.text(&audio[at(t)..at(end)]));
            (!text.is_empty()).then_some((t, end, who, text))
        })
        .collect();
    Some(number(done))
}

fn diarize(dir: &Path, audio: &[f32]) -> Option<Vec<(f64, f64, i32)>> {
    let path = |i: usize| Some(dir.join(MODELS[i].0).to_string_lossy().into_owned());
    let mut cfg = OfflineSpeakerDiarizationConfig::default();
    cfg.segmentation.pyannote.model = path(0);
    cfg.segmentation.num_threads = 4;
    cfg.embedding.model = path(1);
    cfg.embedding.num_threads = 4;
    cfg.clustering.num_clusters = -1;
    cfg.clustering.threshold = THRESHOLD;
    // Dropped at the end of this fn: whisper alone is in memory again for the re-transcription (D35 peak).
    let sd = OfflineSpeakerDiarization::create(&cfg)?;
    let r = sd.process(audio)?;
    Some(
        r.sort_by_start_time()
            .iter()
            .map(|s| (s.start as f64, s.end as f64, s.speaker))
            .collect(),
    )
}

/// Per Others line: (start, end, diarized speaker, text). Text `None` = re-transcribe that span: the line
/// covers ≥ 2 speakers for > `MIN_TURN` each, so it's cut at the diarization boundaries (D35: whisper's own
/// lines run across turns). Otherwise the line keeps its text and takes the speaker it overlaps most
/// (none at all → `None`, it stays "Others").
pub fn plan(
    lines: &[Seg],
    turns: &[(f64, f64, i32)],
) -> Vec<(f64, f64, Option<i32>, Option<String>)> {
    let mut out = Vec::new();
    for s in lines {
        let mut by: Vec<(i32, f64)> = Vec::new();
        for &(a, b, k) in turns {
            let o = b.min(s.end) - a.max(s.t);
            if o <= 0.0 {
                continue;
            }
            match by.iter_mut().find(|x| x.0 == k) {
                Some(x) => x.1 += o,
                None => by.push((k, o)),
            }
        }
        if by.iter().filter(|x| x.1 > MIN_TURN).count() >= 2 {
            let mut cuts: Vec<(f64, f64, i32)> = Vec::new();
            for &(a, b, k) in turns {
                let (a, b) = (a.max(s.t), b.min(s.end));
                if b > a {
                    cuts.push((a, b, k));
                }
            }
            let merge = |c: &mut Vec<(f64, f64, i32)>| {
                c.dedup_by(|x, l| {
                    let same = x.2 == l.2;
                    if same {
                        l.1 = l.1.max(x.1);
                    }
                    same
                })
            };
            merge(&mut cuts);
            // A sliver of a voice (a hand-off, an "mm") joins the piece before it (the first: the one after).
            let mut i = 0;
            while cuts.len() >= 2 && i < cuts.len() {
                if cuts[i].1 - cuts[i].0 >= MIN_TURN {
                    i += 1;
                    continue;
                }
                let c = cuts.remove(i);
                if i > 0 {
                    cuts[i - 1].1 = cuts[i - 1].1.max(c.1);
                } else {
                    cuts[0].0 = c.0;
                }
            }
            merge(&mut cuts);
            out.extend(cuts.into_iter().map(|(a, b, k)| (a, b, Some(k), None)));
            continue;
        }
        let best = by.iter().max_by(|x, y| x.1.total_cmp(&y.1)).map(|x| x.0);
        out.push((s.t, s.end, best, Some(s.text.clone())));
    }
    // Speech no line covers: the live pass loses a turn at a 30 s window edge now and then (4 of 18 on the
    // probe clip), and the diarization knows it was there, so it's transcribed on its own.
    let mut spans: Vec<(f64, f64)> = lines.iter().map(|s| (s.t, s.end)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(a, b, k) in turns {
        let mut at = a;
        for &(t, e) in spans.iter().chain([(b, b)].iter()) {
            if e <= at || at >= b {
                continue;
            }
            if t.min(b) - at >= MIN_GAP {
                out.push((at, t.min(b), Some(k), None));
            }
            at = at.max(e);
        }
    }
    out
}

/// Diarization ids → `Speaker 1..N` in order of first line; a line with no speaker stays "others".
fn number(mut lines: Vec<(f64, f64, Option<i32>, String)>) -> Vec<Seg> {
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut order: Vec<i32> = Vec::new();
    lines
        .into_iter()
        .map(|(t, end, who, text)| {
            let speaker = match who {
                Some(k) => {
                    let n = order.iter().position(|&o| o == k).unwrap_or_else(|| {
                        order.push(k);
                        order.len() - 1
                    });
                    format!("Speaker {}", n + 1)
                }
                None => "others".into(),
            };
            Seg {
                t,
                end,
                speaker,
                text,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t: f64, end: f64, text: &str) -> Seg {
        Seg {
            t,
            end,
            speaker: "others".into(),
            text: text.into(),
        }
    }

    #[test]
    fn tap_reads_back_what_it_wrote_across_chunks() {
        let x: Vec<f32> = (0..700_000)
            .map(|i| ((i % 200) as f32 - 100.0) / 128.0)
            .collect();
        let mut t = Tap::new(-1).unwrap();
        t.write(&x);
        let y = t.read();
        assert_eq!(y.len(), x.len());
        assert!(x.iter().zip(&y).all(|(a, b)| (a - b).abs() < 1e-4));
    }

    #[test]
    fn lines_take_the_voice_they_overlap_most_and_shared_lines_are_cut() {
        let turns = [
            (0.0, 4.0, 7),
            (4.2, 9.0, 3),
            (9.0, 9.4, 7), // a 0.4 s hand-off sliver
            (9.4, 14.0, 7),
            (20.0, 25.0, 3),
        ];
        let lines = [
            seg(0.5, 4.5, "hello all"),
            seg(4.5, 14.0, "two voices in one line"),
            seg(16.0, 18.0, "nobody per diarization"),
            seg(21.0, 24.0, "back again"),
        ];
        let p = plan(&lines, &turns);
        let short: Vec<_> = p
            .iter()
            .map(|(a, b, k, t)| (*a, *b, *k, t.is_some()))
            .collect();
        assert_eq!(
            short,
            [
                (0.5, 4.5, Some(7), true),
                (4.5, 9.0, Some(3), false),
                (9.0, 14.0, Some(7), false), // the sliver joined its own speaker's piece
                (16.0, 18.0, None, true),
                (21.0, 24.0, Some(3), true),
            ]
        );
        let named = number(
            p.into_iter()
                .map(|(t, e, k, x)| (t, e, k, x.unwrap_or_else(|| "re".into())))
                .collect(),
        );
        let who: Vec<&str> = named.iter().map(|s| s.speaker.as_str()).collect();
        assert_eq!(
            who,
            ["Speaker 1", "Speaker 2", "Speaker 1", "others", "Speaker 2"]
        );
        // A third voice's 0.6 s "mm" between two turns stays in the piece before it.
        let p = plan(
            &[seg(30.0, 40.0, "x")],
            &[(30.0, 35.0, 1), (35.0, 35.6, 2), (35.6, 40.0, 3)],
        );
        let cuts: Vec<_> = p.iter().map(|(a, b, k, _)| (*a, *b, *k)).collect();
        assert_eq!(cuts, [(30.0, 35.6, Some(1)), (35.6, 40.0, Some(3))]);
        // Speech no line covers (≥ 2 s) is transcribed on its own; a 1 s rim around a line isn't.
        let p = plan(&[seg(1.0, 4.0, "a"), seg(8.0, 9.0, "b")], &[(0.0, 12.0, 1)]);
        let cuts: Vec<_> = p
            .iter()
            .map(|(a, b, k, t)| (*a, *b, *k, t.is_some()))
            .collect();
        assert_eq!(
            cuts,
            [
                (1.0, 4.0, Some(1), true),
                (8.0, 9.0, Some(1), true),
                (4.0, 8.0, Some(1), false),
                (9.0, 12.0, Some(1), false)
            ]
        );
    }
}
