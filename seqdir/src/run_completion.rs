use std::path::Path;
use std::{fs::File, io::Read};

use roxmltree;

const RUN_ID: &str = "RunId";
const COMPLETION_STATUS: &str = "CompletionStatus";
const ERROR_DESCRIPTION: &str = "ErrorDescription";

pub struct Message {
    run_id: String,
    message: Option<String>,
}

#[non_exhaustive]
pub enum CompletionStatus {
    CompletedAsPlanned(Message),
    ExceptionEndedEarly(Message),
    UserEndedEarly(Message),
    Other(Message),
}

fn parse_run_completion<P: AsRef<Path> + std::fmt::Display>(
    path: P,
) -> Result<CompletionStatus, std::io::Error> {
    let mut handle = File::open(&path)?;
    let mut raw_contents = String::new();
    handle.read_to_string(&mut raw_contents)?;
    let doc = roxmltree::Document::parse(&raw_contents).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Could not parse {path} as XML: {e}"),
        )
    })?;

    let run_id = match doc.descendants().find(|elem| elem.has_tag_name(RUN_ID)) {
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "missing RunId tag",
            ))
        }
        Some(node) => match node.text() {
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "RunId tag is empty",
                ))
            }
            Some(id) => id,
        },
    }
    .to_string();

    let message = match doc
        .descendants()
        .find(|elem| elem.has_tag_name(ERROR_DESCRIPTION))
    {
        Some(node) => match node.text() {
            None => None,
            Some(text) if text == "None" => None,
            Some(text) => Some(text.to_string()),
        },
        None => None,
    };

    let message = Message { run_id, message };

    match doc
        .descendants()
        .find(|elem| elem.has_tag_name(COMPLETION_STATUS))
    {
        None => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing CompletionStatus tag",
        )),
        Some(node) => match node.text() {
            Some("CompletedAsPlanned") => Ok(CompletionStatus::CompletedAsPlanned(message)),
            Some("ExceptionEndedEarly") => Ok(CompletionStatus::ExceptionEndedEarly(message)),
            Some("UserEndedEarly") => Ok(CompletionStatus::UserEndedEarly(message)),
            Some(_) => Ok(CompletionStatus::Other(message)),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "CompletionStatus tag is empty",
            )),
        },
    }
}
