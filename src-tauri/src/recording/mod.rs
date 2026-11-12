//! Meeting recording in Atharu.app (T53, D26/D27): ScreenCaptureKit system audio ("Others") + mic ("Me")
//! as two streams → whisper on a worker thread → segments POSTed to the app's own server as each 30 s
//! window finishes. Audio lives only in memory; no file is ever written.
//!
//! The pages never talk to this directly: they flip the recording's row (Record / Stop / End), and the
//! app's middleware calls `sync` after every meeting POST, which starts or stops capture to match.
pub mod pipeline;

use atharu::{app::AppState, recording::Mac};
use pipeline::{Pipeline, Seg};
use screencapturekit::prelude::*;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub struct Recorder {
    /// The app's own server; segments go through the same endpoint a test or another tool would use.
    server: SocketAddr,
    mac: Arc<Mac>,
    cur: Mutex<Option<Running>>,
    /// The screen-recording prompt is shown once per launch; macOS ignores repeats anyway.
    asked: AtomicBool,
}

struct Running {
    tid: i64,
    stream: SCStream,
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

impl Recorder {
    pub fn new(server: SocketAddr, mac: Arc<Mac>) -> Self {
        Recorder {
            server,
            mac,
            cur: Mutex::new(None),
            asked: AtomicBool::new(false),
        }
    }

    /// Make capture match the DB: a live row with no capture → start; capture whose row is no longer
    /// live (Stop, End, deleted) → stop, the worker finishes the tail and closes the row.
    pub fn sync(&self, st: &AppState) {
        self.check_perms();
        let want = atharu::recording::wanted(&st.conn()).ok().flatten();
        let mut cur = self.cur.lock().unwrap_or_else(|e| e.into_inner());
        if cur.as_ref().is_some_and(|r| r.worker.is_finished()) {
            *cur = None;
        }
        match (cur.as_ref(), want) {
            (Some(r), w) if !w.as_ref().is_some_and(|w| w.live && w.tid == r.tid) => {
                let r = cur.take().expect("checked");
                // Off the request path: stopping SCK waits on its queue.
                std::thread::spawn(move || {
                    let _ = r.stream.stop_capture();
                    r.stop.store(true, Ordering::SeqCst);
                    drop(r.stream);
                });
                *self.mac.levels.lock().unwrap_or_else(|e| e.into_inner()) = [0, 0];
            }
            (None, Some(w)) if w.live => match self.start(&w) {
                Ok(r) => *cur = Some(r),
                Err(e) => eprintln!("recording: {e}"),
            },
            // Finishing with nothing recording it (capture never started, or the app restarted mid-tail
            // before the orphan sweep): close it so the page stops waiting.
            (None, Some(w)) => {
                let server = self.server;
                std::thread::spawn(move || post(server, w.tid, &[], true));
            }
            _ => {}
        }
    }

    fn check_perms(&self) {
        let p = (perms::mic(), perms::screen());
        *self.mac.perms.lock().unwrap_or_else(|e| e.into_inner()) = p;
    }

    fn start(&self, w: &atharu::recording::Want) -> Result<Running, String> {
        let blocked = |b: bool| *self.mac.blocked.lock().unwrap_or_else(|e| e.into_inner()) = b;
        if !self.mac.ready() {
            return Err("no speech model".into());
        }
        if !perms::screen() || perms::mic_denied() {
            if !self.asked.swap(true, Ordering::SeqCst) {
                // Adds Atharu to the Screen & System Audio Recording list and shows macOS's own prompt.
                perms::ask_screen();
            }
            blocked(true);
            return Err("screen & system audio recording is off".into());
        }
        blocked(false);
        let (tx, rx) = mpsc::channel::<(usize, Vec<f32>)>();
        let stream = capture(tx, self.mac.clone()).inspect_err(|_| blocked(true))?;
        let stop = Arc::new(AtomicBool::new(false));
        let (model, lang, prompt) = (self.mac.model_path(), w.lang.clone(), w.prompt.clone());
        let (server, tid, flag) = (self.server, w.tid, stop.clone());
        let worker = std::thread::Builder::new()
            .name("transcribe".into())
            .spawn(move || {
                let mut p = match Pipeline::new(&model, &lang, &prompt) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("recording: model: {e}");
                        post(server, tid, &[], true);
                        return;
                    }
                };
                run(&mut p, &rx, &flag, |segs, done| {
                    post(server, tid, segs, done);
                });
                // `p` drops here: the model leaves memory as soon as the tail is done.
            })
            .map_err(|e| e.to_string())?;
        Ok(Running {
            tid: w.tid,
            stream,
            stop,
            worker,
        })
    }
}

/// The worker loop, shared with the WAV test: feed windows as audio arrives; after `stop`, drain and
/// transcribe the tail, then report `done`.
pub fn run(
    p: &mut Pipeline,
    rx: &mpsc::Receiver<(usize, Vec<f32>)>,
    stop: &AtomicBool,
    mut send: impl FnMut(&[Seg], bool),
) {
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((who, x)) => {
                let segs = p.push(who, &x);
                if !segs.is_empty() {
                    send(&segs, false);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) if !stop.load(Ordering::SeqCst) => {}
            Err(_) => break,
        }
        if stop.load(Ordering::SeqCst) {
            while let Ok((who, x)) = rx.try_recv() {
                let segs = p.push(who, &x);
                if !segs.is_empty() {
                    send(&segs, false);
                }
            }
            break;
        }
    }
    let tail = p.finish();
    send(&tail, true);
}

