#[cfg(feature = "db")]
#[macro_use]
extern crate diesel;

use dropshot::{
    ApiDescription,
    ServerContext,
};

#[cfg(feature = "db")]
pub mod db;
pub mod server;

pub trait Registrar<Context> {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String>
    where
        Context: ServerContext;
}
