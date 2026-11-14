package com.smartbutler.app

import android.Manifest
import android.content.pm.PackageManager
import android.view.View
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import android.os.Bundle
import android.os.Build
import androidx.lifecycle.lifecycleScope
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
    private val downloadNotification = registerForActivityResult(ActivityResultContracts.RequestPermission()) { _ -> }
    fun requestDownloadNotice() { if(Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(this,Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED) downloadNotification.launch(Manifest.permission.POST_NOTIFICATIONS) }
    private val microphonePermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted -> lifecycleScope.launchWhenResumed { VoicePlatform.permissionResult(granted) } }
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        lifecycleScope.launchWhenResumed { if (granted) requestMicrophone() else VoicePlatform.permissionResult(false) }
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        VoicePlatform.attach(this)
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        // Protect the entire WebView, including fixed HTML dialogs, from system UI.
        val content = findViewById<View>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            WindowInsetsCompat.CONSUMED
        }
        ViewCompat.requestApplyInsets(content)
    }
    override fun onResume() { super.onResume(); VoicePlatform.foreground = true }
    override fun onPause() { VoicePlatform.foreground = false; super.onPause() }
    fun requestVoicePermission() {
        if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED)
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        else requestMicrophone()
    }
    private fun requestMicrophone() {
        if (!VoicePlatform.requested) return
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) VoicePlatform.permissionResult(true)
        else microphonePermission.launch(Manifest.permission.RECORD_AUDIO)
    }
}
