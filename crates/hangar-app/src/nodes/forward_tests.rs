//! What the person changes on the mirror of a workspace its node runs:
//! applied here at once, told to the node in order, one request at a time
//! for each node; what the node refuses is read back from it. And a
//! workspace handed to its node, new or one this computer drove.

use iced::Task;
use iced::futures::StreamExt as _;
use iced_runtime::Action;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use super::hosted::{self, Handover, Job, NotTaken, Why};
use super::hosted_tests::{NOW, answered, mirrored, nightly, pane, theirs};
use super::model::{NodeClient, NodeInfo};
use super::testing::{TOKEN, answer, listen, read_request, up};
use super::{Ask, Msg, NodeStatus};
use crate::message::Message;
use crate::state::hosted::HostedOp;
use crate::state::types::{DropZone, Pane, PaneRole, Workspace};
use crate::state::{Model, Patch, StoreAction};

/// `nightly` of `gpu` mirrored, `gpu` connected.
pub(super) fn connected() -> Model {
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
pub(super) async fn outputs(task: Task<Message>) -> Vec<Message> {
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
pub(super) fn reach(m: &mut Model, id: &str, client: &NodeClient) {
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
pub(super) fn serve(
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
pub(super) async fn deliver(m: &mut Model) -> Vec<String> {
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
    notices.extend(told(m));
    notices
}

/// What the model left for the shell to say, taken as the shell takes it.
pub(super) fn told(m: &mut Model) -> Vec<String> {
    std::mem::take(&mut m.live.nodes.told)
        .into_iter()
        .map(|notice| match notice {
            crate::shell::Notice::Problem(text) | crate::shell::Notice::Agent(text) => text,
            other => panic!("not expected: {other:?}"),
        })
        .collect()
}

/// A fake node that takes the next request and hangs up without a word,
/// as a link that drops on the way does; then answers the rest as
/// [`serve`] does.
fn serve_after_a_lost_answer(
    listener: TcpListener,
    answers: Vec<(u16, Value)>,
) -> tokio::task::JoinHandle<Vec<(String, Value)>> {
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        let line = request.line().trim_end_matches(" HTTP/1.1").to_owned();
        let sent = serde_json::from_str(&request.body).unwrap_or(Value::Null);
        drop(stream);
        let mut asked = vec![(line, sent)];
        asked.extend(serve(listener, answers).await.unwrap());
        asked
    })
}

pub(super) fn own_state(revision: u64, workspaces: &[Workspace]) -> Value {
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
    // The mirror still shows the change until the node is read again, and
    // the notice does not say otherwise.
    assert_eq!(
        notices,
        [
            "The change to nightly may not have reached GPU box: GPU box is not connected. \
          Once GPU box answers again, nightly is shown as GPU box has it."
        ]
    );
    assert_eq!(m.workspace("o1").unwrap().name, "lost");
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
pub(super) fn driven(m: &mut Model) {
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
        hosted::failure_words(
            &m,
            "gpu",
            &job,
            &NotTaken::Refused("/srv/fresh is not a folder here".into())
        ),
        "GPU box could not take fresh: /srv/fresh is not a folder here. \
         The workspace was not made."
    );
    // Without an answer nothing is said to be undone: it is not known.
    assert_eq!(
        hosted::failure_words(
            &m,
            "gpu",
            &job,
            &NotTaken::Unknown("POST /node/own/adopt: no answer".into())
        ),
        "GPU box did not say whether it took fresh: POST /node/own/adopt: no answer. \
         It is left as it is until GPU box answers again."
    );
}

/// `a1` as its node holds it once taken.
fn taken(m: &Model) -> Workspace {
    let mut theirs = m.workspace("a1").unwrap().clone();
    theirs.node = None;
    theirs.hosted = None;
    theirs.handover = None;
    theirs.folder_id = None;
    theirs
}

/// The answer to a move is lost on the way (the link dropped, the laptop
/// slept): the node may have taken the workspace. It is not taken back,
/// which would have this computer drive what the node now runs; it stays
/// on its way, not followed and not run from here, and what the node lists
/// next settles it. Here the node did take it.
#[tokio::test]
async fn a_move_whose_answer_is_lost_is_settled_by_the_node_s_list_when_it_took_it() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), true);
    let theirs = taken(&m);
    let server =
        serve_after_a_lost_answer(listener, vec![(200, own_state(4, &[nightly(), theirs]))]);
    let notices = deliver(&mut m).await;
    let asked = server.await.unwrap();
    assert_eq!(asked[0].0, "POST /node/own/adopt");
    assert_eq!(asked[1].0, "GET /node/own/state");
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert!(
        notices[0].starts_with("GPU box did not say whether it took api: "),
        "{notices:?}"
    );
    assert!(notices[0].ends_with("It is left as it is until GPU box answers again."));
    assert_eq!(notices[1], "GPU box did take api: it runs there now.");
    let mirror = m.workspace("a1").unwrap();
    assert!(mirror.is_hosted() && mirror.handover.is_none());
    assert!(!hosted::unfollowed(&m, mirror));
    assert_eq!(mirror.folder_id.as_deref(), Some("clients"));
}

