package com.smartbutler.app
import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.*
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
class VoiceDownloadService:Service(){
 private var wake:PowerManager.WakeLock?=null
 override fun onBind(intent:Intent?):IBinder?=null
 override fun onCreate(){super.onCreate();getSystemService(NotificationManager::class.java).createNotificationChannel(NotificationChannel("voice_download","Voice setup",NotificationManager.IMPORTANCE_LOW))}
 override fun onStartCommand(intent:Intent?,flags:Int,startId:Int):Int{
  if(intent?.action=="PAUSE"){VoicePlatform.setDownloadChoice(0);VoicePlatform.nativePauseDownload();stopSelf();return START_NOT_STICKY}
  val pause=PendingIntent.getService(this,18,Intent(this,VoiceDownloadService::class.java).setAction("PAUSE"),PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
  val open=PendingIntent.getActivity(this,19,Intent(this,MainActivity::class.java),PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
  val notification=NotificationCompat.Builder(this,"voice_download").setSmallIcon(android.R.drawable.stat_sys_download).setContentTitle("Smart Butler · preparing intelligent voice").setContentText("Downloading and verifying speech models. Keep your selected network available.").setContentIntent(open).setOngoing(true).setProgress(0,0,true).addAction(android.R.drawable.ic_media_pause,"Pause",pause).build()
  ServiceCompat.startForeground(this,1202,notification,if(Build.VERSION.SDK_INT>=29)ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0)
  if(wake?.isHeld!=true){wake=getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"com.smartbutler.app:voiceSetup");wake?.acquire(2*60*60*1000L)}
  return START_NOT_STICKY
 }
 override fun onTimeout(startId:Int,fgsType:Int){VoicePlatform.nativePauseDownload();stopSelf()}
 override fun onDestroy(){if(wake?.isHeld==true)wake?.release();VoicePlatform.nativePauseDownload();super.onDestroy()}
}
