static QUIET_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn quiet_active() -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    now < QUIET_UNTIL.load(Ordering::SeqCst)
}
#[tauri::command]
pub fn voice_quiet_until(state: tauri::State<'_, AppState>, until: u64) {
    SPEECH_EPOCH.fetch_add(1, Ordering::SeqCst);
    QUIET_UNTIL.store(until, Ordering::SeqCst);
    state.monitor.set_quiet_until(until);
}
#[tauri::command]
pub fn voice_hush(state: tauri::State<'_, AppState>) {
    SPEECH_EPOCH.fetch_add(1, Ordering::SeqCst);
    state.monitor.hush();
}
static SPEECH_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
use crate::AppState;
use buddy_core::UserProfile;
use buddy_plugin::{BuddyPlugin, PluginRequest};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;

fn coverage_words(title: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "about", "after", "amid", "and", "are", "for", "from", "into", "latest", "new", "news",
        "over", "says", "that", "the", "their", "this", "today", "with",
    ];
    title
        .split(" - ")
        .next()
        .unwrap_or(title)
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() > 2 && !STOP.contains(&word.as_str()))
        .take(14)
        .collect()
}

fn related_coverage(items: &[Value], title: &str, source: &str, url: &str) -> Vec<Value> {
    let origin = coverage_words(title);
    let mut outlets = std::collections::HashSet::new();
    outlets.insert(source.trim().to_ascii_lowercase());
    let mut seen_urls = std::collections::HashSet::new();
    seen_urls.insert(url.to_string());
    let mut matches = Vec::new();
    for item in items {
        let Some(other_title) = item["title"].as_str() else {
            continue;
        };
        let Some(other_url) = item["url"].as_str() else {
            continue;
        };
        let Some(other_source) = item["source"].as_str() else {
            continue;
        };
        let outlet = other_source.trim().to_ascii_lowercase();
        if outlet.is_empty() || outlets.contains(&outlet) || seen_urls.contains(other_url) {
            continue;
        }
        let words = coverage_words(other_title);
        let overlap = words.iter().filter(|word| origin.contains(word)).count();
        let minimum = if origin.len().min(words.len()) <= 3 {
            2
        } else {
            3
        };
        if overlap < minimum {
            continue;
        }
        outlets.insert(outlet);
        seen_urls.insert(other_url.to_string());
        matches.push(item.clone());
        if matches.len() == 6 {
            break;
        }
    }
    matches
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn keeps_distinct_outlets_about_the_same_story() {
        let items = vec![
            json!({"title":"Moon mission lands near lunar south pole - Source A","source":"Source A","url":"https://a.test/one"}),
            json!({"title":"Moon mission lands near lunar south pole - Source B","source":"Source B","url":"https://b.test/two"}),
            json!({"title":"Moon mission reaches lunar south pole - Source B","source":"Source B","url":"https://b.test/three"}),
            json!({"title":"Football scores in Europe - Source C","source":"Source C","url":"https://c.test/four"}),
            json!({"title":"Moon mission lands near lunar south pole - Source D","source":"Source D","url":"https://d.test/five"}),
        ];
        let result = related_coverage(
            &items,
            "Moon mission lands near lunar south pole",
            "Source A",
            "https://a.test/one",
        );
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["source"], "Source B");
        assert_eq!(result[1]["source"], "Source D");
    }

    #[test]
    fn general_news_filter_keeps_english_and_rejects_other_scripts() {
        assert!(headline_is_probably_english(
            &json!({"title":"Scientists report a new lunar discovery"})
        ));
        assert!(!headline_is_probably_english(
            &json!({"title":"वैज्ञानिकों ने नई खोज की घोषणा की"})
        ));
        assert!(!headline_is_probably_english(
            &json!({"title":"નવી સ્થાનિક ઘટનાની જાહેરાત"})
        ));
    }

    #[test]
    fn breaking_filter_rejects_channel_pages_and_known_misclassified_sources() {
        assert!(!headline_is_story(
            &json!({"title":"Breaking News - Kanak News"})
        ));
        assert!(!headline_is_story(
            &json!({"title":"Odisha News - Latest News"})
        ));
        assert!(headline_is_story(&json!({
            "title":"Cabinet approves new national research programme - ANI"
        })));
        assert!(breaking_source_is_noise(
            &json!({"source":"Kanak News","title":"A title long enough to look like a story"})
        ));
        assert!(!breaking_source_is_noise(
            &json!({"source":"Reuters","title":"A title long enough to look like a story"})
        ));
    }

    #[test]
    fn national_agencies_and_major_outlets_receive_source_priority() {
        assert_eq!(news_source_priority(&json!({"source":"ANI"})), 2);
        assert_eq!(news_source_priority(&json!({"source":"MSN"})), 2);
        assert_eq!(news_source_priority(&json!({"source":"CNBC-TV18"})), 2);
        assert_eq!(news_source_priority(&json!({"source":"Unknown blog"})), 1);
    }
}

struct VoiceTestGuard(Arc<AtomicUsize>);
impl VoiceTestGuard {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(Arc::clone(counter))
    }
}
impl Drop for VoiceTestGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let (engine_state, uptime, profile, learning, log_path) = {
        let e = state.engine.lock().await;
        (
            format!("{:?}", e.state),
            e.uptime_seconds(),
            e.profile(),
            e.learning_status(),
            e.log_path(),
        )
    };
    Ok(json!({
        "build":crate::BUILD_LABEL,
        "engine":"ready",
        "state":engine_state,
        "uptime":uptime,
        "profile":profile,
        "learning":learning,
        "log_path":log_path,
        "voice":state.monitor.diagnostics()
    }))
}

#[tauri::command]
pub async fn chat(state: tauri::State<'_, AppState>, input: String) -> Result<Value, String> {
    let (mut r, local_context, public_context) = {
        let mut e = state.engine.lock().await;
        let result = e.chat(&input).await.map_err(|x| x.to_string())?;
        e.log(&format!(
            "chat route: {}",
            result.capability.as_deref().unwrap_or("open conversation")
        ));
        let local_context = e.llm_context(&input, true);
        let public_context = e.llm_context(&input, false);
        (result, local_context, public_context)
    };
    let can_compose = may_use_llm(r.capability.as_deref());
    if can_compose && !input.trim().is_empty() {
        match state
            .llm
            .complete(&input, &r.text, &local_context, &public_context)
            .await
        {
            Ok(Some((text, provider))) => {
                r.text = text;
                if r.capability.is_none() {
                    r.capability = Some(provider);
                }
            }
            Ok(None) => {}
            Err(error) => state
                .engine
                .lock()
                .await
                .log(&format!("LLM fallback to local composer: {error}")),
        }
    }
    state.llm.observe_turn(&input, &r.text);
    serde_json::to_value(r).map_err(|e| e.to_string())
}

fn may_use_llm(capability: Option<&str>) -> bool {
    // Commands, phrasebook, support and sourced retrieval remain authoritative.
    // Local conversational turns can use the model and current dialogue context.
    capability.is_none()
        || matches!(
            capability,
            Some("conversation.local" | "conversation.follow_up")
        )
}

