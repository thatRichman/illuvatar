use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{SeqDir, SeqDirError};

#[derive(Debug, Clone, Serialize)]
pub struct AvailableSeqDir(SeqDir, DateTime<Utc>);
#[derive(Debug, Clone, Serialize)]
pub struct UnavailableSeqDir(SeqDir, DateTime<Utc>);
#[derive(Debug, Clone, Serialize)]
pub struct SequencingSeqDir(SeqDir, DateTime<Utc>);
#[derive(Debug, Clone, Serialize)]
pub struct FailedSeqDir(SeqDir, DateTime<Utc>);
#[derive(Debug, Clone, Serialize)]
pub struct TransferringSeqDir(SeqDir, DateTime<Utc>);

impl From<AvailableSeqDir> for UnavailableSeqDir {
    /// Available -> Unavailable
    fn from(value: AvailableSeqDir) -> Self {
        UnavailableSeqDir(value.0, Utc::now())
    }
}

impl From<UnavailableSeqDir> for AvailableSeqDir {
    /// Unavailable -> Available
    fn from(value: UnavailableSeqDir) -> Self {
        AvailableSeqDir(value.0, Utc::now())
    }
}

impl From<FailedSeqDir> for UnavailableSeqDir {
    /// Failed -> Unavailable
    fn from(value: FailedSeqDir) -> Self {
        UnavailableSeqDir(value.0, Utc::now())
    }
}

impl From<UnavailableSeqDir> for SequencingSeqDir {
    /// Unavailable -> Sequencing
    fn from(value: UnavailableSeqDir) -> Self {
        SequencingSeqDir(value.0, Utc::now())
    }
}

impl From<UnavailableSeqDir> for FailedSeqDir {
    /// Unavailable -> Failed
    fn from(value: UnavailableSeqDir) -> Self {
        FailedSeqDir(value.0, Utc::now())
    }
}

impl From<SequencingSeqDir> for AvailableSeqDir {
    /// Sequencing -> Available
    fn from(value: SequencingSeqDir) -> Self {
        AvailableSeqDir(value.0, Utc::now())
    }
}

impl From<SequencingSeqDir> for FailedSeqDir {
    /// Sequencing -> Failed
    fn from(value: SequencingSeqDir) -> Self {
        FailedSeqDir(value.0, Utc::now())
    }
}

impl From<SequencingSeqDir> for UnavailableSeqDir {
    /// Sequencing -> Unavailable
    fn from(value: SequencingSeqDir) -> Self {
        UnavailableSeqDir(value.0, Utc::now())
    }
}

impl From<SequencingSeqDir> for TransferringSeqDir {
    /// Sequencing -> Transferring
    fn from(value: SequencingSeqDir) -> Self {
        TransferringSeqDir(value.0, Utc::now())
    }
}

impl From<TransferringSeqDir> for AvailableSeqDir {
    /// Transferring -> Available
    fn from(value: TransferringSeqDir) -> Self {
        AvailableSeqDir(value.0, Utc::now())
    }
}

impl From<TransferringSeqDir> for UnavailableSeqDir {
    /// Transferring -> Unavailable
    fn from(value: TransferringSeqDir) -> Self {
        UnavailableSeqDir(value.0, Utc::now())
    }
}

impl From<UnavailableSeqDir> for TransferringSeqDir {
    /// Unavailable -> Transferring
    fn from(value: UnavailableSeqDir) -> Self {
        TransferringSeqDir(value.0, Utc::now())
    }
}

