use api::{
    db::get_db_connection,
    endpoints::Api,
    util::server::get_server,
};
use tokio::sync::Mutex;

const API_NAME: &str = "Bencher API";

#[tokio::main]
async fn main() -> Result<(), String> {
    let db_connection = get_db_connection().map_err(|e| e.to_string())?;
    get_server(API_NAME, &mut Api, Mutex::new(db_connection))
        .await?
        .await
}
