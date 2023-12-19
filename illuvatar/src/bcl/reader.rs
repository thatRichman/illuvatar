use libdeflater::Decompressor;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use samplesheet::SampleSheetSettings;

use super::{into_bin_lookup, parser, BclError, BclTile, CBclHeader, TileData};

pub const DEFAULT_BCL_READER_CAPACITY: usize = 1_000_000;
pub const PREHEADER_SIZE: u32 = 6;
pub const FILTER_HEADER_SIZE: usize = 12;

pub enum CbclReaderState {
    Header,
    Tile,
    Complete,
}

pub struct CBclReader<R>
where
    R: BufRead,
{
    inner: R,
    buffer: Vec<u8>,
    decomp_buffer: Vec<u8>,
    header: CBclHeader,
    tile_cache: Vec<TileData>,
    decomp: Decompressor,
    state: CbclReaderState,
    n_read: u32,
}

impl CBclReader<BufReader<File>> {
    pub fn new<P: AsRef<Path>>(cycle_info: P) -> Result<Self, BclError> {
        let inner = BufReader::new(File::open(cycle_info)?);
        Ok(CBclReader {
            inner,
            buffer: Vec::with_capacity(DEFAULT_BCL_READER_CAPACITY),
            decomp_buffer: Vec::new(),
            header: CBclHeader::default(),
            tile_cache: Vec::new(),
            decomp: Decompressor::new(),
            state: CbclReaderState::Header,
            n_read: 0,
        })
    }

    pub fn with_capacity<P: AsRef<Path>>(cycle_info: P, cap: usize) -> Result<Self, BclError> {
        let inner = BufReader::new(File::open(cycle_info)?);
        Ok(CBclReader {
            inner,
            buffer: Vec::with_capacity(cap),
            header: CBclHeader::default(),
            tile_cache: Vec::new(),
            decomp: Decompressor::new(),
            decomp_buffer: Vec::new(),
            state: CbclReaderState::Header,
            n_read: 0,
        })
    }

    /// Reset the reader, providing a new file to read from
    /// This clears but does not reallocate buffers.
    pub fn reset_with<P: AsRef<Path>>(
        &mut self,
        cycle_info: P,
        clear_tile_cache: bool,
    ) -> Result<(), BclError> {
        let inner = BufReader::new(File::open(cycle_info)?);
        self.buffer.clear();
        self.decomp_buffer.clear();
        self.n_read = 0;
        self.inner = inner;
        self.header = CBclHeader::default();
        if clear_tile_cache {
            self.tile_cache.clear();
        }
        self.state = CbclReaderState::Header;
        Ok(())
    }

    pub fn shrink_buffer(&mut self, to: usize) {
        self.buffer.shrink_to(to);
    }

    pub fn shrink_decomp_buff(&mut self, to: usize) {
        self.decomp_buffer.shrink_to(to)
    }

    pub fn read_tile(&mut self) -> Option<Result<BclTile, BclError>> {
        if self.n_read == self.header.n_tiles {
            return None;
        }
        let tile_data = &self.tile_cache[self.n_read as usize];
        match (&mut self.inner)
            .take(u64::from(tile_data.block_size_comp))
            .read_to_end(&mut self.buffer)
        {
            Ok(v) if v == tile_data.block_size_comp as usize => {}
            Ok(v) => {
                return Some(Err(BclError::CompSizeMismatch {
                    expected: tile_data.block_size_comp,
                    got: v,
                }));
            }
            Err(e) => return Some(Err(BclError::from(e))),
        }
        if (self.decomp_buffer.len() as u32) < tile_data.block_size_un {
            self.decomp_buffer
                .resize(tile_data.block_size_un as usize, 0);
        }
        match self.decomp.gzip_decompress(
            &mut self.buffer.as_slice(),
            &mut self.decomp_buffer.as_mut_slice(),
        ) {
            Ok(v) if (v as u32) == tile_data.block_size_un => {}
            Ok(_) => return Some(Err(BclError::DecompSizeMismatch)),
            Err(e) => return Some(Err(BclError::from(e))),
        }
        self.buffer.clear();
        self.buffer.extend(
            self.decomp_buffer
                .iter()
                .flat_map(|x| [x & 0x0f, (x >> 4) & 0x0f]), // nibbles to bytes
        );
        // multiply by two to account for the nibble explosion
        let mut tile = BclTile::with_capacity((tile_data.block_size_un * 2u32) as usize);
        match parser::cbcl::parse_base_calls(&self.buffer, &mut tile, &self.header.bins) {
            Ok(_) => {}
            Err(e) => {
                return Some(Err(BclError::from(e)));
            }
        };

        if !tile_data.pf_excluded && tile_data.has_filter() {
            match filter_reads(&mut tile, tile_data.get_or_read_filter().as_ref().unwrap()) {
                Ok(_) => {}
                Err(e) => return Some(Err(BclError::from(e))),
            }
        }

        self.n_read += 1;
        self.buffer.clear();
        self.decomp_buffer.clear();
        Some(Ok(tile))
    }
}

