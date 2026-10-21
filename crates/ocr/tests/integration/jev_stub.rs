//! A stand-in for the Jev API on a local port, so the key check can be tried without the network
//! or a key.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
pub struct RecordedRequest {
    pub method: String,
    /// The path with its query, as it was sent.
    pub target: String,
    pub authorization: Option<String>,
}

pub struct JevStub {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl JevStub {
    /// Answers with `first_statuses` one by one, then with a good reply to every request.
    pub async fn start(first_statuses: Vec<u16>) -> JevStub {
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
                recorded.lock().unwrap().push(request);
                let status = statuses.next().unwrap_or(200);
                let body = r#"{"data":[]}"#;
                let head = format!(
                    "HTTP/1.1 {status} stub\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(body.as_bytes()).await.unwrap();
            }
        });
        JevStub { address, requests }
    }

    /// Everything before `/models`, as the real API has it.
    pub fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

/// Reads the head of one request. Nothing the stand-in is asked for has a body.
async fn read_request(socket: &mut TcpStream) -> Option<RecordedRequest> {
    let mut received = Vec::new();
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut chunk = [0u8; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&received).into_owned();
    let mut request_line = head.lines().next()?.split_whitespace();
    let method = request_line.next()?.to_owned();
    let target = request_line.next()?.to_owned();
    let authorization = head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_owned())
    });
    Some(RecordedRequest {
        method,
        target,
        authorization,
    })
}