/// The same, and the node never had it: only then is the workspace this
/// computer's again, and said to be.
#[tokio::test]
async fn a_move_whose_answer_is_lost_is_taken_back_once_the_node_does_not_list_it() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    let _ = super::ask::answered(&mut m, Ask::Move("a1".into()), true);
    // The link is down when the answer is lost: nothing settles it yet.
    let server = serve_after_a_lost_answer(listener, Vec::new());
    let _ = hosted::send_next(&mut m);
    m.live.nodes.statuses.remove("gpu");
    let lost = NotTaken::Unknown("connection reset".into());
    let _ = hosted::sent(&mut m, "gpu", Err(lost));
    server.abort();
    let waiting = m.workspace("a1").unwrap().clone();
    assert!(
        waiting.is_hosted(),
        "not this computer's to drive meanwhile"
    );
    assert_eq!(waiting.handover, Some(Handover::Moved));
    assert!(hosted::unfollowed(&m, &waiting), "nor followed on the node");
    assert!(
        !m.live.nodes.outbox.busy("gpu"),
        "what it lists is taken in"
    );
    let engines = crate::state::snapshot::own(std::sync::Arc::new(crate::state::snapshot_of(&m)));
    assert!(engines.state.workspaces.iter().all(|ws| ws.id != "a1"));
    // Written as it is: on its way, to be settled whenever the node is
    // next heard, by this run of the app or the next.
    let saved = m.to_json();
    let written = saved["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ws| ws["id"] == "a1")
        .unwrap();
    assert_eq!(
        (&written["hosted"], &written["handover"]),
        (&json!(true), &json!("moved"))
    );
    assert!(told(&mut m).is_empty());

    // Back, the node lists what it runs: not this one.
    hosted::take_in(&mut m, "gpu", answered("b1", 3, vec![nightly()]), NOW);
    assert_eq!(
        told(&mut m),
        ["GPU box never took api. It still runs from this computer."]
    );
    let back = m.workspace("a1").expect("kept");
    assert!(!back.is_hosted() && back.handover.is_none());
    assert_eq!(back.node.as_deref(), Some("gpu"));
    assert!(m.take_removed().is_empty());
}

/// A workspace made for a node whose answer is lost: listed, it is the
/// node's; not listed, it was never made, and only then goes.
#[test]
fn a_workspace_made_for_a_node_waits_for_the_node_s_list_too() {
    let made = |m: &mut Model| {
        let mut state = m.state().clone();
        state.workspaces.push(Workspace {
            node: Some("gpu".into()),
            ..theirs("n1", "fresh", vec![pane("np1", "Ava")])
        });
        m.hydrate(state);
        assert!(hosted::hand_over(m, "n1", Handover::Created));
        let _ = hosted::send_next(m);
        let _ = hosted::sent(m, "gpu", Err(NotTaken::Unknown("no answer".into())));
        assert!(m.workspace("n1").is_some(), "not taken back out on a guess");
    };
    let mut m = connected();
    made(&mut m);
    let fresh = theirs("n1", "fresh", vec![pane("np1", "Ava")]);
    hosted::take_in(
        &mut m,
        "gpu",
        answered("b1", 4, vec![nightly(), fresh]),
        NOW,
    );
    assert!(m.workspace("n1").is_some_and(|ws| ws.handover.is_none()));
    assert_eq!(told(&mut m), ["GPU box did take fresh: it runs there now."]);

    let mut m = connected();
    made(&mut m);
    hosted::take_in(&mut m, "gpu", answered("b1", 3, vec![nightly()]), NOW);
    assert!(m.workspace("n1").is_none());
    assert_eq!(
        told(&mut m),
        ["GPU box never took fresh. The workspace was not made."]
    );
}

