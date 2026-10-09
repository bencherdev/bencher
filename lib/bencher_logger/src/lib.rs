use std::{
    fmt::Display,
    io::{self, BufWriter, Stderr, Write},
    sync::Mutex,
};

use slog::{Drain as _, Level, LevelFilter, Logger};
use slog_json::Json;

/// journald's default `LineMax`, newline included: a longer line splits into
/// records, and a leading `<N>` in the tail sets that record's priority.
const LINE_MAX: usize = 48 * 1024;

/// Text from outside the runner is cut to this many bytes, so its escaped
/// form, at most 6 bytes per byte, keeps a record under [`LINE_MAX`].
pub const FIELD_CAP: usize = 7 * 1024;

pub fn bootstrap_logger() -> Logger {
    let drain = Mutex::new(json()).fuse();
    Logger::root(drain, slog::o!())
}

pub fn server_logger(level: Level) -> Logger {
    let drain = LevelFilter(json(), level).fuse();
    let drain = slog_async::Async::new(drain).chan_size(1024).build().fuse();
    Logger::root(drain, slog::o!())
}

/// Synchronous, so no record is lost at an exit or an `exec`, and each record
/// reaches stderr whole, so nothing else written there can land inside it.
pub fn runner_logger() -> Logger {
    runner_logger_to(io::stderr())
}

/// [`runner_logger`], writing to `io` instead of stderr.
pub fn runner_logger_to<W: Write + Send + 'static>(io: W) -> Logger {
    let drain = Mutex::new(runner_json(io)).fuse();
    Logger::root(drain, slog::o!())
}

/// `value`, displayed and cut to [`FIELD_CAP`] bytes at a character boundary.
pub fn capped<T: Display>(value: T) -> String {
    let mut text = value.to_string();
    text.truncate(text.floor_char_boundary(FIELD_CAP));
    text
}

/// Writes each record as one flushed JSON line; `serde_json` escapes C0 control characters, so a logged value cannot split it.
fn json() -> Json<BufWriter<Stderr>> {
    Json::new(BufWriter::new(io::stderr()))
        .set_flush(true)
        .add_default_keys()
        .build()
}

fn runner_json<W: Write>(io: W) -> Json<RecordWriter<W>> {
    Json::new(RecordWriter::new(io))
        .set_flush(true)
        .add_default_keys()
        .build()
}

/// Holds a record until slog-json flushes it, then hands it on in one
/// `write_all`, which `Stderr` makes under one held lock. A record longer than
/// [`LINE_MAX`] is replaced by a short one naming its length.
struct RecordWriter<W> {
    io: W,
    record: Vec<u8>,
    /// Past [`LINE_MAX`] the record is only counted, never held.
    len: usize,
}

impl<W> RecordWriter<W> {
    fn new(io: W) -> Self {
        Self {
            io,
            record: Vec::new(),
            len: 0,
        }
    }
}

impl<W: Write> Write for RecordWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.len = self.len.saturating_add(buf.len());
        if self.len <= LINE_MAX {
            self.record.extend_from_slice(buf);
        } else {
            self.record.clear();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let written = if self.len <= LINE_MAX {
            self.io.write_all(&self.record)
        } else {
            self.io.write_all(
                format!(
                    "{{\"level\":\"ERRO\",\"msg\":\"Record too long\",\"record_bytes\":{}}}\n",
                    self.len
                )
                .as_bytes(),
            )
        };
        self.record.clear();
        self.len = 0;
        written.and_then(|()| self.io.flush())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, PoisonError};

    use slog::info;

    use super::*;

    /// Keeps each `write` call apart, so a record split across calls shows.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<Vec<u8>>>>);

    impl Sink {
        fn writes(&self) -> Vec<Vec<u8>> {
            self.0.lock().unwrap().clone()
        }
    }

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(buf.to_vec());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_record_reaches_stderr_in_one_write() {
        // Fails if the writer hands a record on in pieces, as an 8 KiB `BufWriter` does,
        // or hands a record on again with the next.
        let sink = Sink::default();
        let log = runner_logger_to(sink.clone());
        let field = "x".repeat(40_000);

        info!(log, "First"; "field" => &field);
        info!(log, "Second"; "field" => &field);

        let calls = sink.writes();
        assert_eq!(calls.len(), 2, "each record must be one write");
        for (line, msg) in calls.iter().zip(["First", "Second"]) {
            assert!(line.ends_with(b"\n"));
            let record: serde_json::Value = serde_json::from_slice(line).unwrap();
            assert_eq!(record["msg"], msg);
            assert_eq!(record["field"], field);
        }
    }

    #[test]
    fn a_record_over_the_line_limit_is_replaced() {
        // Fails if a record longer than journald's line limit goes out whole,
        // or if the bound is off by one either way.
        for (len, whole) in [(LINE_MAX, true), (LINE_MAX + 1, false)] {
            let sink = Sink::default();
            let mut writer = RecordWriter::new(sink.clone());
            let mut line = vec![b'x'; len - 1];
            line.push(b'\n');

            writer.write_all(&line).unwrap();
            writer.flush().unwrap();

            let calls = sink.writes();
            assert_eq!(calls.len(), 1, "{len} bytes must be one write");
            assert_eq!(calls.first() == Some(&line), whole, "{len} bytes");
        }

        let sink = Sink::default();

        info!(runner_logger_to(sink.clone()), "Long"; "field" => "x".repeat(100 * 1024));

        let calls = sink.writes();
        assert_eq!(calls.len(), 1, "a record must be one write");
        let line = calls.first().unwrap();
        assert!(line.len() < LINE_MAX && line.ends_with(b"\n"));
        let record: serde_json::Value = serde_json::from_slice(line).unwrap();
        assert_eq!(record["level"], "ERRO");
        assert_eq!(record["msg"], "Record too long");
        assert!(
            record["record_bytes"].as_u64().unwrap() > 100 * 1024,
            "{record}"
        );
    }

    #[test]
    fn a_long_value_is_cut_at_a_char_boundary() {
        // The cap falls inside a 3-byte `€`: fails if the cut panics there, as a plain
        // `truncate` does, or keeps other than the whole characters that fit.
        let len = capped("€".repeat(FIELD_CAP)).len();

        assert!(len <= FIELD_CAP, "{len} bytes");
        assert!(len + '€'.len_utf8() > FIELD_CAP, "{len} bytes");
    }
}
