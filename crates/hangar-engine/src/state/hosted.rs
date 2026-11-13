//! Hosted workspaces: those a node runs itself, which a desktop mirrors
//! (`Workspace::hosted`).
//!
//! The node is the owner: it holds the workspace in its own state and starts
//! its panes. The desktop keeps a copy, the mirror, so that everything that
//! reads a workspace reads this one like any other. Two things cross between
//! them, and both are decided here, with no I/O:
//!
//! - what the node says replaces the mirror's ([`merge`], [`mirror_of`]);
//! - what the person changes on the mirror is sent to the node as
//!   [`HostedOp`]s ([`diff`]), which the node applies to its own copy
//!   ([`apply_op`]).
//!
//! Which fields are the node's and which stay the mirror's own presentation
//! is said once, by [`workspace_fields`] and [`pane_fields`]. Both take
//! their struct apart field by field: a field added to `Workspace` or `Pane`
//! does not compile until it is given a side there.

use serde::{Deserialize, Deserializer, Serialize};

use super::Store;
use super::layout::{grid_tree, insert_leaf, leaf, leaf_ids, normalize_tree, remove_leaf};
use super::layout::{preset_tree, rename_leaf};
use super::store::{PanePatch, Respawn, StoreAction, StoreOutcome, WorkspacePatch};
use super::types::{
    AgentId, DropZone, Extra, OrchestrationSettings, Pane, PaneId, PaneRole, SplitNode, Workspace,
    WorkspaceMcp, WorktreeHooks,
};

