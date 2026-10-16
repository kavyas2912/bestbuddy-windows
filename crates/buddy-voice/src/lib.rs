use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sherpa_onnx::{
    GenerationConfig, KeywordSpotter, KeywordSpotterConfig, LinearResampler, OfflineRecognizer,
    OfflineRecognizerConfig, OfflineSpeechDenoiserGtcrnModelConfig,
    OfflineSpeechDenoiserModelConfig, OfflineTts, OfflineTtsConfig, OfflineTtsVitsModelConfig,
    OnlineSpeechDenoiser, OnlineSpeechDenoiserConfig, VadModelConfig, VoiceActivityDetector,
};
use std::{
    collections::VecDeque,
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoiceState {
    Unavailable,
    Ready,
    WakeListening,
    Listening,
    Transcribing,
    Synthesizing,
    Speaking,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEvent {
    pub kind: String,
    pub text: Option<String>,
    pub keyword: Option<String>,
    pub asr_ms: Option<u64>,
    pub at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoiceRuntimeStatus {
    pub state: String,
    pub always_on: bool,
    pub wake_mode: String,
    pub wake_word: String,
    pub microphone_ready: bool,
    pub microphone: Option<String>,
    pub speaker_ready: bool,
    pub speaker: Option<String>,
    pub asr_ready: bool,
    pub tts_ready: bool,
    pub vad_ready: bool,
    pub kws_ready: bool,
    pub denoise_ready: bool,
    pub punctuation_ready: bool,
    pub last_transcript: Option<String>,
    pub transcript_sequence: u64,
    pub last_voice_ms: Option<u64>,
    pub barge_in_count: u64,
    pub wake_count: u64,
    pub kws_decode_count: u64,
    pub kws_last_decode_ms: u64,
    pub dropped_audio_buffers: u64,
    pub input_level: f32,
    pub raw_input_level: f32,
    pub input_rate: u32,
    pub input_channels: u16,
    pub input_frames: u64,
    pub last_turn_peak: f32,
    pub last_turn_frames: u64,
    pub last_turn_hypotheses: u64,
    pub vad_active: bool,
    pub asr_partial: Option<String>,
    pub rejected_fragments: u64,
    pub last_turn_outcome: Option<String>,
    pub pause_to_finish_ms: u64,
    pub max_listening_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MicrophoneTestClip {
    pub wav_bytes: Vec<u8>,
    pub sample_rate: u32,
    pub frames: usize,
    pub peak: f32,
    pub device: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WavTestResult {
    pub sample_rate: u32,
    pub frames: usize,
    pub raw_transcript: String,
    pub enhanced_transcript: Option<String>,
    pub transcript: String,
    pub accepted: bool,
}

#[derive(Default)]
struct TurnCaptureStats {
    peak: f32,
    frames: u64,
    hypotheses: u64,
}

impl TurnCaptureStats {
    fn accept(&mut self, samples: &[f32]) {
        self.frames = self.frames.saturating_add(samples.len() as u64);
        self.peak = samples
            .iter()
            .fold(self.peak, |peak, sample| peak.max(sample.abs()));
    }
    fn store(&self, status: &mut VoiceRuntimeStatus) {
        status.last_turn_peak = self.peak;
        status.last_turn_frames = self.frames;
        status.last_turn_hypotheses = self.hypotheses;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub asr_dir: PathBuf,
    pub tts_dir: PathBuf,
    pub vad_model: PathBuf,
    pub kws_dir: PathBuf,
    pub kws_keywords: PathBuf,
    pub denoise_model: PathBuf,
    pub num_threads: i32,
}

impl VoiceConfig {
    pub fn from_env() -> Self {
        Self {
            asr_dir: std::env::var("BUDDY_ASR_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| find_model_dir("models/asr/sherpa-onnx-whisper-base.en")),
            tts_dir: std::env::var("BUDDY_TTS_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| find_model_dir("models/tts/vits-piper-en_US-lessac-medium")),
            vad_model: std::env::var("BUDDY_VAD_MODEL")
                .map(PathBuf::from)
                .unwrap_or_else(|_| find_model_dir("models/vad/silero_vad.onnx")),
            kws_dir: std::env::var("BUDDY_KWS_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    find_model_dir(
                        "models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01",
                    )
                }),
            kws_keywords: std::env::var("BUDDY_KWS_KEYWORDS")
                .map(PathBuf::from)
                .unwrap_or_else(|_| find_model_dir("models/kws/keywords.txt")),
            denoise_model: std::env::var("BUDDY_DENOISE_MODEL")
                .map(PathBuf::from)
                .unwrap_or_else(|_| find_model_dir("models/denoise/gtcrn_simple.onnx")),
            num_threads: 2,
        }
    }
}

fn find_model_dir(relative: &str) -> PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..10 {
        let candidate = p.join(relative);
        if candidate.exists() {
            #[cfg(windows)]
            if let Some(alias) = find_native_model_alias(&p, relative) {
                return alias;
            }
            return candidate;
        }
        if !p.pop() {
            break;
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(relative)
}

fn piper_model_file(dir: &Path) -> Option<PathBuf> {
    if !dir.join("tokens.txt").is_file()
        || ["phontab", "phonindex", "phondata", "intonations"]
            .iter()
            .any(|name| !dir.join("espeak-ng-data").join(name).is_file())
        || !dir.join("espeak-ng-data/lang").is_dir()
    {
        return None;
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "onnx"))
}

// A light, clearly labelled character effect on the bundled Piper voice.
// It is not a separately trained bird voice or a voice-cloning model.
fn save_macaw_effect(path: &Path, samples: &[f32], sample_rate: i32) -> bool {
    if sample_rate <= 0 || samples.is_empty() {
        return false;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let Ok(mut writer) = hound::WavWriter::create(path, spec) else {
        return false;
    };
    let mut position = 0.0_f32;
    while position + 1.0 < samples.len() as f32 {
        let index = position as usize;
        let fraction = position - index as f32;
        let sample = samples[index] * (1.0 - fraction) + samples[index + 1] * fraction;
        if writer
            .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .is_err()
        {
            return false;
        }
        position += 1.11 + 0.015 * (position / (sample_rate as f32 * 0.06)).sin();
    }
    writer.finalize().is_ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct PiperVoice {
    pub id: String,
    pub name: String,
    pub language: String,
}

#[cfg(windows)]
fn find_native_model_alias(source_root: &Path, relative: &str) -> Option<PathBuf> {
    let suffix = relative.strip_prefix("models/")?;
    let models = source_root.join("models");
    let link_file = models.join(".native-link");
    let link = PathBuf::from(std::fs::read_to_string(link_file).ok()?.trim());
    if std::fs::canonicalize(&link).ok()? != std::fs::canonicalize(&models).ok()? {
        return None;
    }
    let candidate = link.join(suffix);
    candidate.exists().then_some(candidate)
}

// Windows microphone arrays can expose channels with opposite phase or one
// nearly silent channel. Averaging them can erase the user's speech entirely.
fn microphone_mono<T: Copy>(input: &[T], channels: usize, scale: impl Fn(T) -> f32) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    let frames = input.len() / channels;
    if frames == 0 {
        return Vec::new();
    }
    let mut selected = 0;
    let mut best_power = 0.0f32;
    for channel in 0..channels {
        let power = (0..frames)
            .map(|frame| {
                let value = scale(input[frame * channels + channel]);
                if value.is_finite() {
                    value * value
                } else {
                    0.0
                }
            })
            .sum::<f32>();
        if channel == 0 || power > best_power * 1.1 {
            selected = channel;
            best_power = power;
        }
    }
    (0..frames)
        .map(|frame| {
            let value = scale(input[frame * channels + selected]);
            if value.is_finite() {
                value.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

/// Capture the same default CPAL input and channel-selection format used by
/// the always-on runtime. Recording is explicit and remains in memory until
/// the user chooses to save the returned WAV.
pub fn record_microphone_test(duration_ms: u32) -> Result<MicrophoneTestClip> {
    if !(1_000..=5_000).contains(&duration_ms) {
        return Err(anyhow!("Microphone test must last 1–5 seconds"));
    }
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow!("No default microphone input device"))?;
    let device_name = device
        .name()
        .unwrap_or_else(|_| "Default microphone".into());
    let supported = device.default_input_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let target_frames = (sample_rate as usize * duration_ms as usize) / 1_000;
    let captured = Arc::new(Mutex::new(Vec::<f32>::with_capacity(target_frames)));
    let stream_error = Arc::new(Mutex::new(None::<String>));
    let error_slot = Arc::clone(&stream_error);
    let err_fn = move |error: cpal::StreamError| {
        if let Ok(mut slot) = error_slot.lock() {
            *slot = Some(error.to_string());
        }
    };
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => {
            let sink = Arc::clone(&captured);
            device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    append_test_samples(
                        &sink,
                        microphone_mono(data, channels, |x| x),
                        target_frames,
                    );
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let sink = Arc::clone(&captured);
            device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    append_test_samples(
                        &sink,
                        microphone_mono(data, channels, |x| x as f32 / 32768.0),
                        target_frames,
                    );
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let sink = Arc::clone(&captured);
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    append_test_samples(
                        &sink,
                        microphone_mono(data, channels, |x| (x as f32 - 32768.0) / 32768.0),
                        target_frames,
                    );
                },
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("Unsupported microphone sample format: {other:?}")),
    };
    stream.play()?;
    thread::sleep(Duration::from_millis(duration_ms as u64 + 120));
    drop(stream);
    if let Some(error) = stream_error
        .lock()
        .map_err(|_| anyhow!("Microphone test status unavailable"))?
        .take()
    {
        return Err(anyhow!("Microphone stream error: {error}"));
    }
    let samples = std::mem::take(
        &mut *captured
            .lock()
            .map_err(|_| anyhow!("Microphone test buffer unavailable"))?,
    );
    if samples.len() < sample_rate as usize / 2 {
        return Err(anyhow!(
            "Microphone delivered too few frames: {} at {} Hz",
            samples.len(),
            sample_rate
        ));
    }
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    let wav_bytes = encode_test_wav(&samples, sample_rate)?;
    Ok(MicrophoneTestClip {
        wav_bytes,
        sample_rate,
        frames: samples.len(),
        peak,
        device: device_name,
    })
}

fn append_test_samples(sink: &Arc<Mutex<Vec<f32>>>, chunk: Vec<f32>, limit: usize) {
    if let Ok(mut samples) = sink.lock() {
        let take = limit.saturating_sub(samples.len()).min(chunk.len());
        samples.extend_from_slice(&chunk[..take]);
    }
}

fn encode_test_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn decode_test_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    if bytes.len() < 44 || bytes.len() > 16 * 1024 * 1024 {
        return Err(anyhow!("Choose a WAV file between 44 bytes and 16 MB"));
    }
    let mut reader = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = reader.spec();
    if !(8_000..=192_000).contains(&spec.sample_rate) || !(1..=8).contains(&spec.channels) {
        return Err(anyhow!("Unsupported WAV rate or channel count"));
    }
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|x| x.map(|v| v as f32 / 32768.0))
            .collect::<std::result::Result<_, _>>()?,
        (hound::SampleFormat::Int, 24 | 32) => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|x| x.map(|v| v as f32 / scale))
                .collect::<std::result::Result<_, _>>()?
        }
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        _ => return Err(anyhow!("Use 16/24/32-bit PCM or 32-bit float WAV")),
    };
    let mono = microphone_mono(&interleaved, spec.channels as usize, |x| x);
    if mono.is_empty() {
        return Err(anyhow!("WAV contains no audio frames"));
    }
    Ok((mono, spec.sample_rate))
}

/// Replay a selected WAV through the same sherpa streaming ASR and optional
/// GTCRN enhancement stages used in the live path, retaining both hypotheses
/// so a denoiser regression can be distinguished from capture failure.
pub fn test_wav_through_asr(config: &VoiceConfig, bytes: &[u8]) -> Result<WavTestResult> {
    let (samples, sample_rate) = decode_test_wav(bytes)?;
    let recognizer = build_whisper(config)
        .ok_or_else(|| anyhow!("Whisper ASR model is incomplete or failed to load"))?;
    let audio = if sample_rate != 16_000 {
        LinearResampler::create(sample_rate as i32, 16_000)
            .ok_or_else(|| anyhow!("Cannot resample WAV from {sample_rate} Hz"))?
            .resample(&samples, true)
    } else {
        samples.clone()
    };
    let raw_transcript = decode_whisper(&recognizer, &audio);
    let enhanced_transcript = build_denoiser(config).and_then(|denoiser| {
        let shift = denoiser.frame_shift_in_samples() as usize;
        if shift == 0 {
            return None;
        }
        let mut enhanced = Vec::new();
        for chunk in audio.chunks_exact(shift) {
            enhanced.extend(denoiser.run(chunk, 16_000).samples);
        }
        Some(decode_whisper(&recognizer, &enhanced))
    });
    let selected = if usable_transcript(&raw_transcript) {
        raw_transcript.as_str()
    } else {
        enhanced_transcript
            .as_deref()
            .filter(|text| usable_transcript(text))
            .unwrap_or(&raw_transcript)
    };
    let transcript = selected.trim().to_string();
    let accepted = usable_transcript(&transcript);
    Ok(WavTestResult {
        sample_rate,
        frames: samples.len(),
        raw_transcript,
        enhanced_transcript,
        transcript,
        accepted,
    })
}

enum PlaybackCommand {
    Play(PathBuf, mpsc::Sender<Result<(), String>>),
    Stop,
    Shutdown,
}

#[derive(Clone)]
struct AudioControl {
    tx: mpsc::Sender<PlaybackCommand>,
}

impl AudioControl {
    fn stop(&self) {
        let _ = self.tx.send(PlaybackCommand::Stop);
    }
}

struct AudioWorker {
    tx: mpsc::Sender<PlaybackCommand>,
}

impl AudioWorker {
    fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel::<PlaybackCommand>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        thread::Builder::new()
            .name("buddy-audio-output".into())
            .spawn(move || {
                let output = match OutputStreamBuilder::open_default_stream() {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));

                let mut current: Option<(Sink, mpsc::Sender<Result<(), String>>)> = None;
                loop {
                    if current.is_some() {
                        match rx.recv_timeout(Duration::from_millis(20)) {
                            Ok(PlaybackCommand::Play(path, done)) => {
                                if let Some((sink, old_done)) = current.take() {
                                    sink.stop();
                                    let _ = old_done.send(Ok(()));
                                }
                                let sink = Sink::connect_new(output.mixer());
                                let result = (|| -> Result<(), String> {
                                    let file = File::open(&path).map_err(|e| e.to_string())?;
                                    let source =
                                        Decoder::try_from(file).map_err(|e| e.to_string())?;
                                    sink.append(source);
                                    Ok(())
                                })();
                                match result {
                                    Ok(()) => current = Some((sink, done)),
                                    Err(e) => {
                                        let _ = done.send(Err(e));
                                    }
                                }
                            }
                            Ok(PlaybackCommand::Stop) => {
                                if let Some((sink, done)) = current.take() {
                                    sink.stop();
                                    let _ = done.send(Ok(()));
                                }
                            }
                            Ok(PlaybackCommand::Shutdown) => {
                                if let Some((sink, done)) = current.take() {
                                    sink.stop();
                                    let _ = done.send(Ok(()));
                                }
                                break;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Some((sink, done)) = current.take() {
                                    if sink.empty() {
                                        let _ = done.send(Ok(()));
                                    } else {
                                        current = Some((sink, done));
                                    }
                                }
                            }
                        }
                    } else {
                        match rx.recv() {
                            Ok(PlaybackCommand::Play(path, done)) => {
                                let sink = Sink::connect_new(output.mixer());
                                let result = (|| -> Result<(), String> {
                                    let file = File::open(&path).map_err(|e| e.to_string())?;
                                    let source =
                                        Decoder::try_from(file).map_err(|e| e.to_string())?;
                                    sink.append(source);
                                    Ok(())
                                })();
                                match result {
                                    Ok(()) => current = Some((sink, done)),
                                    Err(e) => {
                                        let _ = done.send(Err(e));
                                    }
                                }
                            }
                            Ok(PlaybackCommand::Stop) => {}
                            Ok(PlaybackCommand::Shutdown) | Err(_) => break,
                        }
                    }
                }
            })
            .map_err(|e| anyhow!("audio worker spawn failed: {e}"))?;

        ready_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| anyhow!("audio worker did not initialize"))?
            .map_err(|e| anyhow!("audio worker initialization failed: {e}"))?;
        Ok(Self { tx })
    }

    fn control(&self) -> AudioControl {
        AudioControl {
            tx: self.tx.clone(),
        }
    }

    fn play(&self, path: PathBuf) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(PlaybackCommand::Play(path, tx))
            .map_err(|e| anyhow!(e.to_string()))?;
        rx.recv()
            .map_err(|e| anyhow!(e.to_string()))?
            .map_err(anyhow::Error::msg)
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(PlaybackCommand::Shutdown);
    }
}

