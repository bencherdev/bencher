#[cfg(feature = "plus")]
mod plus;

#[cfg(feature = "plus")]
pub use plus::*;

crate::macros::typed_id::typed_id!(SpecId);
