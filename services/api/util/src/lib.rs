use dropshot::ApiDescription;
use dropshot::ServerContext;

pub mod server;

pub trait Registrar<Context> {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String>
    where
        Context: ServerContext;
}
