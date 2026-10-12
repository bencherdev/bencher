#![cfg(feature = "plus")]
//! OAuth clients and the metadata they registered, from the server's configuration
//! or from the Client ID Metadata Documents their client IDs point to.

mod clients;
mod documents;

pub use clients::{OAuthClientError, OAuthClients, Throttled};
