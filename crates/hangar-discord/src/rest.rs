//! Discord's REST API, the calls the connector makes: as the bot (its
//! token), or as one of its webhooks (the token in the path).
//!
//! Each call runs on tokio's blocking pool (ureq). A 429 is waited out and
//! tried again, at most a few times; a route Discord says is spent
//! (`X-RateLimit-Remaining: 0`) is waited for before the next call on it,
//! so the cards edited in batches never meet a 429 in the first place
//! (discord-connector.md, "Limits to respect").

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;

/// Discord's API, version 10.
pub const API: &str = "https://discord.com/api/v10";

/// Tries after a 429, at most.
const RETRIES: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(30);

/// A file sent with a message (`attachment://name` in its components).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct Http {
    base: String,
    token: String,
    agent: ureq::Agent,
    /// Until when each route is spent.
    spent: Arc<Mutex<HashMap<String, Instant>>>,
}

impl std::fmt::Debug for Http {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Http").field("base", &self.base).finish()
    }
}

/// What a call answered that was not a success.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discord answered {}: {}", self.status, self.message)
    }
}

impl Http {
    /// The bot's client; `base` is [`API`] but in the tests.
    pub fn new(base: &str, token: &str) -> Http {
        let loopback = base.starts_with("http://127.0.0.1") || base.starts_with("http://localhost");
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(TIMEOUT))
            .proxy(if loopback {
                None
            } else {
                ureq::Proxy::try_from_env()
            })
            .build();
        Http {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            agent: config.into(),
            spent: Arc::default(),
        }
    }

    /// `method path` with a JSON body, as the bot; the JSON answered
    /// (`Null` for a 204).
    pub async fn call(
        &self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, Refused> {
        self.send(method, path, Payload::Json(body)).await
    }

    /// `method path` with files: a multipart body, the JSON as
    /// `payload_json`.
    pub async fn upload(
        &self,
        method: &'static str,
        path: &str,
        body: Value,
        files: Vec<Upload>,
    ) -> Result<Value, Refused> {
        if files.is_empty() {
            return self.call(method, path, Some(body)).await;
        }
        self.send(method, path, Payload::Files(body, files)).await
    }

    async fn send(
        &self,
        method: &'static str,
        path: &str,
        payload: Payload,
    ) -> Result<Value, Refused> {
        let this = self.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || this.exchange(method, &path, &payload))
            .await
            .unwrap_or_else(|err| {
                Err(Refused {
                    status: 0,
                    message: format!("the call stopped: {err}"),
                })
            })
    }

    /// The route's key for its limit: the path without its query.
    fn route(method: &str, path: &str) -> String {
        format!("{method} {}", path.split('?').next().unwrap_or(path))
    }

    fn exchange(&self, method: &str, path: &str, payload: &Payload) -> Result<Value, Refused> {
        let route = Http::route(method, path);
        for _ in 0..RETRIES {
            let wait = self
                .spent
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&route)
                .map(|until| until.saturating_duration_since(Instant::now()));
            if let Some(wait) = wait.filter(|wait| !wait.is_zero()) {
                std::thread::sleep(wait);
            }
            let (status, headers, body) = self.once(method, path, payload)?;
            if headers.remaining == Some(0)
                && let Some(after) = headers.reset_after
            {
                let now = Instant::now();
                let mut spent = self.spent.lock().unwrap_or_else(PoisonError::into_inner);
                // A route is a path, ids included: what is over is forgotten,
                // or the map would keep one entry per message for good.
                spent.retain(|_, until| *until > now);
                spent.insert(route.clone(), now + after);
            }
            if status == 429 {
                let after = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|body| body.get("retry_after")?.as_f64())
                    .unwrap_or(1.0);
                std::thread::sleep(Duration::from_secs_f64(after.clamp(0.05, 60.0)));
                continue;
            }
            if (200..300).contains(&status) {
                if body.trim().is_empty() {
                    return Ok(Value::Null);
                }
                return serde_json::from_str(&body).map_err(|err| Refused {
                    status,
                    message: format!("not JSON: {err}"),
                });
            }
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|body| {
                    let message = body.get("message")?.as_str()?.to_string();
                    Some(match body.get("errors") {
                        Some(errors) => format!("{message} {errors}"),
                        None => message,
                    })
                })
                .unwrap_or(body);
            return Err(Refused { status, message });
        }
        Err(Refused {
            status: 429,
            message: format!("{method} {path}: still limited after {RETRIES} tries"),
        })
    }

    fn once(
        &self,
        method: &str,
        path: &str,
        payload: &Payload,
    ) -> Result<(u16, Limits, String), Refused> {
        let url = format!("{}{path}", self.base);
        let auth = format!("Bot {}", self.token);
        let failed = |err: ureq::Error| Refused {
            status: 0,
            message: format!("{method} {path}: {err}"),
        };
        let response = match (method, payload) {
            ("GET" | "DELETE", _) => {
                let request = if method == "GET" {
                    self.agent.get(&url)
                } else {
                    self.agent.delete(&url)
                };
                request
                    .header("Authorization", &auth)
                    .header("User-Agent", USER_AGENT)
                    .call()
            }
            (_, Payload::Json(body)) => {
                let request = match method {
                    "POST" => self.agent.post(&url),
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.patch(&url),
                };
                let request = request
                    .header("Authorization", &auth)
                    .header("User-Agent", USER_AGENT);
                match body {
                    Some(body) => request.send_json(body),
                    None => request.send_empty(),
                }
            }
            (_, Payload::Files(body, files)) => {
                let boundary = format!("hangar{}", uuid::Uuid::new_v4().simple());
                let bytes = multipart(&boundary, body, files);
                let request = match method {
                    "POST" => self.agent.post(&url),
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.patch(&url),
                };
                request
                    .header("Authorization", &auth)
                    .header("User-Agent", USER_AGENT)
                    .header(
                        "Content-Type",
                        &format!("multipart/form-data; boundary={boundary}"),
                    )
                    .send(&bytes[..])
            }
        }
        .map_err(failed)?;
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        };
        let limits = Limits {
            remaining: header("x-ratelimit-remaining").and_then(|v| v.parse().ok()),
            reset_after: header("x-ratelimit-reset-after")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|secs| Duration::from_secs_f64(secs.clamp(0.0, 60.0))),
        };
        let mut body = String::new();
        let _ = response.into_body().into_reader().read_to_string(&mut body);
        Ok((status, limits, body))
    }
}

