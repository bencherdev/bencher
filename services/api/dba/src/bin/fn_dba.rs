const API_NAME: &str = "dba";

use fn_dba::api::Api;

#[tokio::main]
async fn main() -> Result<(), String> {
    let server = util::server::get_server(API_NAME, &mut Api, ()).await?;
    server.await
}
