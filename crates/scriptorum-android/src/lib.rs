use jni::objects::{JClass, JObject, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use scriptorum_core::scanner::scan_directory;
use std::path::Path;

/// Scan the note directory and return a JSON manifest.
#[no_mangle]
pub extern "system" fn Java_com_scriptorum_app_NativeBridge_scanAndHash(
    mut env: JNIEnv,
    _class: JClass,
    note_path: JString,
) -> jstring {
    let result = (|| -> anyhow::Result<String> {
        let path: String = env.get_string(&note_path)?.into();
        let manifest = scan_directory(Path::new(&path))?;
        Ok(serde_json::to_string(&manifest)?)
    })();

    match result {
        Ok(json) => env.new_string(json).unwrap().into_raw(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            std::ptr::null_mut()
        }
    }
}

/// Perform a sync against the server, reporting progress via JNI callback.
#[no_mangle]
pub extern "system" fn Java_com_scriptorum_app_NativeBridge_performSync(
    mut env: JNIEnv,
    _class: JClass,
    server_url: JString,
    note_path: JString,
    callback: JObject,
) -> jstring {
    let result = (|| -> anyhow::Result<String> {
        let url: String = env.get_string(&server_url)?.into();
        let path: String = env.get_string(&note_path)?.into();

        let sync_result =
            scriptorum_core::client::perform_sync(&url, Path::new(&path), |msg| {
                let jmsg = env.new_string(msg).unwrap();
                let _ = env.call_method(
                    &callback,
                    "onProgress",
                    "(Ljava/lang/String;)V",
                    &[(&jmsg).into()],
                );
            })?;

        let summary = serde_json::json!({
            "uploaded": sync_result.uploaded,
            "downloaded": sync_result.downloaded,
        });
        Ok(summary.to_string())
    })();

    match result {
        Ok(json) => env.new_string(json).unwrap().into_raw(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            std::ptr::null_mut()
        }
    }
}
