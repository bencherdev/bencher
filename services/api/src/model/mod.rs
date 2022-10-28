pub mod organization;
pub mod project;
pub mod user;

// https://docs.rs/chrono/latest/chrono/naive/struct.NaiveDateTime.html#impl-Display-for-NaiveDateTime
pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";
