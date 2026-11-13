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
//!
//! A workspace handed over stays on its way (`Workspace::handover`) until
//! the node said whether it took it: a no takes it back, and no answer at
//! all (the link dropped, the node was slow) leaves it as it is, neither
//! followed there nor run from here, until the node's own list says which
//! it was ([`take_in`]). A mirror its node no longer lists after starting
//! again is kept, marked ([`super::Live::lost`]), for the person to send
//! back or remove.

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
pub use crate::state::types::Handover;
use crate::state::types::{AppState, PaneId, PaneRole, Settings, Workspace, WsId};
use crate::state::{HostedEdit, Model, Patch, StoreAction};

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
        .filter(|ws| !unfollowed(m, ws))
        .count()
}

/// Nothing of workspace `ws` is followed on its node for now: it is on its
/// way there and the node has not said whether it took it (the shells this
/// computer started there are about to be replaced, and those of a new
/// workspace do not exist), or its node no longer has it.
pub fn unfollowed(m: &Model, ws: &Workspace) -> bool {
    ws.handover.is_some() || m.live.nodes.lost.contains(&ws.id)
}

/// Workspace `ws` is a mirror its node no longer has.
pub fn is_lost(m: &Model, ws: &str) -> bool {
    m.live.nodes.lost.contains(ws)
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
        *m.live.nodes.reattach.entry(node.to_owned()).or_default() += 1;
        settle(m, node, &own.workspaces);
        let keep = missing(m, node, &own);
        m.apply(StoreAction::SyncHosted {
            node: node.to_owned(),
            workspaces: own.workspaces,
            keep,
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

/// What is said to the person about node `node` and workspace `name`.
fn words(m: &Model, key: Key, node: &str, name: &str) -> String {
    let label = super::label(m, node);
    Translator::new(m.locale())
        .t_with(key, &args!("name" => name, "node" => label.as_str()))
        .into_owned()
}

/// The workspaces still on their way to node `node` whose answer never
/// came, settled by what the node lists: one it lists is its own, and its
/// mirror from now on; one it does not list was never taken, and is this
/// computer's again ([`take_back`]). The person is told which, either way.
/// One whose request is still in the line is left to its answer.
fn settle(m: &mut Model, node: &str, listed: &[Workspace]) {
    let pending: Vec<(WsId, String, Handover)> = m
        .state()
        .workspaces
        .iter()
        .filter(|ws| ws.node.as_deref() == Some(node))
        .filter(|ws| !m.live.nodes.outbox.adopting(&ws.id))
        .filter_map(|ws| Some((ws.id.clone(), ws.name.clone(), ws.handover?)))
        .collect();
    for (id, name, handover) in pending {
        let taken = listed.iter().any(|their| their.id == id);
        let notice = if taken {
            arrived(m, &id);
            Notice::Agent(words(m, Key::NodesAdoptSettled, node, &name))
        } else {
            take_back(m, &id, handover);
            let key = match handover {
                Handover::Created => Key::NodesAdoptNotTakenCreate,
                Handover::Moved => Key::NodesAdoptNotTakenMove,
                Handover::Restored => Key::NodesAdoptNotTakenRestore,
            };
            Notice::Problem(words(m, key, node, &name))
        };
        m.live.nodes.told.push(notice);
    }
}

/// Workspace `ws` reached its node: it is no longer on its way.
fn arrived(m: &mut Model, ws: &str) {
    if let Some(mut mirror) = m.workspace(ws).filter(|ws| ws.handover.is_some()).cloned() {
        mirror.handover = None;
        m.apply(StoreAction::PutWorkspace(Box::new(mirror)));
    }
}

/// The mirrors of node `node` that `own` does not list and that are kept
/// all the same: those the node no longer has since it started again.
///
/// A workspace missing from a list of the run the mirrors last agreed with
/// (`NodeConfig::boot`) was deleted on the node, and its mirror goes. One
/// missing from the list of another run was not: the node came back
/// without it (stopped before it wrote its state, its state folder gone,
/// another home). Its mirror may be all that is left of the workspace, so
/// it stays, marked ([`super::Live::lost`]), and the person is told once.
/// The run is only taken as the one agreed with once no such mirror is
/// left, so that they are found again when this Hangar next starts.
fn missing(m: &mut Model, node: &str, own: &OwnState) -> Vec<WsId> {
    let agreed = m
        .settings()
        .nodes
        .iter()
        .find(|known| known.id == node)
        .and_then(|known| known.boot.clone());
    let another_run = agreed.as_deref().is_some_and(|boot| boot != own.boot);
    let listed = |id: &str| own.workspaces.iter().any(|their| their.id == id);
    let unlisted: Vec<(WsId, String)> = m
        .state()
        .workspaces
        .iter()
        .filter(|ws| ws.is_hosted() && ws.node.as_deref() == Some(node))
        .filter(|ws| ws.handover.is_none() && !listed(&ws.id))
        .map(|ws| (ws.id.clone(), ws.name.clone()))
        .collect();
    let lost = &mut m.live.nodes.lost;
    lost.retain(|id| !listed(id));
    let mut news = Vec::new();
    for (id, name) in &unlisted {
        if another_run && lost.insert(id.clone()) {
            news.push(name.as_str());
        }
    }
    let keep: Vec<WsId> = unlisted
        .iter()
        .filter(|(id, _)| lost.contains(id))
        .map(|(id, _)| id.clone())
        .collect();
    if !news.is_empty() {
        let text = words(m, Key::NodesLostNotice, node, &news.join(", "));
        m.live.nodes.told.push(Notice::Problem(text));
    }
    if keep.is_empty() && agreed.as_deref() != Some(own.boot.as_str()) {
        let (node, boot) = (node.to_owned(), own.boot.clone());
        m.apply(StoreAction::UpdateSettings(Patch::new(
            move |settings: &mut Settings| {
                for known in settings.nodes.iter_mut().filter(|known| known.id == node) {
                    known.boot = Some(boot.clone());
                }
            },
        )));
    }
    keep
}

/// Node `node` is no longer one of this Hangar's machines: its mirrors go,
/// and what was known of their panes with them. The node runs them still.
/// A workspace still on its way there stays this computer's: taken back
/// first, it is not a mirror when the mirrors go.
pub fn forget(m: &mut Model, node: &str, now: u64) {
    let pending: Vec<(WsId, Handover)> = m
        .state()
        .workspaces
        .iter()
        .filter(|ws| ws.node.as_deref() == Some(node))
        .filter_map(|ws| Some((ws.id.clone(), ws.handover?)))
        .collect();
    for (ws, handover) in pending {
        take_back(m, &ws, handover);
    }
    let before = mirrored_panes(m.state(), node);
    m.apply(StoreAction::SyncHosted {
        node: node.to_owned(),
        workspaces: Vec::new(),
        keep: Vec::new(),
    });
    m.live.nodes.held.remove(node);
    m.live.nodes.outbox.lines.remove(node);
    let state = m.state_arc();
    m.live
        .nodes
        .lost
        .retain(|id| state.workspaces.iter().any(|ws| &ws.id == id));
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
    /// Its node's link dropped a moment ago and is coming back.
    Reconnecting,
    /// Its node is being connected for the first time since the app
    /// started.
    Connecting,
    /// It is on its way to its node, which has not said whether it took
    /// it: deleted now, what the node made of it would be nobody's.
    OnItsWay,
    /// Its node no longer has it: there is nobody to tell.
    Lost,
}

/// Why `edit`, a change the person makes to a workspace its node runs,
/// cannot be told to the node now; `None` when it can. The node no longer
/// has the workspace; the node is away, which is said as what it is (a
/// link that is coming up or coming back is not a machine that is
/// offline); or the change deletes a workspace that is still on its way to
/// the node, whose shells there would be left to nobody.
pub fn edit_refusal(m: &Model, edit: &HostedEdit) -> Option<Why> {
    use super::presence::Presence;
    if is_lost(m, &edit.workspace) {
        return Some(Why::Lost);
    }
    if client(m, &edit.node).is_none() {
        let now = std::time::Instant::now();
        return Some(match super::presence::of(m, &edit.node, now) {
            Presence::Connecting { again: false } => Why::Connecting,
            Presence::Settling | Presence::Connecting { again: true } => {
                let known = m.live.nodes.links.get(&edit.node);
                match known.is_some_and(|link| link.was_connected) {
                    true => Why::Reconnecting,
                    false => Why::Connecting,
                }
            }
            _ => Why::Offline,
        });
    }
    let deletes = edit
        .ops
        .iter()
        .any(|op| matches!(op, crate::state::hosted::HostedOp::RemoveWorkspace));
    let on_its_way = m
        .workspace(&edit.workspace)
        .is_some_and(|ws| ws.handover.is_some());
    (deletes && on_its_way).then_some(Why::OnItsWay)
}

/// What the notice says when a change to a workspace its node runs was
/// refused.
pub fn refusal_words(m: &Model, edit: &HostedEdit, why: Why) -> String {
    let node = super::label(m, &edit.node);
    let key = match why {
        Why::Offline => Key::NodesHostedRefused,
        Why::Unattended => Key::NodesHostedUnattended,
        Why::Worktree => Key::NodesHostedWorktree,
        Why::Reconnecting => Key::NodesHostedReconnecting,
        Why::Connecting => Key::NodesHostedConnecting,
        Why::OnItsWay => Key::NodesHostedOnItsWay,
        Why::Lost => Key::NodesHostedLost,
    };
    Translator::new(m.locale())
        .t_with(
            key,
            &args!("name" => edit.name.as_str(), "node" => node.as_str()),
        )
        .into_owned()
}

/// Why a node did not take what it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotTaken {
    /// It said no: what was asked is not done.
    Refused(String),
    /// It was never asked: the node was not connected.
    Unsent(String),
    /// Nothing says whether it did it: the link dropped on the way, the
    /// answer took too long, or the node failed while answering.
    Unknown(String),
}

impl NotTaken {
    fn reason(&self) -> &str {
        let (NotTaken::Refused(reason) | NotTaken::Unsent(reason) | NotTaken::Unknown(reason)) =
            self;
        reason
    }
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

    /// The handing of workspace `ws` to its node waits in the line or is
    /// on its way: its answer is still to come.
    pub fn adopting(&self, ws: &str) -> bool {
        self.jobs()
            .any(|job| matches!(job, Job::Adopt { workspace, .. } if workspace.id == ws))
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
/// turn. It is on its way (`Workspace::handover`) until the node answered.
/// `false` when it is not a workspace this computer drives on a node.
pub fn hand_over(m: &mut Model, ws: &str, handover: Handover) -> bool {
    let Some(found) = m.workspace(ws).filter(|found| !found.is_hosted()) else {
        return false;
    };
    let Some(node) = found.node.clone() else {
        return false;
    };
    hand(m, found.clone(), &node, handover);
    true
}

/// `found` flagged as its node's and on its way, and its adoption queued.
fn hand(m: &mut Model, found: Workspace, node: &str, handover: Handover) {
    let mut mirror = found;
    mirror.hosted = Some(true);
    mirror.handover = Some(handover);
    m.apply(StoreAction::PutWorkspace(Box::new(mirror.clone())));
    // What the node says of it next is taken whole, whatever was held.
    m.live.nodes.held.remove(node);
    mirror.handover = None;
    m.live.nodes.outbox.push(
        node,
        Job::Adopt {
            workspace: Box::new(mirror),
            handover,
        },
    );
}

/// Why the mirror `ws`, which its node no longer has, cannot be sent back
/// to it now, in words; `None` when it can.
pub fn restore_refusal(m: &Model, ws: &Workspace) -> Option<String> {
    let node = ws.node.as_deref()?;
    client(m, node)
        .is_none()
        .then(|| words(m, Key::NodesMoveOffline, node, &ws.name))
}

/// The mirror `ws`, which its node no longer has, is handed to the node
/// again, as a workspace moved there is: the node starts its panes under
/// their ids, each Claude Code on its session. `false` when it is not such
/// a mirror, or its node cannot be reached.
pub fn restore(m: &mut Model, ws: &str) -> bool {
    let Some(found) = m.workspace(ws).filter(|_| is_lost(m, ws)).cloned() else {
        return false;
    };
    let Some(node) = found.node.clone().filter(|node| client(m, node).is_some()) else {
        return false;
    };
    m.live.nodes.lost.remove(ws);
    hand(m, found, &node, Handover::Restored);
    true
}

/// The mirror `ws`, which its node no longer has, leaves the list: there
/// is nothing on the node to delete, and nobody to tell.
pub fn remove_lost(m: &mut Model, ws: &str) {
    if m.live.nodes.lost.remove(ws) {
        take_back(m, ws, Handover::Created);
    }
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

/// `job` told to the node `client` reaches. Only an answer of the node
/// that says no is a no ([`NotTaken::Refused`]): a request that got no
/// answer may have been carried out all the same.
pub async fn send(client: &NodeClient, job: &Job) -> Result<(), NotTaken> {
    let asked = match job {
        Job::Ops(edit) => {
            let path = format!("/node/own/workspaces/{}/ops", wire::encode(&edit.workspace));
            let body = json!({ "ops": edit.ops });
            wire::request(client, "POST", &path, Some(&body)).await
        }
        Job::Adopt { workspace, .. } => {
            let body =
                serde_json::to_value(workspace).map_err(|err| NotTaken::Unsent(err.to_string()))?;
            wire::request(client, "POST", "/node/own/adopt", Some(&body)).await
        }
    };
    let answer = asked.map_err(NotTaken::Unknown)?;
    match answer.status {
        200..=299 => Ok(()),
        400..=499 => Err(NotTaken::Refused(answer.error())),
        _ => Err(NotTaken::Unknown(answer.error())),
    }
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
            (None, away) => Task::done(answered(Err(NotTaken::Unsent(away.unwrap_or_default())))),
        });
    }
    Task::batch(tasks)
}

/// What the notice says when node `node` did not take `job`, or did not
/// say: what became of the change or of the workspace, which is only said
/// to be undone when it is known to be.
pub fn failure_words(m: &Model, node: &str, job: &Job, why: &NotTaken) -> String {
    let label = super::label(m, node);
    let unknown = matches!(why, NotTaken::Unknown(_));
    let key = match job {
        Job::Ops(_) if matches!(why, NotTaken::Refused(_)) => Key::NodesHostedNotDone,
        Job::Ops(_) => Key::NodesHostedNotSent,
        Job::Adopt { .. } if unknown => Key::NodesAdoptUnsettled,
        Job::Adopt { handover, .. } => match handover {
            Handover::Created => Key::NodesAdoptCreateFailed,
            Handover::Moved => Key::NodesAdoptMoveFailed,
            Handover::Restored => Key::NodesAdoptRestoreFailed,
        },
    };
    Translator::new(m.locale())
        .t_with(
            key,
            &args!(
                "name" => job.name(),
                "node" => label.as_str(),
                "reason" => why.reason().trim_end_matches('.'),
            ),
        )
        .into_owned()
}

/// A workspace handed to its node that the node did not take is this
/// computer's again: one it drove is driven from here as before, its
/// shells found where they were left or started again; one just made for
/// the node is taken back out; a mirror the node no longer had is that
/// again.
fn take_back(m: &mut Model, ws: &str, handover: Handover) {
    let Some(mut local) = m.workspace(ws).cloned() else {
        return;
    };
    local.handover = None;
    if handover != Handover::Restored {
        local.hosted = None;
    }
    m.apply(StoreAction::PutWorkspace(Box::new(local)));
    match handover {
        Handover::Created => {
            m.apply(StoreAction::RemoveWorkspace(ws.to_owned()));
        }
        Handover::Moved => {}
        Handover::Restored => {
            m.live.nodes.lost.insert(ws.to_owned());
        }
    }
}

/// Node `node` answered what it was last sent. A yes moves its line on,
/// and once nothing is left the node is read again, which is when its
/// mirrors say what it made of the changes. A no, or no answer, ends the
/// line there: what waited behind was made on top of what failed. A
/// workspace the node refused, or that was never sent, comes back
/// ([`take_back`]); one whose answer never came stays on its way until the
/// node's list settles it ([`settle`]). The mirrors are read again from
/// the node, and one notice says what was not done, or is not known.
pub fn sent(m: &mut Model, node: &str, result: Result<(), NotTaken>) -> Task<Message> {
    let Some(line) = m.live.nodes.outbox.lines.get_mut(node) else {
        return Task::none();
    };
    let Some(job) = line.sending.take() else {
        return Task::none();
    };
    line.epoch += 1;
    let why = match result {
        Ok(()) => {
            let last = line.waiting.is_empty();
            if let Job::Adopt { workspace, .. } = &job {
                arrived(m, &workspace.id);
            }
            return if last { fetch(m, node) } else { Task::none() };
        }
        Err(why) => why,
    };
    let dropped: Vec<Job> = line.waiting.drain(..).collect();
    if let Job::Adopt {
        workspace,
        handover,
    } = &job
        && !matches!(why, NotTaken::Unknown(_))
    {
        take_back(m, &workspace.id, *handover);
    }
    // What waited behind was never sent.
    for unsent in &dropped {
        if let Job::Adopt {
            workspace,
            handover,
        } = unsent
        {
            take_back(m, &workspace.id, *handover);
        }
    }
    m.live.nodes.held.remove(node);
    let text = failure_words(m, node, &job, &why);
    Task::batch([
        fetch(m, node),
        Task::done(Message::Intent(Intent::Notice(Notice::Problem(text)))),
    ])
}

/// How long the app waits, as it closes, for the nodes to take what was
/// still to be told them.
pub const FLUSH: std::time::Duration = std::time::Duration::from_secs(4);

/// The app is closing: what the nodes were still to be told is sent now,
/// each node's line in order, all of them given `within` to answer. A
/// change left unsent would be undone without a word at the next start,
/// the mirror having been saved with it and the node never told. What was
/// on its way when the app closed is sent again: the node takes the same
/// ops, and the same workspace, twice as once.
///
/// A workspace the node took is its mirror; one it refused, or that could
/// not be sent, is this computer's again; one whose answer did not come in
/// time stays on its way, which is saved with it and settled at the next
/// start. Answers one line for each change that did not get through, for
/// the log: nobody is left to read a notice.
pub fn flush(m: &mut Model, within: std::time::Duration) -> Vec<String> {
    let lines: Vec<(String, Option<NodeClient>, Vec<Job>)> =
        std::mem::take(&mut m.live.nodes.outbox.lines)
            .into_iter()
            .map(|(node, line)| {
                let jobs: Vec<Job> = line.sending.into_iter().chain(line.waiting).collect();
                (node.clone(), client(m, &node), jobs)
            })
            .filter(|(_, _, jobs)| !jobs.is_empty())
            .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let outcomes = told_within(&lines, within);
    let mut log = Vec::new();
    for ((node, _, jobs), answers) in lines.iter().zip(outcomes) {
        for (at, job) in jobs.iter().enumerate() {
            // What follows the first job that did not get through was not
            // sent.
            let answer = answers
                .get(at)
                .cloned()
                .unwrap_or_else(|| Err(NotTaken::Unsent("not sent".into())));
            match (job, &answer) {
                (Job::Adopt { workspace, .. }, Ok(())) => arrived(m, &workspace.id),
                (_, Ok(())) | (Job::Adopt { .. }, Err(NotTaken::Unknown(_))) => {}
                (
                    Job::Adopt {
                        workspace,
                        handover,
                    },
                    Err(_),
                ) => take_back(m, &workspace.id, *handover),
                (Job::Ops(_), Err(_)) => {}
            }
            if let Err(why) = &answer {
                log.push(failure_words(m, node, job, why));
            }
        }
    }
    log
}

/// Each line's jobs sent in order to its node, the lines side by side,
/// all within `within`: for each line, the answers of the jobs that were
/// sent, which stop at the first that did not get through. On a thread of
/// its own: the app's loop is ending, and this is waited for.
fn told_within(
    lines: &[(String, Option<NodeClient>, Vec<Job>)],
    within: std::time::Duration,
) -> Vec<Vec<Result<(), NotTaken>>> {
    let work: Vec<(Option<NodeClient>, Vec<Job>)> = lines
        .iter()
        .map(|(_, client, jobs)| (client.clone(), jobs.clone()))
        .collect();
    let count = work.len();
    let told = std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return Vec::new();
        };
        runtime.block_on(async move {
            let lines = work.into_iter().map(|(client, jobs)| async move {
                let mut answers = Vec::new();
                let Some(client) = client else {
                    return answers;
                };
                let deadline = tokio::time::Instant::now() + within;
                for job in &jobs {
                    let answer = tokio::time::timeout_at(deadline, send(&client, job))
                        .await
                        .unwrap_or_else(|_| Err(NotTaken::Unknown("no answer in time".into())));
                    let through = answer.is_ok();
                    answers.push(answer);
                    if !through {
                        break;
                    }
                }
                answers
            });
            iced::futures::future::join_all(lines).await
        })
    });
    told.join().unwrap_or_else(|_| vec![Vec::new(); count])
}
