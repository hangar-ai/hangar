//! The dialog, drawn and driven headless (`testing::Harness`), and its update
//! logic that needs no service.

// A state is built by assigning the one field a case is about: a struct
// literal with `..Default::default()` would bury it.
#![allow(clippy::field_reassign_with_default)]

use hangar_i18n::Locale;

use crate::git_flow::clone::RepoRef;
use crate::git_flow::flow::{FromGitRefusal, PaneRef, WorkspaceRef};
use crate::message::Message;

use super::testing::{Harness, messages};
use super::*;

fn parsed_ok(input: &str, name: &str) -> Parsed {
    Parsed {
        input: input.into(),
        result: ParseResult::Ok {
            repo: RepoRef {
                url: format!("https://github.com/acme/{name}.git"),
                host: "github.com".into(),
                owner: "acme".into(),
                name: name.into(),
            },
        },
    }
}

fn refused(input: &str) -> Parsed {
    Parsed {
        input: input.into(),
        result: ParseResult::Refused {
            reason: format!("\"{input}\" is not a repository address"),
        },
    }
}

fn sent(messages: &[Message], wanted: impl Fn(&Msg) -> bool) -> bool {
    messages.iter().any(|message| match message {
        Message::WorkspaceDialog(msg) => wanted(msg),
        _ => false,
    })
}

fn closed(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|message| matches!(message, Message::Intent(Intent::CloseOverlay)))
}

// ── the folder form, drawn ────────────────────────────────────────────

/// The form with « More options » unfolded on the panes picked by hand: the
/// whole of the dialog as it was before the orchestrator was the default.
fn custom() -> State {
    let mut st = State::default();
    st.folder.more = true;
    st.folder.custom = true;
    st
}

#[test]
fn the_folder_form_starts_with_an_orchestrator_and_folds_the_rest() {
    let h = Harness::new(Locale::En, "graphite");
    let st = State::default();
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "New workspace",
        "Source",
        "Existing folder",
        "From a Git repository",
        "Project folder",
        "Browse…",
        ".code-workspace…",
        "Name",
        "Starts with",
        "One Claude Code pane, the orchestrator: tell it what you want, it plans the work and hands it to workers",
        "More options",
        "Cancel",
        "Create workspace",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    for folded in [
        "Shell",
        "Number of panes",
        "Agent split",
        "Worktree commands",
    ] {
        assert!(ui.find(folded).is_err(), "{folded} is folded away");
    }
    h.shot(&mut ui, "dialog-folder-default-en-graphite");
    ui.click("More options").expect("the fold is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::MoreOptions)));

    // Unfolded: the default stays an orchestrator until panes are picked.
    let mut st = State::default();
    st.folder.more = true;
    let mut ui = h.show(view(&st, h.ctx()));
    for text in ["Panes", "One orchestrator", "Panes of my choice", "Shell"] {
        assert!(ui.find(text).is_ok(), "{text} is under More options");
    }
    assert!(ui.find("Number of panes").is_err());
    ui.click("Panes of my choice")
        .expect("the segment is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Custom(true))));
}

#[test]
fn the_folder_form_has_every_field_of_the_web_dialog() {
    let h = Harness::new(Locale::En, "graphite");
    let st = custom();
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "New workspace",
        "Source",
        "Existing folder",
        "From a Git repository",
        "Project folder",
        "Browse…",
        ".code-workspace…",
        "Name",
        "4 panes, as chosen under More options",
        "Shell",
        "2 detected on this machine",
        "Number of panes",
        "Agent split",
        "Claude Code",
        "Arrangement",
        "Claude account",
        "Worktree commands",
        "Cancel",
        "Create workspace",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    h.shot(&mut ui, "dialog-folder-en-graphite");
}

#[test]
fn an_agent_that_is_not_installed_says_so() {
    let mut h = Harness::new(Locale::En, "graphite");
    let st = custom();
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("not installed").is_err());
    }
    // Claude Code missing from this machine's PATH.
    h.model.ui.claude_installed = false;
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("not installed").is_ok());
}

#[test]
fn without_claude_code_the_default_is_a_shell_and_the_form_says_why() {
    let mut h = Harness::new(Locale::En, "graphite");
    h.model.ui.claude_installed = false;
    let st = State::default();
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "One plain shell",
        "Claude Code is not installed on this machine: the workspace starts with a shell instead of an orchestrator.",
    ] {
        assert!(ui.find(text).is_ok(), "{text}");
    }
    h.shot(&mut ui, "dialog-folder-no-claude-en-graphite");
}

#[test]
fn the_default_orchestrator_starts_alone_in_its_new_workspace() {
    let mut h = Harness::new(Locale::Fr, "graphite");
    let st = State::default();
    {
        let mut ui = h.show(view(&st, h.ctx()));
        for text in ["Démarre avec", "Plus d'options"] {
            assert!(ui.find(text).is_ok(), "{text}");
        }
        h.shot(&mut ui, "dialog-folder-default-fr-graphite");
    }
    let mut st = State::default();
    st.folder.cwd = "/home/me/dev/blog".into();
    let before = h.model.state().workspaces.len();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let workspaces = &h.model.state().workspaces;
    assert_eq!(workspaces.len(), before + 1);
    let made = workspaces.last().expect("the workspace is made");
    assert_eq!(made.cwd, "/home/me/dev/blog");
    assert_eq!(made.panes.len(), 1);
    let pane = &made.panes[0];
    assert_eq!(pane.agent, AgentId::Claude);
    assert_eq!(pane.role, Some(crate::state::types::PaneRole::Orchestrator));
    assert_eq!(
        pane.model.as_deref(),
        Some(
            crate::state::roles::orchestration_defaults()
                .orchestrator_model
                .as_str()
        )
    );
    // It is the one in front, in the panes view.
    assert_eq!(
        h.model.state().active_workspace_id.as_deref(),
        Some(made.id.as_str())
    );
    assert_eq!(made.chat_view, None);
    // The other workspaces are left as they were.
    for ws in &workspaces[..before] {
        assert!(ws.panes.iter().all(|pane| pane.role.is_none()), "{}", ws.id);
    }
}

#[test]
fn the_orchestrator_made_by_the_dialog_is_saved_and_read_back() {
    let mut h = Harness::new(Locale::En, "graphite");
    let mut st = State::default();
    st.folder.cwd = "/home/me/dev/blog".into();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let written = h.model.to_json();
    let (state, kept) = crate::state::read_state_keeping(written.clone());
    let mut again = Model::default();
    again.hydrate_read(state, kept);
    let made = again
        .state()
        .workspaces
        .iter()
        .find(|ws| ws.cwd == "/home/me/dev/blog")
        .expect("read back");
    assert_eq!(made.panes.len(), 1);
    assert_eq!(made.panes[0].agent, AgentId::Claude);
    assert_eq!(
        made.panes[0].role,
        Some(crate::state::types::PaneRole::Orchestrator)
    );
    assert_eq!(again.to_json(), written);
}

#[test]
fn without_claude_code_the_dialog_makes_one_shell_with_no_role() {
    let mut h = Harness::new(Locale::En, "graphite");
    h.model.ui.claude_installed = false;
    let mut st = State::default();
    st.folder.cwd = "/home/me/dev/blog".into();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let made = h.model.state().workspaces.last().expect("made");
    assert_eq!(made.panes.len(), 1);
    assert_eq!(made.panes[0].agent, AgentId::Shell);
    assert_eq!(made.panes[0].role, None);
}

#[test]
fn more_options_makes_every_layout_as_it_is_chosen_and_no_orchestrator() {
    for layout in folder::LAYOUTS {
        let mut h = Harness::new(Locale::En, "graphite");
        let mut st = State::default();
        st.folder.cwd = format!("/home/me/dev/l{layout}");
        let _ = form(&mut st, &mut h.model, Msg::MoreOptions);
        let _ = form(&mut st, &mut h.model, Msg::Custom(true));
        let _ = form(&mut st, &mut h.model, Msg::Layout(layout));
        let _ = form(&mut st, &mut h.model, Msg::More(AgentId::Claude));
        // Folding the section keeps what was chosen in it.
        let _ = form(&mut st, &mut h.model, Msg::MoreOptions);
        let mut wanted = vec![AgentId::Shell; usize::from(layout)];
        wanted[0] = AgentId::Claude;
        assert_eq!(st.folder.assignments, wanted);
        let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
        let made = h.model.state().workspaces.last().expect("made");
        assert_eq!(made.cwd, st.folder.cwd);
        let agents: Vec<_> = made.panes.iter().map(|pane| pane.agent).collect();
        assert_eq!(agents, wanted, "{layout} panes");
        assert!(
            made.panes.iter().all(|pane| pane.role.is_none()),
            "{layout} panes: no role, as before"
        );
    }
    // Back to « One orchestrator »: the default again.
    let mut h = Harness::new(Locale::En, "graphite");
    let mut st = custom();
    st.folder.cwd = "/home/me/dev/back".into();
    let _ = form(&mut st, &mut h.model, Msg::Custom(false));
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let made = h.model.state().workspaces.last().expect("made");
    assert_eq!(made.panes.len(), 1);
    assert_eq!(
        made.panes[0].role,
        Some(crate::state::types::PaneRole::Orchestrator)
    );
}

