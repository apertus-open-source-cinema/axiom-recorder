use crate::video_io::{dng::Dng, Image};
use bus::BusReader;
use std::{
    fs::{create_dir, File},
    io::prelude::*,
    sync::{
        mpsc::{channel, Sender},
        Arc,
    },
    thread,
};

/// An image sink, that somehow stores the images it receives
trait Writer {
    fn start(image_rx: BusReader<Arc<Image>>, filename: String) -> Self;
    fn stop(&self);
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
