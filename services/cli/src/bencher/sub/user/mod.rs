pub mod key;
#[expect(
    clippy::module_inception,
    reason = "module re-exports the primary type"
)]
pub mod user;