#[test]
fn a_folder_that_is_not_a_repository_still_gets_an_orchestrator_and_a_note() {
    let note = "This folder is not a Git repository: the workers will share it. Run git init in it to give each worker a worktree of its own.";
    let mut h = Harness::new(Locale::En, "graphite");
    let mut st = State::default();
    let _ = form(&mut st, &mut h.model, Msg::Cwd("/home/me/notes".into()));
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find(note).is_err(), "not known yet");
    }
    // An answer for another folder is not about this one.
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::RepoChecked {
            for_cwd: "/home/me/old".into(),
            repo: false,
        },
    );
    assert!(!st.folder.not_a_repo());
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::RepoChecked {
            for_cwd: "/home/me/notes".into(),
            repo: false,
        },
    );
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find(note).is_ok());
        h.shot(&mut ui, "dialog-folder-no-git-en-graphite");
    }
    // A repository needs no note.
    let mut other = State::default();
    other.folder = Folder {
        cwd: "/home/me/notes".into(),
        repo: Some(("/home/me/notes".into(), true)),
        ..Folder::default()
    };
    {
        let mut ui = h.show(view(&other, h.ctx()));
        assert!(ui.find(note).is_err(), "a repository needs no note");
    }
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let made = h.model.state().workspaces.last().expect("made");
    assert_eq!(made.cwd, "/home/me/notes");
    assert_eq!(
        made.panes[0].role,
        Some(crate::state::types::PaneRole::Orchestrator)
    );
}

#[test]
fn a_folder_that_is_not_a_repository_is_told_in_french_too() {
    let h = Harness::new(Locale::Fr, "github-light");
    let mut st = State::default();
    st.folder.cwd = "/home/me/notes".into();
    st.folder.repo = Some(("/home/me/notes".into(), false));
    let mut ui = h.show(view(&st, h.ctx()));
    let fr = Translator::new(Locale::Fr);
    for key in [Key::CreateStartOrchestrator, Key::CreateStartNoGit] {
        let text = fr.t(key);
        assert!(ui.find(text.as_ref()).is_ok(), "{text}");
    }
    h.shot(&mut ui, "dialog-folder-no-git-fr-github-light");
}

#[test]
fn the_form_speaks_french_in_a_light_theme() {
    let h = Harness::new(Locale::Fr, "github-light");
    let st = custom();
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "Nouveau workspace",
        "Dossier du projet",
        "Répartition des agents",
        "Créer le workspace",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    h.shot(&mut ui, "dialog-folder-fr-github-light");
}

#[test]
fn the_create_button_waits_for_a_folder() {
    let h = Harness::new(Locale::En, "graphite");
    let st = State::default();
    let mut ui = h.show(view(&st, h.ctx()));
    ui.click("Create workspace").expect("the button is drawn");
    assert!(!sent(&messages(ui), |msg| matches!(msg, Msg::Submit(_))));

    let mut st = State::default();
    st.folder.cwd = "/home/me/dev/shop".into();
    let mut ui = h.show(view(&st, h.ctx()));
    ui.click("Create workspace").expect("the button is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Submit(_))));
}

#[test]
fn the_source_control_switches_to_the_git_form() {
    let h = Harness::new(Locale::En, "graphite");
    let st = State::default();
    let mut ui = h.show(view(&st, h.ctx()));
    ui.click("From a Git repository")
        .expect("the segment is drawn");
    assert!(sent(&messages(ui), |msg| matches!(
        msg,
        Msg::Source(Source::Git)
    )));
}

#[test]
fn the_layout_preview_names_the_panes_with_the_next_free_names() {
    let h = Harness::new(Locale::En, "graphite");
    let st = custom();
    let mut ui = h.show(view(&st, h.ctx()));
    // Ava, Max and Leo are taken by the harness' workspaces.
    for name in ["Mia", "Sam", "Zoe", "Eli"] {
        assert!(ui.find(name).is_ok(), "{name}");
    }
    assert!(ui.find("Ava").is_err());
}

#[test]
fn a_cell_of_the_preview_asks_to_change_its_agent() {
    let h = Harness::new(Locale::En, "graphite");
    let st = custom();
    let mut ui = h.show(view(&st, h.ctx()));
    ui.click("Mia").expect("the cell is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Cycle(0))));
}

#[test]
fn a_workspace_file_shows_its_roots_and_the_missing_one_cannot_be_picked() {
    let h = Harness::new(Locale::En, "graphite");
    let mut st = State::default();
    st.folder.cwd = "/work/api".into();
    st.folder.roots = vec![
        Root {
            name: "api".into(),
            path: "/work/api".into(),
            exists: true,
        },
        Root {
            name: "web".into(),
            path: "/work/web".into(),
            exists: true,
        },
        Root {
            name: "docs".into(),
            path: "/work/docs".into(),
            exists: false,
        },
    ];
    let mut ui = h.show(view(&st, h.ctx()));
    for text in ["main", "extra", "missing"] {
        assert!(ui.find(text).is_ok(), "{text}");
    }
    h.shot(&mut ui, "dialog-folder-roots-en-graphite");
    ui.click("web").expect("the root is drawn");
    assert!(sent(&messages(ui), |msg| matches!(
        msg,
        Msg::PickRoot(path) if path == "/work/web"
    )));

    let mut ui = h.show(view(&st, h.ctx()));
    ui.click("docs").expect("the root is drawn");
    assert!(!sent(&messages(ui), |msg| matches!(msg, Msg::PickRoot(_))));
}

#[test]
fn a_workspace_file_found_beside_the_folder_is_offered_not_applied() {
    let h = Harness::new(Locale::En, "graphite");
    let mut st = State::default();
    st.folder.cwd = "/work".into();
    st.folder.suggestion = Some("/work/app.code-workspace".into());
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("app.code-workspace sits in this folder").is_ok());
    ui.click("Open it").expect("the button is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::UseSuggestion)));
}

// ── the Git form, drawn ───────────────────────────────────────────────

fn git_state(repo: &str, parsed: Option<Parsed>) -> State {
    let mut st = State::default();
    st.source = Source::Git;
    st.git.repo = repo.into();
    st.git.parsed = parsed;
    st
}

#[test]
fn the_git_form_shows_where_the_clone_will_land_and_starts_when_ready() {
    let h = Harness::new(Locale::En, "graphite");
    // Beside the workspace in front: /home/me/dev/hangar → /home/me/dev.
    let st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "Repository",
        "Clone into",
        "It will be cloned to /home/me/dev/shop",
        "Clone and create workspace",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    h.shot(&mut ui, "dialog-git-ready-en-graphite");
    ui.click("Clone and create workspace")
        .expect("the button is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Submit(_))));
}

#[test]
fn the_git_form_is_not_startable_until_the_address_is_read_and_good() {
    let h = Harness::new(Locale::En, "graphite");
    for (repo, parsed) in [
        ("", None),
        ("acme/shop", None),
        ("nonsense", Some(refused("nonsense"))),
    ] {
        let st = git_state(repo, parsed);
        let mut ui = h.show(view(&st, h.ctx()));
        ui.click("Clone and create workspace")
            .expect("the button is drawn");
        assert!(
            !sent(&messages(ui), |msg| matches!(msg, Msg::Submit(_))),
            "{repo:?}"
        );
    }
}

#[test]
fn a_refused_address_says_why_under_the_field() {
    let h = Harness::new(Locale::En, "graphite");
    let st = git_state("nonsense", Some(refused("nonsense")));
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("\"nonsense\" is not a repository address").is_ok());
    h.shot(&mut ui, "dialog-git-refused-en-graphite");
}

#[test]
fn an_address_read_for_an_older_text_is_not_believed() {
    let h = Harness::new(Locale::En, "graphite");
    // The field says one thing, the reading is of another: still checking.
    let st = git_state("acme/shop2", Some(parsed_ok("acme/shop", "shop")));
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("Reading the address…").is_ok());
}

#[test]
fn a_running_clone_shows_its_progress_and_offers_to_stop_it() {
    let h = Harness::new(Locale::En, "graphite");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    st.git.running = true;
    st.git.job = Some("job-1".into());
    st.git.progress = Some(CloneProgress {
        job_id: "job-1".into(),
        phase: "Receiving objects".into(),
        percent: Some(45),
    });
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("Receiving objects 45 %").is_ok());
    assert!(ui.find("Cloning…").is_ok());
    h.shot(&mut ui, "dialog-git-running-en-graphite");
    ui.click("Cancel the clone")
        .expect("the stop button is drawn");
    assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Stop)));
}

