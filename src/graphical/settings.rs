// the main struct, that stores all settings data needed for drawing the UI
pub struct Settings {
    shutter_angle: f32,
    iso: f32,
    fps: f32,
    recording_format: RecordingFormat,
    grid: Grid,
}

enum Grid {
    Grid3x3,
    None,
}

enum RecordingFormat {
    CinemaDNG,
    MLV,
    rawN,
}

enum DebayeringAlgorithm {
    Bilinear,
    VRNG,
    ResolutionLoss,
    Shodak,
}
