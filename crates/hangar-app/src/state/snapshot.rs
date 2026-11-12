//! What the engines read: the model as it stood after the last update.
//!
//! The TS called `snapshot()` from ~30 modules, off any render (inventory
//! §1.3). Here the control, orchestration and mobile engines run on tokio,
//! away from `Model`, and read an `Arc<Snapshot>` the shell republishes after
//! every update that changed something (`Services::snapshot`, rule 6 of
//! PACKAGES.md). Cheap to take: the persisted state is itself an `Arc`
//! shared with the model until the model next writes it.

//! The snapshot itself is hangar-engine's (`hangar_engine::state::snapshot`);
//! taking it from the `Model` is the app's.

use std::sync::{Arc, Mutex};

use super::Model;

pub use hangar_engine::state::snapshot::Snapshot;

/// The snapshot of `model` now: `asleep` and `activity` from P4's
/// read-model, `delivering` from P18's.
pub fn snapshot_of(model: &Model) -> Snapshot {
    Snapshot {
        state: model.state_arc(),
        opened: model.ui.opened_ids.iter().cloned().collect(),
        asleep: model.live.panes_logic.asleep_ids(),
        activity: model.live.panes_logic.activity_map().clone(),
        delivering: model.live.control.delivering(),
        hydrated: model.live.hydrated,
    }
}

/// Whether the model's live parts moved on since this snapshot was taken,
/// though nothing to save did: a delivery began or ended, an agent
/// started waiting, a workspace went to sleep or was opened. The engines
/// and hibernation read those here, so a stale one is published again.
/// Run after every message: compares in place, cheapest first, and
/// allocates nothing while no delivery runs.
pub fn is_stale(snapshot: &Snapshot, model: &Model) -> bool {
    let opened = &model.ui.opened_ids;
    let asleep = model.live.panes_logic.asleep();
    snapshot.opened.len() != opened.len()
        || opened.iter().any(|id| !snapshot.opened.contains(id))
        || snapshot.asleep.len() != asleep.len()
        || asleep.keys().any(|id| !snapshot.asleep.contains(id))
        || snapshot.activity != *model.live.panes_logic.activity_map()
        || snapshot.delivering != model.live.control.delivering()
}

/// `snapshot` without the workspaces a node runs itself: what the engines
/// of this machine act on (`Snapshot::without_hosted`). Made once for each
/// snapshot the app publishes, since they read it many times a round.
pub fn own(snapshot: Arc<Snapshot>) -> Arc<Snapshot> {
    static LAST: Mutex<Option<(Arc<Snapshot>, Arc<Snapshot>)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((from, own)) = last.as_ref()
        && Arc::ptr_eq(from, &snapshot)
    {
        return own.clone();
    }
    let own = Snapshot::without_hosted(&snapshot);
    *last = Some((snapshot, own.clone()));
    own
}
