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
                // Reflects actual SMTC initialization, which happens in the
                // playback service after this backend is constructed.
                system_media_controls: false,
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
        player.pause();
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
        let is_active_session =
            self.status == PlaybackStatus::Playing || self.status == PlaybackStatus::Paused;
        let path = Path::new(file_path);
        if !path.exists() {
            if !is_active_session {
                self.status = PlaybackStatus::Error;
            }
            return Err(PlaybackError::FileNotFound(file_path.to_string()));
        }

        let file = File::open(path).map_err(|e| {
            if !is_active_session {
                self.status = PlaybackStatus::Error;
            }
            PlaybackError::DecoderError(format!("Could not open file for playback: {e}"))
        })?;

        let decoder = Decoder::try_from(file).map_err(|e| {
            if !is_active_session {
                self.status = PlaybackStatus::Error;
            }
            PlaybackError::DecoderError(format!("Could not decode audio stream: {e:?}"))
        })?;

        let total_dur = decoder
            .total_duration()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.ensure_player_ready()?;
        let sink = self.sink.as_ref().unwrap();

        // Connect candidate player to the mixer in paused state to prevent audio leak
        let player = Player::connect_new(&sink.mixer());
        player.pause();
        player.set_volume(self.volume);
        player.set_speed(self.speed);
        player.append(decoder);

        let clamped_start = if start_ms > 0 {
            let clamped = if total_dur > 0 {
                start_ms.min(total_dur)
            } else {
                start_ms
            };
            let speed_factor = if self.speed > 0.0 {
                self.speed as f64
            } else {
                1.0
            };
            let seek_duration = Duration::from_secs_f64((clamped as f64 / 1000.0) / speed_factor);
            if let Err(e) = player.try_seek(seek_duration) {
                eprintln!("[RodioBackend] Initial seek to {start_ms}ms returned: {e:?}");
                player.stop();
                if !is_active_session {
                    self.status = PlaybackStatus::Error;
                }
                return Err(PlaybackError::SeekError(format!(
                    "Initial seek failed: {e:?}"
                )));
            }
            clamped
        } else {
            0
        };

        // Candidate successfully prepared: atomically stop previous player before starting new one
        if let Some(old_player) = self.player.take() {
            old_player.stop();
        }

        let should_play = self.status != PlaybackStatus::Paused;
        if should_play {
            player.play();
            self.status = PlaybackStatus::Playing;
        } else {
            self.status = PlaybackStatus::Paused;
        }
        self.player = Some(player);
        self.current_path = Some(path.to_path_buf());
        self.current_duration_ms = total_dur;
        self.seek_offset_ms = clamped_start;
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
            let speed_factor = if self.speed > 0.0 {
                self.speed as f64
            } else {
                1.0
            };
            let seek_duration =
                Duration::from_secs_f64((clamped_ms as f64 / 1000.0) / speed_factor);
            match player.try_seek(seek_duration) {
                Ok(()) => {
                    self.seek_offset_ms = clamped_ms;
                    Ok(())
                }
                Err(e) => Err(PlaybackError::SeekError(format!("Seek failed: {e:?}"))),
            }
        } else {
            Err(PlaybackError::SeekError(
                "Audio player is not active".into(),
            ))
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
        if let Some(player) = &self.player {
            let current_media_ms = self.position_ms();
            self.speed = clamped;
            player.set_speed(clamped);
            let new_speed_factor = clamped as f64;
            let seek_duration =
                Duration::from_secs_f64((current_media_ms as f64 / 1000.0) / new_speed_factor);
            let _ = player.try_seek(seek_duration);
        } else {
            self.speed = clamped;
        }
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        if let Some(player) = &self.player {
            let wall_clock_secs = player.get_pos().as_secs_f64();
            let speed_factor = if self.speed > 0.0 {
                self.speed as f64
            } else {
                1.0
            };
            let mut media_ms = (wall_clock_secs * speed_factor * 1000.0).round() as u64;
            // Rodio refreshes its position from the audio callback. When a
            // track is loaded at an offset and paused immediately (startup
            // restore), that callback may not have run yet and get_pos() can
            // still read zero. The requested seek offset is authoritative
            // while paused, so an exit cannot overwrite the resume point.
            if self.status == PlaybackStatus::Paused && self.seek_offset_ms > media_ms {
                media_ms = self.seek_offset_ms;
            }
            if self.current_duration_ms > 0 {
                media_ms.min(self.current_duration_ms)
            } else {
                media_ms
            }
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

    fn validate_source(&self, file_path: &str) -> Result<(), PlaybackError> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(PlaybackError::FileNotFound(file_path.to_string()));
        }
        let file = File::open(path).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not open file for validation: {e}"))
        })?;
        let mut decoder = Decoder::try_from(file).map_err(|e| {
            PlaybackError::DecoderError(format!("Could not decode audio stream: {e:?}"))
        })?;
        // Inspect the first sample to ensure the stream contains valid, decodable audio frames
        // and is not merely an empty or truncated header.
        if decoder.next().is_none() {
            return Err(PlaybackError::DecoderError(
                "Audio stream contains no decodable audio frames or is truncated".into(),
            ));
        }
        Ok(())
    }

    fn is_device_ready(&self) -> bool {
        self.device_available && self.sink.is_some()
    }

    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        self.ensure_player_ready()
    }
}

