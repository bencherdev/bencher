use reqwest::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let client = Client::new();
    let response = client
        .put("http://localhost/v0/admin/migrate")
        .send()
        .await
        .unwrap();
    println!("{response:?}");
}
