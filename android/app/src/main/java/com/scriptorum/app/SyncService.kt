package com.scriptorum.app

import android.app.Activity
import android.content.Context
import android.os.Environment
import java.io.File

/**
 * Orchestrates the full sync flow:
 * 1. Ensure WiFi is connected
 * 2. Run Rust sync via JNI (mTLS)
 * 3. Open WiFi panel (user disables WiFi)
 */
class SyncService(
    private val serverUrl: String,
    private val notePath: String = File(Environment.getExternalStorageDirectory(), "Note").absolutePath,
    private val caCertPem: String,
    private val clientCertPem: String,
    private val clientKeyPem: String,
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
            if (NetworkController.isWifiConnected(activity)) {
                onLog.log("WiFi already connected, skipping panel")
            } else {
                onLog.log("Opening WiFi panel — please enable WiFi")
                activity.runOnUiThread { NetworkController.openWifiPanel(activity) }
                if (!waitForWifi(activity, onLog)) {
                    onLog.log("ERROR: WiFi did not connect within timeout")
                    return
                }
            }

            // Step 2: Sync
            onLog.log("Starting sync to $serverUrl")
            onLog.log("Note path: $notePath")

            val result = NativeBridge.performSync(
                serverUrl, notePath, caCertPem, clientCertPem, clientKeyPem,
                object : NativeBridge.ProgressCallback {
                    override fun onProgress(message: String) {
                        onLog.log(message)
                    }
                },
            )

            onLog.log("Result: $result")
        } catch (e: Exception) {
            onLog.log("ERROR: ${e.message}")
        } finally {
            // Step 3: WiFi off
            onLog.log("Opening WiFi panel — please disable WiFi")
            activity.runOnUiThread { NetworkController.openWifiPanel(activity) }
        }
    }

    private fun waitForWifi(context: Context, onLog: OnLog): Boolean {
        val timeoutMs = 20_000L
        val pollMs = 500L
        val deadline = System.currentTimeMillis() + timeoutMs

        onLog.log("Waiting for WiFi connection (${timeoutMs / 1000}s timeout)...")
        while (System.currentTimeMillis() < deadline) {
            if (NetworkController.isWifiConnected(context)) {
                onLog.log("WiFi connected")
                return true
            }
            Thread.sleep(pollMs)
        }
        return false
    }
}
