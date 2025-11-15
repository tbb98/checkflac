use crate::types::{FlacStatus, JobFile, Statistics};
use anyhow::{Context, Result};
use claxon::FlacReader;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use md5::{Digest, Md5};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Check FLAC files from a job file using parallel processing
pub fn check_flac_files(
    job_file_path: PathBuf,
    threads: Option<usize>,
    continue_on_error: bool,
) -> Result<()> {
    println!("{} Loading job file...", "→".blue().bold());

    // Read and parse the job file
    let job_file_content = fs::read_to_string(&job_file_path)
        .with_context(|| format!("Failed to read job file: {}", job_file_path.display()))?;

    let job_file: JobFile = serde_json::from_str(&job_file_content)
        .context("Failed to parse job file JSON")?;

    // Configure thread pool size
    let thread_count = threads.unwrap_or_else(num_cpus::get);
    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()
        .context("Failed to initialize thread pool")?;

    println!(
        "{} Using {} threads for parallel checking",
        "→".blue().bold(),
        thread_count
    );

    // Count how many files need to be checked
    // Files with status CHECKING will be re-checked (in case of previous interruption)
    let files_to_check: Vec<usize> = job_file
        .jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| {
            matches!(
                job.status,
                FlacStatus::ToBeChecked | FlacStatus::Checking | FlacStatus::Error
            )
        })
        .map(|(idx, _)| idx)
        .collect();

    if files_to_check.is_empty() {
        println!("{} No files to check!", "✓".green().bold());
        return Ok(());
    }

    println!(
        "{} Found {} files to check",
        "→".blue().bold(),
        files_to_check.len()
    );

    // Create progress bar
    let pb = ProgressBar::new(files_to_check.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );

    // Wrap the job file in Arc<Mutex<>> for thread-safe access
    let job_file = Arc::new(Mutex::new(job_file));

    // Process files in parallel
    let results: Vec<_> = files_to_check
        .into_par_iter()
        .map(|idx| {
            // Mark file as CHECKING before we start
            {
                let mut jf = job_file.lock().unwrap();
                jf.jobs[idx].status = FlacStatus::Checking;
                jf.jobs[idx].error_message = None;

                // Save the job file immediately to persist the CHECKING status
                if let Err(e) = save_job_file(&jf, &job_file_path) {
                    eprintln!("Warning: Failed to save job file: {}", e);
                }
            }

            // Get the file path to check
            let file_path = {
                let jf = job_file.lock().unwrap();
                jf.jobs[idx].path.clone()
            };

            // Perform the actual FLAC verification
            let check_result = verify_flac_file(&file_path);

            // Update the job status based on the result
            {
                let mut jf = job_file.lock().unwrap();
                match &check_result {
                    Ok(true) => {
                        jf.jobs[idx].status = FlacStatus::Ok;
                        jf.jobs[idx].error_message = None;
                    }
                    Ok(false) => {
                        jf.jobs[idx].status = FlacStatus::Bad;
                        jf.jobs[idx].error_message = Some("FLAC verification failed".to_string());
                    }
                    Err(e) => {
                        jf.jobs[idx].status = FlacStatus::Error;
                        jf.jobs[idx].error_message = Some(e.to_string());
                    }
                }

                // Save job file after each update (slower but safer in case of interruption)
                if let Err(e) = save_job_file(&jf, &job_file_path) {
                    eprintln!("Warning: Failed to save job file: {}", e);
                }
            }

            // Update progress bar
            pb.inc(1);

            check_result
        })
        .collect();

    pb.finish_with_message("Done!");

    // Final save and statistics update
    {
        let mut jf = job_file.lock().unwrap();
        jf.statistics = Statistics::from_jobs(&jf.jobs);
        save_job_file(&jf, &job_file_path)?;
    }

    // Print summary
    let jf = job_file.lock().unwrap();
    print_check_summary(&jf);

    // Check if we should fail on errors
    if !continue_on_error {
        let error_count = results.iter().filter(|r| r.is_err()).count();
        let bad_count = results
            .iter()
            .filter(|r| matches!(r, Ok(false)))
            .count();

        if error_count > 0 || bad_count > 0 {
            anyhow::bail!(
                "Check completed with {} errors and {} bad files",
                error_count,
                bad_count
            );
        }
    }

    Ok(())
}

