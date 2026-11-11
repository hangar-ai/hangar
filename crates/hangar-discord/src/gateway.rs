//! Discord's gateway: the WebSocket the bot hears its events on.
//!
//! Hello, then Identify (or Resume after a cut), heartbeats at the interval
//! Discord gave, and every dispatch (`op 0`) handed on as a [`Dispatch`].
//! A dropped connection resumes where it was (`resume_gateway_url`,
//! `session_id`, the last sequence); Discord asking for a reconnect or a
//! session it no longer knows starts over. What can never work (a token
//! refused, intents not allowed) ends the loop with why.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// What the bot hears: the guild's messages, their content, and the
/// interactions (which every bot gets).
pub const GUILDS: u64 = 1 << 0;
pub const GUILD_MESSAGES: u64 = 1 << 9;
pub const DIRECT_MESSAGES: u64 = 1 << 12;
pub const MESSAGE_CONTENT: u64 = 1 << 15;
pub const INTENTS: u64 = GUILDS | GUILD_MESSAGES | DIRECT_MESSAGES | MESSAGE_CONTENT;

/// One event: its name (`MESSAGE_CREATE`, `INTERACTION_CREATE`, `READY`…)
/// and its data.
#[derive(Debug, Clone, PartialEq)]
pub struct Dispatch {
    pub kind: String,
    pub data: Value,
}

/// Why the gateway will not have the bot.
fn fatal(code: u16) -> Option<&'static str> {
    match code {
        4004 => Some("Discord refused the bot's token: check it in the bot's settings."),
        4010..=4013 => Some("Discord refused how the bot connects."),
        4014 => Some(
            "Discord refused the intents: turn on Message Content Intent in the Developer Portal (Bot › Privileged Gateway Intents).",
        ),
        _ => None,
    }
}

/// Where a session resumes.
#[derive(Debug, Clone, Default)]
struct Session {
    id: Option<String>,
    resume_url: Option<String>,
    seq: Option<u64>,
}

/// Follows the gateway at `url` until `events` is dropped, or Discord says
/// the bot can never connect (the error).
pub async fn run(
    url: String,
    token: String,
    intents: u64,
    events: mpsc::UnboundedSender<Dispatch>,
) -> Result<(), String> {
    let mut session = Session::default();
    let mut backoff = Duration::from_secs(1);
    loop {
        if events.is_closed() {
            return Ok(());
        }
        let base = session.resume_url.clone().unwrap_or_else(|| url.clone());
        let started = std::time::Instant::now();
        match connect(&base, &token, intents, &mut session, &events).await {
            Ended::Fatal(why) => return Err(why),
            Ended::Gone => return Ok(()),
            Ended::Fresh => {
                session = Session::default();
            }
            Ended::Cut => {}
        }
        if started.elapsed() > Duration::from_secs(60) {
            backoff = Duration::from_secs(1);
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

enum Ended {
    /// The connection dropped: resume.
    Cut,
    /// Start a new session.
    Fresh,
    /// Never again, and why.
    Fatal(String),
    /// Nobody listens any more.
    Gone,
}

async fn connect(
    base: &str,
    token: &str,
    intents: u64,
    session: &mut Session,
    events: &mpsc::UnboundedSender<Dispatch>,
) -> Ended {
    let url = gateway_url(base);
    let Ok((socket, _)) = tokio_tungstenite::connect_async(&url).await else {
        return Ended::Cut;
    };
    let (mut write, mut read) = socket.split();

    // Hello.
    let interval = loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                let Ok(frame) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if frame["op"] == 10 {
                    break frame["d"]["heartbeat_interval"].as_u64().unwrap_or(41_250);
                }
            }
            Some(Ok(_)) => continue,
            _ => return Ended::Cut,
        }
    };
    let hello = match (&session.id, session.seq) {
        (Some(id), Some(seq)) => json!({
            "op": 6,
            "d": { "token": token, "session_id": id, "seq": seq },
        }),
        _ => json!({
            "op": 2,
            "d": {
                "token": token,
                "intents": intents,
                "properties": { "os": std::env::consts::OS, "browser": "hangar", "device": "hangar" },
            },
        }),
    };
    if write
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        return Ended::Cut;
    }

    let mut beat = tokio::time::interval(Duration::from_millis(interval));
    beat.tick().await;
    let mut acked = true;
    loop {
        tokio::select! {
            _ = beat.tick() => {
                if !acked {
                    // A zombied connection: resume on a new one.
                    return Ended::Cut;
                }
                acked = false;
                let frame = json!({ "op": 1, "d": session.seq });
                if write.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return Ended::Cut;
                }
            }
            frame = read.next() => {
                let text = match frame {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(close))) => {
                        let code = close.map(|c| u16::from(c.code)).unwrap_or(1000);
                        if let Some(why) = fatal(code) {
                            return Ended::Fatal(why.to_string());
                        }
                        // A session Discord cannot resume.
                        if matches!(code, 4007 | 4009) {
                            return Ended::Fresh;
                        }
                        return Ended::Cut;
                    }
                    Some(Ok(_)) => continue,
                    _ => return Ended::Cut,
                };
                let Ok(frame) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                match frame["op"].as_u64() {
                    Some(0) => {
                        if let Some(seq) = frame["s"].as_u64() {
                            session.seq = Some(seq);
                        }
                        let kind = frame["t"].as_str().unwrap_or_default().to_string();
                        let data = frame["d"].clone();
                        if kind == "READY" {
                            session.id = data["session_id"].as_str().map(str::to_string);
                            session.resume_url =
                                data["resume_gateway_url"].as_str().map(str::to_string);
                        }
                        if events.send(Dispatch { kind, data }).is_err() {
                            let _ = write.send(Message::Close(Some(close_normal()))).await;
                            return Ended::Gone;
                        }
                    }
                    Some(1) => {
                        let frame = json!({ "op": 1, "d": session.seq });
                        if write.send(Message::Text(frame.to_string().into())).await.is_err() {
                            return Ended::Cut;
                        }
                    }
                    Some(7) => return Ended::Cut,
                    Some(9) => {
                        if frame["d"] == true {
                            return Ended::Cut;
                        }
                        return Ended::Fresh;
                    }
                    Some(11) => acked = true,
                    _ => {}
                }
            }
        }
    }
}

/// The gateway's address with the version and encoding asked for:
/// `wss://gateway.discord.gg/?v=10&encoding=json`.
fn gateway_url(base: &str) -> String {
    let base = base.trim_end_matches('/');
    let has_path = base
        .split_once("://")
        .is_some_and(|(_, rest)| rest.contains('/'));
    let separator = if base.contains('?') {
        "&"
    } else if has_path {
        "?"
    } else {
        "/?"
    };
    format!("{base}{separator}v=10&encoding=json")
}

fn close_normal() -> tokio_tungstenite::tungstenite::protocol::CloseFrame {
    tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: CloseCode::Normal,
        reason: "".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gateway_is_asked_for_version_10_in_json() {
        assert_eq!(
            gateway_url("wss://gateway.discord.gg"),
            "wss://gateway.discord.gg/?v=10&encoding=json"
        );
        assert_eq!(
            gateway_url("ws://127.0.0.1:9/gateway"),
            "ws://127.0.0.1:9/gateway?v=10&encoding=json"
        );
    }

    #[test]
    fn the_codes_that_can_never_work_say_why() {
        assert!(fatal(4004).unwrap().contains("token"));
        assert!(fatal(4014).unwrap().contains("Message Content"));
        assert_eq!(fatal(4000), None);
        assert_eq!(fatal(1001), None);
    }
}
