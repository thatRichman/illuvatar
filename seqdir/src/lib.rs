use std::convert::AsRef;
use std::ffi::OsStr;
use std::fs::read_dir;
use std::num::ParseIntError;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

pub mod manager;

const COPY_COMPLETE_TXT: &str = "CopyComplete.txt";
const RTA_COMPLETE_TXT: &str = "RTAComplete.txt";
const SEQUENCE_COMPLETE_TXT: &str = "SequenceComplete.txt";
const SAMPLESHEET_CSV: &str = "SampleSheet.csv";
const RUN_INFO_XML: &str = "RunInfo.xml";
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
}

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

pub trait SequencingDirectory {
    /// Get the root of the sequencing directory.
    ///
    /// Returns SeqDirError::NotFound if directory is inaccessible.
    fn root(&self) -> &Path;

    fn try_root(&self) -> Result<&Path, SeqDirError> {
        self.root()
            .is_dir()
            .then(|| self.root())
            .ok_or_else(|| SeqDirError::NotFound(self.root().to_owned()))
    }

    fn lanes(&self) -> &Vec<Lane<PathBuf>>;

    /// Get the path to SampleSheet.csv
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn samplesheet(&self) -> Result<&Path, SeqDirError>;

    /// Get the path to RunInfo.xml
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn run_info(&self) -> Result<&Path, SeqDirError>;

    /// Get the path to RunParameters.xml
    ///
    /// Returns SeqDirError::NotFound if path does not exist or is inaccessible.
    fn run_params(&self) -> Result<&Path, SeqDirError>;

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

    // Returns true if the root directory is *not* readable
    fn is_unavailable(&self) -> bool {
        self.try_root().is_err()
    }

    fn is_failed(&self) -> bool {
        todo!()
    }

    fn check_for_failure(&self) -> bool {
        todo!()
    }

    /// Returns true if SequenceComplete.txt is not missing
    fn is_sequencing(&self) -> bool {
        !self.is_sequence_complete()
    }
}

pub struct SeqDir {
    root: PathBuf,
    samplesheet: PathBuf,
    run_info: PathBuf,
    run_params: PathBuf,
    lanes: Vec<Lane<PathBuf>>,
}

impl SeqDir {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, SeqDirError> {
        detect_illumina_seq_dir(path)
    }
}

impl SequencingDirectory for SeqDir {
    fn root(&self) -> &Path {
        &self.root
    }

    fn lanes(&self) -> &Vec<Lane<PathBuf>> {
        &self.lanes
    }

    fn samplesheet(&self) -> Result<&Path, SeqDirError> {
        self.samplesheet
            .is_file()
            .then(|| self.samplesheet.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.samplesheet.clone()))
    }

    fn run_info(&self) -> Result<&Path, SeqDirError> {
        self.run_info
            .is_file()
            .then(|| self.run_info.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.run_info.clone()))
    }

    fn run_params(&self) -> Result<&Path, SeqDirError> {
        self.run_params
            .is_file()
            .then(|| self.run_params.as_path())
            .ok_or_else(|| SeqDirError::NotFound(self.run_params.clone()))
    }
}

/// Like SeqDir, except every path field other than `root` is an Option.
/// Additionally, `lanes` field may be <= 4, and incomplete lanes are excluded.
pub struct IncompleteSeqDir {
    root: PathBuf,
    samplesheet: Option<PathBuf>,
    run_info: Option<PathBuf>,
    run_params: Option<PathBuf>,
    lanes: Vec<Lane<PathBuf>>,
}

impl IncompleteSeqDir {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<IncompleteSeqDir, SeqDirError> {
        detect_incomplete_illumina_seq_dir(path)
    }
}

impl SequencingDirectory for IncompleteSeqDir {
    fn root(&self) -> &Path {
        &self.root
    }

    fn lanes(&self) -> &Vec<Lane<PathBuf>> {
        &self.lanes
    }

    fn samplesheet(&self) -> Result<&Path, SeqDirError> {
        match self.samplesheet.as_ref() {
            None => Err(SeqDirError::IncompleteNotFound),
            Some(p) => p
                .is_file()
                .then(|| p.as_path())
                .ok_or_else(|| SeqDirError::NotFound(p.clone())),
        }
    }

    fn run_info(&self) -> Result<&Path, SeqDirError> {
        match self.run_info.as_ref() {
            None => Err(SeqDirError::IncompleteNotFound),
            Some(p) => p
                .is_file()
                .then(|| p.as_path())
                .ok_or_else(|| SeqDirError::NotFound(p.clone())),
        }
    }

    fn run_params(&self) -> Result<&Path, SeqDirError> {
        match self.run_params.as_ref() {
            None => Err(SeqDirError::IncompleteNotFound),
            Some(p) => p
                .is_file()
                .then(|| p.as_path())
                .ok_or_else(|| SeqDirError::NotFound(p.clone())),
        }
    }
}

impl TryFrom<IncompleteSeqDir> for SeqDir {
    type Error = SeqDirError;
    /// Given an IncompleteSeqDir, attempt to construct a
    /// SeqDir by running `detect_illumina_seq_dir` on the root.
    fn try_from(value: IncompleteSeqDir) -> Result<Self, Self::Error> {
        detect_illumina_seq_dir(value.root)
    }
}

