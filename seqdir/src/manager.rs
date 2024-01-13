use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{SeqDir, SeqDirError};

#[derive(Debug, Clone, Serialize)]
pub struct AvailableSeqDir {
    #[serde(flatten)]
    seq_dir: SeqDir,
    since: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnavailableSeqDir {
    seq_dir: SeqDir,
    since: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequencingSeqDir {
    seq_dir: SeqDir,
    since: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedSeqDir {
    seq_dir: SeqDir,
    since: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferringSeqDir {
    seq_dir: SeqDir,
    since: DateTime<Utc>,
}

impl From<AvailableSeqDir> for UnavailableSeqDir {
    /// Available -> Unavailable
    fn from(value: AvailableSeqDir) -> Self {
        UnavailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<UnavailableSeqDir> for AvailableSeqDir {
    /// Unavailable -> Available
    fn from(value: UnavailableSeqDir) -> Self {
        AvailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<FailedSeqDir> for UnavailableSeqDir {
    /// Failed -> Unavailable
    fn from(value: FailedSeqDir) -> Self {
        UnavailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<UnavailableSeqDir> for SequencingSeqDir {
    /// Unavailable -> Sequencing
    fn from(value: UnavailableSeqDir) -> Self {
        SequencingSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<UnavailableSeqDir> for FailedSeqDir {
    /// Unavailable -> Failed
    fn from(value: UnavailableSeqDir) -> Self {
        FailedSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<SequencingSeqDir> for AvailableSeqDir {
    /// Sequencing -> Available
    fn from(value: SequencingSeqDir) -> Self {
        AvailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<SequencingSeqDir> for FailedSeqDir {
    /// Sequencing -> Failed
    fn from(value: SequencingSeqDir) -> Self {
        FailedSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<SequencingSeqDir> for UnavailableSeqDir {
    /// Sequencing -> Unavailable
    fn from(value: SequencingSeqDir) -> Self {
        UnavailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<SequencingSeqDir> for TransferringSeqDir {
    /// Sequencing -> Transferring
    fn from(value: SequencingSeqDir) -> Self {
        TransferringSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<TransferringSeqDir> for AvailableSeqDir {
    /// Transferring -> Available
    fn from(value: TransferringSeqDir) -> Self {
        AvailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<TransferringSeqDir> for UnavailableSeqDir {
    /// Transferring -> Unavailable
    fn from(value: TransferringSeqDir) -> Self {
        UnavailableSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<UnavailableSeqDir> for TransferringSeqDir {
    /// Unavailable -> Transferring
    fn from(value: UnavailableSeqDir) -> Self {
        TransferringSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
    }
}

impl From<TransferringSeqDir> for FailedSeqDir {
    /// Transferring -> Failed
    fn from(value: TransferringSeqDir) -> Self {
        FailedSeqDir {
            seq_dir: value.seq_dir,
            since: Utc::now(),
        }
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
            SeqDirState::Failed(dir) => &dir.seq_dir,
            SeqDirState::Available(dir) => &dir.seq_dir,
            SeqDirState::Unavailable(dir) => &dir.seq_dir,
            SeqDirState::Sequencing(dir) => &dir.seq_dir,
            SeqDirState::Transferring(dir) => &dir.seq_dir,
        }
    }

    /// Timestamp of when state was entered
    fn since(&self) -> &DateTime<Utc> {
        match self {
            SeqDirState::Failed(dir) => &dir.since,
            SeqDirState::Available(dir) => &dir.since,
            SeqDirState::Unavailable(dir) => &dir.since,
            SeqDirState::Sequencing(dir) => &dir.since,
            SeqDirState::Transferring(dir) => &dir.since,
        }
    }

    /// Mutable reference to inner SeqDir
    fn dir_mut(&mut self) -> &mut SeqDir {
        match self {
            SeqDirState::Failed(dir) => &mut dir.seq_dir,
            SeqDirState::Available(dir) => &mut dir.seq_dir,
            SeqDirState::Unavailable(dir) => &mut dir.seq_dir,
            SeqDirState::Sequencing(dir) => &mut dir.seq_dir,
            SeqDirState::Transferring(dir) => &mut dir.seq_dir,
        }
    }
}

#[derive(Clone)]
struct DirManager {
    seq_dir: SeqDirState,
}

impl DirManager {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, SeqDirError> {
        let mut dir_manager = DirManager {
            seq_dir: SeqDirState::Unavailable(UnavailableSeqDir {
                seq_dir: SeqDir::from_path(&path)?,
                since: Utc::now(),
            }),
        };
        dir_manager.poll();
        Ok(dir_manager)
    }

    /// Consume the DirManager, returning contained SeqDir, regardless of state.
    /// Discards associated timestamp.
    pub fn into_inner(self) -> Result<SeqDir, SeqDirError> {
        match self.seq_dir {
            SeqDirState::Available(dir) => Ok(dir.seq_dir),
            SeqDirState::Sequencing(dir) => Ok(dir.seq_dir),
            SeqDirState::Failed(dir) => Ok(dir.seq_dir),
            SeqDirState::Unavailable(dir) => Ok(dir.seq_dir),
            SeqDirState::Transferring(dir) => Ok(dir.seq_dir),
        }
    }

    /// Reference to the inner SeqDir being managed
    pub fn inner(&self) -> &SeqDir {
        self.seq_dir.dir()
    }

    /// Mutable reference to inner SeqDir being managed
    /// It is not recommended that you mutate the inner Seqdir directly unless you have a very good
    /// reason to. Doing so can cause unexpected behavior.
    pub fn inner_mut(&mut self) -> &mut SeqDir {
        self.seq_dir.dir_mut()
    }

    /// Current state
    pub fn state(&self) -> &SeqDirState {
        &self.seq_dir
    }

    /// Check if the contained SeqDir should be moved to a new state, and transition if so
    pub fn poll(&mut self) -> &SeqDirState {
        *self = match std::mem::replace(&mut self.seq_dir, _default()) {
            SeqDirState::Available(dir) => {
                if dir.seq_dir.try_root().is_err() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    self.seq_dir = SeqDirState::Available(dir);
                    return self.state();
                }
            }
            SeqDirState::Failed(dir) => {
                if dir.seq_dir.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    self.seq_dir = SeqDirState::Failed(dir);
                    return self.state();
                }
            }
            SeqDirState::Unavailable(dir) => {
                if dir.seq_dir.is_unavailable() {
                    self.seq_dir = SeqDirState::Unavailable(dir);
                    return self.state();
                } else if dir.seq_dir.is_failed().unwrap_or(false) {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_sequencing() {
                    DirManager {
                        seq_dir: SeqDirState::Sequencing(SequencingSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_copy_complete() {
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
            SeqDirState::Sequencing(dir) => {
                if dir.seq_dir.is_sequencing() {
                    self.seq_dir = SeqDirState::Sequencing(dir);
                    return self.state();
                }
                if dir.seq_dir.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_failed().unwrap_or(false) {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_copy_complete() {
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
                if dir.seq_dir.is_copy_complete() {
                    DirManager {
                        seq_dir: SeqDirState::Available(AvailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_unavailable() {
                    DirManager {
                        seq_dir: SeqDirState::Unavailable(UnavailableSeqDir::from(dir)),
                        ..*self
                    }
                } else if dir.seq_dir.is_failed().unwrap_or(false) {
                    DirManager {
                        seq_dir: SeqDirState::Failed(FailedSeqDir::from(dir)),
                        ..*self
                    }
                } else {
                    self.seq_dir = SeqDirState::Transferring(dir);
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
    SeqDirState::Unavailable(UnavailableSeqDir {
        seq_dir: SeqDir {
            root: Path::new("").to_owned(),
            samplesheet: Path::new("").to_owned(),
            run_info: Path::new("").to_owned(),
            run_params: Path::new("").to_owned(),
            run_completion: Path::new("").to_owned(),
        },
        since: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use super::{DirManager, SeqDirState};

    const COMPLETE: &str = "test_data/seq_complete/";
    const FAILED: &str = "test_data/seq_failed/";
    const TRANSFERRING: &str = "test_data/seq_transferring/";

    #[test]
    fn goes_to_available() {
        let mut manager = DirManager::new(&COMPLETE).unwrap();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
        manager.poll();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
    }

    #[test]
    fn goes_to_failed() {
        let mut manager = DirManager::new(&FAILED).unwrap();
        match manager.state() {
            SeqDirState::Failed(..) => {}
            x => panic!("expected SeqDirState::Failed, got {x:?}"),
        };
        manager.poll();
        match manager.state() {
            SeqDirState::Failed(..) => {}
            x => panic!("expected SeqDirState::Failed, got {x:?}"),
        };
    }

    #[test]
    fn goes_to_unavailable() {
        // you cannot manage a directory that doesn't exist
        let mut manager = DirManager::new(&COMPLETE).unwrap();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
        manager.inner_mut().root = PathBuf::from_str("/dev/null").unwrap();
        manager.poll();
        match manager.state() {
            SeqDirState::Unavailable(..) => {}
            x => panic!("expected SeqDirState::Unavailable, got {x:?}"),
        };
        manager.inner_mut().root = PathBuf::from_str(&COMPLETE).unwrap();
        manager.poll();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
    }

    #[test]
    fn transferring_to_available() {
        let copy_complete = PathBuf::from_str(&TRANSFERRING)
            .unwrap()
            .join("CopyComplete.txt");
        let mut manager = DirManager::new(&TRANSFERRING).unwrap();
        match manager.state() {
            SeqDirState::Transferring(..) => {}
            x => panic!("expected SeqDirState::Transferring, got {x:?}"),
        };
        std::fs::File::create(&copy_complete).unwrap();
        manager.poll();
        std::fs::remove_file(&copy_complete).unwrap();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
    }

    #[test]
    fn test_serialize_to_json() {
        use serde_json;

        let mut manager = DirManager::new(&COMPLETE).unwrap();
        match manager.state() {
            SeqDirState::Available(..) => {}
            x => panic!("expected SeqDirState::Available, got {x:?}"),
        };
        manager.poll();

        println!("{:#?}", serde_json::to_string(manager.state()).unwrap());
    }
}