/// A state read back with a workspace still on its way (the app closed
/// before its node answered) is settled the same way by the first list.
#[test]
fn a_workspace_saved_on_its_way_is_settled_at_the_next_start() {
    let mut m = connected();
    driven(&mut m);
    let mut state = m.state().clone();
    for ws in state.workspaces.iter_mut().filter(|ws| ws.id == "a1") {
        ws.hosted = Some(true);
        ws.handover = Some(Handover::Moved);
    }
    let mut next = Model::default();
    next.hydrate(state);
    up(&mut next, "gpu", "gpu-01");
    let waiting = next.workspace("a1").unwrap().clone();
    assert!(hosted::unfollowed(&next, &waiting));
    let theirs = taken(&next);
    hosted::take_in(
        &mut next,
        "gpu",
        answered("b2", 1, vec![nightly(), theirs]),
        NOW,
    );
    let mirror = next.workspace("a1").unwrap();
    assert!(mirror.is_hosted() && mirror.handover.is_none());
    assert_eq!(
        told(&mut next),
        ["GPU box did take api: it runs there now."]
    );
}

/// A node removed while a workspace is on its way there: the workspace
/// stays this computer's, as one it drives there does, whether the request
/// waited, was on its way, or had no answer.
#[test]
fn a_node_removed_during_a_move_leaves_the_workspace_here() {
    for stage in 0..3 {
        let mut m = connected();
        driven(&mut m);
        hosted::hand_over(&mut m, "a1", Handover::Moved);
        if stage >= 1 {
            let _ = hosted::send_next(&mut m);
        }
        if stage == 2 {
            let _ = hosted::sent(&mut m, "gpu", Err(NotTaken::Unknown("no answer".into())));
        }
        hosted::forget(&mut m, "gpu", NOW);
        assert!(m.workspace("o1").is_none(), "its mirrors go");
        let kept = m
            .workspace("a1")
            .unwrap_or_else(|| panic!("kept at stage {stage}"));
        assert!(!kept.is_hosted() && kept.handover.is_none());
        assert_eq!(kept.panes.len(), 2);
        assert!(!m.live.nodes.outbox.busy("gpu"));
        // A late answer for a node that is gone changes nothing.
        let _ = hosted::sent(&mut m, "gpu", Ok(()));
        assert!(m.workspace("a1").is_some_and(|ws| !ws.is_hosted()));
    }
}

/// A workspace on its way to its node is not deleted meanwhile: were the
/// node then to refuse it, the shells this computer started for it there
/// would be nobody's. It is said, and nothing is queued.
#[test]
fn a_workspace_on_its_way_is_not_deleted_before_its_node_answered() {
    let mut m = connected();
    driven(&mut m);
    hosted::hand_over(&mut m, "a1", Handover::Moved);
    let before = m.live.nodes.outbox.waiting("gpu").len();
    m.apply(StoreAction::RemoveWorkspace("a1".into()));
    assert!(m.workspace("a1").is_some());
    assert_eq!(m.live.nodes.outbox.waiting("gpu").len(), before);
    let (edit, why) = m.live.nodes.refused.take().expect("refused");
    assert_eq!(why, Why::OnItsWay);
    assert_eq!(
        hosted::refusal_words(&m, &edit, why),
        "api is on its way to GPU box: it can be deleted once GPU box has said whether it took it."
    );
    // Any other change to it is made and waits behind it.
    m.apply(StoreAction::UpdateWorkspace(
        "a1".into(),
        Patch::new(|ws: &mut Workspace| ws.name = "api v2".into()),
    ));
    assert!(m.live.nodes.refused.is_none());
    assert_eq!(m.workspace("a1").unwrap().name, "api v2");
}

