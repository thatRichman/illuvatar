use super::{
    parser, SampleSheet, SampleSheetData, SampleSheetError, SampleSheetHeader, SampleSheetReads,
    SampleSheetSection, SampleSheetSettings, SectionType,
};
use log::debug;
use std::{fs::File, io::Read, path::Path};

pub fn read_samplesheet<P: AsRef<Path>>(path: P) -> Result<SampleSheet, SampleSheetError> {
    let mut handle = File::open(path)?;
    let mut buf = String::new();
    handle.read_to_string(&mut buf)?;
    let mut slice = &buf[..];

    let mut header: Option<SampleSheetHeader> = None;
    let mut reads: Option<SampleSheetReads> = None;
    let mut data: Option<Vec<SampleSheetData>> = None;
    let mut settings: Option<SampleSheetSettings> = None;

    while slice.len() > 0 {
        match parser::parse_section(slice) {
            Ok((i, (section, mut raw_contents))) => {
                match section.get_kind() {
                    SectionType::Standalone => {
                        raw_contents = parser::transmute_kv(&raw_contents);
                    }
                    SectionType::Unknown(..) => {
                        debug!("Ignoring unknown section {}", section);
                        slice = i;
                        continue;
                    }
                    _ => {}
                }
                let mut csv_reader = into_reader(&raw_contents);
                match section {
                    SampleSheetSection::Header(..) => match csv_reader.deserialize().next() {
                        Some(v) => match v {
                            Ok(h) => header = Some(h),
                            Err(e) => return Err(SampleSheetError::from(e)),
                        },
                        None => return Err(SampleSheetError::EofError),
                    },
                    SampleSheetSection::Reads(..) => match csv_reader.deserialize().next() {
                        Some(v) => match v {
                            Ok(h) => reads = Some(h),
                            Err(e) => return Err(SampleSheetError::from(e)),
                        },
                        None => return Err(SampleSheetError::EofError),
                    },
                    SampleSheetSection::Data(..) | SampleSheetSection::BCLConvertData(..) => {
                        data = Some(Vec::new());
                        for row in csv_reader.deserialize() {
                            match row {
                                Ok(h) => data.as_mut().unwrap().push(h),
                                Err(e) => return Err(SampleSheetError::from(e)),
                            }
                        }
                    }
                    SampleSheetSection::Settings(..)
                    | SampleSheetSection::BCLConvertSettings(..) => {
                        match csv_reader.deserialize().next() {
                            Some(v) => match v {
                                Ok(h) => settings = Some(h),
                                Err(e) => return Err(SampleSheetError::from(e)),
                            },
                            None => return Err(SampleSheetError::EofError),
                        }
                    }
                    SampleSheetSection::Other(..) => {
                        debug!("Ignoring unknown section {}", section);
                    }
                }
                slice = i;
            }
            Err(nom::Err::Error(e)) => return Err(e),
            Err(nom::Err::Incomplete(..)) => return Err(SampleSheetError::EofError),
            Err(nom::Err::Failure(e)) => return Err(e),
        }
    }
    if header.is_none() || reads.is_none() || data.is_none() || settings.is_none() {
        Err(SampleSheetError::MissingSection(
            "One or more required sections missing from SampleSheet",
        ))
    } else {
        Ok(SampleSheet {
            header: header.unwrap(),
            reads: reads.unwrap(),
            settings: settings.unwrap(),
            data: data.unwrap(),
        })
    }
}

fn into_reader(input: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(input.as_bytes())
}
