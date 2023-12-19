use std::time::Duration;

use crate::{SeqDir, SeqDirError, SequencingDirectory};

#[derive(Debug, Clone, Copy)]
pub struct AvailableSeqDir<D: SequencingDirectory>(D);
#[derive(Debug, Clone, Copy)]
pub struct UnavailableSeqDir<D: SequencingDirectory>(D);
#[derive(Debug, Clone, Copy)]
pub struct SequencingSeqDir<D: SequencingDirectory>(D);
#[derive(Debug, Clone, Copy)]
pub struct FailedSeqDir<D: SequencingDirectory>(D);
#[derive(Debug, Clone, Copy)]
pub struct TransferringSeqDir<D: SequencingDirectory>(D);

#[derive(Debug, Clone, Copy)]
pub enum SeqDirState<D>
where
    D: SequencingDirectory,
{
    Available {
        dir: AvailableSeqDir<D>,
        since: Duration,
    },
    Transferring {
        dir: TransferringSeqDir<D>,
        since: Duration,
    },
    Unavailable {
        dir: UnavailableSeqDir<D>,
        since: Duration,
    },
    Sequencing {
        dir: SequencingSeqDir<D>,
        since: Duration,
    },
    Failed {
        dir: FailedSeqDir<D>,
        since: Duration,
    },
}

impl<D: SequencingDirectory> SeqDirState<D> {
    fn dir(&self) -> &D {
        match self {
            SeqDirState::Failed { dir, .. } => &dir.0,
            SeqDirState::Available { dir, .. } => &dir.0,
            SeqDirState::Unavailable { dir, .. } => &dir.0,
            SeqDirState::Sequencing { dir, .. } => &dir.0,
            SeqDirState::Transferring { dir, .. } => &dir.0,
        }
    }

    fn since(&self) -> &Duration {
        match self {
            SeqDirState::Failed { since, .. } => &since,
            SeqDirState::Available { since, .. } => &since,
            SeqDirState::Unavailable { since, .. } => &since,
            SeqDirState::Sequencing { since, .. } => &since,
            SeqDirState::Transferring { since, .. } => &since,
        }
    }
}

/// Available -> Unavailable
impl<D: SequencingDirectory> From<AvailableSeqDir<D>> for UnavailableSeqDir<D> {
    fn from(value: AvailableSeqDir<D>) -> Self {
        UnavailableSeqDir(value.0)
    }
}

/// Unavailable -> Available
impl<D: SequencingDirectory> From<UnavailableSeqDir<D>> for AvailableSeqDir<D> {
    fn from(value: UnavailableSeqDir<D>) -> Self {
        AvailableSeqDir(value.0)
    }
}

/// Failed -> Unavailable
impl<D: SequencingDirectory> From<FailedSeqDir<D>> for UnavailableSeqDir<D> {
    fn from(value: FailedSeqDir<D>) -> Self {
        UnavailableSeqDir(value.0)
    }
}

/// Unavailable -> Sequencing
impl<D: SequencingDirectory> From<UnavailableSeqDir<D>> for SequencingSeqDir<D> {
    fn from(value: UnavailableSeqDir<D>) -> Self {
        SequencingSeqDir(value.0)
    }
}

/// Unavailable -> Failed
impl<D: SequencingDirectory> From<UnavailableSeqDir<D>> for FailedSeqDir<D> {
    fn from(value: UnavailableSeqDir<D>) -> Self {
        FailedSeqDir(value.0)
    }
}

/// Sequencing -> Available
impl<D: SequencingDirectory> From<SequencingSeqDir<D>> for AvailableSeqDir<D> {
    fn from(value: SequencingSeqDir<D>) -> Self {
        AvailableSeqDir(value.0)
    }
}

/// Sequencing -> Failed
impl<D: SequencingDirectory> From<SequencingSeqDir<D>> for FailedSeqDir<D> {
    fn from(value: SequencingSeqDir<D>) -> Self {
        FailedSeqDir(value.0)
    }
}

/// Sequencing -> Unavailable
impl<D: SequencingDirectory> From<SequencingSeqDir<D>> for UnavailableSeqDir<D> {
    fn from(value: SequencingSeqDir<D>) -> Self {
        UnavailableSeqDir(value.0)
    }
}

/// Sequencing -> Transferring
impl<D: SequencingDirectory> From<SequencingSeqDir<D>> for TransferringSeqDir<D> {
    fn from(value: SequencingSeqDir<D>) -> Self {
        TransferringSeqDir(value.0)
    }
}

/// Transferring -> Available
impl<D: SequencingDirectory> From<TransferringSeqDir<D>> for AvailableSeqDir<D> {
    fn from(value: TransferringSeqDir<D>) -> Self {
        AvailableSeqDir(value.0)
    }
}

