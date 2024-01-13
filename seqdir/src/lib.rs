use run_completion::CompletionStatus;
use serde::Serialize;
use std::convert::AsRef;
use std::ffi::OsStr;
use std::fs::read_dir;
use std::num::ParseIntError;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

pub mod manager;
pub mod run_completion;

use crate::run_completion::parse_run_completion;

const COPY_COMPLETE_TXT: &str = "CopyComplete.txt";
const RTA_COMPLETE_TXT: &str = "RTAComplete.txt";
const SEQUENCE_COMPLETE_TXT: &str = "SequenceComplete.txt";
const SAMPLESHEET_CSV: &str = "SampleSheet.csv";
const RUN_INFO_XML: &str = "RunInfo.xml";
const RUN_COMPLETION_STATUS_XML: &str = "RunCompletionStatus.xml";
const RUN_PARAMS_XML: &str = "RunParameters.xml";
const LANES: [&str; 4] = ["L001", "L002", "L003", "L004"];
const BASECALLS: &str = "Data/Intensities/BaseCalls/";
const FILTER_EXT: &str = "filter";
const CBCL: &str = "cbcl";
const CBCL_GZ: &str = "cbcl.gz";
const BCL: &str = "bcl";
const BCL_GZ: &str = "bcl.gz";
const CYCLE_PREFIX: &str = "C";

/// A BCL or a CBCL
#[derive(Clone, Debug, Serialize)]
pub enum Bcl {
    Bcl(PathBuf),
    CBcl(PathBuf),
}

impl Bcl {
    /// Construct Bcl enum variant from a path.
    ///
    /// Paths ending in 'bcl' or 'bcl.gz' are mapped to `Bcl`.
    /// Paths ending in 'cbcl' or 'cbcl.gz' are mapped to `Cbcl`.
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path_str = path.as_ref().to_str()?;
        if path_str.ends_with(CBCL) || path_str.ends_with(CBCL_GZ) {
            Some(Self::CBcl(path.as_ref().to_owned()))
        } else if path_str.ends_with(BCL) || path_str.ends_with(BCL_GZ) {
            Some(Self::Bcl(path.as_ref().to_owned()))
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum SeqDirError {
    #[error("cannot find {0} or it is not readable")]
    NotFound(PathBuf),
    #[error("attempt to retrieve unassociated file")]
    IncompleteNotFound,
    #[error("cannot find lane directories")]
    MissingLaneDirs,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("incorrect number of lanes found, expected 2 or 4, found {0}")]
    MissingLanes(usize),
    #[error("expected no more than four lanes, found {0}")]
    TooManyLanes(usize),
    #[error("found no cycles")]
    MissingCycles,
    #[error("found no bcls for cycle {0}")]
    MissingBcls(u16),
    #[error("expected cycle directory in format of C###.#, found: {0}")]
    BadCycle(PathBuf),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
    #[error("unexpected run completion status: {0}")]
    CompletionStatus(CompletionStatus),
}

#[derive(Debug, Clone, Serialize)]
pub struct Cycle {
    cycle_num: u16,
    bcls: Vec<Bcl>,
}

impl Cycle {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Cycle, SeqDirError> {
        let cycle_num = path
            .as_ref()
            .file_stem()
            .ok_or_else(|| SeqDirError::BadCycle(path.as_ref().to_owned()))?
            .to_string_lossy()
            .strip_prefix(CYCLE_PREFIX)
            .ok_or_else(|| SeqDirError::BadCycle(path.as_ref().to_owned()))?
            .parse::<u16>()?;

        let bcls: Vec<Bcl> = read_dir(path)?
            .filter_map(|c| c.ok())
            .map(|c| c.path())
            .filter_map(|c| Bcl::from_path(&c))
            .collect();
        if bcls.is_empty() {
            return Err(SeqDirError::MissingBcls(cycle_num));
        }

        Ok(Cycle { cycle_num, bcls })
    }

    fn cycle_num(&self) -> u16 {
        self.cycle_num
    }

