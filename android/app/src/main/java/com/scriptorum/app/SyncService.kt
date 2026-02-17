package com.scriptorum.app

import android.app.Activity
import android.os.Environment
import java.io.File

/**
 * Orchestrates the full sync flow:
 * 1. Open WiFi panel (user enables WiFi)
 * 2. Bring WireGuard tunnel up
 * 3. Wait for connectivity
 * 4. Run Rust sync via JNI
 * 5. Bring WireGuard tunnel down
 * 6. Open WiFi panel (user disables WiFi)
 */
class SyncService(
    private val serverUrl: String,
    private val tunnelName: String,
    private val notePath: String = File(Environment.getExternalStorageDirectory(), "Note").absolutePath,
) {
    fun interface OnLog {
        fun log(message: String)
    }

    /**
     * Run the sync flow on the calling thread (must not be the main thread).
     */
    fun execute(activity: Activity, onLog: OnLog) {
        try {
            // Step 1: WiFi on
            onLog.log("Opening WiFi panel — please enable WiFi")
            activity.runOnUiThread { WifiController.openWifiPanel(activity) }
            Thread.sleep(5000)

            // Step 2: WireGuard up
            onLog.log("Bringing WireGuard tunnel '$tunnelName' up...")
            WireGuardController.tunnelUp(activity, tunnelName)
            onLog.log("Waiting for tunnel...")
            Thread.sleep(3000)

            // Step 3: Sync
            onLog.log("Starting sync to $serverUrl")
            onLog.log("Note path: $notePath")

            val result = NativeBridge.performSync(serverUrl, notePath, object : NativeBridge.ProgressCallback {
                override fun onProgress(message: String) {
                    onLog.log(message)
                }
            })

            onLog.log("Result: $result")
        } catch (e: Exception) {
            onLog.log("ERROR: ${e.message}")
        } finally {
            // Step 4: WireGuard down
            onLog.log("Bringing WireGuard tunnel down...")
            WireGuardController.tunnelDown(activity, tunnelName)

            // Step 5: WiFi off
            onLog.log("Opening WiFi panel — please disable WiFi")
            activity.runOnUiThread { WifiController.openWifiPanel(activity) }
        }
    }
}
