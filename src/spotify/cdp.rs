//! A small Chrome DevTools Protocol client: one page target over a WebSocket,
//! calls answered by id and events handed to whoever listens.
//!
//! One I/O thread owns the socket (tungstenite is not split across threads):
//! it writes what callers queued and reads with a short timeout, so a call
//! from any thread is answered while events keep flowing.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use serde_json::{Value, json};
use tungstenite::{Message, stream::MaybeTlsStream};

type Reply = mpsc::Sender<Result<Value, String>>;

/// An event of the page: its method and params.
#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
}

/// A connection to one page target.
pub struct Cdp {
    out: mpsc::Sender<String>,
    pending: Arc<Mutex<HashMap<u64, Reply>>>,
    next_id: AtomicU64,
    closed: Arc<AtomicBool>,
}

impl Cdp {
    /// Connects to a target's WebSocket URL; its events go to `events`.
    pub fn connect(url: &str, events: mpsc::Sender<CdpEvent>) -> Result<Cdp, String> {
        let (mut socket, _) = tungstenite::connect(url).map_err(|e| format!("connect {url}: {e}"))?;
        if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
            stream.set_read_timeout(Some(Duration::from_millis(15))).map_err(|e| e.to_string())?;
            let _ = stream.set_nodelay(true);
        }
        let (out, queued) = mpsc::channel::<String>();
        let pending: Arc<Mutex<HashMap<u64, Reply>>> = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        {
            let pending = pending.clone();
            let closed = closed.clone();
            std::thread::Builder::new()
                .name("nshtify-cdp".into())
                .spawn(move || {
                    'io: loop {
                        loop {
                            match queued.try_recv() {
                                Ok(text) => {
                                    if socket.send(Message::text(text)).is_err() {
                                        break 'io;
                                    }
                                }
                                Err(mpsc::TryRecvError::Empty) => break,
                                Err(mpsc::TryRecvError::Disconnected) => break 'io,
                            }
                        }
                        match socket.read() {
                            Ok(Message::Text(text)) => dispatch(&text, &pending, &events),
                            Ok(Message::Close(_)) => break,
                            Ok(_) => {}
                            Err(tungstenite::Error::Io(e))
                                if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                            Err(e) => {
                                log::warn!("devtools connection ended: {e}");
                                break;
                            }
                        }
                    }
                    closed.store(true, Ordering::SeqCst);
                    for (_, reply) in pending.lock().unwrap().drain() {
                        let _ = reply.send(Err("the browser went away".into()));
                    }
                    let _ = events.send(CdpEvent { method: "nshtify.closed".into(), params: Value::Null });
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(Cdp { out, pending, next_id: AtomicU64::new(1), closed })
    }

    /// Whether the connection is gone.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Calls `method` and waits for its answer.
    pub fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        if self.is_closed() {
            return Err("the browser went away".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let message = json!({ "id": id, "method": method, "params": params }).to_string();
        if self.out.send(message).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err("the browser went away".into());
        }
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("{method} timed out"))
            }
        }
    }

    /// Runs `expression` in the page, awaiting a promise, and answers its
    /// value.
    pub fn evaluate(&self, expression: &str, timeout: Duration) -> Result<Value, String> {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
            timeout,
        )?;
        if let Some(details) = result.get("exceptionDetails") {
            let text = details
                .pointer("/exception/description")
                .or_else(|| details.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("an exception");
            return Err(text.lines().next().unwrap_or(text).to_owned());
        }
        Ok(result.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }
}

fn dispatch(text: &str, pending: &Mutex<HashMap<u64, Reply>>, events: &mpsc::Sender<CdpEvent>) {
    let Ok(message) = serde_json::from_str::<Value>(text) else { return };
    if let Some(id) = message.get("id").and_then(Value::as_u64) {
        if let Some(reply) = pending.lock().unwrap().remove(&id) {
            let result = match message.get("error") {
                Some(error) => Err(error.get("message").and_then(Value::as_str).unwrap_or("error").to_owned()),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = reply.send(result);
        }
    } else if let Some(method) = message.get("method").and_then(Value::as_str) {
        let _ = events.send(CdpEvent {
            method: method.to_owned(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        });
    }
}

/// `GET http://127.0.0.1:<port><path>` of the browser's DevTools HTTP
/// endpoint, answered as JSON.
pub fn http_json(port: u16, path: &str) -> Result<Value, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("devtools on port {port}: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
        .map_err(|e| e.to_string())?;
    // Chromium keeps the connection open: read the head, then as many bytes
    // as it says the body has.
    let mut response = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        response.extend_from_slice(&chunk[..n]);
        if let Some(head_end) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&response[..head_end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok());
            if length.is_some_and(|len| response.len() >= head_end + 4 + len) {
                break;
            }
        }
    }
    let response = String::from_utf8_lossy(&response);
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    serde_json::from_str(body).map_err(|e| format!("devtools answered {path} with no JSON: {e}"))
}