#[test]
fn a_clone_that_has_not_started_says_so() {
    let h = Harness::new(Locale::En, "graphite");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    st.git.running = true;
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("Starting the clone…").is_ok());
}

#[test]
fn a_failed_clone_shows_the_failure_in_the_form() {
    let h = Harness::new(Locale::Fr, "graphite");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    st.git.error = Some("Impossible de créer le workspace : dépôt introuvable".into());
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(
        ui.find("Impossible de créer le workspace : dépôt introuvable")
            .is_ok()
    );
    h.shot(&mut ui, "dialog-git-failed-fr-graphite");
}

// ── the kits dialog ───────────────────────────────────────────────────

#[test]
fn the_kits_dialog_names_the_workspace_and_closes_with_done() {
    let h = Harness::new(Locale::En, "graphite");
    let ws = "w1".to_string();
    let mut st = State::default();
    st.kits = kits_dialog::KitsDialog::open(&h.model, &ws);
    let mut ui = h.show(kits_dialog_view(&st, h.ctx(), &ws));
    assert!(ui.find("Done").is_ok());
    h.shot(&mut ui, "dialog-kits-en-graphite");
    ui.click("Done").expect("the button is drawn");
    assert!(closed(&messages(ui)));
}

/// On a workspace its node runs, the dialog says what a kit chosen there
/// is: its tabs in this window, and nothing the node's agents start with.
#[test]
fn the_kits_dialog_of_a_mirror_says_its_kits_stay_in_this_window() {
    let plain = Harness::new(Locale::En, "graphite");
    let ws = "w1".to_string();
    let name = plain.model.workspace(&ws).unwrap().name.clone();
    let note = format!(
        "{name} runs on vps, which starts its agents without kits. \
         A kit chosen here only shows its tabs in this window."
    );
    let mut st = State::default();
    st.kits = kits_dialog::KitsDialog::open(&plain.model, &ws);
    {
        let mut ui = plain.show(kits_dialog_view(&st, plain.ctx(), &ws));
        assert!(
            ui.find(note.as_str()).is_err(),
            "not on one of this computer"
        );
    }
    let h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![crate::nodes::testing::machine(
            "vps",
            "vps",
            crate::state::types::MachineKind::Cloud,
        )];
        state.workspaces[0].node = Some("vps".into());
        state.workspaces[0].hosted = Some(true);
    });
    let mut ui = h.show(kits_dialog_view(&st, h.ctx(), &ws));
    assert!(ui.find(note.as_str()).is_ok());
    assert!(
        ui.find(
            "Enable the kits this workspace uses. Their skills, commands, agents and \
             MCP servers load in its Claude Code panes."
        )
        .is_err(),
        "they do not load in the node's panes"
    );
    h.shot(&mut ui, "dialog-kits-mirror-en-graphite");
}

// ── the update logic that needs no service ────────────────────────────

fn on_top() -> Model {
    let mut model = Model::default();
    model.ui.overlays.push(Overlay::WorkspaceDialog);
    model
}

#[test]
fn nothing_of_the_form_changes_while_a_clone_runs() {
    let mut m = on_top();
    let mut st = git_state("acme/shop", None);
    st.git.running = true;
    st.name = "shop".into();
    st.git.parent = "/x".into();
    for msg in [
        Msg::Source(Source::Folder),
        Msg::Name("other".into()),
        Msg::Repo("other/repo".into()),
        Msg::Parent("/y".into()),
        Msg::Dismiss,
    ] {
        let task = form(&mut st, &mut m, msg);
        assert_eq!(task.units(), 0);
    }
    assert_eq!(st.source, Source::Git);
    assert_eq!(st.name, "shop");
    assert_eq!(st.git.repo, "acme/shop");
    assert_eq!(st.git.parent, "/x");
}

#[test]
fn cancel_and_dismiss_close_the_dialog_when_it_is_in_front() {
    let mut m = on_top();
    let mut st = State::default();
    assert_eq!(form(&mut st, &mut m, Msg::Cancel).units(), 1);
    assert_eq!(form(&mut st, &mut m, Msg::Dismiss).units(), 1);
    // Something else is in front: closing would close that.
    m.ui.overlays.push(Overlay::Palette);
    assert_eq!(form(&mut st, &mut m, Msg::Cancel).units(), 0);
}

#[test]
fn typing_an_address_asks_for_it_to_be_read_after_a_pause_and_only_the_last_counts() {
    let mut m = on_top();
    let mut st = State::default();
    st.source = Source::Git;
    assert_eq!(form(&mut st, &mut m, Msg::Repo("a".into())).units(), 1);
    assert_eq!(st.git.generation, 1);
    assert_eq!(form(&mut st, &mut m, Msg::Repo("ac".into())).units(), 1);
    assert_eq!(st.git.generation, 2);
    // The pause that ended for the first edit is not believed.
    assert_eq!(form(&mut st, &mut m, Msg::ParseDue(1)).units(), 0);
    assert_eq!(form(&mut st, &mut m, Msg::ParseDue(2)).units(), 1);
    // Cleared: nothing to read.
    assert_eq!(form(&mut st, &mut m, Msg::Repo("  ".into())).units(), 0);
    assert_eq!(form(&mut st, &mut m, Msg::ParseDue(3)).units(), 0);
}

#[test]
fn a_reading_that_arrives_late_is_ignored() {
    let mut m = on_top();
    let mut st = State::default();
    st.source = Source::Git;
    st.git.repo = "acme/shop".into();
    st.git.generation = 4;
    let late = Msg::Parsed {
        generation: 3,
        input: "acme/sho".into(),
        result: ParseResult::Refused { reason: "x".into() },
    };
    let _ = form(&mut st, &mut m, late);
    assert!(st.git.parsed.is_none());
    let on_time = Msg::Parsed {
        generation: 4,
        input: "acme/shop".into(),
        result: parsed_ok("acme/shop", "shop").result,
    };
    let _ = form(&mut st, &mut m, on_time);
    assert!(matches!(st.git.parse(), Some(ParseResult::Ok { .. })));
}

#[test]
fn a_new_layout_cuts_or_pads_the_agents() {
    let mut m = on_top();
    let mut st = State::default();
    let _ = form(&mut st, &mut m, Msg::More(AgentId::Claude));
    let _ = form(&mut st, &mut m, Msg::Layout(2));
    assert_eq!(st.folder.assignments, [AgentId::Claude, AgentId::Shell]);
    let _ = form(&mut st, &mut m, Msg::Layout(8));
    assert_eq!(st.folder.assignments.len(), 8);
    // Not one of the sizes offered: refused.
    let _ = form(&mut st, &mut m, Msg::Layout(3));
    assert_eq!(st.folder.layout, 8);
    let _ = form(&mut st, &mut m, Msg::Cycle(1));
    assert_eq!(st.folder.assignments[1], AgentId::Claude);
    let _ = form(&mut st, &mut m, Msg::Less(AgentId::Claude));
    assert_eq!(st.folder.assignments[0], AgentId::Claude);
    assert_eq!(st.folder.assignments[1], AgentId::Shell);
}

#[test]
fn picking_a_folder_fills_the_name_only_while_it_is_empty() {
    let mut m = on_top();
    let mut st = State::default();
    let _ = form(
        &mut st,
        &mut m,
        Msg::Picked(Some("/home/me/dev/shop".into())),
    );
    assert_eq!(st.folder.cwd, "/home/me/dev/shop");
    assert_eq!(st.name, "shop");
    let _ = form(&mut st, &mut m, Msg::Name("My shop".into()));
    let _ = form(
        &mut st,
        &mut m,
        Msg::Picked(Some("/home/me/dev/other".into())),
    );
    assert_eq!(st.name, "My shop");
    // A cancelled dialog changes nothing.
    let _ = form(&mut st, &mut m, Msg::Picked(None));
    assert_eq!(st.folder.cwd, "/home/me/dev/other");
}

#[test]
fn a_workspace_file_makes_its_first_existing_folder_the_main_one() {
    let mut m = on_top();
    let mut st = State::default();
    st.folder.suggestion = Some("/work/My app.code-workspace".into());
    st.folder.reading = Some(7);
    let root = |name: &str, path: &str, exists: bool| Root {
        name: name.into(),
        path: path.into(),
        exists,
    };
    let _ = form(
        &mut st,
        &mut m,
        Msg::FileRead {
            read: 7,
            path: "/work/My app.code-workspace".into(),
            result: Ok(vec![
                root("gone", "/work/gone", false),
                root("api", "/work/api", true),
                root("web", "/work/web", true),
            ]),
        },
    );
    assert_eq!(st.folder.cwd, "/work/api");
    assert_eq!(st.name, "My app");
    assert_eq!(st.folder.roots.len(), 3);
    assert!(st.folder.suggestion.is_none());
    // Another root is picked by a click; a missing one is not.
    let _ = form(&mut st, &mut m, Msg::PickRoot("/work/web".into()));
    assert_eq!(st.folder.cwd, "/work/web");
    let _ = form(&mut st, &mut m, Msg::PickRoot("/work/gone".into()));
    assert_eq!(st.folder.cwd, "/work/web");
}

