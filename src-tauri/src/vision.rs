//! Hand-pose detection through Apple Vision. The Swift side
//! (swift/TafelVision) runs `VNDetectHumanHandPoseRequest` and hands back
//! JSON; this module is the typed Rust face of that call. Everything stays
//! on the machine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use swift_rs::{swift, SRData, SRString};

swift!(fn tafel_detect_hands(image: &SRData) -> SRString);

/// Frame-fraction coordinates, origin top-left.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HandPoint {
    pub x: f64,
    pub y: f64,
    pub confidence: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hand {
    /// "left", "right", or "unknown" (before macOS 14 Vision reports none).
    pub chirality: String,
    /// Keyed by joint name: wrist, thumbCmc … littleTip (21 joints).
    pub points: HashMap<String, HandPoint>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Detection {
    pub hands: Vec<Hand>,
    pub error: Option<String>,
}

/// Detects up to two hands in an encoded image (JPEG or PNG).
pub fn detect_hands(image: &[u8]) -> Result<Detection, String> {
    let data: SRData = image.into();
    let json = unsafe { tafel_detect_hands(&data) };
    let detection: Detection = serde_json::from_str(json.as_str()).map_err(|e| e.to_string())?;
    match detection.error {
        Some(error) => Err(error),
        None => Ok(detection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat grey image has no hands; also proves the bridge round-trips.
    #[test]
    fn blank_image_yields_no_hands() {
        let blank = include_bytes!("../fixtures/blank.png");
        let detection = detect_hands(blank).expect("vision should accept a png");
        assert!(detection.hands.is_empty());
    }

    /// Real hand photo, supplied by the user at src-tauri/fixtures/hand.jpg
    /// (not committed). Skips loudly when absent.
    #[test]
    fn hand_photo_yields_at_least_one_hand() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/hand.jpg");
        let Ok(jpeg) = std::fs::read(&path) else {
            eprintln!("skipped: no hand photo at {}", path.display());
            return;
        };
        let detection = detect_hands(&jpeg).expect("vision should accept a jpeg");
        assert!(!detection.hands.is_empty(), "expected a hand in {}", path.display());
        let hand = &detection.hands[0];
        assert_eq!(hand.points.len(), 21);
        let wrist = &hand.points["wrist"];
        assert!((0.0..=1.0).contains(&wrist.x) && (0.0..=1.0).contains(&wrist.y));
    }

    #[test]
    fn garbage_bytes_report_an_error_instead_of_crashing() {
        let result = detect_hands(b"not an image");
        assert!(result.is_err());
    }
}
