package io.github.songoku119.azerothsurvivors

import android.app.DownloadManager
import android.content.Context
import android.net.Uri
import android.os.StatFs
import java.io.BufferedInputStream
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipFile
import java.util.zip.ZipInputStream

/**
 * The in-app download of the game data, at the owner's choice (docs/ANDROID.md, "Why
 * data-less"): the same two archive.org links `scripts/fetch-data.sh` uses, in the same order,
 * through Android's DownloadManager (its notification shows the progress, it carries on with the
 * app in the background and across a reboot, and it needs no service of our own), then every
 * `.MPQ` archive under `Data` in the zip unpacked into the app's `Data` folder. (Kotlin block
 * comments nest, so the glob is spelled out here and in [extract]: a `/` `*` pair would open
 * one.) The data is Blizzard's; the
 * screen says so before it starts, and the app still ships none of it.
 */
object DataDownload {
    /** The direct file: a real zip with a size, so the manager can resume it. */
    const val PRIMARY_URL = "https://archive.org/download/WoW_1.12.1-5875/1.12.1-5875_enUS.zip"

    /**
     * archive.org's whole-item zip, built on the fly: no size up front, no resume, and it may hold
     * the client zip inside it rather than the Data folder; [extract] handles both shapes.
     */
    const val FALLBACK_URL =
        "https://archive.org/compress/WoW_1.12.1-5875/formats=ZIP&file=/WoW_1.12.1-5875.zip"

    val URLS = listOf(PRIMARY_URL, FALLBACK_URL)
    val URL_NAMES = listOf("the direct file", "the whole-item zip")

    const val ZIP_NAME = "wow-1.12.1.zip"

    /** The zip and the unpacked archives both on the phone at the peak (~5.5 GB each), plus room. */
    const val NEEDED_BYTES = 12L * 1024 * 1024 * 1024

    /**
     * An unpack is running somewhere in this process. Process-wide, not per screen: a relaunch
     * mid-unpack makes a new LaunchActivity, which must not start a second one on the same files.
     */
    @Volatile
    var unpacking: Boolean = false

    private const val PREFS = "data-download"
    private const val KEY_ID = "download-id"
    private const val KEY_URL = "url-index"

    /** Where the zip lands and where `Data` goes: the app's external files folder. */
    fun root(context: Context): File? = context.getExternalFilesDir(null)

    fun zipFile(context: Context): File? = root(context)?.let { File(it, ZIP_NAME) }

    fun freeBytes(context: Context): Long {
        val dir = root(context) ?: return 0L
        return try {
            StatFs(dir.path).availableBytes
        } catch (e: Exception) {
            0L
        }
    }

