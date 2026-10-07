//! A stand-in for the Gemini embeddings API on a local port, so the retries can be checked without
//! the network or a key.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
pub struct RecordedRequest {
    pub body: Value,
}

pub struct GeminiStub {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl GeminiStub {
    /// Answers with `first_statuses` one by one, then with a good reply to every request. A
    /// vector's first value is the last word of the text it is for, read as a number.
    pub async fn start(first_statuses: Vec<u16>) -> GeminiStub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        tokio::spawn(async move {
            let mut statuses = first_statuses.into_iter();
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let Some(request) = read_request(&mut socket).await else {
                    continue;
                };
                let (status, reply) = match statuses.next() {
                    Some(status) => (
                        status,
                        json!({"error": {"code": status, "message": "stub"}}),
                    ),
                    None => (200, good_reply(&request)),
                };
                recorded.lock().unwrap().push(request);
                let body = reply.to_string();
                let head = format!(
                    "HTTP/1.1 {status} stub\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(body.as_bytes()).await.unwrap();
            }
        });
        GeminiStub { address, requests }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(socket: &mut TcpStream) -> Option<RecordedRequest> {
    let mut received = Vec::new();
    let head_end = loop {
        if let Some(at) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        let mut chunk = [0u8; 16384];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&received[..head_end]).into_owned();
    let header = |name: &str| {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length")?.parse().ok()?;
    while received.len() < head_end + length {
        let mut chunk = [0u8; 16384];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    }
    Some(RecordedRequest {
        body: serde_json::from_slice(&received[head_end..head_end + length]).ok()?,
    })
}

fn vector_for(content: &Value) -> Vec<f32> {
    let tag = content["parts"][0]["text"]
        .as_str()
        .and_then(|text| text.rsplit(' ').next())
        .and_then(|word| word.parse().ok())
        .unwrap_or(0.0);
    let mut vector = vec![0.0; 768];
    vector[0] = tag;
    vector
}

fn good_reply(request: &RecordedRequest) -> Value {
    match request.body["requests"].as_array() {
        Some(batch) => {
            let embeddings: Vec<Value> = batch
                .iter()
                .map(|item| json!({"values": vector_for(&item["content"])}))
                .collect();
            json!({"embeddings": embeddings})
        }
        None => json!({"embedding": {"values": vector_for(&request.body["content"])}}),
    }
}