    fn bcls(&self) -> &Vec<Bcl> {
        &self.bcls
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Lane<P: AsRef<Path>> {
    cycles: Vec<Cycle>,
    filters: Vec<P>,
}

impl<P> Lane<P>
where
    P: AsRef<Path>,
{
    fn from_path(path: P) -> Result<Lane<PathBuf>, SeqDirError> {
        let (cycle_paths, other_files): (Vec<PathBuf>, Vec<PathBuf>) = read_dir(path)?
            .filter_map(|p| p.ok())
            .map(|p| p.path())
            .partition(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap_or_else(|| OsStr::new(""))
                        .to_str()
                        .unwrap_or_else(|| "")
                        .starts_with(CYCLE_PREFIX)
            });

        let cycles: Vec<Cycle> = cycle_paths
            .iter()
            .map(|c| Cycle::from_path(c))
            .collect::<Result<Vec<Cycle>, SeqDirError>>()?;
        if cycles.is_empty() {
            return Err(SeqDirError::MissingCycles);
        }

        let filters: Vec<PathBuf> = other_files
            .iter()
            .filter(|p| {
                p.is_file() && p.extension().unwrap_or_else(|| OsStr::new("")) == FILTER_EXT
            })
            .map(|p| p.clone())
            .collect();

        Ok(Lane { cycles, filters })
    }

    pub fn cycles(&self) -> &Vec<Cycle> {
        &self.cycles
    }

    pub fn filters(&self) -> &Vec<P> {
        &self.filters
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SeqDir {
    root: PathBuf,
    #[serde(skip)]
    samplesheet: PathBuf,
    #[serde(skip)]
    run_info: PathBuf,
    #[serde(skip)]
    run_params: PathBuf,
    #[serde(skip)]
    run_completion: PathBuf,
}

impl SeqDir {
    /// Create a new SeqDir
    ///
    /// Succeeds as long as `path` is readable and is a directory.
    /// To enforce that the directory is a well-formed, completed sequencing directory, use
    /// `from_completed`.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, SeqDirError> {
        if path.as_ref().is_dir() {
            Ok(SeqDir {
                root: path.as_ref().to_path_buf(),
                samplesheet: path.as_ref().join(SAMPLESHEET_CSV),
                run_info: path.as_ref().join(RUN_INFO_XML),
                run_params: path.as_ref().join(RUN_PARAMS_XML),
                run_completion: path.as_ref().join(RUN_COMPLETION_STATUS_XML),
            })
        } else {
            Err(SeqDirError::NotFound(path.as_ref().to_path_buf()))
        }
    }

    /// Create a new SeqDir from a completed sequencing directory.
    ///
    /// Errors if the sequencing directory is not complete. Completion is determined by the
    /// following:
    /// 1. CopyComplete.txt is present
    /// 2. RunCompletionStatus (if present) is CompletedAsPlanned
    pub fn from_completed<P: AsRef<Path>>(path: P) -> Result<Self, SeqDirError> {
        let seq_dir = Self::from_path(&path)?;
        seq_dir
            .is_copy_complete()
            .then(|| Ok::<(), SeqDirError>(()))
            .ok_or_else(|| SeqDirError::NotFound(seq_dir.root().join(COPY_COMPLETE_TXT)))??;

        // If RunCompletionStatus exists, verify it, but cannot rely on this
        // since not all platforms output this file
        match seq_dir.get_completion_status() {
            None => {}
            Some(Ok(status)) => match status {
                CompletionStatus::CompletedAsPlanned(..) => {}
                _ => return Err(SeqDirError::CompletionStatus(status)),
            },
            Some(Err(e)) => return Err(e),
        };

        Ok(seq_dir)
    }

    /// get lane data (if any) associated with the sequencing directory
    ///
    /// To keep SeqDir lightweight and to support incomplete sequencing runs, lanes are not stored
    /// within SeqDir.
    pub fn lanes(&self) -> Result<Vec<Lane<PathBuf>>, SeqDirError> {
        detect_lanes(&self.root)
    }

    /// Try to get the root of the sequencing directory.
    /// Returns SeqDirError::NotFound if directory is inaccessible.
    fn try_root(&self) -> Result<&Path, SeqDirError> {
        self.root()
            .is_dir()
            .then(|| self.root())
            .ok_or_else(|| SeqDirError::NotFound(self.root().to_owned()))
    }

    /// Returns true if CopyComplete.txt exists.
    fn is_copy_complete(&self) -> bool {
        self.root().join(COPY_COMPLETE_TXT).exists()
    }

    /// Returns true if RTAComplete.txt exists.
    fn is_rta_complete(&self) -> bool {
        self.root().join(RTA_COMPLETE_TXT).exists()
    }

    /// Returns true if SequenceComplete.txt exists.
    fn is_sequence_complete(&self) -> bool {
        self.root().join(SEQUENCE_COMPLETE_TXT).exists()
    }

    /// Get an arbitrary file rooted at the base of the sequencing directory.
    ///
    /// Returns SeqDirError::NotFound if file does not exist or is inaccessible.
    fn get_file<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, SeqDirError> {
        self.root()
            .join(&path)
            .is_file()
            .then(|| self.root().join(&path))
            .ok_or_else(|| SeqDirError::NotFound(self.root().join(&path)))
    }

    /// Returns true if the root directory is readable.
    ///
    /// CAUTION: The meaning of 'available' as it is used here is not the same as how it used
    /// within the context of a managed directory. Calling this directly means that a sequencing
    /// directory can be read, but not necessarily that sequencing is complete and all contents
    /// have been transferred. You probably want `is_copy_complete`.
    fn is_available(&self) -> bool {
        self.try_root().is_ok()
    }

    // Returns true if the root directory cannot be read
    fn is_unavailable(&self) -> bool {
        self.try_root().is_err()
    }

    /// Attempt to parse RunCompletionStatus.xml and return a `CompletionStatus`
    fn get_completion_status(&self) -> Option<Result<CompletionStatus, SeqDirError>> {
        Some(parse_run_completion(&self.run_completion_status()?).map_err(|e| SeqDirError::from(e)))
    }

    /// Attempt to determine if a run has failed sequencing.
    /// Uses RunCompletionStatus.xml. If RunCompletionStatus is not available, returns false.
    /// unlike other `is_` library methods, this is fallible because it must parse a file.
    fn is_failed(&self) -> Result<bool, SeqDirError> {
        match self.get_completion_status() {
            None => Ok(false),
            Some(Err(e)) => Err(e),
            Some(Ok(res)) => match res {
                CompletionStatus::CompletedAsPlanned(..) => Ok(false),
                _ => Ok(true),
            },
        }
    }

    /// Returns true if SequenceComplete.txt is not present
    /// Convenience method, inverts `is_sequence_complete`
    fn is_sequencing(&self) -> bool {
        !self.is_sequence_complete()
    }

    /// Returns reference to seqdir root
    fn root(&self) -> &Path {
        &self.root
    }

    /// Get the path to SampleSheet.csv
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn samplesheet(&self) -> Result<&Path, SeqDirError> {
        self.samplesheet
            .is_file()
            .then(|| self.samplesheet.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.samplesheet.clone()))
    }

    /// Get the path to RunInfo.xml
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn run_info(&self) -> Result<&Path, SeqDirError> {
        self.run_info
            .is_file()
            .then(|| self.run_info.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.run_info.clone()))
    }

