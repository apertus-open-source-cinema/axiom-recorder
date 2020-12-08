# AXIOM RECORDER
![Build](https://github.com/apertus-open-source-cinema/axiom-recorder/workflows/Build/badge.svg)

Software to record and convert moving images from Apertus° AXIOM cameras via USB3 or ethernet.

## get it!
```shell script
git clone https://github.com/axiom-micro/recorder
cd recorder
cargo run buid --release --all  # add --features mp4_encoder if you want to write .mp4 files
```

If you want to be able to use the mpeg-encoder, add `--features mp4_encoder`
to your `cargo` commands. This requires you to install the following packages
(on ubuntu): `libavformat-dev`, `libavcodec-dev`, `libavfilter-dev`, `libavdevice-dev`, `clang`, `libclang`
# Usage
This project contains two binaries. One GUI tool for recording / previewing footage and one CLI tool for offline 
converting already recorded footage.


## recorder
```shell script
$ cargo run --release --bin recorder -- --help
AXIOM recorder 0.1
record raw footage from AXIOM cameras

USAGE:
    recorder [FLAGS] [OPTIONS] --height <height> --video-source <video_source> --width <width>

FLAGS:
        --help            Prints help information
        --no-histogram    disables the histogram calculation. potentially saves A LOT of cpu ressources
    -V, --version         Prints version information

OPTIONS:
        --gpu-options <gpu-options>    Combine a 'source_*' with a 'debayer_*'. Builtin available options are 
                                               * debayer_halfresolution.glsl() [resolution_div: 2]
                                               * debayer_linearinterpolate.glsl() []
                                               * source_lin.glsl() []
                                               * source_log.glsl(out_bits: 8, in_bits: 12, a: 0.021324) []
        --fps <fps>                            
    -h, --height <height>                      
    -s, --video-source <video_source>          a URI that describes the video source to use. Can be file:// or tcp://
    -w, --width <width>                      
```
### beta raw12 example
Assuming you have a set of files ending in `.raw12` in the directory `test/Darkbox-Timelapse-Clock-Sequence/` you can play them debayered at half resolution using:
```shell script
cargo run --release -- --loop --no-histogram --height 3072 --width 4096 --video-source 'file://test/Darkbox-Timelapse-Clock-Sequence/*.raw12' --gpu-options 'source_beta(); debayer_halfresolution_real();'
```
For full resolution debayering use
```shell script
cargo run --release -- --loop --no-histogram --height 3072 --width 4096 --video-source 'file://test/Darkbox-Timelapse-Clock-Sequence/*.raw12' --gpu-options 'source_beta(); debayer_beta_linearinterpolate_quick();'
```

## converter
```shell script
$ cargo run --release --bin converter -- --help
Raw Image / Video Converter 0.1
convert raw footage from AXIOM cameras into other formats.

USAGE:
    converter [OPTIONS] --input <input> --output <output>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --bitrate <bitrate>                    
        --gpu-options <gpu-options>    Combine a 'source_*' with a 'debayer_*'. Builtin available options are 
                                               * source_lin.glsl() []
                                               * debayer_linearinterpolate.glsl() []
                                               * debayer_halfresolution.glsl() [resolution_div: 2]
                                               * source_log.glsl(in_bits: 12, a: 0.021324, out_bits: 8) []
        --fps <fps>                            
        --gop-size <gop-size>                  
    -h, --height <height>                      
    -i, --input <input>                        the path of the input video / image
        --max-b-frames <max-b-frames>          
    -o, --output <output>                      the path of the output video / image
    -w, --width <width>                        
```