/// POST one batch to `/meetings/transcripts/{tid}/segments`. Plain HTTP/1.1 over loopback: one request
/// shape, so no client crate. A refused post (row deleted with its meeting) is dropped, like the audio.
pub fn post(server: SocketAddr, tid: i64, segs: &[Seg], done: bool) -> Option<u16> {
    let body = serde_json::json!({
        "segments": segs.iter().map(|s| serde_json::json!({"t": s.t, "speaker": s.speaker, "text": s.text})).collect::<Vec<_>>(),
        "done": done,
    })
    .to_string();
    let send = || -> std::io::Result<u16> {
        let mut c = TcpStream::connect_timeout(&server, Duration::from_secs(5))?;
        c.set_read_timeout(Some(Duration::from_secs(30)))?;
        write!(
            c,
            "POST /meetings/transcripts/{tid}/segments HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        let mut head = [0u8; 12];
        c.read_exact(&mut head)?;
        Ok(std::str::from_utf8(&head[9..12])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0))
    };
    // The DB lock can be busy for a moment (CLI write, big page): retry before giving the lines up.
    for _ in 0..3 {
        if let Ok(code) = send() {
            return Some(code);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    eprintln!("recording: could not save {} lines", segs.len());
    None
}

/// One SCK output: converts to 16 kHz mono, feeds the worker, lights the meter.
struct Out {
    who: usize,
    tx: Mutex<mpsc::Sender<(usize, Vec<f32>)>>,
    mac: Arc<Mac>,
}

impl SCStreamOutputTrait for Out {
    fn did_output_sample_buffer(&self, s: CMSampleBuffer, _: SCStreamOutputType) {
        let rate = s
            .format_description()
            .and_then(|f| f.audio_sample_rate())
            .unwrap_or(pipeline::RATE as f64);
        let Ok(list) = s.audio_buffer_list() else {
            return;
        };
        // Asked for 1 channel Float32: buffer 0 is the whole signal (the mic is mono too, at 48 kHz).
        let Some(b) = list.into_iter().next() else {
            return;
        };
        let raw: Vec<f32> = b
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let x = pipeline::to_16k(&raw, rate);
        self.mac.levels.lock().unwrap_or_else(|e| e.into_inner())[self.who] = pipeline::level(&x);
        let _ = self
            .tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send((self.who, x));
    }
}

/// System audio (16 kHz mono, as asked) + mic (48 kHz whatever is asked). A 2×2 display filter: SCK needs
/// a video source to deliver audio; the frames are tiny and never read.
fn capture(tx: mpsc::Sender<(usize, Vec<f32>)>, mac: Arc<Mac>) -> Result<SCStream, String> {
    let e = |e: &dyn std::fmt::Display| e.to_string();
    let content = SCShareableContent::get().map_err(|x| e(&x))?;
    let display = content.displays().into_iter().next().ok_or("no display")?;
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build()
        .map_err(|x| e(&x))?;
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_captures_audio(true)
        .with_excludes_current_process_audio(true)
        .with_sample_rate(16_000)
        .with_channel_count(1)
        .with_captures_microphone(true)
        .map_err(|x| e(&x))?;
    let mut st = SCStream::new(&filter, &config).map_err(|x| e(&x))?;
    let out = |who| Out {
        who,
        tx: Mutex::new(tx.clone()),
        mac: mac.clone(),
    };
    st.add_output_handler(out(1), SCStreamOutputType::Audio)
        .map_err(|x| e(&x))?;
    st.add_output_handler(out(0), SCStreamOutputType::Microphone)
        .map_err(|x| e(&x))?;
    st.start_capture().map_err(|x| e(&x))?;
    Ok(st)
}

/// Screen & System Audio Recording allowed? If not, macOS's prompt (once per app) and false (`Mac::ask_screen`).
pub fn screen_or_ask() -> bool {
    perms::screen() || {
        perms::ask_screen();
        false
    }
}

/// macOS privacy grants (TCC). Screen & System Audio Recording has a C API; the mic has only AVFoundation.
mod perms {
    use objc2::{class, msg_send, runtime::AnyObject};

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: &'static AnyObject;
    }

    pub fn screen() -> bool {
        // SAFETY: no arguments, no state; documented as safe from any thread.
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn ask_screen() {
        // SAFETY: as above; shows the system prompt at most once per app.
        unsafe { CGRequestScreenCaptureAccess() };
    }

    /// AVAuthorizationStatus: 0 not asked yet (SCK asks on first capture), 1 restricted, 2 denied, 3 allowed.
    fn mic_status() -> isize {
        // SAFETY: a class method taking a constant NSString, returning an NSInteger.
        unsafe {
            msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio]
        }
    }

    pub fn mic() -> bool {
        mic_status() == 3
    }

    pub fn mic_denied() -> bool {
        matches!(mic_status(), 1 | 2)
    }
}
