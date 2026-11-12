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
//!
//! What the person changes on a mirror goes the other way. `Model::apply`
//! applies it to the mirror and leaves what the node has to be told in the
//! [`Outbox`]; after every message the shell sends what waits
//! ([`send_next`]), one request at a time for each node, so that a node
//! hears the changes in the order they were made. A workspace handed to a
//! node, new or one this computer drove until now ([`hand_over`]), goes
//! through the same line. Until a node has answered everything it was
//! sent, what it lists is older than the mirrors and is not taken in. A
//! change the node refuses, or that did not reach it, is undone by reading
//! the node again: the node wins, and the person is told.

use std::collections::{HashMap, HashSet, VecDeque};

use iced::Task;
use serde_json::Value;

use hangar_i18n::{Key, Translator, args};
use serde_json::json;

use super::model::NodeClient;
use super::{Msg, client, wire};
use crate::message::{Intent, Message};
use crate::shell::Notice;
use crate::state::live::AgentActivity;
use crate::state::types::{AppState, PaneId, PaneRole, Workspace, WsId};
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
    /// The address of each workspace's repository, for those that have
    /// one; `None` from a node that does not say (an older one).
    pub origins: Option<HashMap<WsId, String>>,
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
    let origins = value
        .get("origins")
        .and_then(Value::as_object)
        .map(|origins| {
            origins
                .iter()
                .filter_map(|(ws, origin)| Some((ws.clone(), origin.as_str()?.to_owned())))
                .collect()
        });
    Some(OwnState {
        origins,
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
    let asked = m.live.nodes.outbox.epoch(&id);
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
                asked,
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

/// How many workspaces on node `node` this computer still drives: they
/// stop when it does.
pub fn driven_here(m: &Model, node: &str) -> usize {
    m.state()
        .workspaces
        .iter()
        .filter(|ws| !ws.is_hosted() && ws.node.as_deref() == Some(node))
        .count()
}

/// What node `node` answered, taken in: its mirrors made to say what it
/// says, unless what is held is newer (an answer that crossed a later one
/// on the way), and each of their panes given where its agent stands. An
/// answer of the revision held changes no mirror but still says where the
/// agents stand, which is what is read again after events were lost.
/// Nothing for a node that left the settings meanwhile.
///
/// `asked` is where the node's line stood when the answer was asked for
/// ([`Outbox::epoch`]). An answer asked for before the node acknowledged
/// the last change sent to it may not hold that change: taking it in would
/// undo on the mirror what the person just did. It is dropped whole; the
/// node is read again once it has answered everything ([`sent`]).
pub fn take_in_asked(m: &mut Model, node: &str, asked: u64, own: OwnState, now: u64) {
    let outbox = &m.live.nodes.outbox;
    if outbox.busy(node) || outbox.epoch(node) != asked {
        return;
    }
    take_in(m, node, own, now);
}

/// [`take_in_asked`] for an answer known to be current.
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
    // Where each mirror's repository comes from, as its node reads it.
    if let Some(origins) = &own.origins {
        let state = m.state_arc();
        for ws in &state.workspaces {
            if ws.is_hosted() && ws.node.as_deref() == Some(node) {
                let origin = origins.get(&ws.id).map(String::as_str);
                m.live.nodes.repos.said(ws, origin);
            }
        }
    }
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
    m.live.nodes.outbox.lines.remove(node);
    crate::panes_logic::mirror_hosted(m, node, &before, &HashMap::new(), &HashSet::new(), now);
}

/// Why a change to a workspace a node runs itself was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Its node cannot be reached: nothing could tell it.
    Offline,
    /// Nobody at this computer asked for it: an agent of this machine did.
    Unattended,
    /// A workspace on one of its worktrees, which only its node could make.
    Worktree,
}

/// What the notice says when a change to a workspace its node runs was
/// refused.
pub fn refusal_words(m: &Model, edit: &HostedEdit, why: Why) -> String {
    let node = super::label(m, &edit.node);
    let key = match why {
        Why::Offline => Key::NodesHostedRefused,
        Why::Unattended => Key::NodesHostedUnattended,
        Why::Worktree => Key::NodesHostedWorktree,
    };
    Translator::new(m.locale())
        .t_with(
            key,
            &args!("name" => edit.name.as_str(), "node" => node.as_str()),
        )
        .into_owned()
}

/// How a workspace came to be handed to its node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handover {
    /// Just made here, for the node to run from the start.
    Created,
    /// Driven from this computer until now.
    Moved,
}

/// One thing a node is told.
#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    /// What the person changed on a mirror.
    Ops(HostedEdit),
    /// A workspace handed over whole (`POST /node/own/adopt`).
    Adopt {
        workspace: Box<Workspace>,
        handover: Handover,
    },
}

impl Job {
    fn name(&self) -> &str {
        match self {
            Job::Ops(edit) => &edit.name,
            Job::Adopt { workspace, .. } => &workspace.name,
        }
    }
}

