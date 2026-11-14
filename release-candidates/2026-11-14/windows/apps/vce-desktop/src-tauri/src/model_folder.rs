/// Windows folder selection without adding a new dependency or opening a terminal.
#[tauri::command]
pub async fn llm_pick_model_folder() -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(|| {
            use std::os::windows::process::CommandExt;
            let script = r#"$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.Windows.Forms; $pick=New-Object System.Windows.Forms.FolderBrowserDialog; $pick.Description='Select your local model library folder'; $pick.ShowNewFolderButton=$false; try { if($pick.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK){[Console]::Write($pick.SelectedPath)} } finally {$pick.Dispose()}"#;
            let output = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-STA", "-Command", script])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| format!("Could not open the folder chooser: {e}"))?;
            if !output.status.success() {
                return Err("The Windows folder chooser did not complete. Try again.".into());
            }
            let path = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
            let path = path.trim().to_string();
            if path.is_empty() { Ok(None) } else { Ok(Some(path)) }
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Desktop model-folder import is unavailable on this platform.".into())
    }
}
