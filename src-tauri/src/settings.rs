//! Persisted user settings: the camera set survives a restart, and the
//! gesture preferences (recognition on/off, hold duration, sound) follow the user across
//! launches. Mirroring is per camera, so it lives on each `Camera`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// One camera in the user's set. `device_id` is origin-scoped and can go
/// stale across builds or reconnects; `label` is the system name at add
/// time, kept as a re-match hint and name fallback.
/// A rectangle in frame fractions (0..1), origin top-left.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn full_frame() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub device_id: String,
    pub label: String,
    pub name: String,
    pub mirror: bool,
    // Defaulted so a camera saved before rotation/crop existed still loads.
    #[serde(default)]
    pub rotation: u16,
    #[serde(default = "full_frame")]
    pub crop: Rect,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Local port the MCP server listens on (127.0.0.1 only).
    pub mcp_port: u16,
    /// The ordered camera set; order drives the overview and delivery order.
    pub cameras: Vec<Camera>,
    /// How long a gesture must be held before its action fires (1000–4000).
    pub hold_ms: u32,
    /// Play a short tone when a gesture fires.
    pub sound_on_fire: bool,
    /// Look for hands at all; off, cameras keep watching but no gesture fires.
    pub gestures_enabled: bool,
    /// Fist pause is opt-in: a pen grip can resemble it.
    pub enabled_gestures: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mcp_port: 41625,
            cameras: Vec::new(),
            hold_ms: 2000,
            sound_on_fire: true,
            gestures_enabled: true,
            enabled_gestures: [
                "thumbs-up",
                "two-thumbs-up",
                "open-palm",
                "ok-sign",
                "pointing",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}

pub fn load(path: &Path) -> Settings {
    let Ok(raw) = fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera(device_id: &str, name: &str, mirror: bool) -> Camera {
        Camera {
            device_id: device_id.to_string(),
            label: device_id.to_string(),
            name: name.to_string(),
            mirror,
            rotation: 0,
            crop: full_frame(),
        }
    }

    #[test]
    fn gesture_preferences_roundtrip_including_empty_and_opt_in_fist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut settings = Settings::default();
        settings.enabled_gestures = vec!["fist".into()];
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).enabled_gestures, vec!["fist"]);
        settings.enabled_gestures.clear();
        save(&path, &settings).unwrap();
        assert!(load(&path).enabled_gestures.is_empty());
    }

    #[test]
    fn camera_set_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = load(&path);
        assert!(settings.cameras.is_empty());

        settings.cameras = vec![
            camera("cam-overhead", "Desk", false),
            camera("cam-wall", "Wall", true),
        ];
        save(&path, &settings).unwrap();

        // A fresh load models an app restart: nothing shared but the file.
        let restarted = load(&path);
        // Ids, names, order, and mirroring all come back intact.
        assert_eq!(restarted.cameras, settings.cameras);
        assert_eq!(restarted.cameras[0].name, "Desk");
        assert_eq!(restarted.cameras[1].device_id, "cam-wall");
        assert!(restarted.cameras[1].mirror);
    }

    #[test]
    fn rotation_and_crop_survive_restart_and_default_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // A camera saved before rotation/crop existed (fields absent) loads
        // with sensible defaults.
        std::fs::write(
            &path,
            r#"{ "cameras": [
                 { "deviceId": "old", "label": "Old", "name": "Old", "mirror": false }
               ] }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.cameras[0].rotation, 0);
        assert_eq!(loaded.cameras[0].crop, full_frame());

        // A rotated, cropped camera round-trips.
        let mut settings = loaded;
        settings.cameras[0].rotation = 90;
        settings.cameras[0].crop = Rect { x: 0.25, y: 0.1, width: 0.5, height: 0.6 };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).cameras[0], settings.cameras[0]);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load(&path), Settings::default());
    }

    /// A settings file from an earlier version (single camera, global mirror,
    /// prototype-era fields) must still load: unknown fields are ignored and
    /// the fields that survived keep their values.
    #[test]
    fn legacy_settings_load_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{
              "mode": "prototype",
              "mcpPort": 41700,
              "cameraDeviceId": "cam-legacy",
              "mirror": true,
              "instructionZoneFraction": 0.3
            }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.mcp_port, 41700);
        // The old single-camera field is dropped; the set starts empty.
        assert!(loaded.cameras.is_empty());
        assert_eq!(loaded.hold_ms, 2000);
        assert!(loaded.sound_on_fire);
        assert!(loaded.gestures_enabled);
        assert!(!loaded.enabled_gestures.iter().any(|g| g == "fist"));
    }
}
