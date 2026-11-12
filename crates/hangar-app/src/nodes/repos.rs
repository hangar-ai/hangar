//! The same repository on several machines (docs/native/nodes.md): which
//! workspaces are checkouts of one repository, so that a row can say its
//! repository is open elsewhere too and a menu can go there.
//!
//! A repository is the address of its `origin`, brought to one spelling
//! ([`identity`]). A workspace with no `origin` has no identity and is never
//! grouped. The address of each workspace is read where its folder is: by
//! git here for a folder of this computer, by the node for a folder of its
//! own (beside its state, `GET /node/own/state`, or asked by folder for a
//! workspace this computer still drives there). What was read is held in
//! memory only ([`Known`]), asked for after any message that left a
//! workspace without it ([`refresh`]), and read again when a node connects
//! or the window comes back to the front. Nothing is drawn from an answer
//! that is not there yet: a row waits for it without a mark.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::Task;

use hangar_i18n::{Key, Translator, args};

use super::{Msg, client, engine};
use crate::message::Message;
use crate::state::Model;
use crate::state::types::{Workspace, WsId};

/// How long the origins read on this computer are believed before the
/// window coming back to the front has them read again.
const LOCAL_FRESH: Duration = Duration::from_secs(60);

/// The one spelling of a repository's address: its host in lower case and
/// its path, without scheme, user, port, `.git` or a trailing slash, so
/// that `git@github.com:Acme/Shop.git` and `https://github.com/acme/shop`
/// are one (`github.com/acme/shop`). The path's case is dropped with the
/// host's, as the hosts that serve repositories do. `None` for an address
/// that names no host: a folder or a `file:` address means another
/// repository on each machine.
pub fn identity(origin: &str) -> Option<String> {
    let origin = origin.trim();
    let (authority, path) = match origin.split_once("://") {
        Some((scheme, _)) if scheme.eq_ignore_ascii_case("file") => return None,
        Some((_, rest)) => rest.split_once('/').unwrap_or((rest, "")),
        None => {
            // `[user@]host:path`, as scp spells it. A colon after the first
            // slash is part of a path; one letter before it is a drive.
            let (host, path) = origin.split_once(':')?;
            if host.contains('/') || host.contains('\\') || host.len() < 2 {
                return None;
            }
            (host, path)
        }
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    };
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let path = path.trim_matches('/');
    let path = match path.len().checked_sub(4) {
        Some(at) if path.is_char_boundary(at) && path[at..].eq_ignore_ascii_case(".git") => {
            path[..at].trim_end_matches('/')
        }
        _ => path,
    };
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{}/{}", host.to_lowercase(), path.to_lowercase()))
}

/// A workspace's repository: its address as its machine spells it, which
/// is what another machine is given to clone, and what it is grouped by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub url: String,
    pub identity: String,
}

impl Origin {
    pub fn of(url: &str) -> Option<Origin> {
        Some(Origin {
            identity: identity(url)?,
            url: url.trim().to_owned(),
        })
    }
}

/// What is known of one workspace's repository, and of which folder: a
/// workspace whose folder or machine changed is read again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    cwd: String,
    node: Option<String>,
    origin: Option<Origin>,
    /// Asked and not answered yet.
    asked: bool,
    /// To be read again; what was read stands meanwhile.
    stale: bool,
}

impl Entry {
    fn is_of(&self, ws: &Workspace) -> bool {
        self.cwd == ws.cwd && self.node == ws.node
    }
}

/// The repositories of the workspaces, by workspace. Never saved.
#[derive(Debug, Default)]
pub struct Known {
    entries: HashMap<WsId, Entry>,
    /// When the folders of this computer were last asked.
    local_read: Option<Instant>,
}

impl Known {
    /// The repository of workspace `ws`, once read for the folder it has.
    pub fn origin(&self, ws: &Workspace) -> Option<&Origin> {
        self.entries
            .get(&ws.id)
            .filter(|entry| entry.is_of(ws))
            .and_then(|entry| entry.origin.as_ref())
    }

