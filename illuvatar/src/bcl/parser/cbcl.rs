#![allow(dead_code)]

use nom::{
    combinator::{all_consuming, map, opt},
    multi::{count, fill},
    number::complete::{le_u16, le_u32, le_u8, u8},
    sequence::{pair, preceded, tuple},
    IResult,
};

use crate::bcl::BclTile;

pub(crate) const ILLUMINA_MIN_QUAL: u8 = 2;
const NO_CALL: u8 = b'N';
const BASES: [u8; 4] = [b'A', b'C', b'G', b'T'];
const BASE_MASK: u8 = 0x03;

const BASE_LOOKUP: [u8; 256] = calculate_base_lookup();
const QUAL_LOOKUP: [u8; 256] = calculate_qual_lookup();

const fn calculate_base_lookup() -> [u8; 256] {
    let mut base_lookup = [0; 256];
    base_lookup[0] = NO_CALL;
    let mut i = 1u8;
    while i < 255u8 {
        base_lookup[i as usize] = BASES[(i & BASE_MASK) as usize];
        i += 1;
    }
    base_lookup
}

const fn calculate_qual_lookup() -> [u8; 256] {
    let mut qual_lookup = [0; 256];
    qual_lookup[0] = ILLUMINA_MIN_QUAL;
    let mut i = 1u8;
    while i < 255u8 {
        qual_lookup[i as usize] =
            [ILLUMINA_MIN_QUAL, i >> 2][(ILLUMINA_MIN_QUAL < (i >> 2)) as usize];
        i += 1;
    }
    qual_lookup
}

fn num_clusters(input: &[u8]) -> IResult<&[u8], u8> {
    le_u8(input)
}

pub(crate) fn parse_base_calls<'a>(
    input: &'a [u8],
    tile: &mut BclTile,
    bins: &Vec<u8>,
) -> IResult<&'a [u8], ()> {
    fill(bcl_base, tile.bases_mut())(input)?;
    // TODO convert this into a nom parser
    if bins.len() > 0 {
        Ok((
            &input[tile.quals.len()..],
            tile.quals = input[0..tile.quals.len()]
                .iter()
                .map(|x| bins[usize::from(x >> 2)])
                .collect::<Vec<u8>>(),
        ))
    } else {
        fill(bcl_qual, tile.quals_mut())(input)
    }
}

fn bcl_base(input: &[u8]) -> IResult<&[u8], u8> {
    map(le_u8, |x| BASE_LOOKUP[usize::from(x)])(input)
}

fn bcl_qual(input: &[u8]) -> IResult<&[u8], u8> {
    map(le_u8, |x| QUAL_LOOKUP[usize::from(x)])(input)
}

/// Version and header size
/// We read this first so we can read the entire
/// rest of the header in one go.
pub(crate) fn cbcl_version_and_size(input: &[u8]) -> IResult<&[u8], (u16, u32)> {
    pair(le_u16, le_u32)(input)
}

pub(crate) fn cbcl_header(
    input: &[u8],
) -> IResult<
    &[u8],
    (
        u8,                        // bits per basecall
        u8,                        // bits per qual
        u32,                       // number of bins
        Option<Vec<(u32, u32)>>,   // qual bin pairs
        u32,                       // number of tiles
        Vec<(u32, u32, u32, u32)>, // tile data
        u8,                        // non-PF excluded
    ),
> {
    let (i, (bits_per_base, bits_per_qual, num_bins)) = tuple((le_u8, le_u8, le_u32))(input)?;
    let (i, (bins, num_tiles)) =
        pair(opt(count(pair(le_u32, le_u32), num_bins as usize)), le_u32)(i)?;
    let (i, (tile_data, pf_excluded)) = pair(count(cbcl_tile_data, num_tiles as usize), u8)(i)?;

    Ok((
        i,
        (
            bits_per_base,
            bits_per_qual,
            num_bins,
            bins,
            num_tiles,
            tile_data,
            pf_excluded,
        ),
    ))
}

/// 16 bytes each
pub(crate) fn cbcl_tile_data(input: &[u8]) -> IResult<&[u8], (u32, u32, u32, u32)> {
    tuple((
        le_u32, // tile number (0-3)
        le_u32, // number of clusters (4-7)
        le_u32, // uncompressed block size (8-11)
        le_u32, // compressed block size (12-15)
    ))(input)
}
