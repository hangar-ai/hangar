//! A Discord of one's own, to run a Hangar's bot against with nothing
//! leaving the machine: its REST routes answer as Discord would and keep
//! every call, its gateway says hello and hears the bot identify, and two
//! routes of its own drive it.
//!
//! ```sh
//! cargo run -p hangar-discord --example fake_discord -- 7777
//! # then, in a debug build of Hangar:
//! HANGAR_DISCORD_API=http://127.0.0.1:7777/api \
//! HANGAR_DISCORD_GATEWAY=ws://127.0.0.1:7777/gateway hangar-ai
//! curl -s 127.0.0.1:7777/fake/calls                 # what the bot did
//! curl -s -XPOST 127.0.0.1:7777/fake/send -d '{"t":"MESSAGE_CREATE","d":{…}}'
//! ```

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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

#[derive(Default)]
struct Fake {
    calls: Vec<Value>,
    next: u64,
    gateways: Vec<mpsc::UnboundedSender<Value>>,
}

type Shared = Arc<Mutex<Fake>>;

fn locked(fake: &Shared) -> MutexGuard<'_, Fake> {
    fake.lock().unwrap_or_else(PoisonError::into_inner)
}

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
    let mut fake = locked(&fake);
    fake.next += 1;
    let id = format!("{}", 1000 + fake.next);
    fake.calls
        .push(json!({ "method": method.to_string(), "path": path, "body": body_of(&body) }));
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
    ws.on_upgrade(move |socket| serve(fake, socket))
}

async fn serve(fake: Shared, socket: WebSocket) {
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
                        locked(&fake).calls.push(json!({ "method": "IDENTIFY", "path": "", "body": frame["d"] }));
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

async fn calls(State(fake): State<Shared>) -> Response {
    axum::Json(Value::Array(locked(&fake).calls.clone())).into_response()
}

async fn send(State(fake): State<Shared>, body: Bytes) -> Response {
    let event: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let gateways = locked(&fake).gateways.clone();
    let mut sent = 0;
    for gateway in gateways {
        if gateway.send(event.clone()).is_ok() {
            sent += 1;
        }
    }
    axum::Json(json!({ "sent": sent })).into_response()
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let fake: Shared = Arc::default();
    let app = Router::new()
        .route("/gateway", get(gateway))
        .route("/fake/calls", get(calls))
        .route("/fake/send", post(send))
        .route("/api/{*rest}", any(rest))
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("a port");
    println!(
        "fake Discord on 127.0.0.1:{}",
        listener.local_addr().unwrap().port()
    );
    axum::serve(listener, app).await.unwrap();
}
