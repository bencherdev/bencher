const API_NAME: &str = "reports";

use fn_reports::api::Api;

#[tokio::main]
async fn main() -> Result<(), String> {
    let server = util::server::get_server(API_NAME, &mut Api, ()).await?;
    server.await
}
