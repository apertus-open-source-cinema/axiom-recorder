use crate::{
    nodes_io::writer_cinema_dng::frame_to_dng_ifd,
    pipeline_processing::{
        node::{InputProcessingNode, NodeID, ProgressUpdate, Request, SinkNode},
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
    },
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use dav_server::{
    davpath::DavPath,
    fs::{
        BoxCloneMd,
        DavDirEntry,
        DavFile,
        DavFileSystem,
        DavMetaData,
        FsError,
        FsFuture,
        FsResult,
        FsStream,
        OpenOptions,
        ReadDirMeta,
    },
    DavHandler,
};
use derivative::Derivative;
use dng::{ifd::Ifd, yaml::IfdYamlParser, DngWriter, FileType};
use futures::{future, FutureExt};
use hyper::{
    body::{Buf, Bytes},
    service::{make_service_fn, service_fn},
    Server,
};
use std::{
    convert::Infallible,
    fmt::Debug,
    fs,
    future::Future,
    io::{Cursor, SeekFrom},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::SystemTime,
};
use zstd::zstd_safe::WriteBuf;


/// A sink that exposes its input as a WebDAV server with CinemaDNG files
pub struct CinemaDngFrameserver {
    input: InputProcessingNode,
    priority: u8,
    address: SocketAddr,
    base_ifd: Ifd,
}

impl Parameterizable for CinemaDngFrameserver {
    const DESCRIPTION: Option<&'static str> = Some("writes Cinema DNG files into a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("priority", WithDefault(U8(), ParameterValue::IntRangeValue(0)))
            .with("host", WithDefault(StringParameter, StringValue("127.0.0.1".to_string())))
            .with("port", Optional(IntRange(0, u16::MAX as i64)))
            .with("dcp-yaml", Optional(StringParameter))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let port = if let Some(port) = parameters.take_option::<u64>("port")? {
            port as u16
        } else {
            portpicker::pick_unused_port().unwrap()
        };

        let host = parameters.take::<String>("host")?;
        let address = SocketAddr::from((IpAddr::from_str(&host)?, port));

        let mut base_ifd =
            IfdYamlParser::default().parse_from_str(include_str!("./base_ifd.yml"))?;

        let dcp_ifd = if let Some(path) = parameters.take_option::<String>("dcp-yaml")? {
            let path = PathBuf::from_str(&path)?;
            let data = fs::read_to_string(path.clone()).context("couldnt read dcp-yaml file")?;
            IfdYamlParser::new(path).parse_from_str(&data).context("couldnt parse dcp-yaml file")?
        } else {
            IfdYamlParser::default().parse_from_str(include_str!("./default_dcp.yml"))?
        };

        base_ifd.insert_from_other(dcp_ifd);

        Ok(Self {
            input: parameters.take("input")?,
            priority: parameters.take("priority")?,
            address,
            base_ifd,
        })
    }
}

#[async_trait]
impl SinkNode for CinemaDngFrameserver {
    async fn run(
        &self,
        context: &ProcessingContext,
        _progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let context = context.clone();
        let base_ifd = self.base_ifd.clone();
        let priority = self.priority;
        let frame_count = self.input.get_caps().frame_count.unwrap();
        let input = self.input.clone_for_same_puller();

        let pull = move |i| {
            let context = context.clone();
            let base_ifd = base_ifd.clone();
            let input = input.clone_for_same_puller();
            async move {
                let payload = input.pull(Request::new(priority, i)).await?;

                let frame = context
                    .ensure_cpu_buffer_frame(&payload)
                    .context("Wrong input format for CinemaDng")?;

                let ifd = frame_to_dng_ifd(frame, base_ifd)?;

                let mut buffer = Cursor::new(Vec::new());
                DngWriter::write_dng(&mut buffer, true, FileType::Dng, vec![ifd])?;
                Ok::<_, anyhow::Error>(Bytes::from(buffer.into_inner()))
            }
        };
        let fs = CDngFs::new(frame_count, pull).await;

        let dav_server = DavHandler::builder().filesystem(Box::new(fs) as _).build_handler();

        let service = make_service_fn(|_| {
            let dav_server = dav_server.clone();
            async move {
                let func = move |req| {
                    let dav_server = dav_server.clone();
                    async move { Ok::<_, Infallible>(dav_server.clone().handle(req).await) }
                };
                Ok::<_, hyper::Error>(service_fn(func))
            }
        });
        let server = Server::bind(&self.address).serve(service);
        println!("Listening on http://{}", self.address);

        server.await?;


        Ok::<(), anyhow::Error>(())
    }
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
struct CDngFs<E, R: Future<Output = Result<Bytes, E>>, F: Fn(u64) -> R + Send + 'static> {
    number_of_frames: u64,
    guessed_file_size: u64,
    pull: Arc<F>,
}

