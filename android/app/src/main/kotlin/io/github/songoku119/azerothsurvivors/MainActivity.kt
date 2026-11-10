package io.github.songoku119.azerothsurvivors

import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import com.google.androidgamesdk.GameActivity

/**
 * The game: GameActivity loads `libwarcraft_survivors_android.so` (the manifest's
 * `android.app.lib_name`) and hands it the window; everything after that is Rust. `LaunchActivity`
 * is what the icon opens; it comes here once the library and the data are known to be there.
 *
 * Until the Rust side draws, an overlay shows the stage it reports through the status file
 * (`Status`): engine start takes seconds on a phone, and the surface is black until the pick
 * screen is built. The overlay goes once the status settles (`ready`, or `nodata` for the
 * in-game panel), and never takes a touch: the views are not clickable, so events fall through
 * to the game's surface.
 */
class MainActivity : GameActivity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var overlay: FrameLayout
    private lateinit var label: TextView
    private var booting = true
    private var settled: Status? = null
    private var shown = ""
    private var shownAt = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        // A Java-side death (GameActivity's own onCreate: the glue's JNI against a
        // games-activity it was not built for, say) has no Rust hook to write `crash`; this one
        // does, then hands the exception on, so the next launch screen can say what happened.
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, e ->
            Status.write(Status.file(filesDir), "crash", "The game window crashed: $e")
            previous?.uncaughtException(thread, e)
        }
        super.onCreate(savedInstanceState)
        // A run is minutes of touch-free watching; no screen timeout mid-wave.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        // Back reaches the native side as a key event first (GameActivity forwards every key).
        // When the game does not consume it, the default would finish the activity and drop the
        // run; this callback takes the event instead and does nothing, so Back is the game's.
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {}
        })

        val dp = resources.displayMetrics.density
        label = TextView(this).apply {
            text = FIRST_WORDS
            textSize = 18f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            val pad = (24 * dp).toInt()
            setPadding(pad, pad, pad, pad)
            setBackgroundColor(Color.argb(200, 0, 0, 0))
        }
        overlay = FrameLayout(this)
        overlay.addView(
            label,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER,
            ),
        )
        // On top of the surface GameActivity set as content.
        addContentView(
            overlay,
            ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        shownAt = SystemClock.elapsedRealtime()
        handler.post(poll)
    }

    private val poll = object : Runnable {
        override fun run() {
            if (!booting) return
            val status = Status.read(Status.file(filesDir))
            if (status != null && status.settled) {
                booting = false
                settled = status
                overlay.visibility = View.GONE
                return
            }
            val now = SystemClock.elapsedRealtime()
            val line = status?.text?.takeIf { it.isNotEmpty() } ?: FIRST_WORDS
            if (line != shown) {
                shown = line
                shownAt = now
            }
            label.text = if (now - shownAt > STALL_MS) {
                "$line\n\nStill working. If this never ends:\n" + Status.LOGS
            } else {
                line
            }
            handler.postDelayed(this, POLL_MS)
        }
    }

    /**
     * The icon tapped while the game runs: `singleTask` brings this activity back instead of a
     * new one, after LaunchActivity wrote `launching` for a boot that is not going to happen.
     * Put the settled stage back, or the next cold start would report a death that was not.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        settled?.let { Status.write(Status.file(filesDir), it.key, it.text) }
    }

    override fun onDestroy() {
        handler.removeCallbacks(poll)
        super.onDestroy()
    }

    private companion object {
        const val FIRST_WORDS = "Starting the engine…"
        const val POLL_MS = 250L
        /** A stage that sits this long is reported as such: the first shader compile can be slow. */
        const val STALL_MS = 30_000L
    }
}