/// A field that is there, whatever it holds: a `null` reads as `Some(None)`
/// for a field that can be emptied, where a field left out reads as `None`
/// (the struct's default).
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A set of fields of `$target`, each there or not: what a patch names is
/// set, the rest is left alone. In JSON a field left out is not named, and
/// one that can be emptied is emptied by `null`.
macro_rules! fields {
    (
        $(#[$doc:meta])*
        $name:ident of $target:ty { $($field:ident: $kind:ty),* $(,)? }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", default)]
        pub struct $name {
            $(
                #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "present")]
                pub $field: Option<$kind>,
            )*
        }

        impl $name {
            /// Names nothing.
            pub fn is_empty(&self) -> bool {
                $(self.$field.is_none())&&*
            }

            /// Sets what it names.
            pub fn apply(&self, target: &mut $target) {
                $(
                    if let Some(value) = &self.$field {
                        target.$field = value.clone();
                    }
                )*
            }

            /// What it holds that `other` does not hold the same.
            fn besides(self, other: &$name) -> $name {
                $name {
                    $(
                        $field: if self.$field != other.$field {
                            self.$field
                        } else {
                            None
                        },
                    )*
                }
            }
        }
    };
}

fields! {
    /// The fields of a workspace its node owns, beside its panes.
    WorkspaceFields of Workspace {
        name: String,
        cwd: String,
        extra_roots: Vec<String>,
        shell_id: Option<String>,
        mcp: Option<WorkspaceMcp>,
        orchestration: Option<OrchestrationSettings>,
        hq: Option<bool>,
        claude_account: Option<String>,
        worktree_hooks: Option<WorktreeHooks>,
    }
}

fields! {
    /// The fields of a pane its node owns: all of them but its id, which
    /// only a respawn changes.
    PaneFields of Pane {
        name: String,
        agent: AgentId,
        session_id: Option<String>,
        cwd: Option<String>,
        shell_id: Option<String>,
        title: Option<String>,
        role: Option<PaneRole>,
        task_id: Option<String>,
        model: Option<String>,
        port_base: Option<u32>,
        shared_folder: Option<bool>,
        node: Option<String>,
        claude_account: Option<String>,
        login: Option<bool>,
        extra: Extra,
    }
}

/// Every field of `ws` its node owns, beside its panes (which are the
/// node's too, whole). The others are the mirror's own presentation: a
/// merge keeps them, and changing them sends the node nothing.
fn workspace_fields(ws: &Workspace) -> WorkspaceFields {
    let Workspace {
        // What it is known by on both sides.
        id: _,
        name,
        cwd,
        extra_roots,
        // The node's, pane by pane (`pane_fields`).
        panes: _,
        shell_id,
        mcp,
        orchestration,
        hq,
        claude_account,
        worktree_hooks,
        // The mirror's own: how this Hangar shows it and files it. Its
        // kits are among them: a node starts its agents without any, so
        // the ones chosen here are what this window shows of them (their
        // tabs), and the node is told nothing of them.
        kits: _,
        theme_id: _,
        tree: _,
        view_panes: _,
        saved_commands: _,
        folder_id: _,
        detached: _,
        window_bounds: _,
        mobile_ports: _,
        chat_view: _,
        // Where this Hangar reaches it.
        node: _,
        hosted: _,
        handover: _,
        extra: _,
    } = ws;
    WorkspaceFields {
        name: Some(name.clone()),
        cwd: Some(cwd.clone()),
        extra_roots: Some(extra_roots.clone()),
        shell_id: Some(shell_id.clone()),
        mcp: Some(mcp.clone()),
        orchestration: Some(orchestration.clone()),
        hq: Some(*hq),
        claude_account: Some(claude_account.clone()),
        worktree_hooks: Some(worktree_hooks.clone()),
    }
}

/// Every field of `pane` its node owns.
fn pane_fields(pane: &Pane) -> PaneFields {
    let Pane {
        id: _,
        name,
        agent,
        session_id,
        cwd,
        shell_id,
        title,
        role,
        task_id,
        model,
        port_base,
        shared_folder,
        node,
        claude_account,
        login,
        // Said once by the Hangar that read the file, and never written.
        retired_agent: _,
        extra,
    } = pane;
    PaneFields {
        name: Some(name.clone()),
        agent: Some(*agent),
        session_id: Some(session_id.clone()),
        cwd: Some(cwd.clone()),
        shell_id: Some(shell_id.clone()),
        title: Some(title.clone()),
        role: Some(*role),
        task_id: Some(task_id.clone()),
        model: Some(model.clone()),
        port_base: Some(*port_base),
        shared_folder: Some(*shared_folder),
        node: Some(node.clone()),
        claude_account: Some(claude_account.clone()),
        login: Some(*login),
        extra: Some(extra.clone()),
    }
}

/// The fields of `after` that `before` does not hold the same.
fn pane_changes(before: &Pane, after: &Pane) -> PaneFields {
    pane_fields(after).besides(&pane_fields(before))
}

/// What a mirror asks of its node: one change to the node's copy of the
/// workspace, as the person made it on the mirror. The workspace is named
/// beside the ops (`POST /node/own/workspaces/{id}/ops`).
///
/// In JSON each is an object tagged by `op`: `{"op": "putPane", "pane":
/// {…}}`, `{"op": "closePane", "pane": "<id>"}`, `{"op": "respawnPane",
/// "from": "<id>", "to": "<id>", "patch": {…}}`, `{"op": "patchWorkspace",
/// "patch": {…}}`, `{"op": "patchPane", "pane": "<id>", "patch": {…}}`,
/// `{"op": "removeWorkspace"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum HostedOp {
    /// A new pane, with the id the desktop chose: what the desktop made of
    /// it (its tree, its focus) is already keyed by that id.
    PutPane {
        pane: Pane,
    },
    ClosePane {
        pane: PaneId,
    },
    /// Pane `from` restarted as `to`, with what the restart changed (its
    /// agent, its session).
    RespawnPane {
        from: PaneId,
        to: PaneId,
        #[serde(default, skip_serializing_if = "PaneFields::is_empty")]
        patch: PaneFields,
    },
    PatchWorkspace {
        patch: WorkspaceFields,
    },
    PatchPane {
        pane: PaneId,
        patch: PaneFields,
    },
    RemoveWorkspace,
}

/// The mirror of `theirs`, a workspace node `node` runs, for a Hangar that
/// did not know it: everything the node owns, the node's layout of its
/// terminals to start from, and nothing else of the node's presentation.
pub fn mirror_of(node: &str, theirs: &Workspace) -> Workspace {
    let mut mirror = Workspace {
        id: theirs.id.clone(),
        tree: Some(normalize_tree(&theirs.panes, theirs.tree.as_ref())),
        node: Some(node.to_string()),
        hosted: Some(true),
        ..Workspace::default()
    };
    workspace_fields(theirs).apply(&mut mirror);
    mirror.panes = theirs.panes.clone();
    mirror
}