/// Given a directory, detect whether it appears to be a complete Illumina sequencing directory.
/// If so, construct a SeqDir struct. Otherwise, raise a SeqDirError.
///
/// If you need to manage a sequencing directory that may be incomplete or malformed,
/// see IncompleteSeqDir and detect_incomplete_illumina_seq_dir.
pub fn detect_illumina_seq_dir<P: AsRef<Path>>(dir: P) -> Result<SeqDir, SeqDirError> {
    dir.as_ref().try_exists()?;
    if !dir.as_ref().is_dir() {
        return Err(SeqDirError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "sequencing directory must be a directory",
        )));
    }
    if !dir.as_ref().join(COPY_COMPLETE_TXT).exists() {
        return Err(SeqDirError::NotFound(dir.as_ref().join(COPY_COMPLETE_TXT)));
    }
    let samplesheet = dir.as_ref().join(SAMPLESHEET_CSV);
    samplesheet
        .try_exists()
        .map_err(|_| SeqDirError::NotFound(samplesheet.clone()))?;
    let run_info = dir.as_ref().join(RUN_INFO_XML);
    run_info
        .try_exists()
        .map_err(|_| SeqDirError::NotFound(run_info.clone()))?;

    let run_params = dir.as_ref().join(RUN_PARAMS_XML);
    run_params
        .try_exists()
        .map_err(|_| SeqDirError::NotFound(run_params.clone()))?;

    let lanes = LANES
        .iter()
        .map(|l| dir.as_ref().join(BASECALLS).join(l))
        .filter(|l| l.exists())
        .map(|l| Lane::from_path(dir.as_ref().join(l)))
        .collect::<Result<Vec<Lane<PathBuf>>, SeqDirError>>()?;
    match lanes.len() {
        2 | 4 => {}
        x if x <= 1 || x == 3 => return Err(SeqDirError::MissingLanes(x)),
        x => return Err(SeqDirError::TooManyLanes(x)),
    }

    Ok(SeqDir {
        root: PathBuf::from(dir.as_ref()),
        samplesheet,
        run_info,
        run_params,
        lanes,
    })
}

/// Given a directory, detect whether it appears to be a (possibly incomplete) Illumina sequencing directory.
/// If so, construct an IncompleteSeqDir struct. Otherwise, raise a SeqDirError.
///
/// To instead raise an error if the directory appears incomplete, see SeqDir and
/// detect_illumina_seq_dir.
pub fn detect_incomplete_illumina_seq_dir<P: AsRef<Path>>(
    dir: P,
) -> Result<IncompleteSeqDir, SeqDirError> {
    dir.as_ref().try_exists()?;
    if !dir.as_ref().is_dir() {
        return Err(SeqDirError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "sequencing directory must be a directory",
        )));
    }
    let samplesheet_path = dir.as_ref().join(SAMPLESHEET_CSV);
    let samplesheet = match samplesheet_path.try_exists().ok() {
        None => None,
        Some(_) => Some(samplesheet_path),
    };
    let run_info_path = dir.as_ref().join(RUN_INFO_XML);
    let run_info = match run_info_path.try_exists().ok() {
        None => None,
        Some(_) => Some(run_info_path),
    };
    let run_params_path = dir.as_ref().join(RUN_PARAMS_XML);
    let run_params = match run_params_path.try_exists().ok() {
        None => None,
        Some(_) => Some(run_params_path),
    };

    let lanes = LANES
        .iter()
        .map(|l| dir.as_ref().join(BASECALLS).join(l))
        .filter(|l| l.exists())
        .map(|l| Lane::from_path(dir.as_ref().join(l)))
        .filter(|l| l.is_ok())
        .collect::<Result<Vec<Lane<PathBuf>>, SeqDirError>>()?;
    match lanes.len() {
        x if x > 4 => return Err(SeqDirError::TooManyLanes(x)),
        _ => {}
    }

    Ok(IncompleteSeqDir {
        root: PathBuf::from(dir.as_ref()),
        samplesheet,
        run_info,
        run_params,
        lanes,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn cbcl() {
        unimplemented!()
    }

    #[test]
    fn bcl() {
        unimplemented!()
    }

    #[test]
    fn good_bcl() {
        unimplemented!()
    }

    #[test]
    fn bad_bcl() {
        unimplemented!();
    }

    #[test]
    fn good_cycle() {
        unimplemented!();
    }

    #[test]
    fn bad_cycle() {
        unimplemented!();
    }

    #[test]
    fn good_lane() {
        unimplemented!();
    }

    #[test]
    fn bad_lane() {
        unimplemented!()
    }

    #[test]
    fn good_seqdir() {
        unimplemented!()
    }

    #[test]
    fn bad_seqdir() {
        unimplemented!();
    }

    #[test]
    fn good_incomplete_seqdir() {
        unimplemented!();
    }

    #[test]
    fn bad_incomplete_seqdir() {
        unimplemented!();
    }

    #[test]
    fn good_transition() {
        unimplemented!();
    }

    #[test]
    fn bad_transition() {
        unimplemented!();
    }
}
