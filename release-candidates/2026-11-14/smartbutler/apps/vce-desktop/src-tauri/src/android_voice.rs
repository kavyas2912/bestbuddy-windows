use jni::{objects::JClass, sys::jboolean, JNIEnv};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
static ALLOWED: AtomicBool = AtomicBool::new(false);
static MONITOR: OnceLock<butler_voice::VoiceMonitor> = OnceLock::new();
pub fn register(monitor: butler_voice::VoiceMonitor) {
    let _ = MONITOR.set(monitor);
}
#[no_mangle]
pub extern "system" fn Java_com_smartbutler_app_VoicePlatform_nativeAllowed(
    _env: JNIEnv,
    _class: JClass,
    allowed: jboolean,
) {
    ALLOWED.store(allowed != 0, Ordering::SeqCst);
    if allowed == 0 {
        if let Some(monitor) = MONITOR.get() {
            monitor.stop();
        }
    }
}
pub fn ready() -> bool {
    ALLOWED.load(Ordering::SeqCst)
        && crate::android_storage::voice_call("status").unwrap_or(-1) == 1
}
pub fn stop() {
    ALLOWED.store(false, Ordering::SeqCst);
    let _ = crate::android_storage::voice_call("stop");
}
pub async fn ensure_ready() -> Result<(), String> {
    if ready() {
        return Ok(());
    }
    crate::android_storage::voice_call("requestStart")?;
    for _ in 0..120 {
        match crate::android_storage::voice_call("status")? {
            1 if ready() => return Ok(()),
            -1 => return Err("Microphone access is unavailable. Allow microphone and notification access and press Talk while the app is open.".into()),
            _ => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
        }
    }
    Err(
        "Microphone permission is still pending. Finish the Android prompt, then press Talk."
            .into(),
    )
}

#[no_mangle]
pub extern "system" fn Java_com_smartbutler_app_VoicePlatform_nativePauseDownload(
    _env: JNIEnv,
    _class: JClass,
) {
    crate::android_models::cancel();
}
