//! The Claude Code setup: when it shows, what « shells only » does, the
//! installer it runs, what follows the install, and the sheet drawn.
//!
//! `HANGAR_P8_SNAPSHOTS=<folder>` writes the sheet's pictures there, as the
//! new-workspace dialog's tests do (workspace_dialog/testing.rs).

use hangar_i18n::Locale;
use iced::futures::StreamExt;

use super::install::{self, Installer, Os};
use super::*;
use crate::message::Message;
use crate::shell::Action;
use crate::workspace_dialog::testing::Harness;

/// A window whose detection found no Claude Code.
fn missing(locale: Locale) -> (Harness, State) {
    let mut h = Harness::new(locale, "graphite");
    h.model.ui.claude_installed = false;
    let st = State {
        detected: true,
        ..State::default()
    };
    (h, st)
}

fn shells_only_on(h: &mut Harness) {
    h.model.apply(StoreAction::UpdateSettings(Patch::new(|s| {
        crate::settings::set_shells_only(s, true)
    })));
}

// ── when it shows ─────────────────────────────────────────────────────

#[test]
fn missing_claude_code_shows_the_setup_at_start() {
    let (h, st) = missing(Locale::En);
    assert!(needed(&st, &h.model));
    assert!(matches!(
        offer_at_start(&st, &h.model),
        Some(Intent::OpenOverlay(Overlay::ClaudeSetup))
    ));
}

#[test]
fn claude_code_present_shows_no_setup() {
    let h = Harness::new(Locale::En, "graphite");
    let st = State {
        detected: true,
        ..State::default()
    };
    assert!(!needed(&st, &h.model));
    assert!(offer_at_start(&st, &h.model).is_none());
}

/// Before `claude_installed` answers, every machine reads as one without
/// Claude Code: the sheet waits for the answer.
#[test]
fn nothing_shows_before_the_detection_answers() {
    let (h, mut st) = missing(Locale::En);
    st.detected = false;
    assert!(offer_at_start(&st, &h.model).is_none());
    let intent = redirect(
        &mut st,
        &h.model,
        Intent::OpenOverlay(Overlay::WorkspaceDialog),
    );
    assert!(matches!(
        intent,
        Intent::OpenOverlay(Overlay::WorkspaceDialog)
    ));
}

#[test]
fn the_setup_is_not_stacked_twice() {
    let (mut h, st) = missing(Locale::En);
    h.model.ui.overlays.push(Overlay::ClaudeSetup);
    assert!(offer_at_start(&st, &h.model).is_none());
    assert!(is_open(&h.model));
}

// ── a new workspace ───────────────────────────────────────────────────

#[test]
fn a_new_workspace_without_claude_code_opens_the_setup_then_the_dialog() {
    let (h, mut st) = missing(Locale::En);
    let intent = redirect(
        &mut st,
        &h.model,
        Intent::OpenOverlay(Overlay::WorkspaceDialog),
    );
    assert!(matches!(intent, Intent::OpenOverlay(Overlay::ClaudeSetup)));
    assert!(st.then_create);

    // Done: the sheet closes, and the dialog it stood in front of opens.
    let intents = closing(&mut st);
    assert!(matches!(
        intents.as_slice(),
        [
            Intent::CloseOverlay,
            Intent::OpenOverlay(Overlay::WorkspaceDialog)
        ]
    ));
    assert!(!st.then_create);
}

#[test]
fn other_overlays_pass_untouched() {
    let (h, mut st) = missing(Locale::En);
    let intent = redirect(&mut st, &h.model, Intent::OpenOverlay(Overlay::Palette));
    assert!(matches!(intent, Intent::OpenOverlay(Overlay::Palette)));
    assert!(!st.then_create);
}

#[test]
fn closing_a_setup_shown_at_start_opens_nothing_else() {
    let (_, mut st) = missing(Locale::En);
    assert!(matches!(
        closing(&mut st).as_slice(),
        [Intent::CloseOverlay]
    ));
}

// ── shells only ───────────────────────────────────────────────────────

