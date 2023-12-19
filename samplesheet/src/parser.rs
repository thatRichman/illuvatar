//! This is a basic sectioned csv parser specifically meant
//! for parsing illumina samplesheets.

use std::str::FromStr;

use nom::{
    bytes::complete::{is_a, is_not, tag},
    character::complete::{line_ending, one_of, u8},
    combinator::map_res,
    combinator::opt,
    multi::many1,
    sequence::{delimited, pair, terminated},
    IResult,
};

use super::{OverrideCycle, SampleSheetError, SampleSheetSection};

fn trailing_commas(input: &str) -> IResult<&str, &str> {
    terminated(is_a(","), line_ending)(input)
}

/// match a line containing only "[<content>]"
/// returns <content>
fn section_header(input: &str) -> IResult<&str, &str> {
    delimited(tag("["), is_not("]"), terminated(tag("]"), trailing_commas))(input)
}

/// Parse a section header into SampleSheetSection and raw contents
pub fn parse_section(input: &str) -> IResult<&str, (SampleSheetSection, String), SampleSheetError> {
    let (i, header) = section_header(input).map_err(|e| {
        nom::Err::Error(SampleSheetError::ParseError(format!(
            "Failed to parse SampleSheet header: {e}"
        )))
    })?;
    let (i, raw_contents) = section_contents(i).map_err(|e| {
        nom::Err::Error(SampleSheetError::ParseError(format!(
            "Failed to parse SampleSheet section '{header}': {e}"
        )))
    })?;
    let section = SampleSheetSection::from_str(header).unwrap();
    Ok((i, (section, preprocess(&raw_contents))))
}

fn section_contents(input: &str) -> IResult<&str, &str> {
    is_not("[")(input)
}

/// Parse a single OverrideCycle
pub fn override_cycle(input: &str) -> IResult<&str, OverrideCycle> {
    map_res(pair(one_of("YIUN"), u8), |v| OverrideCycle::try_from(v))(input)
}

/// Parse a full OverrideCycles field.
/// Assumes paired-end sequencing.
pub fn override_cycles(input: &str) -> IResult<&str, Vec<Vec<OverrideCycle>>> {
    many1(terminated(many1(override_cycle), opt(tag(";"))))(input)
}

/// Remove trailing commas and empty lines in a section
fn preprocess(input: &str) -> String {
    input
        .lines()
        .map(|l| l.trim_end_matches(',').to_string() + "\n")
        .collect()
}

// Convert
// a,b
// c,d
// x,y
// into
// a,c,x
// b,d,y
pub fn transmute_kv(s: &str) -> String {
    let (k, v): (Vec<&str>, Vec<&str>) = s.lines().filter_map(|x| x.split_once(',')).unzip();
    k.join(",") + "\n" + &v.join(",")
}
