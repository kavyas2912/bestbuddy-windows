use buddy_core::BuddyEngine;
use buddy_plugin::PluginRegistry;
use buddy_plugin_fun_facts::FunFactsPlugin;
use buddy_plugin_market::MarketPlugin;
use buddy_plugin_news::NewsPlugin;
use buddy_plugin_weather::WeatherPlugin;
use buddy_voice::{VoiceConfig, VoiceEngine};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

mod commands;
mod llm;
mod llm_assets;
mod microphone;
mod turns;
mod voice_catalog;

pub const BUILD_LABEL: &str = "v0.8.0-core-english-candidate";

pub struct AppState {
    pub turns: turns::ConversationTurns,
    pub engine: Arc<AsyncMutex<BuddyEngine>>,
    pub monitor: buddy_voice::VoiceMonitor,
    pub voice: Arc<Mutex<VoiceEngine>>,
    pub voice_enabled: Arc<AtomicBool>,
    pub voice_tests_active: Arc<AtomicUsize>,
    pub llm: Arc<llm::LlmBridge>,
    pub voice_choice_path: std::path::PathBuf,
    pub voice_library_dir: std::path::PathBuf,
    pub voice_auto: Arc<AtomicBool>,
    pub voice_mode_path: std::path::PathBuf,
}

fn app_data_dir(app: &tauri::App) -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return Ok(std::path::PathBuf::from(local)
            .join("BestBuddy")
            .join("data"));
    }
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn bundled_voice_config(app: &tauri::App) -> VoiceConfig {
    let mut config = VoiceConfig::from_env();
    config.num_threads = buddy_core::resource_budget().inference_threads as i32;
    #[cfg(target_os = "android")]
    let models = match app.path().app_data_dir() {
        Ok(data) => data.join("voice-models"),
        Err(_) => return config,
    };
    #[cfg(target_os = "windows")]
    let models = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")))
        .filter(|models| models.join("asr/sherpa-onnx-whisper-base.en").is_dir())
        .or_else(|| app.path().resource_dir().ok().map(|dir| dir.join("models")))
        .unwrap_or_default();
    #[cfg(not(any(target_os = "android", target_os = "windows")))]
    let models = match app.path().resource_dir() {
        Ok(resources) => resources.join("models"),
        Err(_) => return config,
    };
    // Development runs can use the source tree. Packaged builds must report
    // their own missing bundle assets instead of silently using another copy
    // discovered through the process working directory.
    if cfg!(debug_assertions) && !models.join("asr/sherpa-onnx-whisper-base.en").is_dir() {
        return config;
    }
    config.asr_dir = models.join("asr/sherpa-onnx-whisper-base.en");
    config.tts_dir = models.join("tts/vits-piper-en_US-lessac-medium");
    config.vad_model = models.join("vad/silero_vad.onnx");
    config.kws_dir = models.join("kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01");
    config.kws_keywords = models.join("kws/keywords.txt");
    config.denoise_model = models.join("denoise/gtcrn_simple.onnx");
    config
}

