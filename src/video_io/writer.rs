use crate::util::{
    error::{Error, Res, ResN},
    image::Image,
    options::OptionsStorage,
};
use bus::BusReader;
use glium::{self, backend::glutin::headless::Headless, texture::RawImage2d};
use glutin::dpi::PhysicalSize;

#[cfg(feature = "mp4_encoder")]
use crate::debayer::Debayerer;
#[cfg(feature = "mp4_encoder")]
use crate::debayer::Debayer;
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

use tiff_encoder::{
    ifd::tags,
    prelude::*,
    ASCII,
    BYTE,
    DOUBLE,
    FLOAT,
    LONG,
    RATIONAL,
    SBYTE,
    SHORT,
    SLONG,
    SRATIONAL,
    SSHORT,
    UNDEFINED,
};

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
        &self.file.write_all(&image.data)?;
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
        file.write_all(&image.data)?;
        Ok(())
    }
}

/// A writer, that writes cinemaDNG (a folder with DNG files)
pub struct CinemaDngWriter {
    dir_path: String,
    cnt: u64,
}

impl Writer for CinemaDngWriter {
    fn new(filename: String, _options: &OptionsStorage) -> Res<Self> {
        create_dir(&filename)?;
        Ok(Self { dir_path: filename, cnt: 0 })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        TiffFile::new(
            Ifd::new()
                .with_entry(tags::PhotometricInterpretation, SHORT![1]) // Black is zero
                .with_entry(tags::Compression, SHORT![1]) // No compression

                .with_entry(tags::ImageLength, LONG![image.height])
                .with_entry(tags::ImageWidth, LONG![image.width])

                .with_entry(tags::ResolutionUnit, SHORT![1]) // No resolution unit
                .with_entry(tags::XResolution, RATIONAL![(1, 1)])
                .with_entry(tags::YResolution, RATIONAL![(1, 1)])

                .with_entry(tags::RowsPerStrip, LONG![image.height])
                .with_entry(tags::StripByteCounts, LONG![image.data.len() as u32])
                .with_entry(tags::StripOffsets, ByteBlock::single(image.data.clone()))
                .single(), // This is the only Ifd in its IfdChain
        )
        .write_to(format!("{}/{:06}.dng", &self.dir_path, self.cnt))
        .unwrap();
        Ok(())
    }
}

#[cfg(feature = "mp4_encoder")]
pub struct MpegWriter {
    encoder: Encoder,
    debayerer: Box<Debayerer>,
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
        let debayer_options: String = ((options
            .get_opt_parse("debayer-options")
            .unwrap_or(String::from("source_lin() debayer_halfresolution()"))):
            String)
            .clone();

        let event_loop = glutin::event_loop::EventLoop::new();
        let mut facade = Headless::new(
            glutin::ContextBuilder::new()
                .build_headless(&event_loop, PhysicalSize::new(1, 1))?,
        )?;

        let debayerer = Box::new(Debayerer::new(&debayer_options, (width, height), &mut facade)?);
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
        let debayered = image.debayer_drawable(self.debayerer.as_mut(), &mut self.facade)?;
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