#[derive(Clone)]
pub struct VoiceMonitor {
    runtime: Arc<Mutex<VoiceRuntimeStatus>>,
    events: Arc<Mutex<VecDeque<VoiceEvent>>>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<AtomicBool>,
    playback: Arc<Mutex<Option<AudioControl>>>,
    pause_to_finish_ms: Arc<AtomicU64>,
    max_listening_ms: Arc<AtomicU64>,
    control: Arc<Mutex<Option<mpsc::Sender<RuntimeCommand>>>>,
}
impl VoiceMonitor {
    pub fn arm_manual(&self) -> Result<()> {
        let sender = self
            .control
            .lock()
            .map_err(|_| anyhow!("Voice control unavailable"))?
            .clone()
            .ok_or_else(|| anyhow!("Voice runtime is not running"))?;
        self.interrupt.store(true, Ordering::SeqCst);
        if let Ok(playback) = self.playback.lock() {
            if let Some(playback) = &*playback {
                playback.stop();
            }
        }
        sender
            .send(RuntimeCommand::Arm)
            .map_err(|e| anyhow!(e.to_string()))
    }
    pub fn tune(&self, pause_to_finish_ms: u64, max_listening_ms: u64) -> Result<()> {
        if !(400..=1800).contains(&pause_to_finish_ms)
            || !(6_000..=30_000).contains(&max_listening_ms)
        {
            return Err(anyhow!("Voice timing is outside the supported range"));
        }
        self.pause_to_finish_ms
            .store(pause_to_finish_ms, Ordering::SeqCst);
        self.max_listening_ms
            .store(max_listening_ms, Ordering::SeqCst);
        if let Ok(mut status) = self.runtime.lock() {
            status.pause_to_finish_ms = pause_to_finish_ms;
            status.max_listening_ms = max_listening_ms;
        }
        Ok(())
    }
    pub fn status(&self) -> VoiceRuntimeStatus {
        self.runtime
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }
    pub fn diagnostics(&self) -> serde_json::Value {
        serde_json::to_value(self.runtime.lock().map(|r| r.clone()).unwrap_or_default())
            .unwrap_or_default()
    }
    pub fn take_events(&self) -> Vec<VoiceEvent> {
        self.events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
    pub fn interrupt_playback(&self) {
        self.interrupt.store(true, Ordering::SeqCst);
        if let Ok(playback) = self.playback.lock() {
            if let Some(playback) = &*playback {
                playback.stop();
            }
        }
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.interrupt.store(true, Ordering::SeqCst);
        if let Ok(p) = self.playback.lock() {
            if let Some(p) = &*p {
                p.stop();
            }
        }
    }
}
struct RuntimeGuard(Arc<Mutex<VoiceRuntimeStatus>>);
impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        if let Ok(mut s) = self.0.lock() {
            s.always_on = false;
            s.microphone_ready = false;
            s.asr_ready = false;
            if s.state != "Error" {
                s.state = "Stopped".into();
            }
        }
    }
}
pub struct VoiceEngine {
    monitor: VoiceMonitor,
    worker: Option<thread::JoinHandle<()>>,
    config: VoiceConfig,
    default_tts_dir: PathBuf,
    voice_library_dir: Option<PathBuf>,
    playful_macaw: bool,
    state: VoiceState,
    audio: Option<AudioWorker>,
    tts: Option<OfflineTts>,
    control: Option<mpsc::Sender<RuntimeCommand>>,
    events: Arc<Mutex<VecDeque<VoiceEvent>>>,
    runtime: Arc<Mutex<VoiceRuntimeStatus>>,
}

