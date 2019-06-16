// the main struct, that stores all settings data needed for drawing the UI
pub struct Settings {
    pub shutter_angle: f64,
    pub iso: f64,
    pub fps: f64,
    pub recording_format: RecordingFormat,
    pub grid: Grid,
}

impl Settings {
    pub fn as_text(&self) -> Vec<String> {
        vec![
            format!("ISO {}", self.iso),
            format!("{} fps", self.fps),
            format!("{}°", self.shutter_angle),
            format!("{:#?}", self.recording_format),
            format!("{:#?}", self.grid),
        ]
    }
}

#[derive(Debug)]
pub enum Grid {
    Grid3x3,
    NoGrid,
}

#[derive(Debug)]
pub enum RecordingFormat {
    CinemaDNG,
    MLV,
    Raw8,
    Raw12,
    Raw16,
}
