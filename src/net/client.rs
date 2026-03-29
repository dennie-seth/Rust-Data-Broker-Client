use std::collections::HashSet;
use std::io::ErrorKind;
use std::sync::Arc;
use pyo3::pyclass;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex};
use bytes::BytesMut;

#[derive(Debug, Clone)]
pub struct BrokerClient {
    stream: Arc<Mutex<TcpStream>>,
    files: Arc<Mutex<HashSet<String>>>,
    client_id: u128,
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
    CreateQ = 3,
    DeleteQ = 4,
    PeekM = 5,
    DeleteM = 6,
    Succeeded = 7,
    Failed = 8,
    Requeue = 9,
    UpdateM = 10,
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
/// Request frame: [1b command][16b client_id BE][8b payload_size BE][64b queue_name null-padded][payload]
#[derive(Debug, Clone)]
pub(crate) struct RequestMessage {
    command: Request,
    client_id: u128,
    queue_name: String,
    payload_size: u64,
    payload: Vec<u8>,
}
impl RequestMessage {
    pub(crate) fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.command.clone() as u8);
        bytes.extend_from_slice(&self.client_id.to_be_bytes()); // 16 bytes big-endian
        bytes.extend_from_slice(&self.payload_size.to_be_bytes()); // 8 bytes big-endian
        // queue_name: null-padded to exactly 64 bytes
        let name_bytes = self.queue_name.as_bytes();
        let mut name_padded = [0u8; 64];
        let len = name_bytes.len().min(64);
        name_padded[..len].copy_from_slice(&name_bytes[..len]);
        bytes.extend_from_slice(&name_padded);
        bytes.extend_from_slice(&self.payload);
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
fn parse_message(buffer: &mut BytesMut) -> Result<ResponseMessage, std::io::Error> {
    let payload_size = u64::from_be_bytes(buffer[1..9].try_into().unwrap());
    let message = ResponseMessage::new(Response::from_u8(buffer[0]),
                                       payload_size,
                                       buffer.split_to(payload_size as usize + 9)[9..].to_vec());
    Ok(message)
}
impl BrokerClient {
    pub(crate) fn new(stream: TcpStream) -> BrokerClient {
        BrokerClient {
            stream: Arc::new(Mutex::new(stream)),
            files: Arc::new(Mutex::new(HashSet::new())),
            client_id: {
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
                let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                ((secs as u128) << 64) | (nanos as u128)
            },
        }
    }
    pub(crate) async fn send(self, path: &String, queue_name: &String) -> Result<(), std::io::Error> {
        if std::path::Path::new(path).exists() {
            let file = std::fs::read(path)?;
            if !file.is_empty() {
                self.files.lock().await.insert(path.clone());
                let request_message = RequestMessage {
                    command: Request::Enqueue,
                    client_id: self.client_id,
                    queue_name: queue_name.clone(),
                    payload_size: file.len() as u64,
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
                let payload_size = u64::from_be_bytes(buffer[1..9].try_into().unwrap());
                if buffer.len() >= payload_size as usize + 9 {
                    match Response::from_u8(buffer[0]) {
                        Response::Succeeded => {
                            match parse_message(&mut buffer) {
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
}
pub async fn client_send(client: Arc<Mutex<BrokerClient>>, path: &String, queue_name: &String) -> Result<(), std::io::Error> {
    let client_sender = client.lock().await.clone();
    let path = path.clone();
    let queue_name = queue_name.clone();
    tokio::spawn(async move{
        match client_sender.clone().send(&path, &queue_name).await {
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
