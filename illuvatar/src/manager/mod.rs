use std::{
    fs::File,
    io::BufReader,
    thread::{self},
    time::Duration,
};

pub mod reader;
pub mod writer;

use crossbeam::channel::{bounded, Receiver, Sender};
use log::debug;
use rayon::prelude::*;

use crate::{
    bcl::{reader::CBclReader, DemuxUnit},
    manager::writer::WriteRecord,
    IlluvatarError,
};

use samplesheet::SampleSheetSettings;

type FileReader = CBclReader<BufReader<File>>;

pub(crate) struct DemuxManager {
    demux_pool: rayon::ThreadPool,
    readers: Vec<FileReader>,
    demux_recv: Receiver<DemuxUnit>,
}

impl DemuxManager {
    pub fn new(
        num_threads: usize,
        demux_cap: usize,
        settings: &SampleSheetSettings,
    ) -> Result<(DemuxManager, Sender<DemuxUnit>), IlluvatarError> {
        // This channel holds WorkUnits
        let (demux_send, demux_recv) = bounded(demux_cap);

        // DemuxUnits are sent to this pool
        // We use a rayon threadpool because each DemuxUnit
        // should be (relatively) short lived and is highly parallelizable
        let demux_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("illuv-demux-worker-{i}"))
            .build()?;

        Ok((
            DemuxManager {
                demux_pool,
                readers: vec![],
                demux_recv,
            },
            demux_send,
        ))
    }

    pub fn resolve(&self, write_sender: Sender<WriteRecord>) {
        // spin up the resolver
        let recv_iter = self.demux_recv.iter();
        // we create a parallel iterator over the demux_recv channel
        // and make it immediately return on panic because there is no
        // recovering from a failed demux attempt.
        //
        // Each thread immediately sends the resulting WriteRecord to the write queue,
        // which is routed to the appropriate destination by the write router.
        // Threads block until send succeeds to propagate backpressure.

        // TODO resolve will eventually need to take settings from the samplesheet
        // we either will clone the samplesheet settings or pass specific values
        // as arguments, but cannot pass a reference
        self.demux_pool.install(move || {
            recv_iter.par_bridge().panic_fuse().for_each_with(
                write_sender,
                |sender: &mut Sender<WriteRecord>, demux_unit: DemuxUnit| {
                    sender
                        .send(resolve_tile(demux_unit))
                        .expect("failed to send demux result to write channel")
                },
            )
        });
        debug!("DONE RESOLVING");
    }
}

//// PLACEHOLDERS ////

fn resolve_tile(demux_unit: DemuxUnit) -> WriteRecord {
    return WriteRecord {
        reads: format!("reads for {}", demux_unit.tile_data.tile_num),
        id: format!("test_id_{}", demux_unit.tile_data.tile_num),
        qual: format!("qualities for {}", demux_unit.tile_data.tile_num),
        destination: String::from("S01-TOO-12plex-P1-rep1_R1"),
    };
}
