//! subtitle-deduplicator compacts consecutive runs of identical subtitle text in
//! SubRip (SRT) and Advanced SubStation Alpha (ASS) files.
//!
//! Auto-generated transcripts often repeat the same line over several
//! consecutive cues while it is still on screen. This binary merges each such
//! run into a single cue spanning the run's first start and last end
//! timestamp. See `dedup` for the compaction rules and the README for the
//! observable behaviour, including its limitations.

mod ass;
mod cli;
mod dedup;

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use srtlib::Subtitle;

use crate::cli::Cli;
use crate::dedup::compact;

/// Errors the tool can fail with, each mapped to a single diagnostic line.
#[derive(Debug)]
enum AppError {
    /// The input file could not be read, including invalid UTF-8 content.
    Read { path: PathBuf, source: io::Error },
    /// The input file could not be parsed.
    Parse {
        path: PathBuf,
        format: &'static str,
        source: String,
    },
    /// The requested output file could not be written.
    Write { path: PathBuf, source: io::Error },
    /// Writing to standard output failed.
    WriteStdout { source: io::Error },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Read { path, source } => {
                write!(f, "cannot read {}: {}", path.display(), source)
            }
            AppError::Parse {
                path,
                format,
                source,
            } => {
                write!(
                    f,
                    "{} is not a valid {} file: {}",
                    path.display(),
                    format,
                    source
                )
            }
            AppError::Write { path, source } => {
                write!(f, "cannot write {}: {}", path.display(), source)
            }
            AppError::WriteStdout { source } => {
                write!(f, "cannot write to standard output: {}", source)
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", error);
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), AppError> {
    // Reading as a string rather than using srtlib's file reader keeps input
    // decoding strict: invalid UTF-8 is rejected here instead of being
    // silently replaced during the crate's own decoding.
    let text = fs::read_to_string(&cli.input).map_err(|source| AppError::Read {
        path: cli.input.clone(),
        source,
    })?;

    let is_ass = cli
        .input
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ass"));
    let output = if is_ass {
        ass::compact(&text).map_err(|source| AppError::Parse {
            path: cli.input.clone(),
            format: "ASS",
            source,
        })?
    } else {
        let mut subtitles = srtlib::Subtitles::parse_from_str(text)
            .map_err(|source| AppError::Parse {
                path: cli.input.clone(),
                format: "SRT",
                source: source.to_string(),
            })?
            .to_vec();
        trim_last_cue_newline(subtitles.last_mut());
        let (compacted, _) = compact(subtitles);
        format_output(&compacted)
    };
    match &cli.output {
        Some(path) => fs::write(path, output).map_err(|source| AppError::Write {
            path: path.clone(),
            source,
        }),
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle
                .write_all(output.as_bytes())
                .and_then(|()| handle.flush())
                .map_err(|source| AppError::WriteStdout { source })
        }
    }
}

/// Strips one trailing line break from the final cue's text.
///
/// A file that ends with a single newline leaves that newline inside the
/// last cue's text, while a file ending with a blank line parses cleanly.
/// Stripping the break makes the last cue match how every other cue is
/// read and keeps the output to one trailing newline.
fn trim_last_cue_newline(last: Option<&mut Subtitle>) {
    let Some(subtitle) = last else {
        return;
    };
    if let Some(stripped) = subtitle.text.strip_suffix('\n') {
        let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
        subtitle.text = stripped.to_string();
    }
}

/// Renders subtitles as SRT text: cues separated by a blank line and a single
/// trailing newline.
///
/// The tool formats output itself rather than relying on the crate's writer
/// so that standard output and file output are byte-identical.
fn format_output(subtitles: &[Subtitle]) -> String {
    let mut rendered = subtitles
        .iter()
        .map(|subtitle| subtitle.to_string())
        .collect::<Vec<String>>()
        .join("\n\n");
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}