impl Iterator for CBclReader<BufReader<File>> {
    type Item = Result<BclTile, BclError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            CbclReaderState::Tile => match self.read_tile() {
                Some(x) => Some(x),
                None => {
                    self.state = CbclReaderState::Complete;
                    None
                }
            },
            CbclReaderState::Header => {
                match read_header(
                    &mut self.inner,
                    &mut self.buffer,
                    &mut self.header,
                    &mut self.tile_cache,
                ) {
                    Ok(_) => self.state = CbclReaderState::Tile,
                    Err(e) => return Some(Err(e)),
                }
                self.next()
            }
            CbclReaderState::Complete => None,
        }
    }
}

// We put this here to satisfy the borrow checker
/// Read Cbcl header, including tile metadata entries
fn read_header<'a, T>(
    mut from: T,
    to: &mut Vec<u8>,
    header: &mut CBclHeader,
    tile_cache: &mut Vec<TileData>,
) -> Result<(), BclError>
where
    T: BufRead + Read,
{
    match (&mut from).take(u64::from(PREHEADER_SIZE)).read_to_end(to) {
        Ok(x) if x == PREHEADER_SIZE as usize => {}
        Ok(_) => {
            return Err(BclError::EofError);
        }
        Err(e) => return Err(BclError::from(e)),
    }
    let (version, h_size) = match parser::cbcl::cbcl_version_and_size(to) {
        Ok((_, (version, h_size))) => (version, h_size),
        Err(e) => return Err(BclError::from(e)),
    };
    to.clear();
    match from
        .take(u64::from(h_size - PREHEADER_SIZE))
        .read_to_end(to)
    {
        Ok(amt) if amt as u32 == h_size - PREHEADER_SIZE => {}
        Ok(_) => return Err(BclError::EofError),
        Err(e) => return Err(BclError::from(e)),
    }
    match parser::cbcl::cbcl_header(to) {
        Ok((_, (bits_per_bc, bits_per_qs, n_bins, bins, n_tiles, tile_data, pf_excluded))) => {
            *header = CBclHeader {
                version,
                size: h_size,
                bits_per_bc,
                bits_per_qs,
                n_bins,
                bins: into_bin_lookup(bins),
                n_tiles,
            };
            tile_cache.extend(tile_data.iter().map(
                |(tile_num, num_clusters, block_size_un, block_size_comp)| TileData {
                    tile_num: *tile_num,
                    num_clusters: *num_clusters,
                    block_size_un: *block_size_un,
                    block_size_comp: *block_size_comp,
                    pf_excluded: pf_excluded == 1,
                    filter: get_filter(*tile_num),
                },
            ));
        }
        Err(e) => return Err(BclError::from(e)),
    };
    to.clear();
    Ok(())
}

struct FilterFileReader<T>
where
    T: BufRead,
{
    inner: T,
    buffer: Vec<u8>,
}

impl FilterFileReader<BufReader<File>> {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, BclError> {
        let inner = BufReader::new(File::open(path)?);
        Ok(FilterFileReader {
            inner,
            buffer: Vec::new(),
        })
    }

    pub fn read_filter(&mut self) -> Result<Vec<u8>, BclError> {
        match self.inner.read_to_end(&mut self.buffer) {
            Ok(x) if x >= FILTER_HEADER_SIZE => {}
            Ok(_) => return Err(BclError::EofError),
            Err(e) => return Err(BclError::from(e)),
        }
        let (i, (_, num_clusters)) = parser::filter::filter_header(&self.buffer)?;
        match num_clusters {
            x if x == i.len() as u32 => {}
            _ => return Err(BclError::EofError),
        }
        let mut filter = vec![0; num_clusters as usize];
        parser::filter::filter_file(i, filter.as_mut_slice())?;
        Ok(filter)
    }
}

// OPTIMIZE -> reallocation may actually be faster?
// https://github.com/rust-lang/rust/issues/91497
// I can't tell if the resulting PR was actually merged, need to manually bench
/// Read filter associated with a cycle, remove any indices that do not pass
/// i.e. == 0
fn filter_reads(tile: &mut BclTile, filter: &[u8]) -> Result<(), BclError> {
    //let filter = FilterFileReader::new(filter_path)?.read_filter()?;
    tile.bases.retain(|_| filter.iter().next().unwrap() == &1);
    tile.quals.retain(|_| filter.iter().next().unwrap() == &1);
    Ok(())
}

fn get_filter(tile_num: u32) -> Option<&'static [u8]> {
    todo!()
}

fn resolve_tile(tile: &BclTile, tile_meta: &TileData, settings: &SampleSheetSettings) {}
