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
