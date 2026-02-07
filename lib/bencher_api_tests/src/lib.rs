#![expect(clippy::multiple_inherent_impl)]
//! Test utilities for Bencher API tests.
//!
//! This crate provides helpers for testing Bencher API endpoints:
//! - [`TestServer`]: Spins up a full Dropshot server with in-memory `SQLite`
//! - [`TestUser`], [`TestOrg`], [`TestProject`]: Test data types
//! - Seed helpers for creating test data
//!
//! # Example
//!
//! ```ignore
//! use bencher_api_tests::TestServer;
//!
//! #[tokio::test]
//! async fn test_my_endpoint() {
//!     let server = TestServer::new().await;
//!     let user = server.signup("Test User", "test@example.com").await;
//!
//!     let resp = server.client
//!         .get(server.api_url("/v0/my/endpoint"))
//!         .header("Authorization", server.bearer(&user.token))
//!         .send()
//!         .await
//!         .unwrap();
//!
//!     assert!(resp.status().is_success());
//! }
//! ```

// Needed for distroless builds
use libsqlite3_sys as _;
// Needed for plus feature propagation
use bencher_config as _;

mod context;
pub mod oci;
mod seed;

pub use context::TestServer;
pub use seed::{TestOrg, TestProject, TestUser};
