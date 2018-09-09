// the main struct, that stores all settings data needed for drawing the UI
pub struct Settings {
    pub shutter_angle: f32,
    pub iso: f32,
    pub fps: f32,
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
