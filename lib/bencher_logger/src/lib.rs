use std::{
    io::{self, BufWriter, Stderr},
    sync::Mutex,
};

use slog::{Drain as _, Level, LevelFilter, Logger};
use slog_json::Json;

pub fn bootstrap_logger() -> Logger {
    let drain = Mutex::new(json()).fuse();
    Logger::root(drain, slog::o!())
}

pub fn server_logger(level: Level) -> Logger {
    let drain = LevelFilter(json(), level).fuse();
    let drain = slog_async::Async::new(drain).chan_size(1024).build().fuse();
    Logger::root(drain, slog::o!())
}

/// Writes each record as one flushed JSON line; `serde_json` escapes C0 control characters, so a logged value cannot split it.
fn json() -> Json<BufWriter<Stderr>> {
    Json::new(BufWriter::new(io::stderr()))
        .set_flush(true)
        .add_default_keys()
        .build()
}
