//! The workspaces a node runs itself, mirrored: what its node says is taken
//! in, what the person changes of the node's is refused while the node is
//! away, the node's events reach their panes, and nothing of this machine
//! starts, ends or drives them. What the person changes while the node is
//! there is `forward_tests.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::json;

use super::hosted::{self, OwnState};
use super::remote::{self, Started};
use super::testing::{answer, listen, machine, read_request};
use super::{Msg, event_messages};
use crate::message::Message;
use crate::panes::controller::{ShellEnd, shell_end};
use crate::services::CoreEvent;
use crate::state::live::{AgentActivity, GlyphState};
use crate::state::types::{
    AgentId, AppState, DropZone, MachineKind, Pane, Workspace, WorkspaceFolder,
};
use crate::state::{Model, Patch, Snapshot, StoreAction};

pub(super) const NOW: u64 = 5_000_000;

pub(super) fn pane(id: &str, name: &str) -> Pane {
    Pane {
        id: id.into(),
        name: name.into(),
        agent: AgentId::Claude,
        ..Pane::default()
    }
}

/// A workspace as a node holds one of its own.
pub(super) fn theirs(id: &str, name: &str, panes: Vec<Pane>) -> Workspace {
    Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/srv/{name}"),
        panes,
        ..Workspace::default()
    }
}

/// What node `gpu` answers: `nightly` with two agents, at `revision` of
/// run `boot`.
pub(super) fn answered(boot: &str, revision: u64, workspaces: Vec<Workspace>) -> OwnState {
    OwnState {
        boot: boot.into(),
        revision,
        workspaces,
        activity: HashMap::new(),
        bridged: HashSet::new(),
    }
}

pub(super) fn nightly() -> Workspace {
    theirs(
        "o1",
        "nightly",
        vec![pane("op1", "Rorch"), pane("op2", "Wes")],
    )
}

/// This computer's `site`, and the machines `gpu` and `vps`.
pub(super) fn model() -> Model {
    let mut m = Model::default();
    let mut state = AppState {
        workspaces: vec![Workspace {
            id: "w1".into(),
            name: "site".into(),
            cwd: "/home/me/site".into(),
            panes: vec![pane("p1", "Ava")],
            ..Workspace::default()
        }],
        active_workspace_id: Some("w1".into()),
        folders: vec![WorkspaceFolder {
            id: "clients".into(),
            name: "Clients".into(),
            ..WorkspaceFolder::default()
        }],
        ..AppState::default()
    };
    state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
    state.settings.nodes = vec![
        machine("gpu", "GPU box", MachineKind::Tower),
        machine("vps", "vps", MachineKind::Cloud),
    ];
    m.hydrate(state);
    m
}

/// `model`, with `nightly` of `gpu` mirrored.
pub(super) fn mirrored() -> Model {
    let mut m = model();
    hosted::take_in(&mut m, "gpu", answered("b1", 3, vec![nightly()]), NOW);
    m
}

pub(super) fn names(m: &Model) -> Vec<&str> {
    m.state()
        .workspaces
        .iter()
        .map(|ws| ws.name.as_str())
        .collect()
}

#[test]
fn an_answer_is_read_and_one_that_is_not_one_is_nothing() {
    let value = json!({
        "boot": "b1",
        "revision": 7,
        "workspaces": [{ "id": "o1", "name": "nightly", "cwd": "/srv/nightly", "panes": [
            { "id": "op1", "name": "Rorch", "agent": "claude" },
        ] }],
        "activity": { "op1": "working", "op9": "no such state" },
        "bridged": ["op1"],
    });
    let own = hosted::parse(&value).expect("an answer");
    assert_eq!((own.boot.as_str(), own.revision), ("b1", 7));
    assert_eq!(own.workspaces[0].panes[0].name, "Rorch");
    assert_eq!(own.activity.get("op1"), Some(&AgentActivity::Working));
    assert_eq!(
        own.activity.len(),
        1,
        "a state it does not know is left out"
    );
    assert!(own.bridged.contains("op1"));
    // An older node has no such route.
    assert_eq!(hosted::parse(&json!({ "error": "not found" })), None);
}

