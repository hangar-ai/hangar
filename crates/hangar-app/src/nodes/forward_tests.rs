//! What the person changes on the mirror of a workspace its node runs:
//! applied here at once, told to the node in order, one request at a time
//! for each node; what the node refuses is read back from it. And a
//! workspace handed to its node, new or one this computer drove.

use iced::Task;
use iced::futures::StreamExt as _;
use iced_runtime::Action;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use super::hosted::{self, Handover, Job, Why};
use super::hosted_tests::{NOW, answered, mirrored, nightly, pane, theirs};
use super::model::{NodeClient, NodeInfo};
use super::testing::{TOKEN, answer, listen, read_request, up};
use super::{Ask, Msg, NodeStatus};
use crate::message::Message;
use crate::state::hosted::HostedOp;
use crate::state::types::{DropZone, Pane, PaneRole, Workspace};
use crate::state::{Model, Patch, StoreAction};

/// `nightly` of `gpu` mirrored, `gpu` connected.
fn connected() -> Model {
    let mut m = mirrored();
    up(&mut m, "gpu", "gpu-01");
    m
}

/// The ops waiting for `gpu`, each with the workspace it is for.
fn waiting(m: &Model) -> Vec<(String, HostedOp)> {
    m.live
        .nodes
        .outbox
        .waiting("gpu")
        .into_iter()
        .flat_map(|job| match job {
            Job::Ops(edit) => edit
                .ops
                .into_iter()
                .map(|op| (edit.workspace.clone(), op))
                .collect::<Vec<_>>(),
            Job::Adopt { .. } => panic!("an edit, not a workspace handed over"),
        })
        .collect()
}

/// The ops one action queued, the line emptied for the next.
fn ops_of(m: &mut Model, action: StoreAction) -> Vec<HostedOp> {
    let before = m.live.nodes.outbox.waiting("gpu").len();
    m.apply(action);
    assert!(m.live.nodes.refused.is_none());
    waiting(m)
        .into_iter()
        .skip(before)
        .map(|(ws, op)| {
            assert_eq!(ws, "o1");
            op
        })
        .collect()
}

fn rename(to: &str) -> StoreAction {
    let to = to.to_owned();
    StoreAction::UpdateWorkspace(
        "o1".into(),
        Patch::new(move |ws: &mut Workspace| ws.name = to.clone()),
    )
}

#[test]
fn each_edit_of_a_mirror_is_applied_here_and_queued_for_its_node() {
    let mut m = connected();

    let ops = ops_of(&mut m, rename("nightly builds"));
    assert_eq!(m.workspace("o1").unwrap().name, "nightly builds");
    let [HostedOp::PatchWorkspace { patch }] = ops.as_slice() else {
        panic!("a rename: {ops:?}");
    };
    assert_eq!(patch.name.as_deref(), Some("nightly builds"));
    assert_eq!(patch.cwd, None, "only what changed is named");

    // A new pane goes with the id it was given here, which the mirror's
    // tree and focus already use.
    let made = m.apply(StoreAction::AddPane {
        ws: "o1".into(),
        agent: None,
        near: Some("op1".into()),
        dir: None,
        cwd: None,
        name: None,
        init: None,
    });
    let added = made.pane.expect("the pane is made here at once");
    let ops: Vec<HostedOp> = waiting(&m).into_iter().map(|(_, op)| op).collect();
    let Some(HostedOp::PutPane { pane: put }) = ops.last() else {
        panic!("a pane put: {ops:?}");
    };
    assert_eq!(put.id, added);
    assert!(m.workspace("o1").unwrap().has_leaf(&added));

    let ops = ops_of(
        &mut m,
        StoreAction::ClosePane {
            ws: "o1".into(),
            pane: added.clone(),
        },
    );
    assert_eq!(ops, [HostedOp::ClosePane { pane: added }]);

    // A restart is one op, from the old id to the one made here.
    let restarted = m.apply(StoreAction::RespawnPane {
        ws: "o1".into(),
        pane: "op2".into(),
        patch: Patch::new(|pane: &mut Pane| pane.session_id = Some("s9".into())),
    });
    let to = restarted.pane.expect("restarted here at once");
    let Some((
        _,
        HostedOp::RespawnPane {
            from,
            to: sent,
            patch,
        },
    )) = waiting(&m).pop()
    else {
        panic!("a respawn");
    };
    assert_eq!((from.as_str(), &sent), ("op2", &to));
    assert_eq!(patch.session_id, Some(Some("s9".into())));
    assert_eq!(m.take_respawns().len(), 1, "announced here as any restart");

    // A preset for more panes than there are puts the ones it makes.
    let ops = ops_of(
        &mut m,
        StoreAction::ApplyPreset {
            ws: "o1".into(),
            count: 3,
            model_pane: None,
        },
    );
    let put = ops
        .iter()
        .filter(|op| matches!(op, HostedOp::PutPane { .. }))
        .count();
    assert_eq!(put, 1, "{ops:?}");
    assert_eq!(m.workspace("o1").unwrap().panes.len(), 3);

    let ops = ops_of(&mut m, StoreAction::RemoveWorkspace("o1".into()));
    assert_eq!(ops, [HostedOp::RemoveWorkspace]);
    assert!(m.workspace("o1").is_none());
    // Its folder is the node's: nothing of this machine is cleaned up.
    let removed = m.take_removed();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].node.as_deref(), Some("gpu"));
}

