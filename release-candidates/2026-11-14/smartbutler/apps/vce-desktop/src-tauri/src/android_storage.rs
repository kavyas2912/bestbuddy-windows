use jni::{
    objects::{GlobalRef, JByteArray, JClass, JObject, JValue},
    JavaVM,
};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, Runtime};

struct StorageBridge {
    vm: JavaVM,
    class: GlobalRef,
    voice_class: GlobalRef,
    _context: GlobalRef,
}
static BRIDGE: OnceLock<StorageBridge> = OnceLock::new();

pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.get_webview_window("main")
        .ok_or("Android main window is unavailable")?
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _| {
                let result = (|| -> jni::errors::Result<StorageBridge> {
                    let loader = env
                        .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
                        .l()?;
                    let name = env.new_string("com.smartbutler.app.ProtectedStorage")?;
                    let class = env
                        .call_method(
                            &loader,
                            "loadClass",
                            "(Ljava/lang/String;)Ljava/lang/Class;",
                            &[JValue::Object(&name)],
                        )?
                        .l()?;
                    let voice_name = env.new_string("com.smartbutler.app.VoicePlatform")?;
                    let voice_class = env
                        .call_method(
                            &loader,
                            "loadClass",
                            "(Ljava/lang/String;)Ljava/lang/Class;",
                            &[JValue::Object(&voice_name)],
                        )?
                        .l()?;
                    let context = env
                        .call_method(
                            activity,
                            "getApplicationContext",
                            "()Landroid/content/Context;",
                            &[],
                        )?
                        .l()?;
                    let context = env.new_global_ref(context)?;
                    let vm = env.get_java_vm()?;
                    Ok(StorageBridge {
                        vm,
                        class: env.new_global_ref(class)?,
                        voice_class: env.new_global_ref(voice_class)?,
                        _context: context,
                    })
                })()
                .map_err(|_| "Android encrypted storage could not be initialized".to_string());
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                let _ = tx.send(result);
            });
        })
        .map_err(|_| "Android context is unavailable".to_string())?;
    BRIDGE
        .set(
            rx.recv()
                .map_err(|_| "Android storage initialization interrupted")??,
        )
        .map_err(|_| "Android storage already initialized")?;
    // Locked TAO 0.35.3 does not initialize ndk-context; CPAL 0.16 requires it.
    // Do this once, before any audio workers start. BRIDGE retains the VM and a
    // global Application reference for the process lifetime (including rotation).
    // Revisit this initializer if upgrading TAO to a version that owns ndk-context.
    let bridge = BRIDGE.get().ok_or("Android audio context is unavailable")?;
    unsafe {
        ndk_context::initialize_android_context(
            bridge.vm.get_java_vm_pointer().cast(),
            bridge._context.as_obj().as_raw().cast(),
        );
    }
    butler_core::register_platform_protector(transform)
}

fn transform(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    let bridge = BRIDGE.get().ok_or("Android storage is unavailable")?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|_| "Android storage thread could not attach")?;
    let result = (|| -> jni::errors::Result<Vec<u8>> {
        let input = env.byte_array_from_slice(bytes)?;
        let class: &JClass = bridge.class.as_obj().into();
        let output = env
            .call_static_method(
                class,
                if encrypt { "encrypt" } else { "decrypt" },
                "([B)[B",
                &[JValue::Object(&JObject::from(input))],
            )?
            .l()?;
        env.convert_byte_array(JByteArray::from(output))
    })();
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    result.map_err(|_| "Android could not unlock or protect personal data. The encrypted file has not been replaced.".into())
}

pub fn voice_call(method: &str) -> Result<i32, String> {
    let bridge = BRIDGE.get().ok_or("Android platform is unavailable")?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|_| "Android voice thread could not attach")?;
    let class: &JClass = bridge.voice_class.as_obj().into();
    let result = env
        .call_static_method(class, method, "()I", &[])
        .and_then(|v| v.i());
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    result.map_err(|_| "Android microphone service could not complete this request".into())
}

pub fn erase_key() -> Result<(), String> {
    let bridge = BRIDGE.get().ok_or("Android storage is unavailable")?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|_| "Android storage thread could not attach")?;
    let class: &JClass = bridge.class.as_obj().into();
    let result = env.call_static_method(class, "eraseKey", "()I", &[]);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    result.map(|_| ()).map_err(|_| {
        "Personal files were erased, but Android could not remove the old storage key".into()
    })
}

pub fn voice_choice(choice: i32) -> Result<(), String> {
    let bridge = BRIDGE.get().ok_or("Android setup unavailable")?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|e| e.to_string())?;
    let class: &JClass = bridge.voice_class.as_obj().into();
    let result = env.call_static_method(class, "setDownloadChoice", "(I)I", &[JValue::Int(choice)]);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    result
        .map(|_| ())
        .map_err(|_| "Download choice could not be saved".into())
}
