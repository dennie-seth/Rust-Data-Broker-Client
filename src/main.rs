mod net;

use crate::net::client::client_connect;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    client_connect("127.0.0.1:8080".to_string()).await?;

    Ok(())
}