fn initialize(data_dir: std::path::PathBuf, voice_config: VoiceConfig) -> AppState {
    let mut registry = PluginRegistry::new();
    tauri::async_runtime::block_on(async {
        registry
            .register(Box::new(WeatherPlugin::new()))
            .await
            .expect("weather");
        registry
            .register(Box::new(NewsPlugin::new()))
            .await
            .expect("news");
        registry
            .register(Box::new(MarketPlugin::new()))
            .await
            .expect("markets");
        registry
            .register(Box::new(FunFactsPlugin::new()))
            .await
            .expect("fun facts");
    });
    let mut engine = BuddyEngine::new("Friend", "", registry, data_dir.clone());
    engine.start();
    engine.log(&format!("desktop build: {BUILD_LABEL}"));
    engine.log(&format!(
        "voice assets: asr={} tts={} kws={}",
        voice_config.asr_dir.display(),
        voice_config.tts_dir.display(),
        voice_config.kws_dir.display()
    ));
    let voice_choice_path = data_dir.join("piper-voice.txt");
    let voice_mode_path = data_dir.join("piper-voice-mode.txt");
    let voice_auto = std::fs::read_to_string(&voice_mode_path)
        .map(|mode| mode.trim() != "manual")
        .unwrap_or(true);
    let mut voice_engine = VoiceEngine::new(voice_config);
    let voice_library_dir = data_dir.join("voices");
    voice_engine.set_voice_library_dir(voice_library_dir.clone());
    if let Ok(id) = std::fs::read_to_string(&voice_choice_path) {
        let _ = voice_engine.select_piper_voice(id.trim());
    }

    let monitor = voice_engine.monitor();
    let voice_arc = Arc::new(Mutex::new(voice_engine));

    let voice_enabled = Arc::new(AtomicBool::new(engine.voice_allowed()));
    let state = AppState {
        turns: turns::ConversationTurns::default(),
        voice_enabled,
        monitor,
        engine: Arc::new(AsyncMutex::new(engine)),
        voice: voice_arc,
        voice_tests_active: Arc::new(AtomicUsize::new(0)),
        llm: Arc::new(llm::LlmBridge::new(data_dir)),
        voice_choice_path,
        voice_library_dir,
        voice_auto: Arc::new(AtomicBool::new(voice_auto)),
        voice_mode_path,
    };
    let voice_watch = Arc::clone(&state.voice);
    let monitor_watch = state.monitor.clone();
    let enabled_watch = Arc::clone(&state.voice_enabled);
    let tests_watch = Arc::clone(&state.voice_tests_active);
    let engine_watch = Arc::clone(&state.engine);
    tauri::async_runtime::spawn(async move {
        let mut previous_error = String::new();
        loop {
            if enabled_watch.load(Ordering::SeqCst)
                && tests_watch.load(Ordering::SeqCst) == 0
                && !monitor_watch.status().always_on
            {
                let voice = Arc::clone(&voice_watch);
                let enabled = Arc::clone(&enabled_watch);
                let result = tauri::async_runtime::spawn_blocking(move || {
                    let mut voice = voice.lock().map_err(|e| e.to_string())?;
                    if !enabled.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    voice.warm_up().map_err(|e| e.to_string())?;
                    if enabled.load(Ordering::SeqCst) {
                        voice.start_always_on().map_err(|e| e.to_string())?;
                    }
                    Ok::<(), String>(())
                })
                .await;
                match result {
                    Ok(Ok(())) => {
                        if !previous_error.is_empty() {
                            engine_watch
                                .lock()
                                .await
                                .log("voice runtime recovered automatically");
                            previous_error.clear();
                        }
                    }
                    other => {
                        let error = format!("{other:?}");
                        if error != previous_error {
                            engine_watch.lock().await.log(&format!(
                                "voice startup failed; retrying automatically: {error}"
                            ));
                            previous_error = error;
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    let local_learning = Arc::clone(&state.engine);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let _ = commands::run_local_learning(&local_learning, false).await;
        }
    });
    let engine_bg = Arc::clone(&state.engine);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        loop {
            let _ = commands::run_prefetch(&engine_bg).await;
            tokio::time::sleep(std::time::Duration::from_secs(900)).await;
        }
    });
    let monitor_log = state.monitor.clone();
    let engine_log = state.engine.clone();
    tauri::async_runtime::spawn(async move {
        let mut previous = String::new();
        loop {
            let mut snapshot = monitor_log.diagnostics();
            if let Some(o) = snapshot.as_object_mut() {
                o.remove("last_transcript");
                o.remove("last_voice_ms");
                o.remove("asr_partial");
                o.remove("input_level");
                o.remove("raw_input_level");
                o.remove("input_frames");
                o.remove("kws_decode_count");
                o.remove("kws_last_decode_ms");
                o.remove("vad_active");
            }
            let current = snapshot.to_string();
            if current != previous {
                engine_log.lock().await.log(&format!("voice: {current}"));
                previous = current;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    state
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(microphone::init())
        .setup(|app| {
            let data_dir = app_data_dir(app).map_err(std::io::Error::other)?;
            let state = initialize(data_dir, bundled_voice_config(app));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::contact_open,
            commands::weather_widget,
            commands::weather_coordinates,
            commands::market_widget,
            commands::market_detail,
            commands::market_company_news,
            commands::market_news,
            commands::market_corporate_actions,
            commands::news_widget,
            commands::local_events_widget,
            commands::calendar_day_context,
            commands::news_coverage,
            commands::visible_news_set,
            commands::news_videos_widget,
            commands::read_logs,
            commands::status,
            commands::chat,
            commands::cancel_chat,
            commands::llm_status,
            commands::llm_free_models,
            commands::llm_usage,
            commands::llm_store_key,
            commands::llm_configure,
            commands::llm_unlock,
            commands::llm_remove_key,
            commands::llm_prepare_local,
            commands::plugins,
            commands::voice_status,
            commands::piper_voices,
            commands::piper_select_voice,
            commands::piper_set_auto,
            commands::piper_prepare_voice,
            commands::voice_tune,
            commands::voice_events,
            commands::voice_arm,
            commands::voice_permission_request,
            commands::voice_speak,
            commands::voice_test_microphone,
            commands::voice_test_wav,
            commands::voice_roundtrip,
            commands::profile_get,
            commands::profile_save,
            commands::profile_draft_get,
            commands::profile_draft_save,
            commands::profile_draft_clear,
            commands::profile_export,
            commands::profile_erase,
            commands::learning_status,
            commands::local_learning_status,
            commands::local_learning_configure,
            commands::local_learning_refresh,
            commands::local_learning_import,
            commands::local_learning_clear,
            commands::knowledge,
            commands::conversation_items,
            commands::lexicon,
            commands::chatter,
            commands::prefetch_now,
            commands::open_logs,
            commands::open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running BestBuddy");
}
