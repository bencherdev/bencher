#[macro_use]
extern crate derive_builder;
extern crate log;

pub mod browser;
mod error;
pub mod protocol;
pub mod types;
pub mod wait;

pub use browser::{
    tab::{element::Element, Tab},
    Browser, LaunchOptions, LaunchOptionsBuilder,
};
pub use error::ChromeError;
