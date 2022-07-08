use api::{
    api::{
        server,
        Api,
    },
    db::get_db_connection,
};
use tokio::sync::Mutex;

const API_NAME: &str = "Bencher API";

#[tokio::main]
async fn main() -> Result<(), String> {
    let db_connection = get_db_connection().map_err(|e| e.to_string())?;
    server::get_server(API_NAME, &mut Api, Mutex::new(db_connection))
        .await?
        .await
}