#[test]
fn a_sync_makes_the_mirrors_and_an_older_answer_changes_nothing() {
    let mut m = mirrored();
    assert_eq!(names(&m), ["site", "nightly"]);
    let mirror = m.workspace("o1").unwrap();
    assert!(mirror.is_hosted());
    assert_eq!(mirror.node.as_deref(), Some("gpu"));
    assert_eq!(hosted::count(&m, "gpu"), 1);
    assert_eq!(hosted::count(&m, "vps"), 0);

    // An answer that left before the one held, and arrives after it.
    hosted::take_in(&mut m, "gpu", answered("b1", 2, Vec::new()), NOW);
    assert_eq!(names(&m), ["site", "nightly"]);

    // A later one is what the node now runs.
    let renamed = theirs("o1", "nightly builds", vec![pane("op1", "Rorch")]);
    hosted::take_in(&mut m, "gpu", answered("b1", 4, vec![renamed]), NOW);
    assert_eq!(names(&m), ["site", "nightly builds"]);
    assert_eq!(m.workspace("o1").unwrap().panes.len(), 1);

    // The daemon restarted: its revisions start again, and it is believed.
    hosted::take_in(&mut m, "gpu", answered("b2", 0, Vec::new()), NOW);
    assert_eq!(names(&m), ["site"]);
    assert_eq!(m.live.nodes.held["gpu"].boot, "b2");
}

#[test]
fn an_answer_for_a_node_that_left_the_settings_is_dropped() {
    let mut m = model();
    hosted::take_in(&mut m, "gone", answered("b1", 1, vec![nightly()]), NOW);
    assert_eq!(names(&m), ["site"]);
}

#[test]
fn a_mirrored_pane_shows_what_its_agent_does_before_it_is_ever_opened() {
    let mut m = model();
    let mut own = answered("b1", 3, vec![nightly()]);
    own.activity.insert("op1".into(), AgentActivity::Working);
    own.activity.insert("op2".into(), AgentActivity::Waiting);
    own.bridged.insert("op1".into());
    hosted::take_in(&mut m, "gpu", own.clone(), NOW);
    let glyph = |m: &Model, pane: &str| crate::panes_logic::state_glyph_state(m, &pane.to_string());
    assert_eq!(glyph(&m, "op1"), GlyphState::Working);
    assert_eq!(glyph(&m, "op2"), GlyphState::Waiting);

    // The node passes its agents' reports on: they land in those machines.
    let stop = CoreEvent::Bridge {
        pane: "op1".into(),
        event: json!({ "kind": "turn", "phase": "end", "reason": "answer", "answer": "done" }),
    };
    assert!(crate::panes_logic::reduce_for_test(&mut m, &stop, NOW + 1).is_some());
    assert_ne!(glyph(&m, "op1"), GlyphState::Working);

    // Events were lost: the same revision read again says where they stand.
    hosted::take_in(&mut m, "gpu", own, NOW + 2);
    assert_eq!(glyph(&m, "op1"), GlyphState::Working);

    // A pane the node closed takes what was known of it.
    let one = theirs("o1", "nightly", vec![pane("op1", "Rorch")]);
    hosted::take_in(&mut m, "gpu", answered("b1", 4, vec![one]), NOW + 3);
    assert!(!m.live.panes_logic.is_pane_mounted("op2"));
    assert!(m.live.panes_logic.is_pane_mounted("op1"));
}

#[test]
fn a_node_removed_takes_its_mirrors_and_leaves_the_others() {
    let mut m = mirrored();
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
    assert_eq!(names(&m), ["site", "nightly", "billing"]);
    hosted::forget(&mut m, "gpu", NOW);
    assert_eq!(names(&m), ["site", "billing"]);
    assert!(!m.live.nodes.held.contains_key("gpu"));
    assert!(!m.live.panes_logic.is_pane_mounted("op1"));
    assert!(m.live.panes_logic.is_pane_mounted("vp1"));
}

