use std::sync::{Arc, Mutex, PoisonError};

pub fn discard() -> slog::Logger {
    slog::Logger::root(slog::Discard, slog::o!())
}

/// Keeps the message of every record.
#[derive(Clone, Default)]
pub struct Messages(Arc<Mutex<Vec<String>>>);

impl Messages {
    pub fn logger(&self) -> slog::Logger {
        slog::Logger::root(self.clone(), slog::o!())
    }

    pub fn contains(&self, msg: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|kept| kept == msg)
    }

    pub fn count(&self, msg: &str) -> usize {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|kept| *kept == msg)
            .count()
    }
}

impl slog::Drain for Messages {
    type Ok = ();
    type Err = slog::Never;

    fn log(
        &self,
        record: &slog::Record<'_>,
        _values: &slog::OwnedKVList,
    ) -> Result<(), slog::Never> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(record.msg().to_string());
        Ok(())
    }
}
