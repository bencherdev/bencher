use std::{
    fmt::{self, Write as _},
    sync::{Arc, Mutex, PoisonError},
};

use slog::{Drain, KV as _, OwnedKVList, Record};

/// Keeps every line the server logs, its message and every key-value pair, as text.
#[derive(Clone, Default)]
pub struct LogCapture(Arc<Mutex<Vec<String>>>);

impl LogCapture {
    pub fn logger(&self) -> slog::Logger {
        slog::Logger::root(self.clone(), slog::o!())
    }

    pub fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drain for LogCapture {
    type Ok = ();
    type Err = slog::Never;

    fn log(&self, record: &Record<'_>, values: &OwnedKVList) -> Result<(), slog::Never> {
        let mut line = Line(record.msg().to_string());
        if let Err(e) = record
            .kv()
            .serialize(record, &mut line)
            .and_then(|()| values.serialize(record, &mut line))
        {
            line.0.push_str(" unformatted: ");
            line.0.push_str(&e.to_string());
        }
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.0);
        Ok(())
    }
}

struct Line(String);

impl slog::Serializer for Line {
    fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments<'_>) -> slog::Result {
        write!(self.0, " {key}={val}").map_err(slog::Error::Fmt)
    }
}