#[test]
fn a_workspace_file_that_cannot_be_read_says_so_and_keeps_the_folder() {
    let mut m = on_top();
    let mut st = State::default();
    st.folder.cwd = "/work".into();
    st.folder.reading = Some(3);
    let _ = form(
        &mut st,
        &mut m,
        Msg::FileRead {
            read: 3,
            path: "/work/broken.code-workspace".into(),
            result: Err("no \"folders\" list".into()),
        },
    );
    assert_eq!(
        st.folder.file_error.as_deref(),
        Some("/work/broken.code-workspace")
    );
    assert_eq!(st.folder.cwd, "/work");
    assert!(st.folder.roots.is_empty());
}

fn created(reused: bool) -> FromGitOutcome {
    FromGitOutcome::Created {
        workspace: WorkspaceRef {
            id: "w".into(),
            name: "shop".into(),
            cwd: "/home/me/dev/shop".into(),
            orchestrator: Some(PaneRef {
                id: "p".into(),
                name: "Kai".into(),
            }),
        },
        repo: RepoRef {
            url: "https://github.com/acme/shop.git".into(),
            host: "github.com".into(),
            owner: "acme".into(),
            name: "shop".into(),
        },
        mine: true,
        trusted: true,
        consented: false,
        reused_folder: reused,
        notes: vec![],
    }
}

fn running(job: &str) -> State {
    let mut st = git_state("acme/shop", None);
    st.git.running = true;
    st.git.job = Some(job.into());
    st
}

#[test]
fn a_flow_that_ended_for_another_job_is_ignored() {
    let mut m = on_top();
    let mut st = running("job-2");
    let task = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: created(false),
        },
    );
    assert_eq!(task.units(), 0);
    assert!(st.git.running);
}

#[test]
fn a_created_workspace_closes_the_dialog_and_a_reused_folder_is_told() {
    let mut m = on_top();
    let mut st = running("job-1");
    let task = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: created(false),
        },
    );
    // Only the closing: a workspace that was cloned needs no word.
    assert_eq!(task.units(), 1);
    assert!(!st.git.running && st.git.job.is_none());

    let mut st = running("job-1");
    let task = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: created(true),
        },
    );
    // The notice and the closing.
    assert_eq!(task.units(), 2);
}

#[test]
fn a_refused_flow_stays_in_the_form_with_its_failure_on_one_line() {
    let mut m = on_top();
    let mut st = running("job-1");
    let _ = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: FromGitOutcome::Refused {
                kind: FromGitRefusal::CloneFailed,
                reason: "The clone failed: repository not found\nsecond line".into(),
            },
        },
    );
    assert!(!st.git.running);
    assert_eq!(
        st.git.error.as_deref(),
        Some("Could not create the workspace: The clone failed: repository not found")
    );
}

#[test]
fn a_cancelled_clone_says_cancelled_rather_than_failed() {
    let mut m = on_top();
    let mut st = running("job-1");
    st.git.cancelled = true;
    let _ = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: FromGitOutcome::Refused {
                kind: FromGitRefusal::CloneFailed,
                reason: "The clone was refused (cancelled): stopped".into(),
            },
        },
    );
    assert_eq!(st.git.error.as_deref(), Some("Clone cancelled"));
}

#[test]
fn a_failure_after_the_dialog_was_closed_is_told_where_the_person_is() {
    // Escape closed the dialog while the clone ran: the failure becomes a
    // notice rather than a message in a form nobody sees.
    let mut m = Model::default();
    let mut st = running("job-1");
    let task = form(
        &mut st,
        &mut m,
        Msg::GitDone {
            job: "job-1".into(),
            outcome: FromGitOutcome::Refused {
                kind: FromGitRefusal::CloneFailed,
                reason: "nope".into(),
            },
        },
    );
    assert_eq!(task.units(), 1);
    assert!(st.git.error.is_none());
}

#[test]
fn the_progress_of_the_clone_followed_is_kept_and_another_clones_is_not() {
    let mut m = on_top();
    let mut st = running("job-1");
    let progress = |job: &str| CloneProgress {
        job_id: job.into(),
        phase: "Receiving objects".into(),
        percent: Some(10),
    };
    assert!(!follow(&mut st, &progress("job-2")));
    assert!(st.git.progress.is_none());
    assert!(follow(&mut st, &progress("job-1")));
    assert_eq!(st.git.progress.as_ref().and_then(|p| p.percent), Some(10));
    // Nothing runs, nothing is followed.
    st.git.job = None;
    assert!(!follow(&mut st, &progress("job-1")));
    let _ = &mut m;
}

#[test]
fn opening_the_dialog_starts_a_clean_form_unless_a_clone_is_still_running() {
    let mut st = git_state("acme/shop", None);
    st.name = "left over".into();
    opened(&mut st);
    assert!(st.name.is_empty() && st.git.repo.is_empty());
    assert_eq!(st.source, Source::Folder);

    // The dialog was closed with a clone running: it comes back as it was.
    let mut st = running("job-1");
    st.name = "shop".into();
    opened(&mut st);
    assert!(st.git.running);
    assert_eq!(st.name, "shop");
    assert_eq!(st.git.repo, "acme/shop");
}

#[test]
fn the_kits_dialog_opens_on_the_workspace_it_is_asked_for() {
    let h = Harness::new(Locale::En, "graphite");
    let mut m = h.model;
    let mut st = State::default();
    // `on_intent` hands the intent to this; the services are not reached.
    st.kits = kits_dialog::KitsDialog::open(&m, &"w2".to_string());
    assert_eq!(st.kits.as_ref().map(|kits| kits.ws.as_str()), Some("w2"));
    assert!(kits_dialog::KitsDialog::open(&m, &"gone".to_string()).is_none());
    let _ = &mut m;
}

// ── the fields: the keymap first, Return everywhere ───────────────────

mod fields {
    use iced::keyboard::key::Named;

    use super::*;
    use crate::settings::guard_testing::Fixture;

    #[test]
    fn every_field_of_the_folder_form_leaves_the_shortcut_to_the_keymap() {
        let h = Harness::new(Locale::En, "graphite");
        // The folder and the name; « More options » folded.
        assert_eq!(
            Fixture::new().split_chord(view(&State::default(), h.ctx())),
            ["Noop", "Noop"]
        );
        // Unfolded, the two commands of the worktrees too.
        let st = custom();
        assert_eq!(
            Fixture::new().split_chord(view(&st, h.ctx())),
            ["Noop", "Noop", "Noop", "Noop"]
        );
    }

    #[test]
    fn every_field_of_the_git_form_leaves_it_too() {
        let h = Harness::new(Locale::En, "graphite");
        let st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
        // The address, the destination and the name.
        assert_eq!(
            Fixture::new().split_chord(view(&st, h.ctx())),
            ["Noop", "Noop", "Noop"]
        );
    }

    #[test]
    fn the_hooks_of_the_kits_dialog_leave_it_too() {
        let h = Harness::new(Locale::En, "graphite");
        let mut st = State::default();
        st.kits = kits_dialog::KitsDialog::open(&h.model, &"w2".to_string());
        let ws = "w2".to_string();
        let said = Fixture::new().split_chord(kits_dialog_view(&st, h.ctx(), &ws));
        assert_eq!(said, ["Noop", "Noop"]);
    }

    /// What Return says in the field that shows `placeholder`.
    fn return_in(st: &State, placeholder: &str) -> Vec<Message> {
        let h = Harness::new(Locale::En, "graphite");
        let mut ui = h.show(view(st, h.ctx()));
        ui.click(placeholder).expect("the field is drawn");
        ui.tap_key(Named::Enter);
        messages(ui)
    }

    #[test]
    fn return_in_the_folder_field_sends_the_form() {
        let mut st = State::default();
        st.folder.cwd = "/home/me/dev/shop".into();
        let said = return_in(&st, "/home/me/dev/shop");
        assert!(sent(&said, |msg| matches!(msg, Msg::Submit(_))), "{said:?}");
    }

    #[test]
    fn return_in_the_clone_folder_field_sends_the_form() {
        let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
        st.git.parent = "/home/me/dev".into();
        let said = return_in(&st, "/home/me/dev");
        assert!(sent(&said, |msg| matches!(msg, Msg::Submit(_))), "{said:?}");
    }