impl<E: Debug, R: Future<Output = Result<Bytes, E>>, F: Fn(u64) -> R + Send + 'static>
    CDngFs<E, R, F>
{
    async fn new(number_of_frames: u64, pull: F) -> Self {
        let guessed_file_size = pull(0).await.unwrap().len() as u64;
        let pull = Arc::new(pull);
        Self { number_of_frames, guessed_file_size, pull }
    }
}

impl<
        E: Debug + 'static,
        R: Future<Output = Result<Bytes, E>> + Send + 'static,
        F: Fn(u64) -> R + Send + Sync + 'static,
    > DavFileSystem for CDngFs<E, R, F>
{
    fn open<'a>(&'a self, path: &'a DavPath, _options: OpenOptions) -> FsFuture<Box<dyn DavFile>> {
        async move {
            if let Some(i) = get_framenumber_from_path(path) {
                if let Ok(buffer) = (self.pull)(i).await {
                    Ok(Box::new(CDngFsFile { buffer, read_ptr: 0 }) as _)
                } else {
                    Err(FsError::GeneralFailure)
                }
            } else {
                Err(FsError::NotFound)
            }
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<FsStream<Box<dyn DavDirEntry>>> {
        let len = self.guessed_file_size;
        async move {
            if path == &DavPath::new("/").unwrap() {
                let iter = (0..self.number_of_frames).map(move |i| {
                    Box::new(CDngFsDirEntry {
                        meta: CDngFsMetaData { len, is_dir: false },
                        name: format!("{i:06}.dng"),
                    }) as Box<dyn DavDirEntry>
                });
                let strm = futures_util::stream::iter(iter);
                Ok(Box::pin(strm) as FsStream<Box<dyn DavDirEntry>>)
            } else {
                Err(FsError::NotFound)
            }
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<Box<dyn DavMetaData>> {
        async move {
            if path == &DavPath::new("/").unwrap() {
                Ok(Box::new(CDngFsMetaData { len: self.number_of_frames, is_dir: true }) as _)
            } else if let Some(_i) = get_framenumber_from_path(path) {
                Ok(Box::new(CDngFsMetaData { len: self.guessed_file_size, is_dir: false }) as _)
            } else {
                Err(FsError::NotFound)
            }
        }
        .boxed()
    }
}

fn get_framenumber_from_path(path: &DavPath) -> Option<u64> {
    let pathbuf = path.as_pathbuf();
    pathbuf.file_stem().and_then(|x| x.to_str()).and_then(|x| x.parse().ok())
}

#[derive(Debug)]
struct CDngFsFile {
    buffer: Bytes,
    read_ptr: u64,
}
impl DavFile for CDngFsFile {
    fn metadata(&mut self) -> FsFuture<Box<dyn DavMetaData>> {
        async move { Ok(Box::new(CDngFsMetaData {
            len: self.buffer.len() as u64,
            is_dir: false,
        }) as _) }.boxed()
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<Bytes> {
        async move {
            let as_slice = self.buffer.as_slice();
            let subset = &as_slice[self.read_ptr as usize..self.read_ptr as usize + count];
            self.read_ptr += count as u64;
            Ok(self.buffer.slice_ref(&subset))
        }
        .boxed()
    }
    fn seek(&mut self, pos: SeekFrom) -> FsFuture<u64> {
        async move {
            match pos {
                SeekFrom::Start(x) => self.read_ptr = x,
                SeekFrom::End(x) => self.read_ptr = (self.buffer.len() as i64 - x) as u64,
                SeekFrom::Current(x) => self.read_ptr = (self.read_ptr as i64 + x) as u64,
            }
            Ok(self.read_ptr)
        }
        .boxed()
    }

    fn write_buf(&mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<()> {
        async move { Err(FsError::NotImplemented) }.boxed()
    }
    fn write_bytes(&mut self, _buf: Bytes) -> FsFuture<()> {
        async move { Err(FsError::NotImplemented) }.boxed()
    }
    fn flush(&mut self) -> FsFuture<()> { async move { Err(FsError::NotImplemented) }.boxed() }
}

#[derive(Clone, Debug)]
struct CDngFsMetaData {
    len: u64,
    is_dir: bool,
}
impl DavMetaData for CDngFsMetaData {
    fn len(&self) -> u64 { self.len }
    fn modified(&self) -> FsResult<SystemTime> { Ok(SystemTime::now()) }
    fn is_dir(&self) -> bool { self.is_dir }
}

#[derive(Clone, Debug)]
struct CDngFsDirEntry {
    meta: CDngFsMetaData,
    name: String,
}
impl DavDirEntry for CDngFsDirEntry {
    fn name(&self) -> Vec<u8> { self.name.clone().into_bytes() }
    fn metadata(&self) -> FsFuture<Box<dyn DavMetaData>> {
        Box::pin(future::ok(self.meta.box_clone()))
    }
}
