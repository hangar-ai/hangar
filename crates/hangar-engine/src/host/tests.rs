//! The host with no window, over shells that only record what they are
//! asked: the state read and saved, the panes started and stopped as the
//! state says, the agent's command typed, the mod's reports heard, the
//! agents' requests answered.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::live::AgentActivity;
use crate::state::types::{AgentId, Settings};
use crate::testdir::scratch;

/// Shells that print a prompt when they start, and remember what they were
/// typed.
#[derive(Default)]
struct Shells {
    spawned: Mutex<Vec<Spawn>>,
    typed: Mutex<Vec<(PaneId, String)>>,
    alive: Mutex<HashSet<PaneId>>,
    killed: Mutex<Vec<PaneId>>,
    sinks: Mutex<Vec<(PaneId, TermSink)>>,
    /// How long a shell takes to start.
    slow: Mutex<Duration>,
}

impl Shells {
    fn spawned(&self) -> Vec<PaneId> {
        locked(&self.spawned).iter().map(|s| s.id.clone()).collect()
    }

    fn typed_into(&self, pane: &str) -> Vec<String> {
        locked(&self.typed)
            .iter()
            .filter(|(id, _)| id == pane)
            .map(|(_, text)| text.clone())
            .collect()
    }

    /// The shell of `pane` ends by itself.
    fn exit(&self, pane: &str) {
        locked(&self.alive).remove(pane);
        let sink = locked(&self.sinks)
            .iter()
            .find(|(id, _)| id == pane)
            .map(|(_, sink)| sink.clone());
        if let Some(sink) = sink {
            sink(TermEvent::Exit(pane.to_string()));
        }
    }
}

impl Terminals for Shells {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        let id = spawn.id.clone();
        let slow = *locked(&self.slow);
        std::thread::sleep(slow);
        locked(&self.alive).insert(id.clone());
        locked(&self.spawned).push(spawn);
        locked(&self.sinks).push((id.clone(), sink.clone()));
        // The prompt.
        sink(TermEvent::Output(id));
        Ok(())
    }

    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        locked(&self.typed).push((pane.to_string(), data.to_string()));
        Ok(())
    }

    fn alive(&self, pane: &str) -> bool {
        locked(&self.alive).contains(pane)
    }

    fn kill(&self, pane: &str) -> Result<(), String> {
        locked(&self.alive).remove(pane);
        locked(&self.killed).push(pane.to_string());
        Ok(())
    }

    fn lines(&self, pane: &str, n: usize) -> Option<Vec<String>> {
        let _ = n;
        self.alive(pane).then(|| vec!["$ ".to_string()])
    }

    fn screen(&self, pane: &str) -> Option<Vec<String>> {
        self.alive(pane).then(|| vec!["$ ".to_string()])
    }

    fn kill_all(&self) {
        locked(&self.alive).clear();
    }
}

fn workspace(id: &str, cwd: &str, panes: Vec<Pane>) -> Workspace {
    Workspace {
        id: id.into(),
        name: format!("Space {id}"),
        cwd: cwd.into(),
        panes,
        ..Workspace::default()
    }
}

fn pane(id: &str, name: &str, agent: AgentId) -> Pane {
    Pane {
        id: id.into(),
        name: name.into(),
        agent,
        ..Pane::default()
    }
}

struct Rig {
    host: EngineHost,
    shells: Arc<Shells>,
    file: PathBuf,
    core: hangar_core::Host,
}

/// A host over `state`, written to its file first.
fn rig(state: Option<AppState>) -> Rig {
    let root = scratch("engine-host");
    let file = root.join("engine").join("state.json");
    if let Some(state) = state {
        write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    }
    let core = hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(&root));
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: core.clone(),
        terminals: shells.clone(),
        state_file: file.clone(),
    })
    .unwrap();
    Rig {
        host,
        shells,
        file,
        core,
    }
}

/// Waits up to two seconds for `done`.
async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never happened: {what}");
}