/// `mirror` brought up to date from `theirs`, its node's copy of the same
/// workspace: what the node owns is replaced, the mirror's presentation is
/// kept, and the result is a mirror (`hosted`) whatever `mirror` was, which
/// is how a workspace handed to its node becomes one.
///
/// The panes keep the order the mirror had them in, those that appeared
/// coming last: the order is the mirror's too (a preset sorts it as the
/// panes are read on its own grid).
///
/// The split tree follows the panes. One the node respawned (gone under one
/// id, there under another, with the same name: names stay with their pane)
/// keeps its leaf under the new id, and is answered so that what else is
/// keyed by the pane can follow. One that appeared is placed where `AddPane`
/// would place it, beside the last pane; one gone gives its room to its
/// sibling. A tree whose panes did not change is left exactly as it was.
pub fn merge(mirror: &Workspace, theirs: &Workspace) -> (Workspace, Vec<Respawn>) {
    let mut next = mirror.clone();
    workspace_fields(theirs).apply(&mut next);
    next.hosted = Some(true);
    let gone = |pane: &Pane| !theirs.panes.iter().any(|their| their.id == pane.id);
    let mut tree = (mirror.pane_count() > 0).then(|| grid_tree(mirror));
    let mut respawns = Vec::new();
    // The node's panes, each in the place of the mirror's pane it continues.
    let mut continued: Vec<Option<Pane>> = vec![None; mirror.panes.len()];
    let mut appeared = Vec::new();
    for their in &theirs.panes {
        let same = mirror.panes.iter().position(|pane| pane.id == their.id);
        let respawned = || {
            mirror.panes.iter().enumerate().position(|(at, pane)| {
                continued[at].is_none()
                    && gone(pane)
                    && !their.name.is_empty()
                    && pane.name == their.name
            })
        };
        let Some(at) = same.or_else(respawned) else {
            appeared.push(their.clone());
            continue;
        };
        let before = &mirror.panes[at];
        if before.id != their.id {
            tree = tree.map(|tree| rename_leaf(&tree, &before.id, &their.id));
            respawns.push(Respawn {
                ws: mirror.id.clone(),
                from: before.id.clone(),
                to: their.id.clone(),
                keeps_session: !(before.session_id.is_some() && their.session_id.is_none()),
            });
        }
        continued[at] = Some(Pane {
            retired_agent: before.retired_agent,
            ..their.clone()
        });
    }
    let closed: Vec<&Pane> = mirror
        .panes
        .iter()
        .zip(&continued)
        .filter(|(_, now)| now.is_none())
        .map(|(pane, _)| pane)
        .collect();
    next.panes = continued.iter().flatten().cloned().collect();
    next.panes.extend(appeared.iter().cloned());
    if closed.is_empty() && appeared.is_empty() && respawns.is_empty() {
        return (next, respawns);
    }
    // The new panes first: one that takes the place of the last pane, which
    // is going, is put beside it before it goes.
    for pane in &appeared {
        tree = Some(match &tree {
            Some(tree) => {
                let last = leaf_ids(tree).pop().unwrap_or_default();
                insert_leaf(tree, &last, &pane.id, DropZone::Right)
            }
            None => leaf(pane.id.clone()),
        });
    }
    for pane in closed {
        tree = tree.and_then(|tree| remove_leaf(&tree, &pane.id));
    }
    next.tree = Some(tree.unwrap_or_else(|| preset_tree(&next.leaf_ids())));
    (next, respawns)
}

/// The id pane `id` goes by once `respawns` (oldest first, as the store
/// lists them) have run.
fn respawned_as<'a>(ws: &str, id: &'a str, respawns: &'a [Respawn]) -> &'a str {
    respawns
        .iter()
        .filter(|respawn| respawn.ws == ws)
        .fold(
            id,
            |id, respawn| {
                if respawn.from == id { &respawn.to } else { id }
            },
        )
}