/// What a fake node was asked, in order: each request's line and body.
type Asked = std::sync::Arc<std::sync::Mutex<Vec<(String, Value)>>>;

/// A fake node that answers every request with what `answer` gives for
/// its line, for as long as it is asked.
fn serve_all(
    listener: TcpListener,
    answer_for: fn(&str) -> (u16, Value),
) -> (tokio::task::JoinHandle<()>, Asked) {
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = asked.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let line = request.line().trim_end_matches(" HTTP/1.1").to_owned();
            let sent = serde_json::from_str(&request.body).unwrap_or(Value::Null);
            let (status, body) = answer_for(&line);
            seen.lock().unwrap().push((line, sent));
            answer(&mut stream, status, &body.to_string()).await;
        }
    });
    (server, asked)
}

/// Closing the app sends what the nodes were still to be told, in order,
/// the one on its way included: left unsent, a change saved with the
/// mirror would be undone without a word at the next start.
#[tokio::test(flavor = "multi_thread")]
async fn what_waits_for_a_node_is_sent_as_the_app_closes() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    m.apply(rename("one"));
    m.apply(rename("two"));
    hosted::hand_over(&mut m, "a1", Handover::Moved);
    // The first is on its way, its answer not in yet.
    let _ = hosted::send_next(&mut m);
    let (server, asked) = serve_all(listener, |_| (200, json!({ "revision": 9, "id": "a1" })));
    let unsent = tokio::task::block_in_place(|| hosted::flush(&mut m, hosted::FLUSH));
    server.abort();
    assert_eq!(unsent, Vec::<String>::new());
    let asked = asked.lock().unwrap().clone();
    let lines: Vec<&str> = asked.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(
        lines,
        [
            "POST /node/own/workspaces/o1/ops",
            "POST /node/own/workspaces/o1/ops",
            "POST /node/own/adopt",
        ]
    );
    assert_eq!(asked[0].1["ops"][0]["patch"]["name"], "one");
    assert_eq!(asked[1].1["ops"][0]["patch"]["name"], "two");
    assert!(!m.live.nodes.outbox.busy("gpu"));
    // The node took it: saved as its mirror, no longer on its way.
    let mirror = m.workspace("a1").unwrap();
    assert!(mirror.is_hosted() && mirror.handover.is_none());
}

/// A node that does not answer in time does not hold the app open: what
/// did not get through is listed for the log, a workspace whose answer did
/// not come stays on its way in what is saved, and one never sent is this
/// computer's again.
#[tokio::test(flavor = "multi_thread")]
async fn closing_does_not_wait_long_for_a_node_that_does_not_answer() {
    let (listener, client) = listen().await;
    let mut m = connected();
    driven(&mut m);
    reach(&mut m, "gpu", &client);
    hosted::hand_over(&mut m, "a1", Handover::Moved);
    m.apply(rename("unheard"));
    // Connections are taken and never answered.
    let server = tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            held.push(listener.accept().await.unwrap());
        }
    });
    let started = std::time::Instant::now();
    let within = std::time::Duration::from_millis(300);
    let unsent = tokio::task::block_in_place(|| hosted::flush(&mut m, within));
    server.abort();
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert_eq!(unsent.len(), 2, "{unsent:?}");
    assert!(
        unsent[0].starts_with("GPU box did not say whether it took api"),
        "{unsent:?}"
    );
    assert!(unsent[1].starts_with("The change to nightly may not have reached GPU box"));
    let waiting = m.workspace("a1").unwrap();
    assert!(waiting.is_hosted());
    assert_eq!(waiting.handover, Some(Handover::Moved));

    // With the node away, nothing was sent: the workspace was never the
    // node's.
    let mut m = connected();
    driven(&mut m);
    hosted::hand_over(&mut m, "a1", Handover::Moved);
    m.live.nodes.statuses.remove("gpu");
    let unsent = hosted::flush(&mut m, within);
    assert_eq!(unsent.len(), 1);
    let back = m.workspace("a1").unwrap();
    assert!(!back.is_hosted() && back.handover.is_none());
}

