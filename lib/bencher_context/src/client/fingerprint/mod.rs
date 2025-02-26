use derive_more::Display;
use uuid::Uuid;

mod target_os;

#[derive(Debug, Display, Clone, Copy)]
pub struct Fingerprint(Uuid);
