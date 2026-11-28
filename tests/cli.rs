//! End-to-end tests that drive the compiled binary against real files.

use std::process::Command;

use tempfile::TempDir;

/// The compiled binary, provided by cargo for integration tests.
const BINARY: &str = env!("CARGO_BIN_EXE_subtitle-deduplicator");

/// Writes `content` to a new file named `name` inside `dir`.
fn write_input(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("test input file");
    path
}

/// Runs the binary with the given arguments and returns (exit code, stdout).
fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(BINARY)
        .args(args)
        .output()
        .expect("binary runs");
    (
        output.status.code().expect("exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// Runs the binary with the given arguments and asserts that it failed.
fn expect_failure(args: &[&str]) -> String {
    let output = Command::new(BINARY)
        .args(args)
        .output()
        .expect("binary runs");
    assert!(
        !output.status.success(),
        "expected a failing exit status for {args:?}",
    );
    assert!(output.stdout.is_empty(), "no output on failure");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: "),
        "diagnostics go to stderr, got: {stderr:?}",
    );
    stderr.into_owned()
}

#[test]
fn compacts_runs_written_to_standard_output() {
    let dir = TempDir::new().expect("temp dir");
    let input = write_input(
        &dir,
        "input.srt",
        "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n\
         2\n00:00:02,000 --> 00:00:03,000\nHello\n\n\
         3\n00:00:03,000 --> 00:00:04,000\nHello\n\n\
         4\n00:00:05,000 --> 00:00:06,000\nWorld\n",
    );

    let (code, stdout) = run(&[input.to_str().expect("UTF-8 path")]);

    assert_eq!(code, 0);
    assert_eq!(
        stdout,
        "1\n00:00:01,000 --> 00:00:04,000\nHello\n\n\
         2\n00:00:05,000 --> 00:00:06,000\nWorld\n",
    );
}

#[test]
fn tolerates_a_byte_order_mark_and_carriage_returns() {
    let dir = TempDir::new().expect("temp dir");
    let input = write_input(
        &dir,
        "input.srt",
        "\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n\r\n\
         2\r\n00:00:02,000 --> 00:00:03,000\r\nHello\r\n",
    );

    let (code, stdout) = run(&[input.to_str().expect("UTF-8 path")]);

    assert_eq!(code, 0);
    assert_eq!(stdout, "1\n00:00:01,000 --> 00:00:03,000\nHello\n",);
}

#[test]
fn writes_to_the_requested_output_file() {
    let dir = TempDir::new().expect("temp dir");
    let input = write_input(
        &dir,
        "input.srt",
        "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n\
         2\n00:00:02,000 --> 00:00:03,000\nHello\n",
    );
    let output_path = dir.path().join("output.srt");

    let (code, stdout) = run(&[
        "-o",
        output_path.to_str().expect("UTF-8 path"),
        input.to_str().expect("UTF-8 path"),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "nothing goes to stdout with -o");
    let written = std::fs::read_to_string(&output_path).expect("output file");
    assert_eq!(written, "1\n00:00:01,000 --> 00:00:03,000\nHello\n",);
}

#[test]
fn reports_malformed_input_as_an_error() {
    let dir = TempDir::new().expect("temp dir");
    let input = write_input(&dir, "input.srt", "not a subtitle file\n");

    expect_failure(&[input.to_str().expect("UTF-8 path")]);
}

#[test]
fn reports_a_missing_input_file_as_an_error() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("absent.srt");

    expect_failure(&[missing.to_str().expect("UTF-8 path")]);
}

const ASS_INPUT: &str = "[Script Info]\nScriptType: v4.00+\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\nDialogue: 0,0:00:02.00,0:00:03.00,Default,,0,0,0,,Hello\n";

#[test]
fn ass_output_matches_between_stdout_and_file_and_retains_input() {
    let dir = TempDir::new().unwrap();
    let input = write_input(&dir, "input.ASS", ASS_INPUT);
    let output = dir.path().join("output.ass");
    let expected = ASS_INPUT
        .replace("0:00:01.00,0:00:02.00", "0:00:01.00,0:00:03.00")
        .replace(
            "Dialogue: 0,0:00:02.00,0:00:03.00,Default,,0,0,0,,Hello\n",
            "",
        );
    assert_eq!(run(&[input.to_str().unwrap()]), (0, expected.clone()));
    assert_eq!(
        run(&[input.to_str().unwrap(), "-o", output.to_str().unwrap()]),
        (0, String::new())
    );
    assert_eq!(std::fs::read_to_string(output).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(input).unwrap(), ASS_INPUT);
}

#[test]
fn malformed_ass_does_not_overwrite_existing_output() {
    let dir = TempDir::new().unwrap();
    let input = write_input(&dir, "input.ass", "[Events]\nDialogue: bad\n");
    let output = write_input(&dir, "output.ass", "keep me");
    let error = expect_failure(&[input.to_str().unwrap(), "-o", output.to_str().unwrap()]);
    assert!(error.contains("valid ASS file: line 2:"));
    assert_eq!(std::fs::read_to_string(output).unwrap(), "keep me");
}

#[test]
fn invalid_utf8_ass_is_rejected() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.ass");
    std::fs::write(&input, [0xff, 0xfe]).unwrap();
    expect_failure(&[input.to_str().unwrap()]);
}
