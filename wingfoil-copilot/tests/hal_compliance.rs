// Copyright (c) 2026 Cedric Gegout
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::process::{Command, Stdio};
use std::io::Write;

#[test]
fn test_malformed_input_error_isolation() {
    // Compile binary if needed
    let mut child = Command::new("cargo")
        .args(&["run", "--bin", "wingfoil-copilot"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn cargo run");

    // Send malformed JSON to stdin
    let stdin = child.stdin.as_mut().expect("Failed to get stdin");
    writeln!(stdin, "this is not json").expect("Failed to write to stdin");

    let output = child.wait_with_output().expect("Failed to wait for child");

    // 1. Should exit with non-zero exit code
    assert!(!output.status.success(), "Process should exit with a non-zero code on failure");

    // 2. Stdout must contain a single valid NDJSON line
    let stdout_str = String::from_utf8(output.stdout).expect("Stdout is not valid UTF-8");
    let lines: Vec<&str> = stdout_str.lines().collect();
    assert_eq!(lines.len(), 1, "Stdout must contain exactly one line on malformed stdin");

    // 3. Output should parse to an ErrorResponse JSON with the exact expected fields
    let err_json: serde_json::Value = serde_json::from_str(lines[0])
        .expect("Stdout should be valid JSON");

    assert_eq!(err_json["type"], "error");
    assert!(err_json["request_id"].is_string());
    assert!(err_json["reason"].is_string());
    assert!(err_json["technical_details"].is_string());
    assert!(err_json["suggested_action"].is_string());
    assert_eq!(err_json["format"], "html");
}
