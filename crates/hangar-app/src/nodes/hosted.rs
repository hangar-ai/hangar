//! The workspaces a node runs itself, mirrored here (docs/native/nodes.md):
//! each is a workspace of this Hangar's state like any other, marked
//! `hosted`, which the navigator lists and a window opens, while its node
//! stays the one that holds it and starts its panes.
//!
//! The mirrors follow their node: its state is read (`GET /node/own/state`)
//! when the node connects, when it says its state changed (`own:changed`)
//! and when events were lost, and taken in with `StoreAction::SyncHosted`.
//! An answer older than what is held changes nothing. While a node is away
//! its mirrors stay as last seen, which is what the saved state holds too;
//! a node removed from the settings takes them with it.

use std::collections::{HashMap, HashSet};

use iced::Task;
use serde_json::Value;

use super::{Msg, client, wire};
use crate::message::Message;
use crate::state::live::AgentActivity;
use crate::state::types::{AppState, PaneId, Workspace};
use crate::state::{HostedEdit, Model, StoreAction};

/// What `GET /node/own/state` answers.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnState {
    /// The daemon's run: its revisions start again with it.
    pub boot: String,
    pub revision: u64,
    pub workspaces: Vec<Workspace>,
    /// Where each pane's agent stands, for those that said.
    pub activity: HashMap<PaneId, AgentActivity>,
    /// The panes whose agent the node hears through its mod.
    pub bridged: HashSet<PaneId>,
}

/// The state of a node last taken in: what a later answer is compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub boot: String,
    pub revision: u64,
}

/// The answer read, or `None` when it is not one (an older node, which has
/// no such route). A workspace that cannot be read fails the whole answer:
/// leaving it out would read as the node no longer running it.
pub fn parse(value: &Value) -> Option<OwnState> {
    let workspaces = serde_json::from_value(value.get("workspaces")?.clone()).ok()?;
    let activity = value
        .get("activity")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(pane, activity)| {
            Some((pane.clone(), serde_json::from_value(activity.clone()).ok()?))
        })
        .collect();
    let bridged = value
        .get("bridged")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pane| pane.as_str().map(str::to_owned))
        .collect();
    Some(OwnState {
        boot: value.get("boot")?.as_str()?.to_owned(),
        revision: value.get("revision")?.as_u64()?,
        workspaces,
        activity,
        bridged,
    })
}

/// Node `id` asked for its own state, when it is connected.
pub fn fetch(m: &Model, id: &str) -> Task<Message> {
    let Some(client) = client(m, id) else {
        return Task::none();
    };
    let id = id.to_owned();
    Task::perform(
        async move {
            let answer = wire::request(&client, "GET", "/node/own/state", None).await;
            answer
                .ok()
                .and_then(|answer| answer.json().ok())
                .and_then(|value| parse(&value))
        },
        move |state| {
            Message::Nodes(Msg::OwnState {
                id: id.clone(),
                state: state.map(Box::new),
            })
        },
    )
}

/// The panes of the mirrors of node `node`.
fn mirrored_panes(state: &AppState, node: &str) -> HashSet<PaneId> {
    state
        .workspaces
        .iter()
        .filter(|ws| ws.is_hosted() && ws.node.as_deref() == Some(node))
        .flat_map(|ws| ws.panes.iter().map(|pane| pane.id.clone()))
        .collect()
}

/// How many workspaces node `node` runs itself, as mirrored here.
pub fn count(m: &Model, node: &str) -> usize {
    m.state()
        .workspaces
        .iter()
        .filter(|ws| ws.is_hosted() && ws.node.as_deref() == Some(node))
        .count()
}

/// What node `node` answered, taken in: its mirrors made to say what it
/// says, unless what is held is newer (an answer that crossed a later one
/// on the way), and each of their panes given where its agent stands. An
/// answer of the revision held changes no mirror but still says where the
/// agents stand, which is what is read again after events were lost.
/// Nothing for a node that left the settings meanwhile.
pub fn take_in(m: &mut Model, node: &str, own: OwnState, now: u64) {
    if !m.settings().nodes.iter().any(|known| known.id == node) {
        return;
    }
    let held = m.live.nodes.held.get(node);
    let same_run = held.is_some_and(|held| held.boot == own.boot);
    if same_run && held.is_some_and(|held| own.revision < held.revision) {
        return;
    }
    let before = mirrored_panes(m.state(), node);
    if !(same_run && held.is_some_and(|held| own.revision == held.revision)) {
        m.apply(StoreAction::SyncHosted {
            node: node.to_owned(),
            workspaces: own.workspaces,
            keep: Vec::new(),
        });
        m.live.nodes.held.insert(
            node.to_owned(),
            Held {
                boot: own.boot,
                revision: own.revision,
            },
        );
    }
    crate::panes_logic::mirror_hosted(m, node, &before, &own.activity, &own.bridged, now);
}

/// Node `node` is no longer one of this Hangar's machines: its mirrors go,
/// and what was known of their panes with them. The node runs them still.
pub fn forget(m: &mut Model, node: &str, now: u64) {
    let before = mirrored_panes(m.state(), node);
    m.apply(StoreAction::SyncHosted {
        node: node.to_owned(),
        workspaces: Vec::new(),
        keep: Vec::new(),
    });
    m.live.nodes.held.remove(node);
    crate::panes_logic::mirror_hosted(m, node, &before, &HashMap::new(), &HashSet::new(), now);
}

/// What the notice says when a change to a workspace its node runs was
/// refused.
pub fn refusal_words(m: &Model, edit: &HostedEdit) -> String {
    let name = m
        .workspace(&edit.workspace)
        .map(|ws| ws.name.clone())
        .unwrap_or_default();
    let node = super::label(m, &edit.node);
    hangar_i18n::Translator::new(m.locale())
        .t_with(
            hangar_i18n::Key::NodesHostedRefused,
            &hangar_i18n::args!("name" => name.as_str(), "node" => node.as_str()),
        )
        .into_owned()
}
