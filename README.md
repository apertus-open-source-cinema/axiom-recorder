# AXIOM RECORDER
[![Build Status](https://api.cirrus-ci.com/github/axiom-micro/recorder.svg)](https://cirrus-ci.com/github/axiom-micro/recorder)

Software to record and convert moving images from Apertus° AXIOM cameras via USB3 or ethernet.

# get it!
```shell script
git clone https://github.com/axiom-micro/recorder
cd recorder
cargo run buid --release --all  # add --features mp4_encoder if you want to write .mp4 files
```

# Usage
This project contains two binaries. One GUI tool for recording / previewing footage and one CLI tool for offline 
converting already recorded footage.

## recorder
```shell script
$ cargo run --bin recorder -- --help
AXIOM recorder 0.1
record raw footage from AXIOM cameras

USAGE:
    recorder [FLAGS] [OPTIONS] --height <height> --video-source <video_source> --width <width>

FLAGS:
        --help            Prints help information
        --no-histogram    disables the histogram calculation. potentially saves A LOT of cpu ressources
    -V, --version         Prints version information

OPTIONS:
        --debayer-options <debayer-options>    Combine a 'source_*' with a 'debayer_*'. Builtin available options are 
                                               * debayer_halfresolution.glsl() [resolution_div: 2]
                                               * debayer_linearinterpolate.glsl() []
                                               * source_lin.glsl() []
                                               * source_log.glsl(out_bits: 8, in_bits: 12, a: 0.021324) []
        --fps <fps>                            
    -h, --height <height>                      
    -s, --video-source <video_source>          a URI that describes the video source to use. Can be file:// or tcp://
    -w, --width <width>                      
```

## converter
```shell script
$ cargo run --bin converter -- --help
Raw Image / Video Converter 0.1
convert raw footage from AXIOM cameras into other formats.

USAGE:
    converter [OPTIONS] --input <input> --output <output>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --bitrate <bitrate>                    
        --debayer-options <debayer-options>    Combine a 'source_*' with a 'debayer_*'. Builtin available options are 
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
