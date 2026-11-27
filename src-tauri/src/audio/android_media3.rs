//! Android Media3 bridge. The Rust playback actor remains the single Cavia
//! playback contract; ExoPlayer and its media session live in a native service.

use super::{AudioBackend, AudioCapabilities, PlaybackError, PlaybackStatus};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Manager, Wry,
    plugin::{Builder, PluginHandle, TauriPlugin},
};

pub struct Media3PluginHandle(pub PluginHandle<Wry>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::<Wry>::new("cavia-media3")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle =
                    api.register_android_plugin("com.stretchwave.cavia", "Media3Plugin")?;
                if let Err(error) = handle.run_mobile_plugin::<serde_json::Value>("initialize", ())
                {
                    eprintln!("[AudioBackend] Android Media3 initialization failed: {error}");
                }
                app.manage(Media3PluginHandle(handle));
            }
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    status: String,
    position_ms: u64,
    duration_ms: u64,
    ended: bool,
    device_ready: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: "idle".into(),
            position_ms: 0,
            duration_ms: 0,
            ended: false,
            device_ready: false,
        }
    }
}

pub struct AndroidMedia3Backend {
    handle: PluginHandle<Wry>,
    snapshot: Snapshot,
    last_poll: Instant,
}

impl AndroidMedia3Backend {
    pub fn new(app: AppHandle) -> Self {
        let handle = app.state::<Media3PluginHandle>().0.clone();
        let mut backend = Self {
            handle,
            snapshot: Snapshot::default(),
            last_poll: Instant::now(),
        };
        backend.refresh();
        backend
    }

    fn command<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
        payload: impl serde::Serialize,
    ) -> Result<T, PlaybackError> {
        self.handle
            .run_mobile_plugin(name, payload)
            .map_err(|error| {
                PlaybackError::BackendUnavailable(format!("Media3 {name} failed: {error}"))
            })
    }

    fn refresh(&mut self) {
        if let Ok(snapshot) = self.command("snapshot", ()) {
            self.snapshot = snapshot;
        }
    }
    fn load_internal(
        &mut self,
        file_path: &str,
        start_ms: u64,
        auto_play: bool,
    ) -> Result<(), PlaybackError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            path: &'a str,
            start_ms: u64,
            auto_play: bool,
        }
        let _: serde_json::Value = self.command(
            "load",
            Args {
                path: file_path,
                start_ms,
                auto_play,
            },
        )?;
        self.refresh();
        Ok(())
    }
}

impl AudioBackend for AndroidMedia3Backend {
    fn name(&self) -> &'static str {
        "android_media3_exoplayer"
    }
    fn load(&mut self, file_path: &str, start_ms: u64) -> Result<(), PlaybackError> {
        self.load_internal(file_path, start_ms, true)
    }
    fn load_paused(&mut self, file_path: &str, start_ms: u64) -> Result<(), PlaybackError> {
        self.load_internal(file_path, start_ms, false)
    }
    fn play(&mut self) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("play", ())?;
        self.refresh();
        Ok(())
    }
    fn set_metadata(
        &mut self,
        title: &str,
        artist: &str,
        album: &str,
    ) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command(
            "metadata",
            serde_json::json!({"title": title, "artist": artist, "album": album}),
        )?;
        Ok(())
    }
    fn pause(&mut self) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("pause", ())?;
        self.refresh();
        Ok(())
    }
    fn resume(&mut self) -> Result<(), PlaybackError> {
        self.play()
    }
    fn stop(&mut self) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("stop", ())?;
        self.refresh();
        Ok(())
    }
    fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        let _: serde_json::Value =
            self.command("seek", serde_json::json!({"positionMs": position_ms}))?;
        self.refresh();
        Ok(())
    }
    fn set_volume(&mut self, volume: f32) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("volume", serde_json::json!({"volume": volume}))?;
        Ok(())
    }
    fn set_speed(&mut self, speed: f32) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("speed", serde_json::json!({"speed": speed}))?;
        Ok(())
    }
    fn position_ms(&self) -> u64 {
        self.snapshot.position_ms
    }
    fn duration_ms(&self) -> u64 {
        self.snapshot.duration_ms
    }
    fn status(&self) -> PlaybackStatus {
        match self.snapshot.status.as_str() {
            "playing" => PlaybackStatus::Playing,
            "paused" => PlaybackStatus::Paused,
            "loading" => PlaybackStatus::Loading,
            "stopped" => PlaybackStatus::Stopped,
            "error" => PlaybackStatus::Error,
            _ => PlaybackStatus::Idle,
        }
    }
    fn is_ended(&self) -> bool {
        self.snapshot.ended
    }
    fn capabilities(&self) -> AudioCapabilities {
        AudioCapabilities {
            local_files: true,
            progressive_http: false,
            hls: false,
            dash: false,
            background_playback: true,
            system_media_controls: true,
            gapless: false,
            crossfade: false,
        }
    }
    fn validate_source(&self, path: &str) -> Result<(), PlaybackError> {
        if path.starts_with("content://") {
            return Ok(());
        }
        if let Ok(parsed) = url::Url::parse(path)
            && parsed.scheme() == "file"
            && let Ok(local_path) = parsed.to_file_path()
            && local_path.is_file()
        {
            return Ok(());
        }
        if std::path::Path::new(path).is_file() {
            Ok(())
        } else {
            Err(PlaybackError::FileNotFound(path.to_string()))
        }
    }
    fn is_device_ready(&self) -> bool {
        self.snapshot.device_ready
    }
    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        let _: serde_json::Value = self.command("initialize", ())?;
        self.refresh();
        if self.snapshot.device_ready {
            Ok(())
        } else {
            Err(PlaybackError::DeviceError(
                "Android audio output is unavailable.".into(),
            ))
        }
    }
    fn poll(&mut self) {
        if self.last_poll.elapsed() >= Duration::from_millis(750) {
            self.last_poll = Instant::now();
            self.refresh();
        }
    }
}
