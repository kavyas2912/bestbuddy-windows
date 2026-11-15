package com.smartbutler.app

import android.Manifest
import android.content.Intent
import android.content.Context
import android.os.Build
import android.content.pm.PackageManager
import androidx.annotation.Keep
import androidx.core.content.ContextCompat
import java.lang.ref.WeakReference

@Keep
object VoicePlatform {
    private var activity = WeakReference<MainActivity>(null)
    private var appContext: Context? = null
    @Volatile var foreground = false
    @Volatile var state = 0
    @Volatile var requested = false
    private var permissionOnly = false
    @JvmStatic external fun nativePauseDownload()
    @JvmStatic fun setDownloadChoice(choice: Int): Int { appContext?.getSharedPreferences("voice_setup", Context.MODE_PRIVATE)?.edit()?.putInt("download",choice)?.apply(); return choice }
    @JvmStatic fun downloadChoice(): Int = appContext?.getSharedPreferences("voice_setup", Context.MODE_PRIVATE)?.getInt("download",0) ?: 0
    @JvmStatic fun transferNetwork(): Int {
        val context=appContext ?: return 0
        val manager=context.getSystemService(android.net.ConnectivityManager::class.java)
        val caps=manager.getNetworkCapabilities(manager.activeNetwork) ?: return 0
        if (!caps.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET) || !caps.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED)) return 0
        return if (downloadChoice()==2 || (downloadChoice()==1 && caps.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) && !manager.isActiveNetworkMetered)) 1 else 0
    }
    @JvmStatic fun startTransfer(): Int { val context=appContext ?: return -1; if(!foreground) return -1; activity.get()?.runOnUiThread { activity.get()?.requestDownloadNotice() }; ContextCompat.startForegroundService(context,Intent(context,VoiceDownloadService::class.java)); return 1 }
    @JvmStatic fun stopTransfer(): Int { appContext?.stopService(Intent(appContext,VoiceDownloadService::class.java)); return 0 }
    @JvmStatic fun requestPermissionOnly(): Int { permissionOnly=true; return requestStart() }
    @JvmStatic external fun nativeAllowed(allowed: Boolean)
    fun attach(value: MainActivity) { activity = WeakReference(value); appContext = value.applicationContext }
    @JvmStatic fun status(): Int {
        val app = appContext ?: return -1
        if (ContextCompat.checkSelfPermission(app, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED ||
            (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(app, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED)) {
            if (state == 1) stop()
            return if (state == 0) 0 else -1
        }
        return state
    }
    @JvmStatic fun requestStart(): Int {
        val screen = activity.get() ?: return -1
        if (!foreground) { state = -1; return -1 }
        if (requested) return 0
        requested = true
        state = 0
        screen.runOnUiThread { screen.requestVoicePermission() }
        return 0
    }
    fun permissionResult(granted: Boolean) {
        if (!requested) return
        requested = false
        val screen = activity.get()
        if (!granted || screen == null || !foreground) { permissionOnly=false; state = -1; nativeAllowed(false); return }
        if (permissionOnly) {permissionOnly=false;state=0;nativeAllowed(false);return}
        try { ContextCompat.startForegroundService(screen, Intent(screen, ButlerVoiceService::class.java)) }
        catch (_: RuntimeException) { state = -1; nativeAllowed(false) }
    }
    @JvmStatic fun stop(): Int {
        requested = false
        permissionOnly = false
        state = -1
        nativeAllowed(false)
        appContext?.let { it.stopService(Intent(it, ButlerVoiceService::class.java)) }
        return -1
    }
}
