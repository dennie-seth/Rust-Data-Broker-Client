use std::collections::HashSet;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use pyo3::pyclass;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex};
use bytes::BytesMut;

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
#[repr(u8)]
#[derive(Debug, Clone)]
enum Response {
    Succeeded = 1,
    Failed = 2,
}
impl Response {
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Response::Succeeded,
            2 => Response::Failed,
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
#[derive(Debug, Clone)]
pub(crate) struct ResponseMessage {
    status: Response,
    payload_size: u64,
    payload: Vec<u8>,
}
impl ResponseMessage {
    pub(crate) fn new(status: Response, payload_size: u64, payload: Vec<u8>) -> Self {
        Self {
            status,
            payload_size,
            payload,
        }
    }
}
async fn parse_message(buffer: &mut BytesMut) -> Result<ResponseMessage, std::io::Error> {
    let message;
    loop {
        let payload_size = u64::from_be_bytes(buffer[1..9].try_into().unwrap());
        if buffer.len() >= payload_size as usize + 9 {
            message = ResponseMessage::new(Response::from_u8(buffer[0]),
                                           u64::from_le_bytes(buffer[1..9].try_into().unwrap()),
                                           buffer.split_to(payload_size as usize + 9).to_vec());
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(message)
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
    pub(crate) async fn receive(self) -> Result<(), std::io::Error> {
        let mut buffer = BytesMut::with_capacity(1024*4);
        loop {
            self.stream.lock().await.read_buf(&mut buffer).await?;
            if buffer.len() >= 9 {
                match Response::from_u8(buffer[0]) {
                    Response::Succeeded => {
                        match parse_message(&mut buffer).await {
                            Ok(response_message) => {
                                println!("{:?}", response_message);
                            }
                            Err(err) => {
                                return Err(std::io::Error::new(ErrorKind::Other, err.to_string()))?
                            }
                        }
                    }
                    Response::Failed => {
                        return Err(std::io::Error::new(ErrorKind::Other, "response returned error"))?
                    }
                }
            }
        }
    }
}
pub async fn client_send(client: Arc<Mutex<BrokerClient>>, path: &String) -> Result<(), std::io::Error> {
    let client_sender = client.lock().await.clone();
    let path = path.clone();
    tokio::spawn(async move{
        match client_sender.clone().send(&path).await {
            Ok(_) => {}
            Err(err) => {
                println!("{}", err);
            }
        }
    });
    let client_receiver = client.lock().await.clone();
    tokio::spawn(async move {
        match client_receiver.clone().receive().await {
            Ok(_) => {}
            Err(err) => {
                println!("{}", err);
            }
        }
    });
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
