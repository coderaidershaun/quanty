//! A stand-in web host on a local port, which serves the replies a test gives it and records which
//! paths it was asked for.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// What the host answers for one path.
#[derive(Clone)]
pub enum Reply {
    /// Status 200 with these bytes and their length.
    Ok(Vec<u8>),
    /// This status with no body.
    Status(u16),
    /// Status 302 to this location.
    Redirect(String),
    /// Status 200 with no length, and a body that starts as a PDF and never ends: it is written
    /// until the client hangs up.
    Endless,
}

pub struct PdfHost {
    address: SocketAddr,
    requested: Arc<Mutex<Vec<String>>>,
}

impl PdfHost {
    /// Answers each path of `replies`, and status 404 for any other.
    pub async fn start(replies: Vec<(&str, Reply)>) -> PdfHost {
        PdfHost::start_on("127.0.0.1", replies).await
    }

    /// Like `start`, on the loopback address `ip`, so that a test can tell a connection to this
    /// host from one to the same port of another loopback address.
    pub async fn start_on(ip: &str, replies: Vec<(&str, Reply)>) -> PdfHost {
        let listener = TcpListener::bind((ip, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let requested = Arc::new(Mutex::new(Vec::new()));
        let replies: Arc<Vec<(String, Reply)>> = Arc::new(
            replies
                .into_iter()
                .map(|(path, reply)| (path.to_owned(), reply))
                .collect(),
        );
        let recorded = Arc::clone(&requested);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, Arc::clone(&replies), Arc::clone(&recorded)));
            }
        });
        PdfHost { address, requested }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    pub fn port(&self) -> u16 {
        self.address.port()
    }

    /// The path of every request, in the order they came.
    pub fn requested(&self) -> Vec<String> {
        self.requested.lock().unwrap().clone()
    }
}

/// Write errors are ignored: a client that has what it needs hangs up on purpose.
async fn serve(
    mut socket: TcpStream,
    replies: Arc<Vec<(String, Reply)>>,
    requested: Arc<Mutex<Vec<String>>>,
) {
    let Some(path) = read_path(&mut socket).await else {
        return;
    };
    let reply = replies
        .iter()
        .find_map(|(known, reply)| (*known == path).then(|| reply.clone()))
        .unwrap_or(Reply::Status(404));
    requested.lock().unwrap().push(path);
    let _ = write_reply(&mut socket, reply).await;
}

async fn write_reply(socket: &mut TcpStream, reply: Reply) -> std::io::Result<()> {
    match reply {
        Reply::Ok(body) => {
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(head.as_bytes()).await?;
            socket.write_all(&body).await
        }
        Reply::Status(status) => {
            let head = format!(
                "HTTP/1.1 {status} stand-in\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            socket.write_all(head.as_bytes()).await
        }
        Reply::Redirect(location) => {
            let head = format!(
                "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            socket.write_all(head.as_bytes()).await
        }
        Reply::Endless => {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n%PDF-")
                .await?;
            let zeros = [0u8; 8192];
            loop {
                socket.write_all(&zeros).await?;
            }
        }
    }
}

/// The path of the request, read from its head. Nothing the host is asked for has a body.
async fn read_path(socket: &mut TcpStream) -> Option<String> {
    let mut received = Vec::new();
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut chunk = [0u8; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&received);
    let target = head.lines().next()?.split_whitespace().nth(1)?;
    Some(target.to_owned())
}
