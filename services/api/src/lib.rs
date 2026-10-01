// Needed for binary
#[cfg(feature = "plus")]
use api_checkout as _;
use bencher_config as _;
use bencher_json as _;
use bencher_logger as _;
#[cfg(feature = "otel")]
use bencher_otel as _;
#[cfg(any(feature = "plus", feature = "otel"))]
use bencher_otel_provider as _;
use futures_concurrency as _;
use futures_util as _;
#[cfg(feature = "sentry")]
use sentry as _;
use serde_yaml as _;
use slog as _;
use thiserror as _;
use tokio_rustls as _;
// Needed for distroless builds
use libsqlite3_sys as _;

use tokio::runtime::{Builder, Runtime};

pub mod api;

pub use api_server::{SPEC, SPEC_STR};

#[cfg(not(tokio_unstable))]
compile_error!("`tokio_unstable` required for `enable_eager_driver_handoff`");

pub fn runtime() -> std::io::Result<Runtime> {
    Builder::new_multi_thread()
        .enable_all()
        // A worker that held the I/O driver wakes a parked worker before it polls a task, so one
        // handler that blocks its thread cannot leave the listener and every other socket unpolled.
        // https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html#method.enable_eager_driver_handoff
        .enable_eager_driver_handoff()
        .build()
}
