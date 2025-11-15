use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Status of a FLAC file check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FlacStatus {
    /// Not yet checked
    ToBeChecked,
    /// Currently being checked
    Checking,
    /// Check passed successfully
    Ok,
    /// File is corrupted or invalid
    Bad,
    /// An error occurred during checking
    Error,
}

/// Represents a single FLAC file to be checked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlacJob {
    /// Full path to the FLAC file
    pub path: PathBuf,
    /// Current status of this file
    pub status: FlacStatus,
    /// Optional error message if status is Error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Container for all FLAC jobs in a directory
#[derive(Debug, Serialize, Deserialize)]
pub struct JobFile {
    /// Root directory that was scanned
    pub root_directory: PathBuf,
    /// Total number of FLAC files found
    pub total_files: usize,
    /// Statistics by status
    pub statistics: Statistics,
    /// List of all FLAC files to check
    pub jobs: Vec<FlacJob>,
}

/// Statistics about the job file
#[derive(Debug, Serialize, Deserialize)]
pub struct Statistics {
    pub to_be_checked: usize,
    pub checking: usize,
    pub ok: usize,
    pub bad: usize,
    pub error: usize,
}

impl Statistics {
    /// Create new statistics from a list of jobs
    pub fn from_jobs(jobs: &[FlacJob]) -> Self {
        let mut stats = Statistics {
            to_be_checked: 0,
            checking: 0,
            ok: 0,
            bad: 0,
            error: 0,
        };

        // Count each status type
        for job in jobs {
            match job.status {
                FlacStatus::ToBeChecked => stats.to_be_checked += 1,
                FlacStatus::Checking => stats.checking += 1,
                FlacStatus::Ok => stats.ok += 1,
                FlacStatus::Bad => stats.bad += 1,
                FlacStatus::Error => stats.error += 1,
            }
        }

        stats
    }
}
