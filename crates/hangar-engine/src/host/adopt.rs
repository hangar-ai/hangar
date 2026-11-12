//! A workspace a desktop hands to this host, and what its mirrors ask of it
//! afterwards (`state::hosted`): checked whole before anything is changed,
//! so that a refusal leaves the state, and the shells, as they were.

use std::collections::HashSet;
use std::path::Path;

use super::Model;
use crate::state::StoreAction;
use crate::state::hosted::{self, HostedOp};
use crate::state::layout::MAX_PANES;
use crate::state::pane_names::pick_pane_name;
use crate::state::types::{AppState, Pane, Workspace};

/// The longest id taken: a UUID is 36 characters, the older formats less.
const ID_MAX: usize = 128;

/// An id as the desktop makes them: it names a shell, goes in a URL and in
/// a variable of the pane's environment, so no space, no slash, nothing a
/// terminal would act on.
fn sane_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_MAX
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// A workspace's folder: absolute, and a folder of this machine.
fn folder_here(cwd: &str) -> Result<(), String> {
    let path = Path::new(cwd);
    if !path.is_absolute() {
        return Err(format!("{cwd}: a workspace's folder is an absolute path"));
    }
    if !path.is_dir() {
        return Err(format!("{cwd} is not a folder here"));
    }
    Ok(())
}

fn pane_exists(state: &AppState, id: &str) -> bool {
    state.workspaces.iter().any(|ws| ws.has_leaf(id))
}

/// A pane this host can run: an id, a folder of its own that is absolute
/// when it has one (a worker's worktree that has since gone fails that
/// pane's start, not the whole workspace), and no other machine to run on.
fn pane_fits(pane: &Pane) -> Result<(), String> {
    if !sane_id(&pane.id) {
        return Err(format!("{:?} is not a pane's id", pane.id));
    }
    if let Some(cwd) = pane.cwd.as_deref().filter(|cwd| !cwd.is_empty())
        && !Path::new(cwd).is_absolute()
    {
        return Err(format!("{cwd}: a pane's folder is an absolute path"));
    }
    if pane.node.is_some() {
        return Err(format!(
            "pane {} runs on another machine: a node runs its own panes only",
            pane.name
        ));
    }
    Ok(())
}

/// Whether `ws` can be adopted into `state`: `Ok(false)` when a workspace
/// of that id is there already.
pub(super) fn check(state: &AppState, ws: &Workspace) -> Result<bool, String> {
    if !sane_id(&ws.id) {
        return Err(format!("{:?} is not a workspace's id", ws.id));
    }
    if state.workspaces.iter().any(|own| own.id == ws.id) {
        return Ok(false);
    }
    folder_here(&ws.cwd)?;
    if ws.panes.is_empty() || ws.panes.len() > MAX_PANES {
        return Err(format!(
            "a workspace has between 1 and {MAX_PANES} panes, not {}",
            ws.panes.len()
        ));
    }
    let mut seen = HashSet::new();
    for pane in &ws.panes {
        pane_fits(pane)?;
        if !seen.insert(pane.id.as_str()) || pane_exists(state, &pane.id) {
            return Err(format!("pane id {} is used twice", pane.id));
        }
    }
    Ok(true)
}

/// `ws` as this host keeps it: without what was the desktop's presentation
/// of it, the leaves of its views out of its tree, and every pane under a
/// name none of `taken` (the names in use here) is.
pub(super) fn stripped(ws: Workspace, taken: &[String]) -> Workspace {
    let tree = hosted::terminals_tree(&ws);
    let mut ws = Workspace {
        tree: Some(tree),
        node: None,
        hosted: None,
        folder_id: None,
        detached: None,
        window_bounds: None,
        view_panes: Vec::new(),
        chat_view: None,
        kits: None,
        ..ws
    };
    let mut used: HashSet<String> = taken.iter().cloned().collect();
    for pane in &mut ws.panes {
        if pane.name.trim().is_empty() || used.contains(&pane.name) {
            pane.name = pick_pane_name(&used);
        }
        used.insert(pane.name.clone());
    }
    ws
}

/// Whether every one of `ops` can be applied to workspace `ws` of `state`.
pub(super) fn check_ops(state: &AppState, ws: &str, ops: &[HostedOp]) -> Result<(), String> {
    let Some(workspace) = state.workspaces.iter().find(|own| own.id == ws) else {
        return Err(format!("no workspace {ws} here"));
    };
    // An id this workspace already has is an op applied again; one another
    // workspace has is a mistake.
    let free = |id: &str| workspace.has_leaf(id) || !pane_exists(state, id);
    for op in ops {
        match op {
            HostedOp::PutPane { pane } => {
                pane_fits(pane)?;
                if !free(&pane.id) {
                    return Err(format!("pane id {} is used twice", pane.id));
                }
            }
            HostedOp::RespawnPane { to, patch, .. } => {
                if !sane_id(to) || !free(to) {
                    return Err(format!("{to:?} cannot be a pane's new id"));
                }
                if patch.node.as_ref().is_some_and(Option::is_some) {
                    return Err("a node runs its own panes only".into());
                }
            }
            HostedOp::PatchWorkspace { patch } => {
                if let Some(cwd) = &patch.cwd {
                    folder_here(cwd)?;
                }
            }
            HostedOp::PatchPane { patch, .. } => {
                if patch.node.as_ref().is_some_and(Option::is_some) {
                    return Err("a node runs its own panes only".into());
                }
            }
            HostedOp::ClosePane { .. } | HostedOp::RemoveWorkspace => {}
        }
    }
    Ok(())
}

/// One op applied to workspace `ws`, with what the host does around the
/// same change when an agent makes it (`writes.rs`): a workspace whose
/// panes changed is laid out again, a pane restarted keeps the session it
/// was owed, a workspace removed is no longer up.
pub(super) fn carry_out(model: &mut Model, ws: &str, op: HostedOp) {
    let count = |model: &Model| model.store.workspace(ws).map_or(0, |w| w.panes.len());
    let before = count(model);
    let restart = match &op {
        HostedOp::RespawnPane { from, to, patch } if patch.session_id != Some(None) => {
            Some((from.clone(), to.clone()))
        }
        _ => None,
    };
    let outcome = hosted::apply_op(&mut model.store, ws, op);
    if let Some((from, to)) = restart.filter(|_| outcome.pane.is_some()) {
        model.panes.carry_resume(&from, &to);
    }
    if model.store.workspace(ws).is_none() {
        model.opened.remove(ws);
        model.runs.remove(ws);
    } else if count(model) != before {
        model.store.apply(StoreAction::ArrangePanes(ws.to_string()));
    }
}
