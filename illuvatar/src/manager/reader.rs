use std::{fs::File, future::Future, io::BufReader, path::Path};

use crossbeam::channel::{unbounded, Receiver, RecvError, SendError, Sender};

use log::{debug, error};
use seqdir::lane::Bcl;
use thiserror::Error;
use tokio::runtime;

use crate::bcl::{reader::CBclReader, BclError, DemuxUnit};

#[derive(Debug, Error)]
pub enum ReadError {
    #[error(transparent)]
    BclError(#[from] BclError),
    #[error(transparent)]
    SendError(#[from] SendError<DemuxUnit>),
    #[error(transparent)]
    RecvError(#[from] RecvError),
    #[error("`init` has already been called on this reader")]
    AlreadyInitError,
    #[error("adapter has not been initialized")]
    NoReaderError,
    #[error("illuvatar does not support BCLs")]
    BclUnsupportedError,
}

pub trait RoutableRead {
    fn read(
        &mut self,
        receiver: Receiver<Bcl>,
        destination: Sender<DemuxUnit>,
    ) -> impl Future<Output = Result<(), ReadError>>;
}

#[derive(Debug)]
pub(crate) struct ReaderPool {
    runtime: runtime::Runtime,
    handles: Vec<tokio::task::JoinHandle<Result<(), ReadError>>>,
    pub receiver: Receiver<Bcl>,
    destination: Sender<DemuxUnit>,
}

impl ReaderPool {
    pub fn new(destination: Sender<DemuxUnit>) -> Result<(ReaderPool, Sender<Bcl>), ReadError> {
        let runtime = runtime::Builder::new_multi_thread()
            .thread_name("illuvatar-reader")
            .enable_all()
            .build()
            .unwrap();

        let (sender, receiver) = unbounded::<Bcl>();
        Ok((
            ReaderPool {
                runtime,
                handles: Vec::new(),
                receiver,
                destination,
            },
            sender,
        ))
    }

    pub fn read(&mut self, readers: u8) {
        for _ in 0..readers {
            let read_recv = self.receiver.clone();
            let dest = self.destination.clone();
            self.handles
                .push(self.runtime.spawn(async move {
                    CBclReaderAdapter::default().read(read_recv, dest).await
                }));
        }
        let mut finished = false;
        while !finished {
            finished = self.handles.iter().all(|h| h.is_finished());
        }
        debug!("reader pool is exiting");
    }
}

/// A simple wrapper around a CBCLReader that implements [RoutableRead]
///
/// This lets us spin up a reader thread without initializaing the reader itself 
#[derive(Default)]
struct CBclReaderAdapter {
    reader: Option<CBclReader<BufReader<File>>>,
}

impl CBclReaderAdapter {
    fn init<P: AsRef<Path>>(&mut self, value: P) -> Result<(), ReadError> {
        match self.reader {
            None => {
                self.reader = Some(CBclReader::new(value)?);
                Ok(())
            }
            Some(_) => Err(ReadError::AlreadyInitError),
        }
    }
}

impl RoutableRead for CBclReaderAdapter {
    async fn read(
        &mut self,
        receiver: Receiver<Bcl>,
        destination: Sender<DemuxUnit>,
    ) -> Result<(), ReadError> {
        // spin until we have a task to take
        match receiver.recv() {
            Ok(Bcl::CBcl(path)) => {
                self.init(path.as_path())?;
            }
            Ok(Bcl::Bcl(_)) => return Err(ReadError::BclUnsupportedError),
            Err(e) => return Err(e.into()),
        }

        let mut reader = self.reader.take().unwrap();
        // read the BCL we initialized with
        for demux_unit in &mut reader {
            destination.send(demux_unit?)?;
        }
        // read more BCLs until the sender is dropped
        while let Ok(Bcl::CBcl(bcl)) = receiver.recv() {
            reader.reset_with(bcl, false)?;
            for demux_unit in &mut reader {
                destination.send(demux_unit?)?;
            }
        }
        debug!("READER EXITING");
        Ok(())
    }
}
