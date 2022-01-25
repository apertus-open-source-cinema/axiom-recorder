# AXIOM RECORDER / RAW UTILITIES
![Build](https://github.com/apertus-open-source-cinema/axiom-recorder/workflows/Build/badge.svg)

Software to record and convert moving images from Apertus° AXIOM cameras via USB3 or ethernet.

## Get It!
```shell script
sudo apt install cmake
git clone https://github.com/apertus-open-source-cinema/axiom-recorder
cd axiom-recorder
cargo build --release --all
```

If you want to be able to use the gstreamer integration, add `--features gst`
to your `cargo` commands. This requires you to install the following packages
(on ubuntu): `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`, `gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good`, `gstreamer1.0-plugins-bad`, `gstreamer1.0-plugins-ugly`, `gstreamer1.0-libav`, `libgstrtspserver-1.0-dev`, `libges-1.0-dev`, `libgstreamer-plugins-bad1.0-dev`

## Usage
Currently, this project only exposes a cli tool with which you can create and run Image processing pipelines.
A GUI tool for doing recording in a more convenient way is planned.

```shell
$ target/release/converter --help
Raw Image / Video Converter 
convert raw footage from AXIOM cameras into other formats.

USAGE:
    converter [--app-args] ! <VideoSource> --source arg ! <VideoSink> --sink arg

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

NODES:
    * RawDirectoryWriter --path <path>
    * BitDepthConverter
    * FfmpegWriter [OPTIONS] --fps <fps> --output <output>
    * RawDirectoryReader [OPTIONS] --bit-depth <bit-depth> --file-pattern <file-pattern> --height <height> --width <width> --red-in-first-col <true/false> --red-in-first-row <true/false>
    * Usb3Reader [OPTIONS] --bit-depth <bit-depth> --height <height> --width <width>
    * Debayer
    * RawBlobWriter --path <path>
    * CinemaDngWriter --fps <fps> --path <path>
    * RawBlobReader [OPTIONS] --bit-depth <bit-depth> --file <file> --height <height> --width <width>
    * GstWriter --pipeline <pipeline>
    * Display [OPTIONS]
```

## Examples

Record a cinema dng sequence via usb3 from the micro:
```shell
$ target/release/converter ! Usb3Reader --bit-depth 8 --height 1296 --width 2304 --red-in-first-col false ! CinemaDngWriter --fps 30 --path cinema_dng_folder'
```

Display Live Video from the micro via usb3:
```shell
$ target/release/converter ! Usb3Reader --bit-depth 8 --height 1296 --width 2304 --red-in-first-col false ! Debayer ! Display'
```

Convert a raw directory to mp4 (h264) from the Beta using FFmpeg:
```shell
$ target/release/converter  ! RawDirectoryReader --file-pattern '~/Darkbox-Timelapse-Clock-Sequence/*.raw12' --bit-depth 12 --height 3072 --width 4096 --loop true ! BitDepthConverter ! Debayer ! FfmpegWriter --output darkbox.mp4
```


## Technology

This project is written in Rust making heavy use of Vulkan via vulkano.rs.
Feel free to contribute and / or ask questions :).
