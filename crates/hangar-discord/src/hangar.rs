//! What the connector reaches of the Hangar it serves, whichever runs it:
//! the desktop app (its workspaces, local and on its nodes, the same ones
//! the phone reaches) or a node with no desktop (the workspaces it runs
//! itself, `hangar_engine::host`).
//!
//! The connector reads the snapshot and acts through the control port and
//! the desk, the chat view's and the phone's own calls; it follows core's
//! events (the mods' reports) and the orchestration's journal.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::broadcast;

use hangar_core::board::Task;
use hangar_engine::control::port::{ModelCmd, Port};
use hangar_engine::host::EngineHost;
use hangar_engine::mobile::engine::Mobile;
use hangar_engine::orchestration::events::JournalEntry;
use hangar_engine::state::types::{AgentId, Pane, PaneRole};
use hangar_engine::state::{PanePatch, Snapshot, WorkspaceDraft};

pub trait Hangar: Send + Sync + 'static {
    /// The control port: the snapshot, the model writes, the panes.
    fn port(&self) -> Port;

    /// The desk (`hangar_engine::mobile::desk`'s calls take it), once up.
    fn desk(&self) -> Option<Arc<Mobile>>;

    /// What happened lately in workspace `ws` (the orchestration's journal),
    /// oldest first.
    fn journal(&self, ws: &str) -> Vec<JournalEntry>;

    /// The workspace's board, as the orchestration last read it.
    fn tasks(&self, ws: &str) -> Vec<Task>;

    /// core's events from now on: the mods' reports.
    fn events(&self) -> broadcast::Receiver<hangar_core::Event>;

    /// The state and the read-models as they stand.
    fn snapshot(&self) -> Arc<Snapshot> {
        self.port().snapshot()
    }

    /// One bot can serve several Hangars on one server: a desktop and the
    /// nodes it drives, each in channels of its own. Every instance hears
    /// every event; each acts in the channels it made, and the one that
    /// leads answers in the others (a command typed in #general, a message
    /// before the pairing). The desktop leads; a node leads while no
    /// desktop drives it.
    fn leads(&self) -> bool {
        true
    }

    /// The channels the other Hangars of the same bot made: left to them,
    /// though this one leads.
    fn others(&self) -> HashSet<String> {
        HashSet::new()
    }
}

/// A node's own workspaces.
impl Hangar for EngineHost {
    fn port(&self) -> Port {
        EngineHost::port(self)
    }

    fn desk(&self) -> Option<Arc<Mobile>> {
        EngineHost::desk(self)
    }

    fn journal(&self, ws: &str) -> Vec<JournalEntry> {
        self.orchestration()
            .map(|runtime| runtime.journal(ws))
            .unwrap_or_default()
    }

    fn tasks(&self, ws: &str) -> Vec<Task> {
        self.orchestration()
            .map(|runtime| runtime.tasks(ws))
            .unwrap_or_default()
    }

    fn events(&self) -> broadcast::Receiver<hangar_core::Event> {
        self.core().subscribe()
    }

    fn snapshot(&self) -> Arc<Snapshot> {
        EngineHost::snapshot(self)
    }
}

/// A workspace in `folder` (absolute, existing where the Hangar runs), with
/// one Claude Code pane as its orchestrator, opened: `/workspace`. Through
/// the model writes any host applies, as an agent's `workspace_create`.
pub async fn add_workspace(port: &Port, name: &str, folder: &str) -> Result<String, String> {
    let folder = folder.trim();
    if !Path::new(folder).is_absolute() {
        return Err(format!("{folder}: give the folder's full path"));
    }
    if !port.dir_exists(folder).await {
        return Err(format!("{folder} is not a folder there"));
    }
    let name = match name.trim() {
        "" => Path::new(folder)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Workspace".into()),
        named => named.to_string(),
    };
    let made = port
        .apply(ModelCmd::CreateWorkspace {
            draft: WorkspaceDraft {
                name,
                cwd: folder.to_string(),
                layout: 1,
                agents: vec![AgentId::Claude],
                ..WorkspaceDraft::default()
            },
            activate: false,
        })
        .await?;
    let ws = made.workspace.ok_or("the workspace could not be made")?;
    let first = port
        .snapshot()
        .state
        .workspaces
        .iter()
        .find(|w| w.id == ws)
        .and_then(|w| w.panes.first().map(|p| p.id.clone()));
    if let Some(pane) = first {
        port.apply(ModelCmd::UpdatePane {
            ws: ws.clone(),
            pane,
            patch: PanePatch::new(|pane: &mut Pane| pane.role = Some(PaneRole::Orchestrator)),
        })
        .await?;
    }
    port.apply(ModelCmd::OpenWorkspace(ws.clone())).await?;
    Ok(ws)
}
