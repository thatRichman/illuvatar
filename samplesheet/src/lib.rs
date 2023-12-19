#![allow(dead_code)]

use serde::Deserialize;
use std::{convert::Infallible, fmt::Display, num::ParseIntError, str::FromStr};
use thiserror::Error;

pub mod deserializers;
pub mod parser;
pub mod reader;

use self::deserializers::*;

const DEFAULT_ADAPTER_STRINGENCY: f32 = 0.9;
const DEFAULT_MASK_SHORT_READS: u16 = 22;

const fn default_adapter_stringency() -> f32 {
    DEFAULT_ADAPTER_STRINGENCY
}
const fn default_mask_short_reads() -> u16 {
    DEFAULT_MASK_SHORT_READS
}

#[derive(Debug, Deserialize, Default)]
pub enum CompressionFormat {
    #[default]
    #[serde(rename = "gzip")]
    Gzip,
    #[serde(rename = "dragen")]
    Dragen,
    #[serde(rename = "dragen-interleaved")]
    DragenInterleaved,
}

/// Mask => masks adapters with Ns
/// Trim => removes adapters
#[derive(Deserialize, Default, Debug, PartialEq)]
pub enum AdapterBehavior {
    #[serde(rename = "mask")]
    Mask,
    #[default]
    #[serde(rename = "trim")]
    Trim,
}

/// I => Index reads
/// Y => Sequencing reads
/// U => UMI reads,
/// N => trimmed reads
/// Only one Y or I sequence per read
#[derive(Debug, PartialEq, Eq)]
pub enum OverrideCycle {
    I(u8),
    Y(u8),
    U(u8),
    N(u8),
}

