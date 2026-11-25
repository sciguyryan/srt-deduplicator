//! Compaction of consecutive subtitle runs with identical text.
//!
//! The pass is deliberately pure: it takes owned cues and returns the
//! compacted list plus counters, so it can be tested without touching the
//! command line layer or the file system.

use srtlib::Subtitle;

/// Counters describing what a compaction pass changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Cues present before compaction.
    pub input_cues: usize,
    /// Cues present after compaction.
    pub output_cues: usize,
    /// Runs of two or more identical consecutive cues that were merged.
    pub runs_compacted: usize,
}

/// Compacts consecutive runs of identical cue text.
///
/// A run is a maximal stretch of adjacent cues whose normalised text is equal.
/// The merged cue keeps the first member's text and start time and adopts the
/// end time of the last member, so the text stays on screen for the whole span
/// the repetition covered. Timestamps are not required to be contiguous or
/// monotonic: the first start and last end are taken as found.
///
/// Cue indices are renumbered sequentially from 1 afterwards, which is why
/// the function takes and returns owned cues rather than editing in place.
pub fn compact(cues: Vec<Subtitle>) -> (Vec<Subtitle>, Stats) {
    let input_cues = cues.len();
    let mut compacted: Vec<Subtitle> = Vec::with_capacity(input_cues);
    let mut runs_compacted = 0;
    // Length of the run currently being built at the end of `compacted`.
    let mut current_run_length = 0;

    for cue in cues {
        let key = normalise(&cue.text);
        let identical_to_previous =
            current_run_length > 0 && normalise(&compacted[compacted.len() - 1].text) == key;

        if identical_to_previous {
            compacted.last_mut().unwrap().end_time = cue.end_time;
            current_run_length += 1;
        } else {
            if current_run_length >= 2 {
                runs_compacted += 1;
            }
            compacted.push(cue);
            current_run_length = 1;
        }
    }
    if current_run_length >= 2 {
        runs_compacted += 1;
    }

    for (index, cue) in compacted.iter_mut().enumerate() {
        cue.num = index + 1;
    }

    let stats = Stats {
        input_cues,
        output_cues: compacted.len(),
        runs_compacted,
    };
    (compacted, stats)
}

