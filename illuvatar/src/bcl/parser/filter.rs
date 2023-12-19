use nom::{
    combinator::{all_consuming, map, opt},
    multi::{count, fill},
    number::complete::{le_u16, le_u32, le_u8, u8},
    sequence::{pair, preceded, tuple},
    IResult,
};

/// version and num clusters
pub(crate) fn filter_header(input: &[u8]) -> IResult<&[u8], (u32, u32)> {
    preceded(le_u32, pair(le_u32, le_u32))(input)
}

/// ones and zeros
/// 1 == pass filter, 0 == failed filter
pub(crate) fn filter_file<'a>(input: &'a [u8], buffer: &mut [u8]) -> IResult<&'a [u8], ()> {
    all_consuming(fill(le_u8, buffer))(input)
}
