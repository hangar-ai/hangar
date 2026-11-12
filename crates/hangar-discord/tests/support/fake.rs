//! A Discord of one's own, for the tests, the benchmark and the example
//! (`examples/fake_discord.rs`): its REST routes keep every call and answer
//! as Discord would, its gateway says hello, hears each bot identify, and
//! hands them the events it is told to (`Discord::send`, or `POST
//! /fake/send`; `GET /fake/calls` lists what the bots did).

#![allow(dead_code)]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One REST call a bot made.
#[derive(Debug, Clone)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub body: Value,
}

#[derive(Default)]
pub struct Fake {
    pub calls: Vec<Call>,
    next: u64,
    /// What the last bot identified with.
    pub identified: Option<Value>,
    /// How many bots identified.
    pub identifies: usize,
    /// Every bot connected: two Hangars may share one.
    gateways: Vec<mpsc::UnboundedSender<Value>>,
    /// How long each REST call takes to answer: the network's round trip.
    latency: Duration,
}

type Shared = Arc<Mutex<Fake>>;

/// A multipart body's `payload_json`, or the JSON itself.
fn body_of(bytes: &[u8]) -> Value {
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        return value;
    }
    let text = String::from_utf8_lossy(bytes);
    text.split("\r\n\r\n")
        .nth(1)
        .and_then(|rest| rest.split("\r\n--").next())
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or(Value::Null)
}

async fn rest(State(fake): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
    let path = uri
        .path_and_query()
        .map(|p| p.as_str().trim_start_matches("/api").to_string())
        .unwrap_or_default();
    let (id, latency) = {
        let mut fake = locked(&fake);
        fake.next += 1;
        fake.calls.push(Call {
            method: method.to_string(),
            path: path.clone(),
            body: body_of(&body),
        });
        (format!("{}", 1000 + fake.next), fake.latency)
    };
    if !latency.is_zero() {
        tokio::time::sleep(latency).await;
    }
    let only = path.split('?').next().unwrap_or_default().to_string();
    let answer = match (method.as_str(), only.as_str()) {
        ("GET", "/users/@me") => json!({ "id": "bot1", "username": "Hangar" }),
        ("GET", "/oauth2/applications/@me") => json!({ "id": "app1" }),
        ("PUT", p) if p.ends_with("/commands") => json!([]),
        ("POST", p) if p.starts_with("/channels/") && p.ends_with("/webhooks") => {
            json!({ "id": format!("hook{id}"), "token": format!("tok{id}") })
        }
        ("POST", p) if p.ends_with("/callback") => return StatusCode::NO_CONTENT.into_response(),
        ("PUT", _) | ("DELETE", _) => return StatusCode::NO_CONTENT.into_response(),
        _ => json!({ "id": id }),
    };
    axum::Json(answer).into_response()
}

async fn gateway(State(fake): State<Shared>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| serve_gateway(fake, socket))
}

async fn serve_gateway(fake: Shared, socket: WebSocket) {
    let (mut write, mut read) = socket.split();
    let hello = json!({ "op": 10, "d": { "heartbeat_interval": 45_000 } });
    let _ = write.send(Message::Text(hello.to_string().into())).await;
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    locked(&fake).gateways.push(tx);
    let mut seq = 0u64;
    loop {
        tokio::select! {
            frame = read.next() => {
                let Some(Ok(Message::Text(text))) = frame else { return };
                let frame: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match frame["op"].as_u64() {
                    Some(2) => {
                        {
                            let mut fake = locked(&fake);
                            fake.identified = Some(frame["d"].clone());
                            fake.identifies += 1;
                        }
                        seq += 1;
                        let ready = json!({ "op": 0, "s": seq, "t": "READY", "d": {
                            "session_id": "s1", "resume_gateway_url": "", "user": { "id": "bot1" } } });
                        let _ = write.send(Message::Text(ready.to_string().into())).await;
                    }
                    Some(1) => {
                        let _ = write.send(Message::Text(json!({ "op": 11 }).to_string().into())).await;
                    }
                    _ => {}
                }
            }
            event = rx.recv() => {
                let Some(event) = event else { return };
                seq += 1;
                let frame = json!({ "op": 0, "s": seq, "t": event["t"], "d": event["d"] });
                let _ = write.send(Message::Text(frame.to_string().into())).await;
            }
        }
    }
}

async fn list_calls(State(fake): State<Shared>) -> Response {
    let calls: Vec<Value> = locked(&fake)
        .calls
        .iter()
        .map(|c| json!({ "method": c.method, "path": c.path, "body": c.body }))
        .collect();
    axum::Json(Value::Array(calls)).into_response()
}

async fn send_event(State(fake): State<Shared>, body: Bytes) -> Response {
    let event: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let gateways = locked(&fake).gateways.clone();
    let sent = gateways
        .into_iter()
        .filter(|gateway| gateway.send(event.clone()).is_ok())
        .count();
    axum::Json(json!({ "sent": sent })).into_response()
}

/// The fake, listening.
pub struct Discord {
    pub fake: Shared,
    /// `127.0.0.1:<port>`.
    pub base: String,
}

impl Discord {
    /// On a port of the system's choosing, answering at once.
    pub async fn start() -> Discord {
        Discord::start_on(0, Duration::ZERO).await
    }

    /// On `port` (0: any), each REST call answered after `latency`.
    pub async fn start_on(port: u16, latency: Duration) -> Discord {
        let fake: Shared = Arc::new(Mutex::new(Fake {
            latency,
            ..Fake::default()
        }));
        let app = Router::new()
            .route("/gateway", get(gateway))
            .route("/fake/calls", get(list_calls))
            .route("/fake/send", post(send_event))
            .route("/api/{*rest}", any(rest))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("a port");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Discord {
            fake,
            base: format!("127.0.0.1:{port}"),
        }
    }

    pub fn api(&self) -> String {
        format!("http://{}/api", self.base)
    }

    pub fn gateway_url(&self) -> String {
        format!("ws://{}/gateway", self.base)
    }

    pub fn calls(&self) -> Vec<Call> {
        locked(&self.fake).calls.clone()
    }

    /// The calls of `method` whose path starts with `path`.
    pub fn calls_to(&self, method: &str, path: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.method == method && c.path.starts_with(path))
            .collect()
    }

    /// The first call of `method` whose body or path holds `needle`.
    pub fn find(&self, method: &str, needle: &str) -> Option<Call> {
        self.calls().into_iter().find(|c| {
            c.method == method && (c.body.to_string().contains(needle) || c.path.contains(needle))
        })
    }

    /// How many calls of `method` hold `needle`.
    pub fn count(&self, method: &str, needle: &str) -> usize {
        locked(&self.fake)
            .calls
            .iter()
            .filter(|c| {
                c.method == method
                    && (c.body.to_string().contains(needle) || c.path.contains(needle))
            })
            .count()
    }

    pub fn identifies(&self) -> usize {
        locked(&self.fake).identifies
    }

    pub fn identified(&self) -> Option<Value> {
        locked(&self.fake).identified.clone()
    }

    /// An event to every bot connected, as the gateway dispatches it.
    pub fn send(&self, kind: &str, data: Value) {
        let gateways = locked(&self.fake).gateways.clone();
        assert!(!gateways.is_empty(), "a bot is connected");
        for gateway in gateways {
            let _ = gateway.send(json!({ "t": kind, "d": data }));
        }
    }
}
