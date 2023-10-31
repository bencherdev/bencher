use once_cell::sync::Lazy;
use url::Url;

#[cfg(debug_assertions)]
pub const BENCHER_DEV: &str = "http://localhost:3000";
#[cfg(not(debug_assertions))]
pub const BENCHER_DEV: &str = "https://bencher.dev";
const DEVEL_BENCHER_DEV: &str = "https://devel--bencher.netlify.app";

#[cfg(debug_assertions)]
pub const API_BENCHER_DEV: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const API_BENCHER_DEV: &str = "https://api.bencher.dev";

#[allow(clippy::panic)]
pub static BENCHER_DEV_URL: Lazy<Url> = Lazy::new(|| {
    BENCHER_DEV
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{BENCHER_DEV}\": {e}"))
});
#[allow(clippy::panic)]
static DEVEL_BENCHER_DEV_URL: Lazy<Url> = Lazy::new(|| {
    DEVEL_BENCHER_DEV
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{DEVEL_BENCHER_DEV}\": {e}"))
});

#[allow(clippy::panic)]
pub static API_BENCHER_DEV_URL: Lazy<Url> = Lazy::new(|| {
    API_BENCHER_DEV
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{API_BENCHER_DEV}\": {e}"))
});

pub fn is_bencher_dev(url: &Url) -> bool {
    *url == *BENCHER_DEV_URL || *url == *DEVEL_BENCHER_DEV_URL
}
