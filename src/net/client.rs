use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

async fn client_send(stream: &mut TcpStream) {
    let (sender, mut receiver) = mpsc::channel::<String>(32);
    tokio::spawn(async move { 
        println!("Provide input:");
        let mut reader = BufReader::new(tokio::io::stdin());
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.is_ok() {
                if sender.send(line).await.is_err() { break; }
            }
        }
    });
    while let Some(line) = receiver.recv().await {
        if line.eq_ignore_ascii_case("q\r\n") || line.eq_ignore_ascii_case("q\n") {
            break;
        }
        println!("Sending {}", line.trim());
        if stream.write_all(line.as_bytes()).await.is_err() { break; }
    }
}
pub async fn client_connect() {
    println!("Connecting to server...");
    let stream = TcpStream::connect("127.0.0.1:8080").await;
    if stream.is_ok() {
        println!("Connected to server");
        let _ = tokio::spawn(async move {
            client_send(&mut stream.unwrap()).await;
        }).await;
    }
}