#[cfg(test)]
mod routing_tests {
    use super::{approved_contact_link, may_use_llm};
    #[test]
    fn models_cannot_replace_phrasebook_or_canonical_actions() {
        assert!(!may_use_llm(Some("phrasebook.local")));
        assert!(!may_use_llm(Some("news.headlines")));
        assert!(!may_use_llm(Some("news.read_continue")));
        assert!(!may_use_llm(Some("music.stop")));
        assert!(!may_use_llm(Some("weather.current")));
        assert!(!may_use_llm(Some("rag.local")));
        assert!(may_use_llm(Some("conversation.follow_up")));
        assert!(may_use_llm(Some("conversation.local")));
        assert!(may_use_llm(None));
    }
    #[test]
    fn communication_links_reject_untrusted_hosts() {
        assert!(approved_contact_link("slack", "https://app.slack.com/client/T1/D1").is_ok());
        assert!(approved_contact_link("discord", "https://discord.com/channels/@me/123").is_ok());
        assert!(approved_contact_link("zoom", "https://us02web.zoom.us/j/123456789").is_ok());
        assert!(
            approved_contact_link("teams", "https://teams.microsoft.com/l/meetup-join/abc").is_ok()
        );
        assert!(
            approved_contact_link("slack", "https://app.slack.com.evil.example/client/T1/D1")
                .is_err()
        );
        assert!(approved_contact_link("zoom", "http://zoom.us/j/123456789").is_err());
    }
}

#[tauri::command]
pub async fn llm_status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Ok(state.llm.status().await)
}

#[tauri::command]
pub async fn llm_free_models(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let models = state.llm.free_models().await?;
    Ok(json!(models
        .into_iter()
        .map(|(id, name)| json!({"id":id,"name":name}))
        .collect::<Vec<_>>()))
}

