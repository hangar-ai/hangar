//! Core's events (hangar-core's host.rs) as the JSON `GET /node/events`
//! sends: `{"event": <the name the window listened to>, …}`, with what the
//! event carries beside it.
#![cfg(target_os = "linux")]

use hangar_core::Event;
use serde_json::{json, Value};

pub fn to_json(event: &Event) -> Value {
    let name = event.name();
    match event {
        Event::BoardChanged { cwd } => json!({ "event": name, "cwd": cwd }),
        Event::BridgeEvent { pane_id, event } => {
            json!({ "event": name, "paneId": pane_id, "payload": event })
        }
        // The id beside the payload: what `POST /node/control/{id}/reply`
        // answers (control.rs).
        Event::ControlRequest(request) | Event::ControlInternal(request) => {
            json!({ "event": name, "id": request.id, "payload": request })
        }
        Event::BridgeQueue(queue) => json!({ "event": name, "payload": queue }),
        Event::GitCloneProgress(progress) => json!({ "event": name, "payload": progress }),
        // Where every delivery under way stands and how the one that ended
        // went, spelled out: core's type is not one that serialises.
        Event::DeliveryStages(stages) => json!({
            "event": name,
            "payload": {
                "running": stages.running,
                "ended": stages.ended.as_ref().map(|ended| {
                    json!({ "taskId": ended.task_id, "outcome": ended.outcome })
                }),
            },
        }),
        Event::MemoryChanged
        | Event::RemoteChanged
        | Event::KitsChanged
        | Event::AccountsChanged => json!({ "event": name }),
        // The desktop-only events, there when another crate of the
        // workspace builds core with `desktop`: a node never sends them.
        #[allow(unreachable_patterns)]
        _ => json!({ "event": name }),
    }
}

/// The state of the workspaces the node runs itself changed, and stands at
/// `revision` (`/node/own/state` says what it holds). Not one of core's
/// events: their host is the node's.
pub fn own_changed(revision: u64) -> Value {
    json!({ "event": "own:changed", "revision": revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_carry_their_name_and_payload() {
        assert_eq!(
            to_json(&Event::BoardChanged { cwd: "/w".into() }),
            json!({ "event": "board:changed", "cwd": "/w" })
        );
        assert_eq!(
            to_json(&Event::MemoryChanged),
            json!({ "event": "memory:changed" })
        );
        assert_eq!(
            to_json(&Event::BridgeEvent {
                pane_id: "p1".into(),
                event: json!({ "kind": "stop" })
            }),
            json!({ "event": "bridge:event", "paneId": "p1", "payload": { "kind": "stop" } })
        );
        let request = hangar_core::control::ControlRequest {
            id: "r1".into(),
            op: "worker_spawn".into(),
            args: json!({ "task": "t1" }),
        };
        assert_eq!(
            to_json(&Event::ControlRequest(request.clone())),
            json!({
                "event": Event::ControlRequest(request.clone()).name(),
                "id": "r1",
                "payload": { "id": "r1", "op": "worker_spawn", "args": { "task": "t1" } }
            })
        );
        assert_eq!(to_json(&Event::ControlInternal(request))["id"], "r1");
    }

    /// The queue of a pane and the deliveries' stages go with what they
    /// say, not as a bare name: a desktop that mirrors the node's own
    /// workspaces shows both.
    #[test]
    fn the_queue_and_the_deliveries_stages_carry_their_payload() {
        let queue = hangar_core::bridge::QueueChanged {
            pane_id: "p1".into(),
            pending: vec![hangar_core::bridge::Queued {
                id: "q1".into(),
                text: "next".into(),
                from: Some("Ava".into()),
            }],
        };
        assert_eq!(
            to_json(&Event::BridgeQueue(queue)),
            json!({
                "event": "bridge:queue",
                "payload": {
                    "paneId": "p1",
                    "pending": [{ "id": "q1", "text": "next", "from": "Ava" }]
                }
            })
        );
        let stages = hangar_core::delivery::Stages {
            running: vec![hangar_core::delivery::Running {
                task_id: "t1".into(),
                queue: "/w".into(),
                stage: "checking".into(),
                since: 7,
                batch: None,
            }],
            ended: Some(hangar_core::delivery::StageEnd {
                task_id: "t0".into(),
                outcome: hangar_core::delivery::Kind::Landed,
            }),
        };
        assert_eq!(
            to_json(&Event::DeliveryStages(stages)),
            json!({
                "event": "delivery:stages",
                "payload": {
                    "running": [
                        { "taskId": "t1", "queue": "/w", "stage": "checking", "since": 7 }
                    ],
                    "ended": { "taskId": "t0", "outcome": "landed" }
                }
            })
        );
    }
}
