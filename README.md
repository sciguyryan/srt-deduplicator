# srt-deduplicator

A small command line tool that compacts consecutive runs of identical subtitle
text in SubRip (`.srt`) files.

Auto-generated transcripts tend to repeat the same line over several
consecutive cues while it is still on screen. This tool merges each such run
into a single cue that keeps the run's first start timestamp and its last end
timestamp, so the text still appears for the whole span it covered. Cues whose
neighbours differ are left alone.

## Usage

```console
$ srt-deduplicator input.srt
```

The compacted subtitles go to standard output, which makes the tool usable in
a pipeline. With `-o FILE` they are written to a file instead; the input file
is never modified.

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

## Matching rules

The rules are deliberately simple, and it is worth knowing where they stop:

- Two adjacent cues are considered identical when their text matches after
  surrounding whitespace is trimmed and whitespace runs, including line
  breaks, collapse to single spaces. Comparison is case-sensitive, so `Hello`
  and `hello` remain separate cues.
- Only strictly consecutive cues merge. Text that repeats later in the file
  but is separated by other cues is untouched.
- A run keeps its first member's text verbatim, including any line breaks.
- Timestamps are taken as found: the merged cue spans the first start and the
  last end of the run. Timestamps are not required to be contiguous.
- Blank cues are treated like any other text, so two adjacent blank cues merge
  into one.
- Cue indices are renumbered sequentially from 1 in the output.
- Output uses Unix line endings, cues separated by a blank line and a single
  trailing newline.

## Input handling

Input is expected to be UTF-8; a byte order mark is tolerated and invalid
UTF-8 is rejected with an error. Timestamps written with a dot instead of a
comma are accepted when reading, and position information after the
timestamps (for example `X1:10 Y1:20`) is parsed and then dropped, so it does
not survive into the output.

A malformed SRT file, a missing file or an unwritable output path produces a
single diagnostic on standard error and a non-zero exit status. Nothing is
written on failure.

## Building and testing

The tool is a standard cargo project:

```console
$ cargo build --release
$ cargo test
```

## Dependencies

Two libraries are used, both chosen to avoid rewriting solved problems:
[srtlib](https://crates.io/crates/srtlib) for SRT parsing and formatting,
including the timestamp and line-ending quirks covered above, and
[clap](https://crates.io/crates/clap) for command line handling. The
compaction logic itself is in this repository.
