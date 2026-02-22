package com.scriptorum.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.provider.Settings

/**
 * Manages WiFi state.
 *
 * On Android 11+ apps cannot toggle WiFi programmatically, so we open
 * the system panel for the user to enable/disable it manually.
 */
object NetworkController {

    fun openWifiPanel(activity: Activity) {
        val intent = Intent(Settings.Panel.ACTION_WIFI)
        activity.startActivity(intent)
    }

    fun isWifiConnected(context: Context): Boolean {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork ?: return false
        val caps = cm.getNetworkCapabilities(network) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
    }
}