/// The node is not connected: nothing could tell it.
#[test]
fn a_change_to_what_an_offline_node_owns_is_refused_and_nothing_is_applied() {
    let mut m = mirrored();
    let before = m.state_arc();
    let revision = m.revision();
    let node_owned = [
        StoreAction::UpdateWorkspace(
            "o1".into(),
            Patch::new(|ws: &mut Workspace| ws.name = "mine".into()),
        ),
        StoreAction::UpdatePane {
            ws: "o1".into(),
            pane: "op1".into(),
            patch: Patch::new(|pane: &mut Pane| pane.model = Some("opus".into())),
        },
        StoreAction::ClosePane {
            ws: "o1".into(),
            pane: "op2".into(),
        },
        StoreAction::AddPane {
            ws: "o1".into(),
            agent: None,
            near: None,
            dir: None,
            cwd: None,
            name: None,
            init: None,
        },
        StoreAction::RespawnPane {
            ws: "o1".into(),
            pane: "op1".into(),
            patch: Patch::none(),
        },
        StoreAction::RemoveWorkspace("o1".into()),
    ];
    for action in node_owned {
        let said = format!("{action:?}");
        assert!(m.live.nodes.refused.is_none());
        let outcome = m.apply(action);
        assert!(!outcome.changed, "{said}");
        let (refused, why) = m.live.nodes.refused.take().expect(&said);
        assert_eq!(why, hosted::Why::Offline, "{said}");
        assert_eq!(
            (refused.node.as_str(), refused.workspace.as_str()),
            ("gpu", "o1")
        );
        assert!(
            !refused.ops.is_empty(),
            "what its node would be told: {said}"
        );
    }
    assert!(
        Arc::ptr_eq(&before, &m.state_arc()),
        "the state is untouched"
    );
    assert_eq!(m.revision(), revision);
    assert!(m.take_respawns().is_empty());
    assert!(m.take_removed().is_empty());
    assert!(!m.live.nodes.outbox.busy("gpu"), "nothing waits for it");
}

#[test]
fn the_mirror_s_own_presentation_and_every_other_workspace_change_as_before() {
    let mut m = mirrored();
    let presentation = [
        StoreAction::MoveWorkspace {
            id: "o1".into(),
            folder: Some("clients".into()),
            before: None,
        },
        StoreAction::SetChatView {
            ws: "o1".into(),
            on: true,
        },
        StoreAction::UpdateWorkspace(
            "o1".into(),
            Patch::new(|ws: &mut Workspace| ws.detached = Some(true)),
        ),
        StoreAction::MovePane {
            ws: "o1".into(),
            drag: "op2".into(),
            target: "op1".into(),
            zone: DropZone::Top,
        },
        StoreAction::SetActiveWorkspace(Some("o1".into())),
        // A workspace of this computer, beside the mirror.
        StoreAction::UpdateWorkspace(
            "w1".into(),
            Patch::new(|ws: &mut Workspace| ws.name = "site 2".into()),
        ),
    ];
    for action in presentation {
        let said = format!("{action:?}");
        assert!(m.apply(action).changed, "{said}");
        assert!(m.live.nodes.refused.is_none(), "{said}");
    }
    let mirror = m.workspace("o1").unwrap();
    assert_eq!(mirror.folder_id.as_deref(), Some("clients"));
    assert_eq!(mirror.chat_view, Some(true));
    assert_eq!(mirror.detached, Some(true));
    assert_eq!(m.state().active_workspace_id.as_deref(), Some("o1"));
    assert_eq!(m.workspace("w1").unwrap().name, "site 2");

    // What the node says next keeps all of it.
    hosted::take_in(&mut m, "gpu", answered("b1", 4, vec![nightly()]), NOW);
    let mirror = m.workspace("o1").unwrap();
    assert_eq!(mirror.folder_id.as_deref(), Some("clients"));
    assert_eq!(mirror.chat_view, Some(true));
}