#[test]
fn shells_only_is_kept_in_the_settings_and_honoured() {
    let (mut h, mut st) = missing(Locale::En);
    st.then_create = true;
    let _ = update(&mut st, &mut h.model, Msg::ShellsOnly);
    assert!(crate::settings::shells_only(h.model.settings()));
    assert!(!st.then_create, "the dialog it stood in front of opens");

    // Saved: the settings file says so, and reads back the same.
    let json = serde_json::to_value(h.model.settings()).expect("settings serialise");
    assert_eq!(json["shellsOnly"], serde_json::Value::Bool(true));
    let back: crate::state::types::Settings =
        serde_json::from_value(json).expect("settings read back");
    assert!(crate::settings::shells_only(&back));

    // Honoured: no sheet at start, and a new workspace is the dialog, whose
    // start falls back to a shell.
    assert!(!needed(&st, &h.model));
    assert!(offer_at_start(&st, &h.model).is_none());
    let intent = redirect(
        &mut st,
        &h.model,
        Intent::OpenOverlay(Overlay::WorkspaceDialog),
    );
    assert!(matches!(
        intent,
        Intent::OpenOverlay(Overlay::WorkspaceDialog)
    ));
    assert_eq!(
        crate::workspace_dialog::folder::start_of(false, h.model.ui.claude_installed),
        crate::workspace_dialog::folder::Start::Shell
    );
}

#[test]
fn shells_only_is_undone_in_the_settings() {
    let (mut h, st) = missing(Locale::En);
    shells_only_on(&mut h);
    assert!(!needed(&st, &h.model));
    h.model.apply(StoreAction::UpdateSettings(Patch::new(|s| {
        crate::settings::set_shells_only(s, false)
    })));
    assert!(needed(&st, &h.model));
    // Off is no key at all: the file is written back as it was before.
    let json = serde_json::to_value(h.model.settings()).expect("settings serialise");
    assert!(json.get("shellsOnly").is_none());
}

// ── the installer ─────────────────────────────────────────────────────

/// The commands of https://code.claude.com/docs/en/setup, word for word.
#[test]
fn the_official_installer_of_each_system() {
    assert_eq!(
        install::official(Os::Unix),
        Installer {
            program: "bash".into(),
            args: vec![
                "-o".into(),
                "pipefail".into(),
                "-c".into(),
                "curl -fsSL https://claude.ai/install.sh | bash".into(),
            ],
            shown: "curl -fsSL https://claude.ai/install.sh | bash".into(),
        }
    );
    let windows = install::official(Os::Windows);
    assert_eq!(windows.program, "powershell");
    assert_eq!(
        windows.args.last().map(String::as_str),
        Some("irm https://claude.ai/install.ps1 | iex")
    );
    assert_eq!(windows.shown, "irm https://claude.ai/install.ps1 | iex");
    for installer in [install::official(Os::Unix), windows] {
        assert!(
            !installer.args.iter().any(|arg| arg.contains("sudo")),
            "{installer:?}"
        );
    }
}

#[test]
fn installer_lines_are_shown_plain() {
    assert_eq!(
        install::clean("\u{1b}[1;32m✔ Installed\u{1b}[0m\n"),
        "✔ Installed"
    );
    assert_eq!(install::clean("10%\r50%\r100%\r\n"), "100%");
    assert_eq!(install::clean("a\tb"), "a\tb");
}

/// The runner, on a stand-in installer: every line it prints, both streams,
/// then its exit code.
#[cfg(unix)]
#[test]
fn the_runner_streams_the_output_then_the_exit() {
    let fake = Installer {
        program: "sh".into(),
        args: vec![
            "-c".into(),
            "echo downloading; echo 'not found' >&2; exit 3".into(),
        ],
        shown: "fake".into(),
    };
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let messages: Vec<Message> = runtime.block_on(install::run(fake, 7).collect());
    let mut lines = Vec::new();
    let mut exit = None;
    for message in messages {
        match message {
            Message::Shell(Action::Setup(Msg::Output(7, line))) => lines.push(line),
            Message::Shell(Action::Setup(Msg::Exited(7, ended))) => exit = Some(ended),
            other => panic!("unexpected {other:?}"),
        }
    }
    lines.sort();
    assert_eq!(lines, vec!["downloading", "not found"]);
    assert_eq!(exit, Some(Ok(Some(3))));
}

