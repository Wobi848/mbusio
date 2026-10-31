package io.github.wobi848.mbusio

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/**
 * Hält mbusio am Laufen, auch mit ausgeschaltetem Bildschirm: Vordergrunddienst mit Meldung,
 * startet das Programm (libmbusio.so) und nach einem Ende neu (Absturz oder gewollter Neustart,
 * z.B. nach dem Einspielen einer Sicherung).
 */
class MbusioService : Service() {
    private lateinit var bridge: UsbBridge
    private lateinit var wakeLock: PowerManager.WakeLock
    @Volatile private var process: Process? = null
    @Volatile private var stopping = false

    override fun onCreate() {
        super.onCreate()
        startForeground(1, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        // Auslesepläne laufen auch bei ausgeschaltetem Bildschirm
        wakeLock = (getSystemService(POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mbusio:run")
            .apply { acquire() }
        bridge = UsbBridge(this).apply { start() }
        thread(name = "mbusio") { supervise() }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) stopSelf()
        return START_STICKY
    }

    override fun onDestroy() {
        stopping = true
        process?.destroy()
        bridge.stop()
        wakeLock.release()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun supervise() {
        while (!stopping) {
            val started = System.currentTimeMillis()
            val p = start()
            process = p
            val code = p.waitFor()
            process = null
            Log.i(TAG, "mbusio beendet mit $code")
            // sofort wieder abgestürzt: nicht im Kreis drehen
            if (System.currentTimeMillis() - started < 5000) Thread.sleep(5000)
        }
    }

    private fun start(): Process {
        val log = File(filesDir, "mbusio.log")
        if (log.length() > LOG_MAX) log.renameTo(File(filesDir, "mbusio.log.1"))
        return ProcessBuilder(
            File(applicationInfo.nativeLibraryDir, "libmbusio.so").path,
            "--no-browser",
            "--http-port", HTTP_PORT.toString(),
            "--data-dir", File(filesDir, "data").path,
        ).apply {
            environment()["MBUSIO_USB_BRIDGE"] = bridge.address
            environment()["MBUSIO_USB_BRIDGE_TOKEN"] = bridge.token
            // Androids /data/local/tmp darf eine App nicht beschreiben
            environment()["TMPDIR"] = cacheDir.path
            redirectErrorStream(true)
            redirectOutput(ProcessBuilder.Redirect.appendTo(log))
        }.start()
    }

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.channel), NotificationManager.IMPORTANCE_LOW)
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE
        )
        val stop = PendingIntent.getService(
            this, 1, Intent(this, MbusioService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher)
            .setContentTitle(getString(R.string.running))
            .setContentText(getString(R.string.running_text))
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null, getString(R.string.stop), stop).build())
            .setOngoing(true)
            .build()
    }

    companion object {
        const val HTTP_PORT = 8080
        const val ACTION_STOP = "io.github.wobi848.mbusio.STOP"
        private const val CHANNEL = "run"
        private const val TAG = "mbusio"
        private const val LOG_MAX = 1024 * 1024L

        /** Antwortet mbusio schon? */
        fun reachable(): Boolean = try {
            val c = URL("http://127.0.0.1:$HTTP_PORT/").openConnection() as HttpURLConnection
            c.connectTimeout = 500
            c.readTimeout = 1000
            c.responseCode > 0
        } catch (e: java.io.IOException) {
            false
        }
    }
}
