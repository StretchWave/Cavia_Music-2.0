use rodio::{Decoder, MixerDeviceSink, Player, Source, stream::DeviceSinkBuilder};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{AudioBackend, AudioCapabilities, PlaybackError, PlaybackStatus};

/// Native desktop audio backend leveraging Rodio (CPAL WASAPI/CoreAudio) and Symphonia decoders.
/// Zero external C dynamic libraries required.
pub struct RodioBackend {
    sink: Option<MixerDeviceSink>,
    player: Option<Player>,
    device_available: bool,
    current_path: Option<PathBuf>,
    current_duration_ms: u64,
    status: PlaybackStatus,
    volume: f32,
    speed: f32,
    seek_offset_ms: u64,
    capabilities: AudioCapabilities,
}

impl RodioBackend {
    pub fn new() -> Result<Self, PlaybackError> {
        let (sink, player, device_available) = match Self::init_device() {
            Ok((s, p)) => (Some(s), Some(p), true),
            Err(e) => {
                eprintln!("[RodioBackend] No audio output device available at startup: {e:?}");
                (None, None, false)
            }
        };

        Ok(Self {
            sink,
            player,
            device_available,
            current_path: None,
            current_duration_ms: 0,
            status: PlaybackStatus::Idle,
            volume: 0.85,
            speed: 1.0,
            seek_offset_ms: 0,
            capabilities: AudioCapabilities {
                local_files: device_available,
                progressive_http: false,
                hls: false,
                dash: false,
                background_playback: true,
                system_media_controls: true,
                gapless: false,
                crossfade: false,
            },
        })
    }

    fn init_device() -> Result<(MixerDeviceSink, Player), PlaybackError> {
        let sink = DeviceSinkBuilder::open_default_sink().map_err(|e| {
            PlaybackError::DeviceError(format!("Failed to open default audio output sink: {e:?}"))
        })?;
        let player = Player::connect_new(&sink.mixer());
        Ok((sink, player))
    }

    fn ensure_player_ready(&mut self) -> Result<(), PlaybackError> {
        if self.sink.is_none() || self.player.is_none() {
            let (sink, player) = Self::init_device()?;
            self.sink = Some(sink);
            self.player = Some(player);
            self.device_available = true;
            self.capabilities.local_files = true;
        }
        Ok(())
    }

    fn reopen_and_seek_fallback(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        let path = self
            .current_path
            .clone()
            .ok_or_else(|| PlaybackError::SeekError("No audio file currently loaded".into()))?;

        let file = File::open(&path).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not re-open file for fallback seek: {e}"))
        })?;
        let decoder = Decoder::try_from(file).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not decode stream for seek: {e:?}"))
        })?;

        self.ensure_player_ready()?;
        let sink = self.sink.as_ref().unwrap();

        // Fresh player avoids thread sleep on previous stream
        let player = Player::connect_new(&sink.mixer());
        if position_ms > 0 {
            let skipped = decoder.skip_duration(Duration::from_millis(position_ms));
            player.append(skipped);
        } else {
            player.append(decoder);
        }

        player.set_volume(self.volume);
        player.set_speed(self.speed);
        self.seek_offset_ms = position_ms;

        if self.status == PlaybackStatus::Playing {
            player.play();
        } else {
            player.pause();
        }

        self.player = Some(player);
        Ok(())
    }
}