#[cfg(test)]
pub mod tests {
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
    fn test_candidate_player_is_paused_during_initialization() {
        if let Ok((_sink, player)) = RodioBackend::init_device() {
            assert!(
                player.is_paused(),
                "Candidate player must be paused immediately upon creation before appending audio"
            );
        }
    }

    #[test]
    fn test_decoder_failure_preserves_old_player() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_dec_fail_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let valid_wav = temp_dir.join("valid.wav");
        create_test_wav_with_duration(&valid_wav, 0.5);

        let corrupt_wav = temp_dir.join("corrupt.wav");
        std::fs::write(
            &corrupt_wav,
            b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00corrupt_data",
        )
        .unwrap();

        if let Ok(mut backend) = RodioBackend::new() {
            if backend.is_device_ready() {
                // 1. Establish active playback with valid track
                let res1 = backend.load(valid_wav.to_str().unwrap(), 0);
                assert!(res1.is_ok());
                assert_eq!(backend.status(), PlaybackStatus::Playing);
                assert_eq!(backend.current_path.as_ref(), Some(&valid_wav));

                // 2. Attempt to load corrupt track
                let res2 = backend.load(corrupt_wav.to_str().unwrap(), 0);
                assert!(res2.is_err(), "Corrupt audio must fail decoding");

                // 3. Old player must be retained and active; status must remain Playing
                assert_eq!(backend.status(), PlaybackStatus::Playing);
                assert_eq!(backend.current_path.as_ref(), Some(&valid_wav));
                assert!(backend.player.is_some());
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_candidate_lifecycle_paused_state_preserved() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_pause_pres_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let track1 = temp_dir.join("t1.wav");
        let track2 = temp_dir.join("t2.wav");
        create_test_wav_with_duration(&track1, 0.5);
        create_test_wav_with_duration(&track2, 0.5);

        if let Ok(mut backend) = RodioBackend::new() {
            if backend.is_device_ready() {
                // 1. Start track 1 then pause
                backend.load(track1.to_str().unwrap(), 0).unwrap();
                backend.pause().unwrap();
                assert_eq!(backend.status(), PlaybackStatus::Paused);

                // 2. Load track 2 while paused: candidate must remain paused
                backend.load(track2.to_str().unwrap(), 0).unwrap();
                assert_eq!(
                    backend.status(),
                    PlaybackStatus::Paused,
                    "Loading track while paused must leave candidate player paused"
                );
                assert!(
                    backend.player.as_ref().unwrap().is_paused(),
                    "Candidate rodio player must be paused"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rapid_track_changes_prevent_audio_overlap() {
        let temp_dir = std::env::temp_dir().join(format!("cavia_rapid_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut tracks = Vec::new();
        for i in 0..5 {
            let p = temp_dir.join(format!("t{i}.wav"));
            create_test_wav_with_duration(&p, 0.3);
            tracks.push(p);
        }

        if let Ok(mut backend) = RodioBackend::new() {
            if backend.is_device_ready() {
                for t in &tracks {
                    let res = backend.load(t.to_str().unwrap(), 0);
                    assert!(res.is_ok());
                    assert_eq!(backend.status(), PlaybackStatus::Playing);
                    assert!(backend.player.is_some());
                }
                assert_eq!(backend.current_path.as_ref(), Some(&tracks[4]));
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn test_playback_lifecycle_with_real_audio() {
        let mut backend = RodioBackend::new().expect("backend should initialize");
        assert!(
            backend.is_device_ready(),
            "Physical audio output device required for this test"
        );
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

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn test_seeking_on_real_audio_fixture() {
        let mut backend = RodioBackend::new().expect("backend should initialize");
        assert!(
            backend.is_device_ready(),
            "Physical audio output device required for this test"
        );

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
        assert_eq!(backend.position_ms(), 500);

        // Seek in middle (2500ms)
        assert!(backend.seek(2500).is_ok());
        assert_eq!(backend.seek_offset_ms, 2500);
        assert_eq!(backend.position_ms(), 2500);

        // Seek near end (4800ms)
        assert!(backend.seek(4800).is_ok());
        assert_eq!(backend.seek_offset_ms, 4800);
        assert_eq!(backend.position_ms(), 4800);

        // Seek beyond duration (clamps to 5000ms)
        assert!(backend.seek(10000).is_ok());
        assert_eq!(backend.seek_offset_ms, 5000);
        assert_eq!(backend.position_ms(), 5000);

        let _ = backend.stop();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn test_position_and_seeking_at_multiple_speeds() {
        let mut backend = RodioBackend::new().expect("backend should initialize");
        assert!(
            backend.is_device_ready(),
            "Physical audio output device required for this test"
        );

        let temp_dir =
            std::env::temp_dir().join(format!("cavia_speed_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("speed_audio.wav");
        create_test_wav_with_duration(&wav_path, 10.0);

        assert!(backend.load(wav_path.to_str().unwrap(), 0).is_ok());

        let test_speeds = [0.5f32, 1.0f32, 1.25f32, 2.0f32];
        for &spd in &test_speeds {
            assert!(backend.set_speed(spd).is_ok());
            // Seek to 3000ms in media timeline
            assert!(backend.seek(3000).is_ok());

            // Media position immediately after seek must be 3000ms (+/- 50ms tolerance)
            let pos = backend.position_ms();
            assert!(
                (pos as i64 - 3000).abs() <= 50,
                "Expected ~3000ms at {spd}x speed, got {pos}ms"
            );
        }

        let _ = backend.stop();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn test_seek_latency_on_long_audio_is_instantaneous() {
        let mut backend = RodioBackend::new().expect("backend should initialize");
        assert!(
            backend.is_device_ready(),
            "Physical audio output device required for this test"
        );

        let temp_dir =
            std::env::temp_dir().join(format!("cavia_latency_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("long_audio_30s.wav");
        create_test_wav_with_duration(&wav_path, 30.0);

        assert!(backend.load(wav_path.to_str().unwrap(), 0).is_ok());

        let start = std::time::Instant::now();
        assert!(backend.seek(25000).is_ok());
        let elapsed = start.elapsed();

        // Native seek must complete quickly without decoding 25 seconds of PCM samples
        assert!(
            elapsed < Duration::from_millis(500),
            "Seek took too long ({elapsed:?}), slow fallback detected"
        );

        let _ = backend.stop();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_seek_without_loaded_audio_returns_honest_error() {
        if let Ok(mut backend) = RodioBackend::new() {
            let res = backend.seek(500);
            assert!(matches!(res, Err(PlaybackError::SeekError(_))));
        }
    }

    #[test]
    fn test_real_codec_fixtures_decode_and_seek() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_codec_fixtures_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let fixtures: [(&str, &[u8]); 5] = [
            ("flac", include_bytes!("../../testdata/tone.flac")),
            ("mp3", include_bytes!("../../testdata/tone.mp3")),
            ("aac_m4a", include_bytes!("../../testdata/tone_aac.m4a")),
            ("alac_m4a", include_bytes!("../../testdata/tone_alac.m4a")),
            ("aiff", include_bytes!("../../testdata/tone.aiff")),
        ];

        for (name, bytes) in fixtures {
            let path = temp_dir.join(format!("{name}.audio"));
            std::fs::write(&path, bytes).unwrap();
            let file = File::open(&path).unwrap();
            let mut decoder = Decoder::try_from(file)
                .unwrap_or_else(|e| panic!("{name}: decoder rejected fixture: {e:?}"));
            assert!(decoder.next().is_some(), "{name}: no decoded samples");
            let seek = decoder.try_seek(Duration::from_millis(500));
            assert!(seek.is_ok(), "{name}: native seek failed: {seek:?}");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn test_load_at_offset_keeps_position_when_paused() {
        let mut backend = RodioBackend::new().expect("physical audio output device required");
        assert!(
            backend.is_device_ready(),
            "physical audio output device required"
        );

        let temp_dir =
            std::env::temp_dir().join(format!("cavia_restore_pos_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("restore.flac");
        std::fs::write(&path, include_bytes!("../../testdata/tone_long.flac")).unwrap();

        backend
            .load(path.to_str().unwrap(), 2000)
            .expect("load at offset");
        backend.pause().expect("pause restored track");
        let pos = backend.position_ms();
        assert!(
            (1900..=2100).contains(&pos),
            "paused restored position should stay near 2000ms, got {pos}ms"
        );

        let _ = backend.stop();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_long_flac_seek_is_bounded() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_long_flac_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("tone_long.flac");
        std::fs::write(&path, include_bytes!("../../testdata/tone_long.flac")).unwrap();

        let file = File::open(&path).unwrap();
        let mut decoder = Decoder::try_from(file).unwrap();
        let start = std::time::Instant::now();
        decoder
            .try_seek(Duration::from_secs(25))
            .expect("long FLAC seek failed");
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "long FLAC seek took {:?}",
            start.elapsed()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
