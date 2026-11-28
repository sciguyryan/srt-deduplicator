//! ASS event compaction that retains script records and dialogue formatting.

use crate::dedup::normalise;

#[derive(Clone)]
struct Layout {
    count: usize,
    start: usize,
    end: usize,
    text: usize,
}

impl Layout {
    fn parse(value: &str) -> Result<Self, String> {
        let fields: Vec<_> = value
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        let find = |name: &str| -> Result<usize, String> {
            let positions: Vec<_> = fields
                .iter()
                .enumerate()
                .filter(|(_, s)| *s == name)
                .collect();
            if positions.len() != 1 {
                return Err(format!(
                    "event Format must contain exactly one {name} field"
                ));
            }
            Ok(positions[0].0)
        };
        let layout = Self {
            count: fields.len(),
            start: find("start")?,
            end: find("end")?,
            text: find("text")?,
        };
        if layout.text + 1 != layout.count {
            return Err("event Text must be the final Format field".into());
        }
        Ok(layout)
    }
}

struct Dialogue {
    prefix: String,
    fields: Vec<String>,
    key: Vec<String>,
    line: usize,
}

fn timestamp(value: &str) -> bool {
    let parts: Vec<_> = value.trim().split([':', '.']).collect();
    parts.len() == 4
        && !parts[0].is_empty()
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts[3].len() == 2
        && parts.iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
        && parts[1].parse::<u8>().is_ok_and(|v| v < 60)
        && parts[2].parse::<u8>().is_ok_and(|v| v < 60)
}