impl AudioBackend for RodioBackend {
    fn name(&self) -> &'static str {
        if self.device_available {
            "rodio_native"
        } else {
            "device_unavailable"
        }
    }

    fn load(&mut self, file_path: &str, start_ms: u64) -> Result<(), PlaybackError> {
        let path = Path::new(file_path);
        if !path.exists() {
            self.status = PlaybackStatus::Error;
            return Err(PlaybackError::FileNotFound(file_path.to_string()));
        }

        let file = File::open(path).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not open file for playback: {e}"))
        })?;

        let decoder = Decoder::try_from(file).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not decode audio stream: {e:?}"))
        })?;

        let total_dur = decoder
            .total_duration()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.current_duration_ms = total_dur;

        self.ensure_player_ready()?;
        let sink = self.sink.as_ref().unwrap();

        // Connect a fresh player to the mixer to immediately purge previous track
        let player = Player::connect_new(&sink.mixer());
        player.append(decoder);

        if start_ms > 0 {
            let clamped_start = if total_dur > 0 {
                start_ms.min(total_dur)
            } else {
                start_ms
            };
            if let Err(e) = player.try_seek(Duration::from_millis(clamped_start)) {
                eprintln!("[RodioBackend] Initial seek to {start_ms}ms returned: {e:?}");
            }
            self.seek_offset_ms = clamped_start;
        } else {
            self.seek_offset_ms = 0;
        }

        player.set_volume(self.volume);
        player.set_speed(self.speed);
        player.play();

        self.player = Some(player);
        self.current_path = Some(path.to_path_buf());
        self.status = PlaybackStatus::Playing;
        Ok(())
    }

    fn play(&mut self) -> Result<(), PlaybackError> {
        if let Some(player) = &self.player {
            player.play();
            self.status = PlaybackStatus::Playing;
            Ok(())
        } else {
            Err(PlaybackError::DeviceError("No active audio player".into()))
        }
    }

    fn pause(&mut self) -> Result<(), PlaybackError> {
        if let Some(player) = &self.player {
            player.pause();
            self.status = PlaybackStatus::Paused;
            Ok(())
        } else {
            Err(PlaybackError::DeviceError("No active audio player".into()))
        }
    }

    fn resume(&mut self) -> Result<(), PlaybackError> {
        self.play()
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.status = PlaybackStatus::Stopped;
        self.seek_offset_ms = 0;
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        if self.current_path.is_none() {
            return Err(PlaybackError::SeekError(
                "No audio file currently loaded".into(),
            ));
        }

        let clamped_ms = if self.current_duration_ms > 0 {
            position_ms.min(self.current_duration_ms)
        } else {
            position_ms
        };

        if let Some(player) = &self.player {
            match player.try_seek(Duration::from_millis(clamped_ms)) {
                Ok(()) => {
                    self.seek_offset_ms = clamped_ms;
                    Ok(())
                }
                Err(rodio::source::SeekError::NotSupported { .. }) => {
                    self.reopen_and_seek_fallback(clamped_ms)
                }
                Err(e) => Err(PlaybackError::SeekError(format!("Seek error: {e:?}"))),
            }
        } else {
            self.ensure_player_ready()?;
            self.reopen_and_seek_fallback(clamped_ms)
        }
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), PlaybackError> {
        let clamped = volume.clamp(0.0, 1.0);
        self.volume = clamped;
        if let Some(player) = &self.player {
            player.set_volume(clamped);
        }
        Ok(())
    }

    fn set_speed(&mut self, speed: f32) -> Result<(), PlaybackError> {
        let clamped = speed.clamp(0.25, 4.0);
        self.speed = clamped;
        if let Some(player) = &self.player {
            player.set_speed(clamped);
        }
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        if let Some(player) = &self.player {
            let pos = player.get_pos().as_millis() as u64;
            if pos > 0 { pos } else { self.seek_offset_ms }
        } else {
            self.seek_offset_ms
        }
    }

    fn duration_ms(&self) -> u64 {
        self.current_duration_ms
    }

    fn status(&self) -> PlaybackStatus {
        self.status
    }

    fn is_ended(&self) -> bool {
        if self.status == PlaybackStatus::Playing {
            if let Some(player) = &self.player {
                player.empty()
            } else {
                false
            }
        } else {
            false
        }
    }

    fn capabilities(&self) -> AudioCapabilities {
        self.capabilities.clone()
    }

    fn is_device_ready(&self) -> bool {
        self.device_available && self.sink.is_some()
    }

    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        self.ensure_player_ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    pub fn create_test_wav_with_duration(path: &Path, duration_secs: f32) {
        let sample_rate = 44100u32;
        let num_channels = 1u16;
        let bits_per_sample = 16u16;
        let num_samples = (sample_rate as f32 * duration_secs) as u32;
        let data_size = num_samples * (num_channels as u32) * ((bits_per_sample / 8) as u32);
        let byte_rate = sample_rate * (num_channels as u32) * ((bits_per_sample / 8) as u32);
        let block_align = num_channels * (bits_per_sample / 8);

        let mut file = File::create(path).expect("Create test wav");
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36u32 + data_size).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&num_channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();

        let samples = vec![0u8; data_size as usize];
        file.write_all(&samples).unwrap();
    }

    #[test]
    fn test_missing_file_returns_honest_error() {
        if let Ok(mut backend) = RodioBackend::new() {
            let res = backend.load("C:/non_existent_audio_path_cavia.flac", 0);
            assert!(matches!(res, Err(PlaybackError::FileNotFound(_))));
            assert_eq!(backend.status(), PlaybackStatus::Error);
        }
    }

    #[test]
    fn test_playback_lifecycle_with_real_audio() {
        if let Ok(mut backend) = RodioBackend::new() {
            if !backend.is_device_ready() {
                eprintln!(
                    "Skipping audio lifecycle test: No audio device available in test environment"
                );
                return;
            }
            let temp_dir =
                std::env::temp_dir().join(format!("cavia_test_audio_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            let wav_path = temp_dir.join("tone.wav");
            create_test_wav_with_duration(&wav_path, 0.5);

            assert!(backend.load(wav_path.to_str().unwrap(), 0).is_ok());
            assert_eq!(backend.status(), PlaybackStatus::Playing);

            assert!(backend.pause().is_ok());
            assert_eq!(backend.status(), PlaybackStatus::Paused);

            assert!(backend.resume().is_ok());
            assert_eq!(backend.status(), PlaybackStatus::Playing);

            assert!(backend.set_volume(0.5).is_ok());
            assert!(backend.set_speed(1.25).is_ok());

            assert!(backend.seek(100).is_ok());
            assert_eq!(backend.seek_offset_ms, 100);

            assert!(backend.stop().is_ok());
            assert_eq!(backend.status(), PlaybackStatus::Stopped);

            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }

    #[test]
    fn test_seeking_on_real_audio_fixture() {
        if let Ok(mut backend) = RodioBackend::new() {
            if !backend.is_device_ready() {
                eprintln!("Skipping seek test: No audio device available");
                return;
            }

            let temp_dir =
                std::env::temp_dir().join(format!("cavia_seek_test_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            let wav_path = temp_dir.join("long_audio.wav");
            // Create 5-second PCM WAV file
            create_test_wav_with_duration(&wav_path, 5.0);

            assert!(backend.load(wav_path.to_str().unwrap(), 0).is_ok());
            assert_eq!(backend.duration_ms(), 5000);

            // Seek near start (500ms)
            assert!(backend.seek(500).is_ok());
            assert_eq!(backend.seek_offset_ms, 500);

            // Seek in middle (2500ms)
            assert!(backend.seek(2500).is_ok());
            assert_eq!(backend.seek_offset_ms, 2500);

            // Seek near end (4800ms)
            assert!(backend.seek(4800).is_ok());
            assert_eq!(backend.seek_offset_ms, 4800);

            // Seek beyond duration (clamps to 5000ms)
            assert!(backend.seek(10000).is_ok());
            assert_eq!(backend.seek_offset_ms, 5000);

            let _ = backend.stop();
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }

    #[test]
    fn test_seek_without_loaded_audio_returns_honest_error() {
        if let Ok(mut backend) = RodioBackend::new() {
            let res = backend.seek(500);
            assert!(matches!(res, Err(PlaybackError::SeekError(_))));
        }
    }
}