    /// The address of workspace `ws` is `url` (`None`: it has none), as its
    /// machine just said.
    pub fn said(&mut self, ws: &Workspace, url: Option<&str>) {
        self.entries.insert(
            ws.id.clone(),
            Entry {
                cwd: ws.cwd.clone(),
                node: ws.node.clone(),
                origin: url.and_then(Origin::of),
                asked: false,
                stale: false,
            },
        );
    }

    /// The answer to a question [`refresh`] asked, kept when the workspace
    /// still has the folder it was asked about.
    pub fn answered(&mut self, ws: &str, cwd: &str, node: Option<&str>, url: Option<&str>) {
        if let Some(entry) = self
            .entries
            .get_mut(ws)
            .filter(|entry| entry.cwd == cwd && entry.node.as_deref() == node)
        {
            entry.origin = url.and_then(Origin::of);
            entry.asked = false;
        }
    }

    /// The workspaces this computer drives on node `node` are read again:
    /// the node just connected, and may say what it could not before.
    pub fn stale_node(&mut self, node: &str) {
        for entry in self.entries.values_mut() {
            if entry.node.as_deref() == Some(node) {
                entry.stale = true;
            }
        }
    }

    /// The folders of this computer are read again, unless they just were.
    pub fn stale_local(&mut self, now: Instant) {
        let fresh = self
            .local_read
            .is_some_and(|read| now.saturating_duration_since(read) < LOCAL_FRESH);
        if fresh {
            return;
        }
        for entry in self.entries.values_mut() {
            if entry.node.is_none() {
                entry.stale = true;
            }
        }
    }
}

/// Whether the repositories are worth knowing: only several machines make
/// one repository open in several places.
fn wanted(m: &Model) -> bool {
    super::available(m) && !m.settings().nodes.is_empty()
}

/// Each workspace whose repository is not known for the folder it has is
/// asked for it, off this thread; what is known of workspaces that are gone
/// is dropped. Run after every message: it costs a look at each workspace,
/// and asks nothing twice.
pub fn refresh(m: &mut Model, now: Instant) -> Task<Message> {
    if !wanted(m) {
        return Task::none();
    }
    let mut asks = Vec::new();
    let mut local_asked = false;
    let state = m.state_arc();
    let known = &m.live.nodes.repos;
    for ws in &state.workspaces {
        let entry = known.entries.get(&ws.id).filter(|entry| entry.is_of(ws));
        if entry.is_some_and(|entry| entry.asked || !entry.stale) {
            continue;
        }
        let ask = match ws.node.as_deref() {
            None => Some(None),
            // What a node runs itself it says with its state.
            Some(_) if ws.is_hosted() => None,
            Some(node) => client(m, node).map(Some),
        };
        let Some(client) = ask else {
            continue;
        };
        local_asked |= client.is_none();
        asks.push((
            ws.clone(),
            entry.and_then(|entry| entry.origin.clone()),
            client,
        ));
    }
    let known = &mut m.live.nodes.repos;
    known
        .entries
        .retain(|id, _| state.workspaces.iter().any(|ws| ws.id == *id));
    if local_asked {
        known.local_read = Some(now);
    }
    let mut tasks = Vec::new();
    for (ws, origin, client) in asks {
        known.entries.insert(
            ws.id.clone(),
            Entry {
                cwd: ws.cwd.clone(),
                node: ws.node.clone(),
                origin,
                asked: true,
                stale: false,
            },
        );
        let cwd = ws.cwd.clone();
        let read = async move {
            match client {
                Some(client) => engine::origin(&client, &cwd).await,
                None => tokio::task::spawn_blocking(move || {
                    hangar_core::git_clone::git_origin(std::path::Path::new(&cwd))
                })
                .await
                .ok()
                .flatten(),
            }
        };
        tasks.push(Task::perform(read, move |origin| {
            Message::Nodes(Msg::Origin {
                ws: ws.id.clone(),
                cwd: ws.cwd.clone(),
                node: ws.node.clone(),
                origin,
            })
        }));
    }
    Task::batch(tasks)
}

