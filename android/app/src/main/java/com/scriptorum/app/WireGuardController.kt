package com.scriptorum.app

import android.content.Context
import android.content.Intent

/**
 * Controls WireGuard tunnels via broadcast intents.
 *
 * Requires the WireGuard app to be installed and the
 * com.wireguard.android.permission.CONTROL_TUNNELS permission.
 */
object WireGuardController {
    private const val ACTION_SET_TUNNEL_UP = "com.wireguard.android.action.SET_TUNNEL_UP"
    private const val ACTION_SET_TUNNEL_DOWN = "com.wireguard.android.action.SET_TUNNEL_DOWN"
    private const val EXTRA_TUNNEL = "tunnel"

    fun tunnelUp(context: Context, tunnelName: String) {
        val intent = Intent(ACTION_SET_TUNNEL_UP).apply {
            `package` = "com.wireguard.android"
            putExtra(EXTRA_TUNNEL, tunnelName)
        }
        context.sendBroadcast(intent)
    }

    fun tunnelDown(context: Context, tunnelName: String) {
        val intent = Intent(ACTION_SET_TUNNEL_DOWN).apply {
            `package` = "com.wireguard.android"
            putExtra(EXTRA_TUNNEL, tunnelName)
        }
        context.sendBroadcast(intent)
    }
}
