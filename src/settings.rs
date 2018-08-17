struct EthernetConnection {
    host: String,
    port: u16,
}

struct USB3Conection {
    
}

struct VideoSource {

}

enum RecordingFormat {
    AdobeCinemaDNG,
    MLV,
}

struct RecorderSettings {
    format: RecordingFormat,
    path: Path,
}

enum DebayeringAlgorithm {
    Bilinear,
    VRNG,
    ResolutionLoss,
    Shodak,
}

struct PreviewSettings {
    debayering_algorithm: DebayeringAlgorithm,
}