/// What one node is told, in order.
#[derive(Debug, Default)]
pub struct Line {
    waiting: VecDeque<Job>,
    /// Sent and not answered yet: nothing else is sent meanwhile.
    sending: Option<Job>,
    /// Moves on with each thing added to the line and each answer: what
    /// was read from the node at another epoch is older than the mirrors.
    epoch: u64,
}

/// What the nodes are still to be told, by node id.
#[derive(Debug, Default)]
pub struct Outbox {
    lines: HashMap<String, Line>,
}

impl Outbox {
    fn push(&mut self, node: &str, job: Job) {
        let line = self.lines.entry(node.to_owned()).or_default();
        line.waiting.push_back(job);
        line.epoch += 1;
    }

    /// The person changed a mirror: its node is to be told.
    pub fn edited(&mut self, edit: HostedEdit) {
        let node = edit.node.clone();
        self.push(&node, Job::Ops(edit));
    }

    /// Something for node `node` waits or is on its way.
    pub fn busy(&self, node: &str) -> bool {
        self.lines
            .get(node)
            .is_some_and(|line| line.sending.is_some() || !line.waiting.is_empty())
    }

    /// Where the line of node `node` stands.
    pub fn epoch(&self, node: &str) -> u64 {
        self.lines.get(node).map_or(0, |line| line.epoch)
    }

    fn jobs(&self) -> impl Iterator<Item = &Job> {
        self.lines
            .values()
            .flat_map(|line| line.sending.iter().chain(&line.waiting))
    }

    /// Workspace `ws` was handed to its node, which has not said yes yet.
    /// Until it has, nothing of the workspace is followed on the node: the
    /// shells this computer started there are about to be replaced, and
    /// those of a new workspace do not exist.
    pub fn adopting(&self, ws: &str) -> bool {
        self.jobs()
            .any(|job| matches!(job, Job::Adopt { workspace, .. } if workspace.id == ws))
    }

    /// A workspace is being handed to a node. The state is not saved
    /// meanwhile: written as the node's and read back before the node took
    /// it, the workspace would be a mirror of nothing, and go at the first
    /// sync.
    pub fn handing_over(&self) -> bool {
        self.jobs().any(|job| matches!(job, Job::Adopt { .. }))
    }

    #[cfg(test)]
    pub fn sending(&self, node: &str) -> Option<Job> {
        self.lines.get(node).and_then(|line| line.sending.clone())
    }

