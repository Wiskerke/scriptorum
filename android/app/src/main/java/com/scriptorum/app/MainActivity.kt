package com.scriptorum.app

import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.widget.Button
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import org.json.JSONObject

class MainActivity : AppCompatActivity() {
    private lateinit var btnSync: Button
    private lateinit var btnClose: Button
    private lateinit var tvLog: TextView
    private lateinit var scrollLog: ScrollView

    private val serverUrl: String by lazy {
        val json = assets.open("config.json").bufferedReader().readText()
        JSONObject(json).getString("server_url")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        btnSync = findViewById(R.id.btnSync)
        btnClose = findViewById(R.id.btnClose)
        tvLog = findViewById(R.id.tvLog)
        scrollLog = findViewById(R.id.scrollLog)

        btnSync.setOnClickListener { startSync() }
        btnClose.setOnClickListener { finish() }

        ensureStoragePermission()
    }

    private fun ensureStoragePermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!Environment.isExternalStorageManager()) {
                log("Requesting MANAGE_EXTERNAL_STORAGE permission...")
                val intent = Intent(
                    Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                    Uri.parse("package:$packageName")
                )
                startActivity(intent)
            }
        }
    }

    private fun startSync() {
        btnSync.isEnabled = false
        tvLog.text = ""
        log("=== Sync started ===")

        val syncService = SyncService(
            serverUrl = serverUrl,
            caCertPem = assets.open("certs/ca.pem").bufferedReader().readText(),
            clientCertPem = assets.open("certs/client.pem").bufferedReader().readText(),
            clientKeyPem = assets.open("certs/client-key.pem").bufferedReader().readText(),
        )

        Thread {
            syncService.execute(this) { message ->
                runOnUiThread { log(message) }
            }
            runOnUiThread {
                log("=== Sync finished ===")
                btnSync.isEnabled = true
            }
        }.start()
    }

    private fun log(message: String) {
        tvLog.append("$message\n")
        scrollLog.post { scrollLog.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