/// An edit made while the link is only just down, or coming up, is refused
/// for what is going on: the machine is not said to be offline while every
/// surface shows it connected or connecting.
#[test]
fn an_edit_refused_while_the_link_comes_back_says_so() {
    let refusal = |m: &mut Model| {
        m.apply(rename("later"));
        let (edit, why) = m.live.nodes.refused.take().expect("refused");
        assert_eq!(
            m.workspace("o1").unwrap().name,
            "nightly",
            "nothing applied"
        );
        (hosted::refusal_words(m, &edit, why), why)
    };
    // The link dropped two seconds ago.
    let mut m = connected();
    super::testing::down(&mut m, "gpu", 2, None);
    assert_eq!(
        refusal(&mut m),
        (
            "GPU box is reconnecting: nightly can be changed once it is back.".to_string(),
            Why::Reconnecting
        )
    );
    // Still trying past the grace period, no attempt failed yet.
    super::testing::down(&mut m, "gpu", 30, None);
    assert_eq!(refusal(&mut m).1, Why::Reconnecting);
    // An attempt failed: it is offline.
    let failure = super::model::Failure::Tunnel("Connection reset".into());
    super::testing::down(&mut m, "gpu", 30, Some(failure));
    assert_eq!(
        refusal(&mut m),
        (
            "GPU box is offline: nightly can be looked at, not changed.".to_string(),
            Why::Offline
        )
    );
    // Never connected since the app started, its first attempt running.
    let mut m = mirrored();
    m.live.nodes.statuses.insert(
        "gpu".into(),
        NodeStatus::Connecting(super::model::Step::Install),
    );
    assert_eq!(
        refusal(&mut m),
        (
            "GPU box is still connecting: nightly can be changed once it is connected.".to_string(),
            Why::Connecting
        )
    );
}

/// A role given to, or taken from, a pane of a mirror restarts the pane on
/// its node, on the same conversation: the node's agent only learns what
/// it is at a launch, and nothing on this computer can tell it. Taking the
/// role away is no different: left running, the agent would go on
/// orchestrating a workspace that shows no orchestrator.
#[tokio::test]
async fn a_role_given_or_taken_on_a_mirror_restarts_the_pane_on_its_node() {
    let mut m = connected();
    let restart = |m: &mut Model, pinned: bool| {
        let asked = crate::orchestration_ui::pin_orchestrator(m, "o1", "op1", pinned);
        async move { outputs(asked).await }
    };
    let said = restart(&mut m, true).await;
    let [Message::Intent(crate::message::Intent::ReplacePane { ws, pane, patch })] =
        said.as_slice()
    else {
        panic!("a restart, not {said:?}");
    };
    assert_eq!((ws.as_str(), pane.as_str()), ("o1", "op1"));
    let mut pinned = m.workspace("o1").unwrap().panes[0].clone();
    patch.apply(&mut pinned);
    assert_eq!(pinned.role, Some(PaneRole::Orchestrator));

    // The node has it as its orchestrator.
    let mut theirs = nightly();
    theirs.panes[0].role = Some(PaneRole::Orchestrator);
    hosted::take_in(&mut m, "gpu", answered("b1", 4, vec![theirs]), NOW);
    let said = restart(&mut m, false).await;
    let [Message::Intent(crate::message::Intent::ReplacePane { pane, patch, .. })] =
        said.as_slice()
    else {
        panic!("a restart without the role, not {said:?}");
    };
    assert_eq!(pane, "op1");
    let mut unpinned = m.workspace("o1").unwrap().panes[0].clone();
    patch.apply(&mut unpinned);
    assert_eq!(unpinned.role, None);
    // The mirror itself is only changed by the restart, which is what the
    // node is told.
    assert!(!m.live.nodes.outbox.busy("gpu"));
    assert_eq!(
        m.workspace("o1").unwrap().panes[0].role,
        Some(PaneRole::Orchestrator)
    );
    // A pane that has no role to lose is left alone.
    assert!(restart(&mut m, false).await.len() == 1);
    let said = crate::orchestration_ui::pin_orchestrator(&mut m, "o1", "op2", false);
    assert!(outputs(said).await.is_empty());
}