#[test]
fn the_mirror_s_own_presentation_queues_nothing() {
    let mut m = connected();
    let presentation = [
        StoreAction::MoveWorkspace {
            id: "o1".into(),
            folder: Some("clients".into()),
            before: None,
        },
        StoreAction::MovePane {
            ws: "o1".into(),
            drag: "op2".into(),
            target: "op1".into(),
            zone: DropZone::Top,
        },
        StoreAction::TidyLayout("o1".into()),
        StoreAction::SetChatView {
            ws: "o1".into(),
            on: true,
        },
        StoreAction::SetActiveWorkspace(Some("o1".into())),
    ];
    for action in presentation {
        let said = format!("{action:?}");
        m.apply(action);
        assert!(!m.live.nodes.outbox.busy("gpu"), "{said}");
    }
    // A removal on this computer is still cleaned up after here.
    m.apply(StoreAction::RemoveWorkspace("w1".into()));
    assert_eq!(m.take_removed()[0].node, None);
}

/// What an agent of this machine asks through the control port is not the
/// person's: refused, node connected or not.
#[test]
fn a_change_nobody_here_asked_for_is_refused() {
    let mut m = connected();
    let outcome = m.apply_unattended(rename("mine"));
    assert!(!outcome.changed);
    assert_eq!(m.workspace("o1").unwrap().name, "nightly");
    let (refused, why) = m.live.nodes.refused.take().expect("refused");
    assert_eq!(why, Why::Unattended);
    assert_eq!(
        hosted::refusal_words(&m, &refused, why),
        "nightly runs on GPU box: only you can change it, from this window."
    );
    assert!(!m.live.nodes.outbox.busy("gpu"));
    // Its own workspaces are its to change as before.
    assert!(
        m.apply_unattended(StoreAction::UpdateWorkspace(
            "w1".into(),
            Patch::new(|ws: &mut Workspace| ws.name = "site 2".into()),
        ))
        .changed
    );
}

/// The messages a task gives, run to its end.
async fn outputs(task: Task<Message>) -> Vec<Message> {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    stream
        .filter_map(|action| async move {
            match action {
                Action::Output(message) => Some(message),
                _ => None,
            }
        })
        .collect()
        .await
}

/// Node `id` connected through `client`, a fake node's.
fn reach(m: &mut Model, id: &str, client: &NodeClient) {
    let port = client.base_url.rsplit(':').next().unwrap().parse().unwrap();
    m.live.nodes.statuses.insert(
        id.into(),
        NodeStatus::Connected {
            info: NodeInfo::default(),
            local_port: port,
            token: TOKEN.into(),
        },
    );
}

/// A fake node that answers each request with the next `(status, body)`;
/// what it was asked, once all are answered: each request's line and body.
fn serve(
    listener: TcpListener,
    answers: Vec<(u16, Value)>,
) -> tokio::task::JoinHandle<Vec<(String, Value)>> {
    tokio::spawn(async move {
        let mut asked = Vec::new();
        for (status, body) in answers {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let line = request.line().trim_end_matches(" HTTP/1.1").to_owned();
            let sent = serde_json::from_str(&request.body).unwrap_or(Value::Null);
            asked.push((line, sent));
            answer(&mut stream, status, &body.to_string()).await;
        }
        asked
    })
}

