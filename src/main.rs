// Import the clap derive macros for parsing command-line arguments
use clap::{Parser, Subcommand};
use std::path::PathBuf;

// Declare the modules - Rust will look for explore.rs, types.rs, check.rs, and stats.rs
mod check;
mod explore;
mod stats;
mod types;

/// A tool for checking FLAC file integrity
#[derive(Parser)]
#[command(name = "checkflac")]
#[command(about = "FLAC file integrity checker", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Explore a directory and create a job file with all FLAC files
    Explore {
        /// Directory to explore
        #[arg(value_name = "DIR")]
        directory: PathBuf,

        /// Output job file path (defaults to auto-generated based on directory name)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Check FLAC files from a job file
    Check {
        /// Job file to process
        #[arg(value_name = "JOB_FILE")]
        job_file: PathBuf,

        /// Number of parallel threads (defaults to number of CPU cores)
        #[arg(short, long)]
        threads: Option<usize>,

        /// Continue checking even if errors occur
        #[arg(short, long)]
        continue_on_error: bool,
    },
    /// Show statistics and lists of files by status
    Stats {
        /// Job file to analyze
        #[arg(value_name = "JOB_FILE")]
        job_file: PathBuf,

        /// Show list of OK files
        #[arg(long)]
        show_ok: bool,

        /// Show list of files to be checked
        #[arg(long)]
        show_pending: bool,

        /// Show full paths instead of relative paths
        #[arg(long)]
        full_paths: bool,
    },
}

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Explore { directory, output } => {
            // Run the explore command
            explore::explore_directory(directory, output)?;
        }
        Commands::Check {
            job_file,
            threads,
            continue_on_error,
        } => {
            // Run the check command
            check::check_flac_files(job_file, threads, continue_on_error)?;
        }
        Commands::Stats {
            job_file,
            show_ok,
            show_pending,
            full_paths,
        } => {
            // Run the stats command
            stats::show_statistics(job_file, show_ok, show_pending, full_paths)?;
        }
    }

    Ok(())
}