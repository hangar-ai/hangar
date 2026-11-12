//! The workspaces this node runs itself (docs/native/headless.md, step 3):
//! hangar-engine's host with no window, its state in
//! `~/.local/state/hangar-node/state.json`, its panes among the node's own
//! (panes.rs), so that a desktop streams them as it streams any pane of the
//! node.
//!
//! The node is their authority: their agents' control requests are answered
//! here (control.rs hands them over before anything goes to a desktop), and
//! they run with no desktop connected. The node API shows them and changes
//! them (`/node/own`, below), for a desktop, for `hangar-node workspace …`,
//! and for the Discord connector.
#![cfg(target_os = "linux")]

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hangar_core::pty::{PtyEvent, PtyManager};
use hangar_engine::host::screen::{Screen, Screens};
use hangar_engine::host::terminals::{COLUMNS, ROWS};
use hangar_engine::host::{EngineHost, Spawn, TermEvent, TermSink, Terminals};
use hangar_engine::state::StoreAction;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{error, Shared};
use crate::panes::{Observer, Panes};

/// The node's panes, as the engine's terminals: each with a screen the
/// engine reads, which answers what its program asks of its terminal.
pub struct NodeTerminals {
    panes: Arc<Panes>,
    pty: Arc<PtyManager>,
    screens: Screens,
}

impl NodeTerminals {
    pub fn new(panes: Arc<Panes>, pty: Arc<PtyManager>) -> NodeTerminals {
        NodeTerminals {
            panes,
            pty,
            screens: Screens::default(),
        }
    }
}

impl Terminals for NodeTerminals {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        let id = spawn.id.clone();
        let screen = self.screens.open(&id, COLUMNS, ROWS);
        let observer: Observer = {
            let (pty, id) = (self.pty.clone(), id.clone());
            Arc::new(move |event: &PtyEvent| match event {
                PtyEvent::Output(output) => {
                    let answers = screen.feed(output.data.as_bytes());
                    if !answers.is_empty() {
                        let (pty, id) = (pty.clone(), id.clone());
                        std::thread::spawn(move || {
                            for answer in answers {
                                let _ = hangar_core::pty::pty_write(&pty, id.clone(), answer);
                            }
                        });
                    }
                    sink(TermEvent::Output(id.clone()));
                }
                PtyEvent::Exit(_) => sink(TermEvent::Exit(id.clone())),
            })
        };
        let spawned = self.panes.spawn(crate::panes::Spawn {
            id: spawn.id,
            cwd: spawn.cwd,
            shell: None,
            cols: COLUMNS,
            rows: ROWS,
            name: Some(spawn.name),
            agent: Some(spawn.agent),
            bridge: Some(spawn.bridge),
            port_base: spawn.port_base,
            build_env: spawn.build_env,
            mcp_config: spawn.mcp_config,
            kits: None,
            claude_account: spawn.claude_account,
            observer: Some(observer),
        });
        match spawned {
            Ok(_) => Ok(()),
            Err(err) => {
                self.screens.close(&id);
                Err(err)
            }
        }
    }

    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        self.panes.write(pane, data.to_string())
    }

    fn alive(&self, pane: &str) -> bool {
        self.panes.alive(pane)
    }

    fn kill(&self, pane: &str) -> Result<(), String> {
        self.panes.kill(pane);
        self.screens.close(pane);
        Ok(())
    }

    fn lines(&self, pane: &str, n: usize) -> Option<Vec<String>> {
        self.screens.read(pane, |screen| screen.lines(n))
    }

    fn screen(&self, pane: &str) -> Option<Vec<String>> {
        self.screens.read(pane, Screen::rows)
    }

    fn bracketed_paste(&self, pane: &str) -> bool {
        self.screens
            .read(pane, Screen::bracketed_paste)
            .unwrap_or(false)
    }

    fn application_cursor(&self, pane: &str) -> bool {
        self.screens
            .read(pane, Screen::application_cursor)
            .unwrap_or(false)
    }

    fn kill_all(&self) {
        self.panes.kill_all();
        self.screens.clear();
    }
}