/// What waits is sent until every line is empty: each task run to its end
/// and its answer taken in, as the app's loop does. The notices shown on
/// the way.
async fn deliver(m: &mut Model) -> Vec<String> {
    let mut notices = Vec::new();
    let mut tasks = vec![hosted::send_next(m)];
    while let Some(task) = tasks.pop() {
        for message in outputs(task).await {
            match message {
                Message::Nodes(Msg::Sent { id, result }) => {
                    tasks.push(hosted::sent(m, &id, result));
                    tasks.push(hosted::send_next(m));
                }
                Message::Nodes(Msg::OwnState { id, asked, state }) => {
                    if let Some(state) = state {
                        hosted::take_in_asked(m, &id, asked, *state, NOW);
                    }
                }
                Message::Intent(crate::message::Intent::Notice(crate::shell::Notice::Problem(
                    text,
                ))) => notices.push(text),
                other => panic!("not expected: {other:?}"),
            }
        }
    }
    notices
}

fn own_state(revision: u64, workspaces: &[Workspace]) -> Value {
    json!({ "boot": "b1", "revision": revision, "workspaces": workspaces, "activity": {} })
}

/// Three edits made in a row reach the node as three requests, one after
/// the other in the order they were made, and the node is read once all
/// are answered.
#[tokio::test]
async fn a_node_is_told_the_changes_in_the_order_they_were_made() {
    let (listener, client) = listen().await;
    let mut m = connected();
    reach(&mut m, "gpu", &client);
    m.apply(rename("one"));
    m.apply(StoreAction::UpdatePane {
        ws: "o1".into(),
        pane: "op1".into(),
        patch: Patch::new(|pane: &mut Pane| pane.model = Some("opus".into())),
    });
    m.apply(rename("two"));

    // One request on its way for the node, the others behind it.
    let first = hosted::send_next(&mut m);
    assert!(m.live.nodes.outbox.sending("gpu").is_some());
    assert_eq!(m.live.nodes.outbox.waiting("gpu").len(), 2);
    assert_eq!(
        hosted::send_next(&mut m).units(),
        0,
        "nothing else is sent before the answer"
    );

    let mut theirs = nightly();
    theirs.name = "two".into();
    theirs.panes[0].model = Some("opus".into());
    let server = serve(
        listener,
        vec![
            (200, json!({ "revision": 4 })),
            (200, json!({ "revision": 5 })),
            (200, json!({ "revision": 6 })),
            (200, own_state(6, &[theirs])),
        ],
    );
    for message in outputs(first).await {
        let Message::Nodes(Msg::Sent { id, result }) = message else {
            panic!("an answer");
        };
        let _ = hosted::sent(&mut m, &id, result);
    }
    assert!(deliver(&mut m).await.is_empty());

    let asked = server.await.unwrap();
    let lines: Vec<&str> = asked.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(
        lines,
        [
            "POST /node/own/workspaces/o1/ops",
            "POST /node/own/workspaces/o1/ops",
            "POST /node/own/workspaces/o1/ops",
            "GET /node/own/state",
        ]
    );
    assert_eq!(asked[0].1["ops"][0]["patch"]["name"], "one");
    assert_eq!(asked[1].1["ops"][0]["op"], "patchPane");
    assert_eq!(asked[1].1["ops"][0]["patch"]["model"], "opus");
    assert_eq!(asked[2].1["ops"][0]["patch"]["name"], "two");
    assert!(!m.live.nodes.outbox.busy("gpu"));
    assert_eq!(m.workspace("o1").unwrap().name, "two");
    assert_eq!(
        m.live.nodes.held.get("gpu").map(|held| held.revision),
        Some(6)
    );
}