#[derive(Error, Debug)]
pub enum SampleSheetError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    DeserializeError(#[from] csv::Error),
    #[error("Unexpected end of file")]
    EofError,
    #[error("Error reading SampleSheet: {0}")]
    ParseError(String),
    #[error("Error reading OverrideCycles: unknown cycle type {0}")]
    UnknownCycleKind(char),
    #[error("Error reading OverrideCycles: could not parse cycle count as u8")]
    ParseIntError(#[from] ParseIntError),
    #[error("Samplesheet section format incorrect: exepected {0} got {1}")]
    BadSectionFormat(&'static str, &'static str),
    #[error("Samplesheet missing required section {0}")]
    MissingSection(&'static str),
}

impl FromStr for OverrideCycle {
    type Err = SampleSheetError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parser::override_cycle(s) {
            Ok((_, cyc)) => Ok(cyc),
            Err(e) => Err(SampleSheetError::ParseError(format!(
                "Failed to parse {s} as OverrideCycle: {e}"
            ))),
        }
    }
}

impl TryFrom<(char, u8)> for OverrideCycle {
    type Error = SampleSheetError;
    fn try_from(value: (char, u8)) -> Result<Self, Self::Error> {
        match value.0 {
            'I' => Ok(OverrideCycle::I(value.1)),
            'Y' => Ok(OverrideCycle::Y(value.1)),
            'U' => Ok(OverrideCycle::U(value.1)),
            'N' => Ok(OverrideCycle::N(value.1)),
            otherwise => Err(SampleSheetError::UnknownCycleKind(otherwise)),
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct OverrideCycles {
    cycles: Vec<Vec<OverrideCycle>>,
}

impl FromStr for OverrideCycles {
    type Err = SampleSheetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parser::override_cycles(s) {
            Ok((_, cycles)) => {
                validate_cycles(&cycles)?;
                Ok(OverrideCycles { cycles })
            }
            Err(e) => Err(SampleSheetError::ParseError(format!(
                "Unable to parse {s}: {e}"
            ))),
        }
    }
}

// each read can contain only one Y or I sequence
fn validate_cycles(cycles: &Vec<Vec<OverrideCycle>>) -> Result<(), SampleSheetError> {
    match cycles
        .iter()
        .map(|s| {
            s.iter()
                .filter(|c| match **c {
                    OverrideCycle::I(_) => true,
                    OverrideCycle::Y(_) => true,
                    _ => false,
                })
                .count()
                .eq(&1)
        })
        .all(|r| r == true)
    {
        true => Ok(()),
        false => Err(SampleSheetError::ParseError(String::from(
            "each read can contain only one Y or I sequence",
        ))),
    }
}

#[repr(u8)]
#[derive(Deserialize, Default, Debug)]
pub enum SampleSheetVersion {
    #[default]
    V1 = 1,
    V2 = 2,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SampleSheetSection {
    Header(SectionType),
    Reads(SectionType),
    Settings(SectionType),
    Data(SectionType),
    BCLConvertSettings(SectionType),
    BCLConvertData(SectionType),
    Other(SectionType),
}

impl SampleSheetSection {
    fn get_kind(&self) -> &SectionType {
        match self {
            Self::Header(kind) => &kind,
            Self::Reads(kind) => &kind,
            Self::Settings(kind) => &kind,
            Self::Data(kind) => &kind,
            Self::BCLConvertSettings(kind) => &kind,
            Self::BCLConvertData(kind) => &kind,
            Self::Other(kind) => &kind,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SectionType {
    Standalone,
    CSV,
    Unknown(String),
}

impl Display for SectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standalone => write!(f, "Standalone"),
            Self::CSV => write!(f, "CSV"),
            Self::Unknown(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for SampleSheetSection {
    type Err = Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Header" => Ok(Self::Header(SectionType::Standalone)),
            "Settings" => Ok(Self::Settings(SectionType::Standalone)),
            "Data" => Ok(Self::Data(SectionType::CSV)),
            "Reads" => Ok(Self::Reads(SectionType::Standalone)),
            "BCLConvert_Settings" => Ok(Self::BCLConvertSettings(SectionType::Standalone)),
            "BCLConvert_Data" => Ok(Self::BCLConvertData(SectionType::CSV)),
            s => Ok(Self::Other(SectionType::Unknown(s.to_string()))),
        }
    }
}

impl Display for SampleSheetSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Header(kind) => format!("Header <{kind}>"),
            Self::Settings(kind) => format!("Settings <{kind}>"),
            Self::Data(kind) => format!("Data <{kind}>"),
            Self::Reads(kind) => format!("Reads <{kind}>"),
            Self::BCLConvertData(kind) => format!("BCLConvert_Data <{kind}?"),
            Self::BCLConvertSettings(kind) => format!("BCLConvert_Settings <{kind}?"),
            Self::Other(name) => format!("{name} <Unknown>"),
        };
        write!(f, "{}", s)
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SampleSheetHeader {
    #[serde(deserialize_with="samplesheet_version_from_int")]
    file_format_version: SampleSheetVersion,
    run_name: Option<String>,
    instrument_platform: Option<String>,
    instrument_type: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SampleSheetReads {
    read_1_cycles: u16,
    read_2_cycles: u16,
    index_1_cycles: Option<u16>,
    index_2_cycles: Option<u16>,
}

#[repr(u8)]
#[derive(Debug, Deserialize, Default)]
pub enum MinAdapterOlap {
    #[default]
    One = 1,
    Two = 2,
    Three = 3,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SampleSheetSettings {
    software_version: Option<String>,
    #[serde(deserialize_with = "callback_opt")]
    override_cycles: Option<OverrideCycles>,
    #[serde(deserialize_with = "vec_plus_sign_sep", default = "Vec::new")]
    adapter_read_1: Vec<String>,
    #[serde(deserialize_with = "vec_plus_sign_sep", default = "Vec::new")]
    adapter_read_2: Vec<String>,
    #[serde(default = "AdapterBehavior::default")]
    adapter_behavior: AdapterBehavior,
    #[serde(default = "default_adapter_stringency")]
    adapter_stringency: f32,
    #[serde(default = "MinAdapterOlap::default")]
    minimum_adapter_overlap: MinAdapterOlap,
    #[serde(default = "default_bool::<false>", deserialize_with = "bool_from_int")]
    create_fastq_for_index_reads: bool,
    #[serde(default = "default_bool::<true>", deserialize_with = "bool_from_int")]
    trim_umi: bool,
    #[serde(default = "default_mask_short_reads")]
    mask_short_reads: u16,
    #[serde(default = "default_bool::<false>")]
    no_lane_splitting: bool,
    #[serde(default = "CompressionFormat::default")]
    fastq_compression_format: CompressionFormat,
}

#[derive(Deserialize, Debug, Default)]
pub struct SampleSheetData {
    #[serde(rename = "Lane")]
    lane: u8,
    #[serde(rename = "Sample_ID")]
    sample_id: String,
    #[serde(rename = "index")]
    index: String,
    #[serde(rename = "index2")]
    index_2: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct SampleSheet {
    header: SampleSheetHeader,
    reads: SampleSheetReads,
    settings: SampleSheetSettings,
    data: Vec<SampleSheetData>,
}

impl SampleSheet {
    pub fn version(&self) -> &SampleSheetVersion {
        &self.header.file_format_version
    }
}
