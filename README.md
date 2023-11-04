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
target/debug/cli from-cli --help
specify the processing pipeline directly on the cli

Usage: cli from-cli [PIPELINE]...

Arguments:
  [PIPELINE]...  example: <Node1> --source-arg ! <Node2> --sink-arg

Options:
  -h, --help  Print help

NODES:
    * BenchmarkSink
          --priority <priority>  [default: 0]

    * Cache
          --size <size>  [default: 1]

    * Calibrate
          --height <height>        
          --width <width>          
          --darkframe <darkframe>

    * CinemaDngFrameserver
          --priority <priority>  [default: 0]
          --dcp-yaml <dcp-yaml>  
          --host <host>          [default: 127.0.0.1]
          --port <port>

    * CinemaDngReader
          --internal-loop <internal-loop>  
          --cache-frames <cache-frames>    
          --file-pattern <file-pattern>

    * CinemaDngWriter
          --number-of-frames <number-of-frames>  
          --path <path>                          
          --dcp-yaml <dcp-yaml>                  
          --priority <priority>                  [default: 0]

    * ColorVoodoo
          --pedestal <pedestal>  [default: 0]
          --s_gamma <s_gamma>    [default: 1]
          --v_gamma <v_gamma>    [default: 1]

    * Debayer

    * DualFrameRawDecoder
          --bayer <bayer>  [default: RGBG]
          --debug <debug>

    * FfmpegWriter
          --input-options <input-options>  [default: ]
          --output <output>                
          --priority <priority>            [default: 0]

    * Histogram

    * Lut3d
          --file <file>

    * NullFrameSource
          --fps <fps>              [default: 24]
          --height <height>        
          --bayer <bayer>          [default: RGGB]
          --uint-bits <uint-bits>  
          --fp32                   
          --rgb                    
          --fp16                   
          --width <width>          
          --rgba

    * RawBlobReader
          --cache-frames <cache-frames>  
          --fp16                         
          --fp32                         
          --uint-bits <uint-bits>        
          --width <width>                
          --bayer <bayer>                [default: RGGB]
          --height <height>              
          --rgb                          
          --fps <fps>                    [default: 24]
          --rgba                         
          --file <file>

    * RawBlobWriter
          --path <path>                          
          --priority <priority>                  [default: 0]
          --number-of-frames <number-of-frames>

    * RawDirectoryReader
          --fps <fps>                      [default: 24]
          --height <height>                
          --fp32                           
          --bayer <bayer>                  [default: RGGB]
          --cache-frames <cache-frames>    
          --internal-loop <internal-loop>  
          --fp16                           
          --uint-bits <uint-bits>          
          --rgb                            
          --file-pattern <file-pattern>    
          --width <width>                  
          --rgba

    * RawDirectoryWriter
          --number-of-frames <number-of-frames>  
          --priority <priority>                  [default: 0]
          --path <path>

    * ReverseDualFrameRawDecoder
          --flip <flip>

    * Split
          --element <element>

    * TcpReader
          --fps <fps>              [default: 24]
          --address <address>      
          --rgb                    
          --uint-bits <uint-bits>  
          --fp16                   
          --width <width>          
          --height <height>        
          --fp32                   
          --bayer <bayer>          [default: RGGB]
          --rgba

    * ZstdBlobReader
          --uint-bits <uint-bits>  
          --fps <fps>              [default: 24]
          --rgba                   
          --file <file>            
          --height <height>        
          --bayer <bayer>          [default: RGGB]
          --rgb                    
          --fp16                   
          --fp32                   
          --width <width>
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

### Frame interpretations
Some Nodes e.g. `RawBlobReader` or `RawDirectoryReader` need to know how to interpret the data. For this some things need to be specified:

* A `--width` and a `--height`
* A frame rate with `--fps`
* A format for the samples (pixels / color channels of the pixels) needs to be specified: Either `--fp16`, `--fp32` or `--uint-bits N` where `N` is the number of bits per sample (e.g. 12 for the AXIOM Beta).
* A color format. This can either be `--rgb`, `--rgba` or `--bayer PATTERN`. Bayer Patterns are specified as left to right top to bottom as a 2x2 pixel string indicating the color. E.g. `GRBG` is the following cfa pattern
  | Green | Red   | Green | Red   | ... |
  |-------|-------|-------|-------|-----|
  | Blue  | Green | Blue  | Green | ... |
  | Green | Red   | Green | Red   | ... |
  | Blue  | Green | Blue  | Green | ... |
  | ...   | ...   | ...   | ...   | ... |


These parameters only specify the interpretation but no conversions. So if you want to get an RGB image from a Bayer source you still need to take care of conversion with a `Debayer` node in between.

### DNG output
In the pipeline of the AXIOM raw recorder, sometimes images in floating point format can occur
(for example when averaging multiple frames). Although the DNG specification specifies floating 
point DNG files, not a lot of software actually supports it. To avoid issues with downstream
processing software you can convert your data to uint16 before writing DNG files. Example:

```shell
$ target/release/cli from-cli RawBlobReader --fp32 --width 3840 --height 2160 --bayer GBRG --file Halogen_Xrite_0_8ms.raw32 ! Fp32ToUInt16 --multiplier 16 ! CinemaDngWriter --path out
```

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
