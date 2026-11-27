package com.stretchwave.cavia

import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.MediaMetadata
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import java.io.File

class CaviaPlaybackService : MediaSessionService() {
  lateinit var player: ExoPlayer
    private set
  private var mediaSession: MediaSession? = null

  override fun onCreate() {
    super.onCreate()
    player = ExoPlayer.Builder(this).build()
    mediaSession = MediaSession.Builder(this, player).build()
    instance = this
  }

  fun load(path: String, startMs: Long) {
    val uri = if (path.startsWith("content://") || path.startsWith("file://")) {
      Uri.parse(path)
    } else {
      Uri.fromFile(File(path))
    }
    player.setMediaItem(MediaItem.fromUri(uri), startMs.coerceAtLeast(0))
    player.prepare()
    player.play()
  }

  fun setSpeed(speed: Float) {
    require(speed in 0.25f..4f) { "Playback speed must be between 0.25x and 4x" }
    player.playbackParameters = PlaybackParameters(speed)
  }

  fun setMetadata(title: String, artist: String, album: String) {
    player.replaceMediaItem(0, player.currentMediaItem!!.buildUpon()
      .setMediaMetadata(MediaMetadata.Builder().setTitle(title).setArtist(artist).setAlbumTitle(album).build())
      .build())
  }

  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = mediaSession

  override fun onDestroy() {
    instance = null
    mediaSession?.run { player.release(); release() }
    mediaSession = null
    super.onDestroy()
  }

  companion object { @Volatile var instance: CaviaPlaybackService? = null }
}
