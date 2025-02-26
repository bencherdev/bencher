use derive_more::Display;
use uuid::Uuid;

mod target_os;

#[derive(Debug, Display, Clone, Copy)]
pub struct Fingerprint(Uuid);

#[allow(dead_code)]
#[derive(Debug, Display, Clone, Copy)]
pub enum OperatingSystem {
    Linux,
    Macos,
    Windows,
}
