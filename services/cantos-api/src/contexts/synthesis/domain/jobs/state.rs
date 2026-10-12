//! Run transitions and their persisted spellings.
use super::JobsError;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for RunStatus {
    type Err = JobsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "paused" => Ok(Self::Paused),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(JobsError::InvalidRunStatus),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEvent {
    Start,
    Complete,
    Pause,
    Resume,
    Fail,
    Retry,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError {
    pub status: RunStatus,
    pub event: RunEvent,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid run transition: {:?} + {:?}",
            self.status, self.event
        )
    }
}

impl Error for TransitionError {}

/// The shell checks completion facts and scoped authorization before committing a transition.
/// Resume and explicit retry requeue work; a worker claim starts it under a new fence.
/// Completed and cancelled are terminal. A replacement is a separate run.
pub fn evolve_run(status: RunStatus, event: RunEvent) -> Result<RunStatus, TransitionError> {
    let next = match status {
        RunStatus::Queued => match event {
            RunEvent::Start => Some(RunStatus::Running),
            RunEvent::Pause => Some(RunStatus::Paused),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Complete | RunEvent::Resume | RunEvent::Fail | RunEvent::Retry => None,
        },
        RunStatus::Running => match event {
            RunEvent::Complete => Some(RunStatus::Completed),
            RunEvent::Pause => Some(RunStatus::Paused),
            RunEvent::Fail => Some(RunStatus::Failed),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start | RunEvent::Resume | RunEvent::Retry => None,
        },
        RunStatus::Paused => match event {
            RunEvent::Resume => Some(RunStatus::Queued),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Fail
            | RunEvent::Retry => None,
        },
        RunStatus::Failed => match event {
            RunEvent::Retry => Some(RunStatus::Queued),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Resume
            | RunEvent::Fail => None,
        },
        RunStatus::Completed | RunStatus::Cancelled => match event {
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Resume
            | RunEvent::Fail
            | RunEvent::Retry
            | RunEvent::Cancel => None,
        },
    };
    next.ok_or(TransitionError { status, event })
}
