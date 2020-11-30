use crate::util::{
    error::{Error, Res, ResN},
    image::Image,
    options::OptionsStorage,
};
use bus::BusReader;


#[cfg(feature = "mp4_encoder")]
use crate::debayer::Debayer;
#[cfg(feature = "mp4_encoder")]
use crate::debayer::OnscreenDebayerer;
#[cfg(feature = "mp4_encoder")]
use mpeg_encoder::Encoder;
use std::{
    cell::Cell,
    fs::{create_dir, File},
    io::prelude::*,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use crate::graphical::ui_lib::headless_util::build_context;
use glium::texture::RawImage2d;

use glium::backend::glutin::headless::Headless;
use glutin::{GlProfile, GlRequest};
use tiff_encoder::{ifd::tags, prelude::*, LONG, RATIONAL, SHORT, ASCII, BYTE, SRATIONAL};
use tiff_encoder::ifd::types::SRATIONAL;


/// An image sink, that somehow stores the images it receives
pub trait Writer {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self>
    where
        Self: Sized;
    fn write_frame(&mut self, image: Arc<Image>) -> ResN;
}

pub struct MetaWriter {
    writer: Arc<Mutex<Box<dyn Writer + Send>>>,
    bus_writer_running: Arc<Mutex<Cell<bool>>>,
}

impl Writer for MetaWriter {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self> {
        let extension = Path::new(&filename).extension();
        let bus_writer_running = Arc::new(Mutex::new(Cell::new(false)));
        match extension.and_then(|s| s.to_str()) {
            Some("raw8") => Ok(Self {
                writer: Arc::new(Mutex::new(Box::new(Raw8BlobWriter::new(filename, options)?))),
                bus_writer_running,
            }),
            #[cfg(feature = "mp4_encoder")]
            Some("mp4") => Ok(Self {
                writer: Arc::new(Mutex::new(Box::new(MpegWriter::new(filename, options)?))),
                bus_writer_running,
            }),
            Some("dng") => Ok(Self {
                writer: Arc::new(Mutex::new(Box::new(CinemaDngWriter::new(filename, options)?))),
                bus_writer_running,
            }),
            Some(extention) => Error::error(format!("No writer for file type .{}", extention)),
            None => Ok(Self {
                writer: Arc::new(Mutex::new(Box::new(Raw8FilesWriter::new(filename, options)?))),
                bus_writer_running,
            }),
        }
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        self.writer.lock().unwrap().write_frame(image)
    }
}

impl MetaWriter {
    fn start_write_from_bus(self, mut image_rx: BusReader<Arc<Image>>) {
        let bus_writer_running = self.bus_writer_running.clone();
        bus_writer_running.lock().unwrap().replace(false);

        let writer = self.writer.clone();

        thread::spawn(move || loop {
            if !bus_writer_running.lock().unwrap().get() {
                return;
            }

            let img = image_rx.recv().unwrap();
            let result = writer.lock().unwrap().write_frame(img);

            if result.is_err() {
                eprintln!("Source Error: {}", result.err().unwrap());
                return;
            }
        });
    }

    fn stop_write_from_bus(&mut self) { self.bus_writer_running.lock().unwrap().replace(false); }
}

/// A writer, that simply writes the bytes of the received images to a single
/// file
pub struct Raw8BlobWriter {
    file: File,
}

impl Writer for Raw8BlobWriter {
    fn new(filename: String, _options: &OptionsStorage) -> Res<Self> {
        Ok(Self { file: File::create(filename)? })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        self.file.write_all(&image.buffer.u8_buffer())?;
        Ok(())
    }
}

// A writer, that writes a folder of individual raw8 files
pub struct Raw8FilesWriter {
    dir_path: String,
    cnt: u64,
}

impl Writer for Raw8FilesWriter {
    fn new(filename: String, _options: &OptionsStorage) -> Res<Self> {
        create_dir(&filename)?;
        Ok(Self { dir_path: filename, cnt: 0 })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        let mut file = File::create(format!("{}/{:06}.raw8", &self.dir_path, self.cnt))?;
        file.write_all(&image.buffer.u8_buffer())?;
        Ok(())
    }
}

/// A writer, that writes cinemaDNG (a folder with DNG files)
pub struct CinemaDngWriter {
    dir_path: String,
    cnt: u64,
    fps: f32,
}

impl Writer for CinemaDngWriter {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self> {
        create_dir(&filename)?;
        Ok(Self { dir_path: filename, cnt: 0, fps: options.get_opt_parse("fps")? })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        TiffFile::new(
            Ifd::new()
                .with_entry(50706, BYTE![1, 4, 0, 0])  // DNG version
                .with_entry(tags::Compression, SHORT![1]) // No compression
                .with_entry(tags::SamplesPerPixel, SHORT![1])
                .with_entry(tags::NewSubfileType, LONG![0])
                .with_entry(tags::XResolution, RATIONAL![(1, 1)])
                .with_entry(tags::YResolution, RATIONAL![(1, 1)])
                .with_entry(tags::ResolutionUnit, SHORT!(1))
                .with_entry(tags::FillOrder, SHORT![1])
                .with_entry(tags::Orientation, SHORT![1])
                .with_entry(tags::PlanarConfiguration, SHORT![1])

                .with_entry(tags::Make, ASCII!["Apertus"])
                .with_entry(tags::Model, ASCII!["AXIOM"])
                .with_entry(50708, ASCII!("Apertus AXIOM")) // unique camera model
                .with_entry(tags::Software, ASCII!["axiom-recorder"])

                .with_entry(tags::PhotometricInterpretation, SHORT![32803]) // Black is zero
                .with_entry(33421, SHORT![2, 2]) // CFARepeatPatternDim
                .with_entry(33422, BYTE![0, 1, 1, 2]) // CFAPattern (R=0, G=1, B=2)

                // color matrix from https://github.com/apertus-open-source-cinema/misc-tools-utilities/blob/8c8e9fca96b4b3fec50756fd7a72be6ea5c7b77c/raw2dng/raw2dng.c#L46-L49
                .with_entry(50721, SRATIONAL![  // ColorMatrix1
                        (11038, 10000), (3184, 10000), (1009, 10000),
                        (3284, 10000), (11499, 10000), (1737, 10000),
                        (1283, 10000), (3550, 10000), (5967, 10000)
               ])

                .with_entry(51044, SRATIONAL![((self.fps * 10000.0) as i32, 10000)])// FrameRate

                .with_entry(tags::ImageLength, LONG![image.height])
                .with_entry(tags::ImageWidth, LONG![image.width])
                .with_entry(tags::RowsPerStrip, LONG![image.height])
                .with_entry(tags::StripByteCounts, LONG![image.buffer.packed_data.len() as u32])
                .with_entry(tags::BitsPerSample, SHORT![image.buffer.bit_depth as u16])
                .with_entry(tags::StripOffsets, ByteBlock::single(image.buffer.packed_data.to_vec()))
                .single()
        )
        .write_to(format!("{}/{:06}.dng", &self.dir_path, self.cnt))
        .unwrap();
        self.cnt += 1;
        Ok(())
    }
}

#[cfg(feature = "mp4_encoder")]
pub struct MpegWriter {
    encoder: Encoder,
    debayerer: Box<OnscreenDebayerer>,
    facade: Headless,
}

// TODO: WTF, NO!!!
#[cfg(feature = "mp4_encoder")]
unsafe impl Send for MpegWriter {}

#[cfg(feature = "mp4_encoder")]
impl Writer for MpegWriter {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self> {
        let fps: f32 = options.get_opt_parse("fps")?;
        let width: u32 = options.get_opt_parse("width")?;
        let height: u32 = options.get_opt_parse("height")?;
        let debayer_options: String = options
            .get_opt_parse("debayer-options")
            .unwrap_or(String::from("source_lin() debayer_halfresolution()"))
            .clone();

        let cb = glutin::ContextBuilder::new()
            .with_gl_profile(GlProfile::Core)
            .with_gl(GlRequest::Latest);
        let (context, _event_loop) = build_context(cb).unwrap();
        let context = unsafe { context.treat_as_current() };
        let mut facade = glium::backend::glutin::headless::Headless::new(context)?;

        let debayerer =
            Box::new(OnscreenDebayerer::new(&debayer_options, (width, height), &mut facade)?);
        let size = debayerer.get_size();

        let mut encoder = Encoder::new_with_params(
            filename,
            size.0 as usize,
            size.1 as usize,
            Some(options.get_opt_parse_or("bitrate", 40_0000)?),
            Some((1000, (fps * 1000.0) as usize)),
            Some(options.get_opt_parse_or("gop-size", 10)?),
            Some(options.get_opt_parse_or("max-b-frames", 1)?),
            None,
        );
        encoder.init();
        Ok(Self { encoder, debayerer, facade })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        let debayered = image.debayer_to_drawable(self.debayerer.as_mut(), &mut self.facade)?;
        let debayered_image: RawImage2d<u8> = debayered.read();

        self.encoder.encode_rgba(
            debayered_image.width as usize,
            debayered_image.height as usize,
            debayered_image.data.as_ref(),
            false,
        );
        Ok(())
    }
}
