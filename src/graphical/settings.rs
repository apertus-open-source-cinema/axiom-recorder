// the main struct, that stores all settings data needed for drawing the UI
pub struct Settings {
    pub shutter_angle: f64,
    pub iso: f64,
    pub fps: f64,
    pub recording_format: RecordingFormat,
    pub grid: Grid,
}

pub enum Grid {
    Grid3x3,
    None,
}

pub enum RecordingFormat {
    CinemaDNG,
    MLV,
    RawN,
}
