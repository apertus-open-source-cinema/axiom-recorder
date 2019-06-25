use crate::video_io::{debayer::Debayer, dng::Dng, Image};
use bus::BusReader;
use mpeg_encoder::Encoder;
use std::{
    fs::{create_dir, File},
    io::{prelude::*, Error, ErrorKind},
    path::Path,
    sync::{
        mpsc::{channel, Sender},
        Arc,
    },
    thread,
};

/// An image sink, that somehow stores the images it receives
pub trait Writer {
    fn start(image_rx: BusReader<Arc<Image>>, filename: String) -> Self
    where
        Self: Sized;
    fn stop(&self);
}

pub struct PathWriter {
    writer: Box<dyn Writer>,
}
impl PathWriter {
    pub fn from_path(
        image_rx: BusReader<Arc<Image>>,
        filename: String,
    ) -> Result<PathWriter, Error> {
        let extension = Path::new(&filename).extension();
        match extension.and_then(|s| s.to_str()) {
            Some("raw8") => {
                Ok(PathWriter { writer: Box::new(Raw8BlobWriter::start(image_rx, filename)) })
            }
            Some("mp4") => {
                Ok(PathWriter { writer: Box::new(MpegWriter::start(image_rx, filename)) })
            }
            Some(_) => Err(Error::new(ErrorKind::InvalidData, "file type is not supported")),
            None => Ok(PathWriter { writer: Box::new(Raw8FilesWriter::start(image_rx, filename)) }),
        }
    }
    pub fn stop(&self) { self.writer.stop() }
}

/// A writer, that simply writes the bytes of the received images to a single
/// file
pub struct Raw8BlobWriter {
    stop_channel: Sender<()>,
}

impl Writer for Raw8BlobWriter {
    fn start(mut image_rx: BusReader<Arc<Image>>, filename: String) -> Self {
        let (tx, rx) = channel::<()>();

        thread::spawn(move || {
            let mut file = File::create(filename).unwrap();

            loop {
                if rx.try_recv().is_ok() {
                    break;
                }

                let img = image_rx.recv().unwrap();
                file.write_all(&img.data).unwrap();
            }
        });

        Self { stop_channel: tx }
    }

    fn stop(&self) { self.stop_channel.send(()).unwrap(); }
}

// A writer, that writes a folder of individual raw8 files
pub struct Raw8FilesWriter {
    stop_channel: Sender<()>,
}

impl Writer for Raw8FilesWriter {
    fn start(mut image_rx: BusReader<Arc<Image>>, filename: String) -> Self {
        let (stop_tx, stop_rx) = channel::<()>();

        thread::spawn(move || {
            create_dir(&filename).unwrap();

            let mut i = 0;
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let mut file = File::create(format!("{}/{:06}.raw8", &filename, i)).unwrap();
                let img = image_rx.recv().unwrap();
                file.write_all(&img.data).unwrap();

                i += 1;
            }
        });

        Self { stop_channel: stop_tx }
    }

    fn stop(&self) { self.stop_channel.send(()).unwrap(); }
}


/// A writer, that writes cinemaDNG (a folder with DNG files)
pub struct CinemaDngWriter {
    stop_channel: Sender<()>,
}

impl Writer for CinemaDngWriter {
    fn start(mut image_rx: BusReader<Arc<Image>>, filename: String) -> Self {
        let (stop_tx, stop_rx) = channel::<()>();

        thread::spawn(move || {
            create_dir(&filename).unwrap();

            let mut i = 0;
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let mut file = File::create(format!("{}/{:06}.dng", &filename, i)).unwrap();
                let img = image_rx.recv().unwrap();
                file.write_all(&img.format_dng()).unwrap();

                i += 1;
            }
        });


        Self { stop_channel: stop_tx }
    }

    fn stop(&self) { self.stop_channel.send(()).unwrap(); }
}

pub struct MpegWriter {
    stop_channel: Sender<()>,
}

impl Writer for MpegWriter {
    fn start(mut image_rx: BusReader<Arc<Image>>, filename: String) -> Self {
        let (stop_tx, stop_rx) = channel::<()>();

        thread::spawn(move || {
            let img = image_rx.recv().unwrap();
            let mut encoder = Encoder::new(filename, img.width as usize, img.height as usize);

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let img = image_rx.recv().unwrap();
                encoder.encode_rgba(
                    (img.width / 2) as usize,
                    (img.height / 2) as usize,
                    img.debayer().unwrap().data.as_ref(),
                    false,
                );
            }
        });


        Self { stop_channel: stop_tx }
    }

    fn stop(&self) { self.stop_channel.send(()).unwrap(); }
}
