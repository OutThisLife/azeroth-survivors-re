package io.github.songoku119.azerothsurvivors

import android.app.Activity
import android.app.AlertDialog
import android.app.DownloadManager
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.io.File

/**
 * What the icon opens. `MainActivity` (GameActivity) shows nothing until the Rust side draws,
 * which needs the game library to load, the game data to be on the phone and seconds of engine
 * start; before this screen, a missing library or folder was a black window or an instant return
 * to the launcher. The checks here are the ones the Rust side makes anyway
 * (`env_shim::data_dir`, the library the manifest names), made first and put into words, with
 * what to do about each. When they all pass, it hands over to `MainActivity` at once.
 */
class LaunchActivity : Activity() {
    private lateinit var headline: TextView
    private lateinit var detail: TextView
    private lateinit var buttons: LinearLayout
    private val handler = Handler(Looper.getMainLooper())
    private var watching: Runnable? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(buildView())
        check()
    }

    override fun onDestroy() {
        stopWatching()
        super.onDestroy()
    }

    private fun buildView(): ScrollView {
        val dp = resources.displayMetrics.density
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            val pad = (28 * dp).toInt()
            setPadding(pad, pad, pad, pad)
        }
        column.addView(TextView(this).apply {
            text = getString(R.string.app_name)
            textSize = 30f
            setTextColor(Color.rgb(255, 210, 90))
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
        })
        headline = TextView(this).apply {
            textSize = 18f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            setPadding(0, (18 * dp).toInt(), 0, 0)
        }
        column.addView(headline)
        detail = TextView(this).apply {
            textSize = 14f
            setTextColor(Color.rgb(216, 201, 163))
            setTextIsSelectable(true)
            setPadding(0, (14 * dp).toInt(), 0, 0)
        }
        column.addView(detail)
        buttons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            setPadding(0, (18 * dp).toInt(), 0, 0)
        }
        column.addView(buttons)
        return ScrollView(this).apply {
            setBackgroundColor(Color.BLACK)
            addView(
                column,
                ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT),
            )
        }
    }

    private fun check() {
        stopWatching()
        buttons.removeAllViews()

        // 1. The game library. GameActivity loads it in its own onCreate, where a failure is a
        //    crash with no words; here it is a sentence. Loading it twice is a no-op.
        try {
            System.loadLibrary("warcraft_survivors_android")
        } catch (e: UnsatisfiedLinkError) {
            show(
                "The game library did not load.",
                "This APK does not fit this phone (it needs a 64-bit ARM processor and Android 10 " +
                    "or newer), or the download is damaged.\n\n$e\n\n" + Status.LOGS,
            )
            button("Try again") { check() }
            return
        }

        // 2. The game data, where the Rust side looks (`env_shim::data_dir`): the app's external
        //    files folder first (where `adb push` lands), then its internal one.
        val external = getExternalFilesDir(null)
        val candidates = listOfNotNull(external, filesDir).map { File(it, "Data") }
        val data = candidates.firstOrNull {
            !File(it, DataDownload.UNPACKING_MARK).exists() && hasArchive(it, "terrain.MPQ") && hasArchive(it, "patch.MPQ")
        }
        if (data == null) {
            // A download already under way, or finished and waiting to be unpacked, comes first.
            if (resumeDownload()) return
            val target = candidates.first()
            show(
                "No game data on this phone.",
                "Azeroth Survivors runs on the art, maps and sounds of a World of Warcraft 1.12.1 " +
                    "client and ships none of them. Put the client's Data folder (about 5.5 GB of " +
                    ".MPQ archives) at:\n\n${target.path}\n\n" +
                    "Download fetches that client from archive.org onto this phone (about 5.5 GB, " +
                    "12 GB free needed while it unpacks; Wi-Fi advised).\n\n" +
                    "Or, from a PC with the phone on USB debugging, in the game's source folder:\n" +
                    "    make android-data\n\n" +
                    "or, with a Data folder you already have:\n" +
                    "    adb push /path/to/WoW/Data ${target.parent}/\n\n" +
                    "Then tap Check again.",
            )
            button("Download") { confirmDownload(DataDownload.urlIndex(this)) }
            button("Check again") { check() }
            button("Start anyway") { launch() }
            return
        }

        // 3. The last run. A stage that never reached the pick screen means it died on the way,
        //    and the Rust panic hook leaves `crash` with the reason.
        val last = Status.read(Status.file(filesDir))
        if (last != null && !last.settled) {
            val what = if (last.key == "crash") last.text else "Last time, the game stopped while: ${last.text}"
            show("The last start did not finish.", "$what\n\n" + Status.LOGS)
            button("Start") { launch() }
            return
        }

        show("Starting…", "Game data: ${data.path}")
        launch()
    }

    // --- The in-app download (DataDownload) ---------------------------------------------------

    /** The words before any bytes move: whose data it is, how much, where from. */
    private fun confirmDownload(urlIndex: Int) {
        val free = DataDownload.freeBytes(this)
        if (DataDownload.root(this) == null) {
            show("No storage for the download.", "This phone offers the app no external files folder.")
            button("Back") { check() }
            return
        }
        if (free < DataDownload.NEEDED_BYTES) {
            show(
                "Not enough space for the download.",
                "About ${DataDownload.mb(DataDownload.NEEDED_BYTES) / 1024} GB free is needed while the " +
                    "client unpacks (the zip and the archives are both on the phone for a while); " +
                    "${DataDownload.mb(free) / 1024} GB is free. Make room, then tap Download again.",
            )
            button("Back") { check() }
            return
        }
        AlertDialog.Builder(this)
            .setTitle("Download the game data?")
            .setMessage(
                "This fetches a World of Warcraft 1.12.1 client (about 5.5 GB) from archive.org, " +
                    "${DataDownload.URL_NAMES[urlIndex]}, and unpacks its Data folder here. The client " +
                    "is Blizzard Entertainment's; Azeroth Survivors ships none of it and only reads it. " +
                    "Wi-Fi is advised. The download continues with the app closed; come back here to " +
                    "unpack it.",
            )
            .setPositiveButton("Download") { _, _ ->
                // Two dialogs from a double tap confirm one download, not two.
                if (DataDownload.currentId(this) < 0) DataDownload.start(this, urlIndex)
                watchDownload()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /**
     * A download the manager still knows about, or a finished zip not yet unpacked: show it
     * instead of the no-data words. True when this screen is now about that.
     */
    private fun resumeDownload(): Boolean {
        if (DataDownload.unpacking) {
            // A thread from before this screen is still writing; its words go nowhere now.
            buttons.removeAllViews()
            show("Unpacking the game data…", "Still working; tap Check again in a minute.")
            button("Check again") { check() }
            return true
        }
        val id = DataDownload.currentId(this)
        if (id < 0) return false
        if (DataDownload.progress(this, id) == null) {
            // The manager forgot it (cleared by the system, or the notification swiped away).
            DataDownload.forget(this)
            return false
        }
        watchDownload()
        return true
    }

    /** Poll the manager once a second and put its state into words; hand over when it is done. */
    private fun watchDownload() {
        stopWatching()
        buttons.removeAllViews()
        val id = DataDownload.currentId(this)
        if (id < 0) return
        val urlIndex = DataDownload.urlIndex(this)
        button("Cancel download") {
            DataDownload.cancel(this, id)
            check()
        }
        val tick = object : Runnable {
            override fun run() {
                val p = DataDownload.progress(this@LaunchActivity, id)
                if (p == null) {
                    DataDownload.forget(this@LaunchActivity)
                    check()
                    return
                }
                val size = if (p.total > 0) "${DataDownload.mb(p.soFar)} of ${DataDownload.mb(p.total)} MB"
                else "${DataDownload.mb(p.soFar)} MB so far"
                when (p.status) {
                    DownloadManager.STATUS_SUCCESSFUL -> {
                        stopWatching()
                        unpack(id, DataDownload.downloadedFile(this@LaunchActivity, p))
                        return
                    }
                    DownloadManager.STATUS_FAILED -> {
                        stopWatching()
                        downloadFailed(id, urlIndex, DataDownload.reasonText(p.reason))
                        return
                    }
                    DownloadManager.STATUS_PAUSED ->
                        show(
                            "Download paused: ${DataDownload.reasonText(p.reason)}.",
                            "$size (${DataDownload.URL_NAMES[urlIndex]})\n\nOn mobile data, Android holds a download this " +
                                "large for Wi-Fi; its notification offers to let it through.",
                        )
                    DownloadManager.STATUS_PENDING ->
                        show("Download starting…", DataDownload.URLS[urlIndex])
                    else ->
                        show("Downloading the game data…", "$size (${DataDownload.URL_NAMES[urlIndex]})\n\nThe download goes on with the app closed; the notification shows it.")
                }
                handler.postDelayed(this, 1000L)
            }
        }
        watching = tick
        handler.post(tick)
    }

    private fun downloadFailed(id: Long, urlIndex: Int, reason: String) {
        DataDownload.cancel(this, id)
        buttons.removeAllViews()
        val other = (urlIndex + 1) % DataDownload.URLS.size
        show(
            "The download failed: $reason.",
            "From ${DataDownload.URL_NAMES[urlIndex]}:\n${DataDownload.URLS[urlIndex]}\n\n" +
                "Try again, or try ${DataDownload.URL_NAMES[other]} (it cannot resume a broken transfer).",
        )
        button("Try again") { confirmDownload(urlIndex) }
        button("Try ${DataDownload.URL_NAMES[other]}") { confirmDownload(other) }
        button("Back") { check() }
    }

    /**
     * The zip is down: unpack it on a worker thread, with the words updated from there. One at a
     * time in the process, and nothing is posted to a screen that is gone.
     */
    private fun unpack(id: Long, zip: File?) {
        if (DataDownload.unpacking) {
            resumeDownload()
            return
        }
        val root = DataDownload.root(this)
        if (zip == null || root == null || !zip.isFile) {
            DataDownload.cancel(this, id)
            buttons.removeAllViews()
            show("The downloaded file is missing.", "Tap Download to fetch it again.")
            button("Download") { confirmDownload(DataDownload.urlIndex(this)) }
            button("Back") { check() }
            return
        }
        DataDownload.unpacking = true
        buttons.removeAllViews()
        show("Unpacking the game data…", "Reading ${zip.name} (${DataDownload.mb(zip.length())} MB). Keep the app open; this takes a few minutes.")
        val target = File(root, "Data")
        Thread {
            try {
                val count = DataDownload.extract(zip, target) { line ->
                    handler.post {
                        if (isDestroyed) return@post
                        detail.text = line
                    }
                }
                DataDownload.unpacking = false
                // Through the manager, which also drops the entry from the Downloads app.
                DataDownload.cancel(this, id)
                handler.post {
                    if (isDestroyed) return@post
                    show("Unpacked $count archives.", "Game data: ${target.path}")
                    check()
                }
            } catch (e: Exception) {
                DataDownload.unpacking = false
                // The stack for `adb logcat -s wsurvivors:*`; the screen gets the message alone.
                Log.e("wsurvivors", "unpacking ${zip.path} failed", e)
                val why = e.message?.takeIf { it.isNotBlank() } ?: e.toString()
                handler.post {
                    if (isDestroyed) return@post
                    buttons.removeAllViews()
                    show(
                        "Unpacking failed.",
                        "$why\n\nThe zip is kept at ${zip.path}. If this keeps failing with the whole-item " +
                            "zip, download the direct file instead.",
                    )
                    button("Unpack again") { unpack(id, zip) }
                    button("Download again") {
                        DataDownload.cancel(this, id)
                        confirmDownload(DataDownload.urlIndex(this))
                    }
                    button("Back") { check() }
                }
            }
        }.start()
    }

    private fun stopWatching() {
        watching?.let { handler.removeCallbacks(it) }
        watching = null
    }

    /** The two archives nothing boots without, in whichever case the client was unpacked in. */
    private fun hasArchive(dir: File, name: String): Boolean =
        dir.listFiles()?.any { it.isFile && it.name.equals(name, ignoreCase = true) } ?: false

    private fun show(head: String, body: String) {
        headline.text = head
        detail.text = body
    }

    private fun button(label: String, onClick: () -> Unit) {
        buttons.addView(Button(this).apply {
            text = label
            setOnClickListener { onClick() }
        })
    }

    private fun launch() {
        // Before the native side exists: a stale `ready` from the last run would otherwise hide
        // MainActivity's overlay on its first poll. Its own words, so the next launch can tell
        // a death here from one after the engine started (`starting`, written by the Rust side).
        Status.write(Status.file(filesDir), "launching", "Opening the game window (the engine had not started yet)…")
        startActivity(Intent(this, MainActivity::class.java))
        finish()
    }
}
