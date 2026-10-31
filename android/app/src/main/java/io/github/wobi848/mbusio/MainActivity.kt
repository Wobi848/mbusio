package io.github.wobi848.mbusio

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.ContentValues
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.MediaStore
import android.webkit.URLUtil
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/** Die Weboberfläche von mbusio im ganzen Fenster; mbusio selbst läuft im MbusioService */
class MainActivity : Activity() {
    private lateinit var web: WebView
    private var pickFile: ValueCallback<Array<Uri>>? = null
    private val home = "http://127.0.0.1:${MbusioService.HTTP_PORT}/"

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 0)
        startForegroundService(Intent(this, MbusioService::class.java))

        web = WebView(this)
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.webViewClient = object : WebViewClient() {
            // fremde Links (Hilfe, GitHub) im Browser, mbusio selbst hier
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                if (request.url.toString().startsWith(home)) return false
                startActivity(Intent(Intent.ACTION_VIEW, request.url))
                return true
            }
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(
                view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams
            ): Boolean {
                pickFile?.onReceiveValue(null)
                pickFile = callback
                startActivityForResult(params.createIntent(), PICK_FILE)
                return true
            }
        }
        web.setDownloadListener { url, _, disposition, mime, _ ->
            download(url, URLUtil.guessFileName(url, disposition, mime), mime)
        }
        web.loadData(getString(R.string.starting), "text/plain; charset=utf-8", null)
        setContentView(web)
        waitForMbusio()
    }

    private fun waitForMbusio() = thread {
        for (i in 0 until 60) {
            if (MbusioService.reachable()) {
                runOnUiThread { web.loadUrl(home) }
                return@thread
            }
            Thread.sleep(500)
        }
        runOnUiThread { web.loadData(getString(R.string.failed), "text/plain; charset=utf-8", null) }
    }

    /** Exporte von mbusio nach „Downloads“ */
    private fun download(url: String, name: String, mime: String?) = thread {
        val message = try {
            val c = URL(url).openConnection() as HttpURLConnection
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, name)
                put(MediaStore.Downloads.MIME_TYPE, mime ?: "application/octet-stream")
            }
            val target = contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                ?: throw IOException(name)
            c.inputStream.use { input ->
                contentResolver.openOutputStream(target)!!.use { input.copyTo(it) }
            }
            getString(R.string.saved, name)
        } catch (e: IOException) {
            getString(R.string.save_failed, e.message)
        }
        runOnUiThread { Toast.makeText(this, message, Toast.LENGTH_LONG).show() }
    }

    @Deprecated("Activity ohne AndroidX")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != PICK_FILE) return super.onActivityResult(requestCode, resultCode, data)
        pickFile?.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(resultCode, data))
        pickFile = null
    }

    @Deprecated("Activity ohne AndroidX")
    override fun onBackPressed() {
        if (web.canGoBack()) web.goBack() else super.onBackPressed()
    }

    companion object {
        private const val PICK_FILE = 1
    }
}