impl From<TransferringSeqDir> for FailedSeqDir {
    /// Transferring -> Failed
    fn from(value: TransferringSeqDir) -> Self {
        FailedSeqDir(value.0, Utc::now())
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum SeqDirState {
    Available(AvailableSeqDir),
    Transferring(TransferringSeqDir),
    Unavailable(UnavailableSeqDir),
    Sequencing(SequencingSeqDir),
    Failed(FailedSeqDir),
}

impl SeqDirState {
    /// Reference to inner SeqDir
    fn dir(&self) -> &SeqDir {
        match self {
            SeqDirState::Failed(dir) => &dir.0,
            SeqDirState::Available(dir) => &dir.0,
            SeqDirState::Unavailable(dir) => &dir.0,
            SeqDirState::Sequencing(dir) => &dir.0,
            SeqDirState::Transferring(dir) => &dir.0,
        }
    }

    /// Timestamp of when state was entered
    fn since(&self) -> &DateTime<Utc> {
        match self {
            SeqDirState::Failed(dir) => &dir.1,
            SeqDirState::Available(dir) => &dir.1,
            SeqDirState::Unavailable(dir) => &dir.1,
            SeqDirState::Sequencing(dir) => &dir.1,
            SeqDirState::Transferring(dir) => &dir.1,
        }
    }
}


#[derive(Clone)]
struct DirManager {
    seq_dir: SeqDirState,
}

impl DirManager {
    /// Consume the DirManager, returning contained SeqDir, regardless of state.
    /// Discards associated timestamp.
    pub fn into_inner(self) -> Result<SeqDir, SeqDirError> {
        match self.seq_dir {
            SeqDirState::Available(dir) => Ok(dir.0),
            SeqDirState::Sequencing(dir) => Ok(dir.0),
            SeqDirState::Failed(dir) => Ok(dir.0),
            SeqDirState::Unavailable(dir) => Ok(dir.0),
            SeqDirState::Transferring(dir) => Ok(dir.0),
        }
    }

    /// Reference to the inner SeqDir being managed
    pub fn inner(&self) -> &SeqDir {
        &self.seq_dir.dir()
    }

    /// Current state
    pub fn state(&self) -> &SeqDirState {
        &self.seq_dir
    }

    /// Check if the contained SeqDir should be moved to a new state, and transition if so
    pub fn poll<'a>(&'a mut self) -> &'a SeqDirState {
        *self = match std::mem::replace(&mut self.seq_dir, _default()) {
            SeqDirState::Available(dir) => {
                if dir.0.try_root().is_err() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    return self.state();
                }
            }
            SeqDirState::Failed(dir) => {
                if dir.0.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    return self.state();
                }
            }
            SeqDirState::Unavailable(dir) => {
                if dir.0.is_unavailable() {
                    return self.state();
                } else if dir.0.is_failed() {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.0.is_sequencing() {
                    DirManager {
                        seq_dir: SeqDirState::Sequencing(SequencingSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    DirManager {
                        seq_dir: SeqDirState::Available(AvailableSeqDir::from(dir)),
                        ..*self
                    }
                }
            }
            SeqDirState::Sequencing(dir) => {
                if dir.0.is_sequencing() {
                    return self.state();
                }
                if dir.0.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.0.is_failed() {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.0.is_copy_complete() {
                    DirManager {
                        seq_dir: SeqDirState::Available(AvailableSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    DirManager {
                        seq_dir: SeqDirState::Transferring(TransferringSeqDir::from(dir)),
                        ..*self
                    }
                }
            }
            SeqDirState::Transferring(dir) => {
                if dir.0.is_copy_complete() {
                    DirManager {
                        seq_dir: SeqDirState::Available(AvailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.0.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.0.is_failed() {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    return self.state();
                }
            }
        };
        self.state()
    }

    /// Timestamp of when the DirManager's SeqDir entered its current state
    pub fn since(&self) -> &DateTime<Utc> {
        self.seq_dir.since()
    }
}

/// This SeqDirState contains a completely invalid SeqDir and is only used as a placeholder when
/// `poll`ing for updated state. This really should not be used anywhere else.
fn _default() -> SeqDirState {
    SeqDirState::Unavailable(UnavailableSeqDir(
        SeqDir {
            root: Path::new("").to_owned(),
            samplesheet: Path::new("").to_owned(),
            run_info: Path::new("").to_owned(),
            run_params: Path::new("").to_owned(),
            run_completion: Path::new("").to_owned(),
        },
        Utc::now(),
    ))
}
