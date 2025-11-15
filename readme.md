```
⚠️ Warning: This is a vibe-coded piece of crap!
It seems to work. Use at your own risk. No responsibility accepted.  
```

# checkflac

A **FLAC file integrity checker** for verifying large collections of FLAC audio files. It scans directories, creates job files, checks files in parallel, and reports results with detailed statistics. It will not modify your FLAC files.

---

## Features

* **Directory scanning**: Recursively find all .flac files.
* **Job file creation**: Save discovered files in a JSON “job file” for later checking.
* **Parallel FLAC verification**: Uses all CPU cores (or configurable threads) to check files efficiently.
* **FLAC integrity checks**:
  * Decodes the audio completely.
  * Computes MD5 hash of raw audio and compares it to the FLAC file’s header MD5 (if present).
* **Safe incremental updates**: Saves job file after each file check to handle interruptions.
* **Statistics and summaries**: Shows counts for OK, Bad, Error, and pending files.

---

## Installation

Requires [Rust toolchain](https://rustup.rs/)

```bash
git clone https://github.com/tbb98/checkflac.git
cd checkflac
cargo build --release
```
The compiled binary will be in `target/release/checkflac`



## Usage

### Explore a directory

Create a job file from a directory containing FLAC files:

```bash
checkflac explore <DIR> [--output <JOB_FILE>]
```

* `<DIR>` — directory to scan
* `--output` — optional output path for the job file (defaults to auto-generated filename)

Example:

```bash
checkflac explore "M:\Music FLAC"
```

Produces a JSON job file like `checkflac_my_music_20251115_123456_job.json`

---

### Check FLAC files

Run integrity checks on a job file:

```bash
checkflac check <JOB_FILE> [--threads <N>] [--continue-on-error]
```

* `<JOB_FILE>` — previously generated job file
* `--threads <N>` — optional number of threads to use (default: CPU cores)
* `--continue-on-error` — continues checking even if some files fail

Example:

```bash
checkflac check checkflac_my_music_20251115_123456_job.json
```

---

### View statistics

View detailed statistics and optionally list files by status:

```bash
checkflac stats <JOB_FILE> [--show-ok] [--show-pending] [--full-paths]
```

* `--show-ok` — display OK files
* `--show-pending` — display files still to be checked
* `--full-paths` — show full file paths instead of relative paths

---

## How the FLAC check works

1. **Decoding**: Each FLAC file is fully decoded using [claxon](https://docs.rs/claxon/latest/claxon/)
2. **MD5 verification**:

   * The FLAC file header (STREAMINFO block) **may contain an MD5 checksum** of the raw audio data.
   * If present, the computed MD5 of the decoded audio is compared to the header.
3. **Result classification**:

| Status      | Meaning                                                                                                |
| ----------- | -------------------------------------------------------------------------------------------------------|
| OK          | File decoded successfully, MD5 matches (or no MD5 in header)                                           |
| Bad         | File decoded but MD5 does **not** match → **likely corrupted audio**                                   |
| Error       | File could not be decoded (→ **likely corrupted audio**), is unreadable, or has an unsupported format  |
| ToBeChecked | File has not been processed yet                                                                        |
| Checking    | File is currently being checked                                                                        |

* Any errors during decoding (e.g., malformed frames) mark a file as **Error**
* MD5 mismatch files are **Bad**, even if the audio can technically play
* Running the check again will try to re-check the errored out files again

---

## Job File Structure

Job files are JSON files containing:

```json
{
  "root_directory": "/music/flac",
  "total_files": 120,
  "statistics": {
    "to_be_checked": 0,
    "checking": 0,
    "ok": 110,
    "bad": 5,
    "error": 5
  },
  "jobs": [
    {
      "path": "/music/flac/album1/song1.flac",
      "status": "OK",
      "error_message": null
    },
    {
      "path": "/music/flac/album1/song2.flac",
      "status": "Bad",
      "error_message": "FLAC verification failed"
    }
  ]
}
```

---

## Implementation Notes

* **Parallel processing**: Uses [rayon](https://docs.rs/rayon/latest/rayon/) to fully utilize CPU cores.
* **Thread safety**: `Arc<Mutex<JobFile>>` ensures safe concurrent updates.
* **Incremental saves**: Saves the job file after each file update to avoid losing progress on interruption.
* **Progress display**: Uses [indicatif](https://docs.rs/indicatif/latest/indicatif/) for progress bars and spinners.
* **Error handling**: Uses [anyhow](https://docs.rs/anyhow/latest/anyhow/) for detailed error reporting.

---