#[test]
fn an_installer_that_cannot_start_says_why() {
    let gone = Installer {
        program: "/nonexistent/hangar-no-such-installer".into(),
        args: vec![],
        shown: "gone".into(),
    };
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let messages: Vec<Message> = runtime.block_on(install::run(gone, 1).collect());
    assert!(matches!(
        messages.as_slice(),
        [Message::Shell(Action::Setup(Msg::Exited(1, Err(_))))]
    ));
}

// ── after the install ─────────────────────────────────────────────────

fn installing(h: &mut Harness, st: &mut State) -> u64 {
    let _ = update(st, &mut h.model, Msg::Install);
    assert_eq!(st.step, Step::Installing);
    st.run
}

#[test]
fn a_successful_install_looks_again_and_moves_on_to_signing_in() {
    let (mut h, mut st) = missing(Locale::En);
    let run = installing(&mut h, &mut st);
    let _ = update(
        &mut st,
        &mut h.model,
        Msg::Output(run, "Claude Code installed".into()),
    );
    // Exit 0 asks the PATH again: still installing until it answers.
    let _ = update(&mut st, &mut h.model, Msg::Exited(run, Ok(Some(0))));
    assert_eq!(st.step, Step::Installing);
    let _ = update(&mut st, &mut h.model, Msg::Found(run, true));
    assert_eq!(st.step, Step::SignIn);
    assert!(h.model.ui.claude_installed);
    assert!(!needed(&st, &h.model));
}

#[test]
fn an_install_that_leaves_no_claude_says_so() {
    let (mut h, mut st) = missing(Locale::En);
    let run = installing(&mut h, &mut st);
    let _ = update(&mut st, &mut h.model, Msg::Exited(run, Ok(Some(0))));
    let _ = update(&mut st, &mut h.model, Msg::Found(run, false));
    assert_eq!(st.step, Step::Failed(Failure::NotFound));
    assert!(!h.model.ui.claude_installed);
}

#[test]
fn a_failed_install_keeps_its_output_and_says_its_code() {
    let (mut h, mut st) = missing(Locale::En);
    let run = installing(&mut h, &mut st);
    let _ = update(
        &mut st,
        &mut h.model,
        Msg::Output(run, "curl: (22) 403".into()),
    );
    let _ = update(&mut st, &mut h.model, Msg::Exited(run, Ok(Some(22))));
    assert_eq!(st.step, Step::Failed(Failure::Exit(22)));
    assert_eq!(st.output, vec!["curl: (22) 403"]);

    // Try again: a run of its own, the old one's late lines ignored.
    let again = installing(&mut h, &mut st);
    assert!(st.output.is_empty());
    let _ = update(&mut st, &mut h.model, Msg::Output(run, "late".into()));
    let _ = update(&mut st, &mut h.model, Msg::Found(run, true));
    assert!(st.output.is_empty());
    assert_eq!(st.step, Step::Installing);
    let _ = update(&mut st, &mut h.model, Msg::Exited(again, Ok(None)));
    assert_eq!(st.step, Step::Failed(Failure::Stopped));
}

#[test]
fn signing_in_opens_claude_code_in_a_pane_of_the_workspace_on_screen() {
    let (mut h, mut st) = missing(Locale::En);
    st.step = Step::SignIn;
    h.model.ui.claude_installed = true;
    let before = h.model.workspace("w1").expect("w1").panes.len();
    let _ = update(&mut st, &mut h.model, Msg::SignIn);
    let panes = &h.model.workspace("w1").expect("w1").panes;
    assert_eq!(panes.len(), before + 1);
    assert_eq!(panes.last().map(|p| p.agent), Some(AgentId::Claude));
    assert_eq!(st.step, Step::Offer);
}

#[test]
fn signing_in_on_the_way_to_a_new_workspace_opens_its_dialog() {
    let (mut h, mut st) = missing(Locale::En);
    st.then_create = true;
    st.step = Step::SignIn;
    let before = h.model.workspace("w1").expect("w1").panes.len();
    let mut probe = State {
        then_create: true,
        ..State::default()
    };
    assert!(matches!(
        closing(&mut probe).as_slice(),
        [_, Intent::OpenOverlay(Overlay::WorkspaceDialog)]
    ));
    let _ = update(&mut st, &mut h.model, Msg::SignIn);
    assert_eq!(h.model.workspace("w1").expect("w1").panes.len(), before);
    assert!(!st.then_create);
}

