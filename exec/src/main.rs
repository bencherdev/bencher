use std::env;

use http_types::headers::HeaderValue;
use tide::security::{CorsMiddleware, Origin};
use tide::Route;

pub mod routes;

use crate::routes::ping::ping;
use crate::routes::v1::exec;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let address = env::var("ENDPOINT_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("ENDPOINT_PORT").unwrap_or_else(|_| "4040".to_string());
    let endpoint = format!("{}:{}", address, port);

    let mut server = tide::new();

    let cors = CorsMiddleware::new()
        .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
        .allow_origin(Origin::from("*"))
        .allow_credentials(false);
    server.with(cors);

    server.at("/ping").get(ping);

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
