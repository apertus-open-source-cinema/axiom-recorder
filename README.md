# AXIOM RECORDER / RAW UTILITIES
![Build](https://github.com/apertus-open-source-cinema/axiom-recorder/workflows/Build/badge.svg)

Software to record and convert moving images from Apertus° AXIOM cameras via USB3 or ethernet.

This software-package features a graph-based environment for developing raw images and raw image sequences
in real time with GPU-acceleration.

## Get It!
```shell script
git clone https://github.com/apertus-open-source-cinema/axiom-recorder
cd axiom-recorder
cargo build --release --all
```

## Usage
Currently, this project only exposes a cli tool with which you can create and run image processing pipelines.
A GUI tool for doing recording in a more convenient way is planned but not implemented yet.

```sh
$ target/release/cli --help
Raw Image / Video Converter 
convert raw footage from AXIOM cameras into other formats.

USAGE:
    cli <pipeline>...

ARGS:
    <pipeline>...    example: <Node1> --source-arg ! <Node2> --sink-arg

OPTIONS:
    -h, --help    Print help information

NODES:
    * TcpReader [OPTIONS] --width <width> --height <height> --address <address>
    * Debayer
    * WebcamInput [OPTIONS]
    * DualFrameRawDecoder [OPTIONS]
    * RawBlobWriter [OPTIONS] --path <path>
    * Lut3d --file <file>
    * GpuBitDepthConverter
    * Display [OPTIONS]
    * BenchmarkSink
    * RawDirectoryReader [OPTIONS] --file-pattern <file-pattern> --width <width> --height <height>
    * RawBlobReader [OPTIONS] --file <file> --width <width> --height <height>
    * RawDirectoryWriter [OPTIONS] --path <path>
    * BitDepthConverter
    * CinemaDngWriter [OPTIONS] --path <path>
    * ColorVoodoo [OPTIONS]
    * Average --n <n>
```

## Examples

Record a cinema dng sequence via usb3 from the micro:
```shell
$ target/release/converter Usb3Reader --bit-depth 8 --height 1296 --width 2304 --red-in-first-col false --fps 30 ! CinemaDngWriter --path cinema_dng_folder'
```

Convert a raw directory to mp4 (h264) from the Beta using FFmpeg:
```shell
$ target/release/converter RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true --fps 30 ! BitDepthConverter ! Debayer ! FfmpegWriter --output darkbox.mp4
```


## Technology

This project is written in Rust making heavy use of Vulkan via vulkano.rs.
Feel free to contribute and / or ask questions :).
