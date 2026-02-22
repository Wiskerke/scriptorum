use jni::objects::{JClass, JObject, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use scriptorum_core::client::TlsConfig;
use scriptorum_core::scanner::scan_directory;
use std::path::Path;

/// Scan the note directory and return a JSON manifest.
#[no_mangle]
pub extern "system" fn Java_com_scriptorum_app_NativeBridge_scanAndHash(
    mut env: JNIEnv,
    _class: JClass,
    note_path: JString,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (|| -> anyhow::Result<String> {
            let path: String = env.get_string(&note_path)?.into();
            let manifest = scan_directory(Path::new(&path))?;
            Ok(serde_json::to_string(&manifest)?)
        })()
    }));

    match result {
        Ok(Ok(json)) => env
            .new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Ok(Err(e)) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            std::ptr::null_mut()
        }
        Err(panic) => {
            let msg = panic_message(&panic);
            let _ = env.throw_new("java/lang/RuntimeException", format!("native panic: {msg}"));
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
    ca_cert_pem: JString,
    client_cert_pem: JString,
    client_key_pem: JString,
    callback: JObject,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (|| -> anyhow::Result<String> {
            let url: String = env.get_string(&server_url)?.into();
            let path: String = env.get_string(&note_path)?.into();
            let ca_cert: String = env.get_string(&ca_cert_pem)?.into();
            let client_cert: String = env.get_string(&client_cert_pem)?.into();
            let client_key: String = env.get_string(&client_key_pem)?.into();

            let tls_config = TlsConfig {
                ca_cert_pem: ca_cert,
                client_cert_pem: client_cert,
                client_key_pem: client_key,
            };

            let sync_result = scriptorum_core::client::perform_sync(
                &url,
                Path::new(&path),
                Some(&tls_config),
                |msg| {
                    if let Ok(jmsg) = env.new_string(msg) {
                        let _ = env.call_method(
                            &callback,
                            "onProgress",
                            "(Ljava/lang/String;)V",
                            &[(&jmsg).into()],
                        );
                    }
                },
            )?;

            let summary = serde_json::json!({
                "uploaded": sync_result.uploaded,
                "downloaded": sync_result.downloaded,
            });
            Ok(summary.to_string())
        })()
    }));

    match result {
        Ok(Ok(json)) => env
            .new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Ok(Err(e)) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            std::ptr::null_mut()
        }
        Err(panic) => {
            let msg = panic_message(&panic);
            let _ = env.throw_new("java/lang/RuntimeException", format!("native panic: {msg}"));
            std::ptr::null_mut()
        }
    }
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