/// Each node has its own line: one waiting for its answer does not hold
/// the other's back.
#[test]
fn each_node_has_one_request_on_its_way_at_a_time() {
    let mut m = connected();
    up(&mut m, "vps", "vps-01");
    hosted::take_in(
        &mut m,
        "vps",
        answered(
            "v1",
            1,
            vec![theirs("o2", "billing", vec![pane("vp1", "Bo")])],
        ),
        NOW,
    );
    m.apply(rename("one"));
    m.apply(rename("two"));
    m.apply(StoreAction::UpdateWorkspace(
        "o2".into(),
        Patch::new(|ws: &mut Workspace| ws.name = "invoices".into()),
    ));
    assert_eq!(hosted::send_next(&mut m).units(), 2);
    let outbox = &m.live.nodes.outbox;
    assert!(outbox.sending("gpu").is_some() && outbox.sending("vps").is_some());
    assert_eq!(outbox.waiting("gpu").len(), 1);
    assert!(outbox.waiting("vps").is_empty());
    // `vps` answered: nothing more for it, and `gpu` still waits its turn.
    let _ = hosted::sent(&mut m, "vps", Ok(()));
    assert_eq!(hosted::send_next(&mut m).units(), 0);
    let _ = hosted::sent(&mut m, "gpu", Ok(()));
    assert_eq!(hosted::send_next(&mut m).units(), 1);
}

/// What the node listed before it acknowledged the last change does not
/// hold that change: it is not allowed to undo it on the mirror.
#[test]
fn a_listing_older_than_the_last_change_does_not_revert_it() {
    let mut m = connected();
    let before = m.live.nodes.outbox.epoch("gpu");
    m.apply(rename("renamed"));
    // Asked for before the change, answered after it.
    hosted::take_in_asked(
        &mut m,
        "gpu",
        before,
        answered("b1", 4, vec![nightly()]),
        NOW,
    );
    assert_eq!(m.workspace("o1").unwrap().name, "renamed");
    // Asked for after the change was made, while it is still on its way.
    let during = m.live.nodes.outbox.epoch("gpu");
    let _ = hosted::send_next(&mut m);
    hosted::take_in_asked(
        &mut m,
        "gpu",
        during,
        answered("b1", 4, vec![nightly()]),
        NOW,
    );
    assert_eq!(m.workspace("o1").unwrap().name, "renamed");
    // Acknowledged: a listing asked for before that is still too old.
    let _ = hosted::sent(&mut m, "gpu", Ok(()));
    hosted::take_in_asked(
        &mut m,
        "gpu",
        during,
        answered("b1", 4, vec![nightly()]),
        NOW,
    );
    assert_eq!(m.workspace("o1").unwrap().name, "renamed");
    // One asked for since says what the node made of it, the same revision
    // as before or not: the node's word is the last.
    let after = m.live.nodes.outbox.epoch("gpu");
    let mut theirs = nightly();
    theirs.name = "renamed (2)".into();
    hosted::take_in_asked(&mut m, "gpu", after, answered("b1", 3, vec![theirs]), NOW);
    assert_eq!(m.workspace("o1").unwrap().name, "renamed (2)");
}

/// The node refuses a change: what waited behind it is not sent, the
/// mirror is read again from the node, and one notice says so.
#[tokio::test]
async fn a_change_the_node_refuses_is_undone_from_the_node_and_said_once() {
    let (listener, client) = listen().await;
    let mut m = connected();
    reach(&mut m, "gpu", &client);
    m.apply(StoreAction::UpdateWorkspace(
        "o1".into(),
        Patch::new(|ws: &mut Workspace| ws.cwd = "/srv/gone".into()),
    ));
    m.apply(rename("never sent"));
    let server = serve(
        listener,
        vec![
            (400, json!({ "error": "/srv/gone is not a folder here" })),
            (200, own_state(3, &[nightly()])),
        ],
    );
    let notices = deliver(&mut m).await;
    assert_eq!(
        notices,
        [
            "GPU box did not take the change to nightly: /srv/gone is not a folder here. \
          It is shown as GPU box has it."
        ]
    );
    let asked = server.await.unwrap();
    assert_eq!(asked.len(), 2, "the rename behind it was not sent");
    assert_eq!(asked[1].0, "GET /node/own/state");
    let mirror = m.workspace("o1").unwrap();
    assert_eq!(
        (mirror.name.as_str(), mirror.cwd.as_str()),
        ("nightly", "/srv/nightly")
    );
    assert!(!m.live.nodes.outbox.busy("gpu"));
}

