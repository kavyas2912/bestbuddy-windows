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
        "resources":buddy_core::resource_budget(),
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
    let response = conversation_reply(&state, &input).await?;
    serde_json::to_value(response).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn cancel_chat(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.monitor.interrupt_playback();
    state.turns.cancel_and_wait().await;
    Ok(())
}
async fn conversation_reply(
    state: &AppState,
    input: &str,
) -> Result<buddy_core::BuddyResponse, String> {
    if input.chars().count() > 2000 {
        return Err("Please keep a conversation turn within 2,000 characters.".into());
    }
    state
        .turns
        .run_with_cleanup(
            Duration::from_secs(buddy_core::resource_budget().completion_seconds),
            compose_reply(state, input),
            || state.llm.cancel_local(),
        )
        .await
}
async fn compose_reply(state: &AppState, input: &str) -> Result<buddy_core::BuddyResponse, String> {
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
    state.engine.lock().await.finalize_reply(input, &r);
    Ok(r)
}

fn may_use_llm(capability: Option<&str>) -> bool {
    // A concrete answer from commands, phrasebook, RAG, or a source-backed
    // discussion is already the first-choice response. Models handle only the
    // open-ended fallback, so they cannot erase source attribution or actions.
    capability.is_none()
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
        assert!(!may_use_llm(Some("conversation.follow_up")));
        assert!(!may_use_llm(Some("conversation.local")));
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
        json!({"installed":voice.piper_voices(),"available":available,"selected":voice.current_piper_voice(),"auto":state.voice_auto.load(Ordering::SeqCst)}),
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
pub fn voice_events(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let events = state.monitor.take_events();
    if !state.voice_enabled.load(Ordering::SeqCst) {
        return Ok(json!([]));
    }
    serde_json::to_value(events).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn voice_arm(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.turns.cancel_and_wait().await;
    require_microphone_consent(&state).await?;
    crate::microphone::ensure(&app).await?;
    require_microphone_consent(&state).await?;
    state.voice_enabled.store(true, Ordering::SeqCst);
    if state.monitor.status().always_on {
        return state.monitor.arm_manual().map_err(|e| e.to_string());
    }
    let voice = std::sync::Arc::clone(&state.voice);
    let enabled = std::sync::Arc::clone(&state.voice_enabled);
    tauri::async_runtime::spawn_blocking(move || {
        if !enabled.load(Ordering::SeqCst) {
            return Err("Voice consent was revoked.".into());
        }
        let mut v = voice.lock().map_err(|e| e.to_string())?;
        v.warm_up().map_err(|e| e.to_string())?;
        if !enabled.load(Ordering::SeqCst) {
            return Err("Voice consent was revoked.".into());
        }
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
    let voice = std::sync::Arc::clone(&state.voice);
    let automatic = state.voice_auto.load(Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let mut v = voice.lock().map_err(|e| e.to_string())?;
        let result = if automatic {
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
pub async fn voice_test_microphone(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    require_microphone_consent(&state).await?;
    crate::microphone::ensure(&app).await?;
    require_microphone_consent(&state).await?;
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
    if !state.engine.lock().await.user.consents.voice_processing {
        return Err("Enable local voice processing before testing a WAV.".into());
    }
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
    let response = conversation_reply(&state, &test.transcript).await?;
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

    let response = conversation_reply(&state, &transcript).await?;
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
    profile: UserProfile,
) -> Result<Value, String> {
    let mut e = state.engine.lock().await;
    e.set_profile(profile)?;
    let enabled = e.voice_allowed();
    let saved = serde_json::to_value(e.profile()).map_err(|x| x.to_string())?;
    drop(e);
    sync_voice_consent(&state, enabled).await?;
    Ok(saved)
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
    sync_voice_consent(&state, false).await?;
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
pub async fn market_widget(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let result = buddy_plugin_market::MarketPlugin::new()
        .execute(PluginRequest {
            capability: "market.dashboard".into(),
            params: json!({}),
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
    _state: tauri::State<'_, AppState>,
    symbol: String,
) -> Result<Value, String> {
    let result = buddy_plugin_market::MarketPlugin::new()
        .execute(PluginRequest {
            capability: "market.detail".into(),
            params: json!({"symbol":symbol}),
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.data)
}

#[tauri::command]
pub async fn market_company_news(name: String, symbol: String) -> Result<Value, String> {
    if name.trim().is_empty() || name.len() > 100 || symbol.len() > 24 {
        return Err("Company selection is invalid".into());
    }
    let query = format!("{name} {symbol} company news");
    let filings_query = format!(
        "{name} {symbol} results earnings dividend bonus split acquisition corporate action"
    );
    let sector_query = format!("{name} {symbol} Reuters CNBC TV18 Economic Times Mint business");
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
) -> Result<Value, String> {
    let profile = state.engine.lock().await.profile();
    let scope = match exchange.as_str() {
        "India" => "India NSE BSE",
        "USA" => "United States NYSE NASDAQ",
        "UK" => "United Kingdom London Stock Exchange",
        "Germany" => "Germany Frankfurt DAX",
        "Japan" => "Japan Tokyo Stock Exchange Nikkei",
        _ => profile.country.as_str(),
    };
    let market_query =
        format!("{scope} stock market news Reuters CNBC Bloomberg exchange when:30d");
    let actions_query = format!("{scope} listed companies dividend bonus split merger acquisition earnings corporate actions when:30d");
    let exchange_query = format!("{scope} stock exchange notices corporate filings when:30d");
    let (market, actions, notices) = tokio::join!(
        news_search(market_query, 24),
        news_search(actions_query, 24),
        news_search(exchange_query, 24)
    );
    let mut items = market;
    items.extend(actions);
    items.extend(notices);
    items.retain(headline_is_probably_english);
    items.retain(headline_is_story);
    items.sort_by_key(|item| std::cmp::Reverse(news_source_priority(item)));
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        let key = item["url"]
            .as_str()
            .unwrap_or_else(|| item["title"].as_str().unwrap_or_default())
            .to_ascii_lowercase();
        !key.is_empty() && seen.insert(key)
    });
    items.truncate(40);
    Ok(
        json!({"items":items,"scope":format!("{scope}; recent market, company and corporate-action coverage") }),
    )
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
    items.sort_by_key(|item| std::cmp::Reverse(news_source_priority(item)));
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
) -> Result<Value, String> {
    let p = state.engine.lock().await.profile();
    let local_sources = if p.country.to_ascii_lowercase().contains("india") {
        "ANI PTI DD News The Hindu Indian Express Times of India Hindustan Times News18 NDTV"
    } else {
        "Reuters Associated Press BBC public broadcaster leading local newspaper"
    };
    let home = [p.location.as_str(), p.region.as_str(), p.country.as_str()]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let query = match category.as_str() {
        "breaking" => format!("{home} breaking top headlines {local_sources}"),
        "national" => format!("{} national news", p.country),
        "regional" => format!("{} {} regional news {local_sources}", p.region, p.country),
        "local" => format!(
            "{} {} local news events {local_sources}",
            p.location, p.region
        ),
        "political" => format!("{home} politics government news"),
        "business" => format!("{home} business economy company markets news"),
        "science" => format!("{} science research discoveries", p.country),
        "ai-tech" => format!("{} artificial intelligence technology news", p.country),
        "sports" => format!("{home} sports news"),
        "entertainment" => format!("{home} entertainment film music news"),
        "viral" => format!("{home} viral news social media internet trends"),
        _ => "world news".into(),
    };
    let result = buddy_plugin_news::NewsPlugin::new()
        .execute(PluginRequest {
            capability: "news.headlines".into(),
            params: json!({"query":query}),
        })
        .await
        .map_err(|e| e.to_string());
    match result {
        Ok(response) => {
            let mut data = response.data;
            // General feeds stay in English. Local and regional views may retain
            // the language used around the profile location.
            if !matches!(category.as_str(), "local" | "regional") {
                if let Some(items) = data.get_mut("items").and_then(Value::as_array_mut) {
                    items.retain(headline_is_probably_english);
                }
            }
            if let Some(items) = data.get_mut("items").and_then(Value::as_array_mut) {
                items.retain(headline_is_story);
                if category == "breaking" {
                    items.retain(|item| !breaking_source_is_noise(item));
                }
                items.sort_by_key(|item| std::cmp::Reverse(news_source_priority(item)));
                let mut seen = std::collections::HashSet::new();
                items.retain(|item| {
                    let key = item["url"]
                        .as_str()
                        .unwrap_or_else(|| item["title"].as_str().unwrap_or_default())
                        .to_ascii_lowercase();
                    !key.is_empty() && seen.insert(key)
                });
                items.truncate(40);
            }
            let items = data["items"].as_array();
            let total = items.map(|items| items.len()).unwrap_or(0);
            let images = items
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item["thumbnail"].is_string())
                        .count()
                })
                .unwrap_or(0);
            let videos = items
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item["video_url"].is_string())
                        .count()
                })
                .unwrap_or(0);
            state.engine.lock().await.log(&format!(
                "news refresh {category}: {total} stories, {images} images, {videos} videos"
            ));
            Ok(data)
        }
        Err(error) => {
            state
                .engine
                .lock()
                .await
                .log(&format!("news refresh {category} failed: {error}"));
            Err(error)
        }
    }
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
    let queries = vec![
        "world news".to_string(),
        format!("{} national news", p.country),
        format!("{} regional news", p.region),
        format!("{} local news", p.location),
    ];
    let mut tasks = tokio::task::JoinSet::new();
    let concurrency = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut seen = std::collections::HashSet::new();
    for query in queries
        .into_iter()
        .chain(p.interests.iter().map(|i| format!("{i} news")))
    {
        if query.trim().is_empty() || !seen.insert(query.to_lowercase()) {
            continue;
        }
        let permits = concurrency.clone();
        tasks.spawn(async move {
            let _permit = permits
                .acquire_owned()
                .await
                .map_err(|e| buddy_plugin::PluginError::Failed(e.to_string()))?;
            buddy_plugin_news::NewsPlugin::new()
                .execute(PluginRequest {
                    capability: "news.headlines".into(),
                    params: json!({"query":query}),
                })
                .await
        });
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
    serde_json::to_value(e.accept_prefetch(responses, cues, errors)).map_err(|e| e.to_string())
}

async fn require_microphone_consent(state: &AppState) -> Result<(), String> {
    if state.engine.lock().await.voice_allowed() {
        Ok(())
    } else {
        Err("Complete onboarding and enable microphone and local voice-processing consent in Profile & people before listening.".into())
    }
}

async fn sync_voice_consent(state: &AppState, enabled: bool) -> Result<(), String> {
    state.voice_enabled.store(enabled, Ordering::SeqCst);
    if !enabled {
        state.monitor.stop();
        let voice = std::sync::Arc::clone(&state.voice);
        tauri::async_runtime::spawn_blocking(move || {
            voice.lock().map_err(|e| e.to_string())?.stop_always_on();
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())??;
        state.monitor.take_events();
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_permission_request(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    require_microphone_consent(&state).await?;
    crate::microphone::ensure(&app).await
}

pub async fn run_local_learning(
    engine: &Arc<tokio::sync::Mutex<buddy_core::BuddyEngine>>,
    force: bool,
) -> Result<Value, String> {
    let policy = engine.lock().await.begin_local_learning(force);
    let Some(policy) = policy else {
        return Ok(engine.lock().await.local_learning_status());
    };
    // Fetch without holding the conversation engine lock: spoken turns remain responsive.
    let mut errors = Vec::new();
    for source in policy.sources {
        match buddy_core::local_learning::fetch(&source).await {
            Ok(documents) => {
                if let Err(error) = engine.lock().await.import_transcripts(documents) {
                    errors.push(format!("{}: {error}", source.name));
                }
            }
            Err(error) => errors.push(format!("{}: {error}", source.name)),
        }
    }
    let mut e = engine.lock().await;
    e.finish_local_learning(errors);
    Ok(e.local_learning_status())
}

#[tauri::command]
pub async fn local_learning_status(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Ok(state.engine.lock().await.local_learning_status())
}

#[tauri::command]
pub async fn local_learning_configure(
    state: tauri::State<'_, AppState>,
    policy: buddy_core::local_learning::LearningPolicy,
) -> Result<Value, String> {
    let mut e = state.engine.lock().await;
    if !e.user.consents.local_learning || !e.user.consents.personalization {
        return Err("Enable local learning and personalization in Profile & people first.".into());
    }
    let mut profile = e.profile();
    profile.local_learning = policy;
    e.set_profile(profile)?;
    Ok(e.local_learning_status())
}

#[tauri::command]
pub async fn local_learning_refresh(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    run_local_learning(&state.engine, true).await
}

#[tauri::command]
pub async fn local_learning_import(
    state: tauri::State<'_, AppState>,
    documents: Vec<buddy_core::local_learning::Transcript>,
) -> Result<Value, String> {
    let mut e = state.engine.lock().await;
    e.import_transcripts(documents)?;
    Ok(e.local_learning_status())
}

#[tauri::command]
pub async fn local_learning_clear(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let mut e = state.engine.lock().await;
    e.clear_local_learning()?;
    Ok(e.local_learning_status())
}