#[test]
fn the_main_accounts_login_is_what_signed_in_reads() {
    let (mut h, _) = missing(Locale::En);
    assert!(signed_in(&h.model));
    h.model.live.accounts.accounts = Some(vec![crate::workspace_dialog::testing::account(
        "main",
        "Principal",
        false,
    )]);
    assert!(!signed_in(&h.model));
}

// ── drawn ─────────────────────────────────────────────────────────────

#[test]
fn the_offer_says_what_claude_code_is_and_how_out() {
    for (locale, texts, name) in [
        (
            Locale::En,
            [
                "Install Claude Code",
                "Use Hangar with shells only",
                "curl -fsSL https://claude.ai/install.sh | bash",
            ],
            "setup-offer-en-graphite",
        ),
        (
            Locale::Fr,
            [
                "Installer Claude Code",
                "Utiliser Hangar avec des shells uniquement",
                "curl -fsSL https://claude.ai/install.sh | bash",
            ],
            "setup-offer-fr-graphite",
        ),
    ] {
        let (h, st) = missing(locale);
        let mut ui = h.show(view(&st, h.ctx()));
        for text in texts {
            if cfg!(windows) && text.starts_with("curl") {
                continue;
            }
            assert!(ui.find(text).is_ok(), "{text}");
        }
        h.shot(&mut ui, name);
    }
}

#[test]
fn the_offer_installs_on_a_click() {
    let (h, st) = missing(Locale::En);
    let mut ui = h.show(view(&st, h.ctx()));
    let _ = ui.click("Install Claude Code");
    let messages = crate::workspace_dialog::testing::messages(ui);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Shell(Action::Setup(Msg::Install)))),
        "{messages:?}"
    );
}

#[test]
fn a_failure_shows_the_output_and_the_guide() {
    let (h, mut st) = missing(Locale::En);
    st.step = Step::Failed(Failure::Exit(22));
    st.output = vec!["curl: (22) The requested URL returned error: 403".into()];
    let mut ui = h.show(view(&st, h.ctx()));
    for text in [
        "The installation failed (exit code 22).",
        "curl: (22) The requested URL returned error: 403",
        "Open the install guide",
        "Try again",
        "Use Hangar with shells only",
    ] {
        assert!(ui.find(text).is_ok(), "{text}");
    }
    h.shot(&mut ui, "setup-failed-en-graphite");
}

#[test]
fn installed_offers_to_sign_in_until_signed_in() {
    let (mut h, mut st) = missing(Locale::En);
    st.step = Step::SignIn;
    h.model.live.accounts.accounts = Some(vec![crate::workspace_dialog::testing::account(
        "main",
        "Principal",
        false,
    )]);
    {
        let mut ui = h.show(view(&st, h.ctx()));
        assert!(ui.find("Sign in").is_ok());
        assert!(ui.find("Done").is_ok());
        assert!(ui.find("Use Hangar with shells only").is_err());
        h.shot(&mut ui, "setup-sign-in-en-graphite");
    }
    h.model.live.accounts.accounts = Some(vec![crate::workspace_dialog::testing::account(
        "main",
        "Principal",
        true,
    )]);
    let mut ui = h.show(view(&st, h.ctx()));
    assert!(ui.find("Signed in: Claude Code is ready.").is_ok());
    assert!(ui.find("Sign in").is_err());
}

/// `HANGAR_DEV_CLAUDE_INSTALLER` stands in for the official command in a
/// debug build (the tests' own), and an empty one does not.
#[test]
fn a_debug_build_takes_a_stand_in_installer() {
    assert_eq!(
        install::fake(Some("echo fake".into())),
        cfg!(debug_assertions).then(|| "echo fake".to_string())
    );
    assert_eq!(install::fake(Some("  ".into())), None);
    assert_eq!(install::fake(None), None);
}