/// The other workspaces that are checkouts of the repository of `ws`, on
/// another machine than its own, in the order of the list. Those of a
/// machine that is switched off are out of the lists and do not count; two
/// checkouts on one machine are that machine's business.
pub fn siblings<'a>(m: &'a Model, ws: &Workspace) -> Vec<&'a Workspace> {
    if !wanted(m) || !super::shown(m, ws) {
        return Vec::new();
    }
    let known = &m.live.nodes.repos;
    let Some(origin) = known.origin(ws) else {
        return Vec::new();
    };
    m.state()
        .workspaces
        .iter()
        .filter(|other| other.id != ws.id && other.node != ws.node)
        .filter(|other| other.hq != Some(true) && super::shown(m, other))
        .filter(|other| {
            known
                .origin(other)
                .is_some_and(|theirs| theirs.identity == origin.identity)
        })
        .collect()
}

/// Where the siblings are, in words: "vps (api), studio (render)", each
/// machine with the name the workspace has there. This computer is named
/// as it is in a sentence.
fn places(m: &Model, tr: Translator, siblings: &[&Workspace]) -> String {
    siblings
        .iter()
        .map(|other| {
            let machine = match other.node.as_deref() {
                Some(node) => super::label(m, node),
                None => tr.t(Key::NodesRepoHere).into_owned(),
            };
            tr.t_with(
                Key::NodesRepoPlace,
                &args!("machine" => machine, "name" => other.name.as_str()),
            )
            .into_owned()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the mark of a row of this computer says when its repository is
/// open on other machines too: "Also on vps (api)". `None` when it is not.
pub fn also_on(m: &Model, tr: Translator, ws: &Workspace) -> Option<String> {
    let siblings = siblings(m, ws);
    (!siblings.is_empty()).then(|| {
        tr.t_with(
            Key::NodesRepoAlso,
            &args!("where" => places(m, tr, &siblings)),
        )
        .into_owned()
    })
}

/// The tooltip of the machine mark of a row, `tip`, with the other machines
/// its repository is open on: "vps · connected · also on this computer
/// (shop)". `tip` as it is when there is none.
pub fn tip_with_others(m: &Model, tr: Translator, ws: &Workspace, tip: String) -> String {
    let siblings = siblings(m, ws);
    if siblings.is_empty() {
        return tip;
    }
    tr.t_with(
        Key::NodesRepoTipAlso,
        &args!("tip" => tip, "where" => places(m, tr, &siblings)),
    )
    .into_owned()
}

/// Whether the repository of a workspace can be opened on one more
/// machine, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elsewhere {
    /// On this machine (`None` is this computer), by cloning `repo`.
    Ready { repo: String, place: Option<String> },
    /// Every machine that is connected has it already.
    Everywhere,
}

/// Where the repository of `ws` could be opened next: the first machine,
/// this computer then the connected nodes in their order, that is not its
/// own and has no checkout of it yet. `None` when the workspace has no
/// repository, or there is no other machine.
pub fn elsewhere(m: &Model, ws: &Workspace) -> Option<Elsewhere> {
    if !wanted(m) {
        return None;
    }
    let origin = m.live.nodes.repos.origin(ws)?;
    let has_it: Vec<Option<&str>> = std::iter::once(ws)
        .chain(siblings(m, ws))
        .map(|holder| holder.node.as_deref())
        .collect();
    let connected = super::connected(m);
    let place = std::iter::once(None)
        .chain(connected.iter().map(|(id, _)| Some(id.as_str())))
        .find(|machine| !has_it.contains(machine));
    Some(match place {
        Some(place) => Elsewhere::Ready {
            repo: origin.url.clone(),
            place: place.map(str::to_owned),
        },
        None => Elsewhere::Everywhere,
    })
}

#[cfg(test)]
mod tests;
