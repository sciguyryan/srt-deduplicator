//! Command line definition for the subtitle-deduplicator binary.

use std::path::PathBuf;

use clap::Parser;

/// Command line arguments accepted by the tool.
#[derive(Parser, Debug)]
#[command(
    name = "subtitle-deduplicator",
    version,
    about = "Compact consecutive runs of identical subtitle text in an SRT or ASS file.",
    long_about = "Compact consecutive runs of identical subtitle text in a \
        SubRip (SRT) or Advanced SubStation Alpha (ASS) file. Each run is merged into a single cue that keeps \
        the run's first start timestamp and its last end timestamp. Cues \
        whose neighbours differ are left alone. SRT cue indices are renumbered \
        sequentially from 1. ASS merges require matching non-timing fields; \
        dialogue with override blocks is preserved without merging."
)]
pub struct Cli {
    /// The SRT or ASS file to compact. Expected to be UTF-8; a byte order mark is
    /// tolerated.
    pub input: PathBuf,

    /// Write the compacted subtitles to this file instead of standard
    /// output.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}
