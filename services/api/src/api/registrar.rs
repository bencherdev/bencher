use dropshot::{
    ApiDescription,
    ServerContext,
};

pub trait Registrar<Context> {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String>
    where
        Context: ServerContext;
}
