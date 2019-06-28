use crate::{
    debayer::Debayer,
    util::{
        error::{Error, Res, ResN},
        image::Image,
        options::OptionsStorage,
    },
};
use bus::BusReader;

use mpeg_encoder::Encoder;
use std::{
    any::Any,
    cell::Cell,
    collections::HashMap,
    fs::{create_dir, File},
    io::{prelude::*, ErrorKind},
    path::Path,
    sync::{
        atomic::AtomicBool,
        mpsc::{channel, Sender},
        Arc,
        Mutex,
    },
    thread,
};

/// An image sink, that somehow stores the images it receives
pub trait Writer {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self>
    where
        Self: Sized;
    fn write_frame(&mut self, image: Arc<Image>) -> ResN;
}

pub struct BusWriter {
    writer: Mutex<Box<dyn Writer>>,
    bus_writer_running: Arc<Mutex<Cell<bool>>>,
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
            Some("mp4") => Ok(Self {
                writer: Arc::new(Mutex::new(Box::new(MpegWriter::new(filename, options)?))),
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
            writer.lock().unwrap().write_frame(img);
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

    fn write_frame(&mut self, _image: Arc<Image>) -> ResN {
        unimplemented!();
    }
}

pub struct MpegWriter {
    encoder: Encoder,
    debayer_options: String,
}

// TODO: WTF, NO!!!
unsafe impl Send for MpegWriter {}

impl Writer for MpegWriter {
    fn new(filename: String, options: &OptionsStorage) -> Res<Self> {
        let fps: f32 = options.get_opt_parse("fps")?;
        let width: u64 = options.get_opt_parse("width")?;
        let height: u64 = options.get_opt_parse("height")?;
        let debayer_options: String = ((options
            .get_opt_parse("debayer-options")
            .unwrap_or(String::from("source_lin() debayer_halfresolution()"))):
            String)
            .clone();

        let mut encoder = Encoder::new_with_params(
            filename,
            width as usize,
            height as usize,
            None,
            Some((1000, (fps * 1000.0) as usize)),
            None,
            None,
            None,
        );
        encoder.init();
        Ok(Self { encoder, debayer_options })
    }

    fn write_frame(&mut self, image: Arc<Image>) -> ResN {
        self.encoder.encode_rgba(
            image.width as usize / 2,
            image.height as usize / 2,
            image.debayer(&self.debayer_options)?.data.as_ref(),
            false,
        );
        Ok(())
    }
}
