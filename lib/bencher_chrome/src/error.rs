use thiserror::Error;
use tokio::time::Duration;

#[derive(Debug, Error)]
pub enum ChromeError {
    #[error("Timeout after {} nanosecond(s)", _0.as_nanos())]
    Timeout(Duration),
    #[error("Chrome launched, but didn't give us a WebSocket URL before we timed out")]
    PortOpenTimeout,
    #[error("There are no available ports between 8000 and 9000 for debugging")]
    NoAvailablePorts,
    #[error("The chosen debugging port is already in use")]
    DebugPortInUse,
    #[error("No element found")]
    NoElementFound,
    #[error("Navigate failed: {0}")]
    NavigationFailed(String),
    #[error("No LocalStorage item was found")]
    NoLocalStorageItemFound,
    #[error("No UserAgent evaluated")]
    NoUserAgentEvaluated,
    #[error("Could not get element quad")]
    NoQuadFound,
    #[error("Scrolling element into view failed: {0}")]
    ScrollFailed(String),
    #[error("Unable to make method calls because underlying connection is closed")]
    ConnectionClosed,
    #[error("Could not auto detect a chrome executable")]
    NoExecutable,
    #[error("Key not found: {0}")]
    KeyNotFound(String),
}
