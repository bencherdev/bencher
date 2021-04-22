use std::env;

use anyhow::{anyhow, bail, Result};
use async_std::task;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Request {
    code: String,
}

pub fn exec(code: String) -> Result<String> {
    let exec = task::block_on(async_exec(code));
    Ok("".to_owned())
}

async fn async_exec(code: String) -> Result<String> {
    let address = env::var("ENDPOINT_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("ENDPOINT_PORT").unwrap_or_else(|_| "8080".to_string());
    let endpoint = format!("{}:{}/api/v1/exec", address, port);

    // let data = &Request { code: String };
    // let res = surf::client::Client::post(endpoint)
    //     .body_json(data)?
    //     .await?;

    Ok("".to_owned())
}
