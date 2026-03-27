mod net;

use crate::net::client::client_connect;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    let args: Vec<String> = std::env::args().collect();
    let addr = args.windows(2)
        .find(|w| w[0] == "--address")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    client_connect(addr).await?;

    Ok(())
}
