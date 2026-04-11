use std::io::ErrorKind;
use std::sync::Arc;
use pyo3::prelude::*;
use pyo3::pyclass;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex};
use bytes::BytesMut;

const META_SIZE: usize = 56;

#[derive(Debug, Clone)]
pub struct BrokerClient {
    stream: Arc<Mutex<TcpStream>>,
    client_id: u128,
}
#[pyclass]
pub struct PyBrokerClient {
    pub client: Arc<Mutex<BrokerClient>>,
}
#[pyclass]
#[derive(Debug, Clone)]
pub struct MessageMeta {
    #[pyo3(get)]
    pub id: u128,
    #[pyo3(get)]
    pub publisher_id: u128,
    #[pyo3(get)]
    pub timestamp: u64,
    #[pyo3(get)]
    pub locked_by: Option<u128>,
}
impl MessageMeta {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let id = u128::from_be_bytes(bytes[0..16].try_into().unwrap());
        let publisher_id = u128::from_be_bytes(bytes[16..32].try_into().unwrap());
        let timestamp = u64::from_be_bytes(bytes[32..40].try_into().unwrap());
        let locked_raw = u128::from_be_bytes(bytes[40..56].try_into().unwrap());
        let locked_by = if locked_raw == u128::MAX { None } else { Some(locked_raw) };
        MessageMeta { id, publisher_id, timestamp, locked_by }
    }
}
#[pyclass]
#[derive(Debug, Clone)]
pub struct QueueStat {
    #[pyo3(get)]
    pub queue_name: String,
    #[pyo3(get)]
    pub total_messages: u64,
    #[pyo3(get)]
    pub total_bytes: u64,
    #[pyo3(get)]
    pub total_messages_locked: u64,
    #[pyo3(get)]
    pub total_bytes_locked: u64,
}
pub fn parse_list_response(payload: &[u8]) -> Vec<MessageMeta> {
    payload.chunks_exact(META_SIZE)
        .map(MessageMeta::from_bytes)
        .collect()
}
pub fn parse_dequeue_response(payload: &[u8]) -> Result<(MessageMeta, Vec<u8>), std::io::Error> {
    if payload.len() < META_SIZE {
        return Err(std::io::Error::new(ErrorKind::InvalidData, "dequeue response too short for meta"));
    }
    let meta = MessageMeta::from_bytes(&payload[..META_SIZE]);
    let data = payload[META_SIZE..].to_vec();
    Ok((meta, data))
}
/// Parse NetStats response. Layout (big-endian):
///   [4b count u32][for each stat: [4b length u32][stat bytes]]
/// stat bytes layout:
///   [2b name_len u16][name bytes][8b total_messages][8b total_bytes]
///   [8b total_messages_locked][8b total_bytes_locked]
/// The server writes the four counters via `usize::to_be_bytes()`; on the 64-bit
/// Linux/Windows targets the server ships for, `usize` is 8 bytes.
pub fn parse_stats_response(payload: &[u8]) -> Result<Vec<QueueStat>, std::io::Error> {
    let short = || std::io::Error::new(ErrorKind::InvalidData, "stats response truncated");
    if payload.len() < 4 { return Err(short()); }
    let count = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    let mut offset = 4usize;
    let mut stats = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 4 > payload.len() { return Err(short()); }
        let stat_len = u32::from_be_bytes(payload[offset..offset+4].try_into().unwrap()) as usize;
        offset += 4;
        if offset + stat_len > payload.len() { return Err(short()); }
        let stat = &payload[offset..offset+stat_len];
        offset += stat_len;

        if stat.len() < 2 { return Err(short()); }
        let name_len = u16::from_be_bytes(stat[0..2].try_into().unwrap()) as usize;
        let mut p = 2usize;
        if p + name_len + 32 > stat.len() { return Err(short()); }
        let queue_name = String::from_utf8(stat[p..p+name_len].to_vec())
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, format!("invalid utf-8 in queue name: {e}")))?;
        p += name_len;
        let total_messages        = u64::from_be_bytes(stat[p..p+8].try_into().unwrap()); p += 8;
        let total_bytes           = u64::from_be_bytes(stat[p..p+8].try_into().unwrap()); p += 8;
        let total_messages_locked = u64::from_be_bytes(stat[p..p+8].try_into().unwrap()); p += 8;
        let total_bytes_locked    = u64::from_be_bytes(stat[p..p+8].try_into().unwrap());
        stats.push(QueueStat {
            queue_name,
            total_messages,
            total_bytes,
            total_messages_locked,
            total_bytes_locked,
        });
    }
    Ok(stats)
}
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum Request {
    Enqueue = 1,
    Dequeue = 2,
    CreateQ = 3,
    DeleteQ = 4,
    ListM = 5,
    DeleteM = 6,
    Succeeded = 7,
    Failed = 8,
    Requeue = 9,
    UpdateM = 10,
    UpdateQ = 11,
    NetStats = 12,
}
impl Request {
    pub fn from_u8(value: u8) -> Result<Self, std::io::Error> {
        match value {
            1 => Ok(Request::Enqueue),
            2 => Ok(Request::Dequeue),
            3 => Ok(Request::CreateQ),
            4 => Ok(Request::DeleteQ),
            5 => Ok(Request::ListM),
            6 => Ok(Request::DeleteM),
            7 => Ok(Request::Succeeded),
            8 => Ok(Request::Failed),
            9 => Ok(Request::Requeue),
            10 => Ok(Request::UpdateM),
            11 => Ok(Request::UpdateQ),
            12 => Ok(Request::NetStats),
            _ => Err(std::io::Error::new(ErrorKind::InvalidInput, format!("unknown command: {}", value))),
        }
    }
}
#[repr(u8)]
#[derive(Debug, Clone)]
enum Response {
    Succeeded = 1,
    Failed = 2,
}
impl Response {
    pub(crate) fn from_u8(value: u8) -> Result<Self, std::io::Error> {
        match value {
            1 => Ok(Response::Succeeded),
            2 => Ok(Response::Failed),
            _ => Err(std::io::Error::new(ErrorKind::InvalidData, format!("unknown response status: {}", value))),
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
fn error_code_message(code: u16) -> &'static str {
    match code {
        0   => "unknown request type",
        1   => "failed to parse request message",
        2   => "failed to send response",
        3   => "payload exceeds maximum size",
        100 => "queue hash not found",
        101 => "queue not found",
        102 => "queue already exists",
        103 => "queue does not exist",
        104 => "queue is empty",
        105 => "failed to get queue stats",
        200 => "payload missing message id",
        201 => "no such message id",
        202 => "message already locked",
        203 => "no such message id locked",
        204 => "message not locked by client",
        205 => "message not in queue",
        300 => "invalid config payload size",
        301 => "config bytes empty",
        302 => "invalid config auto_fail",
        303 => "invalid config fail_timeout",
        _   => "unknown error",
    }
}
fn parse_error_payload(payload: &[u8]) -> String {
    if payload.len() >= 2 {
        let code = u16::from_be_bytes([payload[0], payload[1]]);
        format!("server error {}: {}", code, error_code_message(code))
    } else {
        "server returned error with no details".to_string()
    }
}
fn parse_message(buffer: &mut BytesMut) -> Result<ResponseMessage, std::io::Error> {
    let payload_size = u64::from_be_bytes(buffer[1..9].try_into().unwrap());
    let payload_len: usize = payload_size.try_into()
        .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "payload size exceeds platform limit"))?;
    let total = payload_len.checked_add(9)
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "payload size overflow"))?;
    let message = ResponseMessage::new(Response::from_u8(buffer[0])?,
                                       payload_size,
                                       buffer.split_to(total)[9..].to_vec());
    Ok(message)
}
impl BrokerClient {
    pub(crate) fn new(stream: TcpStream) -> BrokerClient {
        BrokerClient {
            stream: Arc::new(Mutex::new(stream)),
            client_id: {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                ((duration.as_secs() as u128) << 64) | (duration.subsec_nanos() as u128)
            },
        }
    }
    pub(crate) async fn send(&self, command: Request, payload: Vec<u8>, queue_name: &str) -> Result<(), std::io::Error> {
        if queue_name.len() > 64 {
            return Err(std::io::Error::new(ErrorKind::InvalidInput, "queue name exceeds 64 bytes"));
        }
        let request_message = RequestMessage {
            command,
            client_id: self.client_id,
            queue_name: queue_name.to_string(),
            payload_size: payload.len() as u64,
            payload,
        };
        self.stream.lock().await.write_all(&request_message.as_bytes()).await?;
        Ok(())
    }
    pub(crate) async fn receive(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buffer = BytesMut::with_capacity(1024*4);
        let mut stream = self.stream.lock().await;
        loop {
            let n = stream.read_buf(&mut buffer).await?;
            if n == 0 {
                return Err(std::io::Error::new(ErrorKind::UnexpectedEof, "server closed connection"));
            }
            if buffer.len() >= 9 {
                let payload_size = u64::from_be_bytes(buffer[1..9].try_into().unwrap());
                let payload_len: usize = payload_size.try_into()
                    .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "payload size exceeds platform limit"))?;
                let total = payload_len.checked_add(9)
                    .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "payload size overflow"))?;
                if buffer.len() >= total {
                    match Response::from_u8(buffer[0])? {
                        Response::Succeeded => {
                            let response = parse_message(&mut buffer)?;
                            return Ok(response.payload);
                        }
                        Response::Failed => {
                            let response = parse_message(&mut buffer)?;
                            let msg = parse_error_payload(&response.payload);
                            return Err(std::io::Error::new(ErrorKind::Other, msg));
                        }
                    }
                }
            }
        }
    }
}
pub async fn client_send(client: Arc<Mutex<BrokerClient>>, command: u8, payload: Vec<u8>, queue_name: &str) -> Result<Vec<u8>, std::io::Error> {
    let command = Request::from_u8(command)?;
    let broker = client.lock().await;
    broker.send(command, payload, queue_name).await?;
    broker.receive().await
}
pub async fn client_connect(url: String) -> Result<Arc<Mutex<BrokerClient>>, std::io::Error> {
    println!("Connecting to server...");
    match TcpStream::connect(url).await {
        Ok(stream) => {
            println!("Connected to server");
            Ok(Arc::new(Mutex::new(BrokerClient::new(stream))))
        }
        Err(err) => Err(std::io::Error::new(ErrorKind::Other, format!("failed to connect: {err}")))
    }
}
