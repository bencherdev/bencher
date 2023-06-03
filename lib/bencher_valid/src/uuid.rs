/// This type exists solely for generating type information
/// And it functions on the basis of having a name collision with `uuid::Uuid`
/// For all other use cases, use `uuid::Uuid` instead
#[typeshare::typeshare]
pub struct Uuid(String);
