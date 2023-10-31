pub mod organization;
pub mod project;
#[cfg(feature = "plus")]
pub mod server;
pub mod user;

// https://docs.rs/chrono/latest/chrono/naive/struct.NaiveDateTime.html#impl-Display-for-NaiveDateTime
pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";