    /** Enqueue `URLS[urlIndex]`; the id is kept so a relaunch picks the download back up. */
    fun start(context: Context, urlIndex: Int): Long {
        val manager = context.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
        zipFile(context)?.delete()
        val request = DownloadManager.Request(Uri.parse(URLS[urlIndex]))
            .setTitle("Azeroth Survivors game data")
            .setDescription("World of Warcraft 1.12.1 client, about 5.5 GB")
            .setDestinationInExternalFilesDir(context, null, ZIP_NAME)
            .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE)
            .setAllowedOverMetered(true)
            .setAllowedOverRoaming(false)
        val id = manager.enqueue(request)
        prefs(context).edit().putLong(KEY_ID, id).putInt(KEY_URL, urlIndex).apply()
        return id
    }

    /** The remembered download, or -1. */
    fun currentId(context: Context): Long = prefs(context).getLong(KEY_ID, -1L)

    fun urlIndex(context: Context): Int = prefs(context).getInt(KEY_URL, 0).coerceIn(0, URLS.size - 1)

    fun forget(context: Context) {
        prefs(context).edit().remove(KEY_ID).apply()
    }

    /** The file a finished download landed in, from the manager; the expected path when it says nothing. */
    fun downloadedFile(context: Context, progress: Progress): File? =
        progress.localUri?.let { Uri.parse(it).path }?.let { File(it) } ?: zipFile(context)

    /** Stop and discard a download (the manager deletes its file), or drop a finished one from its list. */
    fun cancel(context: Context, id: Long) {
        val manager = context.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
        manager.remove(id)
        forget(context)
    }

    class Progress(val status: Int, val reason: Int, val soFar: Long, val total: Long, val localUri: String?)

    /** The manager's view of a download, or null when it no longer knows the id. */
    fun progress(context: Context, id: Long): Progress? {
        val manager = context.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
        val cursor = manager.query(DownloadManager.Query().setFilterById(id)) ?: return null
        cursor.use { c ->
            if (!c.moveToFirst()) return null
            return Progress(
                c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS)),
                c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON)),
                c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR)),
                c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES)),
                c.getString(c.getColumnIndexOrThrow(DownloadManager.COLUMN_LOCAL_URI)),
            )
        }
    }

    /** The manager's reason code for a paused or failed download, in words. */
    fun reasonText(reason: Int): String = when (reason) {
        DownloadManager.PAUSED_WAITING_FOR_NETWORK -> "waiting for a network"
        DownloadManager.PAUSED_WAITING_TO_RETRY -> "waiting to retry"
        DownloadManager.PAUSED_QUEUED_FOR_WIFI -> "queued for Wi-Fi"
        DownloadManager.ERROR_INSUFFICIENT_SPACE -> "not enough space on the phone"
        DownloadManager.ERROR_HTTP_DATA_ERROR -> "the transfer broke"
        DownloadManager.ERROR_CANNOT_RESUME -> "the transfer broke and this link cannot resume"
        DownloadManager.ERROR_FILE_ERROR -> "could not write the file"
        DownloadManager.ERROR_TOO_MANY_REDIRECTS -> "too many redirects"
        DownloadManager.ERROR_UNHANDLED_HTTP_CODE -> "the server refused"
        DownloadManager.ERROR_DEVICE_NOT_FOUND -> "no storage for the file"
        else -> if (reason in 400..599) "HTTP $reason" else "code $reason"
    }

    fun mb(bytes: Long): Long = bytes / (1024 * 1024)

    private val MPQ = Regex("(^|/)Data/[^/]+\\.mpq$", RegexOption.IGNORE_CASE)

    private fun isMpq(name: String): Boolean = MPQ.containsMatchIn(name)

    private fun baseName(name: String): String = name.substringAfterLast('/')

    /**
     * Unpack every `.MPQ` under a `Data` folder of [zip] (at any depth) into [dataDir], flat. A zip with none but
     * with a zip inside it, the whole-item download's shape, is read through that inner zip as a
     * stream, so the 5.5 GB client zip is never written out a second time. Minutes of work: call
     * it off the main thread. Returns how many archives were written; throws on a zip with
     * neither, or an I/O failure.
     */
    fun extract(zip: File, dataDir: File, onProgress: (String) -> Unit): Int {
        dataDir.mkdirs()
        // Present while archives are landing: terrain and patch come out before texture and wmo,
        // so without it a death mid-way leaves a folder the launch check would pass.
        val mark = File(dataDir, UNPACKING_MARK).apply { createNewFile() }
        var written = 0
        ZipFile(zip).use { outer ->
            val entries = outer.entries().toList()
            val mpqs = entries.filter { !it.isDirectory && isMpq(it.name) }
            if (mpqs.isNotEmpty()) {
                val total = mpqs.sumOf { it.size.coerceAtLeast(0L) }
                var done = 0L
                for (entry in mpqs) {
                    onProgress("Unpacking ${baseName(entry.name)} (${mb(done)} of ${mb(total)} MB done)…")
                    outer.getInputStream(entry).use { copyTo(it, File(dataDir, baseName(entry.name))) }
                    done += entry.size.coerceAtLeast(0L)
                    written++
                }
                mark.delete()
                return written
            }
            val inner = entries.firstOrNull { !it.isDirectory && it.name.endsWith(".zip", true) && it.name.contains("enUS", true) }
                ?: entries.firstOrNull { !it.isDirectory && it.name.endsWith(".zip", true) }
                ?: throw IllegalStateException("${zip.name} holds no Data/*.MPQ and no zip inside it")
            onProgress("Unpacking the client zip inside the download (${baseName(inner.name)})…")
            ZipInputStream(BufferedInputStream(outer.getInputStream(inner), 1 shl 20)).use { stream ->
                var entry: ZipEntry? = stream.nextEntry
                while (entry != null) {
                    if (!entry.isDirectory && isMpq(entry.name)) {
                        onProgress("Unpacking ${baseName(entry.name)} ($written done)…")
                        copyTo(stream, File(dataDir, baseName(entry.name)))
                        written++
                    }
                    stream.closeEntry()
                    entry = stream.nextEntry
                }
            }
        }
        if (written == 0) throw IllegalStateException("the zip inside ${zip.name} holds no Data/*.MPQ")
        mark.delete()
        return written
    }

    /** The file [extract] keeps in `Data` until every archive is in. */
    const val UNPACKING_MARK = ".unpacking"

    /** Through a `.part` file and a rename, so a half-written archive never passes for a whole one. */
    private fun copyTo(input: InputStream, target: File) {
        val part = File(target.path + ".part")
        FileOutputStream(part).use { out -> input.copyTo(out, 1 shl 20) }
        if (!part.renameTo(target)) {
            part.delete()
            throw IllegalStateException("could not move ${part.name} into place")
        }
    }

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