/// Transferring -> Unavailable
impl<D: SequencingDirectory> From<TransferringSeqDir<D>> for UnavailableSeqDir<D> {
    fn from(value: TransferringSeqDir<D>) -> Self {
        UnavailableSeqDir(value.0)
    }
}

/// Transferring -> Failed
impl<D: SequencingDirectory> From<TransferringSeqDir<D>> for FailedSeqDir<D> {
    fn from(value: TransferringSeqDir<D>) -> Self {
        FailedSeqDir(value.0)
    }
}

#[derive(Debug, Clone, Copy)]
struct DirManager<D>
where
    D: SequencingDirectory,
{
    seq_dir: SeqDirState<D>,
}

impl<D> DirManager<D>
where
    D: SequencingDirectory,
{
    /// Consume the DirManager, returning contained SequencingDirectory, regardless of state
    pub fn into_inner(self) -> Result<D, SeqDirError> {
        match self.seq_dir {
            SeqDirState::Available { dir, .. } => Ok(dir.0),
            SeqDirState::Sequencing { dir, .. } => Ok(dir.0),
            SeqDirState::Failed { dir, .. } => Ok(dir.0),
            SeqDirState::Unavailable { dir, .. } => Ok(dir.0),
            SeqDirState::Transferring { dir, .. } => Ok(dir.0),
        }
    }

    pub fn inner(&self) -> &D {
        &self.seq_dir.dir()
    }

    pub fn transition(mut self) {
        match self.seq_dir {
            SeqDirState::Available { dir, .. } => match dir.0.try_root().is_ok() {
                true => {}
                false => {
                    self.seq_dir = SeqDirState::Unavailable {
                        dir: UnavailableSeqDir::from(dir),
                        since: Duration::new(0, 0),
                    };
                }
            },
            SeqDirState::Failed { dir, .. } => match dir.0.is_unavailable() {
                true => {
                    self.seq_dir = SeqDirState::Unavailable {
                        dir: UnavailableSeqDir::from(dir),
                        since: Duration::new(0, 0),
                    }
                }
                false => {}
            },
            SeqDirState::Unavailable { dir, .. } => match dir.0.is_unavailable() {
                true => {}
                false => match dir.0.is_failed() {
                    true => {
                        self.seq_dir = SeqDirState::Failed {
                            dir: FailedSeqDir::from(dir),
                            since: Duration::new(0, 0),
                        };
                    }
                    false => match dir.0.is_sequencing() {
                        true => {
                            self.seq_dir = SeqDirState::Sequencing {
                                dir: SequencingSeqDir::from(dir),
                                since: Duration::new(0, 0),
                            }
                        }
                        false => {
                            self.seq_dir = SeqDirState::Available {
                                dir: AvailableSeqDir::from(dir),
                                since: Duration::new(0, 0),
                            }
                        }
                    },
                },
            },
            SeqDirState::Sequencing { dir, .. } => match dir.0.is_sequencing() {
                true => {}
                false => match dir.0.is_unavailable() {
                    true => {
                        self.seq_dir = SeqDirState::Unavailable {
                            dir: UnavailableSeqDir::from(dir),
                            since: Duration::new(0, 0),
                        }
                    }
                    false => match dir.0.is_failed() {
                        true => {
                            self.seq_dir = SeqDirState::Failed {
                                dir: FailedSeqDir::from(dir),
                                since: Duration::new(0, 0),
                            }
                        }
                        false => match dir.0.is_copy_complete() {
                            true => {
                                self.seq_dir = SeqDirState::Available {
                                    dir: AvailableSeqDir::from(dir),
                                    since: Duration::new(0, 0),
                                }
                            }
                            false => {
                                self.seq_dir = SeqDirState::Transferring {
                                    dir: TransferringSeqDir::from(dir),
                                    since: Duration::new(0, 0),
                                }
                            }
                        },
                    },
                },
            },
            SeqDirState::Transferring { dir, .. } => match dir.0.is_copy_complete() {
                true => {
                    self.seq_dir = SeqDirState::Available {
                        dir: AvailableSeqDir::from(dir),
                        since: Duration::new(0, 0),
                    }
                }
                false => match dir.0.is_unavailable() {
                    true => {
                        self.seq_dir = SeqDirState::Unavailable {
                            dir: UnavailableSeqDir::from(dir),
                            since: Duration::new(0, 0),
                        }
                    }
                    false => match dir.0.is_failed() {
                        true => {
                            self.seq_dir = SeqDirState::Failed {
                                dir: FailedSeqDir::from(dir),
                                since: Duration::new(0, 0),
                            }
                        }
                        false => {} // assume we are still transferring
                    },
                },
            },
        }
    }

    pub fn state_since(&self) -> &Duration {
        self.seq_dir.since()
    }
}

// todo
//
// implement serialize for SeqDirState that provides a timestamped event e.g.
// {
// "event": "Available",
// "directory": Path,
// "timestamp": Duration
// }
