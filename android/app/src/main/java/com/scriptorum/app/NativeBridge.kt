package com.scriptorum.app

/**
 * JNI bridge to the Rust scriptorum-android library.
 */
object NativeBridge {
    init {
        System.loadLibrary("scriptorum_android")
    }

    /**
     * Scan the note directory and return a JSON manifest string.
     */
    external fun scanAndHash(notePath: String): String

    /**
     * Perform a full sync against the server.
     * Returns a JSON summary string with "uploaded" and "downloaded" counts.
     */
    external fun performSync(serverUrl: String, notePath: String, callback: ProgressCallback): String

    interface ProgressCallback {
        fun onProgress(message: String)
    }
}
