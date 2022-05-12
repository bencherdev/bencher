use dropshot::ApiDescription;
use dropshot::ServerContext;

#[cfg(feature = "db")]
pub mod db;
pub mod server;

pub trait Registrar<Context> {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String>
    where
        Context: ServerContext;
}
