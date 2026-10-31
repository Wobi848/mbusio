package io.github.wobi848.mbusio

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.util.Log
import com.hoho.android.usbserial.driver.UsbSerialDriver
import com.hoho.android.usbserial.driver.UsbSerialPort
import com.hoho.android.usbserial.driver.UsbSerialProber
import com.hoho.android.usbserial.util.SerialInputOutputManager
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * Reicht USB-Seriell-Wandler an mbusio durch (Gegenstück zu src/link/bridge.rs, dort steht das
 * Protokoll). Lauscht nur auf 127.0.0.1; weil dort auch andere Apps hinkommen, muss mbusio zuerst
 * den Schlüssel schicken.
 */
class UsbBridge(private val context: Context) {
    val token: String = ByteArray(16).also { SecureRandom().nextBytes(it) }
        .joinToString("") { "%02x".format(it) }
    private val server = ServerSocket(0, 8, InetAddress.getLoopbackAddress())
    val address: String get() = "127.0.0.1:${server.localPort}"
    private val usb = context.getSystemService(Context.USB_SERVICE) as UsbManager

    fun start() {
        thread(name = "usb-bridge") {
            while (!server.isClosed) {
                val socket = try {
                    server.accept()
                } catch (e: IOException) {
                    break // geschlossen
                }
                thread(name = "usb-session") { Session(socket).run() }
            }
        }
    }

    fun stop() = server.close()

    /** Wandler mit festem Namen aus Hersteller- und Produktnummer; gleiche bekommen -2, -3 … */
    private fun drivers(): List<Pair<String, UsbSerialDriver>> {
        val seen = mutableMapOf<String, Int>()
        return UsbSerialProber.getDefaultProber().findAllDrivers(usb).map { d ->
            val base = "%04x:%04x".format(d.device.vendorId, d.device.productId)
            val n = seen.merge(base, 1, Int::plus)!!
            (if (n == 1) base else "$base-$n") to d
        }
    }

    private fun label(d: UsbSerialDriver): String =
        d.device.productName ?: d.javaClass.simpleName.removeSuffix("SerialDriver")

    /** Fragt bei Bedarf nach der USB-Erlaubnis und wartet auf die Antwort */
    private fun permit(device: UsbDevice) {
        if (usb.hasPermission(device)) return
        val action = "${context.packageName}.USB_PERMISSION"
        val answered = CountDownLatch(1)
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) = answered.countDown()
        }
        context.registerReceiver(receiver, IntentFilter(action), Context.RECEIVER_NOT_EXPORTED)
        try {
            val intent = Intent(action).setPackage(context.packageName)
            // veränderbar: Android trägt Gerät und Antwort in den Intent ein
            usb.requestPermission(
                device,
                PendingIntent.getBroadcast(context, 0, intent, PendingIntent.FLAG_MUTABLE)
            )
            answered.await(50, TimeUnit.SECONDS)
        } finally {
            context.unregisterReceiver(receiver)
        }
        if (!usb.hasPermission(device)) throw IOException(context.getString(R.string.usb_denied))
    }

    private inner class Session(private val socket: Socket) : SerialInputOutputManager.Listener {
        private val input = DataInputStream(BufferedInputStream(socket.getInputStream()))
        private val output = DataOutputStream(socket.getOutputStream())
        private var port: UsbSerialPort? = null
        private var reader: SerialInputOutputManager? = null

        fun run() {
            try {
                val (kind, token) = read()
                if (kind != 'H' || String(token) != this@UsbBridge.token) return
                while (true) {
                    val (k, payload) = read()
                    try {
                        send('R', handle(k, payload))
                    } catch (e: Exception) {
                        Log.w(TAG, "USB $k", e)
                        send('E', (e.message ?: e.toString()).toByteArray())
                    }
                }
            } catch (e: IOException) {
                Log.d(TAG, "Verbindung zu mbusio beendet: ${e.message}")
            } finally {
                close()
            }
        }

        private fun handle(kind: Char, payload: ByteArray): ByteArray {
            when (kind) {
                'L' -> return list()
                'O' -> {
                    val (name, baud) = String(payload).split("\n")
                    open(name, baud.toInt())
                }
                'W' -> opened().write(payload, 2000)
                'B' -> opened().setParameters(String(payload).toInt(), 8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_EVEN)
                'C' -> try {
                    opened().purgeHwBuffers(false, true)
                } catch (e: UnsupportedOperationException) {
                    // manche Chips (CH340) können das nicht; mbusio verwirft danach selbst
                }
                else -> throw IOException("unbekannter Auftrag $kind")
            }
            return ByteArray(0)
        }

        private fun list(): ByteArray {
            val all = JSONArray()
            for ((name, d) in drivers()) all.put(JSONObject().put("name", name).put("label", label(d)))
            return all.toString().toByteArray()
        }

        private fun opened(): UsbSerialPort = port ?: throw IOException("nicht geöffnet")

        private fun open(name: String, baud: Int) {
            val d = drivers().firstOrNull { it.first == name }?.second
                ?: throw IOException(context.getString(R.string.usb_missing, name))
            permit(d.device)
            val connection = usb.openDevice(d.device) ?: throw IOException(context.getString(R.string.usb_denied))
            val p = d.ports[0]
            p.open(connection)
            p.setParameters(baud, 8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_EVEN)
            port = p
            reader = SerialInputOutputManager(p, this).also { it.start() }
        }

        override fun onNewData(data: ByteArray) {
            try {
                send('D', data)
            } catch (e: IOException) {
                close() // mbusio weg
            }
        }

        override fun onRunError(e: Exception) {
            try {
                send('E', (e.message ?: e.toString()).toByteArray())
            } catch (ignored: IOException) {
                // mbusio ist auch schon weg; close() unten räumt auf
            }
            close()
        }

        private fun read(): Pair<Char, ByteArray> {
            val kind = input.readUnsignedByte().toChar()
            val payload = ByteArray(input.readInt())
            input.readFully(payload)
            return kind to payload
        }

        @Synchronized
        private fun send(kind: Char, payload: ByteArray) {
            output.writeByte(kind.code)
            output.writeInt(payload.size)
            output.write(payload)
            output.flush()
        }

        @Synchronized
        private fun close() {
            reader?.stop()
            reader = null
            try {
                port?.close()
            } catch (e: IOException) {
                Log.d(TAG, "Wandler schon zu: ${e.message}")
            }
            port = null
            socket.close()
        }
    }

    companion object {
        private const val TAG = "mbusio-usb"
    }
}
