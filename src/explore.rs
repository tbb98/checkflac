use crate::types::{FlacJob, FlacStatus, JobFile, Statistics};
use anyhow::{Context, Result};
use chrono::Local;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

/// Explore a directory and create a job file with all FLAC files found
pub fn explore_directory(directory: PathBuf, output: Option<PathBuf>) -> Result<()> {
    println!(
        "{} Exploring directory: {}",
        "→".blue().bold(),
        directory.display()
    );

    // Check if the directory exists
    if !directory.exists() {
        anyhow::bail!("Directory does not exist: {}", directory.display());
    }

    if !directory.is_dir() {
        anyhow::bail!("Path is not a directory: {}", directory.display());
    }

    // Generate output filename if not provided
    let output = match output {
        Some(path) => path,
        None => generate_job_filename(&directory),
    };

    // Create a spinner for the directory scanning phase
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
    );
    spinner.set_message("Scanning directory tree...");

    // Find all FLAC files in the directory tree
    let flac_files = find_flac_files(&directory, &spinner)?;
    
    spinner.finish_and_clear();

    if flac_files.is_empty() {
        println!("{} No FLAC files found", "✗".red().bold());
        return Ok(());
    }

    println!(
        "{} Found {} FLAC files",
        "✓".green().bold(),
        flac_files.len()
    );

    // Create a progress bar for processing the files
    let pb = ProgressBar::new(flac_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message("Creating job entries...");

    // Use an atomic counter to track progress across threads
    let counter = Arc::new(AtomicUsize::new(0));

    // Create jobs for all FLAC files (all start as ToBeChecked)
    let jobs: Vec<FlacJob> = flac_files
        .into_par_iter() // Use parallel iterator for performance
        .map(|path| {
            let job = FlacJob {
                path,
                status: FlacStatus::ToBeChecked,
                error_message: None,
            };
            
            // Update progress bar (thread-safe)
            let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
            pb.set_position(count as u64);
            
            job
        })
        .collect();

    pb.finish_with_message("Done!");

    // Calculate statistics
    let statistics = Statistics::from_jobs(&jobs);

    // Create the job file structure
    let job_file = JobFile {
        root_directory: directory.clone(),
        total_files: jobs.len(),
        statistics,
        jobs,
    };

    // Serialize to JSON with pretty printing for human readability
    println!("{} Serializing job file...", "→".blue().bold());
    let json = serde_json::to_string_pretty(&job_file)
        .context("Failed to serialize job file to JSON")?;

    // Write to the output file
    fs::write(&output, json)
        .with_context(|| format!("Failed to write job file to {}", output.display()))?;

    println!(
        "{} Job file created: {}",
        "✓".green().bold(),
        output.display()
    );

    // Print summary statistics
    print_summary(&job_file);

    Ok(())
}

/// Find all FLAC files in a directory tree
/// Returns a vector of paths to FLAC files
fn find_flac_files(directory: &Path, spinner: &ProgressBar) -> Result<Vec<PathBuf>> {
    let mut flac_files = Vec::new();
    let mut file_count = 0;

    // WalkDir recursively walks through the directory tree
    // It's efficient and handles symlinks properly
    for entry in WalkDir::new(directory)
        .follow_links(false) // Don't follow symbolic links to avoid loops
        .into_iter()
        .filter_map(|e| e.ok()) // Skip entries that cause errors (permissions, etc.)
    {
        // Update spinner every 100 entries for performance
        file_count += 1;
        if file_count % 100 == 0 {
            spinner.set_message(format!("Scanning... (checked {} items)", file_count));
            spinner.tick();
        }

        // Check if this is a file (not a directory)
        if entry.file_type().is_file() {
            // Get the file path
            let path = entry.path();

            // Check if the extension is .flac (case-insensitive)
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("flac") {
                    flac_files.push(path.to_path_buf());
                    spinner.set_message(format!("Found {} FLAC files...", flac_files.len()));
                }
            }
        }
    }

    Ok(flac_files)
}

/// Generate a job filename based on the directory path
/// Sanitizes the path to only include alphanumeric characters, dashes, and underscores
/// Includes timestamp with second accuracy
fn generate_job_filename(directory: &Path) -> PathBuf {
    // Get the directory name (last component of the path)
    let dir_name = directory
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("checkflac");

    // Sanitize the directory name - keep only alphanumeric, dashes, and underscores
    let sanitized: String = dir_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == '-' || c == '_' {
                c
            } else {
                '_' // Replace any other character with underscore
            }
        })
        .collect();

    // Make sure it's not empty
    let sanitized = if sanitized.is_empty() {
        "checkflac".to_string()
    } else {
        sanitized
    };

    // Get current timestamp with second accuracy
    // Format: YYYYMMDD_HHMMSS (e.g., 20241115_143025)
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");

    // Create the filename: checkflac_<sanitized_dir_name>_<timestamp>_job.json
    PathBuf::from(format!("checkflac_{}_{}_job.json", sanitized, timestamp))
}

/// Print a summary of the job file statistics
fn print_summary(job_file: &JobFile) {
    println!("\n{}", "Summary:".bold().underline());
    println!("  Root directory: {}", job_file.root_directory.display());
    println!("  Total files:    {}", job_file.total_files);
    println!("\n{}", "Status breakdown:".bold());
    println!(
        "  {} To be checked: {}",
        "○".yellow(),
        job_file.statistics.to_be_checked
    );
    println!(
        "  {} Checking:      {}",
        "◐".cyan(),
        job_file.statistics.checking
    );
    println!("  {} OK:            {}", "✓".green(), job_file.statistics.ok);
    println!("  {} Bad:           {}", "✗".red(), job_file.statistics.bad);
    println!(
        "  {} Error:         {}",
        "⚠".yellow(),
        job_file.statistics.error
    );
}