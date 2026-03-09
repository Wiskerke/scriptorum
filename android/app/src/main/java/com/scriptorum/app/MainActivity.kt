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
import java.io.File

class MainActivity : AppCompatActivity() {
    private lateinit var btnSync: Button
    private lateinit var btnClose: Button
    private lateinit var tvLog: TextView
    private lateinit var scrollLog: ScrollView

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

        val dir = File(Environment.getExternalStorageDirectory(), "Scriptorum")
        val ca         = File(dir, "ca.pem").takeIf { it.exists() }?.readText()
        val cert       = File(dir, "client.pem").takeIf { it.exists() }?.readText()
        val key        = File(dir, "client-key.pem").takeIf { it.exists() }?.readText()
        val configJson = File(dir, "config.json").takeIf { it.exists() }?.readText()

        if (ca == null || cert == null || key == null || configJson == null) {
            log("Not configured. Push your certs to /sdcard/Scriptorum/:")
            log("  adb push ca.pem /sdcard/Scriptorum/ca.pem")
            log("  adb push client.pem /sdcard/Scriptorum/client.pem")
            log("  adb push client-key.pem /sdcard/Scriptorum/client-key.pem")
            log("""  echo '{"server_url":"https://..."}' | adb shell 'cat > /sdcard/Scriptorum/config.json'""")
            log("Or from the repo: just install-device-certs https://your.server")
            btnSync.isEnabled = true
            return
        }

        val serverUrl = JSONObject(configJson).getString("server_url")
        val syncService = SyncService(
            serverUrl = serverUrl,
            caCertPem = ca,
            clientCertPem = cert,
            clientKeyPem = key,
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