fn shop() -> AppState {
    AppState {
        workspaces: vec![workspace(
            "shop",
            "/srv/shop",
            vec![
                pane("p1", "Leo", AgentId::Claude),
                pane("p2", "Sh", AgentId::Shell),
            ],
        )],
        active_workspace_id: Some("shop".into()),
        settings: Settings {
            claude_bridge: true,
            ..Settings::default()
        },
        ..AppState::default()
    }
}

#[test]
fn loads_the_state_with_every_workspace_open() {
    let rig = rig(Some(shop()));
    let snapshot = rig.host.snapshot();
    assert_eq!(snapshot.state.workspaces.len(), 1);
    assert!(snapshot.opened.contains("shop"));
    assert!(snapshot.hydrated);
}

#[test]
fn starts_empty_without_a_file_and_refuses_one_it_cannot_read() {
    let rig = rig(None);
    assert!(rig.host.snapshot().state.workspaces.is_empty());

    let root = scratch("engine-host-bad");
    let file = root.join("state.json");
    std::fs::write(&file, "{ not json").unwrap();
    let refused = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(&root)),
        terminals: Arc::new(Shells::default()),
        state_file: file.clone(),
    });
    assert!(refused.is_err());
    // Left as it was, for the person to look at.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
}

#[tokio::test(flavor = "multi_thread")]
async fn starts_a_shell_per_pane_and_types_the_agent_once_it_has_settled() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("both shells started", || rig.shells.spawned().len() == 2).await;
    let spawned = locked(&rig.shells.spawned).clone();
    let leo = spawned.iter().find(|s| s.id == "p1").unwrap();
    assert_eq!(leo.cwd, "/srv/shop");
    assert_eq!(leo.agent, "claude");
    assert!(leo.bridge);
    eventually("Claude Code typed in Leo's shell", || {
        !rig.shells.typed_into("p1").is_empty()
    })
    .await;
    let typed = rig.shells.typed_into("p1");
    assert_eq!(typed.len(), 1);
    assert!(typed[0].starts_with("claude"), "{typed:?}");
    assert!(typed[0].ends_with('\r'));
    // A plain shell is left at its prompt.
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(rig.shells.typed_into("p2").is_empty());
    assert_eq!(
        rig.host.snapshot().state.workspaces[0].panes.len(),
        2,
        "starting changed nothing in the state"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pane_added_starts_and_one_closed_is_killed_and_the_state_is_saved() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;

    let added = rig
        .host
        .apply(ModelCmd::AddPane {
            ws: "shop".into(),
            agent: Some(AgentId::Claude),
            near: None,
            dir: None,
            cwd: Some("/srv/shop/.worktrees/t1".into()),
            name: Some("Ada".into()),
            init: None,
        })
        .unwrap();
    let ada = added.pane.expect("a pane");
    eventually("Ada's shell started", || {
        rig.shells.spawned().contains(&ada)
    })
    .await;
    let spawn = locked(&rig.shells.spawned)
        .iter()
        .find(|s| s.id == ada)
        .cloned()
        .unwrap();
    assert_eq!(spawn.cwd, "/srv/shop/.worktrees/t1");

    rig.host
        .apply(ModelCmd::ClosePane {
            ws: "shop".into(),
            pane: "p2".into(),
        })
        .unwrap();
    eventually("the shell of the pane closed killed", || {
        locked(&rig.shells.killed).contains(&"p2".to_string())
    })
    .await;

    // Saved a moment later, whole.
    eventually("the state saved", || {
        std::fs::read_to_string(&rig.file)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .is_some_and(|saved| {
                let panes = &saved["workspaces"][0]["panes"];
                panes.as_array().is_some_and(|panes| {
                    panes.iter().any(|p| p["name"] == "Ada")
                        && !panes.iter().any(|p| p["id"] == "p2")
                })
            })
    })
    .await;
    // Read back by the next run.
    let again = EngineHost::load(Config {
        core: rig.core.clone(),
        terminals: Arc::new(Shells::default()),
        state_file: rig.file.clone(),
    })
    .unwrap();
    let names: Vec<String> = again.snapshot().state.workspaces[0]
        .panes
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(names, ["Leo", "Ada"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn hears_the_mod_report_a_turn_and_tells_who_follows() {
    let rig = rig(Some(shop()));
    let mut news = rig.host.news();
    rig.host.start();
    eventually("Leo mounted", || {
        locked(&rig.host.0.model).panes.machines.contains_key("p1")
    })
    .await;
    rig.core
        .emit(Event::BridgeEvent {
            pane_id: "p1".into(),
            event: json!({"kind": "hello", "sessionId": "s1", "model": null, "version": "2.1"}),
        })
        .unwrap();
    rig.core
        .emit(Event::BridgeEvent {
            pane_id: "p1".into(),
            event: json!({"kind": "turn", "phase": "start"}),
        })
        .unwrap();
    eventually("Leo working in the snapshot", || {
        rig.host
            .snapshot()
            .activity
            .get("p1")
            .is_some_and(|a| a.activity == AgentActivity::Working)
    })
    .await;
    // The session the mod said is the pane's now, and saved with it.
    eventually("the session captured", || {
        rig.host.snapshot().state.workspaces[0].panes[0]
            .session_id
            .as_deref()
            == Some("s1")
    })
    .await;
    let mut changed = false;
    while let Ok(said) = news.try_recv() {
        changed |= said == News::Changed;
    }
    assert!(changed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_that_moves_nothing_publishes_nothing() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("Leo mounted", || {
        locked(&rig.host.0.model).panes.machines.contains_key("p1")
    })
    .await;
    let report = |event: serde_json::Value| {
        rig.host.feed(Event::BridgeEvent {
            pane_id: "p1".into(),
            event,
        });
    };
    report(json!({"kind": "hello", "sessionId": "s1", "model": null, "version": "2.1"}));
    report(json!({"kind": "turn", "phase": "start"}));
    assert_eq!(
        rig.host.snapshot().activity.get("p1").map(|a| a.activity),
        Some(AgentActivity::Working)
    );
    // Let the launch finish its own writes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let before = rig.host.snapshot();
    let mut news = rig.host.news();
    // Twenty tool calls: the agent works on, nothing the engines read moved.
    for n in 0..20 {
        report(
            json!({"kind": "tool", "phase": "start", "toolUseId": format!("t{n}"),
            "tool": "Bash", "input": {"command": "ls"}}),
        );
        report(json!({"kind": "tool", "phase": "end", "toolUseId": format!("t{n}")}));
    }
    assert!(
        Arc::ptr_eq(&before, &rig.host.snapshot()),
        "the same snapshot"
    );
    assert!(news.try_recv().is_err(), "and nobody was told");
    // The turn ends: that is news.
    report(
        json!({"kind": "turn", "phase": "end", "reason": "answer", "durationMs": 10,
        "answer": "Done."}),
    );
    assert!(!Arc::ptr_eq(&before, &rig.host.snapshot()));
    let mut said = Vec::new();
    while let Ok(news) = news.try_recv() {
        said.push(news);
    }
    assert!(said.contains(&News::Changed), "{said:?}");
    // And the agent's last word, for whoever notifies the person.
    assert!(
        said.iter()
            .any(|news| matches!(news, News::Notify { pane, body, .. } if pane == "p1" && body == "Done.")),
        "{said:?}"
    );
}

#[test]
fn what_a_pane_printed_is_handed_over_as_one_burst() {
    let printed = Printed::default();
    assert_eq!(printed.take(), None);
    assert_eq!(printed.last(), None);
    printed.note(1_000);
    printed.note(1_040);
    printed.note(1_250);
    assert_eq!(printed.last(), Some(1_250));
    assert_eq!(printed.take(), Some((1_000, 1_250)));
    // Handed over once; when it last printed is still known.
    assert_eq!(printed.take(), None);
    assert_eq!(printed.last(), Some(1_250));
    printed.note(2_000);
    assert_eq!(printed.take(), Some((2_000, 2_000)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shell_s_output_reaches_its_machine_with_the_tick() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the plain shell mounted", || {
        locked(&rig.host.0.model).panes.machines.contains_key("p2")
    })
    .await;
    // The shell printed at its start (the fake's prompt): its machine has
    // the burst once the host looked.
    eventually("the burst handed over", || {
        locked(&rig.host.0.model)
            .panes
            .machines
            .get("p2")
            .is_some_and(|machine| machine.watcher.burst().is_some())
    })
    .await;
}

#[test]
fn the_state_file_is_compact_and_the_owner_s_alone() {
    let root = scratch("engine-host-file");
    let file = root.join("deep").join("state.json");
    write_state_file(
        &file,
        &json!({"workspaces": [], "settings": {"locale": "fr"}}),
    )
    .unwrap();
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(!raw.contains('\n'), "{raw}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Written over, whole.
    write_state_file(&file, &json!({"workspaces": [1]})).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        r#"{"workspaces":[1]}"#
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shell_that_ends_stays_down_until_restarted() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;
    rig.shells.exit("p2");
    eventually("p2 exited", || {
        locked(&rig.host.0.model).panes.status.get("p2") == Some(&PaneStatus::Exited)
    })
    .await;
    // Another change to the state does not start it again.
    rig.host
        .apply(ModelCmd::UpdateWorkspace {
            ws: "shop".into(),
            patch: crate::state::WorkspacePatch::new(|ws| ws.name = "Shop".into()),
        })
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        rig.shells.spawned().iter().filter(|id| *id == "p2").count(),
        1
    );
    // A restart gives it a new id, and a new shell.
    let restarted = rig
        .host
        .apply(ModelCmd::RespawnPane {
            ws: "shop".into(),
            pane: "p2".into(),
            patch: PanePatch::none(),
            fresh: true,
        })
        .unwrap();
    let next = restarted.pane.expect("a new id");
    eventually("the restarted shell", || {
        rig.shells.spawned().contains(&next)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_the_agents_and_refuses_what_only_a_window_does() {
    let rig = rig(Some(shop()));
    rig.host.start();
    let listed = rig
        .host
        .answer("workspace_list", json!({}), false)
        .await
        .unwrap();
    assert!(listed.to_string().contains("shop"), "{listed}");
    assert_eq!(
        rig.host.apply(ModelCmd::Detach("shop".into())),
        Err(writes::NO_WINDOW.to_string())
    );
    assert!(
        rig.host
            .answer("nothing_like_this", json!({}), true)
            .await
            .is_err()
    );
    assert!(rig.host.has_pane("p1"));
    assert!(!rig.host.has_pane("zz"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_made_by_the_person_opens_and_its_panes_start() {
    let rig = rig(None);
    rig.host.start();
    let made = rig
        .host
        .edit(StoreAction::AddWorkspace(crate::state::WorkspaceDraft {
            name: "Blog".into(),
            cwd: "/srv/blog".into(),
            layout: 1,
            agents: vec![AgentId::Claude],
            ..Default::default()
        }));
    let ws = made.workspace.expect("a workspace");
    assert!(rig.host.snapshot().opened.contains(&ws));
    let panes = rig.host.snapshot().state.workspaces[0].panes.len();
    assert_eq!(panes, 1);
    eventually("its pane started", || rig.shells.spawned().len() == 1).await;
}

/// A workspace made with its orchestrator: the pane is never started as a
/// plain Claude Code, before its role was known. Its role is what gives it
/// Hangar's tools (the `--mcp-config` file the shell is handed), and
/// nothing gives them to a pane that already runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_workspace_s_orchestrator_starts_with_its_role() {
    let rig = rig(None);
    rig.host.start();
    for n in 0..24 {
        let folder = scratch("engine-host-ws");
        let ws = rig
            .host
            .add_workspace(&format!("Project {n}"), folder.to_str().unwrap(), true)
            .unwrap();
        let snapshot = rig.host.snapshot();
        let made = snapshot.state.workspaces.iter().find(|w| w.id == ws);
        let role = made.and_then(|w| w.panes.first()).and_then(|p| p.role);
        assert_eq!(role, Some(crate::state::types::PaneRole::Orchestrator));
    }
    eventually("every orchestrator started", || {
        rig.shells.spawned().len() == 24
    })
    .await;
    for spawn in locked(&rig.shells.spawned).iter() {
        assert!(
            spawn.mcp_config.is_some(),
            "{} started without Hangar's tools",
            spawn.name
        );
    }
}

/// A pane that leaves the state while its shell is starting (a worker's
/// start undone, a workspace removed right after it was made): the kill
/// that comes with the removal finds no shell yet, so the launch takes
/// down the one it started.
#[tokio::test(flavor = "multi_thread")]
async fn a_pane_removed_while_its_shell_starts_is_not_left_running() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;
    *locked(&rig.shells.slow) = Duration::from_millis(250);
    let added = rig
        .host
        .apply(ModelCmd::AddPane {
            ws: "shop".into(),
            agent: Some(AgentId::Shell),
            near: None,
            dir: None,
            cwd: None,
            name: Some("Ada".into()),
            init: None,
        })
        .unwrap();
    let ada = added.pane.expect("a pane");
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(!rig.shells.alive(&ada), "its shell is still starting");
    rig.host
        .apply(ModelCmd::ClosePane {
            ws: "shop".into(),
            pane: ada.clone(),
        })
        .unwrap();
    eventually("the shell started all the same", || {
        rig.shells.spawned().contains(&ada)
    })
    .await;
    eventually("and was taken down", || !rig.shells.alive(&ada)).await;
    // Its neighbours were left alone.
    assert!(rig.shells.alive("p1") && rig.shells.alive("p2"));
}

/// What was queued for a pane before it had a machine (a worker's brief is
/// queued the moment its pane is added, its machine made two task hops
/// later) is known once it has one.
#[tokio::test(flavor = "multi_thread")]
async fn what_was_queued_before_a_pane_was_mounted_is_not_lost() {
    let rig = rig(Some(shop()));
    hangar_core::bridge::bridge_enqueue(&rig.core, "p1".into(), "the brief".into(), None);
    rig.host.start();
    eventually("the pane's queue is known", || {
        locked(&rig.host.0.model)
            .panes
            .insights
            .get("p1")
            .queue
            .len()
            == 1
    })
    .await;
    hangar_core::bridge::bridge_cancel(&rig.core, "p1".into(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_save_on_the_way_out_and_every_shell_down() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;
    rig.host
        .apply(ModelCmd::UpdateWorkspace {
            ws: "shop".into(),
            patch: crate::state::WorkspacePatch::new(|ws| ws.name = "Renamed".into()),
        })
        .unwrap();
    rig.host.shutdown().await;
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&rig.file).unwrap()).unwrap();
    assert_eq!(saved["workspaces"][0]["name"], "Renamed");
    assert!(locked(&rig.shells.alive).is_empty());
}

/// Core's own PTYs: a real shell, typed into and read back from its screen.
#[cfg(unix)]
#[test]
fn a_real_shell_is_typed_into_and_read_back() {
    let ptys = CorePtys::new(Arc::new(hangar_core::pty::PtyManager::default()));
    let printed = Arc::new(Mutex::new(0usize));
    let sink: TermSink = {
        let printed = printed.clone();
        Arc::new(move |event| {
            if let TermEvent::Output(_) = event {
                *locked(&printed) += 1;
            }
        })
    };
    let cwd = scratch("engine-pty");
    ptys.spawn(
        Spawn {
            id: "real".into(),
            cwd: cwd.to_string_lossy().into_owned(),
            name: "Real".into(),
            agent: "shell".into(),
            ..Spawn::default()
        },
        sink,
    )
    .expect("a shell");
    assert!(ptys.alive("real"));
    ptys.write("real", "echo hangar-$((40+2))\r").unwrap();
    let mut seen = false;
    for _ in 0..300 {
        if ptys
            .lines("real", 50)
            .unwrap_or_default()
            .iter()
            .any(|line| line.trim() == "hangar-42")
        {
            seen = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(seen, "{:?}", ptys.lines("real", 50));
    assert!(*locked(&printed) > 0);
    ptys.kill("real").unwrap();
    assert!(!ptys.alive("real"));
    assert!(ptys.lines("real", 5).is_none());
}

/// A workspace as a desktop that ran it on this machine hands it over:
/// with the desktop's presentation of it, and a view among its panes.
fn handed_over(cwd: &std::path::Path) -> Workspace {
    use crate::state::layout::{insert_leaf, preset_tree};
    use crate::state::types::{DropZone, ViewPane, WindowBounds, WorkspaceKit};
    let terminals = preset_tree(&["d1".to_string(), "d2".to_string()]);
    Workspace {
        tree: Some(insert_leaf(&terminals, "d1", "view", DropZone::Bottom)),
        view_panes: vec![ViewPane::new("view")],
        node: Some("n1".into()),
        hosted: Some(true),
        folder_id: Some("work".into()),
        detached: Some(true),
        window_bounds: Some(WindowBounds::default()),
        chat_view: Some(true),
        kits: Some(vec![WorkspaceKit::default()]),
        handover: Some(crate::state::types::Handover::Moved),
        theme_id: Some("paper".into()),
        ..workspace(
            "docs",
            &cwd.to_string_lossy(),
            vec![
                Pane {
                    session_id: Some("s-1".into()),
                    ..pane("d1", "Leo", AgentId::Claude)
                },
                pane("d2", "Bo", AgentId::Shell),
            ],
        )
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_handed_over_is_kept_without_the_desktop_s_presentation_and_started() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;
    let cwd = scratch("engine-adopt");
    let handed = handed_over(&cwd);
    let before = rig.host.revision();
    assert_eq!(rig.host.adoptable(&handed), Ok(true));
    assert_eq!(rig.host.adopt(handed.clone()).as_deref(), Ok("docs"));
    assert_eq!(rig.host.revision(), before + 1);

    let snapshot = rig.host.snapshot();
    assert!(snapshot.opened.contains("docs"));
    let kept = snapshot
        .state
        .workspaces
        .iter()
        .find(|ws| ws.id == "docs")
        .unwrap();
    let ids: Vec<&str> = kept.panes.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["d1", "d2"], "under the desktop's ids");
    // Leo is a pane of the shop here: the one handed over gets a free name.
    assert_ne!(kept.panes[0].name, "Leo");
    assert_eq!(kept.panes[1].name, "Bo");
    assert_eq!(kept.panes[0].session_id.as_deref(), Some("s-1"));
    let expected = Workspace {
        panes: kept.panes.clone(),
        tree: Some(crate::state::layout::preset_tree(&[
            "d1".to_string(),
            "d2".to_string(),
        ])),
        view_panes: Vec::new(),
        node: None,
        hosted: None,
        folder_id: None,
        detached: None,
        window_bounds: None,
        chat_view: None,
        kits: None,
        handover: None,
        ..handed.clone()
    };
    assert_eq!(*kept, expected);
    eventually("its panes started", || {
        let spawned = rig.shells.spawned();
        spawned.contains(&"d1".to_string()) && spawned.contains(&"d2".to_string())
    })
    .await;

    // Handed over again: there already, nothing changes, nothing restarts.
    assert_eq!(rig.host.adoptable(&handed), Ok(false));
    assert_eq!(rig.host.adopt(handed).as_deref(), Ok("docs"));
    assert_eq!(rig.host.revision(), before + 1);
    assert_eq!(rig.shells.spawned().len(), 4);
    assert!(locked(&rig.shells.killed).is_empty());
}

#[test]
fn a_workspace_that_cannot_run_here_is_refused_whole() {
    let rig = rig(Some(shop()));
    let cwd = scratch("engine-adopt-refused");
    let good = handed_over(&cwd);
    let refused = |edit: fn(&mut Workspace)| {
        let mut ws = good.clone();
        edit(&mut ws);
        let asked = rig.host.adoptable(&ws);
        assert_eq!(rig.host.adopt(ws).err(), asked.clone().err());
        asked.expect_err("refused")
    };
    assert!(refused(|ws| ws.cwd = "/no/such/folder".into()).contains("/no/such/folder"));
    assert!(refused(|ws| ws.cwd = "relative".into()).contains("absolute"));
    refused(|ws| ws.id = String::new());
    refused(|ws| ws.id = "two words".into());
    refused(|ws| ws.panes.clear());
    refused(|ws| ws.panes[0].id = "a/b".into());
    refused(|ws| ws.panes[1].id = "d1".into());
    // A pane id the host already has, in another workspace.
    refused(|ws| ws.panes[1].id = "p1".into());
    refused(|ws| ws.panes[0].cwd = Some("docs".into()));
    refused(|ws| ws.panes[0].node = Some("n2".into()));
    assert_eq!(rig.host.revision(), 0);
    assert_eq!(rig.host.snapshot().state.workspaces.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn what_a_mirror_asks_is_applied_and_the_shells_follow() {
    use crate::state::hosted::{PaneFields, WorkspaceFields};
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("the panes started", || rig.shells.spawned().len() == 2).await;
    let ops = vec![
        HostedOp::PatchWorkspace {
            patch: WorkspaceFields {
                name: Some("Shop".into()),
                ..WorkspaceFields::default()
            },
        },
        HostedOp::RespawnPane {
            from: "p1".into(),
            to: "p1b".into(),
            patch: PaneFields::default(),
        },
        HostedOp::PutPane {
            pane: pane("p3", "Ada", AgentId::Shell),
        },
        HostedOp::ClosePane { pane: "p2".into() },
    ];
    let before = rig.host.revision();
    rig.host.apply_ops("shop", ops.clone()).unwrap();
    let after = rig.host.revision();
    assert!(after > before);
    let state = rig.host.revised_state();
    assert_eq!(state.0, after);
    let shop = &state.1.workspaces[0];
    assert_eq!(shop.name, "Shop");
    let ids: Vec<&str> = shop.panes.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["p1b", "p3"]);
    eventually("the new shells started and the old ones killed", || {
        let spawned = rig.shells.spawned();
        let killed = locked(&rig.shells.killed).clone();
        ["p1b", "p3"]
            .iter()
            .all(|id| spawned.contains(&id.to_string()))
            && ["p1", "p2"]
                .iter()
                .all(|id| killed.contains(&id.to_string()))
    })
    .await;
    // Asked again (an answer lost on the way): nothing moves.
    rig.host.apply_ops("shop", ops).unwrap();
    assert_eq!(rig.host.revision(), after);

    // One op that cannot be applied refuses them all.
    let mixed = vec![
        HostedOp::PatchWorkspace {
            patch: WorkspaceFields {
                name: Some("Never".into()),
                cwd: Some("/no/such/folder".into()),
                ..WorkspaceFields::default()
            },
        },
        HostedOp::ClosePane { pane: "p3".into() },
    ];
    assert!(rig.host.apply_ops("shop", mixed).is_err());
    assert!(
        rig.host
            .apply_ops("nowhere", vec![HostedOp::RemoveWorkspace])
            .is_err()
    );
    // A patch is held to what a pane or a workspace put whole is held to.
    let patched = |patch: PaneFields| HostedOp::PatchPane {
        pane: "p3".into(),
        patch,
    };
    let relative = PaneFields {
        cwd: Some(Some("worktrees/a".into())),
        ..PaneFields::default()
    };
    let elsewhere = PaneFields {
        node: Some(Some("n2".into())),
        ..PaneFields::default()
    };
    let workspace = |patch: WorkspaceFields| HostedOp::PatchWorkspace { patch };
    let refused = [
        patched(relative.clone()),
        patched(elsewhere),
        HostedOp::RespawnPane {
            from: "p3".into(),
            to: "p3b".into(),
            patch: relative,
        },
        workspace(WorkspaceFields {
            extra_roots: Some(vec!["../docs".into()]),
            ..WorkspaceFields::default()
        }),
        workspace(WorkspaceFields {
            name: Some("  ".into()),
            ..WorkspaceFields::default()
        }),
    ];
    for op in refused {
        let said = format!("{op:?}");
        assert!(rig.host.apply_ops("shop", vec![op]).is_err(), "{said}");
    }
    let taken = vec![
        patched(PaneFields {
            cwd: Some(Some("/srv/worktrees/a".into())),
            ..PaneFields::default()
        }),
        patched(PaneFields {
            cwd: Some(None),
            ..PaneFields::default()
        }),
    ];
    let before = rig.host.revision();
    rig.host.apply_ops("shop", taken).unwrap();
    assert!(rig.host.revision() > before);
    let after = rig.host.revision();
    assert_eq!(rig.host.revision(), after);

    rig.host
        .apply_ops("shop", vec![HostedOp::RemoveWorkspace])
        .unwrap();
    let snapshot = rig.host.snapshot();
    assert!(snapshot.state.workspaces.is_empty());
    assert!(snapshot.opened.is_empty());
    eventually("its shells killed", || {
        let killed = locked(&rig.shells.killed).clone();
        ["p1b", "p3"]
            .iter()
            .all(|id| killed.contains(&id.to_string()))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_revision_moves_with_the_state_and_not_with_an_agent_s_activity() {
    let rig = rig(Some(shop()));
    rig.host.start();
    eventually("Leo mounted", || {
        locked(&rig.host.0.model).panes.machines.contains_key("p1")
    })
    .await;
    assert_eq!(rig.host.revision(), 0);
    let mut news = rig.host.news();
    rig.host.feed(Event::BridgeEvent {
        pane_id: "p1".into(),
        event: json!({"kind": "turn", "phase": "start"}),
    });
    eventually("Leo working", || {
        rig.host
            .snapshot()
            .activity
            .get("p1")
            .is_some_and(|a| a.activity == AgentActivity::Working)
    })
    .await;
    assert!(
        std::iter::from_fn(|| news.try_recv().ok()).any(|said| said == News::Changed),
        "those who follow are told"
    );
    assert_eq!(rig.host.revision(), 0, "nothing to save, nothing moved");
    rig.host.edit(StoreAction::UpdateWorkspace(
        "shop".into(),
        crate::state::WorkspacePatch::new(|ws| ws.name = "Renamed".into()),
    ));
    assert_eq!(rig.host.revision(), 1);
}

/// A real shell told its new size, and its screen with it.
#[cfg(unix)]
#[test]
fn a_real_shell_and_its_screen_take_a_new_size() {
    let ptys = CorePtys::new(Arc::new(hangar_core::pty::PtyManager::default()));
    let cwd = scratch("engine-pty-size");
    ptys.spawn(
        Spawn {
            id: "sized".into(),
            cwd: cwd.to_string_lossy().into_owned(),
            name: "Sized".into(),
            agent: "shell".into(),
            ..Spawn::default()
        },
        Arc::new(|_| {}),
    )
    .expect("a shell");
    assert_eq!(
        ptys.screen("sized").unwrap().len(),
        usize::from(terminals::ROWS)
    );
    ptys.resize("sized", 100, 30).unwrap();
    assert_eq!(ptys.screen("sized").unwrap().len(), 30);
    ptys.write("sized", "stty size\r").unwrap();
    let mut seen = false;
    for _ in 0..300 {
        let lines = ptys.lines("sized", 50).unwrap_or_default();
        if lines.iter().any(|line| line.trim() == "30 100") {
            seen = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(seen, "{:?}", ptys.lines("sized", 50));
    assert!(ptys.resize("nobody", 80, 24).is_err());
    ptys.kill("sized").unwrap();
}