    #[test]
    fn return_in_a_hook_field_says_what_the_form_asked() {
        // Far down a scrolled form the field is out of the simulator's reach,
        // so the section is shown alone, with a message of its own to send.
        let h = Harness::new(Locale::En, "graphite");
        let mut draft = hooks::HooksDraft::default();
        draft.set(hooks::HookField::PostCreate, "pnpm i".into());
        let mut ui = h.show(hooks::view(
            h.ctx(),
            &draft,
            Some(Message::WorkspaceDialog(Msg::Cancel)),
            |field, text| Message::WorkspaceDialog(Msg::Hook(field, text)),
        ));
        ui.click("pnpm i").expect("the field is drawn");
        ui.tap_key(Named::Enter);
        assert!(sent(&messages(ui), |msg| matches!(msg, Msg::Cancel)));
    }

    #[test]
    fn return_in_the_kits_dialog_s_hooks_sends_nothing() {
        // It saves as it goes: there is no form to send.
        let h = Harness::new(Locale::En, "graphite");
        let mut st = State::default();
        st.kits = kits_dialog::KitsDialog::open(&h.model, &"w2".to_string());
        st.kits
            .as_mut()
            .expect("opened")
            .hooks
            .set(hooks::HookField::PostCreate, "make".into());
        let ws = "w2".to_string();
        let mut ui = h.show(kits_dialog_view(&st, h.ctx(), &ws));
        ui.click("make").expect("the field is drawn");
        ui.tap_key(Named::Enter);
        assert!(!sent(&messages(ui), |msg| matches!(msg, Msg::Submit(_))));
    }
}

// ── the clone's bar ───────────────────────────────────────────────────

mod bar {
    use super::*;

    fn clone_with(phase: Option<(&str, Option<u8>)>) -> State {
        let mut st = running("job-1");
        st.git.progress = phase.map(|(phase, percent)| CloneProgress {
            job_id: "job-1".into(),
            phase: phase.into(),
            percent,
        });
        st
    }

    #[test]
    fn a_clone_with_no_percentage_yet_has_a_bar_that_cannot_say_how_far_it_is() {
        assert!(clone_with(None).git.indeterminate());
        // Git printed a phase but no number, or one the bar has no share for.
        assert!(
            clone_with(Some(("Receiving objects", None)))
                .git
                .indeterminate()
        );
        assert!(
            clone_with(Some(("Counting objects", Some(50))))
                .git
                .indeterminate()
        );
    }

    #[test]
    fn a_clone_with_a_percentage_has_a_bar_that_measures() {
        assert!(
            !clone_with(Some(("Receiving objects", Some(45))))
                .git
                .indeterminate()
        );
        assert!(
            !clone_with(Some(("Resolving deltas", Some(0))))
                .git
                .indeterminate()
        );
        // Nothing runs: nothing to measure or to wait for.
        assert!(!State::default().git.indeterminate());
    }

    #[test]
    fn the_tick_that_moves_the_bar_runs_only_while_it_is_on_show() {
        let mut m = on_top();
        let waiting = clone_with(None);
        assert_eq!(subscription(&waiting, &m).units(), 1);
        // The bar measures: no frames are needed for it.
        let measured = clone_with(Some(("Receiving objects", Some(45))));
        assert_eq!(subscription(&measured, &m).units(), 0);
        // The dialog was closed with the clone running: nobody looks.
        m.ui.overlays.clear();
        assert_eq!(subscription(&waiting, &m).units(), 0);
    }

    #[test]
    fn the_tick_moves_the_bar_along() {
        let mut st = clone_with(None);
        let began = Instant::now();
        st.git.began = Some(began);
        let mut m = on_top();
        let _ = form(
            &mut st,
            &mut m,
            Msg::Tick(began + Duration::from_millis(400)),
        );
        assert_eq!(st.git.sweep_elapsed(), Duration::from_millis(400));
    }

    #[test]
    fn the_form_draws_both_bars() {
        let h = Harness::new(Locale::En, "graphite");
        let waiting = clone_with(None);
        let mut ui = h.show(view(&waiting, h.ctx()));
        assert!(ui.find("Starting the clone…").is_ok());
        h.shot(&mut ui, "dialog-git-waiting-en-graphite");
        let measured = clone_with(Some(("Receiving objects", Some(45))));
        let mut ui = h.show(view(&measured, h.ctx()));
        assert!(ui.find("Receiving objects 45 %").is_ok());
    }
}

// ── WSD-01, WSD-02, WSD-03: stale answers and a Stop that waits ────────

fn roots_of(path: &str) -> Result<Vec<Root>, String> {
    Ok(vec![Root {
        name: "a".into(),
        path: path.into(),
        exists: true,
    }])
}

#[test]
fn a_suggestion_for_a_folder_since_left_is_not_shown() {
    let mut m = on_top();
    let mut st = State::default();
    let _ = form(&mut st, &mut m, Msg::Picked(Some("/share/a".into())));
    let _ = form(&mut st, &mut m, Msg::Picked(Some("/share/b".into())));
    // A's answer arrives late.
    let _ = form(
        &mut st,
        &mut m,
        Msg::Suggestion {
            for_cwd: "/share/a".into(),
            found: Some("/share/a/a.code-workspace".into()),
        },
    );
    assert!(st.folder.suggestion.is_none());
    let _ = form(
        &mut st,
        &mut m,
        Msg::Suggestion {
            for_cwd: "/share/b".into(),
            found: Some("/share/b/b.code-workspace".into()),
        },
    );
    assert_eq!(
        st.folder.suggestion.as_deref(),
        Some("/share/b/b.code-workspace")
    );
}

#[test]
fn a_workspace_file_read_for_an_older_folder_is_not_applied() {
    let mut m = on_top();
    let mut st = State::default();
    let _ = form(&mut st, &mut m, Msg::Picked(Some("/share/a".into())));
    st.folder.suggestion = Some("/share/a/a.code-workspace".into());
    let _ = form(&mut st, &mut m, Msg::UseSuggestion);
    let read = st.folder.reading.expect("a read is pending");
    // The person moves on to B before the file is read.
    let _ = form(&mut st, &mut m, Msg::Picked(Some("/share/b".into())));
    let _ = form(
        &mut st,
        &mut m,
        Msg::FileRead {
            read,
            path: "/share/a/a.code-workspace".into(),
            result: roots_of("/share/a"),
        },
    );
    assert_eq!(st.folder.cwd, "/share/b");
    assert!(st.folder.roots.is_empty());
}

#[test]
fn a_workspace_file_read_does_not_fill_a_form_reset_since() {
    let mut m = on_top();
    let mut st = State::default();
    let _ = form(&mut st, &mut m, Msg::Picked(Some("/share/a".into())));
    st.folder.suggestion = Some("/share/a/a.code-workspace".into());
    let _ = form(&mut st, &mut m, Msg::UseSuggestion);
    let read = st.folder.reading.expect("a read is pending");
    // Closed and opened again.
    opened(&mut st);
    let _ = form(
        &mut st,
        &mut m,
        Msg::FileRead {
            read,
            path: "/share/a/a.code-workspace".into(),
            result: roots_of("/share/a"),
        },
    );
    assert_eq!(st.folder.cwd, "");
    assert_eq!(st.name, "");
    // And the next read is not mistaken for the old one.
    st.folder.suggestion = Some("/share/a/a.code-workspace".into());
    let _ = form(&mut st, &mut m, Msg::UseSuggestion);
    assert_ne!(st.folder.reading, Some(read));
}

#[test]
fn typing_a_folder_drops_the_loaded_files_roots() {
    let mut m = on_top();
    let mut st = State::default();
    st.folder.reading = Some(1);
    let _ = form(
        &mut st,
        &mut m,
        Msg::FileRead {
            read: 1,
            path: "/work/x.code-workspace".into(),
            result: roots_of("/work/x"),
        },
    );
    assert_eq!(st.folder.roots.len(), 1);
    let _ = form(&mut st, &mut m, Msg::Cwd("/elsewhere".into()));
    assert!(st.folder.roots.is_empty());
}

#[tokio::test]
async fn a_stop_waits_for_the_job_however_long_the_checks_take() {
    let ended = Arc::new(AtomicBool::new(false));
    let mut asked = 0;
    // The job only exists on the 40th ask: 8 s at the real pace, well past
    // the 3 s the Stop used to give up after.
    let cancelled = cancel_until("job".into(), ended, Duration::from_millis(1), |job| {
        assert_eq!(job, "job");
        asked += 1;
        asked >= 40
    })
    .await;
    assert!(cancelled);
    assert_eq!(asked, 40);
}