/// The link drops with a change on its way: it counts as not made.
#[tokio::test]
async fn a_change_that_cannot_be_sent_is_said_and_the_line_emptied() {
    let mut m = connected();
    m.apply(rename("lost"));
    m.live.nodes.statuses.remove("gpu");
    let notices = deliver(&mut m).await;
    assert_eq!(
        notices,
        [
            "GPU box did not take the change to nightly: GPU box is not connected. \
          It is shown as GPU box has it."
        ]
    );
    assert!(!m.live.nodes.outbox.busy("gpu"));
    assert!(
        !m.live.nodes.held.contains_key("gpu"),
        "read whole when it is back"
    );
    // Back, the node says what it has.
    hosted::take_in(&mut m, "gpu", answered("b1", 3, vec![nightly()]), NOW);
    assert_eq!(m.workspace("o1").unwrap().name, "nightly");
}

/// `api` on `gpu`, driven from this computer: two agents with a session.
fn driven(m: &mut Model) {
    let mut state = m.state().clone();
    let mut worker = pane("ap2", "Wes");
    worker.role = Some(PaneRole::Worker);
    worker.session_id = Some("s2".into());
    let mut orch = pane("ap1", "Rorch");
    orch.session_id = Some("s1".into());
    state.workspaces.push(Workspace {
        id: "a1".into(),
        name: "api".into(),
        cwd: "/srv/api".into(),
        node: Some("gpu".into()),
        folder_id: Some("clients".into()),
        panes: vec![orch, worker],
        ..Workspace::default()
    });
    m.hydrate(state);
}

/// The workspace is the node's here before the node is asked, so that this
/// computer's engines have let go of it when the node takes its shells
/// down; nothing of it is followed until the node said yes.
#[tokio::test]
async fn a_move_is_flagged_here_first_then_handed_over_once() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    let before = m.revision();
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), true);
    let flagged = m.workspace("a1").unwrap();
    assert!(flagged.is_hosted(), "before anything is sent");
    assert!(m.revision() > before);
    assert!(m.live.nodes.outbox.sending("gpu").is_none());
    assert!(m.live.nodes.outbox.adopting("a1"));
    let engines = crate::state::snapshot::own(std::sync::Arc::new(crate::state::snapshot_of(&m)));
    assert!(engines.state.workspaces.iter().all(|ws| ws.id != "a1"));

    let mut theirs = m.workspace("a1").unwrap().clone();
    theirs.node = None;
    theirs.hosted = None;
    theirs.folder_id = None;
    let server = serve(
        listener,
        vec![
            (200, json!({ "id": "a1" })),
            (200, own_state(4, &[nightly(), theirs])),
        ],
    );
    assert!(deliver(&mut m).await.is_empty());
    let asked = server.await.unwrap();
    assert_eq!(asked[0].0, "POST /node/own/adopt");
    let sent = &asked[0].1;
    assert_eq!(sent["id"], "a1");
    let ids: Vec<&str> = sent["panes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pane| pane["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["ap1", "ap2"],
        "under the ids this computer knows them by"
    );
    assert_eq!(sent["panes"][1]["sessionId"], "s2");
    assert_eq!(asked.len(), 2, "one adopt, then the node is read");

    let mirror = m.workspace("a1").unwrap();
    assert!(mirror.is_hosted() && !m.live.nodes.outbox.adopting("a1"));
    assert_eq!(
        mirror.folder_id.as_deref(),
        Some("clients"),
        "filed as it was"
    );
}

#[tokio::test]
async fn a_move_the_node_refuses_leaves_the_workspace_driven_from_here() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), true);
    let server = serve(
        listener,
        vec![
            (400, json!({ "error": "/srv/api is not a folder here" })),
            (200, own_state(3, &[nightly()])),
        ],
    );
    let notices = deliver(&mut m).await;
    assert_eq!(
        notices,
        [
            "GPU box could not take api: /srv/api is not a folder here. \
          It still runs from this computer."
        ]
    );
    server.await.unwrap();
    let back = m.workspace("a1").expect("still there");
    assert!(!back.is_hosted());
    assert_eq!(back.node.as_deref(), Some("gpu"));
    assert_eq!(back.panes.len(), 2);
    assert!(!m.live.nodes.outbox.adopting("a1"));
    assert!(m.take_removed().is_empty());
}

