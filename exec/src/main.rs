use std::env;

use tide::Route;

pub mod routes;

use crate::routes::v1::exec;
use crate::routes::pong::pong;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let address = env::var("ENDPOINT_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("ENDPOINT_PORT").unwrap_or_else(|_| "8080".to_string());
    let endpoint = format!("{}:{}", address, port);

    let mut server = tide::new();

    server.at("/pong").get(pong);

    v1_routes(server.at("/api/v1"));

    server.listen(endpoint).await?;

    Ok(())
}

fn v1_routes(mut route: Route<()>) {
    exec_routes(route.at("/exec"));
}

pub fn exec_routes(mut route: Route<()>) {
    route.at("/").post(exec::exec);
    route.at("").post(exec::exec);
}