#[tokio::test]
async fn a_stop_ends_with_the_flow() {
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let mut asked = 0;
    let cancelled = cancel_until("job".into(), ended, Duration::from_millis(1), |_| {
        asked += 1;
        if asked == 5 {
            // The flow failed before any clone started.
            flag.store(true, Ordering::SeqCst);
        }
        false
    })
    .await;
    assert!(!cancelled);
    assert_eq!(asked, 5);
}

#[test]
fn the_end_of_the_flow_releases_its_stop() {
    let mut m = on_top();
    let mut st = State::default();
    let ended = Arc::new(AtomicBool::new(false));
    st.git.job = Some("job".into());
    st.git.ended = Some(ended.clone());
    st.git.running = true;
    let t = Translator::new(m.locale());
    let _ = git_done(&mut st, &mut m, t, "job", created(false));
    assert!(ended.load(Ordering::SeqCst));
    assert!(st.git.ended.is_none());
}

/// With the `nodes` experiment on and a node connected, the folder form
/// offers it; its folders are the node's, and the workspace made lives on it.
#[test]
fn a_workspace_can_be_made_on_a_connected_node() {
    use crate::nodes::NodeStatus;
    use crate::nodes::model::NodeInfo;
    use crate::nodes::remote::Listing;
    use crate::state::types::NodeConfig;

    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![NodeConfig {
            id: "n1".into(),
            label: "GPU box".into(),
            ssh_target: "me@gpu".into(),
            ..NodeConfig::default()
        }];
    });
    let st = State::default();
    {
        // Not connected: listed, and not to be chosen.
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("GPU box").is_ok());
        let _ = ui.click("GPU box");
        assert!(!sent(&messages(ui), |msg| matches!(msg, Msg::Place(_))));
    }
    h.model.live.nodes.statuses.insert(
        "n1".into(),
        NodeStatus::Connected {
            info: NodeInfo::default(),
            local_port: 21011,
            token: "tok".into(),
        },
    );
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("Runs on").is_ok());
        assert!(ui.find("This computer").is_ok());
        h.shot(&mut ui, "machines-dialog-runs-on-en-graphite");
        ui.click("GPU box").expect("the node is offered");
        assert!(sent(&messages(ui), |msg| matches!(
            msg,
            Msg::Place(Some(node)) if node == "n1"
        )));
    }

    let mut st = State::default();
    st.folder.cwd = "/home/me/local".into();
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("n1".into())));
    assert_eq!(
        st.folder.cwd, "",
        "a folder of this machine means nothing there"
    );
    let asked = st.place.listing_asked.expect("the node's home is listed");
    let listing = Listing {
        path: "/home/hangar".into(),
        parent: Some("/home".into()),
        folders: vec!["demo".into()],
    };
    // An answer to an older listing is not believed.
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeListed {
            asked: asked + 1000,
            result: Ok(listing.clone()),
        },
    );
    assert_eq!(st.folder.cwd, "");
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeListed {
            asked,
            result: Ok(listing),
        },
    );
    assert_eq!(st.folder.cwd, "/home/hangar");
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("Folder on that machine").is_ok());
        assert!(ui.find("Browse…").is_err(), "not this machine's dialog");
        ui.click("demo/").expect("a sub-folder of the node");
        assert!(sent(&messages(ui), |msg| matches!(
            msg,
            Msg::NodeFolder(Some(path)) if path == "/home/hangar/demo"
        )));
    }

    st.folder.cwd = "/home/hangar/demo".into();
    let before = h.model.state().workspaces.len();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let workspaces = &h.model.state().workspaces;
    assert_eq!(workspaces.len(), before + 1);
    let made = workspaces
        .iter()
        .find(|ws| ws.cwd == "/home/hangar/demo")
        .expect("the workspace is made");
    assert_eq!(made.node.as_deref(), Some("n1"));
}

/// A workspace made on a node is the node's own from the start: flagged
/// here, handed over once, and taken back out when the node refuses it.
#[test]
fn a_workspace_made_on_a_node_is_handed_to_it_once_and_taken_back_if_refused() {
    use crate::nodes::hosted::{self, Handover, Job};
    use crate::nodes::remote::Listing;
    use crate::nodes::testing::{machine, up};
    use crate::state::types::{MachineKind, PaneRole};

    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![machine("vps", "vps", MachineKind::Cloud)];
    });
    up(&mut h.model, "vps", "vps-01");
    let mut st = State::default();
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("vps".into())));
    let asked = st.place.listing_asked.expect("listed");
    let listing = Listing {
        path: "/home/vps/projects".into(),
        parent: Some("/home/vps".into()),
        folders: vec!["api".into(), "site".into()],
    };
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeListed {
            asked,
            result: Ok(listing),
        },
    );
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(
            ui.find(
                "That machine runs this workspace itself: its agents keep working with this \
                 computer off, and this window shows them."
            )
            .is_ok()
        );
        h.shot(&mut ui, "create-on-node-en-graphite");
    }

    st.folder.cwd = "/home/vps/projects/api".into();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let made = h
        .model
        .state()
        .workspaces
        .iter()
        .find(|ws| ws.cwd == "/home/vps/projects/api")
        .expect("made here at once")
        .clone();
    assert!(made.is_hosted());
    assert_eq!(made.node.as_deref(), Some("vps"));
    // Everything the dialog set went into the one workspace handed over:
    // no edit follows it.
    let waiting = h.model.live.nodes.outbox.waiting("vps");
    let [
        Job::Adopt {
            workspace,
            handover: Handover::Created,
        },
    ] = waiting.as_slice()
    else {
        panic!("one adopt: {waiting:?}");
    };
    assert_eq!(workspace.id, made.id);
    assert_eq!(workspace.panes[0].id, made.panes[0].id);
    assert_eq!(workspace.panes[0].role, Some(PaneRole::Orchestrator));
    assert!(h.model.live.nodes.outbox.adopting(&made.id));

    // The node says no: the workspace is not kept, and nothing of this
    // machine is cleaned up after a folder that is the node's.
    let _ = hosted::send_next(&mut h.model);
    let _ = hosted::sent(
        &mut h.model,
        "vps",
        Err(hosted::NotTaken::Refused(
            "/home/vps/projects/api is not a folder here".into(),
        )),
    );
    assert!(h.model.workspace(&made.id).is_none());
    assert!(!h.model.live.nodes.outbox.busy("vps"));
    let removed = h.model.take_removed();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].node.as_deref(), Some("vps"));
}

#[test]
fn a_workspace_the_node_takes_stays_as_its_mirror() {
    use crate::nodes::hosted;
    use crate::nodes::testing::{machine, up};
    use crate::state::types::MachineKind;

    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![machine("vps", "vps", MachineKind::Cloud)];
    });
    up(&mut h.model, "vps", "vps-01");
    let mut st = State::default();
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("vps".into())));
    st.folder.cwd = "/home/vps/projects/api".into();
    let _ = form(&mut st, &mut h.model, Msg::Submit(Vec::new()));
    let _ = hosted::send_next(&mut h.model);
    let _ = hosted::sent(&mut h.model, "vps", Ok(()));
    let made = h
        .model
        .state()
        .workspaces
        .iter()
        .find(|ws| ws.cwd == "/home/vps/projects/api")
        .expect("kept");
    assert!(made.is_hosted());
    assert!(!h.model.live.nodes.outbox.adopting(&made.id));
}

/// Three nodes beside this computer: `GPU box` connected, `vps` whose link
/// is down, `nas` switched off.
fn with_machines(locale: Locale, theme: &str) -> Harness {
    with_nodes(locale, theme, 3)
}

/// The first `count` of the nodes of [`with_machines`].
fn with_nodes(locale: Locale, theme: &str, count: usize) -> Harness {
    use crate::nodes::model::{Failure, Step};
    use crate::nodes::testing::{down, machine, up};
    use crate::state::types::MachineKind;
    let mut h = Harness::new(locale, theme).edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![
            machine("gpu", "GPU box", MachineKind::Tower),
            machine("vps", "vps", MachineKind::Cloud),
            machine("nas", "nas", MachineKind::MiniPc),
        ];
        state.settings.nodes[2].enabled = Some(false);
        state.settings.nodes.truncate(count);
    });
    up(&mut h.model, "gpu", "gpu-01");
    down(
        &mut h.model,
        "vps",
        30,
        Some(Failure::Ssh {
            step: Step::Probe,
            detail: "Connection timed out".into(),
        }),
    );
    h.model.ui.overlays.push(Overlay::WorkspaceDialog);
    h
}

