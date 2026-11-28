# subtitle-deduplicator

A small command line tool that compacts consecutive runs of identical subtitle text in SubRip (`.srt`) and Advanced SubStation Alpha (`.ass`) files.

Auto-generated transcripts tend to repeat the same line over several consecutive cues while it is still on screen. This tool merges each such run into a single cue that keeps the run's first start timestamp and its last end timestamp, so the text still appears for the whole span it covered. Cues whose neighbours differ are left alone.

## Usage

```console
$ subtitle-deduplicator input.srt
$ subtitle-deduplicator input.ass -o compacted.ass
```

The compacted subtitles go to standard output, which makes the tool usable in a pipeline. With `-o FILE` they are written to a file instead; the input file is never modified.

For a file like this:

```srt
1
00:00:01,000 --> 00:00:02,000
Hello

2
00:00:02,000 --> 00:00:03,000
Hello

3
00:00:03,000 --> 00:00:04,000
Hello

4
00:00:05,000 --> 00:00:06,000
World
```

the output is:

```srt
1
00:00:01,000 --> 00:00:04,000
Hello

2
00:00:05,000 --> 00:00:06,000
World
```

## SRT matching rules

The rules are deliberately simple, and it is worth knowing where they stop:

- Two adjacent cues are considered identical when their text matches after surrounding whitespace is trimmed and whitespace runs, including line breaks, collapse to single spaces. Comparison is case-sensitive, so `Hello` and `hello` remain separate cues.
- Only strictly consecutive cues merge. Text that repeats later in the file but is separated by other cues is untouched.
- A run keeps its first member's text verbatim, including any line breaks.
- Timestamps are taken as found: the merged cue spans the first start and the last end of the run. Timestamps are not required to be contiguous.
- Blank cues are treated like any other text, so two adjacent blank cues merge into one.
- Cue indices are renumbered sequentially from 1 in the output.
- Output uses Unix line endings, cues separated by a blank line and a single trailing newline.

## ASS matching rules

Files with a `.ass` extension (case-insensitive) are read as ASS; other filenames are read as SRT. Output retains the input format regardless of the output filename. ASS files need an `[Events]` section and an event `Format:` declaration before any `Dialogue:` records. The declaration must contain unique `Start`, `End` and `Text` fields, with `Text` last; the other fields retain their declared order. Timestamps use `H:MM:SS.cc` notation.

Adjacent dialogue records merge when their text matches using the SRT whitespace rules and every non-timing field matches exactly. This includes style, layer, speaker, margins, effects and any additional fields. The merged record keeps the first record's text, start time and formatting and takes the last record's end time. Commas in dialogue text are preserved. Literal ASS escapes such as `\N`, `\n` and `\h` remain significant during comparison.

Dialogue containing an override block (`{...}`) is preserved without merging because overrides may contain animation, karaoke timing or drawing instructions. Comments, blank lines, section boundaries and other records also break runs. Script information, styles, attachments and other non-dialogue records are retained. Output uses Unix line endings and ends with a newline; an input byte order mark is removed.

## Input handling

Input is expected to be UTF-8; a byte order mark is tolerated and invalid UTF-8 is rejected with an error. For SRT, timestamps written with a dot instead of a comma are accepted when reading, and position information after the timestamps (for example `X1:10 Y1:20`) is parsed and then dropped, so it does not survive into the output.

A malformed subtitle file, a missing file or an unwritable output path produces a single diagnostic on standard error and a non-zero exit status. Nothing is written on failure.

## Building and testing

The tool is a standard cargo project:

```console
$ cargo build --release
$ cargo test
```

## Releases

Pushing a `v*` tag produces a GitHub Release automatically. Every supported target is built and packaged into one archive containing the binary, this README and the licence, and all archives plus a SHA256 checksum file are attached to the release. Untagged pushes and pull requests build every target as well, with the bare binaries available as workflow artifacts.

The release archives use the naming scheme `subtitle-deduplicator-<version>-<target>.zip`, where the target is the Rust target triple, for example `x86_64-unknown-linux-musl`.

## Dependencies

Two libraries are used, both chosen to avoid reinventing the wheel of solved problems: [srtlib](https://crates.io/crates/srtlib) for SRT parsing and formatting, including the timestamp and line-ending quirks covered above, and [clap](https://crates.io/crates/clap) for command line handling. ASS event parsing and the compaction logic are in this repository.
