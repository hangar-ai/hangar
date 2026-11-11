//! Where things run: which node a pane runs on, and how Hangar lays out a
//! node's repositories. The connections themselves are the host's (the
//! app's `nodes` package drives them over SSH).

use crate::state::types::{NodeConfig, Pane, Workspace};

/// The node pane `pane` runs on, when it is one sent from a local workspace.
pub fn pane_node(pane: &Pane) -> Option<String> {
    pane.node
        .as_deref()
        .map(str::trim)
        .filter(|node| !node.is_empty())
        .map(str::to_string)
}

/// Pane `pane` runs on node `node`.
pub fn set_pane_node(pane: &mut Pane, node: &str) {
    pane.node = Some(node.to_string());
}

/// The node pane `pane` of `ws` runs on: its own, else its workspace's.
pub fn node_of(ws: &Workspace, pane: &Pane) -> Option<String> {
    pane_node(pane).or_else(|| ws.node.clone())
}

/// The label of a node: its name, else its SSH target.
pub fn label_of(node: &NodeConfig) -> String {
    if node.label.trim().is_empty() {
        node.ssh_target.trim().to_string()
    } else {
        node.label.trim().to_string()
    }
}

/// The node's repository and the worktree root a worker's folder there is
/// in: `<repo>-worktrees/<folder>[/<subfolder>]` gives `(<repo>,
/// <repo>-worktrees/<folder>)`. `None` for a folder Hangar did not lay out.
pub fn repo_of(folder: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = folder.split('/').collect();
    let at = parts
        .iter()
        .enumerate()
        .rev()
        .find(|(index, part)| {
            *index > 0 && part.len() > "-worktrees".len() && part.ends_with("-worktrees")
        })
        .map(|(index, _)| index)?;
    if at + 1 >= parts.len() || parts[at + 1].is_empty() {
        return None;
    }
    let mut repo: Vec<&str> = parts[..at].to_vec();
    let name = parts[at].strip_suffix("-worktrees")?;
    repo.push(name);
    Some((repo.join("/"), parts[..=at + 1].join("/")))
}