#[test]
fn the_refusal_names_the_workspace_and_its_machine() {
    let mut m = mirrored();
    m.apply(StoreAction::RemoveWorkspace("o1".into()));
    let (refused, why) = m.live.nodes.refused.take().unwrap();
    assert_eq!(
        hosted::refusal_words(&m, &refused, why),
        "GPU box is offline: nightly can be looked at, not changed."
    );
}

fn core_events(messages: Vec<Message>) -> Vec<CoreEvent> {
    messages
        .into_iter()
        .filter_map(|message| match message {
            Message::Core(event) => Some(event),
            _ => None,
        })
        .collect()
}

#[test]
fn a_node_s_queue_reaches_its_panes_and_no_other() {
    let m = mirrored();
    let queue = |pane: &str| {
        json!({ "event": "bridge:queue", "payload": {
            "paneId": pane,
            "pending": [
                { "id": "q1", "text": "run the tests", "from": "Rorch" },
                { "id": "q2", "text": "then ship" },
                { "text": "no id: not a prompt" },
            ],
        } })
    };
    let events = core_events(event_messages("gpu", m.state(), queue("op2")));
    let [CoreEvent::BridgeQueue { pane, pending }] = events.as_slice() else {
        panic!("one queue event, not {events:?}");
    };
    assert_eq!(pane, "op2");
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].from.as_deref(), Some("Rorch"));
    assert_eq!(
        (pending[1].text.as_str(), &pending[1].from),
        ("then ship", &None)
    );
    // A node speaks for its own panes: not this computer's, not another's.
    assert!(event_messages("gpu", m.state(), queue("p1")).is_empty());
    assert!(event_messages("vps", m.state(), queue("op2")).is_empty());
}

#[test]
fn a_state_that_changed_and_events_lost_are_both_read_again() {
    let m = mirrored();
    let syncs = |event| {
        event_messages("gpu", m.state(), event)
            .into_iter()
            .filter(|message| matches!(message, Message::Nodes(Msg::SyncOwn(id)) if id == "gpu"))
            .count()
    };
    assert_eq!(syncs(json!({ "event": "own:changed", "revision": 9 })), 1);
    assert_eq!(syncs(json!({ "lagged": 3 })), 1);
    assert_eq!(syncs(json!({ "event": "memory:changed" })), 0);
}

/// A pane the node runs itself is looked for until the node lists it, and
/// followed whether or not its shell still runs: every request is a read.
#[tokio::test]
async fn a_pane_its_node_runs_is_found_there_and_never_started() {
    let (listener, client) = listen().await;
    let server = tokio::spawn(async move {
        let mut lines = Vec::new();
        for listed in [
            json!({ "panes": [{ "id": "other", "alive": true, "seq": 1 }] }),
            json!({ "panes": [{ "id": "op1", "alive": false, "seq": 42 }] }),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            lines.push(read_request(&mut stream).await.line().to_owned());
            answer(&mut stream, 200, &listed.to_string()).await;
        }
        // Nothing follows: no spawn after the listings.
        let more = tokio::time::timeout(Duration::from_millis(150), listener.accept()).await;
        assert!(more.is_err(), "a third request was made");
        lines
    });
    let started = remote::attach(&client, "op1", 5, Duration::from_millis(5)).await;
    assert_eq!(started, Ok(Started::Attached { seq: 42 }));
    for line in server.await.unwrap() {
        assert!(line.starts_with("GET /node/panes "), "{line}");
    }
}