    #[cfg(test)]
    pub fn waiting(&self, node: &str) -> Vec<Job> {
        self.lines
            .get(node)
            .map(|line| line.waiting.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Workspace `ws`, which lives on a node, becomes the node's own: flagged
/// here first, so that this computer's engines let go of it before the
/// node takes down the shells they started, then handed to the node in its
/// turn. `false` when it is not a workspace this computer drives on a node.
pub fn hand_over(m: &mut Model, ws: &str, handover: Handover) -> bool {
    let Some(found) = m.workspace(ws).filter(|found| !found.is_hosted()) else {
        return false;
    };
    let Some(node) = found.node.clone() else {
        return false;
    };
    let mut mirror = found.clone();
    mirror.hosted = Some(true);
    m.apply(StoreAction::PutWorkspace(Box::new(mirror.clone())));
    // What the node says of it next is taken whole, whatever was held.
    m.live.nodes.held.remove(&node);
    m.live.nodes.outbox.push(
        &node,
        Job::Adopt {
            workspace: Box::new(mirror),
            handover,
        },
    );
    true
}

/// Why workspace `ws` cannot be moved to its node now, in words; `None`
/// when it can. A delivery under way is run by this computer and would be
/// cut; a worker that has no session yet would start again without its
/// task.
pub fn move_refusal(m: &Model, ws: &Workspace) -> Option<String> {
    let node = ws.node.as_deref()?;
    let label = super::label(m, node);
    let key = if client(m, node).is_none() {
        Key::NodesMoveOffline
    } else if m.live.control.is_delivering(&ws.id) {
        Key::NodesMoveDelivering
    } else if ws
        .panes
        .iter()
        .any(|pane| pane.role == Some(PaneRole::Worker) && pane.session_id.is_none())
    {
        Key::NodesMoveStarting
    } else {
        return None;
    };
    Some(
        Translator::new(m.locale())
            .t_with(
                key,
                &args!("name" => ws.name.as_str(), "node" => label.as_str()),
            )
            .into_owned(),
    )
}

/// The agents of `ws` in the middle of a turn: a move interrupts them.
pub fn working(m: &Model, ws: &Workspace) -> Vec<String> {
    ws.panes
        .iter()
        .filter(|pane| {
            matches!(
                m.live.panes_logic.activity_or_idle(&pane.id),
                AgentActivity::Working | AgentActivity::Waiting
            )
        })
        .map(|pane| pane.name.clone())
        .collect()
}

/// `job` told to the node `client` reaches.
pub async fn send(client: &NodeClient, job: &Job) -> Result<(), String> {
    let answer = match job {
        Job::Ops(edit) => {
            let path = format!("/node/own/workspaces/{}/ops", wire::encode(&edit.workspace));
            let body = json!({ "ops": edit.ops });
            wire::request(client, "POST", &path, Some(&body)).await?
        }
        Job::Adopt { workspace, .. } => {
            let body = serde_json::to_value(workspace).map_err(|err| err.to_string())?;
            wire::request(client, "POST", "/node/own/adopt", Some(&body)).await?
        }
    };
    answer.json().map(|_| ())
}

/// The next thing each node is to be told, sent: one at a time for each
/// node, the one after it waiting for the answer ([`sent`]). Run after
/// every message, so a line never stands still while something waits.
pub fn send_next(m: &mut Model) -> Task<Message> {
    let ready: Vec<String> = m
        .live
        .nodes
        .outbox
        .lines
        .iter()
        .filter(|(_, line)| line.sending.is_none() && !line.waiting.is_empty())
        .map(|(node, _)| node.clone())
        .collect();
    let mut tasks = Vec::new();
    for node in ready {
        let client = client(m, &node);
        let away = || {
            let label = super::label(m, &node);
            Translator::new(m.locale())
                .t_with(Key::NodesPaneAway, &args!("node" => label.as_str()))
                .into_owned()
        };
        let away = client.is_none().then(away);
        let Some(line) = m.live.nodes.outbox.lines.get_mut(&node) else {
            continue;
        };
        let Some(job) = line.waiting.pop_front() else {
            continue;
        };
        line.sending = Some(job.clone());
        let answered = move |result| {
            Message::Nodes(Msg::Sent {
                id: node.clone(),
                result,
            })
        };
        tasks.push(match (client, away) {
            (Some(client), _) => Task::perform(async move { send(&client, &job).await }, answered),
            (None, away) => Task::done(answered(Err(away.unwrap_or_default()))),
        });
    }
    Task::batch(tasks)
}

/// What the notice says when node `node` did not take `job`, for `reason`.
pub fn failure_words(m: &Model, node: &str, job: &Job, reason: &str) -> String {
    let label = super::label(m, node);
    let key = match job {
        Job::Ops(_) => Key::NodesHostedNotDone,
        Job::Adopt {
            handover: Handover::Created,
            ..
        } => Key::NodesAdoptCreateFailed,
        Job::Adopt {
            handover: Handover::Moved,
            ..
        } => Key::NodesAdoptMoveFailed,
    };
    Translator::new(m.locale())
        .t_with(
            key,
            &args!(
                "name" => job.name(),
                "node" => label.as_str(),
                "reason" => reason.trim_end_matches('.'),
            ),
        )
        .into_owned()
}

/// A workspace handed to its node that the node did not take is this
/// computer's again: one it drove is driven from here as before, its
/// shells found where they were left or started again; one just made for
/// the node is taken back out.
fn take_back(m: &mut Model, ws: &str, handover: Handover) {
    let Some(mut local) = m.workspace(ws).cloned() else {
        return;
    };
    local.hosted = None;
    m.apply(StoreAction::PutWorkspace(Box::new(local)));
    if handover == Handover::Created {
        m.apply(StoreAction::RemoveWorkspace(ws.to_owned()));
    }
}

/// The app is closing: every workspace still being handed to a node is
/// this computer's again before the last save, for the same reason no save
/// is made meanwhile ([`Outbox::handing_over`]). A node that did take one
/// lists it when it is next read, which makes it its mirror then.
pub fn abandon(m: &mut Model) {
    let pending: Vec<(WsId, Handover)> = m
        .live
        .nodes
        .outbox
        .jobs()
        .filter_map(|job| match job {
            Job::Adopt {
                workspace,
                handover,
            } => Some((workspace.id.clone(), *handover)),
            Job::Ops(_) => None,
        })
        .collect();
    m.live.nodes.outbox.lines.clear();
    for (ws, handover) in pending {
        take_back(m, &ws, handover);
    }
}

/// Node `node` answered what it was last sent. A yes moves its line on,
/// and once nothing is left the node is read again, which is when its
/// mirrors say what it made of the changes. A no, or no answer, ends the
/// line there: what waited behind was made on top of what failed. The
/// workspaces being handed over come back ([`take_back`]), the mirrors are
/// read again from the node, and one notice says what was not done.
pub fn sent(m: &mut Model, node: &str, result: Result<(), String>) -> Task<Message> {
    let Some(line) = m.live.nodes.outbox.lines.get_mut(node) else {
        return Task::none();
    };
    let Some(job) = line.sending.take() else {
        return Task::none();
    };
    line.epoch += 1;
    let reason = match result {
        Ok(()) if line.waiting.is_empty() => return fetch(m, node),
        Ok(()) => return Task::none(),
        Err(reason) => reason,
    };
    let dropped: Vec<Job> = line.waiting.drain(..).collect();
    for lost in std::iter::once(&job).chain(&dropped) {
        if let Job::Adopt {
            workspace,
            handover,
        } = lost
        {
            take_back(m, &workspace.id, *handover);
        }
    }
    m.live.nodes.held.remove(node);
    let text = failure_words(m, node, &job, &reason);
    Task::batch([
        fetch(m, node),
        Task::done(Message::Intent(Intent::Notice(Notice::Problem(text)))),
    ])
}