    /// Get the path to RunParameters.xml
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn run_params(&self) -> Result<&Path, SeqDirError> {
        self.run_params
            .is_file()
            .then(|| self.run_params.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.run_params.clone()))
    }

    /// Get the path to RunCompletionStatus.xml
    /// Returns Option because not all illumina sequencers generate this file.
    /// To actually parse RunCompletionStatus.xml, see `get_completion_status`
    fn run_completion_status(&self) -> Option<&Path> {
        self.run_completion
            .is_file()
            .then(|| self.run_completion.as_path())
            .or(None)
    }
}

/// Find outputs per-lane for a sequencing directory and construct `Lane` objects
/// Will only find lanes 'L001' - 'L004', because those are the only ones that should exist.
fn detect_lanes<P: AsRef<Path>>(dir: P) -> Result<Vec<Lane<PathBuf>>, SeqDirError> {
    LANES
        .iter()
        .map(|l| dir.as_ref().join(BASECALLS).join(l))
        .filter(|l| l.exists())
        .map(|l| Lane::from_path(dir.as_ref().join(l)))
        .collect::<Result<Vec<Lane<PathBuf>>, SeqDirError>>()
}

#[cfg(test)]
mod tests {

    use crate::{SeqDir, SeqDirError};

    const COMPLETE: &str = "test_data/seq_complete/";
    const FAILED: &str = "test_data/seq_failed/";
    const TRANSFERRING: &str = "test_data/seq_transferring/";
    const SEQUENCING: &str = "test_data/seq_sequencing/";

    #[test]
    fn complete_seqdir() {
        let seq_dir = SeqDir::from_completed(&COMPLETE).unwrap();
        seq_dir.samplesheet().unwrap();
        seq_dir.run_info().unwrap();
        seq_dir.run_params().unwrap();
        assert!(seq_dir.is_available());
        assert!(seq_dir.is_sequence_complete());
        assert!(seq_dir.is_copy_complete());
        assert!(seq_dir.is_rta_complete());
        assert!(!seq_dir.is_sequencing());
        assert!(seq_dir.lanes().is_ok())
    }

    #[test]
    fn failed_seqdir() {
        let seq_dir = SeqDir::from_path(&FAILED).unwrap();
        assert!(seq_dir.is_failed().unwrap());
        assert!(matches!(
            SeqDir::from_completed(&FAILED),
            Err(SeqDirError::CompletionStatus(..))
        ));
    }

    #[test]
    fn transferring_seqdir() {
        let seq_dir = SeqDir::from_path(&TRANSFERRING).unwrap();
        assert!(seq_dir.is_available());
        assert!(seq_dir.is_sequence_complete());
        assert!(!seq_dir.is_sequencing());
        assert!(!seq_dir.is_failed().unwrap());
        assert!(!seq_dir.is_copy_complete());
        assert!(seq_dir.is_rta_complete());
    }
    
    #[test]
    fn sequencing_seqdir() {
        let seq_dir = SeqDir::from_path(&SEQUENCING).unwrap();
        assert!(seq_dir.is_available());
        assert!(!seq_dir.is_sequence_complete());
        assert!(seq_dir.is_sequencing());
        assert!(!seq_dir.is_failed().unwrap());
        assert!(!seq_dir.is_copy_complete());
        assert!(seq_dir.is_rta_complete());
    }
}
