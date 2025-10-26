mod net;

use crate::net::client;
use crate::net::client::client_connect;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    println!("Hello, world!");
    client_connect().await;
}