#[test]
fn a_move_is_refused_with_its_reason_while_it_would_break_something() {
    let mut m = connected();
    driven(&mut m);
    let ws = |m: &Model| m.workspace("a1").unwrap().clone();
    assert_eq!(hosted::move_refusal(&m, &ws(&m)), None);

    // A worker that has no session yet would start over without its task.
    m.apply(StoreAction::UpdatePane {
        ws: "a1".into(),
        pane: "ap2".into(),
        patch: Patch::new(|pane: &mut Pane| pane.session_id = None),
    });
    assert_eq!(
        hosted::move_refusal(&m, &ws(&m)).as_deref(),
        Some("api cannot move to GPU box while a worker is starting.")
    );
    // The yes of a question asked before that changes nothing.
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), true);
    assert!(!ws(&m).is_hosted());
    assert!(!m.live.nodes.outbox.busy("gpu"));

    m.live.control.begin_for_test("a1", "t1");
    assert_eq!(
        hosted::move_refusal(&m, &ws(&m)).as_deref(),
        Some("api cannot move to GPU box while one of its deliveries runs.")
    );
    m.live.nodes.statuses.remove("gpu");
    assert_eq!(
        hosted::move_refusal(&m, &ws(&m)).as_deref(),
        Some("GPU box is offline: api cannot move there now.")
    );
    // A no closes the question and does nothing.
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), false);
    assert!(!ws(&m).is_hosted());
}

/// Deleting a mirror, once confirmed, deletes the workspace on its node.
#[test]
fn a_mirror_deleted_is_deleted_on_its_node() {
    let mut m = connected();
    let _ = super::ask::answered(&mut m, Ask::Remove("o1".into()), false);
    assert!(m.workspace("o1").is_some());
    let _ = super::ask::answered(&mut m, Ask::Remove("o1".into()), true);
    assert!(m.workspace("o1").is_none());
    assert_eq!(waiting(&m), [("o1".to_string(), HostedOp::RemoveWorkspace)]);
}

/// A node removed from the settings takes its mirrors and what waited for
/// it; a workspace this computer drives there stays, as before, without a
/// machine to run on.
#[test]
fn a_node_removed_leaves_what_this_computer_drives_there() {
    let mut m = connected();
    driven(&mut m);
    m.apply(rename("unsent"));
    hosted::forget(&mut m, "gpu", NOW);
    assert!(m.workspace("o1").is_none());
    assert!(!m.live.nodes.outbox.busy("gpu"));
    let kept = m.workspace("a1").expect("kept");
    assert_eq!(kept.node.as_deref(), Some("gpu"));
    assert!(!kept.is_hosted());
    assert_eq!(hosted::driven_here(&m, "gpu"), 1);
    assert_eq!(hosted::count(&m, "gpu"), 0);
}

#[test]
fn the_failure_words_say_what_became_of_the_workspace() {
    let m = connected();
    let job = Job::Adopt {
        workspace: Box::new(theirs("n1", "fresh", vec![pane("np1", "Ava")])),
        handover: Handover::Created,
    };
    assert_eq!(
        hosted::failure_words(&m, "gpu", &job, "/srv/fresh is not a folder here"),
        "GPU box could not take fresh: /srv/fresh is not a folder here. \
         The workspace was not made."
    );
}

/// Closing the app with a workspace on its way to a node saves it as this
/// computer's: as the node's, before the node took it, it would be a mirror
/// of nothing at the next start.
#[test]
fn a_workspace_on_its_way_when_the_app_closes_is_saved_as_this_computer_s() {
    let mut m = connected();
    driven(&mut m);
    assert!(!m.live.nodes.outbox.handing_over());
    hosted::hand_over(&mut m, "a1", Handover::Moved);
    assert!(m.live.nodes.outbox.handing_over(), "no save meanwhile");
    hosted::abandon(&mut m);
    assert!(!m.workspace("a1").unwrap().is_hosted());
    assert!(!m.live.nodes.outbox.handing_over());
}