/// « Runs on » is there whatever the workspace is made from, with every
/// machine: one that is away keeps its place and says its state, and
/// cannot be chosen, from the pointer or otherwise.
#[test]
fn runs_on_lists_every_machine_in_clone_mode_and_an_offline_one_cannot_be_chosen() {
    let mut h = with_nodes(Locale::En, "graphite", 2);
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    {
        let mut ui = h.show(view(&st, h.ctx()));
        for text in ["Runs on", "This computer", "GPU box", "vps", "offline"] {
            assert!(ui.find(text).is_ok(), "{text} is on the form");
        }
        h.shot(&mut ui, "clone-runs-on-en-graphite");
        let _ = ui.click("vps");
        assert!(
            !sent(&messages(ui), |msg| matches!(msg, Msg::Place(_))),
            "a machine that is away is not a button"
        );
    }
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("vps".into())));
    assert_eq!(st.place.node, None, "vps cannot be chosen");
    // The connected one can, and the clone then lands in its `projects`.
    {
        let mut ui = h.show(view(&st, h.ctx()));
        ui.click("GPU box").expect("a connected machine is offered");
        assert!(sent(&messages(ui), |msg| matches!(
            msg,
            Msg::Place(Some(node)) if node == "gpu"
        )));
    }
    st.git.parent = "/home/me/dev".into();
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("gpu".into())));
    assert_eq!(st.place.node.as_deref(), Some("gpu"));
    assert_eq!(
        st.git.parent, "",
        "a folder of this computer means nothing there"
    );
    assert!(
        st.place.listing_asked.is_none(),
        "nothing is listed unasked"
    );
    assert_eq!(
        st.parent_for(&h.model).as_deref(),
        Some("/home/me/projects"),
        "empty: projects in the node's home"
    );
    assert_eq!(st.git_status(&h.model), FromGitStatus::Ready);
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "empty: the projects folder in the home of GPU box",
        "It will be cloned to /home/me/projects/shop on GPU box",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    h.shot(&mut ui, "clone-on-node-en-graphite");
}

/// From four machines on they no longer sit side by side on one line of
/// the dialog: « Runs on » is a field that shows the machine chosen and
/// opens the list of them all, each by its icon and its name, one that is
/// away greyed with its state and not to be picked.
#[test]
fn runs_on_is_a_list_from_four_machines_on() {
    let mut h = with_machines(Locale::En, "graphite");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("Runs on").is_ok());
        assert!(ui.find("This computer").is_ok(), "the one chosen");
        for text in ["GPU box", "vps", "nas"] {
            assert!(ui.find(text).is_err(), "{text} waits in the list");
        }
        h.shot(&mut ui, "clone-runs-on-list-closed-en-graphite");
        ui.click("This computer").expect("the field");
        assert!(sent(&messages(ui), |msg| matches!(
            msg,
            Msg::PlaceList(true)
        )));
    }
    let _ = form(&mut st, &mut h.model, Msg::PlaceList(true));
    assert!(st.place.picking);
    {
        // The list is drawn over the form: what it holds is read from what
        // it is made of, and looked at in the picture.
        let lines: Vec<(Option<String>, String, Option<String>)> =
            super::view::machine_lines(h.ctx())
                .into_iter()
                .map(|line| (line.id, line.label, line.away))
                .collect();
        let line = |id: Option<&str>, label: &str, away: Option<&str>| {
            (
                id.map(str::to_string),
                label.to_string(),
                away.map(str::to_string),
            )
        };
        assert_eq!(
            lines,
            [
                line(None, "This computer", None),
                line(Some("gpu"), "GPU box", None),
                line(Some("vps"), "vps", Some("offline")),
                line(Some("nas"), "nas", Some("switched off")),
            ]
        );
        let mut ui = h.show(view(&st, h.ctx()));
        h.shot(&mut ui, "clone-runs-on-list-en-graphite");
    }
    // One that is away cannot be chosen, whatever sends it.
    for away in ["vps", "nas"] {
        let _ = form(&mut st, &mut h.model, Msg::Place(Some(away.into())));
        assert_eq!(st.place.node, None, "{away} cannot be chosen");
    }
    let _ = form(&mut st, &mut h.model, Msg::PlaceList(true));
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("gpu".into())));
    assert_eq!(st.place.node.as_deref(), Some("gpu"));
    assert!(!st.place.picking, "picking one closes the list");
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("GPU box").is_ok(), "the field shows the one chosen");
    assert!(ui.find("This computer").is_err());
}

#[test]
fn the_clone_form_lists_the_machines_in_french_in_a_light_theme() {
    let mut h = with_machines(Locale::Fr, "github-light");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    let _ = form(&mut st, &mut h.model, Msg::PlaceList(true));
    let mut ui = h.show(view(&st, h.ctx()));
    for text in ["S’exécute sur", "Cet ordinateur"] {
        assert!(ui.find(text).is_ok(), "{text} is on the form");
    }
    let states: Vec<Option<String>> = super::view::machine_lines(h.ctx())
        .into_iter()
        .map(|line| line.away)
        .collect();
    assert_eq!(
        states,
        [
            None,
            None,
            Some("hors ligne".to_string()),
            Some("désactivé".to_string())
        ]
    );
    h.shot(&mut ui, "clone-runs-on-fr-github-light");
}

/// Without a node there is nothing to choose, in either mode.
#[test]
fn runs_on_is_not_there_without_a_node_or_the_experiment() {
    let h = Harness::new(Locale::En, "graphite");
    for st in [State::default(), git_state("acme/shop", None)] {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("Runs on").is_err());
    }
    let h = with_machines(Locale::En, "graphite").edit(|state| state.settings.experiments.clear());
    let st = State::default();
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("Runs on").is_err());
}

/// The folder a node clones into is picked among the node's own, listed
/// in the form; switching what the workspace is made from lists again for
/// the other field.
#[test]
fn the_folder_to_clone_into_is_browsed_on_the_node() {
    use crate::nodes::remote::Listing;
    let mut h = with_machines(Locale::En, "graphite");
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("gpu".into())));
    let _ = form(&mut st, &mut h.model, Msg::BrowseParent);
    assert!(st.place.browsing);
    let asked = st.place.listing_asked.expect("the node's home is listed");
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeListed {
            asked,
            result: Ok(Listing {
                path: "/home/me".into(),
                parent: Some("/home".into()),
                folders: vec!["dev".into(), "projects".into()],
            }),
        },
    );
    assert_eq!(
        st.git.parent, "/home/me",
        "the folder listed is the one picked"
    );
    assert_eq!(st.folder.cwd, "", "not the folder of a workspace");
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(
            ui.find("It will be cloned to /home/me/shop on GPU box")
                .is_ok()
        );
        h.shot(&mut ui, "clone-on-node-browsing-en-graphite");
        ui.click("dev/").expect("a sub-folder of the node");
        assert!(sent(&messages(ui), |msg| matches!(
            msg,
            Msg::NodeFolder(Some(path)) if path == "/home/me/dev"
        )));
    }
    // Back to a folder: the listing is the workspace's folder's again.
    let _ = form(&mut st, &mut h.model, Msg::Source(Source::Folder));
    assert!(!st.place.browsing);
    assert!(st.place.listing.is_none());
    assert!(st.place.listing_asked.is_some());
}

/// What the fake node of this test answers one request with.
async fn serve_once(
    listener: &tokio::net::TcpListener,
    status: u16,
    body: serde_json::Value,
) -> crate::nodes::testing::Request {
    use crate::nodes::testing::{answer, read_request};
    let (mut stream, _) = listener.accept().await.expect("a connection");
    let request = read_request(&mut stream).await;
    answer(&mut stream, status, &body.to_string()).await;
    request
}

/// `vps` connected through `client`, its home `/home/vps`, and the clone
/// form ready for `acme/shop` on it.
fn ready_to_clone_on(client: &crate::nodes::model::NodeClient) -> (Harness, State) {
    use crate::nodes::NodeStatus;
    use crate::nodes::model::NodeInfo;
    use crate::nodes::testing::{TOKEN, machine};
    use crate::state::types::MachineKind;
    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![machine("vps", "vps", MachineKind::Cloud)];
    });
    let port = client
        .base_url
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .expect("a port");
    h.model.live.nodes.statuses.insert(
        "vps".into(),
        NodeStatus::Connected {
            info: NodeInfo {
                home: "/home/vps".into(),
                ..NodeInfo::default()
            },
            local_port: port,
            token: TOKEN.into(),
        },
    );
    h.model.ui.overlays.push(Overlay::WorkspaceDialog);
    let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
    let _ = form(&mut st, &mut h.model, Msg::Place(Some("vps".into())));
    (h, st)
}