/// Whether the node answers the control request `args` (an agent's when
/// `internal` is false, else Hangar's own, a pane's briefing) itself, not a
/// desktop: it is about one of the node's own panes; or it comes from no
/// pane at all (an agent started by hand on the node, or in a pane with no
/// token of its own), no desktop drives the node (`desktops` is 0) and the
/// node has workspaces of its own to answer about.
pub fn owns(own: &EngineHost, args: &Value, internal: bool, desktops: usize) -> bool {
    let pane = if internal {
        args.get("paneId")
            .and_then(Value::as_str)
            .map(str::to_string)
    } else {
        hangar_engine::control::types::Caller::of(args).pane_id
    };
    match pane {
        Some(pane) => own.has_pane(&pane),
        None => !internal && desktops == 0 && !own.snapshot().state.workspaces.is_empty(),
    }
}

/// The node's own workspaces, as `GET /node/own` lists them.
pub fn listing(own: &EngineHost) -> Value {
    let snapshot = own.snapshot();
    let workspaces: Vec<Value> = snapshot
        .state
        .workspaces
        .iter()
        .map(|ws| {
            let panes: Vec<Value> = ws
                .panes
                .iter()
                .map(|pane| {
                    let activity = snapshot.activity.get(&pane.id).map(|a| a.activity);
                    json!({
                        "id": pane.id,
                        "name": pane.name,
                        "agent": pane.agent,
                        "role": pane.role,
                        "taskId": pane.task_id,
                        "activity": activity,
                    })
                })
                .collect();
            json!({
                "id": ws.id,
                "name": ws.name,
                "cwd": ws.cwd,
                "panes": panes,
            })
        })
        .collect();
    json!({ "workspaces": workspaces })
}

/// Said when the node runs no workspace of its own.
fn unavailable() -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "this node runs no workspace of its own",
    )
}

pub async fn list(State(node): State<Shared>) -> Response {
    match node.own.as_ref() {
        Some(own) => Json(listing(own)).into_response(),
        None => unavailable(),
    }
}

#[derive(Deserialize)]
pub struct AddBody {
    name: String,
    cwd: String,
    /// Its first pane is the workspace's orchestrator (default true): a
    /// workspace on a server is driven by talking to its orchestrator.
    #[serde(default = "yes")]
    orchestrator: bool,
}

fn yes() -> bool {
    true
}

/// A workspace of the node's own, in folder `cwd`, with one Claude Code
/// pane, its orchestrator unless asked otherwise.
pub fn add(own: &EngineHost, name: &str, cwd: &str, orchestrator: bool) -> Result<String, String> {
    own.add_workspace(name, cwd, orchestrator)
}

pub async fn create(State(node): State<Shared>, Json(body): Json<AddBody>) -> Response {
    let Some(own) = node.own.as_ref() else {
        return unavailable();
    };
    match add(own, &body.name, &body.cwd, body.orchestrator) {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(err) => error(StatusCode::BAD_REQUEST, &err),
    }
}

pub async fn remove(State(node): State<Shared>, Path(id): Path<String>) -> Response {
    let Some(own) = node.own.as_ref() else {
        return unavailable();
    };
    if !own.snapshot().state.workspaces.iter().any(|ws| ws.id == id) {
        return error(StatusCode::NOT_FOUND, "no such workspace on this node");
    }
    own.edit(StoreAction::RemoveWorkspace(id));
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct SendBody {
    text: String,
}

/// What the person says to a pane of the node's own: queued for its
/// session, as the chat view's send is.
pub async fn send(
    State(node): State<Shared>,
    Path(pane): Path<String>,
    Json(body): Json<SendBody>,
) -> Response {
    let Some(own) = node.own.as_ref() else {
        return unavailable();
    };
    if !own.has_pane(&pane) {
        return error(StatusCode::NOT_FOUND, "no such pane among the node's own");
    }
    if body.text.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "nothing to send");
    }
    hangar_core::bridge::bridge_enqueue(own.core(), pane, body.text, None);
    Json(json!({ "ok": true })).into_response()
}
