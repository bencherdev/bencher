use anyhow::{Error, Result};
use tokio::time::{sleep, Duration, Instant};

use crate::ChromeError;

/// A helper to wait until some event has passed.
#[derive(Debug, Clone, Copy)]
pub struct Wait {
    timeout: Duration,
    sleep: Duration,
}

impl Wait {
    /// Wait `timeout` and `sleep`
    pub fn new(timeout: Duration, sleep: Duration) -> Self {
        Self { timeout, sleep }
    }

    /// Wait until the given predicate returns `Some(G)` or timeout arrives.
    ///
    /// Note: If your predicate function shadows potential unexpected
    ///   errors you should consider using `#strict_until`.
    pub async fn until<F, G>(&self, predicate: F) -> Result<G, ChromeError>
    where
        F: FnMut() -> Option<G>,
    {
        let mut predicate = predicate;
        let start = Instant::now();
        loop {
            if let Some(v) = predicate() {
                return Ok(v);
            }
            if start.elapsed() > self.timeout {
                return Err(ChromeError::Timeout(self.timeout));
            }
            sleep(self.sleep).await;
        }
    }

    /// Wait until the given predicate returns `Ok(G)`, an unexpected error occurs or timeout arrives.
    ///
    /// Errors produced by the predicate are downcasted by the additional provided closure.
    /// If the downcast is successful - the error is ignored, otherwise the wait is terminated
    /// and `Err(error)` containing the unexpected failure is returned to the caller.
    ///
    /// You can use `failure::Error::downcast::<YourStructName>` out-of-the-box,
    /// if you need to ignore one expected error, or you can implement a matching closure
    /// that responds to multiple error types.
    pub async fn strict_until<F, D, E, G>(
        &self,
        predicate: F,
        downcast: D,
    ) -> Result<G, ChromeError>
    where
        F: FnMut() -> Result<G, ChromeError>,
        D: FnMut(Error) -> Result<E, ChromeError>,
    {
        let mut predicate = predicate;
        let mut downcast = downcast;
        let start = Instant::now();
        loop {
            match predicate() {
                Ok(value) => return Ok(value),
                Err(error) => downcast(error)?,
            };
            if start.elapsed() > self.timeout {
                return Err(ChromeError::Timeout(self.timeout));
            }
            sleep(self.sleep).await;
        }
    }
}