/// Who calls, as Discord asks a bot to say.
const USER_AGENT: &str = concat!(
    "DiscordBot (https://github.com/hangar-ai/hangar, ",
    env!("CARGO_PKG_VERSION"),
    ")"
);

enum Payload {
    Json(Option<Value>),
    Files(Value, Vec<Upload>),
}

struct Limits {
    remaining: Option<u32>,
    reset_after: Option<Duration>,
}

/// `body` as `payload_json` and each file as `files[n]`.
fn multipart(boundary: &str, body: &Value, files: &[Upload]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"payload_json\"\r\nContent-Type: application/json\r\n\r\n{body}\r\n"
        )
        .as_bytes(),
    );
    for (n, file) in files.iter().enumerate() {
        let name = file.name.replace(['"', '\r', '\n'], "_");
        out.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"files[{n}]\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        out.extend_from_slice(&file.bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_multipart_body_holds_the_json_then_each_file() {
        let body = serde_json::json!({ "flags": 32768 });
        let bytes = multipart(
            "b",
            &body,
            &[Upload {
                name: "plan.md".into(),
                bytes: b"# Plan".to_vec(),
            }],
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("--b\r\nContent-Disposition: form-data; name=\"payload_json\""));
        assert!(text.contains("{\"flags\":32768}"));
        assert!(text.contains("name=\"files[0]\"; filename=\"plan.md\""));
        assert!(text.contains("# Plan"));
        assert!(text.ends_with("--b--\r\n"));
    }

    #[test]
    fn a_route_is_its_path_without_the_query() {
        assert_eq!(
            Http::route("POST", "/webhooks/1/t?wait=true"),
            "POST /webhooks/1/t"
        );
    }
}
