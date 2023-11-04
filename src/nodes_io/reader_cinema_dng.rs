use crate::pipeline_processing::{
    frame::{CfaDescriptor, Frame, Raw},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dng::{tags, DngReader};
use glob::glob;
use std::{fs::File, path::PathBuf, sync::Mutex};


pub struct CinemaDngReader {
    files: Vec<PathBuf>,
    cache_frames: bool,
    internal_loop: bool,
    cache: Mutex<Vec<Option<Payload>>>,
    context: ProcessingContext,
}
impl Parameterizable for CinemaDngReader {
    const DESCRIPTION: Option<&'static str> = Some("read Cinema DNG files from a directory");


    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file-pattern", Mandatory(StringParameter))
            .with("cache-frames", Optional(BoolParameter))
            .with("internal-loop", Optional(BoolParameter))
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.take("file-pattern")?;
        let files = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = files.len();
        if frame_count == 0 {
            return Err(anyhow!("no files matched the pattern {}", file_pattern));
        }
        Ok(Self {
            files,
            cache_frames: options.take("cache-frames")?,
            internal_loop: options.take("internal-loop")?,
            cache: Mutex::new((0..frame_count).map(|_| None).collect()),
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for CinemaDngReader {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let mut frame_number = request.frame_number();
        if self.internal_loop {
            frame_number %= self.files.len() as u64;
        }
        if frame_number >= self.files.len() as u64 {
            return Err(anyhow!(
                "frame {} was requested but this stream only has a length of {}",
                frame_number,
                self.files.len()
            ));
        }

        if self.cache_frames {
            if let Some(cached) = &self.cache.lock().unwrap()[frame_number as usize] {
                return Ok(cached.clone());
            }
        }

        let path = &self.files[frame_number as usize];
        let file = File::open(path).context(format!("couldn't open DNG file {path:?}"))?;
        let dng = DngReader::read(file).context(format!("couldn't parse DNG file {path:?}"))?;
        let main_ifd = dng.main_image_data_ifd_path();
        let buffer_length = dng.needed_buffer_length_for_image_data(&main_ifd)?;
        let mut buffer = unsafe { self.context.get_uninit_cpu_buffer(buffer_length) };
        buffer.as_mut_slice(|buffer| {
            dng.read_image_data_to_buffer(&main_ifd, buffer).context("couldnt read to buffer")
        })?;

        let cfa_raw = dng
            .get_entry_by_path(&main_ifd.chain_tag(tags::ifd::CFAPattern))
            .ok_or(anyhow!("couldnt read CFA Pattern of DNG {path:?}"))?
            .value
            .as_list()
            .map(|x| x.as_u32())
            .collect::<Option<Vec<_>>>()
            .ok_or(anyhow!("couldnt interpret CFA Pattern elements as u32 of DNG {path:?} "))?;
        let cfa = CfaDescriptor {
            red_in_first_col: cfa_raw[0] == 0 || cfa_raw[2] == 0,
            red_in_first_row: cfa_raw[0] == 0 || cfa_raw[1] == 0,
        };

        let get_tag_as_u32 = |tag| {
            dng.get_entry_by_path(&main_ifd.chain_tag(tag))
                .ok_or(anyhow!("couldnt read {tag:?} of DNG {path:?}"))
                .and_then(|x| {
                    x.value
                        .as_u32()
                        .ok_or(anyhow!("couldnt interpret {tag:?} of DNG {path:?} as u32"))
                })
                .map(|x| x as u64)
        };

        let interp = Raw {
            width: get_tag_as_u32(tags::ifd::ImageWidth)?,
            height: get_tag_as_u32(tags::ifd::ImageLength)?,
            bit_depth: get_tag_as_u32(tags::ifd::BitsPerSample)?,
            fps: dng
                .get_entry_by_path(&main_ifd.chain_tag(tags::ifd::FrameRate))
                .map(|entry| {
                    entry
                        .value
                        .as_f64()
                        .ok_or(anyhow!("couldnt interpret frame rate of DNG {path:?} as f64"))
                })
                .unwrap_or_else(|| {
                    eprintln!("DNG has no fps, falling back to 24");
                    Ok(24.0)
                })?,
            cfa,
        };

        let payload = Payload::from(Frame { storage: buffer, interp });

        if self.cache_frames {
            self.cache.lock().unwrap()[frame_number as usize] = Some(payload.clone());
        }
        Ok(payload)
    }

    fn get_caps(&self) -> Caps {
        Caps {
            frame_count: if self.internal_loop { None } else { Some(self.files.len() as u64) },
            random_access: true,
        }
    }
}
