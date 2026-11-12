//! Nodes (experiment `nodes`, docs/native/nodes.md §4): Linux machines the
//! desktop reaches over SSH, where it installs, starts and keeps a tunnel to
//! `hangar-node`.
//!
//! The machines are `settings.nodes` (how to reach each). Their state is
//! this package's: one connection per node runs on tokio (`connect.rs`) and
//! reports through the inbox; what it reported is `Model.live.nodes`, which
//! the Settings › Nodes page draws and later packages read with [`client`].
//! A node is connected for as long as it is in the settings, switched on,
//! and the experiment is on: from the app's start, and again whenever its
//! link drops (`connect.rs`). Switching it off, removing it, turning the
//! experiment off or quitting ends its connection and kills its tunnel.
//! How a machine and its link are shown, everywhere, is `presence.rs`.

pub mod away;
pub mod commands;
pub mod connect;
pub mod engine;
pub mod hosted;
pub mod model;
mod page;
pub mod presence;
pub mod remote;
pub mod runner;
pub mod wire;

#[cfg(test)]
mod hosted_tests;
#[cfg(test)]
mod orchestrate_tests;
#[cfg(test)]
mod page_tests;
#[cfg(test)]
mod remote_tests;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use iced::{Subscription, Task};

use crate::message::{Intent, Message};
use crate::services::{CoreEvent, Services};
use crate::settings::experiments;
use crate::state::types::{MachineKind, NodeConfig, Workspace};
use crate::state::{Model, Patch, Settings, StoreAction};

pub use model::{NodeClient, NodeStatus};
pub use page::{failure_words, settings_page};

use connect::{Setup, Timing, TunnelSlot, locked};
use model::{BinarySource, NODE_VERSION, target_problem};
use runner::{System, SystemTunnel, Tunnel};

/// The running connections, and what the page has open.
#[derive(Debug, Default)]
pub struct State {
    conns: HashMap<String, Conn>,
    /// Bumped for each connection started, so that what an old one still
    /// had in flight is told from the new one's.
    generation: u64,
    pub(crate) form: AddForm,
    pub(crate) open: Option<Open>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AddForm {
    pub label: String,
    pub target: String,
    pub config: String,
    pub kind: MachineKind,
    /// Sent once: its problem shows from then on.
    pub touched: bool,
}

/// What the page has open, one thing at a time: a menu under a row's
/// button, a name being typed, a removal waiting for its yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    /// The kind picker of this node.
    Kind(String),
    /// The kind picker of the add form.
    AddKind,
    /// The « … » menu of this node.
    More(String),
    Rename {
        id: String,
        draft: String,
    },
    Remove(String),
}

