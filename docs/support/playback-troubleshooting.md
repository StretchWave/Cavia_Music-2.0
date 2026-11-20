# Cavia Music — Playback & Audio Engine Troubleshooting

---

## 1. No Audio Output or Device Disconnected

### Symptoms
- Play button toggles, timecode advances, but no sound is heard.
- Error message appears: *"Playback engine error: audio device unavailable"*.

### Resolution Steps
1. **Verify Default Audio Device**: Check that your operating system's default playback output (speakers, headphones, or DAC) is active and unmuted.
2. **Audio Device Disconnect Recovery**: If headphones were unplugged during playback, click **Retry Audio Device** in **Settings → Audio & Engine → Audio Backend Status**.
3. **Exclusive Mode Conflicts**: Cavia uses non-exclusive shared WASAPI on Windows. Ensure other professional audio software (DAWs, ASIO bridges) is not holding exclusive locks on your sound card.

---

## 2. Unsupported Audio Codecs or Corrupt Files

### Supported Formats
Cavia Music supports the following audio formats natively via Rust decoders:
- **Lossless**: FLAC (`.flac`), ALAC (`.m4a`, `.alac`), WAV (`.wav`), AIFF (`.aiff`, `.aif`).
- **Lossy**: MP3 (`.mp3`), AAC (`.aac`, `.m4a`), OGG Vorbis (`.ogg`), Opus (`.opus`), WMA (`.wma`).

### Symptoms & Fallbacks
- Track is listed in the library but skips immediately or displays an unplayable warning.
- **Cause**: The audio file is DRM-protected (e.g. FairPlay `.m4p`), uses an unsupported proprietary container, or the physical file was deleted/moved from disk.
- **Resolution**: Check the file path in track details. If the file was moved to a new drive, run **Scan Folder** on the new location to let Cavia reconnect the file automatically.

---

## 3. High CPU Usage or Scrubber Latency

### Resolution Steps
- Cavia utilizes a dedicated audio actor thread with hardware-accelerated timecode tracking.
- If scrubbing is sluggish on long recordings (e.g. 2-hour DJ sets):
  1. Ensure the audio file is stored on an internal SSD or fast local drive rather than a slow network share (SMB/NFS).
  2. Verify that high-frequency audio decoupling is active (default in v2.0.0+).
