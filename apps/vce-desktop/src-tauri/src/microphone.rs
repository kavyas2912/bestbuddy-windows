//! OS permission is requested only after the host's explicit voice-consent check.
#[cfg(target_os = "android")]
use tauri::Manager;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("buddy-microphone")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin("com.vce.buddy", "MicrophonePlugin")?;
                _app.manage(AndroidMicrophone(handle));
            }
            Ok(())
        })
        .build()
}
#[cfg(target_os = "android")]
struct AndroidMicrophone<R: tauri::Runtime>(tauri::plugin::PluginHandle<R>);

pub async fn ensure<R: tauri::Runtime>(_app: &tauri::AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let response: serde_json::Value = _app
            .state::<AndroidMicrophone<R>>()
            .0
            .run_mobile_plugin_async("request", serde_json::json!({}))
            .await
            .map_err(|e| e.to_string())?;
        if response.get("granted").and_then(|v| v.as_bool()) != Some(true) {
            return Err("Microphone permission was not granted. You can retry from Talk or enable it in Android Settings.".into());
        }
    }
    Ok(())
}