/// Verify a FLAC file by:
/// 1. Decoding all frames
/// 2. Computing MD5 hash of decoded audio
/// 3. Comparing with MD5 stored in FLAC header
/// Returns Ok(true) if file is valid, Ok(false) if corrupted, Err on other errors
fn verify_flac_file(path: &PathBuf) -> Result<bool> {
    // Open the FLAC file using claxon
    let mut reader = FlacReader::open(path)
        .with_context(|| format!("Failed to open FLAC file: {}", path.display()))?;

    // Get stream info which contains the expected MD5
    let streaminfo = reader.streaminfo();
    let expected_md5 = streaminfo.md5sum;

    // If MD5 is all zeros, it means no MD5 was stored
    let has_md5 = expected_md5.iter().any(|&b| b != 0);

    // Prepare MD5 hasher for computed checksum
    let mut hasher = Md5::new();

    // Get sample information
    let bits_per_sample = streaminfo.bits_per_sample;

    // Create a buffer to hold samples
    let mut samples = Vec::new();

    // Decode all samples using the samples() iterator
    for sample_result in reader.samples() {
        match sample_result {
            Ok(sample) => {
                samples.push(sample);
            }
            Err(e) => {
                // Any error means the file is corrupted or invalid
                return Err(anyhow::anyhow!(
                    "FLAC decoding error: {}",
                    e
                ));
            }
        }
    }

    // Now compute MD5 from the samples
    // MD5 is computed on the raw audio data in the file's native format
    // We need to convert samples to bytes in the proper format
    for &sample in &samples {
        // Convert sample to bytes based on bits_per_sample
        match bits_per_sample {
            8 => {
                // 8-bit samples are unsigned
                let byte = (sample + 128) as u8;
                hasher.update(&[byte]);
            }
            16 => {
                // 16-bit samples, little-endian
                let bytes = (sample as i16).to_le_bytes();
                hasher.update(&bytes);
            }
            24 => {
                // 24-bit samples, stored in 3 bytes little-endian
                let bytes = sample.to_le_bytes();
                hasher.update(&bytes[0..3]);
            }
            32 => {
                // 32-bit samples
                let bytes = sample.to_le_bytes();
                hasher.update(&bytes);
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported bits per sample: {}",
                    bits_per_sample
                ));
            }
        }
    }

    // Finalize MD5 hash
    let computed_md5: [u8; 16] = hasher.finalize().into();

    // Compare MD5 if available
    if has_md5 {
        if computed_md5 == expected_md5 {
            Ok(true) // File is valid
        } else {
            // MD5 mismatch - file is corrupted
            Ok(false)
        }
    } else {
        // No MD5 in header, but file decoded successfully
        // Consider it OK since we at least verified it decodes
        Ok(true)
    }
}

/// Save the job file to disk
fn save_job_file(job_file: &JobFile, path: &PathBuf) -> Result<()> {
    let json = serde_json::to_string_pretty(job_file)
        .context("Failed to serialize job file")?;
    
    fs::write(path, json)
        .with_context(|| format!("Failed to write job file to {}", path.display()))?;
    
    Ok(())
}

/// Print a summary of the check results
fn print_check_summary(job_file: &JobFile) {
    println!("\n{}", "Check Summary:".bold().underline());
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
    println!(
        "  {} OK:            {}",
        "✓".green().bold(),
        job_file.statistics.ok
    );
    println!(
        "  {} Bad:           {}",
        "✗".red().bold(),
        job_file.statistics.bad
    );
    println!(
        "  {} Error:         {}",
        "⚠".yellow().bold(),
        job_file.statistics.error
    );

    // Show percentage
    if job_file.total_files > 0 {
        let ok_percent = (job_file.statistics.ok as f64 / job_file.total_files as f64) * 100.0;
        println!("\n  Success rate: {:.1}%", ok_percent);
    }
}