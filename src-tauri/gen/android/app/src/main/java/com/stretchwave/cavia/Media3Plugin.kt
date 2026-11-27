package com.stretchwave.cavia

import android.app.Activity
import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.activity.result.ActivityResult
import androidx.documentfile.provider.DocumentFile
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import java.security.MessageDigest
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import java.util.concurrent.Executors

@InvokeArg
class LoadArgs {
  lateinit var path: String
  var startMs: Long = 0
  var autoPlay: Boolean = true
}

@InvokeArg class PositionArgs { var positionMs: Long = 0 }
@InvokeArg class VolumeArgs { var volume: Double = 1.0 }
@InvokeArg class SpeedArgs { var speed: Double = 1.0 }
@InvokeArg class MetadataArgs { lateinit var title: String; lateinit var artist: String; lateinit var album: String }
@InvokeArg class CredentialArgs {
  lateinit var service: String
  lateinit var account: String
  var secret: String? = null
}

@TauriPlugin
class Media3Plugin(private val activity: Activity) : Plugin(activity) {
  private val main = Handler(Looper.getMainLooper())
  private val io = Executors.newSingleThreadExecutor { task -> Thread(task, "cavia-import-stage") }

  @Command fun credentialGet(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CredentialArgs::class.java)
      validateCredentialIdentity(args.service, args.account)
      val preferences = activity.getSharedPreferences(CREDENTIAL_PREFS, Activity.MODE_PRIVATE)
      val stored = preferences.getString(credentialPreferenceKey(args.service, args.account), null)
      if (stored == null) {
        invoke.resolve(JSObject().put("found", false))
        return
      }
      val packed = Base64.decode(stored, Base64.DEFAULT)
      check(packed.size > GCM_IV_BYTES) { "Encrypted credential payload is malformed" }
      val iv = packed.copyOfRange(0, GCM_IV_BYTES)
      val ciphertext = packed.copyOfRange(GCM_IV_BYTES, packed.size)
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.DECRYPT_MODE, credentialKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
      val secret = String(cipher.doFinal(ciphertext), StandardCharsets.UTF_8)
      invoke.resolve(JSObject().put("found", true).put("secret", secret))
    } catch (_: Exception) {
      invoke.reject("Android secure credential read failed")
    }
  }

  @Command fun credentialSet(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CredentialArgs::class.java)
      validateCredentialIdentity(args.service, args.account)
      val secret = args.secret ?: throw IllegalArgumentException("Missing credential")
      require(secret.toByteArray(StandardCharsets.UTF_8).size <= MAX_CREDENTIAL_BYTES) {
        "Credential exceeds the secure storage size limit"
      }
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.ENCRYPT_MODE, credentialKey())
      val iv = cipher.iv
      val encrypted = cipher.doFinal(secret.toByteArray(StandardCharsets.UTF_8))
      val packed = ByteArray(iv.size + encrypted.size)
      System.arraycopy(iv, 0, packed, 0, iv.size)
      System.arraycopy(encrypted, 0, packed, iv.size, encrypted.size)
      val saved = activity.getSharedPreferences(CREDENTIAL_PREFS, Activity.MODE_PRIVATE)
        .edit()
        .putString(credentialPreferenceKey(args.service, args.account), Base64.encodeToString(packed, Base64.NO_WRAP))
        .commit()
      check(saved) { "Could not persist the encrypted credential" }
      invoke.resolve(JSObject())
    } catch (_: Exception) {
      invoke.reject("Android secure credential write failed")
    }
  }

  @Command fun credentialDelete(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CredentialArgs::class.java)
      validateCredentialIdentity(args.service, args.account)
      val deleted = activity.getSharedPreferences(CREDENTIAL_PREFS, Activity.MODE_PRIVATE)
        .edit()
        .remove(credentialPreferenceKey(args.service, args.account))
        .commit()
      check(deleted) { "Could not remove the encrypted credential" }
      invoke.resolve(JSObject())
    } catch (_: Exception) {
      invoke.reject("Android secure credential deletion failed")
    }
  }

  private fun validateCredentialIdentity(service: String, account: String) {
    require(service.isNotBlank() && service.length <= MAX_IDENTITY_LENGTH)
    require(account.isNotBlank() && account.length <= MAX_IDENTITY_LENGTH)
  }

  private fun credentialPreferenceKey(service: String, account: String): String {
    val identity = "$service\u0000$account".toByteArray(StandardCharsets.UTF_8)
    val digest = MessageDigest.getInstance("SHA-256").digest(identity)
    return Base64.encodeToString(digest, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
  }

  private fun credentialKey(): SecretKey {
    val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
    (keyStore.getKey(CREDENTIAL_KEY_ALIAS, null) as? SecretKey)?.let { return it }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
    generator.init(
      KeyGenParameterSpec.Builder(
        CREDENTIAL_KEY_ALIAS,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
      )
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setRandomizedEncryptionRequired(true)
        .setKeySize(256)
        .build(),
    )
    return generator.generateKey()
  }

  @Command fun initialize(invoke: Invoke) {
    try {
      activity.startService(Intent(activity, CaviaPlaybackService::class.java))
      awaitPlaybackService(invoke, 60)
    } catch (error: Exception) {
      invoke.reject(error.message ?: "Media3 playback service could not start")
    }
  }

  private fun awaitPlaybackService(invoke: Invoke, attemptsRemaining: Int) {
    main.postDelayed({
        val service = CaviaPlaybackService.instance
        if (service == null) {
          if (attemptsRemaining > 0) {
            awaitPlaybackService(invoke, attemptsRemaining - 1)
          } else {
            invoke.reject("Media3 playback service did not initialize")
          }
        } else {
          invoke.resolve(JSObject().put("deviceReady", service.player.playerError == null))
        }
    }, 25)
  }

  private fun respond(invoke: Invoke, action: (CaviaPlaybackService) -> Unit) {
    main.post {
      try {
        val service = CaviaPlaybackService.instance
          ?: throw IllegalStateException("Media3 playback service is not ready")
        action(service)
        invoke.resolve(JSObject())
      } catch (error: Exception) {
        invoke.reject(error.message ?: "Media3 command failed")
      }
    }
  }

  @Command fun load(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(LoadArgs::class.java)
      activity.startService(Intent(activity, CaviaPlaybackService::class.java))
      main.post {
        try {
          val service = CaviaPlaybackService.instance
            ?: throw IllegalStateException("Media3 playback service did not start")
          service.load(args.path, args.startMs, args.autoPlay)
          invoke.resolve(JSObject())
        } catch (error: Exception) { invoke.reject(error.message ?: "Unable to load audio") }
      }
    } catch (error: Exception) { invoke.reject(error.message ?: "Invalid load request") }
  }

  @Command fun pickMusicFolder(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
    }
    startActivityForResult(invoke, intent, "musicFolderResult")
  }

  @ActivityCallback
  fun musicFolderResult(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode != Activity.RESULT_OK) {
      invoke.resolve(JSObject().put("path", ""))
      return
    }
    val uri = result.data?.data ?: run { invoke.reject("The selected folder URI was unavailable"); return }
    io.execute {
      val destination = File(activity.filesDir, "music-imports/${UUID.randomUUID()}")
      try {
        val folder = DocumentFile.fromTreeUri(activity, uri) ?: error("Could not open the selected folder")
        val limits = CopyLimits()
        copyAudioTree(activity.contentResolver, folder, destination, limits)
        check(limits.files > 0) { "The selected folder contains no supported audio files" }
        invoke.resolve(JSObject().put("path", destination.absolutePath))
      } catch (error: Exception) {
        destination.deleteRecursively()
        invoke.reject(error.message ?: "Unable to stage audio from the selected folder")
      }
    }
  }

  private data class CopyLimits(var files: Int = 0, var bytes: Long = 0)

  private fun copyAudioTree(resolver: ContentResolver, folder: DocumentFile, destination: File, limits: CopyLimits) {
    check(limits.files <= 5000) { "The selected folder contains more than 5,000 audio files" }
    for (child in folder.listFiles()) {
      val safeName = File(child.name ?: "").name
      if (safeName.isBlank() || safeName == "." || safeName == "..") continue
      if (child.isDirectory) {
        copyAudioTree(resolver, child, File(destination, safeName), limits)
        continue
      }
      if (!child.isFile || safeName.substringAfterLast('.', "").lowercase() !in AUDIO_EXTENSIONS) continue
      check(limits.files < 5000) { "The selected folder contains more than 5,000 audio files" }
      destination.mkdirs()
      var target = File(destination, safeName)
      var suffix = 1
      while (target.exists()) {
        val dot = safeName.lastIndexOf('.')
        val stem = if (dot > 0) safeName.substring(0, dot) else safeName
        val extension = if (dot > 0) safeName.substring(dot) else ""
        target = File(destination, "$stem ($suffix)$extension")
        suffix++
      }
      target.parentFile?.mkdirs()
      val partial = File(target.parentFile, target.name + ".part")
      try {
        val input = resolver.openInputStream(child.uri) ?: error("Could not read ${child.name}")
        input.use { source -> FileOutputStream(partial).use { output ->
          val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
          while (true) {
            val count = source.read(buffer)
            if (count < 0) break
            limits.bytes += count
            check(limits.bytes <= MAX_STAGED_BYTES) { "The selected music folder exceeds the 5 GB import limit" }
            output.write(buffer, 0, count)
          }
        } }
        check(partial.length() > 0) { "${child.name} is empty" }
        check(partial.renameTo(target)) { "Could not finish staging ${child.name}" }
        limits.files += 1
      } finally { if (partial.exists()) partial.delete() }
    }
  }

  companion object {
    private const val CREDENTIAL_PREFS = "cavia-secure-credentials-v1"
    private const val CREDENTIAL_KEY_ALIAS = "com.cavia.music.credentials.v1"
    private const val ANDROID_KEYSTORE = "AndroidKeyStore"
    private const val MAX_CREDENTIAL_BYTES = 8192
    private const val MAX_IDENTITY_LENGTH = 512
    private const val GCM_IV_BYTES = 12
    private const val GCM_TAG_BITS = 128
    private const val MAX_STAGED_BYTES = 5L * 1024 * 1024 * 1024
    private val AUDIO_EXTENSIONS = setOf("flac", "m4a", "mp3", "wav", "ogg", "opus", "aac")
  }

  @Command fun play(invoke: Invoke) = respond(invoke) { it.player.play() }
  @Command fun pause(invoke: Invoke) = respond(invoke) { it.player.pause() }
  @Command fun stop(invoke: Invoke) = respond(invoke) { it.player.stop() }
  @Command fun seek(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(PositionArgs::class.java)
      respond(invoke) { it.player.seekTo(args.positionMs) }
    } catch (error: Exception) { invoke.reject(error.message ?: "Invalid seek request") }
  }
  @Command fun volume(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(VolumeArgs::class.java)
      respond(invoke) { it.player.volume = args.volume.toFloat().coerceIn(0f, 1f) }
    } catch (error: Exception) { invoke.reject(error.message ?: "Invalid volume request") }
  }
  @Command fun speed(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(SpeedArgs::class.java)
      respond(invoke) { it.setSpeed(args.speed.toFloat()) }
    } catch (error: Exception) { invoke.reject(error.message ?: "Invalid speed request") }
  }
  @Command fun metadata(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(MetadataArgs::class.java)
      respond(invoke) { it.setMetadata(args.title, args.artist, args.album) }
    } catch (error: Exception) { invoke.reject(error.message ?: "Invalid metadata request") }
  }

  @Command fun snapshot(invoke: Invoke) {
    main.post {
      val service = CaviaPlaybackService.instance
      val state = JSObject()
      val player = service?.player
      state.put("status", when {
        player == null -> "idle"
        player.playerError != null -> "error"
        player.playbackState == androidx.media3.common.Player.STATE_ENDED -> "stopped"
        player.playWhenReady && player.playbackState == androidx.media3.common.Player.STATE_READY -> "playing"
        player.playbackState == androidx.media3.common.Player.STATE_BUFFERING -> "loading"
        player.playbackState == androidx.media3.common.Player.STATE_READY -> "paused"
        else -> "idle"
      })
      state.put("positionMs", player?.currentPosition?.coerceAtLeast(0) ?: 0)
      state.put("durationMs", player?.duration?.takeIf { it >= 0 } ?: 0)
      state.put("ended", player?.playbackState == androidx.media3.common.Player.STATE_ENDED)
      state.put("deviceReady", service != null && player?.playerError == null)
      invoke.resolve(state)
    }
  }
}
