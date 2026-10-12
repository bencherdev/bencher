#![cfg(feature = "plus")]

use std::{error::Error, iter};

mod address;
mod client;

pub use address::AddressClass;
pub use client::{
    EgressBlock, EgressClient, EgressError, EgressTimeouts, GetJsonError, JsonDocument,
};
#[cfg(any(test, feature = "test-loopback"))]
pub use client::{EgressPolicy, Lookup};

/// An error and every cause under it, which tell a missing name from a refused connection.
pub fn error_chain(error: &dyn Error) -> String {
    iter::successors(error.source(), |&cause| cause.source()).fold(
        error.to_string(),
        |mut chain, cause| {
            chain.push_str(": ");
            chain.push_str(&cause.to_string());
            chain
        },
    )
}
