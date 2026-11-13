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
//!
//! A desktop mirrors them (hangar-engine's `state::hosted`): it reads them
//! whole (`/node/own/state`), sends what the person changes on its copy as
//! ops (`/node/own/workspaces/{id}/ops`), and hands over a workspace it ran
//! on this node, or has just made for it (`/node/own/adopt`).
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
use hangar_engine::state::hosted::HostedOp;
use hangar_engine::state::{StoreAction, Workspace};
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

    fn resize(&self, pane: &str, columns: u16, rows: u16) -> Result<(), String> {
        self.panes.resize(pane, columns, rows)?;
        self.screens.resize(pane, columns, rows);
        Ok(())
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

/// The node's own workspaces as a desktop mirrors them, for `GET
/// /node/own/state`: each whole, as the state holds it, with the revision
/// of that state and the daemon's `boot` (the revisions start again with
/// the daemon). What each pane's agent is doing is beside them, by pane,
/// for those that said: it is not part of the state, and inside a pane it
/// would be read as one of its fields and saved with it. `origins` is the
/// same for the repositories: the address of each workspace's `origin`, by
/// workspace, for those that have one.
pub fn full(own: &EngineHost, boot: &str, origins: &serde_json::Map<String, Value>) -> Value {
    let (revision, state) = own.revised_state();
    let snapshot = own.snapshot();
    let activity: serde_json::Map<String, Value> = state
        .workspaces
        .iter()
        .flat_map(|ws| &ws.panes)
        .filter_map(|pane| {
            let live = snapshot.activity.get(&pane.id)?;
            Some((pane.id.clone(), json!(live.activity)))
        })
        .collect();
    // The panes whose agent reports through its mod: a mirror then leaves
    // its own guesses from their output aside.
    let bridged: Vec<&str> = state
        .workspaces
        .iter()
        .flat_map(|ws| &ws.panes)
        .filter(|pane| {
            snapshot
                .activity
                .get(&pane.id)
                .is_some_and(|live| live.bridged)
        })
        .map(|pane| pane.id.as_str())
        .collect();
    json!({
        "boot": boot,
        "revision": revision,
        "workspaces": state.workspaces,
        "activity": activity,
        "bridged": bridged,
        "origins": origins,
    })
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

pub async fn state(State(node): State<Shared>) -> Response {
    match node.own.as_ref() {
        Some(own) => {
            // Read first, and each off the runtime: git is asked once for
            // each folder. A workspace added meanwhile is in the next read.
            let folders: Vec<(String, String)> = own
                .snapshot()
                .state
                .workspaces
                .iter()
                .map(|ws| (ws.id.clone(), ws.cwd.clone()))
                .collect();
            let mut origins = serde_json::Map::new();
            for (id, cwd) in folders {
                if let Some(origin) = crate::engine::origin_of(cwd).await {
                    origins.insert(id, Value::String(origin));
                }
            }
            Json(full(own, &node.boot, &origins)).into_response()
        }
        None => unavailable(),
    }
}

/// A workspace a desktop hands to the node, whole: one it ran here, its
/// panes' shells started by that desktop, or one it has just made. Refused
/// before anything is touched when the node cannot run it. The shells that
/// run under its pane ids are then taken down, since the node's own must
/// start under those ids (core makes a start wait for the shell of the same
/// id to be gone), and each Claude Code comes back on its session. Handed
/// over again, a workspace the node already has is left as it is, its
/// shells too.
pub async fn adopt(State(node): State<Shared>, Json(body): Json<Value>) -> Response {
    if node.own.is_none() {
        return unavailable();
    }
    let workspace: Workspace = match serde_json::from_value(body) {
        Ok(workspace) => workspace,
        Err(err) => return error(StatusCode::BAD_REQUEST, &format!("not a workspace: {err}")),
    };
    // A task of its own: a desktop that hangs up while it waits (its link
    // dropped, it gave up, it quit) ends this handler where it stands, and
    // an adoption stopped after its shells were taken down and before the
    // workspace was the node's would leave neither side running it.
    match tokio::spawn(take_over(node, workspace)).await {
        Ok(Ok(id)) => Json(json!({ "id": id })).into_response(),
        Ok(Err(err)) => error(StatusCode::BAD_REQUEST, &err),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    }
}

/// One adoption, start to end, alone on the node ([`Node::adopting`]): two
/// of the same workspace at once (a desktop asking again after no answer)
/// would have the second take down the shells the first had just started.
/// The workspace is saved before the desktop hears yes: from then on the
/// desktop keeps no copy to run, and a node stopped a moment later must
/// find the workspace in its state when it starts again.
async fn take_over(node: Shared, workspace: Workspace) -> Result<String, String> {
    let Some(own) = node.own.as_ref() else {
        return Err("this node runs no workspace of its own".into());
    };
    let _alone = node.adopting.clone().write_owned().await;
    if !own.adoptable(&workspace)? {
        return Ok(workspace.id);
    }
    // A shell of the node's own is never one to take down, whatever the
    // check above let through.
    let ids: Vec<String> = workspace
        .panes
        .iter()
        .map(|pane| pane.id.clone())
        .filter(|id| !own.has_pane(id))
        .collect();
    let panes = node.panes.clone();
    tokio::task::spawn_blocking(move || {
        for id in ids {
            panes.kill(&id);
        }
    })
    .await
    .map_err(|err| err.to_string())?;
    let id = own.adopt(workspace)?;
    saved(own).await;
    Ok(id)
}

/// The state written now rather than a moment later (the host saves by
/// itself after each change): what a desktop is about to be told was done
/// is on the disk first. A save that fails is the host's to try again.
async fn saved(own: &EngineHost) {
    if let Err(err) = own.save().await {
        eprintln!("the state could not be saved: {err}");
    }
}

#[derive(Deserialize)]
pub struct OpsBody {
    ops: Vec<HostedOp>,
}

/// What the person changed on a desktop's mirror of workspace `id`,
/// applied in order: all of it, or none when one op cannot be. Answers the
/// revision the state stands at afterwards.
pub async fn ops(
    State(node): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(own) = node.own.as_ref() else {
        return unavailable();
    };
    let body: OpsBody = match serde_json::from_value(body) {
        Ok(body) => body,
        Err(err) => return error(StatusCode::BAD_REQUEST, &format!("not ops: {err}")),
    };
    if !own.snapshot().state.workspaces.iter().any(|ws| ws.id == id) {
        return error(StatusCode::NOT_FOUND, "no such workspace on this node");
    }
    match own.apply_ops(&id, body.ops) {
        Ok(()) => {
            saved(own).await;
            Json(json!({ "revision": own.revision() })).into_response()
        }
        Err(err) => error(StatusCode::BAD_REQUEST, &err),
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

#[derive(Deserialize)]
pub struct DeskBody {
    op: String,
    #[serde(default)]
    args: Value,
}

/// What the person does about a delivery of one of the node's own
/// workspaces from a desktop that mirrors it, or from a phone paired with
/// that desktop, by the names the desk is called with: what waits and what
/// it changes, land it, send it back to its worker.
const DESK_OPS: [&str; 4] = hangar_engine::mobile::delivery::OPS;
/// Every worker of a workspace of the node's own stopped: `{workspace}`,
/// answered `{stopped}`.
const STOP_WORKERS: &str = "stop_workers";

/// One thing the person asked of a workspace the node runs, from a desktop
/// that mirrors it: run here, by the node's own engines, since the desktop
/// has none for it. Only what is listed above is taken; the desk's other
/// calls stay the phone's and the connector's.
pub async fn desk(State(node): State<Shared>, Json(body): Json<DeskBody>) -> Response {
    let Some(own) = node.own.as_ref() else {
        return unavailable();
    };
    let done = if body.op == STOP_WORKERS {
        stop_workers(own, &body.args).await
    } else if DESK_OPS.contains(&body.op.as_str()) {
        match own.desk() {
            Some(desk) => {
                hangar_engine::mobile::engine::run(&body.op, &body.args, &own.port(), &desk).await
            }
            None => Err("the node is still starting".into()),
        }
    } else {
        Err(format!("{:?} is not asked of a node", body.op))
    };
    match done {
        Ok(answer) => Json(answer).into_response(),
        Err(err) => error(StatusCode::BAD_REQUEST, &err),
    }
}

async fn stop_workers(own: &EngineHost, args: &Value) -> Result<Value, String> {
    let ws = args
        .get("workspace")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !own.snapshot().state.workspaces.iter().any(|w| w.id == ws) {
        return Err("no such workspace on this node".into());
    }
    let Some(runtime) = own.orchestration() else {
        return Err("the node is still starting".into());
    };
    let stopped = runtime.stop_all_workers(ws).await;
    Ok(json!({ "stopped": stopped }))
}