#[tauri::command]
pub async fn llm_provider_models(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<Value, String> {
    let models = state.llm.provider_models(&provider).await?;
    Ok(json!(models
        .into_iter()
        .map(|(id, name)| json!({"id":id,"name":name}))
        .collect::<Vec<_>>()))
}

#[tauri::command]
pub async fn llm_usage(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Ok(state.llm.usage().await)
}

#[tauri::command]
pub async fn llm_store_key(
    state: tauri::State<'_, AppState>,
    provider: String,
    api_key: String,
) -> Result<Value, String> {
    state.llm.store_key(&provider, &api_key)?;
    Ok(state.llm.status().await)
}

#[tauri::command]
pub async fn llm_configure(
    state: tauri::State<'_, AppState>,
    settings: crate::llm::LlmPreferences,
    api_key: Option<String>,
) -> Result<Value, String> {
    state.llm.configure(settings, api_key)?;
    Ok(state.llm.status().await)
}

#[tauri::command]
pub async fn llm_unlock(
    state: tauri::State<'_, AppState>,
    password: String,
) -> Result<Value, String> {
    state.llm.unlock(password)?;
    Ok(state.llm.status().await)
}

#[tauri::command]
pub async fn llm_remove_key(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<Value, String> {
    state.llm.remove_key(&provider)?;
    Ok(state.llm.status().await)
}

#[tauri::command]
pub async fn llm_prepare_local(
    state: tauri::State<'_, AppState>,
    model: String,
) -> Result<Value, String> {
    state.llm.prepare_local(&model).await
}

#[tauri::command]
pub async fn plugins(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    serde_json::to_value(e.plugins.manifests()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn voice_status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Ok(state.monitor.diagnostics())
}

#[tauri::command]
pub fn piper_voices(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let voice = state.voice.lock().map_err(|e| e.to_string())?;
    let available = crate::voice_catalog::OFFERS
        .iter()
        .map(|offer| {
            json!({
                "id":offer.id,"name":offer.label,"gender":offer.gender,"language":offer.language,
                "source":offer.source,"dataset_license":offer.dataset_license,
                "installed":crate::voice_catalog::complete(&state.voice_library_dir.join(offer.id))
            })
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"installed":voice.piper_voices().into_iter().filter(|v| !v.id.contains("pratham")).collect::<Vec<_>>(),"available":available,"selected":voice.current_piper_voice(),"auto":state.voice_auto.load(Ordering::SeqCst)}),
    )
}

#[tauri::command]
pub async fn piper_prepare_voice(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Value, String> {
    crate::voice_catalog::prepare(&id, &state.voice_library_dir).await?;
    piper_voices(state)
}

#[tauri::command]
pub async fn piper_select_voice(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Value, String> {
    if id.contains("pratham") {
        return Err("This voice was withdrawn. Choose an available English fallback or another supported voice.".into());
    }

    state.llm.require_unlocked()?;
    let voice = std::sync::Arc::clone(&state.voice);
    let selected = tauri::async_runtime::spawn_blocking(move || {
        let mut voice = voice.lock().map_err(|e| e.to_string())?;
        voice.select_piper_voice(&id).map_err(|e| e.to_string())?;
        Ok::<_, String>(voice.current_piper_voice())
    })
    .await
    .map_err(|e| e.to_string())??;
    std::fs::write(&state.voice_choice_path, &selected).map_err(|e| e.to_string())?;
    std::fs::write(&state.voice_mode_path, "manual").map_err(|e| e.to_string())?;
    state.voice_auto.store(false, Ordering::SeqCst);
    piper_voices(state)
}

#[tauri::command]
pub fn piper_set_auto(state: tauri::State<'_, AppState>, enabled: bool) -> Result<Value, String> {
    state.llm.require_unlocked()?;
    std::fs::write(
        &state.voice_mode_path,
        if enabled { "auto" } else { "manual" },
    )
    .map_err(|e| e.to_string())?;
    state.voice_auto.store(enabled, Ordering::SeqCst);
    piper_voices(state)
}

#[tauri::command]
pub fn voice_tune(
    state: tauri::State<'_, AppState>,
    pause_to_finish_ms: u64,
    max_listening_ms: u64,
) -> Result<(), String> {
    state
        .monitor
        .tune(pause_to_finish_ms, max_listening_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn voice_events(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let permitted = {
        let engine = state.engine.lock().await;
        crate::voice_permission(&engine.profile())
    };
    let events = state.monitor.take_events();
    serde_json::to_value(if permitted { events } else { Vec::new() }).map_err(|e| e.to_string())
}

async fn require_voice_consent(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    let engine = state.engine.lock().await;
    if crate::voice_permission(&engine.profile()) {
        Ok(())
    } else {
        Err("Complete registration and enable both microphone and local voice-processing consent in Profile settings.".into())
    }
}

#[tauri::command]
pub async fn voice_arm(state: tauri::State<'_, AppState>) -> Result<(), String> {
    require_voice_consent(&state).await?;
    if state.monitor.status().always_on {
        return state.monitor.arm_manual().map_err(|e| e.to_string());
    }
    let voice = std::sync::Arc::clone(&state.voice);
    tauri::async_runtime::spawn_blocking(move || {
        let mut v = voice.lock().map_err(|e| e.to_string())?;
        v.warm_up().map_err(|e| e.to_string())?;
        v.start_always_on().map_err(|e| e.to_string())?;
        v.arm_manual().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn voice_speak(
    state: tauri::State<'_, AppState>,
    text: String,
    request: Option<String>,
    capability: Option<String>,
) -> Result<(), String> {
    if quiet_active() {
        return Ok(());
    }
    let speech_epoch = SPEECH_EPOCH.load(Ordering::SeqCst);
    let voice = std::sync::Arc::clone(&state.voice);
    let automatic = state.voice_auto.load(Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let mut v = voice.lock().map_err(|e| e.to_string())?;
        if quiet_active() || speech_epoch != SPEECH_EPOCH.load(Ordering::SeqCst) {
            return Ok(());
        }
        let result = if capability.as_deref() == Some("ui.guide") {
            let previous = v.current_piper_voice();
            let result = v
                .select_piper_voice("macaw-playful")
                .and_then(|_| v.speak(&text));
            let restored = v.select_piper_voice(&previous);
            result.and(restored)
        } else if automatic {
            v.speak_adaptive(
                &text,
                request.as_deref().unwrap_or(""),
                capability.as_deref().unwrap_or(""),
            )
        } else {
            v.speak(&text)
        }
        .map_err(|e| e.to_string());
        if let Err(ref error) = result {
            v.record_error(error);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn voice_test_microphone(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    require_voice_consent(&state).await?;
    let _test = VoiceTestGuard::new(&state.voice_tests_active);
    // Stop the always-on path while measuring the same default CPAL device;
    // simultaneous streams would make the test ambiguous on some drivers.
    state.monitor.stop();
    for _ in 0..40 {
        if !state.monitor.status().always_on {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if state.monitor.status().always_on {
        return Err("The voice microphone stream did not stop before the test.".into());
    }
    let clip = tauri::async_runtime::spawn_blocking(|| buddy_voice::record_microphone_test(5_000))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    serde_json::to_value(clip).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn voice_test_wav(
    state: tauri::State<'_, AppState>,
    wav_bytes: Vec<u8>,
) -> Result<Value, String> {
    let _test = VoiceTestGuard::new(&state.voice_tests_active);
    if wav_bytes.len() > 16 * 1024 * 1024 {
        return Err("Choose a WAV file no larger than 16 MB.".into());
    }
    state.monitor.stop();
    let started = std::time::Instant::now();
    let config = state
        .voice
        .lock()
        .map_err(|error| error.to_string())?
        .config();
    let test = tauri::async_runtime::spawn_blocking(move || {
        buddy_voice::test_wav_through_asr(&config, &wav_bytes)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let asr_ms = started.elapsed().as_millis() as u64;
    if !test.accepted {
        return Ok(json!({"asr":test,"asr_ms":asr_ms,"response":null,"spoken":false}));
    }
    let response = {
        let mut engine = state.engine.lock().await;
        engine
            .chat(&test.transcript)
            .await
            .map_err(|error| error.to_string())?
    };
    let response_ms = started.elapsed().as_millis() as u64 - asr_ms;
    let spoken_text = response.text.clone();
    let voice = std::sync::Arc::clone(&state.voice);
    let speaker_error = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = voice.lock().map_err(|error| error.to_string())?;
        engine
            .speak(&spoken_text)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
    .err();
    Ok(json!({
        "asr":test,
        "asr_ms":asr_ms,
        "response":response,
        "response_ms":response_ms,
        "spoken":speaker_error.is_none(),
        "speaker_error":speaker_error,
        "total_ms":started.elapsed().as_millis() as u64
    }))
}

#[tauri::command]
pub async fn voice_roundtrip(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    require_voice_consent(&state).await?;
    let started = std::time::Instant::now();
    let initial_sequence = state.monitor.status().transcript_sequence;
    {
        let v = state.voice.lock().map_err(|e| e.to_string())?;
        v.arm_manual().map_err(|e| e.to_string())?;
    }

    let mut transcript = String::new();
    let mut asr_ms = 0u64;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let status = state.monitor.status();
        if status.state == "Error" {
            return Err(status
                .last_error
                .unwrap_or_else(|| "Voice runtime failed".into()));
        }
        if status.transcript_sequence > initial_sequence {
            transcript = status.last_transcript.unwrap_or_default();
            asr_ms = status.last_voice_ms.unwrap_or(0);
        }
        if status.state == "Stopped" {
            break;
        }
        if !transcript.is_empty() {
            break;
        }
    }

    if transcript.trim().is_empty() {
        return Ok(json!({
            "transcript":"",
            "asr_ms":asr_ms,
            "total_ms":started.elapsed().as_millis() as u64,
            "spoke":false
        }));
    }

    let response = {
        let mut e = state.engine.lock().await;
        e.chat(&transcript).await.map_err(|e| e.to_string())?
    };
    let response_ms = started.elapsed().as_millis() as u64;

    {
        let mut v = state.voice.lock().map_err(|e| e.to_string())?;
        v.speak(&response.text).map_err(|e| e.to_string())?;
    }
    let total_ms = started.elapsed().as_millis() as u64;
    Ok(json!({
        "transcript":transcript,
        "response":response.text,
        "asr_ms":asr_ms,
        "response_ms":response_ms,
        "total_ms":total_ms,
        "spoke":true
    }))
}

#[tauri::command]
pub async fn profile_get(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    serde_json::to_value(e.profile()).map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn profile_save(
    state: tauri::State<'_, AppState>,
    mut profile: UserProfile,
) -> Result<Value, String> {
    state.llm.require_unlocked()?;
    // A full address can explicitly change the country selected earlier.
    if profile.location.contains(',') {
        let (city, country) =
            buddy_plugin_news::curation::explicit_location(&profile.location, &profile.country);
        profile.location = city;
        profile.country = country
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(region) =
            buddy_plugin_news::curation::city_state(&profile.location, &profile.country)
        {
            profile.region = region;
        }
    }
    let mut e = state.engine.lock().await;
    let previous = e.profile();
    let location_changed = previous.location != profile.location
        || previous.country != profile.country
        || previous.region != profile.region;
    e.set_profile(profile)?;
    let current = e.profile();
    drop(e);
    if location_changed {
        crate::editorial::clear().await;
    }
    if !crate::voice_permission(&current) {
        state.monitor.stop();
        // Do not deliver a recognition event captured before consent was revoked.
        let _ = state.monitor.take_events();
    }
    serde_json::to_value(current).map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn profile_draft_get(state: tauri::State<'_, AppState>) -> Result<Option<Value>, String> {
    let e = state.engine.lock().await;
    Ok(e.registration_draft())
}

#[tauri::command]
pub async fn profile_draft_save(
    state: tauri::State<'_, AppState>,
    draft: Value,
) -> Result<(), String> {
    let e = state.engine.lock().await;
    e.save_registration_draft(&draft)
}

#[tauri::command]
pub async fn profile_draft_clear(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let e = state.engine.lock().await;
    e.clear_registration_draft()
}

#[tauri::command]
pub async fn profile_export(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    if !e.user.consents.profile_storage {
        return Err("There is no saved profile to export.".into());
    }
    Ok(e.export_personal_data())
}

#[tauri::command]
pub async fn profile_erase(
    state: tauri::State<'_, AppState>,
    confirmation: String,
) -> Result<(), String> {
    if confirmation != "DELETE MY BUDDY DATA" {
        return Err("Type DELETE MY BUDDY DATA to confirm local data deletion.".into());
    }
    state.monitor.stop();
    let mut e = state.engine.lock().await;
    e.erase_personal_data()?;
    state.llm.erase_credentials()
}

#[tauri::command]
pub async fn learning_status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    Ok(serde_json::to_value(e.learning_status()).map_err(|x| x.to_string())?)
}

#[tauri::command]
pub async fn knowledge(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    Ok(serde_json::to_value(e.knowledge_items()).map_err(|x| x.to_string())?)
}

#[tauri::command]
pub async fn conversation_items(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    serde_json::to_value(e.conversation_items()).map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn lexicon(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let e = state.engine.lock().await;
    Ok(serde_json::to_value(e.lexicon()).map_err(|x| x.to_string())?)
}

#[tauri::command]
pub async fn chatter(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let mut e = state.engine.lock().await;
    Ok(e.next_chatter())
}

#[tauri::command]
pub async fn prefetch_now(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    run_prefetch(&state.engine).await
}

#[tauri::command]
pub async fn open_logs(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let e = state.engine.lock().await;
    let dir = e
        .log_path()
        .parent()
        .ok_or("log folder unavailable")?
        .to_path_buf();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(&dir)
            .spawn()
            .map_err(|x| x.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "android")]
    {
        Err(
            "Android keeps logs inside BestBuddy Settings. Use the Logs page to read or share them."
                .into(),
        )
    }
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        open::that(&dir).map_err(|x| x.to_string())
    }
}

#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web URLs may be opened.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn weather_widget(
    _state: tauri::State<'_, AppState>,
    location: String,
) -> Result<Value, String> {
    buddy_plugin_weather::WeatherPlugin::new()
        .execute(PluginRequest {
            capability: "weather.current".into(),
            params: json!({"location":location}),
        })
        .await
        .map(|r| r.data)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn weather_coordinates(latitude: f64, longitude: f64) -> Result<Value, String> {
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err("Geolocation coordinates are invalid".into());
    }
    let url=format!("https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=7");
    let raw = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .header("User-Agent", "BestBuddy/0.6.12 geolocation weather")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !raw.status().is_success() {
        return Err(format!("Weather provider returned HTTP {}", raw.status()));
    }
    let value = raw.json::<Value>().await.map_err(|e| e.to_string())?;
    let current = &value["current"];
    let daily = &value["daily"];
    let dates = daily["time"].as_array().cloned().unwrap_or_default();
    let codes = daily["weather_code"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let highs = daily["temperature_2m_max"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let lows = daily["temperature_2m_min"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let rain = daily["precipitation_probability_max"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let days=dates.into_iter().enumerate().map(|(i,date)|json!({"date":date,"weather_code":codes.get(i).cloned().unwrap_or(Value::Null),"high":highs.get(i).cloned().unwrap_or(Value::Null),"low":lows.get(i).cloned().unwrap_or(Value::Null),"rain_probability":rain.get(i).cloned().unwrap_or(Value::Null)})).collect::<Vec<_>>();
    Ok(
        json!({"timezone":value["timezone"],"current":{"temperature":current["temperature_2m"],"apparent_temperature":current["apparent_temperature"],"humidity":current["relative_humidity_2m"],"weather_code":current["weather_code"],"wind_speed":current["wind_speed_10m"],"condition":"Current conditions"},"daily":days,"coordinates":{"latitude":latitude,"longitude":longitude}}),
    )
}

#[tauri::command]
pub async fn market_widget(
    state: tauri::State<'_, AppState>,
    exchange: Option<String>,
) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    buddy_plugin_market::set_refresh_minutes(p.market_refresh_minutes)?;
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err("Enable live information and external services for market prices.".into());
    }
    let selected = exchange
        .filter(|value| value != "all")
        .or_else(|| buddy_plugin_market::home_exchange(&p.country).map(str::to_owned))
        .ok_or("No bundled exchange for this home location; choose a listed exchange.")?;
    let result = buddy_plugin_market::MarketPlugin::new()
        .execute(PluginRequest {
            capability: "market.dashboard".into(),
            params: json!({"exchange":selected,"include_global_indices":true}),
        })
        .await
        .map_err(|e| e.to_string());
    match result {
        Ok(response) => {
            let data = response.data;
            let live = data["live"].as_u64().unwrap_or(0);
            let total = data["items"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0);
            let failures = data["items"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            let error = item["error"].as_str()?;
                            Some(format!(
                                "{}: {}",
                                item["name"].as_str().unwrap_or("index"),
                                error
                            ))
                        })
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .unwrap_or_default();
            state.engine.lock().await.log(&format!(
                "market refresh: {live}/{total} quotes; {failures}"
            ));
            Ok(data)
        }
        Err(error) => {
            state
                .engine
                .lock()
                .await
                .log(&format!("market refresh failed: {error}"));
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn market_detail(
    state: tauri::State<'_, AppState>,
    symbol: String,
    range: Option<String>,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err("Enable live information and external services for stock details.".into());
    }
    let result = buddy_plugin_market::MarketPlugin::new()
        .execute(PluginRequest {
            capability: "market.detail".into(),
            params: json!({"symbol":symbol,"range":range.unwrap_or_else(|| "1d".into())}),
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.data)
}

#[tauri::command]
pub async fn market_company_news(
    state: tauri::State<'_, AppState>,
    name: String,
    symbol: String,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err("Enable live information and external services for company news.".into());
    }
    if name.trim().is_empty() || name.chars().count() > 120 || symbol.len() > 32 {
        return Err("Company selection is invalid".into());
    }
    let query =
        format!("{name} {symbol} company news (site:finance.yahoo.com OR site:moneycontrol.com)");
    let filings_query = format!(
        "{name} {symbol} results earnings dividend bonus split acquisition corporate action (site:finance.yahoo.com OR site:moneycontrol.com)"
    );
    let sector_query =
        format!("{name} {symbol} business (site:finance.yahoo.com OR site:moneycontrol.com)");
    let (company, filings, sector) = tokio::join!(
        news_search(query.clone(), 16),
        news_search(filings_query, 16),
        news_search(sector_query, 16)
    );
    let mut items = company;
    items.extend(filings);
    items.extend(sector);
    items.retain(headline_is_probably_english);
    items.sort_by_key(|item| std::cmp::Reverse(news_source_priority(item)));
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        let key = item["url"]
            .as_str()
            .unwrap_or_else(|| item["title"].as_str().unwrap_or_default())
            .to_ascii_lowercase();
        !key.is_empty() && seen.insert(key)
    });
    items.truncate(20);
    let exchange_url = if let Some(code) = symbol.strip_suffix(".NS") {
        format!("https://www.nseindia.com/companies-listing/corporate-filings-actions?symbol={code}&tabIndex=equity")
    } else if symbol.ends_with(".BO") {
        "https://www.bseindia.com/corporates/corporate_act.aspx".into()
    } else {
        format!(
            "https://finance.yahoo.com/quote/{}/",
            urlencoding::encode(&symbol)
        )
    };
    Ok(json!({"items":items,"exchange_url":exchange_url,"query":query}))
}

#[tauri::command]
pub async fn market_news(
    state: tauri::State<'_, AppState>,
    exchange: String,
    source: Option<String>,
) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err("Enable live information and external-service consent for market news.".into());
    }
    let country = buddy_plugin_market::country_for_exchange(&exchange, &p.country);
    crate::editorial::fetch(
        "business-national".into(),
        source.unwrap_or("market".into()),
        p.location,
        p.region,
        country,
        false,
    )
    .await
}

#[tauri::command]
pub async fn market_corporate_actions(
    state: tauri::State<'_, AppState>,
    exchange: String,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    let (scope, official) = match exchange.as_str() {
        "India" => ("India NSE BSE", "NSE BSE SEBI"),
        "USA" => ("United States NYSE NASDAQ", "SEC NYSE NASDAQ"),
        "UK" => ("United Kingdom London Stock Exchange", "LSE FCA"),
        "Germany" => ("Germany Frankfurt Xetra", "Deutsche Boerse BaFin"),
        "Japan" => ("Japan Tokyo Stock Exchange", "JPX FSA"),
        _ => (
            profile.country.as_str(),
            "stock exchange securities regulator",
        ),
    };
    let week = format!("{scope} {official} corporate actions dividends splits bonus rights buyback filings when:7d");
    let month = format!("{scope} listed companies dividends bonus rights split merger acquisition earnings corporate action when:30d");
    let trusted = format!("{scope} corporate actions Reuters Bloomberg CNBC Economic Times Business Standard Financial Express Mint when:30d");
    let (a, b, c) = tokio::join!(
        news_search(week, 30),
        news_search(month, 40),
        news_search(trusted, 30)
    );
    let mut items = a;
    items.extend(b);
    items.extend(c);
    items.retain(headline_is_probably_english);
    items.retain(headline_is_story);
    // The plugin already sorts by publication time; preserve freshness ordering.
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        let key = item["url"]
            .as_str()
            .unwrap_or_else(|| item["title"].as_str().unwrap_or_default())
            .to_ascii_lowercase();
        !key.is_empty() && seen.insert(key)
    });
    items.truncate(60);
    Ok(
        json!({"items":items,"scope":format!("{scope}; official-exchange, regulator and established business coverage from the last 7–30 days")}),
    )
}

fn headline_is_probably_english(item: &Value) -> bool {
    let Some(title) = item.get("title").and_then(Value::as_str) else {
        return false;
    };
    let letters = title.chars().filter(|ch| ch.is_alphabetic()).count();
    if letters == 0 {
        return false;
    }
    let latin = title.chars().filter(|ch| ch.is_ascii_alphabetic()).count();
    latin * 100 / letters >= 72
}

fn headline_is_story(item: &Value) -> bool {
    let title = item["title"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if title.len() < 18 {
        return false;
    }
    let generic = [
        "breaking news",
        "latest news",
        "top stories",
        "news live",
        "live updates",
        "today's news",
        "today news",
        "odia news",
        "odisha news",
    ];
    !generic
        .iter()
        .any(|label| title == *label || title.starts_with(&format!("{label} -")))
}

fn news_source_priority(item: &Value) -> u8 {
    let source = item["source"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let trusted = [
        "ani",
        "press trust of india",
        "pti",
        "reuters",
        "associated press",
        "ap news",
        "bbc",
        "dd news",
        "ndtv",
        "cnn-news18",
        "news18",
        "india today",
        "the hindu",
        "indian express",
        "hindustan times",
        "times of india",
        "economic times",
        "mint",
        "business standard",
        "financial express",
        "cnbc-tv18",
        "msn",
    ];
    if trusted.iter().any(|name| source.contains(name)) {
        2
    } else {
        1
    }
}

fn breaking_source_is_noise(item: &Value) -> bool {
    let source = item["source"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["kanak news", "odia news", "odisha tv", "otv"]
        .iter()
        .any(|name| source.contains(name))
}

#[tauri::command]
pub async fn news_widget(
    state: tauri::State<'_, AppState>,
    category: String,
    source: Option<String>,
    force: Option<bool>,
) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err(
            "Enable live information and external-service consent in Profile to fetch news.".into(),
        );
    }
    let result = crate::editorial::fetch_language(
        category,
        source.unwrap_or("all".into()),
        p.location.clone(),
        p.region.clone(),
        p.country.clone(),
        force.unwrap_or(false),
        p.language.clone(),
    )
    .await?;
    let current = state.engine.lock().await.profile();
    if current.location != p.location
        || current.region != p.region
        || current.country != p.country
        || current.language != p.language
        || !current.consents.live_information
        || !current.consents.external_services
    {
        return Err(
            "Location or consent changed during news refresh. Refresh the selected location."
                .into(),
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn local_events_widget(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    let query = format!("{} events today culture community", profile.location);
    let result = buddy_plugin_news::NewsPlugin::new()
        .execute(PluginRequest {
            capability: "news.search".into(),
            params: json!({"query":query}),
        })
        .await
        .map_err(|error| error.to_string())?;
    let data = result.data;
    let total = data["items"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    state.engine.lock().await.log(&format!(
        "local events refresh {}: {total} items",
        profile.location
    ));
    Ok(data)
}

async fn news_search(query: String, limit: usize) -> Vec<Value> {
    buddy_plugin_news::NewsPlugin::new()
        .execute(PluginRequest {
            capability: "news.search".into(),
            params: json!({"query":query}),
        })
        .await
        .ok()
        .and_then(|response| response.data["items"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .take(limit)
        .collect()
}

async fn calendar_news(query: String) -> Vec<Value> {
    news_search(query, 8).await
}

#[tauri::command]
pub async fn calendar_day_context(
    state: tauri::State<'_, AppState>,
    year: i32,
    month: u32,
    day: u32,
) -> Result<Value, String> {
    use chrono::Datelike;
    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| "Calendar date is invalid".to_string())?;
    let profile = state.engine.lock().await.profile();
    let label = date.format("%B %-d, %Y").to_string();
    let local_query = format!("{} events {}", profile.location, label);
    let regional_query = format!("{} events {}", profile.region, label);
    let national_query = format!("{} events holidays {}", profile.country, label);
    let science_query = format!("on this day science history {}", date.format("%B %-d"));
    let wiki_url = format!(
        "https://en.wikipedia.org/api/rest_v1/feed/onthisday/events/{:02}/{:02}",
        date.month(),
        date.day()
    );
    let wiki = async {
        let response = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|error| error.to_string())?
            .get(wiki_url)
            .header("User-Agent", "BestBuddy/0.6.12 Wikimedia calendar reader")
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Wikimedia returned HTTP {}", response.status()));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(value["events"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(12)
            .map(|event| {
                let url = event["pages"][0]["content_urls"]["desktop"]["page"]
                    .as_str()
                    .unwrap_or("https://en.wikipedia.org/wiki/Portal:Current_events");
                json!({"year":event["year"],"text":event["text"],"url":url,"source":"Wikipedia · On this day"})
            })
            .collect::<Vec<_>>())
    };
    let (local, regional, national, science, history) = tokio::join!(
        calendar_news(local_query),
        calendar_news(regional_query),
        calendar_news(national_query),
        calendar_news(science_query),
        wiki
    );
    let history_error = history.as_ref().err().cloned();
    let history = history.unwrap_or_default();
    state.engine.lock().await.log(&format!(
        "calendar context {}: local={} regional={} national={} science={} history={}",
        date,
        local.len(),
        regional.len(),
        national.len(),
        science.len(),
        history.len()
    ));
    Ok(json!({
        "date":date.to_string(), "local":local, "regional":regional,
        "national":national, "science":science, "history":history,
        "history_error":history_error,
        "attribution":"Historical entries: Wikipedia contributors (CC BY-SA); open the source for details."
    }))
}

#[tauri::command]
pub async fn news_coverage(title: String, source: String, url: String) -> Result<Value, String> {
    if title.trim().len() < 8 || title.len() > 240 || title.contains(['\r', '\n']) {
        return Err("Headline is invalid for related coverage.".into());
    }
    let query = coverage_words(&title)
        .into_iter()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return Err("Headline has no searchable words.".into());
    }
    let response = buddy_plugin_news::NewsPlugin::new()
        .execute(PluginRequest {
            capability: "news.search".into(),
            params: json!({"query":query}),
        })
        .await
        .map_err(|e| e.to_string())?;
    let candidates = response.data["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let matches = related_coverage(&candidates, &title, &source, &url);
    Ok(json!({"items":matches,"query":query,"searched":candidates.len()}))
}

#[tauri::command]
pub async fn visible_news_set(
    state: tauri::State<'_, AppState>,
    headlines: Vec<String>,
) -> Result<(), String> {
    state.engine.lock().await.set_visible_headlines(headlines);
    Ok(())
}

#[tauri::command]
pub async fn news_videos_widget(
    state: tauri::State<'_, AppState>,
    category: String,
) -> Result<Value, String> {
    let result = buddy_plugin_news::NewsPlugin::new()
        .execute(PluginRequest {
            capability: "news.videos".into(),
            params: json!({"category":category}),
        })
        .await
        .map_err(|e| e.to_string());
    match result {
        Ok(response) => {
            let total = response.data["items"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0);
            state
                .engine
                .lock()
                .await
                .log(&format!("news videos {category}: {total} available"));
            Ok(response.data)
        }
        Err(error) => {
            state
                .engine
                .lock()
                .await
                .log(&format!("news videos failed: {error}"));
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn read_logs(state: tauri::State<'_, AppState>) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let path = state.engine.lock().await.log_path();
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(65536)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tauri::command]
pub async fn contact_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    contact_id: String,
    channel: Option<String>,
) -> Result<(), String> {
    let e = state.engine.lock().await;
    let c = e
        .user
        .contacts
        .iter()
        .find(|c| c.id == contact_id)
        .ok_or("Contact not found")?;
    let phone: String = c.phone.chars().filter(|c| c.is_ascii_digit()).collect();
    let selected = channel.as_deref().unwrap_or(&c.channel);
    let url = match selected {
        "messenger" => {
            let parsed = reqwest::Url::parse(&c.messenger_link).map_err(|_| {
                "Add a Messenger conversation link copied from your browser.".to_string()
            })?;
            let valid_host_path = match parsed.host_str() {
                Some("m.me") => !parsed.path().trim_matches('/').is_empty(),
                Some("www.messenger.com") | Some("messenger.com") => {
                    parsed.path().starts_with("/t/") && parsed.path().len() > 3
                }
                Some("www.facebook.com") | Some("facebook.com") => {
                    parsed.path().starts_with("/messages/t/") && parsed.path().len() > 12
                }
                _ => false,
            };
            if parsed.scheme() != "https" || !valid_host_path {
                return Err(
                    "Use a contact-specific HTTPS m.me or Messenger conversation link.".into(),
                );
            }
            parsed.to_string()
        }
        "signal" => {
            let parsed = reqwest::Url::parse(&c.signal_link).map_err(|_| {
                "Add this contact's Signal share link in their profile.".to_string()
            })?;
            if parsed.scheme() != "https" || parsed.host_str() != Some("signal.me") {
                return Err("Signal links must be copied from signal.me.".into());
            }
            parsed.to_string()
        }
        "slack" | "discord" | "zoom" | "teams" => {
            let saved = match selected {
                "slack" => &c.slack_link,
                "discord" => &c.discord_link,
                "zoom" => &c.zoom_link,
                _ => &c.teams_link,
            };
            approved_contact_link(selected, saved)?
        }
        "email" => {
            if !c.email.contains('@') || c.email.contains(['\r', '\n']) {
                return Err("A valid email address is required.".into());
            }
            format!("mailto:{}", urlencoding::encode(&c.email))
        }
        "whatsapp" | "telegram" | "phone" | "sms" => {
            if !(7..=15).contains(&phone.len()) {
                return Err("Enter a phone number including its country code.".into());
            }
            match selected {
                "whatsapp" => format!("https://wa.me/{phone}"),
                "telegram" => format!("https://t.me/+{phone}"),
                "phone" => format!("tel:+{phone}"),
                _ => format!("sms:+{phone}"),
            }
        }
        _ => return Err("This connector is not configured.".into()),
    };
    // Open only a constructed URI; never pass contact input through a command shell.
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn approved_contact_link(channel: &str, saved: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(saved).map_err(|_| {
        format!("Add a valid {channel} conversation or meeting link in this contact's profile.")
    })?;
    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path();
    let allowed = match channel {
        "slack" => {
            (host == "slack.com" || host.ends_with(".slack.com"))
                && (path.starts_with("/client/")
                    || path.starts_with("/archives/")
                    || path.starts_with("/app_redirect"))
        }
        "discord" => {
            (host == "discord.com" || host == "www.discord.com") && path.starts_with("/channels/")
        }
        "zoom" => (host == "zoom.us" || host.ends_with(".zoom.us")) && path.starts_with("/j/"),
        "teams" => {
            (host == "teams.microsoft.com" && path.starts_with("/l/"))
                || (host == "teams.live.com" && path.starts_with("/meet/"))
        }
        _ => false,
    };
    if parsed.scheme() != "https"
        || !allowed
        || parsed.username() != ""
        || parsed.password().is_some()
    {
        return Err(format!(
            "Use a contact-specific HTTPS {channel} link from the official app."
        ));
    }
    Ok(parsed.to_string())
}

pub async fn run_prefetch(
    engine: &std::sync::Arc<tokio::sync::Mutex<buddy_core::BuddyEngine>>,
) -> Result<Value, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Prefetch is already running.".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    let p = engine.lock().await.profile();
    // Personalized region/city news searches leave the device. Never prefetch
    // them in the background before registration and affirmative data consent.
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err("Live information prefetch requires onboarding and consent for live information and external services.".into());
    }
    let mut tasks = tokio::task::JoinSet::new();
    for category in crate::editorial::CATEGORIES {
        let p = p.clone();
        let category = category.to_string();
        tasks.spawn(async move {
            crate::editorial::fetch_language(
                category,
                "all".into(),
                p.location,
                p.region,
                p.country,
                true,
                p.language,
            )
            .await
            .map(crate::editorial::as_response)
        });
    }
    for interest in p.interests.iter().take(12) {
        let interest = interest.clone();
        let country = p.country.clone();
        tasks.spawn(async move { crate::editorial::topic(interest, country).await });
    }
    let mut conversation_tasks = tokio::task::JoinSet::new();
    let conversation_permits = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    for feed in buddy_core::conversation::FEEDS {
        let permits = conversation_permits.clone();
        conversation_tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.map_err(|e| e.to_string())?;
            buddy_core::conversation::fetch_feed(feed).await
        });
    }
    let mut responses = Vec::new();
    let mut errors = Vec::new();
    let mut cues = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(r)) => responses.push(r),
            Ok(Err(e)) => errors.push(e.to_string()),
            Err(e) => errors.push(e.to_string()),
        }
    }
    while let Some(result) = conversation_tasks.join_next().await {
        match result {
            Ok(Ok(items)) => cues.extend(items),
            Ok(Err(error)) => errors.push(error),
            Err(error) => errors.push(error.to_string()),
        }
    }
    let mut e = engine.lock().await;
    let current = e.profile();
    if !current.onboarding_complete
        || !current.consents.live_information
        || !current.consents.external_services
        || current.location != p.location
        || current.region != p.region
        || current.country != p.country
        || current.interests != p.interests
        || current.language != p.language
    {
        crate::editorial::clear().await;
        return Err("Prefetch cancelled after profile or consent change".into());
    }
    serde_json::to_value(e.accept_prefetch(responses, cues, errors)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn source_catalog(
    state: tauri::State<'_, AppState>,
    exchange: Option<String>,
) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err(
            "Enable live information and external services to curate local sources.".into(),
        );
    }
    let mut news = serde_json::Map::new();
    for category in crate::editorial::CATEGORIES {
        news.insert(
            (*category).into(),
            crate::editorial::catalog(
                category,
                p.location.clone(),
                p.region.clone(),
                p.country.clone(),
            )
            .await,
        );
    }
    let country =
        buddy_plugin_market::country_for_exchange(exchange.as_deref().unwrap_or("all"), &p.country);
    let market =
        crate::editorial::catalog("business-national", p.location, p.region, country).await;
    Ok(json!({"news":news,"market":market,"exchanges":buddy_plugin_market::exchange_catalog()}))
}

pub async fn prefetch_market_prices(
    engine: &std::sync::Arc<tokio::sync::Mutex<buddy_core::BuddyEngine>>,
) {
    let p = engine.lock().await.profile();
    let _ = buddy_plugin_market::set_refresh_minutes(p.market_refresh_minutes);
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return;
    }
    if let Some(exchange) = buddy_plugin_market::home_exchange(&p.country) {
        let _ = buddy_plugin_market::MarketPlugin::new()
            .execute(PluginRequest {
                capability: "market.dashboard".into(),
                params: json!({"exchange":exchange}),
            })
            .await;
    }
    for list in p.market_watchlists.iter().take(10) {
        let _ = buddy_plugin_market::quote_symbols(
            list.symbols
                .iter()
                .map(|item| {
                    (
                        item.name.clone(),
                        item.symbol.clone(),
                        item.exchange.clone(),
                    )
                })
                .collect(),
        )
        .await;
    }
}

#[tauri::command]
pub async fn market_watchlist(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    buddy_plugin_market::set_refresh_minutes(profile.market_refresh_minutes)?;
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err("Enable live information and external services for watchlist prices.".into());
    }
    let list = profile
        .market_watchlists
        .iter()
        .find(|list| list.id == id)
        .ok_or("Save this watchlist before refreshing prices.")?;
    Ok(buddy_plugin_market::quote_symbols(
        list.symbols
            .iter()
            .map(|item| {
                (
                    item.name.clone(),
                    item.symbol.clone(),
                    item.exchange.clone(),
                )
            })
            .collect(),
    )
    .await)
}

#[tauri::command]
pub async fn market_analyst_news(
    state: tauri::State<'_, AppState>,
    name: String,
    symbol: String,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err("Enable live information and external services for analyst coverage.".into());
    }
    if name.trim().is_empty()
        || name.chars().count() > 120
        || symbol.len() > 32
        || !symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "^.-=".contains(c))
    {
        return Err("Company selection is invalid".into());
    }
    let query = format!("\"{name}\" (analyst OR brokerage OR rating OR \"price target\") (site:finance.yahoo.com OR site:moneycontrol.com OR site:cnbc.com OR site:economictimes.indiatimes.com OR site:reuters.com)");
    let items = news_search(query, 12).await;
    Ok(
        json!({"items":items,"note":"Published analyst coverage when available; no consensus rating is inferred from headlines."}),
    )
}

#[tauri::command]
pub async fn llm_local_catalog(
    state: tauri::State<'_, AppState>,
    allow_network: Option<bool>,
    scan_folder: Option<String>,
    local_only: Option<bool>,
) -> Result<Value, String> {
    let permitted = state
        .engine
        .lock()
        .await
        .profile()
        .consents
        .external_services;
    let mut catalog = crate::model_catalog::catalog_cached(
        &state.llm.model_catalog_cache_path(),
        !local_only.unwrap_or(false) && (permitted || allow_network.unwrap_or(false)),
    )
    .await;
    if let Some(folder) = scan_folder.as_deref().filter(|v| !v.trim().is_empty()) {
        crate::local_services::remember_folder(folder)?;
    }
    let available = crate::local_services::discover().await;
    let status = state.llm.status().await;
    if let Some(models) = catalog["models"].as_array_mut() {
        for model in models {
            model["installed"] = serde_json::json!(status["local_models"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["id"] == model["id"])));
        }
    }
    catalog["local_available"] = serde_json::json!(available);
    catalog["scan_folders"] = serde_json::json!(crate::local_services::scan_roots()
        .into_iter()
        .map(|p| serde_json::json!({"path":p,"exists":p.is_dir()}))
        .collect::<Vec<_>>());
    Ok(catalog)
}

#[tauri::command]
pub fn speech_phrase(situation: String) -> String {
    buddy_core::phrases::phrase(&situation)
}

#[tauri::command]
pub async fn briefing_headlines(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    if !(p.onboarding_complete && p.consents.live_information && p.consents.external_services) {
        return Err("Enable live information and external-service consent for briefings.".into());
    }
    let mut tasks = tokio::task::JoinSet::new();
    for category in crate::editorial::CATEGORIES {
        let category = category.to_string();
        let p = p.clone();
        tasks.spawn(async move {
            let data = crate::editorial::fetch_language(
                category.clone(),
                "all".into(),
                p.location,
                p.region,
                p.country,
                false,
                p.language,
            )
            .await;
            (category, data)
        });
    }
    let mut categories = Vec::new();
    let mut errors = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((_, Ok(data))) => categories.push(data),
            Ok((category, Err(error))) => errors.push(format!("{category}: {error}")),
            Err(error) => errors.push(error.to_string()),
        }
    }
    categories.sort_by_key(|row| {
        crate::editorial::CATEGORIES
            .iter()
            .position(|category| Some(*category) == row["category"].as_str())
            .unwrap_or(99)
    });
    Ok(json!({"categories":categories,"errors":errors}))
}

/// Warning headlines are news coverage, never a substitute for an official alert feed.
#[tauri::command]
pub async fn weather_reports(
    state: tauri::State<'_, AppState>,
    location: String,
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err(
            "Enable live information and external services to fetch local weather articles.".into(),
        );
    }
    let location = location
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == ',')
        .take(120)
        .collect::<String>();
    if location.is_empty() {
        return Err("Choose a weather city first.".into());
    }
    static CACHE: std::sync::OnceLock<
        tokio::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, Value)>>,
    > = std::sync::OnceLock::new();
    let key = format!("{}|{}|{}", location, profile.region, profile.country);
    let cache = CACHE.get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some((time, data)) = cache.lock().await.get(&key) {
        if time.elapsed().as_secs() < 600 {
            return Ok(data.clone());
        }
    }
    let scope = if location.eq_ignore_ascii_case(&profile.location) {
        format!("{location} {} {}", profile.region, profile.country)
    } else {
        location.clone()
    };
    let (warning_rows, news_rows) = tokio::join!(
        news_search(
            format!("{scope} weather (warning OR alert OR storm OR flood OR heatwave) when:2d"),
            16
        ),
        news_search(
            format!("{scope} weather (forecast OR rain OR temperature OR storm) when:2d"),
            16
        )
    );
    let now = chrono::Utc::now();
    let mut seen = std::collections::HashSet::new();
    let warnings = warning_rows
        .into_iter()
        .filter(|row| {
            weather_report_is_fresh(row, now)
                && seen.insert(row["url"].as_str().unwrap_or("").to_string())
        })
        .take(6)
        .collect::<Vec<_>>();
    let items = news_rows
        .into_iter()
        .filter(|row| {
            weather_report_is_fresh(row, now)
                && seen.insert(row["url"].as_str().unwrap_or("").to_string())
        })
        .take(6)
        .collect::<Vec<_>>();
    let official = match profile.country.trim().to_ascii_lowercase().as_str() {
        "india" | "in" => Some("https://mausam.imd.gov.in/responsive/districtWiseWarningGIS.php"),
        "united kingdom" | "uk" | "gb" => {
            Some("https://weather.metoffice.gov.uk/warnings-and-advice/uk-warnings")
        }
        "united states" | "usa" | "us" => Some("https://www.weather.gov/alerts"),
        _ => None,
    };
    let data = json!({"location":location,"warnings":warnings,"items":items,"official_url":official,"warning_type":"news coverage, not an official alert feed","fetched_at":now.to_rfc3339()});
    let mut cache = cache.lock().await;
    if cache.len() >= 24 {
        cache.clear();
    }
    cache.insert(key, (std::time::Instant::now(), data.clone()));
    Ok(data)
}
fn weather_report_is_fresh(row: &Value, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Some(text) = row["published"].as_str() else {
        return false;
    };
    let date = chrono::DateTime::parse_from_rfc2822(text)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(text));
    date.is_ok_and(|date| {
        date >= now - chrono::Duration::hours(48) && date <= now + chrono::Duration::minutes(5)
    }) && row["title"]
        .as_str()
        .is_some_and(|s| s.trim().chars().count() > 15)
        && row["url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://"))
}
#[cfg(test)]
mod weather_report_tests {
    use super::*;
    #[test]
    fn warning_reports_require_recent_dated_articles() {
        let now = chrono::Utc::now();
        let row = json!({"title":"City issues a heavy rainfall warning","url":"https://example.com/news/rainfall-warning","published":now.to_rfc2822()});
        assert!(weather_report_is_fresh(&row, now));
        let mut old = row.clone();
        old["published"] = json!((now - chrono::Duration::days(3)).to_rfc2822());
        assert!(!weather_report_is_fresh(&old, now));
        let mut undated = row.clone();
        undated["published"] = Value::Null;
        assert!(!weather_report_is_fresh(&undated, now));
    }
}

