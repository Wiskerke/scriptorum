package com.scriptorum.app

import android.app.Activity
import android.content.Intent
import android.provider.Settings

/**
 * Opens the system WiFi settings panel.
 *
 * On Android 11+ (API 30), apps cannot toggle WiFi programmatically.
 * Instead we open the system panel where the user taps to enable/disable WiFi.
 */
object WifiController {

    fun openWifiPanel(activity: Activity) {
        val intent = Intent(Settings.Panel.ACTION_WIFI)
        activity.startActivity(intent)
    }
}