enum RuntimeCommand {
    Arm,
    Stop,
    Shutdown,
}

impl VoiceEngine {
    pub fn set_voice_library_dir(&mut self, path: PathBuf) {
        self.voice_library_dir = Some(path);
    }

    pub fn piper_voices(&self) -> Vec<PiperVoice> {
        let Some(root) = self.config.tts_dir.parent() else {
            return Vec::new();
        };
        let directories = std::fs::read_dir(root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .chain(
                self.voice_library_dir
                    .as_ref()
                    .and_then(|path| std::fs::read_dir(path).ok())
                    .into_iter()
                    .flatten()
                    .flatten(),
            );
        let mut voices: Vec<_> = directories
            .filter_map(|entry| {
                let path = entry.path();
                let model = piper_model_file(&path)?;
                let id = entry.file_name().to_string_lossy().into_owned();
                let name = model.file_stem()?.to_string_lossy().into_owned();
                let language = name
                    .split('-')
                    .next()
                    .unwrap_or("unknown")
                    .replace('_', "-");
                Some(PiperVoice { id, name, language })
            })
            .collect();
        voices.push(PiperVoice {
            id: "macaw-playful".into(),
            name: "Macaw · playful voice effect".into(),
            language: "en-US".into(),
        });
        voices.sort_by(|a, b| a.id.cmp(&b.id));
        voices
    }

    pub fn current_piper_voice(&self) -> String {
        if self.playful_macaw {
            return "macaw-playful".into();
        }
        self.config
            .tts_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn select_piper_voice(&mut self, id: &str) -> Result<()> {
        if id == self.current_piper_voice() {
            return Ok(());
        }
        if id == "macaw-playful" {
            let old_dir = self.config.tts_dir.clone();
            let old_playful = self.playful_macaw;
            self.playful_macaw = true;
            self.config.tts_dir = self.default_tts_dir.clone();
            self.tts = None;
            if let Err(error) = self.warm_up() {
                self.config.tts_dir = old_dir;
                self.playful_macaw = old_playful;
                self.tts = None;
                let _ = self.warm_up();
                return Err(error);
            }
            return Ok(());
        }
        if !self.piper_voices().iter().any(|voice| voice.id == id) {
            return Err(anyhow!(
                "This Piper voice is not installed or is incomplete: {id}"
            ));
        }
        let old_dir = self.config.tts_dir.clone();
        let old_playful = self.playful_macaw;
        let bundled = self
            .default_tts_dir
            .parent()
            .ok_or_else(|| anyhow!("Piper voice directory has no parent"))?
            .join(id);
        let optional = self.voice_library_dir.as_ref().map(|root| root.join(id));
        self.config.tts_dir = if piper_model_file(&bundled).is_some() {
            bundled
        } else {
            optional
                .filter(|path| piper_model_file(path).is_some())
                .ok_or_else(|| anyhow!("Voice package is incomplete"))?
        };
        self.playful_macaw = false;
        self.tts = None;
        if let Ok(mut status) = self.runtime.lock() {
            status.tts_ready = false;
        }
        if let Err(error) = self.warm_up() {
            self.config.tts_dir = old_dir;
            self.playful_macaw = old_playful;
            self.tts = None;
            let _ = self.warm_up();
            return Err(error);
        }
        Ok(())
    }
    /// Snapshot the model paths selected by this running application.
    pub fn config(&self) -> VoiceConfig {
        self.config.clone()
    }

    pub fn new(config: VoiceConfig) -> Self {
        let host = cpal::default_host();
        let mic = host.default_input_device().and_then(|d| d.name().ok());
        let speaker = host.default_output_device().and_then(|d| d.name().ok());
        let asr_ready = false;
        let tts_ready = false;
        let kws_ready = kws_files_present(&config);
        let runtime = VoiceRuntimeStatus {
            state: if asr_ready && tts_ready {
                "Ready".into()
            } else {
                "Unavailable".into()
            },
            always_on: false,
            wake_mode: if kws_ready {
                "kws".into()
            } else {
                "vad".into()
            },
            wake_word: "Hey Buddy".into(),
            microphone_ready: false,
            microphone: mic,
            speaker_ready: false,
            speaker,
            asr_ready,
            tts_ready,
            vad_ready: false,
            kws_ready: false,
            denoise_ready: false,
            punctuation_ready: false,
            pause_to_finish_ms: 700,
            max_listening_ms: 18_000,
            ..Default::default()
        };
        let runtime = Arc::new(Mutex::new(runtime));
        let events = Arc::new(Mutex::new(VecDeque::with_capacity(32)));
        let monitor = VoiceMonitor {
            runtime: runtime.clone(),
            events: events.clone(),
            stop: Arc::new(AtomicBool::new(false)),
            interrupt: Arc::new(AtomicBool::new(false)),
            playback: Arc::new(Mutex::new(None)),
            pause_to_finish_ms: Arc::new(AtomicU64::new(700)),
            max_listening_ms: Arc::new(AtomicU64::new(18_000)),
            control: Arc::new(Mutex::new(None)),
        };
        Self {
            monitor,
            worker: None,
            default_tts_dir: config.tts_dir.clone(),
            voice_library_dir: None,
            playful_macaw: false,
            config,
            state: if asr_ready && tts_ready {
                VoiceState::Ready
            } else {
                VoiceState::Unavailable
            },
            audio: None,
            tts: None,
            control: None,
            events,
            runtime,
        }
    }

    pub fn monitor(&self) -> VoiceMonitor {
        self.monitor.clone()
    }
    pub fn record_error(&self, error: &str) {
        if let Ok(mut s) = self.runtime.lock() {
            s.last_error = Some(error.into());
            s.state = "Error".into();
        }
        Self::push_event(
            &self.events,
            VoiceEvent {
                kind: "error".into(),
                text: Some(error.into()),
                keyword: None,
                asr_ms: None,
                at_ms: now_ms(),
            },
        );
    }
    pub fn state(&self) -> VoiceState {
        self.state.clone()
    }

    pub fn diagnostics(&self) -> serde_json::Value {
        let r = self.runtime.lock().map(|x| x.clone()).unwrap_or_default();
        json!({
            "state": r.state,
            "always_on": r.always_on,
            "wake_mode": r.wake_mode,
            "wake_word": r.wake_word,
            "microphone_ready": r.microphone_ready,
            "microphone": r.microphone,
            "speaker_ready": r.speaker_ready,
            "speaker": r.speaker,
            "asr_ready": r.asr_ready,
            "tts_ready": r.tts_ready,
            "vad_ready": r.vad_ready,
            "kws_ready": r.kws_ready,
            "denoise_ready": r.denoise_ready,
            "punctuation_ready": r.punctuation_ready,
            "last_transcript": r.last_transcript,
            "transcript_sequence": r.transcript_sequence,
            "last_voice_ms": r.last_voice_ms,
            "barge_in_count": r.barge_in_count,
            "wake_count": r.wake_count,
            "kws_decode_count": r.kws_decode_count,
            "kws_last_decode_ms": r.kws_last_decode_ms,
            "dropped_audio_buffers": r.dropped_audio_buffers,
            "input_level": r.input_level,
            "raw_input_level": r.raw_input_level,
            "input_rate": r.input_rate,
            "input_channels": r.input_channels,
            "input_frames": r.input_frames,
            "last_turn_peak": r.last_turn_peak,
            "last_turn_frames": r.last_turn_frames,
            "last_turn_hypotheses": r.last_turn_hypotheses,
            "vad_active": r.vad_active,
            "asr_partial": r.asr_partial,
            "rejected_fragments": r.rejected_fragments,
            "last_turn_outcome": r.last_turn_outcome,
            "pause_to_finish_ms": r.pause_to_finish_ms,
            "max_listening_ms": r.max_listening_ms,
            "last_error": r.last_error,
        })
    }

    fn push_event(events: &Arc<Mutex<VecDeque<VoiceEvent>>>, event: VoiceEvent) {
        if let Ok(mut q) = events.lock() {
            if q.len() >= 32 {
                q.pop_front();
            }
            q.push_back(event);
        }
    }

    pub fn take_events(&self) -> Vec<VoiceEvent> {
        if let Ok(mut q) = self.events.lock() {
            return q.drain(..).collect();
        }
        Vec::new()
    }

    pub fn arm_manual(&self) -> Result<()> {
        self.monitor.arm_manual()
    }

    pub fn stop_always_on(&self) {
        self.monitor.stop();
        if let Some(tx) = &self.control {
            let _ = tx.send(RuntimeCommand::Stop);
        }
    }

    pub fn start_always_on(&mut self) -> Result<()> {
        if let Some(worker) = &self.worker {
            if !worker.is_finished() && !self.monitor.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.control = None;
        self.monitor.stop.store(false, Ordering::SeqCst);
        if let Ok(mut status) = self.runtime.lock() {
            status.state = "Starting".into();
            status.always_on = false;
            status.microphone_ready = false;
            status.asr_ready = false;
            status.last_error = None;
        }
        if !self.config.asr_dir.exists() || !self.config.tts_dir.exists() {
            let error = format!(
                "Missing voice models. ASR: {} ; TTS: {}. Run tools/DOWNLOAD_MODELS_WINDOWS.cmd.",
                self.config.asr_dir.display(),
                self.config.tts_dir.display()
            );
            self.record_error(&error);
            return Err(anyhow!(error));
        }
        for path in [
            self.config.asr_dir.join("base.en-encoder.int8.onnx"),
            self.config.asr_dir.join("base.en-decoder.int8.onnx"),
            self.config.asr_dir.join("base.en-tokens.txt"),
            self.config.vad_model.clone(),
        ] {
            if !path.is_file() {
                let error = format!(
                    "Required voice file missing: {}. Run tools/DOWNLOAD_MODELS_WINDOWS.cmd.",
                    path.display()
                );
                self.record_error(&error);
                return Err(anyhow!(error));
            }
        }
        if self.audio.is_none() {
            self.audio = Some(AudioWorker::new()?);
        }
        let audio_control = self
            .audio
            .as_ref()
            .ok_or_else(|| anyhow!("speaker output is unavailable"))?
            .control();
        let (tx, rx) = mpsc::channel::<RuntimeCommand>();
        let config = self.config.clone();
        let events = Arc::clone(&self.events);
        let runtime = Arc::clone(&self.runtime);
        *self.monitor.playback.lock().unwrap() = Some(audio_control.clone());
        let stop = self.monitor.stop.clone();
        let interrupt = self.monitor.interrupt.clone();
        let pause_to_finish_ms = self.monitor.pause_to_finish_ms.clone();
        let max_listening_ms = self.monitor.max_listening_ms.clone();
        let worker = thread::Builder::new()
            .name("buddy-always-on-voice".into())
            .spawn(move || {
                let _guard = RuntimeGuard(runtime.clone());
                if let Ok(mut s) = runtime.lock() {
                    s.last_error = None;
                    s.always_on = false;
                    s.state = "WakeListening".into();
                }

                let host = cpal::default_host();
                let device = match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        if let Ok(mut s) = runtime.lock() {
                            s.last_error = Some("No microphone input device".into());
                            s.state = "Error".into();
                        }
                        return;
                    }
                };
                let supported = match device.default_input_config() {
                    Ok(v) => v,
                    Err(e) => {
                        if let Ok(mut s) = runtime.lock() {
                            s.last_error = Some(format!("Microphone configuration failed: {e}"));
                            s.state = "Error".into();
                        }
                        return;
                    }
                };
                let channels = supported.channels() as usize;
                let format = supported.sample_format();
                let (audio_tx, audio_rx) = mpsc::sync_channel::<Vec<f32>>(64);
                let dropped_audio = Arc::new(AtomicU64::new(0));
                let runtime_error = runtime.clone();
                let err_fn = move |e| {
                    if let Ok(mut s) = runtime_error.lock() {
                        s.last_error = Some(format!("Microphone stream: {e:?}"));
                        s.microphone_ready = false;
                        s.state = "Error".into();
                    }
                };
                let cfg = supported.config();
                let stream = match format {
                    SampleFormat::F32 => { let dropped_audio = dropped_audio.clone(); device.build_input_stream(
                        &cfg,
                        move |d: &[f32], _| {
                            let mono = microphone_mono(d, channels, |sample| sample);
                            if audio_tx.try_send(mono).is_err() { dropped_audio.fetch_add(1, Ordering::Relaxed); }
                        },
                        err_fn,
                        None,
                    ) },
                    SampleFormat::I16 => { let dropped_audio = dropped_audio.clone(); device.build_input_stream(
                        &cfg,
                        move |d: &[i16], _| {
                            let mono = microphone_mono(d, channels, |sample| sample as f32 / 32768.0);
                            if audio_tx.try_send(mono).is_err() { dropped_audio.fetch_add(1, Ordering::Relaxed); }
                        },
                        err_fn,
                        None,
                    ) },
                    SampleFormat::U16 => { let dropped_audio = dropped_audio.clone(); device.build_input_stream(
                        &cfg,
                        move |d: &[u16], _| {
                            let mono = microphone_mono(d, channels, |sample| (sample as f32 - 32768.0) / 32768.0);
                            if audio_tx.try_send(mono).is_err() { dropped_audio.fetch_add(1, Ordering::Relaxed); }
                        },
                        err_fn,
                        None,
                    ) },
                    _ => {
                        if let Ok(mut s) = runtime.lock() {
                            s.last_error =
                                Some(format!("Unsupported microphone format: {format:?}"));
                            s.state = "Error".into();
                        }
                        return;
                    }
                };
                let stream = match stream {
                    Ok(stream) => stream,
                    Err(e) => {
                        if let Ok(mut s) = runtime.lock() {
                            s.last_error = Some(format!("Microphone stream failed: {e}"));
                            s.state = "Error".into();
                        }
                        return;
                    }
                };
                if let Err(e) = stream.play() {
                    if let Ok(mut s) = runtime.lock() {
                        s.last_error = Some(format!("Microphone start failed: {e}"));
                        s.state = "Error".into();
                    }
                    return;
                }

                let mic_rate = supported.sample_rate().0 as i32;
                const ASR_RATE: i32 = 16_000;
                let resampler = if mic_rate != ASR_RATE {
                    LinearResampler::create(mic_rate, ASR_RATE)
                } else {
                    None
                };

                let recognizer = match build_whisper(&config) {
                    Some(v) => v,
                    None => {
                        if let Ok(mut s) = runtime.lock() {
                            s.last_error = Some("Whisper ASR model is incomplete or failed to load".into());
                            s.state = "Error".into();
                        }
                        return;
                    }
                };

                let vad = build_vad(&config);
                let denoiser = build_denoiser(&config);
                let kws = build_kws(&config);

                if let Ok(mut s) = runtime.lock() {
                    s.always_on = true;
                    s.microphone_ready = true;
                    s.input_rate = mic_rate as u32;
                    s.input_channels = channels as u16;
                    s.asr_ready = true;
                    s.vad_ready = vad.is_some();
                    s.kws_ready = kws.is_some();
                    s.denoise_ready = denoiser.is_some();
                    s.punctuation_ready = true; // Whisper includes punctuation in its transcript.
                    s.wake_mode = if kws.is_some() {
                        "kws".into()
                    } else {
                        "vad".into()
                    };
                    s.state = "WakeListening".into();
                }

                let mut kws_stream = kws.as_ref().map(|k| k.create_stream());
                let mut active = false;
                let mut wake_turn = false;
                let mut force_arm = false;
                let mut speech_started = false;
                let mut started_at = Instant::now();
                let mut resample_buffer = Vec::<f32>::new();
                let mut enhance_buffer = Vec::<f32>::new();
                let mut last_speech = Instant::now();
                let mut barge_frames = 0u8;
                let mut turn_stats = TurnCaptureStats::default();
                let mut turn_audio = Vec::<f32>::new();
                let mut last_asr_attempt = 0usize;
                // KWS remains the low-latency primary wake path. This short VAD-gated
                // Whisper probe is an independent fallback for microphones/accents on
                // which the small keyword model misses a clearly spoken wake phrase.
                let mut wake_probe_audio = Vec::<f32>::new();
                let mut wake_probe_active = false;
                let mut wake_probe_last_speech = Instant::now();
                // Keep a short conversational window open after a completed turn.
                // During this window a clearly detected follow-up does not require
                // repeating the wake phrase, while sustained VAD prevents a single
                // click or background sound from opening ASR.
                let mut follow_up_until: Option<Instant> = None;
                let mut follow_up_speech_frames = 0u8;

                loop {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    if runtime
                        .lock()
                        .map(|status| status.state == "Error")
                        .unwrap_or(true)
                    {
                        return;
                    }
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            RuntimeCommand::Arm => {
                                force_arm = true;
                                wake_turn = false;
                                // A manual turn must feed ASR from the moment Talk is
                                // pressed. Waiting for VAD made quiet speakers invisible.
                                active = true;
                                speech_started = false;
                                started_at = Instant::now();
                                last_speech = Instant::now();
                                turn_stats = TurnCaptureStats::default();
                                turn_audio.clear();
                                last_asr_attempt = 0;
                                if let Ok(mut s) = runtime.lock() {
                                    s.state = "Listening".into();
                                    s.asr_partial = None;
                                    s.last_turn_outcome = Some("Manual capture started".into());
                                }
                            }
                            RuntimeCommand::Stop => {
                                if let Ok(mut s) = runtime.lock() {
                                    s.always_on = false;
                                    s.state = "Stopped".into();
                                }
                                return;
                            }
                            RuntimeCommand::Shutdown => return,
                        }
                    }

                    let raw = match audio_rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(v) => v,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if active && started_at.elapsed() > Duration::from_millis(max_listening_ms.load(Ordering::SeqCst)) {
                                let manual_timeout = force_arm;
                                active = false;
                                force_arm = false;
                                speech_started = false;
                                if let Ok(mut status) = runtime.lock() {
                                    status.state = "WakeListening".into();
                                    status.asr_partial = None;
                                    status.last_turn_outcome = Some("Microphone stream stalled during Talk".into());
                                    turn_stats.store(&mut status);
                                }
                                if manual_timeout { Self::push_event(&events, VoiceEvent {
                                    kind: "timeout".into(),
                                    text: Some("The microphone stopped delivering audio. Check the input device, then try Talk again.".into()),
                                    keyword: None,
                                    asr_ms: Some(started_at.elapsed().as_millis() as u64),
                                    at_ms: now_ms(),
                                }); }
                            }
                            continue;
                        },
                        Err(_) => return,
                    };
                    let raw_level = raw.iter().map(|sample| sample.abs()).sum::<f32>() / raw.len().max(1) as f32;
                    if active { turn_stats.accept(&raw); }
                    if let Ok(mut status) = runtime.lock() {
                        status.raw_input_level = raw_level;
                        status.input_frames = status.input_frames.saturating_add(raw.len() as u64);
                        status.dropped_audio_buffers = dropped_audio.load(Ordering::Relaxed);
                    }
                    if let Some(r) = &resampler {
                        resample_buffer.extend_from_slice(&r.resample(&raw, false));
                    } else {
                        resample_buffer.extend_from_slice(&raw);
                    }

                    while resample_buffer.len() >= 512 {
                        let chunk: Vec<f32> = resample_buffer.drain(..512).collect();
                        // KWS consumes the clean, resampled mic signal immediately.
                        // GTCRN runs later for VAD; placing KWS behind denoising added
                        // avoidable latency and could distort the short wake phrase.
                        let mut wake_triggered = false;
                        if let (Some(k), Some(s)) = (&kws, kws_stream.as_ref()) {
                            if !active && !force_arm {
                                s.accept_waveform(ASR_RATE, &chunk);
                                while k.is_ready(s) {
                                    let decode_start = Instant::now();
                                    k.decode(s);
                                    if let Ok(mut st) = runtime.lock() {
                                        st.kws_decode_count = st.kws_decode_count.saturating_add(1);
                                        st.kws_last_decode_ms = decode_start.elapsed().as_millis() as u64;
                                    }
                                    if let Some(result) = k.get_result(s) {
                                        if !result.keyword.is_empty() {
                                            wake_triggered = true;
                                            k.reset(s);
                                            active = true;
                                            wake_turn = true;
                                            speech_started = false;
                                            started_at = Instant::now();
                                            last_speech = Instant::now();
                                            turn_stats = TurnCaptureStats::default();
                                            turn_audio.clear();
                                            last_asr_attempt = 0;
                                            let was_speaking = runtime.lock().map(|st| st.state == "Speaking" || st.state == "Synthesizing").unwrap_or(false);
                                            if was_speaking {
                                                interrupt.store(true, Ordering::SeqCst);
                                                audio_control.stop();
                                            }
                                            if let Ok(mut st) = runtime.lock() {
                                                st.state = "Listening".into();
                                                st.wake_count = st.wake_count.saturating_add(1);
                                                if was_speaking { st.barge_in_count = st.barge_in_count.saturating_add(1); }
                                            }
                                            if was_speaking {
                                                Self::push_event(&events, VoiceEvent {
                                                    kind: "barge_in".into(), text: None, keyword: None,
                                                    asr_ms: None, at_ms: now_ms(),
                                                });
                                            }
                                            Self::push_event(&events, VoiceEvent {
                                                kind: "wake".into(),
                                                text: None,
                                                keyword: Some(result.keyword),
                                                asr_ms: None,
                                                at_ms: now_ms(),
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if active && turn_audio.len() < 16_000 * 31 {
                            turn_audio.extend_from_slice(&chunk);
                        }
                        enhance_buffer.extend_from_slice(&chunk);
                        let mut process_chunks: Vec<Vec<f32>> = Vec::new();
                        if let Some(d) = denoiser.as_ref() {
                            let shift = d.frame_shift_in_samples() as usize;
                            if shift > 0 {
                                while enhance_buffer.len() >= shift {
                                    let part: Vec<f32> = enhance_buffer.drain(..shift).collect();
                                    process_chunks.push(d.run(&part, ASR_RATE).samples);
                                }
                            } else {
                                process_chunks.push(enhance_buffer.split_off(0));
                            }
                        } else {
                            process_chunks.push(chunk);
                        }

                        for audio in process_chunks {
                            // Feed VAD exactly once per audio chunk. This is important for
                            // true barge-in: the user may start speaking while Buddy is
                            // playing TTS, before an ASR session is marked active.
                            let vad_detected = if let Some(v) = vad.as_ref() {
                                v.accept_waveform(&audio);
                                v.detected()
                            } else {
                                false
                            };

                            let energy = audio.iter().map(|x| x.abs()).sum::<f32>()
                                / audio.len().max(1) as f32;
                            let speaking_now = runtime.lock().map(|mut s| {
                                s.input_level = energy;
                                s.vad_active = vad_detected;
                                s.state == "Speaking" || s.state == "Synthesizing"
                            }).unwrap_or(false);

                            // BARGE-IN IS INDEPENDENT OF `active`.
                            // Previous versions checked this only inside the active
                            // transcription branch, which meant KWS/wake-mode could
                            // miss an interruption while Buddy was speaking.
                            if speaking_now {
                                if kws.is_some() && wake_triggered {
                                    barge_frames = 24;
                                } else if kws.is_none() && vad_detected && energy > 0.025 {
                                    barge_frames = barge_frames.saturating_add(1);
                                } else {
                                    barge_frames = 0;
                                }
                                // In wake-word mode, require the wake word. VAD-only mode
                                // requires sustained speech before interrupting playback.
                                if barge_frames >= 24 {
                                    interrupt.store(true, Ordering::SeqCst);
                                    audio_control.stop();
                                    barge_frames = 0;
                                    active = true;
                                    force_arm = false;
                                    speech_started = true;
                                    started_at = Instant::now();
                                    last_speech = Instant::now();
                                    turn_stats = TurnCaptureStats::default();
                                    turn_audio.clear();
                                    last_asr_attempt = 0;
                                    if let Ok(mut s) = runtime.lock() {
                                        s.barge_in_count += 1;
                                        s.state = "Listening".into();
                                    }
                                    Self::push_event(
                                        &events,
                                        VoiceEvent {
                                            kind: "barge_in".into(),
                                            text: None,
                                            keyword: None,
                                            asr_ms: None,
                                            at_ms: now_ms(),
                                        },
                                    );
                                }
                            } else {
                                barge_frames = 0;
                            }

                            if !active && !speaking_now && kws.is_some() && !wake_triggered {
                                let probe_speech = vad_detected || energy > 0.010;
                                if probe_speech {
                                    if !wake_probe_active {
                                        wake_probe_audio.clear();
                                        wake_probe_active = true;
                                    }
                                    wake_probe_last_speech = Instant::now();
                                }
                                if wake_probe_active && wake_probe_audio.len() < 16_000 * 7 {
                                    wake_probe_audio.extend_from_slice(&audio);
                                }
                                let probe_finished = wake_probe_active
                                    && !probe_speech
                                    && wake_probe_last_speech.elapsed() > Duration::from_millis(280);
                                let probe_full = wake_probe_audio.len() >= 16_000 * 7;
                                if (probe_finished || probe_full) && wake_probe_audio.len() >= 6_400 {
                                    let probe_started = Instant::now();
                                    let heard = decode_whisper(&recognizer, &wake_probe_audio);
                                    if let Some(request) = wake_request(&heard) {
                                        if let Ok(mut st) = runtime.lock() {
                                            st.wake_count = st.wake_count.saturating_add(1);
                                            st.last_turn_outcome = Some("Wake phrase verified by Whisper fallback".into());
                                        }
                                        Self::push_event(&events, VoiceEvent {
                                            kind: "wake".into(), text: None,
                                            keyword: Some("HEY_BUDDY_WHISPER".into()),
                                            asr_ms: Some(probe_started.elapsed().as_millis() as u64),
                                            at_ms: now_ms(),
                                        });
                                        if usable_transcript(&request) {
                                            let text = request.trim().to_string();
                                            if let Ok(mut st) = runtime.lock() {
                                                st.state = "WakeListening".into();
                                                st.last_transcript = Some(text.clone());
                                                st.transcript_sequence = st.transcript_sequence.saturating_add(1);
                                                st.last_voice_ms = Some(probe_started.elapsed().as_millis() as u64);
                                                st.last_turn_outcome = Some("Wake phrase and request accepted by Whisper fallback".into());
                                            }
                                            Self::push_event(&events, VoiceEvent {
                                                kind: "transcript".into(), text: Some(text), keyword: None,
                                                asr_ms: Some(probe_started.elapsed().as_millis() as u64), at_ms: now_ms(),
                                            });
                                            if let Some(k) = &kws { kws_stream = Some(k.create_stream()); }
                                        } else {
                                            active = true;
                                            wake_turn = false;
                                            speech_started = false;
                                            started_at = Instant::now();
                                            last_speech = Instant::now();
                                            turn_stats = TurnCaptureStats::default();
                                            turn_audio.clear();
                                            last_asr_attempt = 0;
                                            if let Ok(mut st) = runtime.lock() { st.state = "Listening".into(); }
                                        }
                                    }
                                    wake_probe_audio.clear();
                                    wake_probe_active = false;
                                }
                            } else if active || speaking_now || wake_triggered {
                                wake_probe_audio.clear();
                                wake_probe_active = false;
                            }

                            if speaking_now {
                                // Start the follow-up allowance after Buddy finishes,
                                // rather than consuming it while TTS is still playing.
                                follow_up_until = Some(Instant::now() + Duration::from_secs(12));
                                follow_up_speech_frames = 0;
                            } else if !active
                                && follow_up_until.is_some_and(|until| Instant::now() < until)
                            {
                                if vad_detected && energy > 0.010 {
                                    follow_up_speech_frames = follow_up_speech_frames.saturating_add(1);
                                } else {
                                    follow_up_speech_frames = 0;
                                }
                                if follow_up_speech_frames >= 3 {
                                    active = true;
                                    wake_turn = false;
                                    force_arm = false;
                                    speech_started = true;
                                    started_at = Instant::now();
                                    last_speech = Instant::now();
                                    turn_stats = TurnCaptureStats::default();
                                    turn_audio.clear();
                                    let pre_roll_start = wake_probe_audio.len().saturating_sub(16_000);
                                    turn_audio.extend_from_slice(&wake_probe_audio[pre_roll_start..]);
                                    turn_stats.accept(&turn_audio);
                                    last_asr_attempt = 0;
                                    follow_up_speech_frames = 0;
                                    follow_up_until = None;
                                    if let Ok(mut st) = runtime.lock() {
                                        st.state = "Listening".into();
                                        st.last_turn_outcome = Some("Conversational follow-up detected".into());
                                    }
                                }
                            } else {
                                follow_up_speech_frames = 0;
                                if follow_up_until.is_some_and(|until| Instant::now() >= until) {
                                    follow_up_until = None;
                                }
                            }

                            if !active && (force_arm || kws.is_none()) {
                                if vad_detected {
                                    active = true;
                                    wake_turn = false;
                                    speech_started = true;
                                    force_arm = false;
                                    started_at = Instant::now();
                                    last_speech = Instant::now();
                                    turn_stats = TurnCaptureStats::default();
                                    turn_audio.clear();
                                    last_asr_attempt = 0;
                                    if let Ok(mut st) = runtime.lock() {
                                        st.state = "Listening".into();
                                    }
                                } else if force_arm && vad.is_none() {
                                    active = true;
                                    wake_turn = false;
                                    speech_started = true;
                                    force_arm = false;
                                    started_at = Instant::now();
                                    last_speech = Instant::now();
                                    turn_stats = TurnCaptureStats::default();
                                    turn_audio.clear();
                                    last_asr_attempt = 0;
                                }
                            }

                            if active {
                                if vad_detected || energy > 0.012 {
                                    speech_started = true;
                                    last_speech = Instant::now();
                                } else if vad.is_none() {
                                    speech_started = true;
                                }

                                let elapsed_limit = started_at.elapsed()
                                    > Duration::from_millis(max_listening_ms.load(Ordering::SeqCst));
                                let pause_reached = speech_started
                                    && last_speech.elapsed()
                                        > Duration::from_millis(pause_to_finish_ms.load(Ordering::SeqCst));
                                let enough_new_audio = last_asr_attempt == 0
                                    || turn_audio.len().saturating_sub(last_asr_attempt) >= 16_000;
                                if (pause_reached || elapsed_limit)
                                    && speech_started
                                    && turn_stats.peak > 0.01
                                    && turn_audio.len() >= 8_000
                                    && enough_new_audio
                                {
                                    last_asr_attempt = turn_audio.len();
                                    if let Ok(mut status) = runtime.lock() {
                                        status.state = "Transcribing".into();
                                    }
                                    let raw_transcript = decode_whisper(&recognizer, &turn_audio);
                                    let transcript = if wake_turn {
                                        strip_wake_prefix(&raw_transcript)
                                    } else {
                                        raw_transcript
                                    };
                                    if !transcript.trim().is_empty() {
                                        turn_stats.hypotheses = turn_stats.hypotheses.saturating_add(1);
                                    }
                                    if usable_transcript(&transcript) {
                                        let elapsed = started_at.elapsed().as_millis() as u64;
                                        let text = transcript.trim().to_string();
                                        if let Ok(mut status) = runtime.lock() {
                                            status.state = "WakeListening".into();
                                            status.last_transcript = Some(text.clone());
                                            status.transcript_sequence =
                                                status.transcript_sequence.saturating_add(1);
                                            status.last_voice_ms = Some(elapsed);
                                            status.asr_partial = None;
                                            status.last_turn_outcome =
                                                Some("Transcript accepted by Whisper ASR".into());
                                            turn_stats.store(&mut status);
                                        }
                                        Self::push_event(&events, VoiceEvent {
                                            kind: "transcript".into(),
                                            text: Some(text),
                                            keyword: None,
                                            asr_ms: Some(elapsed),
                                            at_ms: now_ms(),
                                        });
                                        active = false;
                                        wake_turn = false;
                                        force_arm = false;
                                        speech_started = false;
                                        turn_audio.clear();
                                        follow_up_until = Some(Instant::now() + Duration::from_secs(12));
                                        if let Some(k) = &kws {
                                            kws_stream = Some(k.create_stream());
                                        }
                                    } else if let Ok(mut status) = runtime.lock() {
                                        status.state = "Listening".into();
                                        status.last_turn_outcome =
                                            Some("Whisper ASR found no clear request yet".into());
                                        status.rejected_fragments =
                                            status.rejected_fragments.saturating_add(1);
                                    }
                                }

                                if active && elapsed_limit {
                                    let manual_timeout = force_arm;
                                    let had_speech = speech_started;
                                    active = false;
                                    wake_turn = false;
                                    force_arm = false;
                                    speech_started = false;
                                    turn_audio.clear();
                                    if let Some(k) = &kws {
                                        kws_stream = Some(k.create_stream());
                                    }
                                    if let Ok(mut status) = runtime.lock() {
                                        status.state = "WakeListening".into();
                                        status.asr_partial = None;
                                        turn_stats.store(&mut status);
                                        status.last_turn_outcome = Some(if turn_stats.frames == 0 {
                                            "No microphone frames reached the voice runtime"
                                        } else if turn_stats.peak < 0.003 {
                                            "Microphone signal was too quiet"
                                        } else if !had_speech {
                                            "Timed out without detected speech"
                                        } else {
                                            "Whisper ASR heard no clear request"
                                        }.into());
                                    }
                                    if manual_timeout {
                                        Self::push_event(&events, VoiceEvent {
                                            kind: "timeout".into(),
                                            text: Some("I didn't catch that clearly. Could you say it again?".into()),
                                            keyword: None,
                                            asr_ms: Some(started_at.elapsed().as_millis() as u64),
                                            at_ms: now_ms(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .map_err(|e| anyhow!("voice runtime spawn failed: {e}"))?;
        self.worker = Some(worker);
        if let Ok(mut control) = self.monitor.control.lock() {
            *control = Some(tx.clone());
        }
        self.control = Some(tx);
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let status = self
                .runtime
                .lock()
                .map_err(|_| anyhow!("voice status unavailable"))?
                .clone();
            if status.always_on && status.microphone_ready && status.asr_ready {
                return Ok(());
            }
            if status.state == "Error"
                || self
                    .worker
                    .as_ref()
                    .is_some_and(|worker| worker.is_finished())
            {
                return Err(anyhow!(status
                    .last_error
                    .unwrap_or_else(|| "Voice runtime stopped during startup".into())));
            }
            if Instant::now() >= deadline {
                let error = "Voice startup timed out before microphone and recognizer became ready";
                self.stop_always_on();
                self.record_error(error);
                return Err(anyhow!(error));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn warm_up(&mut self) -> Result<()> {
        for name in [
            "tokens.txt",
            "espeak-ng-data/phontab",
            "espeak-ng-data/phonindex",
            "espeak-ng-data/phondata",
            "espeak-ng-data/intonations",
        ] {
            if !self.config.tts_dir.join(name).exists() {
                return Err(anyhow!(
                    "TTS support asset missing: {}",
                    self.config.tts_dir.join(name).display()
                ));
            }
        }
        let Some(tts_model) = piper_model_file(&self.config.tts_dir) else {
            return Err(anyhow!(
                "Piper voice model or support files missing at {}.",
                self.config.tts_dir.display()
            ));
        };
        if self.audio.is_none() {
            self.audio = Some(AudioWorker::new()?);
        }
        if self.tts.is_none() && self.config.tts_dir.exists() {
            let d = &self.config.tts_dir;
            let cfg = OfflineTtsConfig {
                model: sherpa_onnx::OfflineTtsModelConfig {
                    vits: OfflineTtsVitsModelConfig {
                        model: Some(tts_model.to_string_lossy().into_owned()),
                        tokens: Some(d.join("tokens.txt").to_string_lossy().into_owned()),
                        data_dir: Some(d.join("espeak-ng-data").to_string_lossy().into_owned()),
                        ..Default::default()
                    },
                    num_threads: self.config.num_threads,
                    debug: false,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.tts = OfflineTts::create(&cfg);
            if self.tts.is_none() {
                return Err(anyhow!(
                    "TTS engine creation failed with model={} tokens={} espeak={}",
                    tts_model.display(),
                    d.join("tokens.txt").display(),
                    d.join("espeak-ng-data").display()
                ));
            }
        }
        if let Ok(mut r) = self.runtime.lock() {
            r.tts_ready = self.tts.is_some();
            r.speaker_ready = self.audio.is_some();
        }
        if let Some(a) = &self.audio {
            *self.monitor.playback.lock().unwrap() = Some(a.control());
        }
        Ok(())
    }

    pub fn speak(&mut self, text: &str) -> Result<()> {
        self.speak_with_speed(text, 1.0)
    }

    pub fn speak_adaptive(&mut self, text: &str, request: &str, capability: &str) -> Result<()> {
        let (preferred, speed) = adaptive_voice_plan(request, capability);
        let default = self
            .default_tts_dir
            .file_name()
            .ok_or_else(|| anyhow!("Missing default voice"))?
            .to_string_lossy()
            .into_owned();
        let selected = if !preferred.is_empty()
            && self
                .piper_voices()
                .iter()
                .any(|voice| voice.id == preferred)
        {
            preferred
        } else {
            &default
        };
        self.select_piper_voice(selected)?;
        self.speak_with_speed(text, speed)
    }

    fn speak_with_speed(&mut self, text: &str, speed: f32) -> Result<()> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.monitor.interrupt.store(false, Ordering::SeqCst);
        self.warm_up()?;
        if let Ok(mut s) = self.runtime.lock() {
            s.state = "Synthesizing".into();
        }
        self.state = VoiceState::Synthesizing;
        let tts = self.tts.as_ref().ok_or_else(|| anyhow!("TTS not ready"))?;
        let interrupted = self.monitor.interrupt.clone();
        let generated = tts.generate_with_config(
            text,
            &GenerationConfig {
                sid: 0,
                speed: if self.playful_macaw {
                    speed * 0.93
                } else {
                    speed
                },
                ..Default::default()
            },
            Some(move |_: &[f32], _: f32| !interrupted.load(Ordering::SeqCst)),
        );
        if self.monitor.interrupt.load(Ordering::SeqCst) {
            self.state = VoiceState::Ready;
            return Ok(());
        }
        let audio = generated.ok_or_else(|| anyhow!("TTS generation failed"))?;
        let tmp = std::env::temp_dir().join(format!(
            "vce_buddy_tts_{}_{}.wav",
            std::process::id(),
            now_ms()
        ));
        let saved = if self.playful_macaw {
            save_macaw_effect(&tmp, audio.samples(), audio.sample_rate())
        } else {
            audio.save(tmp.to_string_lossy().as_ref())
        };
        if !saved {
            return Err(anyhow!("TTS WAV save failed"));
        }
        if let Ok(mut s) = self.runtime.lock() {
            s.state = "Speaking".into();
        }
        self.state = VoiceState::Speaking;
        let result = self
            .audio
            .as_ref()
            .ok_or_else(|| anyhow!("speaker output is unavailable"))?
            .play(tmp.clone());
        let _ = std::fs::remove_file(&tmp);
        if let Ok(mut s) = self.runtime.lock() {
            s.state = if self.monitor.interrupt.load(Ordering::SeqCst) {
                if s.always_on {
                    "Listening".into()
                } else {
                    "Stopped".into()
                }
            } else if result.is_ok() {
                if s.always_on {
                    "WakeListening".into()
                } else {
                    "Ready".into()
                }
            } else {
                "Error".into()
            };
        }
        self.state = if result.is_ok() {
            VoiceState::Ready
        } else {
            VoiceState::Error("Audio playback failed".into())
        };
        result
    }

    pub fn stop_playback(&self) {
        if let Some(a) = &self.audio {
            a.control().stop();
        }
    }
}

fn build_whisper(config: &VoiceConfig) -> Option<OfflineRecognizer> {
    let dir = &config.asr_dir;
    let encoder = dir.join("base.en-encoder.int8.onnx");
    let decoder = dir.join("base.en-decoder.int8.onnx");
    let tokens = dir.join("base.en-tokens.txt");
    let complete = [
        (&encoder, 20_000_000),
        (&decoder, 100_000_000),
        (&tokens, 500_000),
    ]
    .iter()
    .all(|(path, minimum)| path.metadata().is_ok_and(|m| m.len() >= *minimum));
    if !complete {
        return None;
    }
    let mut c = OfflineRecognizerConfig::default();
    c.model_config.whisper.encoder = Some(encoder.to_string_lossy().into_owned());
    c.model_config.whisper.decoder = Some(decoder.to_string_lossy().into_owned());
    c.model_config.whisper.language = Some("en".into());
    c.model_config.whisper.task = Some("transcribe".into());
    c.model_config.whisper.tail_paddings = -1;
    c.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
    c.model_config.provider = Some("cpu".into());
    c.model_config.num_threads = config.num_threads;
    OfflineRecognizer::create(&c)
}

fn decode_whisper(recognizer: &OfflineRecognizer, samples: &[f32]) -> String {
    if samples.is_empty() {
        return String::new();
    }
    let stream = recognizer.create_stream();
    stream.accept_waveform(16_000, samples);
    recognizer.decode(&stream);
    stream
        .get_result()
        .map(|result| result.text)
        .unwrap_or_default()
}

fn build_vad(config: &VoiceConfig) -> Option<VoiceActivityDetector> {
    if !config.vad_model.exists() {
        return None;
    }
    let mut c = VadModelConfig::default();
    c.silero_vad.model = Some(config.vad_model.to_string_lossy().into_owned());
    c.silero_vad.threshold = 0.48;
    c.silero_vad.min_silence_duration = 0.18;
    c.silero_vad.min_speech_duration = 0.20;
    c.silero_vad.max_speech_duration = 12.0;
    c.silero_vad.window_size = 512;
    c.sample_rate = 16_000;
    c.num_threads = 1;
    c.provider = Some("cpu".into());
    VoiceActivityDetector::create(&c, 20.0)
}

fn build_kws(config: &VoiceConfig) -> Option<KeywordSpotter> {
    if !kws_files_present(config) {
        return None;
    }
    let encoder = config
        .kws_dir
        .join("encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx");
    let decoder = config
        .kws_dir
        .join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx");
    let joiner = config
        .kws_dir
        .join("joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx");
    let tokens = config.kws_dir.join("tokens.txt");
    let keywords = config.kws_keywords.clone();
    let mut c = KeywordSpotterConfig::default();
    c.model_config.transducer.encoder = Some(encoder.to_string_lossy().into_owned());
    c.model_config.transducer.decoder = Some(decoder.to_string_lossy().into_owned());
    c.model_config.transducer.joiner = Some(joiner.to_string_lossy().into_owned());
    c.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
    c.model_config.provider = Some("cpu".into());
    c.model_config.num_threads = 1;
    c.max_active_paths = 4;
    c.keywords_score = 3.0;
    c.keywords_threshold = 0.08;
    c.keywords_file = Some(keywords.to_string_lossy().into_owned());
    KeywordSpotter::create(&c)
}

fn kws_files_present(config: &VoiceConfig) -> bool {
    [
        config
            .kws_dir
            .join("encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx"),
        config
            .kws_dir
            .join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
        config
            .kws_dir
            .join("joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx"),
        config.kws_dir.join("tokens.txt"),
        config.kws_keywords.clone(),
    ]
    .iter()
    .all(|path| {
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    })
}

fn build_denoiser(config: &VoiceConfig) -> Option<OnlineSpeechDenoiser> {
    if !config.denoise_model.exists() {
        return None;
    }
    let c = OnlineSpeechDenoiserConfig {
        model: OfflineSpeechDenoiserModelConfig {
            gtcrn: OfflineSpeechDenoiserGtcrnModelConfig {
                model: Some(config.denoise_model.to_string_lossy().into_owned()),
            },
            ..Default::default()
        },
    };
    OnlineSpeechDenoiser::create(&c)
}

// Whisper can include the wake phrase in the first turn. Route only the
// request to the composer, and keep listening when the phrase was said alone.
fn strip_wake_prefix(text: &str) -> String {
    let trimmed = text.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    for prefix in [
        "hey buddy",
        "hey, buddy",
        "hi buddy",
        "hi, buddy",
        "hello buddy",
        "hello, buddy",
    ] {
        if lower.starts_with(prefix) {
            let rest = &trimmed[prefix.len()..];
            if rest.chars().next().is_none_or(|c| !c.is_alphabetic()) {
                return rest
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .trim()
                    .to_string();
            }
        }
    }
    trimmed.to_string()
}

fn wake_request(text: &str) -> Option<String> {
    let stripped = strip_wake_prefix(text);
    (stripped.trim() != text.trim()).then_some(stripped)
}

fn usable_transcript(text: &str) -> bool {
    let words: Vec<_> = text
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect();
    if words.len() == 1 {
        return matches!(
            words[0].as_str(),
            "hi" | "yes"
                | "no"
                | "stop"
                | "wait"
                | "next"
                | "play"
                | "pause"
                | "continue"
                | "news"
                | "music"
                | "video"
                | "youtube"
                | "weather"
                | "hello"
        );
    }
    if matches!(
        words.join(" ").as_str(),
        "read news"
            | "play music"
            | "play video"
            | "open video"
            | "open youtube"
            | "play news video"
            | "play a news video"
            | "play latest video"
            | "play the latest video"
            | "play latest news video"
            | "play news video from youtube"
            | "play news video on youtube"
            | "stop video"
            | "stop the video"
            | "stop playing video"
            | "pause video"
            | "close video"
            | "stop music"
            | "pause music"
            | "resume music"
            | "next track"
            | "next song"
            | "skip song"
            | "mute music"
            | "unmute music"
            | "open news"
            | "go on"
    ) {
        return true;
    }
    words.len() >= 2
        && words
            .iter()
            .filter(|word| word.chars().count() == 1)
            .count()
            <= 1
        && words.iter().map(|word| word.chars().count()).sum::<usize>() >= 10
}

fn adaptive_voice_plan(request: &str, capability: &str) -> (&'static str, f32) {
    let request = request.to_lowercase();
    let gentle = capability.contains("conversation.support")
        || [
            "sad",
            "lonely",
            "anxious",
            "stressed",
            "worried",
            "frustrated",
            "cheer me up",
            "encourage me",
        ]
        .iter()
        .any(|word| request.contains(word));
    if capability.contains("briefing") || capability.contains("chatter") {
        ("vits-piper-en_US-kristin-medium", 0.94)
    } else if gentle {
        ("vits-piper-en_US-kristin-medium", 0.91)
    } else if capability.contains("joke") {
        ("macaw-playful", 0.97)
    } else if capability.contains("health") {
        ("vits-piper-en_US-kristin-medium", 0.94)
    } else if capability.contains("science") {
        ("vits-piper-en_US-joe-medium", 1.01)
    } else if capability.contains("story") || capability.contains("audiobook") {
        ("vits-piper-en_GB-cori-medium", 0.93)
    } else if capability.contains("thought") {
        ("vits-piper-en_US-john-medium", 0.92)
    } else {
        ("", 1.0)
    }
}

#[cfg(test)]
mod transcript_tests {
    use super::*;

    #[test]
    fn adaptive_voice_follows_reply_and_explicit_mood() {
        assert_eq!(
            adaptive_voice_plan("Tell me a joke", "persona.joke").0,
            "macaw-playful"
        );
        assert!(adaptive_voice_plan("I feel lonely", "conversation.support").1 < 1.0);
        assert_eq!(
            adaptive_voice_plan("yes please", "conversation.support_story").0,
            "vits-piper-en_US-kristin-medium"
        );
        assert_eq!(adaptive_voice_plan("read news", "news.read_visible").0, "");
    }

    #[test]
    fn incomplete_kws_directory_is_not_reported_ready() {
        let root = std::env::temp_dir().join(format!(
            "buddy-kws-check-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = VoiceConfig::from_env();
        config.kws_dir = root.clone();
        config.kws_keywords = root.join("keywords.txt");
        std::fs::write(root.join("tokens.txt"), b"tokens").unwrap();
        std::fs::write(&config.kws_keywords, b"HEY BUDDY").unwrap();
        assert!(!kws_files_present(&config));
        for name in [
            "encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx",
            "decoder-epoch-12-avg-2-chunk-16-left-64.onnx",
            "joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx",
        ] {
            std::fs::write(root.join(name), b"fixture").unwrap();
        }
        assert!(kws_files_present(&config));
        std::fs::remove_file(root.join("encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx"))
            .unwrap();
        assert!(!kws_files_present(&config));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires downloaded KWS model"]
    fn bundled_keyword_spotter_initializes() {
        let config = VoiceConfig::from_env();
        assert!(kws_files_present(&config));
        assert!(
            build_kws(&config).is_some(),
            "sherpa KWS model failed to initialize"
        );
    }

    #[test]
    #[ignore = "requires downloaded KWS and Piper models"]
    fn piper_hey_buddy_replays_through_live_kws_path() {
        let config = VoiceConfig::from_env();
        let model = piper_model_file(&config.tts_dir).expect("Piper model");
        let tts = OfflineTts::create(&OfflineTtsConfig {
            model: sherpa_onnx::OfflineTtsModelConfig {
                vits: OfflineTtsVitsModelConfig {
                    model: Some(model.to_string_lossy().into_owned()),
                    tokens: Some(
                        config
                            .tts_dir
                            .join("tokens.txt")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    data_dir: Some(
                        config
                            .tts_dir
                            .join("espeak-ng-data")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    ..Default::default()
                },
                num_threads: 2,
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("Piper starts");
        let generated = tts
            .generate_with_config(
                "Hey Buddy",
                &GenerationConfig::default(),
                None::<fn(&[f32], f32) -> bool>,
            )
            .expect("Piper audio");
        let resampler =
            LinearResampler::create(generated.sample_rate(), 16_000).expect("resampler");
        let kws = build_kws(&config).expect("KWS starts");
        let stream = kws.create_stream();
        let mut detected = false;
        let mut input = vec![0.0f32; generated.sample_rate() as usize / 2];
        input.extend_from_slice(generated.samples());
        input.extend(vec![0.0f32; generated.sample_rate() as usize]);
        let samples = resampler.resample(&input, true);
        let speech_end_ms =
            500 + generated.samples().len() as u64 * 1_000 / generated.sample_rate() as u64;
        let mut fed_frames = 0u64;
        for chunk in samples.chunks(512) {
            fed_frames += chunk.len() as u64;
            stream.accept_waveform(16_000, chunk);
            while kws.is_ready(&stream) {
                kws.decode(&stream);
                if kws
                    .get_result(&stream)
                    .is_some_and(|r| !r.keyword.is_empty())
                {
                    detected = true;
                    break;
                }
            }
            if detected {
                break;
            }
        }
        let detected_audio_ms = fed_frames * 1_000 / 16_000;
        println!(
            "Wake replay: detected at {detected_audio_ms} ms; phrase ended at {speech_end_ms} ms"
        );
        assert!(
            detected,
            "KWS did not detect Piper's Hey Buddy through 512-frame raw-audio chunks"
        );
        assert!(
            detected_audio_ms <= speech_end_ms + 800,
            "Wake detection lagged the phrase by over 800 ms of audio"
        );

        let unrelated = tts
            .generate_with_config(
                "The weather is sunny today",
                &GenerationConfig::default(),
                None::<fn(&[f32], f32) -> bool>,
            )
            .expect("unrelated Piper audio");
        let unrelated_resampler =
            LinearResampler::create(unrelated.sample_rate(), 16_000).expect("resampler");
        let mut unrelated_input = vec![0.0f32; unrelated.sample_rate() as usize / 2];
        unrelated_input.extend_from_slice(unrelated.samples());
        unrelated_input.extend(vec![0.0f32; unrelated.sample_rate() as usize]);
        let unrelated_stream = kws.create_stream();
        for chunk in unrelated_resampler
            .resample(&unrelated_input, true)
            .chunks(512)
        {
            unrelated_stream.accept_waveform(16_000, chunk);
            while kws.is_ready(&unrelated_stream) {
                kws.decode(&unrelated_stream);
                assert!(
                    kws.get_result(&unrelated_stream)
                        .is_none_or(|r| r.keyword.is_empty()),
                    "KWS fired on unrelated speech"
                );
            }
        }
    }

    #[test]
    fn microphone_test_wav_roundtrips_without_disk_storage() {
        let signal = [0.0f32, 0.25, -0.5, 0.75];
        let wav = encode_test_wav(&signal, 16_000).expect("encode WAV");
        assert!(wav.starts_with(b"RIFF"));
        let (decoded, rate) = decode_test_wav(&wav).expect("decode WAV");
        assert_eq!(rate, 16_000);
        assert_eq!(decoded.len(), signal.len());
        for (actual, expected) in decoded.iter().zip(signal) {
            assert!((actual - expected).abs() < 0.0001);
        }
        assert!(decode_test_wav(b"not a wav").is_err());
    }

    #[test]
    fn wake_only_is_not_routed_as_a_chat_request() {
        assert_eq!(strip_wake_prefix("Hey Buddy."), "");
        assert_eq!(strip_wake_prefix("Hey Buddy, stop music."), "stop music.");
        assert_eq!(strip_wake_prefix("Hey, Buddy!"), "");
        assert_eq!(strip_wake_prefix("Hi buddy, continue"), "continue");
        assert_eq!(strip_wake_prefix("Buddy, stop music"), "Buddy, stop music");
    }

    #[test]
    #[ignore = "requires downloaded ASR model"]
    fn selected_wav_replays_through_public_asr_path() {
        let config = VoiceConfig::from_env();
        let bytes = std::fs::read(config.asr_dir.join("test_wavs/0.wav")).expect("sample WAV");
        let result = test_wav_through_asr(&config, &bytes).expect("WAV ASR test");
        assert!(result
            .raw_transcript
            .to_uppercase()
            .contains("YELLOW LAMPS"));
        assert!(result.accepted);
    }

    #[test]
    #[ignore = "requires BUDDY_TEST_WAV and installed ASR model"]
    fn supplied_microphone_wav_reports_both_asr_paths() {
        let path =
            std::env::var("BUDDY_TEST_WAV").expect("set BUDDY_TEST_WAV to a saved microphone WAV");
        let bytes = std::fs::read(path).expect("read saved microphone WAV");
        let result = test_wav_through_asr(&VoiceConfig::from_env(), &bytes).expect("WAV ASR test");
        println!("Raw ASR: {:?}", result.raw_transcript);
        println!("Denoised ASR: {:?}", result.enhanced_transcript);
        println!(
            "Accepted: {}; selected: {:?}",
            result.accepted, result.transcript
        );
    }

    #[test]
    #[ignore = "requires downloaded ASR model"]
    fn streaming_48khz_microphone_resampling_preserves_speech() {
        let config = VoiceConfig::from_env();
        let wave = sherpa_onnx::Wave::read(
            config
                .asr_dir
                .join("test_wavs/0.wav")
                .to_string_lossy()
                .as_ref(),
        )
        .expect("model test WAV");
        assert_eq!(wave.sample_rate(), 16_000);
        let recognizer = build_whisper(&config).unwrap();
        let resampler = LinearResampler::create(48_000, 16_000).unwrap();
        let mut resampled = Vec::new();
        // The Windows microphone supplies small buffers. Exercise the live
        // non-flushing resampler path, rather than the whole-file WAV path.
        for chunk in wave.samples().chunks(160) {
            let input: Vec<f32> = chunk.iter().flat_map(|sample| [*sample; 3]).collect();
            let output = resampler.resample(&input, false);
            resampled.extend(output);
        }
        let transcript = decode_whisper(&recognizer, &resampled);
        assert!(
            transcript.to_ascii_uppercase().contains("YELLOW LAMPS"),
            "{transcript}"
        );
    }

    #[test]
    fn microphone_array_channels_do_not_cancel_each_other() {
        let opposite = [0.4f32, -0.4, 0.2, -0.2];
        assert_eq!(microphone_mono(&opposite, 2, |x| x), vec![0.4, 0.2]);
        let second_only = [0.0f32, 0.3, 0.0, 0.2];
        assert_eq!(microphone_mono(&second_only, 2, |x| x), vec![0.3, 0.2]);
        assert_eq!(
            microphone_mono(&[i16::MIN, i16::MAX], 1, |x| x as f32 / 32768.0).len(),
            2
        );
    }

    #[test]
    #[ignore = "requires downloaded ASR and denoise models"]
    fn bundled_speech_reaches_the_recognizer_with_and_without_denoise() {
        let config = VoiceConfig::from_env();
        let wave_path = config.asr_dir.join("test_wavs/0.wav");
        let wave =
            sherpa_onnx::Wave::read(wave_path.to_string_lossy().as_ref()).expect("model test WAV");
        assert_eq!(wave.sample_rate(), 16_000);
        let recognizer = build_whisper(&config).unwrap();
        let raw = decode_whisper(&recognizer, wave.samples());
        println!("Raw ASR: {raw}");
        assert!(raw.to_ascii_uppercase().contains("YELLOW LAMPS"));
        if let Some(denoiser) = build_denoiser(&config) {
            let shift = denoiser.frame_shift_in_samples() as usize;
            let mut processed = Vec::new();
            for part in wave.samples().chunks_exact(shift) {
                processed.extend(denoiser.run(part, 16_000).samples);
            }
            let cleaned = decode_whisper(&recognizer, &processed);
            println!(
                "Denoised ASR: {cleaned}; samples {} -> {}",
                wave.samples().len(),
                processed.len()
            );
            assert!(!cleaned.trim().is_empty());
        }
    }

    #[test]
    fn talk_can_arm_through_monitor_without_locking_voice_engine() {
        let (sender, receiver) = mpsc::channel();
        let monitor = VoiceMonitor {
            runtime: Arc::new(Mutex::new(VoiceRuntimeStatus::default())),
            events: Arc::new(Mutex::new(VecDeque::new())),
            stop: Arc::new(AtomicBool::new(false)),
            interrupt: Arc::new(AtomicBool::new(false)),
            playback: Arc::new(Mutex::new(None)),
            pause_to_finish_ms: Arc::new(AtomicU64::new(700)),
            max_listening_ms: Arc::new(AtomicU64::new(18_000)),
            control: Arc::new(Mutex::new(Some(sender))),
        };
        monitor.arm_manual().unwrap();
        assert!(monitor.interrupt.load(Ordering::SeqCst));
        assert!(matches!(receiver.try_recv(), Ok(RuntimeCommand::Arm)));
    }

    #[test]
    fn rejects_short_noise_without_losing_common_voice_commands() {
        for noise in ["UL", "'S", "ULAR BOOK", "S MI", "S I WILL BE NOT"] {
            assert!(!usable_transcript(noise), "{noise}");
        }
        for phrase in [
            "Hi",
            "Stop",
            "Stop music",
            "Continue",
            "Go on",
            "Read news",
            "Read the news",
            "Play video",
            "Open YouTube",
            "Play news video",
            "Pause music",
            "Next song",
            "Mute music",
            "What is the weather",
        ] {
            assert!(usable_transcript(phrase), "{phrase}");
        }
    }

    #[test]
    fn whisper_wake_fallback_requires_a_leading_buddy_address() {
        assert_eq!(
            wake_request("Hey Buddy, read the news").as_deref(),
            Some("read the news")
        );
        assert_eq!(
            wake_request("Hello, Buddy! What's the weather?").as_deref(),
            Some("What's the weather?")
        );
        assert_eq!(wake_request("Hi buddy").as_deref(), Some(""));
        assert_eq!(wake_request("The presenter said hey buddy yesterday"), None);
        assert_eq!(wake_request("Anything at all"), None);
    }
}

fn now_ms() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    d.as_millis() as u64
}

impl Drop for VoiceEngine {
    fn drop(&mut self) {
        self.monitor.stop();
        if let Some(tx) = &self.control {
            let _ = tx.send(RuntimeCommand::Shutdown);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