/// Cloning on a node, start to end against a node that says yes: the node
/// is asked to clone into its `projects`, and the workspace made on the
/// folder it answers is handed to it as its own, one orchestrator pane.
#[tokio::test]
async fn a_clone_on_a_node_is_asked_of_it_then_the_workspace_handed_over() {
    use crate::nodes::hosted::{self, Handover, Job};
    use crate::nodes::testing::listen;
    use crate::state::types::PaneRole;
    let (listener, client) = listen().await;
    let (mut h, mut st) = ready_to_clone_on(&client);

    let _ = start_git_on_node(&mut st, &mut h.model, "vps");
    assert!(st.git.running);
    let job = st.git.job.clone().expect("a job to follow");
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(
            ui.find("Cloning on vps…").is_ok(),
            "said until git says more"
        );
        assert!(ui.find("Cancel the clone").is_ok());
        h.shot(&mut ui, "clone-on-node-running-en-graphite");
    }
    // Its progress comes with the node's events, and is followed as a
    // local clone's.
    let told = crate::nodes::event_messages(
        "vps",
        h.model.state(),
        serde_json::json!({
            "event": "git-clone:progress",
            "payload": { "jobId": job, "phase": "Receiving objects", "percent": 40 },
        }),
    );
    let [Message::Core(CoreEvent::GitCloneProgress(progress))] = told.as_slice() else {
        panic!("one progress: {told:?}");
    };
    assert!(follow(&mut st, progress));
    assert_eq!(st.git.progress.as_ref().and_then(|p| p.percent), Some(40));

    let asked = job.clone();
    let cloning = tokio::spawn(async move {
        crate::nodes::engine::clone(
            &client,
            "https://github.com/acme/shop.git",
            "/home/vps/projects",
            "shop",
            &asked,
        )
        .await
    });
    let request = serve_once(
        &listener,
        200,
        serde_json::json!({ "value": {
            "status": "cloned", "path": "/home/vps/projects/shop", "reused": false, "branch": "main",
        } }),
    )
    .await;
    assert_eq!(request.line(), "POST /node/clone HTTP/1.1");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "url": "https://github.com/acme/shop.git",
            "parent": "/home/vps/projects",
            "name": "shop",
            "jobId": job,
        })
    );
    let result = cloning.await.unwrap();
    assert_eq!(
        result.as_ref().map(|cloned| cloned.path.as_str()),
        Ok("/home/vps/projects/shop")
    );

    let before = h.model.state().workspaces.len();
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeCloned {
            job,
            node: "vps".into(),
            result,
        },
    );
    assert!(!st.git.running);
    assert_eq!(st.git.error, None);
    assert_eq!(h.model.state().workspaces.len(), before + 1);
    let made = h
        .model
        .state()
        .workspaces
        .iter()
        .find(|ws| ws.cwd == "/home/vps/projects/shop")
        .expect("made on the folder the node cloned")
        .clone();
    assert_eq!(made.name, "shop");
    assert_eq!(made.node.as_deref(), Some("vps"));
    assert!(made.is_hosted(), "the node's own from the start");
    assert_eq!(made.panes.len(), 1);
    assert_eq!(made.panes[0].role, Some(PaneRole::Orchestrator));

    // Then the adopt, sent in the node's line.
    let waiting = h.model.live.nodes.outbox.waiting("vps");
    let [
        job @ Job::Adopt {
            handover: Handover::Created,
            ..
        },
    ] = waiting.as_slice()
    else {
        panic!("one adopt: {waiting:?}");
    };
    let client = crate::nodes::client(&h.model, "vps").expect("connected");
    let job = job.clone();
    let sending = tokio::spawn(async move { hosted::send(&client, &job).await });
    let request = serve_once(&listener, 200, serde_json::json!({ "id": made.id })).await;
    assert_eq!(request.line(), "POST /node/own/adopt HTTP/1.1");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["id"], made.id.as_str());
    assert_eq!(body["cwd"], "/home/vps/projects/shop");
    assert_eq!(sending.await.unwrap(), Ok(()));
    let _ = hosted::send_next(&mut h.model);
    let _ = hosted::sent(&mut h.model, "vps", Ok(()));
    assert!(h.model.workspace(&made.id).is_some_and(|ws| ws.is_hosted()));
}

/// The node could not clone: git's own words are shown, with what is
/// likely behind them, and nothing is made.
#[tokio::test]
async fn a_clone_the_node_refuses_shows_git_s_words_and_makes_nothing() {
    use crate::nodes::testing::listen;
    let (listener, client) = listen().await;
    let (mut h, mut st) = ready_to_clone_on(&client);
    let _ = start_git_on_node(&mut st, &mut h.model, "vps");
    let job = st.git.job.clone().expect("a job");
    let asked = job.clone();
    let cloning = tokio::spawn(async move {
        crate::nodes::engine::clone(
            &client,
            "git@github.com:acme/shop.git",
            "/home/vps/projects",
            "shop",
            &asked,
        )
        .await
    });
    serve_once(
        &listener,
        200,
        serde_json::json!({ "value": {
            "status": "refused", "kind": "failed",
            "reason": "git@github.com: Permission denied (publickey).",
        } }),
    )
    .await;
    let result = cloning.await.unwrap();
    assert_eq!(
        result.as_ref().map_err(|refusal| refusal.kind.as_str()),
        Err("failed")
    );
    let before = h.model.state().workspaces.len();
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeCloned {
            job,
            node: "vps".into(),
            result,
        },
    );
    assert!(!st.git.running);
    assert_eq!(
        st.git.error.as_deref(),
        Some(
            "vps could not clone it: git@github.com: Permission denied (publickey). vps may \
             not have access to this repository: it clones with its own keys, not this \
             computer’s."
        )
    );
    assert_eq!(h.model.state().workspaces.len(), before, "nothing is made");
    assert!(
        !h.model.live.nodes.outbox.busy("vps"),
        "nothing is handed over"
    );
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(
        ui.find("Clone and create workspace").is_ok(),
        "to try again"
    );
    h.shot(&mut ui, "clone-on-node-refused-en-graphite");
}

/// A folder that holds something else is not git failing: no word on
/// access. A node that does not answer is said as it is; a clone the
/// person stopped says so.
#[test]
fn a_refusal_that_is_not_git_s_says_nothing_of_access() {
    use crate::nodes::engine::CloneRefusal;
    use crate::nodes::testing::{machine, up};
    use crate::state::types::MachineKind;
    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![machine("vps", "vps", MachineKind::Cloud)];
    });
    up(&mut h.model, "vps", "vps-01");
    h.model.ui.overlays.push(Overlay::WorkspaceDialog);
    let mut answer = |kind: &str, reason: &str, cancelled: bool| {
        let mut st = git_state("acme/shop", Some(parsed_ok("acme/shop", "shop")));
        st.place.node = Some("vps".into());
        st.git.job = Some("job".into());
        st.git.running = true;
        st.git.cancelled = cancelled;
        let _ = form(
            &mut st,
            &mut h.model,
            Msg::NodeCloned {
                job: "job".into(),
                node: "vps".into(),
                result: Err(CloneRefusal {
                    kind: kind.into(),
                    reason: reason.into(),
                }),
            },
        );
        st.git.error.unwrap_or_default()
    };
    assert_eq!(
        answer(
            "exists",
            "/home/vps/projects/shop already holds other files",
            false
        ),
        "vps could not clone it: /home/vps/projects/shop already holds other files."
    );
    assert_eq!(
        answer("unreachable", "the node did not answer", false),
        "vps could not clone it: the node did not answer."
    );
    assert_eq!(
        answer("cancelled", "the clone was cancelled", true),
        "Clone cancelled"
    );
    // An answer for another job changes nothing.
    let mut st = git_state("acme/shop", None);
    st.git.job = Some("newer".into());
    st.git.running = true;
    let _ = form(
        &mut st,
        &mut h.model,
        Msg::NodeCloned {
            job: "older".into(),
            node: "vps".into(),
            result: Err(CloneRefusal {
                kind: "failed".into(),
                reason: "x".into(),
            }),
        },
    );
    assert!(st.git.running);
}

/// « Open on another machine… » opens the dialog on the repository of the
/// workspace, « Runs on » already on a machine that lacks it.
#[test]
fn opening_a_repository_elsewhere_readies_the_clone_form() {
    use crate::nodes::testing::{machine, up};
    use crate::state::types::MachineKind;
    let mut h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![machine("vps", "vps", MachineKind::Cloud)];
    });
    up(&mut h.model, "vps", "vps-01");
    let shop = h.model.workspace("w2").unwrap().clone();
    let known = &mut h.model.live.nodes.repos;
    known.said(&shop, Some("git@github.com:acme/shop.git"));
    let mut st = State::default();
    st.name = "left over".into();
    let _ = open_elsewhere(&mut st, &h.model, "w2");
    assert_eq!(st.source, Source::Git);
    assert_eq!(st.git.repo, "git@github.com:acme/shop.git");
    assert_eq!(st.place.node.as_deref(), Some("vps"));
    assert_eq!(st.name, "", "a clean form otherwise");
    // A workspace without a repository opens the plain form.
    let mut st = State::default();
    let _ = open_elsewhere(&mut st, &h.model, "w1");
    assert_eq!(st.source, Source::Folder);
    assert_eq!(st.place.node, None);
}
