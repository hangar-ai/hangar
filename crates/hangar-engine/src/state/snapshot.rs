//! What the engines read: the state and the live read-models as they stood after the last update.
//!
//! The TS called `snapshot()` from ~30 modules, off any render (inventory
//! §1.3). Here the control, orchestration and mobile engines run on tokio,
//! away from `Model`, and read an `Arc<Snapshot>` the shell republishes after
//! every update that changed something (`Services::snapshot`, rule 6 of
//! PACKAGES.md). Cheap to take: the persisted state is itself an `Arc`
//! shared with the model until the model next writes it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::live::{Delivering, PaneActivity};
use crate::state::types::{AppState, PaneId, WsId};

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub state: Arc<AppState>,
    /// Workspaces opened in this run (their panes are alive).
    pub opened: HashSet<WsId>,
    /// Workspaces put to sleep (P4's hibernation).
    pub asleep: HashSet<WsId>,
    pub activity: HashMap<PaneId, PaneActivity>,
    pub delivering: Delivering,
    /// `Model.live.hydrated`: engines answer nothing before.
    pub hydrated: bool,
}

impl Snapshot {
    /// This snapshot without the workspaces a node runs itself
    /// (`Workspace::hosted`): what an engine of this machine acts on. Their
    /// node's own engines run those; one of this machine's reading them
    /// would act a second time (a worker closed twice, an event posted
    /// twice). The same snapshot when it holds none, which costs nothing.
    pub fn without_hosted(snapshot: &Arc<Snapshot>) -> Arc<Snapshot> {
        if !snapshot.state.workspaces.iter().any(|ws| ws.is_hosted()) {
            return snapshot.clone();
        }
        let hosted: HashSet<&WsId> = snapshot
            .state
            .workspaces
            .iter()
            .filter(|ws| ws.is_hosted())
            .map(|ws| &ws.id)
            .collect();
        let mut state = (*snapshot.state).clone();
        state.workspaces.retain(|ws| !ws.is_hosted());
        let active = state.active_workspace_id.as_ref();
        if active.is_some_and(|id| hosted.contains(id)) {
            state.active_workspace_id = None;
        }
        let kept = |id: &WsId| !hosted.contains(id);
        Arc::new(Snapshot {
            state: Arc::new(state),
            opened: snapshot
                .opened
                .iter()
                .filter(|id| kept(id))
                .cloned()
                .collect(),
            asleep: snapshot
                .asleep
                .iter()
                .filter(|id| kept(id))
                .cloned()
                .collect(),
            activity: snapshot.activity.clone(),
            delivering: snapshot
                .delivering
                .iter()
                .filter(|(id, _)| kept(id))
                .map(|(id, runs)| (id.clone(), runs.clone()))
                .collect(),
            hydrated: snapshot.hydrated,
        })
    }
}