#[tauri::command]
pub async fn voice_welcome() -> Result<(), String> {
    let epoch = SPEECH_EPOCH.load(Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        buddy_voice::play_welcome_while(|| {
            !quiet_active() && epoch == SPEECH_EPOCH.load(Ordering::SeqCst)
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn llm_prepare_default(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    state.llm.prepare_default().await
}

#[tauri::command]
pub async fn research_fetch(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<String, String> {
    let profile = state.engine.lock().await.profile();
    if !(profile.onboarding_complete
        && profile.consents.live_information
        && profile.consents.external_services)
    {
        return Err("Online research is off".into());
    }
    crate::public_web::fetch(&url).await
}
#[tauri::command]
pub async fn research_ingest(
    state: tauri::State<'_, AppState>,
    rows: Vec<buddy_core::research::ResearchExtract>,
) -> Result<usize, String> {
    state.engine.lock().await.accept_research(rows)
}

#[tauri::command]
pub async fn companion_invitation(
    state: tauri::State<'_, AppState>,
    topic: String,
) -> Result<String, String> {
    let topic = topic.trim().chars().take(64).collect::<String>();
    let (local, public) = {
        let engine = state.engine.lock().await;
        (
            engine.llm_context(&topic, true),
            engine.llm_context(&topic, false),
        )
    };
    let fallback = if topic.is_empty() {
        "What has caught your curiosity today?".into()
    } else {
        format!("With {topic}, what would you enjoy exploring next?")
    };
    let prompt=format!("Invite your companion to share about {topic}. Return one short relevant question with gentle wit, at most 35 words. Respect stated dislikes. No diagnosis, invented news, human memories or feelings. Do not repeat recent openings. Avoid humour if the context is distressing.");
    match state
        .llm
        .complete(&prompt, &fallback, &local, &public)
        .await
    {
        Ok(Some((text, _)))
            if text.chars().filter(|ch| *ch == '?').count() == 1 && text.trim().ends_with('?') =>
        {
            Ok(text)
        }
        _ => Ok(fallback),
    }
}