/// The ops that take the node's copy of a workspace from `before` to
/// `after`, both the mirror's: what the person changed, in what the node
/// owns. A change to the mirror's own presentation (its tree, its folder, a
/// window of its own) gives nothing. `after` is `None` once the mirror was
/// removed. `respawns` are those the store made in between
/// (`Store::respawns`): without them a restarted pane would read as one
/// closed and another opened.
///
/// The panes opened come before those closed: the node, like the store,
/// keeps a workspace's last pane.
pub fn diff(before: &Workspace, after: Option<&Workspace>, respawns: &[Respawn]) -> Vec<HostedOp> {
    let Some(after) = after else {
        return vec![HostedOp::RemoveWorkspace];
    };
    let mut ops = Vec::new();
    let patch = workspace_fields(after).besides(&workspace_fields(before));
    if !patch.is_empty() {
        ops.push(HostedOp::PatchWorkspace { patch });
    }
    let find = |panes: &[Pane], id: &str| panes.iter().position(|pane| pane.id == id);
    // Each pane of `after`, with the pane of `before` it continues.
    let mut continued: Vec<Option<&Pane>> = vec![None; after.panes.len()];
    let mut closed = Vec::new();
    for pane in &before.panes {
        let now = respawned_as(&before.id, &pane.id, respawns);
        match find(&after.panes, now) {
            Some(at) => continued[at] = Some(pane),
            None => closed.push(HostedOp::ClosePane {
                pane: pane.id.clone(),
            }),
        }
    }
    let mut put = Vec::new();
    let mut patched = Vec::new();
    for (pane, was) in after.panes.iter().zip(continued) {
        match was {
            None => put.push(HostedOp::PutPane { pane: pane.clone() }),
            Some(was) if was.id != pane.id => ops.push(HostedOp::RespawnPane {
                from: was.id.clone(),
                to: pane.id.clone(),
                patch: pane_changes(was, pane),
            }),
            Some(was) => {
                let patch = pane_changes(was, pane);
                if !patch.is_empty() {
                    patched.push(HostedOp::PatchPane {
                        pane: pane.id.clone(),
                        patch,
                    });
                }
            }
        }
    }
    ops.extend(put);
    ops.extend(patched);
    ops.extend(closed);
    ops
}

/// `patch` for pane `pane` of the node's, without a name another pane of
/// the node already goes by, or an empty one: the node names its panes,
/// and its agents reach each other by name.
fn settled(store: &Store, pane: &str, mut patch: PaneFields) -> PaneFields {
    let taken = |name: &str| {
        name.trim().is_empty()
            || store
                .state()
                .workspaces
                .iter()
                .flat_map(|ws| &ws.panes)
                .any(|other| other.id != pane && other.name == name)
    };
    patch.name = patch.name.filter(|name| !taken(name));
    patch
}

fn pane_patch(patch: PaneFields) -> PanePatch {
    PanePatch::new(move |pane: &mut Pane| patch.apply(pane))
}

/// `op` applied to workspace `ws` of `store`, the node's: the store action
/// that does it. Applied a second time it changes nothing: the pane is
/// there already, or gone already, or holds what the patch names.
pub fn apply_op(store: &mut Store, ws: &str, op: HostedOp) -> StoreOutcome {
    let ws = ws.to_string();
    let action = match op {
        HostedOp::PutPane { pane } => StoreAction::PutPane {
            ws,
            pane: Box::new(pane),
            near: None,
            dir: None,
        },
        HostedOp::ClosePane { pane } => StoreAction::ClosePane { ws, pane },
        HostedOp::RespawnPane { from, to, patch } => StoreAction::RespawnPaneAs {
            patch: pane_patch(settled(store, &from, patch)),
            ws,
            pane: from,
            to,
        },
        HostedOp::PatchWorkspace { patch } => StoreAction::UpdateWorkspace(
            ws,
            WorkspacePatch::new(move |ws: &mut Workspace| patch.apply(ws)),
        ),
        HostedOp::PatchPane { pane, patch } => StoreAction::UpdatePane {
            patch: pane_patch(settled(store, &pane, patch)),
            ws,
            pane,
        },
        HostedOp::RemoveWorkspace => StoreAction::RemoveWorkspace(ws),
    };
    store.apply(action)
}

/// The tree of a workspace handed to a node: its terminals alone, the
/// leaves of the views (which are the desktop's) dropped.
pub fn terminals_tree(ws: &Workspace) -> SplitNode {
    let terminals: Vec<PaneId> = ws.panes.iter().map(|pane| pane.id.clone()).collect();
    let mut tree = (ws.pane_count() > 0).then(|| grid_tree(ws));
    for view in &ws.view_panes {
        tree = tree.and_then(|tree| remove_leaf(&tree, &view.id));
    }
    tree.unwrap_or_else(|| preset_tree(&terminals))
}

#[cfg(test)]
mod tests;
