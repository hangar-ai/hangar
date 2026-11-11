//! The last delivery check of each workspace, for the orchestrator's next
//! briefing: the process's, as the deliveries are.

use std::sync::Mutex;

/// What the workspace's last delivery check cost and said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckCost {
    pub seconds: u64,
    pub passed: bool,
}

/// The last delivery check of each workspace, by id: what it took and said.
fn checks() -> &'static Mutex<std::collections::HashMap<String, CheckCost>> {
    static CHECKS: std::sync::OnceLock<Mutex<std::collections::HashMap<String, CheckCost>>> =
        std::sync::OnceLock::new();
    CHECKS.get_or_init(Default::default)
}

/// Remembers how long a workspace's delivery check ran and what it said, for
/// the orchestrator's next briefing. A check that did not run is not recorded.
pub fn record_check(workspace_id: &str, seconds: u64, passed: bool) {
    checks()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(workspace_id.to_string(), CheckCost { seconds, passed });
}

/// The workspace's last check, if one ran since the app started.
pub fn last_check(workspace_id: &str) -> Option<CheckCost> {
    checks()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(workspace_id)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_check_of_a_workspace_is_the_one_kept() {
        assert_eq!(last_check("ws-never-checked"), None);
        record_check("ws-checked", 700, true);
        record_check("ws-checked", 95, false);
        assert_eq!(
            last_check("ws-checked"),
            Some(CheckCost {
                seconds: 95,
                passed: false
            })
        );
        assert_eq!(last_check("ws-other"), None);
    }
}
