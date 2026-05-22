pub mod token;
#[expect(
    clippy::module_inception,
    reason = "module re-exports the primary type"
)]
pub mod user;
