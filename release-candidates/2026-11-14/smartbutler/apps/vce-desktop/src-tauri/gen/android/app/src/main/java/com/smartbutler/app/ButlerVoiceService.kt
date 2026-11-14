package com.smartbutler.app

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

class ButlerVoiceService : Service() {
    companion object { private const val CHANNEL = "butler_voice"; private const val NOTICE = 1201; private const val STOP = "com.smartbutler.app.STOP_VOICE" }
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onCreate() {
        super.onCreate()
        getSystemService(NotificationManager::class.java).createNotificationChannel(NotificationChannel(CHANNEL, "Voice listening", NotificationManager.IMPORTANCE_LOW))
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == STOP) { VoicePlatform.stop(); stopSelf(); return START_NOT_STICKY }
        val stop = PendingIntent.getService(this, 1, Intent(this, ButlerVoiceService::class.java).setAction(STOP), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val open = PendingIntent.getActivity(this, 2, Intent(this, MainActivity::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notice = NotificationCompat.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle("Smart Butler voice is on").setContentText("Listening for your wake phrase. Tap Stop to turn off the microphone.")
            .setContentIntent(open).setOngoing(true).setCategory(NotificationCompat.CATEGORY_SERVICE)
            .addAction(android.R.drawable.ic_media_pause, "Stop", stop).build()
        try {
            ServiceCompat.startForeground(this, NOTICE, notice, if (Build.VERSION.SDK_INT >= 30) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE else 0)
            VoicePlatform.state = 1
            VoicePlatform.nativeAllowed(true)
        } catch (_: RuntimeException) { VoicePlatform.state = -1; VoicePlatform.nativeAllowed(false); stopSelf() }
        return START_NOT_STICKY
    }
    override fun onTaskRemoved(rootIntent: Intent?) { VoicePlatform.stop(); stopSelf(); super.onTaskRemoved(rootIntent) }
    override fun onDestroy() { VoicePlatform.state = -1; VoicePlatform.nativeAllowed(false); super.onDestroy() }
}