/// Only adjacent Dialogue records with matching non-timing fields merge.
/// Other records break runs and survive unchanged, apart from line endings.
pub fn compact(input: &str) -> Result<String, String> {
    let mut output: Vec<String> = Vec::new();
    let mut events = false;
    let mut seen_events = false;
    let mut layout: Option<Layout> = None;
    let mut previous: Option<Dialogue> = None;
    for (number, line) in input.trim_start_matches('\u{feff}').lines().enumerate() {
        let trimmed = line.trim();
        let fail = |message: String| format!("line {}: {message}", number + 1);
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            events = trimmed.eq_ignore_ascii_case("[Events]");
            seen_events |= events;
            layout = None;
        }
        let record = line.split_once(':');
        if events && record.is_some_and(|(kind, _)| kind.trim().eq_ignore_ascii_case("Format")) {
            layout = Some(Layout::parse(record.unwrap().1).map_err(fail)?);
        } else if events
            && record.is_some_and(|(kind, _)| kind.trim().eq_ignore_ascii_case("Dialogue"))
        {
            let layout = layout
                .as_ref()
                .ok_or_else(|| fail("Dialogue precedes event Format".into()))?;
            let (kind, value) = record.unwrap();
            let fields: Vec<String> = value.splitn(layout.count, ',').map(str::to_owned).collect();
            if fields.len() != layout.count {
                return Err(fail("Dialogue has too few fields".into()));
            }
            if !timestamp(&fields[layout.start]) || !timestamp(&fields[layout.end]) {
                return Err(fail(
                    "invalid Dialogue timestamp; expected H:MM:SS.cc".into(),
                ));
            }
            let key = fields
                .iter()
                .enumerate()
                .filter_map(|(i, field)| {
                    if i == layout.start || i == layout.end {
                        None
                    } else if i == layout.text {
                        Some(normalise(field))
                    } else {
                        Some(field.clone())
                    }
                })
                .collect::<Vec<_>>();
            // Overrides can contain animation, karaoke or drawing instructions
            // whose timing or whitespace is meaningful. Leave those events alone.
            let has_overrides = fields[layout.text].contains('{');
            if !has_overrides && previous.as_ref().is_some_and(|prev| prev.key == key) {
                let prev = previous.as_mut().unwrap();
                prev.fields[layout.end] = fields[layout.end].clone();
                output[prev.line] = format!("{}:{}", prev.prefix, prev.fields.join(","));
                continue;
            }
            previous = if has_overrides {
                None
            } else {
                Some(Dialogue {
                    prefix: kind.to_owned(),
                    fields,
                    key,
                    line: output.len(),
                })
            };
            output.push(line.to_owned());
            continue;
        }
        previous = None;
        output.push(line.to_owned());
    }
    if !seen_events {
        return Err("missing [Events] section".into());
    }
    if !output.is_empty() {
        output.push(String::new());
    }
    Ok(output.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "[Script Info]\nScriptType: v4.00+\n[V4+ Styles]\nStyle: Default,Arial,20\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

    fn dialogue(start: &str, end: &str, text: &str) -> String {
        format!("Dialogue: 0,{start},{end},Default,,0,0,0,,{text}\n")
    }

    #[test]
    fn preserves_script_and_first_text_while_merging_a_run() {
        let first = dialogue("0:00:01.00", "0:00:02.00", " Hello,  world ");
        let second = dialogue("0:00:04.00", "0:00:05.00", "Hello, world");
        let third = dialogue("0:00:05.00", "0:00:06.00", "Hello, world");
        assert_eq!(
            compact(&format!("{HEADER}{first}{second}{third}")).unwrap(),
            format!(
                "{HEADER}{}",
                dialogue("0:00:01.00", "0:00:06.00", " Hello,  world ")
            )
        );
    }

    #[test]
    fn preserves_differences_in_every_non_timing_field() {
        let first = dialogue("0:00:01.00", "0:00:02.00", "Hello");
        for index in [0, 3, 4, 5, 6, 7, 8, 9] {
            let mut fields: Vec<_> = first.trim_end().splitn(10, ',').collect();
            fields[index] = if index == 0 {
                "Dialogue: 1"
            } else {
                "Different"
            };
            let input = format!("{HEADER}{first}{}\n", fields.join(","));
            assert_eq!(compact(&input).unwrap(), input, "field {index}");
        }
    }

    #[test]
    fn other_records_and_override_blocks_break_runs() {
        let cue = dialogue("0:00:01.00", "0:00:02.00", "Hello");
        for barrier in [
            "Comment: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,note\n",
            "\n",
            "; note\n",
            "[Fonts]\nfontname: example\n",
        ] {
            let input = format!("{HEADER}{cue}{barrier}{cue}");
            assert_eq!(compact(&input).unwrap(), input);
        }
        for text in [r"{\k20}Hello", r"{\i1}Hello", r"{\p1}m 0 0 l 10 10"] {
            let cue = dialogue("0:00:01.00", "0:00:02.00", text);
            let input = format!("{HEADER}{cue}{cue}");
            assert_eq!(compact(&input).unwrap(), input);
        }
    }

    #[test]
    fn reads_declared_field_order_and_retains_unknown_fields() {
        let header = "[Events]\nFormat: End, Extra, Start, Text\n";
        let input = format!(
            "{header}Dialogue: 0:00:02.00,x,0:00:01.00,Hi\nDialogue: 0:00:04.00,x,0:00:03.00,Hi\n"
        );
        assert_eq!(
            compact(&input).unwrap(),
            format!("{header}Dialogue: 0:00:04.00,x,0:00:01.00,Hi\n")
        );
    }

    #[test]
    fn rejects_invalid_event_layouts_and_timestamps() {
        for input in [
            "",
            "not ASS",
            "[Events]\nDialogue: Hi",
            "[Events]\nFormat: Start, End, Text, Extra",
            "[Events]\nFormat: Start, Start, End, Text",
            "[Events]\nFormat: End, Text",
            "[Events]\nFormat: Start, End, Text\nDialogue: Hi",
            "[Events]\nFormat: Start, End, Text\nDialogue: 0:60:00.00,0:00:02.00,Hi",
            "[Events]\nFormat: Start, End, Text\nDialogue: 0:00:00.000,0:00:02.00,Hi",
        ] {
            assert!(compact(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn normalises_line_endings_and_tolerates_bom_and_empty_events() {
        assert_eq!(compact("\u{feff}[Events]\r\n").unwrap(), "[Events]\n");
        let cue = dialogue("0:00:01.00", "0:00:02.00", r"Hello\Nworld\h!");
        let input = format!("{HEADER}{cue}{cue}");
        assert_eq!(
            compact(&format!("\u{feff}{}", input.replace('\n', "\r\n"))).unwrap(),
            format!("{HEADER}{cue}")
        );
    }
}
