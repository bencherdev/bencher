use core::fmt;

use opentelemetry::KeyValue;

/// Bounds for durations from 10 ms to about 17 minutes.
pub(crate) const SECONDS_BOUNDARIES: &[f64] = &[
    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
];

/// Bounds for sizes from 1 KiB to 4 GiB, in powers of 4.
pub(crate) const BYTES_BOUNDARIES: &[f64] = &[
    1_024.0,
    4_096.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    16_777_216.0,
    67_108_864.0,
    268_435_456.0,
    1_073_741_824.0,
    4_294_967_296.0,
];

#[derive(Debug, Clone, Copy)]
pub enum OciStorageOperation {
    Put,
    CreateMultipart,
    PutPart,
    Complete,
    Abort,
    Copy,
    PartCopy,
    Rename,
    Get,
    Head,
    Delete,
    List,
}

impl fmt::Display for OciStorageOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Put => write!(f, "put"),
            Self::CreateMultipart => write!(f, "create_multipart"),
            Self::PutPart => write!(f, "put_part"),
            Self::Complete => write!(f, "complete"),
            Self::Abort => write!(f, "abort"),
            Self::Copy => write!(f, "copy"),
            Self::PartCopy => write!(f, "part_copy"),
            Self::Rename => write!(f, "rename"),
            Self::Get => write!(f, "get"),
            Self::Head => write!(f, "head"),
            Self::Delete => write!(f, "delete"),
            Self::List => write!(f, "list"),
        }
    }
}

impl From<OciStorageOperation> for KeyValue {
    fn from(operation: OciStorageOperation) -> Self {
        Self::new(OciStorageOperation::KEY, operation.to_string())
    }
}

impl OciStorageOperation {
    const KEY: &str = "operation";
}

#[derive(Debug, Clone, Copy)]
pub enum OciStorageOutcome {
    Ok,
    NotFound,
    Precondition,
    Error,
}

impl fmt::Display for OciStorageOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::NotFound => write!(f, "not_found"),
            Self::Precondition => write!(f, "precondition"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl From<OciStorageOutcome> for KeyValue {
    fn from(outcome: OciStorageOutcome) -> Self {
        Self::new(OUTCOME_KEY, outcome.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OciStorageErrorCode {
    NotFound,
    RangeNotSatisfiable,
    TooManyRequests,
    ClientError,
    ServerError,
    Timeout,
    Connect,
    SignatureDoesNotMatch,
    ExpiredRequest,
    AccessDenied,
    NoSuchKey,
    NoSuchUpload,
    InvalidRange,
    SlowDown,
    InternalError,
    Other,
}

impl fmt::Display for OciStorageErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "404"),
            Self::RangeNotSatisfiable => write!(f, "416"),
            Self::TooManyRequests => write!(f, "429"),
            Self::ClientError => write!(f, "4xx"),
            Self::ServerError => write!(f, "5xx"),
            Self::Timeout => write!(f, "timeout"),
            Self::Connect => write!(f, "connect"),
            Self::SignatureDoesNotMatch => write!(f, "SignatureDoesNotMatch"),
            Self::ExpiredRequest => write!(f, "ExpiredRequest"),
            Self::AccessDenied => write!(f, "AccessDenied"),
            Self::NoSuchKey => write!(f, "NoSuchKey"),
            Self::NoSuchUpload => write!(f, "NoSuchUpload"),
            Self::InvalidRange => write!(f, "InvalidRange"),
            Self::SlowDown => write!(f, "SlowDown"),
            Self::InternalError => write!(f, "InternalError"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl From<OciStorageErrorCode> for KeyValue {
    fn from(code: OciStorageErrorCode) -> Self {
        Self::new(OciStorageErrorCode::KEY, code.to_string())
    }
}

impl OciStorageErrorCode {
    const KEY: &str = "error.code";
}

#[derive(Debug, Clone, Copy)]
pub enum OciUploadPhase {
    Patch,
    Finalize,
}

impl fmt::Display for OciUploadPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Patch => write!(f, "patch"),
            Self::Finalize => write!(f, "finalize"),
        }
    }
}

impl From<OciUploadPhase> for KeyValue {
    fn from(phase: OciUploadPhase) -> Self {
        Self::new(OciUploadPhase::KEY, phase.to_string())
    }
}

impl OciUploadPhase {
    const KEY: &str = "phase";
}

#[derive(Debug, Clone, Copy)]
pub enum OciPhaseOutcome {
    Ok,
    Error,
    Aborted,
}

impl fmt::Display for OciPhaseOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Error => write!(f, "error"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

impl From<OciPhaseOutcome> for KeyValue {
    fn from(outcome: OciPhaseOutcome) -> Self {
        Self::new(OUTCOME_KEY, outcome.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OciFinalizeOutcome {
    Ok,
    DigestMismatch,
    Resumed,
    EtagMismatch,
    Error,
}

impl fmt::Display for OciFinalizeOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::DigestMismatch => write!(f, "digest_mismatch"),
            Self::Resumed => write!(f, "resumed"),
            Self::EtagMismatch => write!(f, "etag_mismatch"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl From<OciFinalizeOutcome> for KeyValue {
    fn from(outcome: OciFinalizeOutcome) -> Self {
        Self::new(OUTCOME_KEY, outcome.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OciSweepAction {
    Aborted,
    Deleted,
}

impl fmt::Display for OciSweepAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aborted => write!(f, "aborted"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

impl From<OciSweepAction> for KeyValue {
    fn from(action: OciSweepAction) -> Self {
        Self::new(OciSweepAction::KEY, action.to_string())
    }
}

impl OciSweepAction {
    const KEY: &str = "action";
}

#[derive(Debug, Clone, Copy)]
pub enum OciPullOutcome {
    Complete,
    Aborted,
}

impl fmt::Display for OciPullOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complete => write!(f, "complete"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

impl From<OciPullOutcome> for KeyValue {
    fn from(outcome: OciPullOutcome) -> Self {
        Self::new(OUTCOME_KEY, outcome.to_string())
    }
}

const OUTCOME_KEY: &str = "outcome";
