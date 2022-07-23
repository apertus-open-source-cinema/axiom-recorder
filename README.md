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
Processing pipelines can either be specified directly on the cli

```sh
$ target/release/cli --help
cli-from-cli
specify the processing pipeline directly on the cli

USAGE:
    cli from-cli [PIPELINE]...

ARGS:
    <PIPELINE>...    example: <Node1> --source-arg ! <Node2> --sink-arg

OPTIONS:
    -h, --help    Print help information

NODES:
    * Lut3d --file <file>
    * BenchmarkSink
    * CinemaDngWriter [OPTIONS] --path <path>
    * ZstdBlobReader [OPTIONS] --file <file> --height <height> --width <width>
    * ReverseDualFrameRawDecoder [OPTIONS]
    * WebcamInput [OPTIONS]
    * RawDirectoryReader [OPTIONS] --height <height> --width <width> --file-pattern <file-pattern>
    * GpuBitDepthConverter
    * RawDirectoryWriter [OPTIONS] --path <path>
    * BitDepthConverter
    * TcpReader [OPTIONS] --width <width> --height <height> --address <address>
    * Cache
    * DualFrameRawDecoder [OPTIONS]
    * ColorVoodoo [OPTIONS]
    * RawBlobWriter [OPTIONS] --path <path>
    * Split --element <element>
    * Average [OPTIONS] --n <n>
    * SZ3Compress [OPTIONS] --tolerance <tolerance> --error_control <error_control> --data_type <data_type>
    * RawBlobReader [OPTIONS] --width <width> --file <file> --height <height>
    * Debayer
    * Display [OPTIONS]
```
Alternatively you can use the yaml based config file, for example
```yaml
dir_input:
  type: RawDirectoryReader
  internal-loop: true
  cache-frames: true
  file-pattern: {{input-dir}}
  width: 1920
  height: 1080
  rgb: true

dual_frame_decoder:
  type: DualFrameRawDecoder
  input: <dir_input

bitdepth_conv:
  type: BitDepthConverter
  input: <dual_frame_decoder

debayer:
  type: Debayer
  input: <bitdepth_conv

display:
  type: Display
  input: <debayer
```
The config file supports variable substitution. You can set name value pairs on the cli using `--set name=value`.

## Examples

Convert a raw directory to mp4 (h264) from the Beta using FFmpeg:
```shell
$ target/release/cli from-cli RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true --fps 30 ! BitDepthConverter ! Debayer ! FfmpegWriter --output darkbox.mp4
```

Display help for a particular node (WebcamInput in this example) and display its supported OPTIONS:
```shell
target/release/cli from-cli WebcamInput --help
```

## Technology

This project is written in Rust making heavy use of Vulkan via vulkano.rs.
Feel free to contribute and / or ask questions :).
