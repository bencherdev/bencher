// https://github.com/tinysearch/tinysearch/blob/master/bin/src/index.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub title: String,
    pub url: String,
    pub body: Option<String>,
}

pub type Posts = Vec<Post>;

pub fn read(raw: String) -> Result<Posts, serde_json::Error> {
    serde_json::from_str(&raw)
}
