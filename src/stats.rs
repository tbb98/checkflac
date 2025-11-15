use crate::types::{FlacStatus, JobFile};
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::PathBuf;

/// Show statistics and lists of files by status from a job file
pub fn show_statistics(
    job_file_path: PathBuf,
    show_ok: bool,
    show_pending: bool,
    full_paths: bool,
) -> Result<()> {
    println!("{} Loading job file...", "→".blue().bold());

    // Read and parse the job file
    let job_file_content = fs::read_to_string(&job_file_path)
        .with_context(|| format!("Failed to read job file: {}", job_file_path.display()))?;

    let mut job_file: JobFile = serde_json::from_str(&job_file_content)
        .context("Failed to parse job file JSON")?;

    // Recalculate statistics from actual job statuses
    // (in case the JSON file's statistics are outdated)
    job_file.statistics = crate::types::Statistics::from_jobs(&job_file.jobs);

    // Print summary (same as explore command)
    print_summary(&job_file);

    // Collect files by status
    let mut bad_files = Vec::new();
    let mut error_files = Vec::new();
    let mut ok_files = Vec::new();
    let mut pending_files = Vec::new();

    for job in &job_file.jobs {
        // Get the path to display (full or relative to root)
        let display_path = if full_paths {
            job.path.display().to_string()
        } else {
            // Try to strip the root directory prefix
            match job.path.strip_prefix(&job_file.root_directory) {
                Ok(relative) => relative.display().to_string(),
                Err(_) => job.path.display().to_string(),
            }
        };

        match job.status {
            FlacStatus::Bad => bad_files.push((display_path, job.error_message.clone())),
            FlacStatus::Error => error_files.push((display_path, job.error_message.clone())),
            FlacStatus::Ok => ok_files.push(display_path),
            FlacStatus::ToBeChecked | FlacStatus::Checking => pending_files.push(display_path),
        }
    }

    // Print BAD files list (always shown)
    if !bad_files.is_empty() {
        println!("\n{}", "BAD Files (corrupted):".red().bold());
        for (path, error_msg) in &bad_files {
            println!("  {} {}", "✗".red(), path);
            if let Some(msg) = error_msg {
                println!("    {}: {}", "Reason".dimmed(), msg.dimmed());
            }
        }
    }

    // Print ERROR files list (always shown)
    if !error_files.is_empty() {
        println!("\n{}", "ERROR Files (could not check):".yellow().bold());
        for (path, error_msg) in &error_files {
            println!("  {} {}", "⚠".yellow(), path);
            if let Some(msg) = error_msg {
                println!("    {}: {}", "Error".dimmed(), msg.dimmed());
            }
        }
    }

    // Print OK files list (optional)
    if show_ok && !ok_files.is_empty() {
        println!("\n{}", "OK Files (verified):".green().bold());
        for path in &ok_files {
            println!("  {} {}", "✓".green(), path);
        }
    } else if !ok_files.is_empty() {
        println!(
            "\n{} {} OK files (use {} to list them)",
            "→".blue(),
            ok_files.len(),
            "--show-ok".cyan()
        );
    }

    // Print pending files list (optional)
    if show_pending && !pending_files.is_empty() {
        println!(
            "\n{}",
            "Pending Files (to be checked):".yellow().bold()
        );
        for path in &pending_files {
            println!("  {} {}", "○".yellow(), path);
        }
    } else if !pending_files.is_empty() {
        println!(
            "\n{} {} pending files (use {} to list them)",
            "→".blue(),
            pending_files.len(),
            "--show-pending".cyan()
        );
    }

    // Summary message
    println!();
    if bad_files.is_empty() && error_files.is_empty() {
        if pending_files.is_empty() {
            println!("{} All files verified successfully!", "✓".green().bold());
        } else {
            println!(
                "{} No issues found in checked files. {} files pending.",
                "✓".green().bold(),
                pending_files.len()
            );
        }
    } else {
        println!(
            "{} Found {} bad and {} error files.",
            "⚠".yellow().bold(),
            bad_files.len(),
            error_files.len()
        );
    }

    Ok(())
}

/// Print a summary of the job file (same as explore command)
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

    // Show percentage if any files have been checked
    let checked_files = job_file.statistics.ok + job_file.statistics.bad + job_file.statistics.error;
    if checked_files > 0 {
        let ok_percent = (job_file.statistics.ok as f64 / checked_files as f64) * 100.0;
        println!("\n  Success rate: {:.1}%", ok_percent);
    }
}