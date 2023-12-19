pub mod parser;
pub mod reader;

use std::path::{Path, PathBuf};

use libdeflater::DecompressionError;
use parser::cbcl::ILLUMINA_MIN_QUAL;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BclError {
    #[error("Error parsing BCL")]
    ParseError {
        msg: &'static str,
        code: nom::error::ErrorKind,
    },
    #[error("I/O error")]
    IoError(#[from] std::io::Error),
    #[error("Unexpected EOF")]
    EofError,
    #[error("Decompression error")]
    DecompressError(#[from] DecompressionError),
    #[error("Decompressed basecalls did not match expected size")]
    DecompSizeMismatch,
    #[error("Compressed block size {got} did not match expected size {expected}")]
    CompSizeMismatch { expected: u32, got: usize },
}

impl<'a> From<nom::Err<nom::error::Error<&[u8]>>> for BclError {
    fn from(value: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        match value {
            nom::Err::Failure(nom::error::Error { input: _, code }) => BclError::ParseError {
                msg: "Failed parsing BCL, error code {code}",
                code,
            },
            nom::Err::Error(nom::error::Error { input: _, code }) => BclError::ParseError {
                msg: "Failed Parsing BCL, error code {code}",
                code,
            },
            nom::Err::Incomplete(_) => BclError::ParseError {
                msg: "Needed more bytes to parse BCL. File is most likely truncated.",
                code: nom::error::ErrorKind::Fail,
            },
        }
    }
}

#[derive(Debug)]
pub struct BclTile {
    bases: Vec<u8>,
    quals: Vec<u8>,
}

impl BclTile {
    pub fn with_capacity(cap: usize) -> Self {
        BclTile {
            bases: vec![0; cap],
            quals: vec![0; cap],
        }
    }
    pub fn get_bases(&self) -> &[u8] {
        &self.bases
    }

    pub fn get_quals(&self) -> &[u8] {
        &self.quals
    }

    pub fn bases_mut(&mut self) -> &mut [u8] {
        &mut self.bases
    }

    pub fn quals_mut(&mut self) -> &mut [u8] {
        &mut self.quals
    }
}

#[derive(Debug, Default)]
pub struct CBclHeader {
    version: u16,
    size: u32,
    bits_per_bc: u8,
    bits_per_qs: u8,
    n_bins: u32,
    bins: Vec<u8>,
    n_tiles: u32,
}

#[derive(Debug)]
pub struct TileData {
    tile_num: u32,
    num_clusters: u32,
    block_size_un: u32,
    block_size_comp: u32,
    pf_excluded: bool,
    filter: Option<&'static [u8]>,
}

impl TileData {
    pub fn has_filter(&self) -> bool {
        self.filter.is_some()
    }

    pub fn get_or_read_filter(&self) -> Option<&'static [u8]> {
        todo!()
    }
}

pub fn bin_base_calls(calls: &mut [u8], bins: &mut [u8]) {
    calls
        .iter_mut()
        .for_each(|x| *x = bins[usize::from(*x >> 2)])
}

pub fn into_bin_lookup(raw_bins: Option<Vec<(u32, u32)>>) -> Vec<u8> {
    if let Some(raw_bins) = raw_bins {
        let mut bins = raw_bins.iter().map(|b| b.1 as u8).collect::<Vec<u8>>();
        bins[0] = ILLUMINA_MIN_QUAL;
        bins
    } else {
        Vec::with_capacity(0)
    }
}