/// Normalises cue text for comparison.
///
/// Surrounding whitespace is trimmed and whitespace runs, including line
/// breaks, collapse to single spaces. Case stays significant: "Hello" and
/// "hello" are different lines more often than not, and changing that would
/// be a policy decision the caller did not ask for.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    use srtlib::Timestamp;

    /// Builds a cue with its text and `HH:MM:SS,mmm` timestamps.
    fn cue(num: usize, start: &str, end: &str, text: &str) -> Subtitle {
        Subtitle::new(num, timestamp(start), timestamp(end), text.to_string())
    }

    /// Parses a zero-padded `HH:MM:SS,mmm` timestamp.
    fn timestamp(input: &str) -> Timestamp {
        let (clock, milliseconds) = input.split_once(',').expect("comma in timestamp");
        let (hours, rest) = clock.split_once(':').expect("hour separator");
        let (minutes, seconds) = rest.split_once(':').expect("minute separator");
        Timestamp::new(
            hours.parse().expect("hours"),
            minutes.parse().expect("minutes"),
            seconds.parse().expect("seconds"),
            milliseconds.parse().expect("milliseconds"),
        )
    }

    /// Renders a compacted list for assertions, one cue per line.
    fn render(cues: &[Subtitle]) -> Vec<String> {
        cues.iter()
            .map(|cue| {
                format!(
                    "{} {} {} {:?}",
                    cue.num, cue.start_time, cue.end_time, cue.text
                )
            })
            .collect()
    }

    #[test]
    fn merges_a_run_into_one_cue_spanning_its_timestamps() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(2, "00:00:02,000", "00:00:03,000", "Hello"),
            cue(3, "00:00:03,000", "00:00:04,000", "Hello"),
        ];

        let (compacted, stats) = compact(cues);

        assert_eq!(
            render(&compacted),
            vec!["1 00:00:01,000 00:00:04,000 \"Hello\"".to_string()],
        );
        assert_eq!(
            stats,
            Stats {
                input_cues: 3,
                output_cues: 1,
                runs_compacted: 1,
            },
        );
    }

    #[test]
    fn different_text_breaks_a_run() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(2, "00:00:02,000", "00:00:03,000", "World"),
            cue(3, "00:00:03,000", "00:00:04,000", "Hello"),
        ];

        let (compacted, stats) = compact(cues);

        // Repeated text separated by another cue is not a run and stays put.
        assert_eq!(compacted.len(), 3);
        assert_eq!(
            stats,
            Stats {
                input_cues: 3,
                output_cues: 3,
                runs_compacted: 0,
            },
        );
    }

    #[test]
    fn whitespace_differences_still_merge() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Hello world"),
            cue(2, "00:00:02,000", "00:00:03,000", "Hello  world"),
            cue(3, "00:00:03,000", "00:00:04,000", " Hello world "),
        ];

        let (compacted, _) = compact(cues);

        assert_eq!(
            render(&compacted),
            vec!["1 00:00:01,000 00:00:04,000 \"Hello world\"".to_string()],
        );
    }

    #[test]
    fn case_differences_do_not_merge() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(2, "00:00:02,000", "00:00:03,000", "hello"),
        ];

        let (compacted, stats) = compact(cues);

        assert_eq!(compacted.len(), 2);
        assert_eq!(stats.runs_compacted, 0);
    }

    #[test]
    fn blank_cues_merge_like_any_other() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", ""),
            cue(2, "00:00:02,000", "00:00:03,000", ""),
            cue(3, "00:00:03,000", "00:00:04,000", "World"),
        ];

        let (compacted, stats) = compact(cues);

        assert_eq!(compacted.len(), 2);
        assert_eq!(stats.runs_compacted, 1);
        assert_eq!(compacted[0].start_time, timestamp("00:00:01,000"));
        assert_eq!(compacted[0].end_time, timestamp("00:00:03,000"));
    }

    #[test]
    fn multi_line_text_is_compared_and_preserved() {
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Line one\nLine two"),
            cue(2, "00:00:02,000", "00:00:03,000", "Line one\nLine two"),
        ];

        let (compacted, stats) = compact(cues);

        assert_eq!(stats.runs_compacted, 1);
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].text, "Line one\nLine two");
    }

    #[test]
    fn indices_are_renumbered_sequentially() {
        let cues = vec![
            cue(5, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(9, "00:00:02,000", "00:00:03,000", "Hello"),
            cue(12, "00:00:03,000", "00:00:04,000", "World"),
        ];

        let (compacted, _) = compact(cues);

        assert_eq!(
            render(&compacted),
            vec![
                "1 00:00:01,000 00:00:03,000 \"Hello\"".to_string(),
                "2 00:00:03,000 00:00:04,000 \"World\"".to_string(),
            ],
        );
    }

    #[test]
    fn end_time_comes_from_the_last_cue_of_the_run() {
        // Non-contiguous timestamps are taken as found rather than repaired.
        let cues = vec![
            cue(1, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(2, "00:00:05,000", "00:00:06,000", "Hello"),
        ];

        let (compacted, _) = compact(cues);

        assert_eq!(
            render(&compacted),
            vec!["1 00:00:01,000 00:00:06,000 \"Hello\"".to_string()],
        );
    }

    #[test]
    fn a_file_without_runs_is_unchanged_apart_from_renumbering() {
        let cues = vec![
            cue(7, "00:00:01,000", "00:00:02,000", "Hello"),
            cue(3, "00:00:03,000", "00:00:04,000", "World"),
        ];

        let (compacted, stats) = compact(cues);

        assert_eq!(
            render(&compacted),
            vec![
                "1 00:00:01,000 00:00:02,000 \"Hello\"".to_string(),
                "2 00:00:03,000 00:00:04,000 \"World\"".to_string(),
            ],
        );
        assert_eq!(
            stats,
            Stats {
                input_cues: 2,
                output_cues: 2,
                runs_compacted: 0,
            },
        );
    }

    #[test]
    fn an_empty_input_produces_an_empty_output() {
        let (compacted, stats) = compact(Vec::new());

        assert!(compacted.is_empty());
        assert_eq!(
            stats,
            Stats {
                input_cues: 0,
                output_cues: 0,
                runs_compacted: 0,
            },
        );
    }
}
