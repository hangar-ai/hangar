//! The persisted state (`types.rs`) and the one door through which it
//! changes (`store.rs`): what `state.json` holds, whatever draws it.
//!
//! `Store` keeps the state and what goes with saving it; the app's `Model`
//! holds one beside the window's own state, and a Hangar with no window will
//! hold one alone. The state is private: it is read with `state()` and
//! written with `apply(StoreAction)`, so a save and a snapshot never miss a
//! change.

pub mod folders;
pub mod hosted;
pub mod layout;
mod lenient;
pub mod pane_names;
pub mod persist;
pub mod roles;
pub mod snapshot;
pub mod store;
pub mod types;
pub mod unread;

#[cfg(test)]
mod roundtrip_tests;

use std::sync::Arc;

pub use lenient::{app_state as read_state, read as read_state_keeping, read_checked};
pub use store::{
    CommandScope, FolderPatch, PanePatch, Patch, Removed, Respawn, SavedCommandDraft,
    SettingsPatch, StoreAction, StoreOutcome, WorkspaceDraft, WorkspacePatch,
};
pub use types::{
    AgentId, AppState, Pane, PaneId, Settings, SplitDir, SplitNode, ViewPane, Workspace, WsId,
};

pub use snapshot::Snapshot;
// What the engines name at `crate::state::*` beside the persisted state, as
// the app does.
pub use crate::live::{AgentActivity, PaneActivity};

/// What the boot read from `state.json`: the state, and the values it held
/// that no field could read (`Store::hydrate_read`).
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub state: AppState,
    pub kept: Vec<unread::Kept>,
}

/// The persisted state and what saving it needs.
#[derive(Debug, Default)]
pub struct Store {
    /// `state.json`'s content. An `Arc` so a snapshot shares it until the
    /// next write (`Arc::make_mut` in store.rs).
    state: Arc<AppState>,
    /// Bumped by every store action that changed `state`: what the save and
    /// the snapshot follow.
    revision: u64,
    /// Whether the store action being applied wrote `state` (`state_mut`):
    /// how `apply` tells a change without keeping a copy to compare.
    wrote: bool,
    /// What `state.json` held that no field could read, written back on
    /// every save (unread.rs).
    unread: unread::Unread,
    /// Respawns not yet announced.
    respawned: Vec<Respawn>,
    /// Workspaces removed and not yet cleaned up after (their delivery
    /// check worktree).
    removed: Vec<Removed>,
}

impl Store {
    /// The persisted state, to read.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The persisted state, shared (for a snapshot or a save off the thread
    /// that writes it).
    pub fn state_arc(&self) -> Arc<AppState> {
        self.state.clone()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The JSON a save writes: the state, with what `state.json` held that
    /// this version could not read put back where it was.
    pub fn to_json(&self) -> serde_json::Value {
        persist::to_json(&self.state, &self.unread)
    }

    /// What a save writes, to be turned into JSON off the thread that
    /// writes the state (`persist::Save::into_json`). The values set aside at
    /// load that have since been written over are forgotten for good here,
    /// once per save rather than after every store action: that needs the
    /// state as JSON, which only a file written by another version (`unread`
    /// not empty) pays for.
    pub fn take_save(&mut self) -> persist::Save {
        if !self.unread.is_empty() {
            let now = serde_json::to_value(&*self.state).unwrap_or_default();
            self.unread.prune(&now);
        }
        persist::Save {
            state: self.state_arc(),
            unread: self.unread.clone(),
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.state.settings
    }

    /// A copy to try an action on, which is thrown away: the same state,
    /// shared until the action writes it, and nothing waiting to be
    /// announced or saved. What an action would change is read from it
    /// before the action is applied for good.
    pub fn trial(&self) -> Store {
        Store {
            state: self.state.clone(),
            ..Store::default()
        }
    }

    /// The respawns not yet taken (`take_respawns`), oldest first: what
    /// telling a restarted pane from one closed and another opened needs
    /// (`hosted::diff`).
    pub fn respawns(&self) -> &[Respawn] {
        &self.respawned
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.state.workspaces.iter().find(|ws| ws.id == id)
    }
}
