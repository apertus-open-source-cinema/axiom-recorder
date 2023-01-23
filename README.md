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

Processing pipelines can either be specified directly on the cli:

```sh
$ target/debug/cli from-cli --help
cli-from-cli 
specify the processing pipeline directly on the cli

USAGE:
    cli from-cli [PIPELINE]...

ARGS:
    <PIPELINE>...    example: <Node1> --source-arg ! <Node2> --sink-arg

OPTIONS:
    -h, --help    Print help information

NODES:
    * Average [OPTIONS] --n <n>
    * BenchmarkSink [OPTIONS]
    * BitDepthConverter
    * Cache [OPTIONS]
    * Calibrate --height <height> --darkframe <darkframe> --width <width>
    * CinemaDngFrameserver [OPTIONS]
    * CinemaDngReader [OPTIONS] --file-pattern <file-pattern>
    * CinemaDngWriter [OPTIONS] --path <path>
    * ColorVoodoo [OPTIONS]
    * Debayer
    * DualFrameRawDecoder [OPTIONS]
    * FfmpegWriter [OPTIONS] --output <output>
    * GpuBitDepthConverter
    * Histogram
    * Lut3d --file <file>
    * RawBlobReader [OPTIONS] --height <height> --width <width> --file <file>
    * RawBlobWriter [OPTIONS] --path <path>
    * RawDirectoryReader [OPTIONS] --height <height> --width <width> --file-pattern <file-pattern>
    * RawDirectoryWriter [OPTIONS] --path <path>
    * ReverseDualFrameRawDecoder [OPTIONS]
    * SZ3Compress [OPTIONS] --data_type <data_type> --tolerance <tolerance> --error_control <error_control>
    * Split --element <element>
    * TcpReader [OPTIONS] --width <width> --height <height> --address <address>
    * ZstdBlobReader [OPTIONS] --file <file> --width <width> --height <height>
```

Alternatively you can use the yaml based config file, for example:
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

Convert a directory of raw12 files from the Beta to mp4 (h264) using FFmpeg:
```shell
$ target/release/cli from-cli RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true --fps 30 ! BitDepthConverter ! Debayer ! FfmpegWriter --output darkbox.mp4
```

Convert a directory of raw12 files from the Beta into CinemaDng with a specified dcp file.
Information on the DCP yaml file format can be found [in the dng-rs crate](https://github.com/apertus-open-source-cinema/dng-rs/).
```shell
$ target/release/cli from-cli RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true --fps 30 ! CinemaDngWriter --dcp-yaml axiom-beta-simulated.yml --output dng_out_dir
```

Serve a directory of raw12 files from the Beta as CinemaDng files with the WebDAV frameserver:
```shell
$ target/release/cli from-cli RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true --fps 30 ! CinemaDngFrameserver --port 9178
# the frameserver can then be mounted. On macOS like so:
# mkdir -p /tmp/frameserver-mnt
# mount_webdav -o rdonly -v frameserver http://127.0.0.1:9178 /tmp/frameserver-mnt
```

Display help for a particular node (WebcamInput in this example) and display its supported OPTIONS:
```shell
target/release/cli from-cli WebcamInput --help
```

## Technology

This project is written in Rust making heavy use of Vulkan via vulkano.rs.
Feel free to contribute and / or ask questions :).
