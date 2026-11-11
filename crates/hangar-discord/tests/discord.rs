//! The connector against a Discord of the test's own: its REST routes
//! record every call and answer as Discord would, its gateway says hello,
//! hears the bot identify, and hands it the events the test makes up. The
//! engine runs with no window over shells that only record what they are
//! typed.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{Method, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use hangar_engine::control::port::ModelCmd;
use hangar_engine::host::{Config, EngineHost, Spawn, TermEvent, TermSink, Terminals};
use hangar_engine::orchestration::events::OrchestratorEvent;
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};
use hangar_i18n::Locale;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/* ── shells that record ─────────────────────────────────────────── */

#[derive(Default)]
struct Shells {
    alive: Mutex<HashSet<String>>,
    typed: Mutex<Vec<(String, String)>>,
}

impl Shells {
    fn typed_into(&self, pane: &str) -> String {
        locked(&self.typed)
            .iter()
            .filter(|(id, _)| id == pane)
            .map(|(_, text)| text.as_str())
            .collect()
    }
}

impl Terminals for Shells {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        locked(&self.alive).insert(spawn.id.clone());
        sink(TermEvent::Output(spawn.id));
        Ok(())
    }
    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        locked(&self.typed).push((pane.to_string(), data.to_string()));
        Ok(())
    }
    fn alive(&self, pane: &str) -> bool {
        locked(&self.alive).contains(pane)
    }
    fn kill(&self, pane: &str) -> Result<(), String> {
        locked(&self.alive).remove(pane);
        Ok(())
    }
    fn lines(&self, pane: &str, _n: usize) -> Option<Vec<String>> {
        self.alive(pane).then(Vec::new)
    }
    fn screen(&self, pane: &str) -> Option<Vec<String>> {
        self.alive(pane).then(Vec::new)
    }
    fn kill_all(&self) {
        locked(&self.alive).clear();
    }
}

/* ── a Discord of the test's own ────────────────────────────────── */

#[derive(Debug, Clone)]
struct Call {
    method: String,
    path: String,
    body: Value,
}

