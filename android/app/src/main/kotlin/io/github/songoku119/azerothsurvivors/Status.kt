package io.github.songoku119.azerothsurvivors

import java.io.File

/**
 * One stage of the native boot, as the Rust side writes it to `<filesDir>/benilla-config/status.txt`:
 * `<key>` on the first line, a sentence for the player on the rest. The launcher writes `starting`
 * and `crash` (crates/warcraft-survivors-android/src/android.rs), the game writes `opening`,
 * `building`, `nodata` and `ready` (crates/benilla-app/src/survivors/status.rs); this side writes
 * `launching` before the native side starts, so a stale `ready` from the previous run cannot hide
 * the overlay early.
 */
data class Status(val key: String, val text: String) {
    /** The engine drew something the player can act on (the pick screen or the no-data panel). */
    val settled: Boolean
        get() = key == "ready" || key == "nodata"

    companion object {
        fun file(filesDir: File): File = File(filesDir, "benilla-config/status.txt")

        /** `null` when the file is missing, unreadable or empty: never a reason to stop. */
        fun read(file: File): Status? {
            val lines = try {
                if (!file.isFile) return null
                file.readLines()
            } catch (e: Exception) {
                return null
            }
            val key = lines.firstOrNull()?.trim() ?: return null
            if (key.isEmpty()) return null
            return Status(key, lines.drop(1).joinToString("\n").trim())
        }

        /** The same shape the Rust side writes: through a temp file and a rename. */
        fun write(file: File, key: String, text: String) {
            try {
                file.parentFile?.mkdirs()
                val tmp = File(file.parentFile, "status.tmp")
                tmp.writeText("$key\n$text\n")
                tmp.renameTo(file)
            } catch (e: Exception) {
                // A progress line; the overlay falls back to its own first words.
            }
        }

        /** Where to look when it goes wrong; the same line everywhere a player sees it. */
        const val LOGS: String =
            "Logs, with the phone on USB debugging:\nadb logcat -s wsurvivors:* wgpu:* RustStdoutStderr:*"
    }
}
