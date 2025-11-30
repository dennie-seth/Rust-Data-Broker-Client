use std::collections::HashSet;
use std::io::ErrorKind;
use std::sync::Arc;
use pyo3::pyclass;
use tokio::io::{AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex};

#[derive(Debug, Clone)]
pub struct BrokerClient {
    stream: Arc<Mutex<TcpStream>>,
    files: Arc<Mutex<HashSet<String>>>,
}
#[pyclass]
pub struct PyBrokerClient {
    pub client: Arc<Mutex<BrokerClient>>,
}
#[repr(u8)]
#[derive(Debug, Clone)]
enum Request {
    Enqueue = 1,
    Dequeue = 2,
}
impl Request {
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Request::Enqueue,
            2 => Request::Dequeue,
            _ => unimplemented!(),
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct RequestMessage {
    command: Request,
    payload_size: usize,
    payload: Vec<u8>,
}

impl RequestMessage {
    pub(crate) fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec!();
        bytes.push(self.command.clone() as u8);
        bytes.push(self.payload_size as u8);
        bytes.append(&mut self.payload.clone());
        bytes
    }
}

impl BrokerClient {
    pub(crate) fn new(stream: TcpStream) -> BrokerClient {
        BrokerClient {
            stream: Arc::new(Mutex::new(stream)),
            files: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    pub(crate) async fn send(self, path: &String) -> Result<(), std::io::Error> {
        if std::path::Path::new(path).exists() {
            let file = std::fs::read(path)?;
            if !file.is_empty() {
                self.files.lock().await.insert(path.clone());
                let request_message = RequestMessage {
                    command: Request::Enqueue,
                    payload_size: file.len(),
                    payload: file,
                };
                if self.stream.lock().await.write_all(&request_message.as_bytes()).await.is_err() {
                    return Err(std::io::Error::new(ErrorKind::Other, "failed to send"))?
                }
                return Ok(())
            }
            Err(std::io::Error::new(ErrorKind::Other, "file is empty"))?
        }
        Err(std::io::Error::new(ErrorKind::NotFound, "file not found"))?
    }
}
pub async fn client_send(client: Arc<Mutex<BrokerClient>>, path: &String) -> Result<(), std::io::Error> {
    client.lock().await.clone().send(path).await?;
    Ok(())
}
pub async fn client_connect(url: String) -> Result<Arc<Mutex<BrokerClient>>, std::io::Error> {
    println!("Connecting to server...");
    let stream = TcpStream::connect(url).await;
    if stream.is_ok() {
        println!("Connected to server");
        let broker_client = Arc::new(Mutex::new(BrokerClient::new(stream?)));
        return Ok(broker_client)
    }
    Err(std::io::Error::new(ErrorKind::Other, "failed to connect"))?
}
