package io.github.songoku119.azerothsurvivors

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(buildView())
        check()
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
        val data = candidates.firstOrNull { hasArchive(it, "terrain.MPQ") && hasArchive(it, "patch.MPQ") }
        if (data == null) {
            val target = candidates.first()
            show(
                "No game data on this phone.",
                "Azeroth Survivors runs on the art, maps and sounds of a World of Warcraft 1.12.1 " +
                    "client and ships none of them. Put the client's Data folder (about 5.5 GB of " +
                    ".MPQ archives) at:\n\n${target.path}\n\n" +
                    "From a PC with the phone on USB debugging, in the game's source folder:\n" +
                    "    make android-data\n\n" +
                    "or, with a Data folder you already have:\n" +
                    "    adb push /path/to/WoW/Data ${target.parent}/\n\n" +
                    "Then tap Check again.",
            )
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
