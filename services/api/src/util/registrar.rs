use dropshot::{ApiDescription, ServerContext};

use crate::ApiError;

pub trait Registrar<Context> {
    fn register(api: &mut ApiDescription<Context>) -> Result<(), ApiError>
    where
        Context: ServerContext;
}