#[derive(Default)]
struct Fake {
    calls: Vec<Call>,
    next: u64,
    identified: Option<Value>,
    gateway: Option<mpsc::UnboundedSender<Value>>,
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
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let path = path.trim_start_matches("/api").to_string();
    let mut fake = locked(&fake);
    fake.next += 1;
    let id = format!("{}", 1000 + fake.next);
    fake.calls.push(Call {
        method: method.to_string(),
        path: path.clone(),
        body: body_of(&body),
    });
    let only = path.split('?').next().unwrap_or_default().to_string();
    let answer = match (method.as_str(), only.as_str()) {
        ("GET", "/users/@me") => json!({ "id": "bot1", "username": "Hangar" }),
        ("GET", "/oauth2/applications/@me") => json!({ "id": "app1" }),
        ("PUT", p) if p.ends_with("/commands") => json!([]),
        ("POST", p) if p.ends_with("/webhooks") && p.starts_with("/channels/") => {
            json!({ "id": format!("hook{id}"), "token": format!("tok{id}") })
        }
        ("POST", p) if p.ends_with("/callback") => {
            return axum::http::StatusCode::NO_CONTENT.into_response();
        }
        ("PUT", _) | ("DELETE", _) => return axum::http::StatusCode::NO_CONTENT.into_response(),
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
    locked(&fake).gateway = Some(tx);
    let mut seq = 0u64;
    loop {
        tokio::select! {
            frame = read.next() => {
                let Some(Ok(Message::Text(text))) = frame else { return };
                let frame: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match frame["op"].as_u64() {
                    Some(2) => {
                        locked(&fake).identified = Some(frame["d"].clone());
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

struct Discord {
    fake: Shared,
    base: String,
}

impl Discord {
    async fn start() -> Discord {
        let fake: Shared = Arc::default();
        let app = Router::new()
            .route("/gateway", get(gateway))
            .route("/api/{*rest}", any(rest))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Discord {
            fake,
            base: format!("127.0.0.1:{port}"),
        }
    }

    fn calls(&self) -> Vec<Call> {
        locked(&self.fake).calls.clone()
    }

    /// The calls of `method` whose path starts with `path`.
    fn calls_to(&self, method: &str, path: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.method == method && c.path.starts_with(path))
            .collect()
    }

    /// A call whose body (or path) holds `needle`, after `from` calls.
    fn find(&self, method: &str, needle: &str) -> Option<Call> {
        self.calls().into_iter().find(|c| {
            c.method == method && (c.body.to_string().contains(needle) || c.path.contains(needle))
        })
    }

    fn send(&self, kind: &str, data: Value) {
        let gateway = locked(&self.fake)
            .gateway
            .clone()
            .expect("the bot is connected");
        gateway.send(json!({ "t": kind, "d": data })).unwrap();
    }
}

/// Waits up to twenty seconds for `found` (a loaded machine is slow).
async fn eventually<T>(what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    for _ in 0..2_000 {
        if let Some(found) = found() {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never happened: {what}");
}

fn pane(id: &str, name: &str, role: Option<PaneRole>) -> Pane {
    Pane {
        id: id.into(),
        name: name.into(),
        agent: AgentId::Claude,
        role,
        ..Pane::default()
    }
}

struct Rig {
    host: EngineHost,
    shells: Arc<Shells>,
    discord: Discord,
    dir: PathBuf,
    _root: tempfile::TempDir,
}

async fn rig() -> Rig {
    let root = tempfile::tempdir().unwrap();
    let state = AppState {
        workspaces: vec![Workspace {
            id: "shop".into(),
            name: "My Shop".into(),
            cwd: root.path().to_string_lossy().into_owned(),
            panes: vec![pane("leo", "Leo", Some(PaneRole::Orchestrator))],
            ..Workspace::default()
        }],
        settings: Settings {
            locale: Some(Locale::Fr),
            ..Settings::default()
        },
        ..AppState::default()
    };
    let dir = root.path().join("node");
    let file = dir.join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(root.path())),
        terminals: shells.clone(),
        state_file: file,
    })
    .unwrap();
    host.start();
    let discord = Discord::start().await;
    hangar_discord::Settings {
        token: "secret".into(),
        guild: "g1".into(),
        ..hangar_discord::Settings::default()
    }
    .save(&dir)
    .unwrap();
    let options = hangar_discord::Options {
        api: format!("http://{}/api", discord.base),
        gateway: Some(format!("ws://{}/gateway", discord.base)),
        dir: dir.clone(),
    };
    let bot_host = host.clone();
    tokio::spawn(async move {
        if let Err(error) = hangar_discord::run(bot_host, options).await {
            panic!("the connector stopped: {error}");
        }
    });
    Rig {
        host,
        shells,
        discord,
        dir,
        _root: root,
    }
}

fn interaction(
    id: &str,
    user: &str,
    channel: &str,
    kind: u8,
    data: Value,
    message: Option<&str>,
) -> Value {
    let mut i = json!({
        "id": id, "token": format!("{id}tok"), "type": kind, "channel_id": channel,
        "member": { "user": { "id": user } }, "data": data,
    });
    if let Some(message) = message {
        i["message"] = json!({ "id": message });
    }
    i
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_on_discord_from_pairing_to_a_worker_s_thread() {
    let rig = rig().await;
    let d = &rig.discord;

    // The bot identifies with its token and the intents it needs.
    let identified = eventually("identify", || locked(&d.fake).identified.clone()).await;
    assert_eq!(identified["token"], "secret");
    assert_eq!(identified["intents"].as_u64().unwrap() & (1 << 15), 1 << 15);
    // Its commands, on the guild alone.
    eventually("the commands registered", || {
        d.calls_to("PUT", "/applications/app1/guilds/g1/commands")
            .pop()
    })
    .await;

    // A category, the workspace's channel, its webhook, its dashboard pinned.
    let made = eventually("the workspace's channel", || {
        d.calls_to("POST", "/guilds/g1/channels")
            .into_iter()
            .find(|c| c.body["type"] == 0)
    })
    .await;
    assert_eq!(made.body["name"], "my-shop");
    let category = d.calls_to("POST", "/guilds/g1/channels")[0].clone();
    assert_eq!(category.body["type"], 4);
    assert_eq!(category.body["permission_overwrites"][0]["id"], "bot1");
    let layout: Value = eventually("the layout saved", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .filter(|layout| layout["workspaces"]["shop"]["dashboard"].is_string())
    })
    .await;
    let channel = layout["workspaces"]["shop"]["channel"]
        .as_str()
        .unwrap()
        .to_string();
    let hook = layout["workspaces"]["shop"]["webhook"][0]
        .as_str()
        .unwrap()
        .to_string();
    eventually("the dashboard pinned", || {
        d.calls_to("PUT", &format!("/channels/{channel}/messages/pins/"))
            .pop()
    })
    .await;

    // Not paired yet: the code in the settings pairs whoever sends it.
    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(rig.dir.join("discord.json")).unwrap())
            .unwrap();
    let code = settings["pairing"].as_str().unwrap().to_string();
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m1", "channel_id": channel, "content": "hello",
        "author": { "id": "u1" } }),
    );
    eventually("told to pair", || d.find("POST", "pair <code>")).await;
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m2", "channel_id": channel,
        "content": format!("pair {code}"), "author": { "id": "u1" } }),
    );
    eventually("paired", || {
        std::fs::read_to_string(rig.dir.join("discord.json"))
            .ok()
            .filter(|raw| raw.contains("\"owner\": \"u1\""))
    })
    .await;

    // What the owner writes in the channel reaches the orchestrator.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m3", "channel_id": channel,
        "content": "Ajoute un panier", "author": { "id": "u1" } }),
    );
    // Typed when Leo is idle, queued (⏳) while it works.
    eventually("Leo given the message", || {
        let queued = !d
            .calls_to(
                "PUT",
                &format!("/channels/{channel}/messages/m3/reactions/%E2%8F%B3"),
            )
            .is_empty();
        (queued || rig.shells.typed_into("leo").contains("Ajoute un panier")).then_some(())
    })
    .await;
    eventually("the message marked", || {
        d.calls_to(
            "PUT",
            &format!("/channels/{channel}/messages/m3/reactions/"),
        )
        .pop()
    })
    .await;
    // Anyone else is not heard.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m4", "channel_id": channel,
        "content": "rm -rf /", "author": { "id": "intruder" } }),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!rig.shells.typed_into("leo").contains("rm -rf /"));

    // A turn of Leo's: its card under Leo's name, its calls, then folded.
    let core = rig.host.core().clone();
    let bridge = |event: Value| {
        core.emit(hangar_core::Event::BridgeEvent {
            pane_id: "leo".into(),
            event,
        })
        .unwrap();
    };
    bridge(json!({ "kind": "hello", "sessionId": "s1", "model": null, "version": "2.1" }));
    bridge(json!({ "kind": "turn", "phase": "start" }));
    let card = eventually("the turn's card", || d.find("POST", "travaille")).await;
    assert!(card.path.starts_with(&format!("/webhooks/{hook}/")));
    assert!(card.path.contains("with_components=true"));
    assert_eq!(card.body["username"], "Leo");
    bridge(
        json!({ "kind": "tool", "phase": "start", "toolUseId": "t1", "tool": "Bash",
        "input": { "command": "npm test" } }),
    );
    eventually("the card edited with the call", || {
        d.find("PATCH", "Bash npm test")
    })
    .await;
    bridge(
        json!({ "kind": "turn", "phase": "end", "reason": "answer", "durationMs": 1000,
        "answer": "Le panier est prêt." }),
    );
    eventually("the turn folded", || d.find("PATCH", "1 action")).await;
    let answer = eventually("the answer", || d.find("POST", "Le panier est prêt.")).await;
    assert_eq!(answer.body["username"], "Leo");

    // A permission: a card that calls the owner, Allow answers it.
    bridge(
        json!({ "kind": "permission", "waiting": true, "tool": "Bash",
        "input": { "command": "rm -rf build" } }),
    );
    let asked = eventually("the permission's card", || d.find("POST", "p:leo:y")).await;
    assert!(asked.body.to_string().contains("<@u1>"));
    assert_eq!(asked.body["allowed_mentions"]["users"], json!(["u1"]));
    // A stranger's click is refused, to them alone.
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i0",
            "intruder",
            &channel,
            3,
            json!({ "custom_id": "p:leo:y" }),
            None,
        ),
    );
    let refused = eventually("the stranger refused", || {
        d.calls_to("POST", "/interactions/i0/i0tok/callback").pop()
    })
    .await;
    assert_eq!(refused.body["type"], 4);
    assert_eq!(refused.body["data"]["flags"], 64);
    let typed_before = rig.shells.typed_into("leo").len();
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i1",
            "u1",
            &channel,
            3,
            json!({ "custom_id": "p:leo:y" }),
            Some("1"),
        ),
    );
    let acked = eventually("the click acknowledged", || {
        d.calls_to("POST", "/interactions/i1/i1tok/callback").pop()
    })
    .await;
    assert_eq!(acked.body["type"], 6);
    eventually("the answer typed or handed to the mod", || {
        (rig.shells.typed_into("leo").len() > typed_before || d.find("PATCH", "Autorisé").is_some())
            .then_some(())
    })
    .await;

    // A worker started: its line, with the button that opens its thread.
    let added = rig
        .host
        .apply(ModelCmd::AddPane {
            ws: "shop".into(),
            agent: Some(AgentId::Claude),
            near: None,
            dir: None,
            cwd: None,
            name: Some("Ada".into()),
            init: Some(hangar_engine::state::PanePatch::new(|p: &mut Pane| {
                p.role = Some(PaneRole::Worker);
                p.task_id = Some("t1".into());
            })),
        })
        .unwrap();
    let ada = added.pane.unwrap();
    rig.host.orchestration().unwrap().post(
        "shop",
        OrchestratorEvent::WorkerStarted {
            worker: "Ada".into(),
            task_id: Some("t1".into()),
        },
    );
    let line = eventually("the worker's line", || d.find("POST", &format!("t:{ada}"))).await;
    assert!(line.body.to_string().contains("Ouvrir le fil"));
    let line_id = eventually("its id kept", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|layout| layout["workers"][&ada]["line"].as_str().map(str::to_string))
    })
    .await;
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i2",
            "u1",
            &channel,
            3,
            json!({ "custom_id": format!("t:{ada}") }),
            Some(&line_id),
        ),
    );
    let thread = eventually("the thread started on the line", || {
        d.calls_to(
            "POST",
            &format!("/channels/{channel}/messages/{line_id}/threads"),
        )
        .pop()
    })
    .await;
    assert_eq!(thread.body["auto_archive_duration"], 10080);
    assert!(thread.body["name"].as_str().unwrap().starts_with("Ada"));
    eventually("the line links to the thread", || {
        d.calls_to("PATCH", &format!("/channels/{channel}/messages/{line_id}"))
            .into_iter()
            .find(|c| c.body.to_string().contains("Voir le fil"))
    })
    .await;
    let thread_id = eventually("the thread kept", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|layout| {
                layout["workers"][&ada]["thread"]
                    .as_str()
                    .map(str::to_string)
            })
    })
    .await;
    // What the owner writes in the thread goes to the worker.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m5", "channel_id": thread_id,
        "content": "Pense aux tests", "author": { "id": "u1" } }),
    );
    eventually("Ada given the message", || {
        let queued = !d
            .calls_to(
                "PUT",
                &format!("/channels/{thread_id}/messages/m5/reactions/%E2%8F%B3"),
            )
            .is_empty();
        (queued || rig.shells.typed_into(&ada).contains("Pense aux tests")).then_some(())
    })
    .await;
    // The worker stops: a last line, the thread archived.
    rig.host.orchestration().unwrap().post(
        "shop",
        OrchestratorEvent::WorkerExited {
            worker: "Ada".into(),
            task_id: Some("t1".into()),
        },
    );
    eventually("the thread archived", || {
        d.calls_to("PATCH", &format!("/channels/{thread_id}"))
            .into_iter()
            .find(|c| c.body["archived"] == true)
    })
    .await;

    // A workspace made from Discord: its channel follows.
    let folder = rig._root.path().join("blog");
    std::fs::create_dir_all(&folder).unwrap();
    d.send("INTERACTION_CREATE", interaction("i3", "u1", &channel, 2, json!({
        "name": "workspace",
        "options": [{ "name": "folder", "value": folder.to_string_lossy() }, { "name": "name", "value": "Blog" }],
    }), None));
    eventually("the blog's channel", || {
        d.calls_to("POST", "/guilds/g1/channels")
            .into_iter()
            .find(|c| c.body["name"] == "blog")
    })
    .await;
    let said = eventually("the command answered", || d.find("PATCH", "Workspace créé")).await;
    assert!(
        said.path
            .starts_with("/webhooks/app1/i3tok/messages/@original")
    );
    assert!(
        rig.host
            .snapshot()
            .state
            .workspaces
            .iter()
            .any(|ws| ws.name == "Blog")
    );
}
