use std::collections::BTreeMap;
use std::fmt::Write as _;

use bencher_json::RunnerKey;

const MARKER: &str = "@@audit ";
const STATUS: &str = "@@status ";
pub const RUNNER_KEY: &str = "BENCHER_RUNNER_KEY";

/// The remote script that prints each named command's output between a marker line and its exit status.
pub fn script<F>(frames: F) -> String
where
    F: IntoIterator<Item = (String, String)>,
{
    let mut script =
        String::from("(set -o pipefail) 2>/dev/null && set -o pipefail\nexport LC_ALL=C\n");
    for (name, command) in frames {
        _ = writeln!(script, "{}{{ {command}; }} 2>&1", marker(&name));
        _ = writeln!(script, "printf '\\n{STATUS}%s\\n' \"$?\"");
    }
    // A failing command is data; only a failed connection is an error.
    script.push_str("exit 0\n");
    script
}

// The leading newline keeps a marker off the last line of output that has no trailing newline.
pub fn marker(name: &str) -> String {
    format!("printf '\\n{MARKER}%s\\n' '{name}'\n")
}

/// Raw output and exit status of one framed command.
#[derive(Debug, Default)]
pub struct Output {
    pub text: String,
    pub status: Option<i32>,
}

/// Each frame's output by name, with every runner key line removed.
pub fn parse(output: &str) -> BTreeMap<String, Output> {
    let mut frames = BTreeMap::<String, Output>::new();
    let mut current = None;
    for line in output.lines() {
        if let Some(name) = line.strip_prefix(MARKER) {
            frames.entry(name.to_owned()).or_default();
            current = Some(name);
        } else if let Some(name) = current
            && !line.contains(RUNNER_KEY)
            && !line.contains(RunnerKey::PREFIX)
        {
            let output = frames.entry(name.to_owned()).or_default();
            if let Some(status) = line.strip_prefix(STATUS) {
                output.status = status.parse().ok();
            } else {
                output.text.push_str(line);
                output.text.push('\n');
            }
        }
    }
    frames
}

/// Fake `script()` output for one frame, cut off before its exit status when `status` is `None`.
#[cfg(test)]
pub fn fake_frame(name: &str, text: &str, status: Option<i32>) -> String {
    let status = status.map_or_else(String::new, |status| format!("\n{STATUS}{status}\n"));
    format!("\n{MARKER}{name}\n{text}\n{status}")
}
