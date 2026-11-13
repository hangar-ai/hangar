//! What the person asks of a workspace its node runs that takes the node's
//! engines: a delivery landed, its workers stopped. It is the node that is
//! asked, and nothing of it is run by this computer.

use serde_json::json;

use super::desk;
use super::forward_tests::{connected, driven, outputs, reach, serve};
use super::testing::listen;
use crate::message::{Intent, Message};
use crate::needs_you;
use crate::shell::Notice;

fn land() -> Intent {
    Intent::Land {
        ws: "o1".into(),
        task: "t1".into(),
    }
}

/// « Merge » on a mirror asks its node, again while the node says the
/// landing still runs, and says it merged as for a workspace of this
/// computer.
#[tokio::test]
async fn a_delivery_of_a_mirror_is_landed_by_its_node() {
    let (listener, client) = listen().await;
    let mut m = connected();
    reach(&mut m, "gpu", &client);
    let server = serve(
        listener,
        vec![
            (200, json!({ "landed": false, "stage": "testing" })),
            (200, json!({ "landed": true, "target": "main" })),
        ],
    );
    let landing = crate::control::landing_on_its_node(&m, &land()).expect("the node's to land");
    let said = outputs(landing).await;
    let [Message::Intent(Intent::Notice(Notice::Agent(notice)))] = said.as_slice() else {
        panic!("a notice alone, not {said:?}");
    };
    assert_eq!(notice, "Merged: t1");
    let asked = server.await.unwrap();
    assert_eq!(asked.len(), 2);
    for (line, body) in asked {
        assert_eq!(line, "POST /node/own/desk");
        assert_eq!(
            body,
            json!({ "op": "remote_land", "args": { "task": "t1" } })
        );
    }
}

/// What the node answers when it does not land it is the row's word and
/// the notice's, as a landing refused here is.
#[tokio::test]
async fn a_landing_its_node_refuses_says_why_on_its_row() {
    let (listener, client) = listen().await;
    let mut m = connected();
    reach(&mut m, "gpu", &client);
    let server = serve(
        listener,
        vec![(
            400,
            json!({ "error": "Nothing of \"Export\" waits to land any more." }),
        )],
    );
    let landing = crate::control::landing_on_its_node(&m, &land()).expect("the node's to land");
    let said = outputs(landing).await;
    let [
        Message::Intent(Intent::Notice(Notice::Problem(notice))),
        Message::NeedsYou(needs_you::Msg::LandFailed { key, error }),
    ] = said.as_slice()
    else {
        panic!("a notice and the row's failure, not {said:?}");
    };
    assert_eq!(key, "task:/srv/nightly:t1:ready-to-land");
    assert_eq!(error, "Nothing of \"Export\" waits to land any more.");
    assert!(notice.contains(error.as_str()), "{notice}");
    assert_eq!(server.await.unwrap().len(), 1);
}

/// With its node away nothing is landed, from anywhere, and the person is
/// told which machine is missing.
#[tokio::test]
async fn a_landing_while_its_node_is_away_is_not_run_from_here() {
    let mut m = connected();
    m.live.nodes.statuses.remove("gpu");
    let landing = crate::control::landing_on_its_node(&m, &land()).expect("the node's to land");
    let said = outputs(landing).await;
    let Some(Message::Intent(Intent::Notice(Notice::Problem(notice)))) = said.first() else {
        panic!("a notice, not {said:?}");
    };
    assert_eq!(notice, "Could not merge: GPU box is not connected");
}

/// Only a workspace its node runs is the node's to land: one of this
/// computer, or one it drives on a node, is landed by the engine here.
#[test]
fn a_workspace_this_computer_runs_is_not_landed_by_a_node() {
    let mut m = connected();
    driven(&mut m);
    for ws in ["w1", "a1"] {
        let intent = Intent::Land {
            ws: ws.into(),
            task: "t1".into(),
        };
        assert!(crate::control::landing_on_its_node(&m, &intent).is_none());
    }
    assert!(crate::control::landing_on_its_node(&m, &Intent::CloseOverlay).is_none());
}

/// « Stop the workers » of a mirror is the node's to do: it says how many
/// it stopped, or why it could not.
#[tokio::test]
async fn the_workers_of_a_mirror_are_stopped_by_its_node() {
    let (listener, client) = listen().await;
    let mut m = connected();
    reach(&mut m, "gpu", &client);
    let server = serve(
        listener,
        vec![
            (200, json!({ "stopped": 2 })),
            (400, json!({ "error": "the node is still starting" })),
        ],
    );
    let ws = m.workspace("o1").unwrap().clone();
    let notice = |said: Vec<Message>| match said.as_slice() {
        [Message::Intent(Intent::Notice(notice))] => notice.clone(),
        other => panic!("a notice, not {other:?}"),
    };
    assert_eq!(
        notice(outputs(desk::stop_all(&m, &ws)).await),
        Notice::Agent("2 workers stopped in nightly".into())
    );
    assert_eq!(
        notice(outputs(desk::stop_all(&m, &ws)).await),
        Notice::Problem(
            "The workers of nightly could not be stopped: the node is still starting.".into()
        )
    );
    let asked = server.await.unwrap();
    assert_eq!(asked[0].0, "POST /node/own/desk");
    assert_eq!(
        asked[0].1,
        json!({ "op": "stop_workers", "args": { "workspace": "o1" } })
    );
}

/// A phone paired with this computer asks about a delivery: when the task
/// is held by a worker of a workspace its node runs, the call is the
/// node's to answer, never this computer's desk.
#[test]
fn the_phone_s_call_about_a_mirror_s_delivery_is_its_node_s_to_answer() {
    use crate::state::types::PaneRole;
    let mut m = connected();
    driven(&mut m);
    let mut state = m.state().clone();
    let held = |ws: &mut crate::state::types::Workspace, pane: usize, task: &str| {
        ws.panes[pane].role = Some(PaneRole::Worker);
        ws.panes[pane].task_id = Some(task.into());
    };
    for ws in &mut state.workspaces {
        match ws.id.as_str() {
            "o1" => held(ws, 1, "t-theirs"),
            "a1" => held(ws, 1, "t-driven"),
            _ => held(ws, 0, "t-here"),
        }
    }
    let args = |task: &str| json!({ "task": task });
    for op in [
        "remote_delivery",
        "remote_delivery_file",
        "remote_land",
        "remote_rework",
    ] {
        assert_eq!(
            desk::node_for(op, &args("t-theirs"), &state).as_deref(),
            Some("gpu"),
            "{op}"
        );
        assert_eq!(desk::node_for(op, &args("t-driven"), &state), None);
        assert_eq!(desk::node_for(op, &args("t-here"), &state), None);
        assert_eq!(desk::node_for(op, &args("t-nobody"), &state), None);
    }
    assert_eq!(
        desk::node_for("remote_state", &args("t-theirs"), &state),
        None
    );
}

/// A call handed to a node that is not connected is refused in words, and
/// nothing is run here in its place.
#[tokio::test]
async fn a_call_for_a_node_that_is_away_is_refused() {
    let refused = desk::forward("no-such-node", "remote_land", json!({ "task": "t1" })).await;
    assert_eq!(
        refused.unwrap_err(),
        "The machine that runs this workspace is not connected: nothing was done."
    );
}