/// One running connection.
struct Conn {
    generation: u64,
    task: tokio::task::JoinHandle<()>,
    tunnel: TunnelSlot<SystemTunnel>,
    /// Cuts short the wait before the connection's next attempt.
    wake: Arc<tokio::sync::Notify>,
    /// The node's events followed (`/node/events`), once it connected.
    events: Option<tokio::task::JoinHandle<()>>,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Conn {
    /// The task stopped and its tunnel killed now, not when the task is
    /// next polled.
    fn stop(self) {
        self.task.abort();
        if let Some(events) = &self.events {
            events.abort();
        }
        if let Some(mut tunnel) = locked(&self.tunnel).take() {
            tunnel.kill();
        }
    }
}

/// What other packages read: each node's status, by id. A node with no entry
/// is disconnected.
#[derive(Debug, Default)]
pub struct Live {
    pub statuses: HashMap<String, NodeStatus>,
    /// What each link went through since it was last up, by node id.
    pub links: HashMap<String, presence::Link>,
    /// The state of each node's own workspaces last taken in, by node id:
    /// what their mirrors say (`hosted::take_in`).
    pub held: HashMap<String, hosted::Held>,
    /// A change to a workspace a node runs itself that was just refused
    /// (`Model::apply`), until the shell has said so.
    pub refused: Option<crate::state::HostedEdit>,
}

impl Live {
    pub fn status(&self, id: &str) -> NodeStatus {
        self.statuses.get(id).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// What a connection reported; dropped unless `generation` is the
    /// node's current one.
    Status {
        id: String,
        generation: u64,
        status: NodeStatus,
    },
    /// The node's next attempt starts now instead of at the end of its
    /// wait.
    Retry(String),
    /// The same for every node: the window got the focus back, which is
    /// when a laptop woke or the person came to look.
    Wake,
    /// Switched on or off by the person.
    SetEnabled(String, bool),
    SetKind(String, MachineKind),
    Remove(String),
    /// A menu, a rename or a removal's question opened on the page, or
    /// whatever was open closed.
    Show(Option<Open>),
    RenameInput(String),
    RenameSubmit,
    AddLabel(String),
    AddTarget(String),
    AddConfig(String),
    AddKind(MachineKind),
    AddSubmit,
    /// The connections brought in line with the settings: one for each
    /// node that is switched on, none for the others (as the app starts,
    /// then every second while there is a node to keep connected).
    Reconcile,
    /// Node `id` is asked for the state of the workspaces it runs itself:
    /// it connected, it said the state changed, or events were lost.
    SyncOwn(String),
    /// What node `id` answered; `None`: it could not say (an older node).
    OwnState {
        id: String,
        state: Option<Box<hosted::OwnState>>,
    },
}

/// The experiment is on.
pub fn available(m: &Model) -> bool {
    experiments::enabled(m.settings(), experiments::NODES)
}

/// How to talk to node `node_id`, while it is connected: its API through the
/// tunnel and the token it asks for.
pub fn client(m: &Model, node_id: &str) -> Option<NodeClient> {
    match m.live.nodes.statuses.get(node_id)? {
        NodeStatus::Connected {
            local_port, token, ..
        } => Some(NodeClient {
            base_url: format!("http://127.0.0.1:{local_port}"),
            token: token.clone(),
        }),
        _ => None,
    }
}

/// The label node `node_id` goes by: its name, else its SSH target, else
/// its id (a node removed from the settings that a workspace still names).
pub fn label(m: &Model, node_id: &str) -> String {
    m.settings()
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .map(|node| {
            if node.label.trim().is_empty() {
                node.ssh_target.clone()
            } else {
                node.label.clone()
            }
        })
        .unwrap_or_else(|| node_id.to_owned())
}

/// The nodes connected now, by id and label: where a workspace can be made.
pub fn connected(m: &Model) -> Vec<(String, String)> {
    if !available(m) {
        return Vec::new();
    }
    m.settings()
        .nodes
        .iter()
        .filter(|node| client(m, &node.id).is_some())
        .map(|node| (node.id.clone(), label(m, &node.id)))
        .collect()
}

/// Whether the lists show workspace `ws`: all but those of a node the
/// person switched off, which are out of the navigator and the palette
/// until it is switched on again. Nothing is hidden without the
/// experiment, which is also what would bring them back.
pub fn shown(m: &Model, ws: &Workspace) -> bool {
    let Some(node) = ws.node.as_deref() else {
        return true;
    };
    !available(m) || presence::config(m, node).is_none_or(NodeConfig::is_on)
}

/// Every tunnel killed: the app is quitting.
pub fn shutdown(st: &mut State) {
    for (id, conn) in st.conns.drain() {
        conn.stop();
        remote::publish(&id, None);
    }
}

/// A status a connection reported, kept when it is the current one's. The
/// node's client goes to the directory the remote panes reach it through.
/// `true` when its events are to be followed now: it connected, and they
/// are not yet.
fn apply_status(
    st: &mut State,
    live: &mut Live,
    id: String,
    generation: u64,
    status: NodeStatus,
    now: Instant,
) -> bool {
    let Some(conn) = st
        .conns
        .get_mut(&id)
        .filter(|conn| conn.generation == generation)
    else {
        return false;
    };
    let client = match &status {
        NodeStatus::Connected {
            local_port, token, ..
        } => Some(NodeClient {
            base_url: format!("http://127.0.0.1:{local_port}"),
            token: token.clone(),
        }),
        _ => None,
    };
    let follow = client.is_some() && conn.events.is_none();
    remote::publish(&id, client);
    let before = live.status(&id);
    live.links
        .entry(id.clone())
        .or_default()
        .note(&before, &status, now);
    live.statuses.insert(id, status);
    follow
}

/// What the node says, followed as the desktop that drives it
/// (`/node/events?control=1`): its control requests, run here against its
/// workspaces and answered there (`control::Msg::Node`); its panes' bridge
/// reports, as a local pane's (their activity, the orchestrator's news);
/// the queue of each; its boards changed; the state of the workspaces it
/// runs itself changed (or events lost: everything is read again).
async fn follow_events(sv: Services, node: String) {
    let id = node.clone();
    let emit = move |event: serde_json::Value| {
        for message in event_messages(&id, &sv.snapshot().state, event) {
            sv.send(message);
        }
    };
    remote::follow_events_at(
        node,
        "/node/events?control=1",
        emit,
        remote::Backoff::default(),
        remote::directory,
    )
    .await;
}

/// What one event of node `node` becomes here.
pub(crate) fn event_messages(
    node: &str,
    state: &crate::state::types::AppState,
    event: serde_json::Value,
) -> Vec<Message> {
    use serde_json::Value;
    let text = |key: &str| event.get(key).and_then(Value::as_str).map(str::to_owned);
    if event.get("lagged").is_some() {
        return vec![
            Message::Board(crate::board::Msg::Reload),
            Message::Nodes(Msg::SyncOwn(node.to_owned())),
        ];
    }
    match text("event").as_deref() {
        Some(name @ ("control:request" | "control:internal")) => {
            let payload = event.get("payload");
            let id = text("id").or_else(|| {
                payload
                    .and_then(|payload| payload.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            let op = payload
                .and_then(|payload| payload.get("op"))
                .and_then(Value::as_str);
            let (Some(id), Some(op)) = (id, op) else {
                return Vec::new();
            };
            let args = payload
                .and_then(|payload| payload.get("args"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let channel = if name == "control:request" {
                crate::services::Channel::Agent
            } else {
                crate::services::Channel::Internal
            };
            vec![Message::Control(crate::control::Msg::Node(
                crate::control::NodeRequest {
                    node: node.to_owned(),
                    id,
                    op: op.to_owned(),
                    args,
                    channel,
                },
            ))]
        }
        // Only a pane of this node's: a node speaks for its own panes.
        Some("bridge:event") => match (text("paneId"), event.get("payload")) {
            (Some(pane), Some(payload))
                if engine::node_of_pane(state, &pane).as_deref() == Some(node) =>
            {
                vec![Message::Core(CoreEvent::Bridge {
                    pane,
                    event: payload.clone(),
                })]
            }
            _ => Vec::new(),
        },
        // What waits for a pane of this node's, as its own core holds it.
        Some("bridge:queue") => {
            let payload = event.get("payload");
            let pane = payload
                .and_then(|payload| payload.get("paneId"))
                .and_then(Value::as_str);
            match pane {
                Some(pane) if engine::node_of_pane(state, pane).as_deref() == Some(node) => {
                    vec![Message::Core(CoreEvent::BridgeQueue {
                        pane: pane.to_owned(),
                        pending: queued(payload.and_then(|payload| payload.get("pending"))),
                    })]
                }
                _ => Vec::new(),
            }
        }
        Some("board:changed") => vec![Message::Core(CoreEvent::BoardChanged {
            cwd: text("cwd").unwrap_or_default(),
        })],
        Some("own:changed") => vec![Message::Nodes(Msg::SyncOwn(node.to_owned()))],
        _ => Vec::new(),
    }
}

/// The prompts of a `bridge:queue` event, in their order; one that cannot
/// be read is left out.
fn queued(pending: Option<&serde_json::Value>) -> Vec<crate::state::live::QueuedPrompt> {
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    pending
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(crate::state::live::QueuedPrompt {
                id: text(entry, "id")?,
                text: text(entry, "text")?,
                from: text(entry, "from"),
            })
        })
        .collect()
}

fn disconnect(st: &mut State, live: &mut Live, id: &str) {
    if let Some(conn) = st.conns.remove(id) {
        conn.stop();
    }
    remote::publish(id, None);
    live.statuses.remove(id);
    live.links.remove(id);
}

fn connect(st: &mut State, m: &mut Model, sv: &Services, id: &str) {
    let Some(node) = m.settings().nodes.iter().find(|n| n.id == id).cloned() else {
        return;
    };
    if !available(m) {
        return;
    }
    disconnect(st, &mut m.live.nodes, id);
    st.generation += 1;
    let generation = st.generation;
    let wake = Arc::new(tokio::sync::Notify::new());
    let setup = Setup {
        node,
        source: BinarySource::from_env(),
        version: NODE_VERSION.to_string(),
        timing: Timing::default(),
        control_dir: connect::control_dir(),
        wake: wake.clone(),
    };
    let tunnel: TunnelSlot<SystemTunnel> = Arc::new(Mutex::new(None));
    let sender = sv.clone();
    let node_id = id.to_string();
    let report = move |status: NodeStatus| {
        sender.send(Message::Nodes(Msg::Status {
            id: node_id.clone(),
            generation,
            status,
        }));
    };
    let task = tokio::spawn(connect::run(System, setup, tunnel.clone(), report));
    st.conns.insert(
        id.to_string(),
        Conn {
            generation,
            task,
            tunnel,
            wake,
            events: None,
        },
    );
    let live = &mut m.live.nodes;
    let status = NodeStatus::Connecting(model::Step::Probe);
    live.links.entry(id.to_string()).or_default().note(
        &NodeStatus::Disconnected,
        &status,
        Instant::now(),
    );
    live.statuses.insert(id.to_string(), status);
}

/// The node the add form describes, or `None` when its target does not do.
pub(crate) fn node_from_form(form: &AddForm, id: String) -> Option<NodeConfig> {
    if target_problem(&form.target).is_some() {
        return None;
    }
    let config = form.config.trim();
    Some(NodeConfig {
        id,
        label: form.label.trim().to_string(),
        ssh_target: form.target.trim().to_string(),
        ssh_config: (!config.is_empty()).then(|| config.to_string()),
        kind: form.kind,
        ..NodeConfig::default()
    })
}

/// One field of node `id` changed in the settings.
fn patch_node(m: &mut Model, id: String, change: impl Fn(&mut NodeConfig) + Send + Sync + 'static) {
    m.apply(StoreAction::UpdateSettings(Patch::new(
        move |s: &mut Settings| {
            if let Some(node) = s.nodes.iter_mut().find(|node| node.id == id) {
                change(node);
            }
        },
    )));
}

/// The main window moves on to a workspace the lists still show, when the
/// one it was on has just gone out of them.
fn leave_hidden_workspace(m: &mut Model) {
    if m.active_workspace().is_none_or(|ws| shown(m, ws)) {
        return;
    }
    let state = m.state();
    let next = crate::state::folders::sidebar_order(&state.workspaces, &state.folders)
        .into_iter()
        .find(|ws| shown(m, ws) && ws.detached != Some(true) && ws.hq != Some(true))
        .map(|ws| ws.id.clone());
    if let Some(next) = next {
        m.apply(StoreAction::SetActiveWorkspace(Some(next)));
    }
}

pub fn update(st: &mut State, m: &mut Model, sv: &Services, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Status {
            id,
            generation,
            status,
        } => {
            let now = Instant::now();
            let was = client(m, &id).is_some();
            if apply_status(st, &mut m.live.nodes, id.clone(), generation, status, now)
                && let Some(conn) = st.conns.get_mut(&id)
            {
                conn.events = Some(tokio::spawn(follow_events(sv.clone(), id.clone())));
            }
            // Just connected: what the node runs itself is read at once.
            if !was && client(m, &id).is_some() {
                return hosted::fetch(m, &id);
            }
        }
        Msg::Retry(id) => match st.conns.get(&id) {
            Some(conn) => conn.wake.notify_waiters(),
            None => reconcile(st, m, sv),
        },
        Msg::Wake => {
            for conn in st.conns.values() {
                conn.wake.notify_waiters();
            }
        }
        Msg::SetEnabled(id, on) => {
            patch_node(m, id, move |node| node.enabled = (!on).then_some(false));
            st.open = None;
            reconcile(st, m, sv);
            leave_hidden_workspace(m);
        }
        Msg::SetKind(id, kind) => {
            patch_node(m, id, move |node| node.kind = kind);
            st.open = None;
        }
        Msg::Remove(id) => {
            st.open = None;
            disconnect(st, &mut m.live.nodes, &id);
            // The mirrors of what it runs itself go with it: nothing reaches
            // them any more.
            hosted::forget(m, &id, crate::panes_logic::wall_clock_ms());
            m.apply(StoreAction::UpdateSettings(Patch::new(
                move |s: &mut Settings| s.nodes.retain(|node| node.id != id),
            )));
            leave_hidden_workspace(m);
        }
        Msg::Show(open) => st.open = open,
        Msg::RenameInput(value) => {
            if let Some(Open::Rename { draft, .. }) = &mut st.open {
                *draft = value;
            }
        }
        Msg::RenameSubmit => {
            if let Some(Open::Rename { id, draft }) = st.open.take() {
                let name = draft.trim().to_string();
                patch_node(m, id, move |node| node.label = name.clone());
            }
        }
        Msg::AddLabel(value) => st.form.label = value,
        Msg::AddTarget(value) => st.form.target = value,
        Msg::AddConfig(value) => st.form.config = value,
        Msg::AddKind(kind) => {
            st.form.kind = kind;
            st.open = None;
        }
        Msg::AddSubmit => {
            st.form.touched = true;
            if let Some(node) = node_from_form(&st.form, crate::state::store::new_id()) {
                m.apply(StoreAction::UpdateSettings(Patch::new(
                    move |s: &mut Settings| s.nodes.push(node.clone()),
                )));
                st.form = AddForm::default();
                // A machine is on from the moment it is added.
                reconcile(st, m, sv);
            }
        }
        Msg::Reconcile => reconcile(st, m, sv),
        Msg::SyncOwn(id) => return hosted::fetch(m, &id),
        Msg::OwnState { id, state } => {
            if let Some(state) = state {
                hosted::take_in(m, &id, *state, crate::panes_logic::wall_clock_ms());
                leave_hidden_workspace(m);
            }
        }
    }
    Task::none()
}

/// The nodes to keep connected: those of the settings that are switched
/// on, while the experiment is on and the state is the person's own (not
/// the empty one the app starts with).
pub(crate) fn wanted(m: &Model) -> Vec<String> {
    if !available(m) || !m.live.hydrated {
        return Vec::new();
    }
    m.settings()
        .nodes
        .iter()
        .filter(|node| node.is_on())
        .map(|node| node.id.clone())
        .collect()
}

/// The connections that should not run, and the nodes that should have one
/// and do not.
pub(crate) fn plan(running: &[String], wanted: &[String]) -> (Vec<String>, Vec<String>) {
    let stop = running
        .iter()
        .filter(|id| !wanted.contains(id))
        .cloned()
        .collect();
    let start = wanted
        .iter()
        .filter(|id| !running.contains(id))
        .cloned()
        .collect();
    (stop, start)
}

fn reconcile(st: &mut State, m: &mut Model, sv: &Services) {
    let running: Vec<String> = st.conns.keys().cloned().collect();
    let (stop, start) = plan(&running, &wanted(m));
    for id in stop {
        disconnect(st, &mut m.live.nodes, &id);
    }
    for id in start {
        connect(st, m, sv, &id);
    }
}

pub fn subscription(st: &State, m: &Model) -> Subscription<Message> {
    if st.conns.is_empty() && wanted(m).is_empty() {
        return Subscription::none();
    }
    Subscription::batch([
        // At once, then every second: the first connections do not wait for
        // the first beat. The beat also redraws what counts time, the grace
        // period of a link that dropped and the seconds to its next attempt.
        Subscription::run_with("hangar nodes start", |_| {
            iced::futures::stream::once(async { Message::Nodes(Msg::Reconcile) })
        }),
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Nodes(Msg::Reconcile)),
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Window(iced::window::Event::Focused) => Some(Message::Nodes(Msg::Wake)),
            _ => None,
        }),
    ])
}

pub fn on_intent(
    _st: &mut State,
    _m: &mut Model,
    _sv: &Services,
    _intent: &Intent,
) -> Option<Task<Message>> {
    None
}

pub fn on_event(
    _st: &mut State,
    _m: &mut Model,
    _sv: &Services,
    _event: &CoreEvent,
) -> Option<Task<Message>> {
    None
}