#[tokio::test]
async fn a_pane_the_node_never_lists_says_so_without_starting_it() {
    let (listener, client) = listen().await;
    let server = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            assert!(request.line().starts_with("GET /node/panes "));
            answer(&mut stream, 200, &json!({ "panes": [] }).to_string()).await;
        }
    });
    let started = remote::attach(&client, "op1", 3, Duration::from_millis(5)).await;
    assert_eq!(started, Err("its node has not started this pane".into()));
    server.await.unwrap();
}

/// Letting go of a pane its node runs kills nothing: not at the unmount,
/// and not through what reaches the remote panes by id (a sleep, an
/// engine), which is refused before the node is asked.
#[tokio::test]
async fn a_pane_its_node_runs_is_never_killed_from_here() {
    assert_eq!(shell_end(Some("gpu".into()), true), ShellEnd::Kept);
    assert_eq!(
        shell_end(Some("gpu".into()), false),
        ShellEnd::OnNode("gpu".into())
    );
    assert_eq!(shell_end(None, false), ShellEnd::Local);

    let (listener, client) = listen().await;
    let node = format!("hosted-kill-{}", client.base_url);
    remote::publish(&node, Some(client));
    let (outgoing, _inbox) = tokio::sync::mpsc::unbounded_channel();
    let alive = Arc::new(AtomicBool::new(true));
    let pane = format!("{node}-pane");
    remote::register(&pane, &node, true, outgoing, alive.clone());
    assert_eq!(
        remote::kill_pane(&pane).await,
        Some(Err(remote::HOSTED_PANE.into()))
    );
    let asked = tokio::time::timeout(Duration::from_millis(150), listener.accept()).await;
    assert!(asked.is_err(), "the node was asked to end the pane");
    // Typing into it still goes to its node.
    assert_eq!(remote::write(&pane, "x".into()), Some(Ok(())));
    remote::unregister(&pane, &alive);
    remote::publish(&node, None);
}

#[test]
fn a_registration_is_only_undone_by_the_pane_that_made_it() {
    let (outgoing, _inbox) = tokio::sync::mpsc::unbounded_channel();
    let first = Arc::new(AtomicBool::new(true));
    let second = Arc::new(AtomicBool::new(true));
    remote::register(
        "remounted-pane",
        "n",
        false,
        outgoing.clone(),
        first.clone(),
    );
    // Mounted again under the same id before the first terminal was dropped.
    remote::register("remounted-pane", "n", true, outgoing, second.clone());
    remote::unregister("remounted-pane", &first);
    assert_eq!(remote::node_of("remounted-pane").as_deref(), Some("n"));
    remote::unregister("remounted-pane", &second);
    assert_eq!(remote::node_of("remounted-pane"), None);
}

/// The engines of this machine read a snapshot without the mirrors: the
/// node's own engines run those workspaces.
#[test]
fn the_engines_snapshot_leaves_the_mirrors_out() {
    let mut m = mirrored();
    m.apply(StoreAction::SetActiveWorkspace(Some("o1".into())));
    m.mount("o1");
    let full = Arc::new(crate::state::snapshot_of(&m));
    assert!(full.opened.contains("o1"));
    let own = crate::state::snapshot::own(full.clone());
    let ids: Vec<&str> = own
        .state
        .workspaces
        .iter()
        .map(|ws| ws.id.as_str())
        .collect();
    assert_eq!(ids, ["w1"]);
    assert!(!own.opened.contains("o1"));
    assert_eq!(own.state.active_workspace_id, None);
    assert!(
        Arc::ptr_eq(&own, &crate::state::snapshot::own(full)),
        "made once for a snapshot"
    );
    // With no mirror there is nothing to leave out, and nothing is copied.
    let plain = Arc::new(crate::state::snapshot_of(&model()));
    assert!(Arc::ptr_eq(&plain, &Snapshot::without_hosted(&plain)));
    // The requests of a node's own agents are not answered here either.
    let narrowed = crate::control::remote::narrowed(&crate::state::snapshot_of(&m), "gpu");
    assert!(narrowed.state.workspaces.is_empty());
}
