fn main() {
    // This is here to test that the fingerprinting is working correctly on all platforms.
    #[allow(clippy::expect_used)]
    let _ = bencher_fingerprint::Fingerprint::new().expect("Failed to create fingerprint");
}
