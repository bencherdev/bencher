pub mod benchmark;
pub mod branch;
pub mod metrics;
pub mod perf;
pub mod project;
pub mod report;
pub mod testbed;
pub mod threshold;
pub mod user;
pub mod version;
pub mod nonce;

// https://docs.rs/chrono/latest/chrono/naive/struct.NaiveDateTime.html#impl-Display-for-NaiveDateTime
pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";
