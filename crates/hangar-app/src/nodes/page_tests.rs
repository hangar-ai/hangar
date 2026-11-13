//! Settings › Nodes drawn: the machines as rows, each saying how its link
//! is doing, and what a row's icon, switch and menu ask for.

use hangar_i18n::Locale;
use std::time::Instant;

use iced_test::Simulator;

use super::model::{Failure, NodeStatus, Step};
use super::testing::{down, machine, up};
use super::{Msg, Open, State, settings_page};
use crate::ctx::Ctx;
use crate::message::Message;
use crate::state::StoreAction;
use crate::state::types::{MachineKind, Workspace};
use crate::workspace_dialog::testing::{Harness, messages};

/// Three nodes: `GPU box` connected (no git there, two workspaces of its
/// own), `vps` offline after a failed attempt, `nas` switched off.
fn page(locale: Locale, theme: &str) -> Harness {
    page_at(locale, theme).0
}

/// The same, with the moment its links are as described: the page drawn
/// then counts the same seconds whenever the test runs.
fn page_at(locale: Locale, theme: &str) -> (Harness, Instant) {
    let mut h = Harness::new(locale, theme).edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![
            machine("gpu", "GPU box", MachineKind::Tower),
            machine("vps", "vps", MachineKind::Cloud),
            machine("nas", "nas", MachineKind::MiniPc),
        ];
        state.settings.nodes[1].ssh_config = Some("~/.ssh/work".into());
        state.settings.nodes[2].enabled = Some(false);
    });
    up(&mut h.model, "gpu", "gpu-01");
    if let Some(NodeStatus::Connected { info, .. }) = h.model.live.nodes.statuses.get_mut("gpu") {
        info.git = false;
    }
    // What `gpu` runs itself, mirrored here.
    let own = |id: &str, name: &str| Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/srv/{name}"),
        ..Workspace::default()
    };
    h.model.apply(StoreAction::SyncHosted {
        node: "gpu".into(),
        workspaces: vec![own("o1", "nightly"), own("o2", "render-farm")],
        keep: Vec::new(),
    });
    let at = down(
        &mut h.model,
        "vps",
        40,
        Some(Failure::Ssh {
            step: Step::Probe,
            detail: "ssh: connect to host vps port 22: Connection timed out".into(),
        }),
    );
    (h, at)
}

/// The page as it is drawn at `now`.
fn drawn_at<'a>(st: &'a State, h: &'a Harness, now: Instant) -> iced::Element<'a, Message> {
    settings_page(st, Ctx { now, ..h.ctx() })
}

fn sent(ui: Simulator<'_, Message>, wanted: impl Fn(&Msg) -> bool) -> bool {
    messages(ui)
        .into_iter()
        .any(|message| matches!(&message, Message::Nodes(msg) if wanted(msg)))
}

#[test]
fn each_machine_is_a_row_that_says_how_its_link_is_doing() {
    let (h, at) = page_at(Locale::En, "graphite");
    let st = State::default();
    let mut ui = h.show(drawn_at(&st, &h, at));
    for text in [
        // This computer heads the list.
        "This computer",
        "GPU box",
        "me@gpu · Connected · hangar-node 2.3.0 on gpu-01",
        "Claude Code",
        "No git",
        "Runs 2 workspaces itself, listed in the navigator",
        "vps",
        "me@vps · -F ~/.ssh/work",
        "Offline · retrying in 8\u{a0}s: unreachable over SSH (Connection timed out)",
        // What ssh said in full, apart and quieter.
        "ssh: connect to host vps port 22: Connection timed out",
        "nas",
        "me@nas · Switched off",
        "Add a machine",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the page");
    }
    // Under its name, this computer's own, when the system says it: not
    // the same thing said twice.
    assert!(ui.find("This machine").is_err());
    if let Some(hostname) = super::page::local_hostname() {
        assert!(
            ui.find(hostname).is_ok(),
            "{hostname} is under This computer"
        );
    }
    h.shot(&mut ui, "machines-settings-en-graphite");
}

#[test]
fn the_page_speaks_french_in_a_light_theme() {
    let (h, at) = page_at(Locale::Fr, "github-light");
    let st = State::default();
    let mut ui = h.show(drawn_at(&st, &h, at));
    for text in [
        "Cet ordinateur",
        "me@gpu · Connecté · hangar-node 2.3.0 sur gpu-01",
        "me@nas · Désactivé",
        "Ajouter une machine",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the page");
    }
    h.shot(&mut ui, "machines-settings-fr-github-light");
}

#[test]
fn a_link_on_its_way_says_which_step_and_one_that_dropped_when_it_tries_again() {
    let mut h = page(Locale::En, "graphite");
    let st = State::default();
    // Never connected yet: connecting, with the step under way.
    h.model.live.nodes.links.remove("gpu");
    h.model
        .live
        .nodes
        .statuses
        .insert("gpu".into(), NodeStatus::Connecting(Step::Tunnel));
    // Dropped, no attempt failed yet: reconnecting, with the seconds left.
    let at = down(&mut h.model, "vps", 1, None);
    {
        let mut ui = h.show(drawn_at(&st, &h, at));
        assert!(
            ui.find("me@gpu · Connecting: Opening the tunnel…").is_ok(),
            "the first connection"
        );
        assert!(
            ui.find("Reconnecting in 8\u{a0}s: the tunnel closed (Connection reset)")
                .is_ok(),
            "said at once in the settings, without the grace period"
        );
        h.shot(&mut ui, "machines-settings-connecting-en-graphite");
    }
    // A new attempt after a drop is a reconnection.
    h.model
        .live
        .nodes
        .statuses
        .insert("vps".into(), NodeStatus::Connecting(Step::Probe));
    let mut ui = h.show(settings_page(&st, h.ctx()));
    assert!(
        ui.find("me@vps · -F ~/.ssh/work · Reconnecting: Looking at the machine…")
            .is_ok()
    );
}

/// The picker of a row's kind, open under its icon. What it lists is the
/// kinds and their icons (`presence`'s tests); here it is drawn.
#[test]
fn the_icon_of_a_row_opens_the_picker_of_its_kind() {
    let h = page(Locale::En, "graphite");
    let st = State {
        open: Some(Open::Kind("vps".into())),
        ..State::default()
    };
    let mut ui = h.show(settings_page(&st, h.ctx()));
    assert!(ui.find("vps").is_ok());
    h.shot(&mut ui, "machines-settings-kind-en-graphite");
}

fn menu_of(h: &Harness, id: &str) -> Vec<(String, bool, Option<Msg>)> {
    let ctx = h.ctx();
    let node = super::presence::config(ctx.model, id).expect("a node of the settings");
    super::page::more_lines(ctx, node)
        .into_iter()
        .map(|line| (ctx.t(line.label).into_owned(), line.danger, line.send))
        .collect()
}

#[test]
fn the_menu_of_a_row_retries_renames_and_removes() {
    let h = page(Locale::En, "graphite");
    let lines = menu_of(&h, "vps");
    let labels: Vec<&str> = lines.iter().map(|(label, ..)| label.as_str()).collect();
    assert_eq!(
        labels,
        ["Retry now", "Rename", "Remove from this computer…"]
    );
    assert!(
        matches!(&lines[0].2, Some(Msg::Retry(node)) if node == "vps"),
        "a link that is down can be retried"
    );
    assert!(matches!(
        &lines[1].2,
        Some(Msg::Show(Some(Open::Rename { id, draft }))) if id == "vps" && draft == "vps"
    ));
    // While an attempt is running there is none to bring forward: the
    // line is there, and cannot be picked.
    let mut trying = page(Locale::En, "graphite");
    trying
        .model
        .live
        .nodes
        .statuses
        .insert("vps".into(), NodeStatus::Connecting(Step::Install));
    let lines = menu_of(&trying, "vps");
    assert_eq!(lines[0].0, "Retry now");
    assert!(lines[0].2.is_none(), "nothing to retry mid-attempt");
    assert!(lines[2].1, "the removal is marked as one");
    assert!(
        matches!(&lines[2].2, Some(Msg::Show(Some(Open::Remove(node)))) if node == "vps"),
        "it asks before removing"
    );

    let mut st = State {
        open: Some(Open::More("vps".into())),
        ..State::default()
    };
    {
        let mut ui = h.show(settings_page(&st, h.ctx()));
        h.shot(&mut ui, "machines-settings-menu-en-graphite");
    }
    // The question, under the row, and its yes.
    st.open = Some(Open::Remove("vps".into()));
    {
        let mut ui = h.show(settings_page(&st, h.ctx()));
        assert!(ui.find("Remove vps from this computer? Nothing is deleted on the machine. The workspaces it runs itself leave the list here; those this computer drives there stay, without a machine to run on.").is_ok());
        h.shot(&mut ui, "machines-settings-remove-en-graphite");
        ui.click("Remove").expect("the yes");
        assert!(sent(
            ui,
            |msg| matches!(msg, Msg::Remove(node) if node == "vps")
        ));
    }
    // A name being typed over the row's.
    st.open = Some(Open::Rename {
        id: "vps".into(),
        draft: "Hetzner".into(),
    });
    let mut ui = h.show(settings_page(&st, h.ctx()));
    h.shot(&mut ui, "machines-settings-rename-en-graphite");
    ui.click("Save").expect("the name is saved");
    assert!(sent(ui, |msg| matches!(msg, Msg::RenameSubmit)));
}

#[test]
fn a_machine_that_is_connected_or_switched_off_has_nothing_to_retry() {
    let h = page(Locale::En, "graphite");
    assert!(menu_of(&h, "gpu")[0].2.is_none(), "connected");
    assert!(menu_of(&h, "nas")[0].2.is_none(), "switched off");
}

#[test]
fn with_no_node_the_list_is_this_computer_and_the_form() {
    let h = Harness::new(Locale::En, "graphite").edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
    });
    let st = State::default();
    let mut ui = h.show(settings_page(&st, h.ctx()));
    assert!(ui.find("This computer").is_ok());
    assert!(ui.find("No other machine yet.").is_ok());
    assert!(ui.find("Add a machine").is_ok());
    assert_eq!(st.form.kind, MachineKind::Server, "a server until told");
}

/// Under a machine's row, how many workspaces there still stop with this
/// computer: one quiet line, and nothing when there is none.
#[test]
fn a_machine_says_how_many_workspaces_still_need_this_computer() {
    let on_gpu = |id: &str, name: &str| Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/srv/{name}"),
        node: Some("gpu".into()),
        ..Workspace::default()
    };
    let (h, at) = page_at(Locale::En, "graphite");
    let st = State::default();
    {
        let mut ui = h.show(drawn_at(&st, &h, at));
        assert!(
            ui.find("1 workspace here still needs this computer on")
                .is_err()
        );
    }
    let mut h = h;
    h.model
        .apply(StoreAction::PutWorkspace(Box::new(on_gpu("d1", "api"))));
    {
        let mut ui = h.show(drawn_at(&st, &h, at));
        assert!(
            ui.find("1 workspace here still needs this computer on")
                .is_ok()
        );
        assert!(
            ui.find("Runs 2 workspaces itself, listed in the navigator")
                .is_ok()
        );
    }
    h.model
        .apply(StoreAction::PutWorkspace(Box::new(on_gpu("d2", "site"))));
    let mut ui = h.show(drawn_at(&st, &h, at));
    assert!(
        ui.find("2 workspaces here still need this computer on")
            .is_ok()
    );
    h.shot(&mut ui, "machines-settings-still-here-en-graphite");
}

/// A failure's row: what failed in a short sentence, what the tool said
/// under it; in French too.
#[test]
fn a_failure_is_a_short_sentence_over_what_the_tool_said() {
    let (h, at) = page_at(Locale::Fr, "github-light");
    let st = State::default();
    let mut ui = h.show(drawn_at(&st, &h, at));
    let vps = &h.model.settings().nodes[1];
    let ctx = Ctx { now: at, ..h.ctx() };
    let (sentence, alert) = super::page::state_words(ctx, vps);
    assert!(alert);
    assert!(
        sentence.starts_with("Hors ligne · nouvel essai dans 8")
            && sentence.ends_with("injoignable en SSH (Connection timed out)"),
        "{sentence}"
    );
    for text in [
        sentence.as_str(),
        "ssh: connect to host vps port 22: Connection timed out",
    ] {
        assert!(ui.find(text).is_ok(), "{text} is on the page");
    }
    assert_eq!(
        super::page::failure_detail(ctx, vps).as_deref(),
        Some("ssh: connect to host vps port 22: Connection timed out")
    );
    // A machine that is up or switched off has nothing of the kind.
    assert_eq!(
        super::page::failure_detail(ctx, &h.model.settings().nodes[0]),
        None
    );
    assert_eq!(
        super::page::failure_detail(ctx, &h.model.settings().nodes[2]),
        None
    );
    h.shot(&mut ui, "machines-settings-failure-fr-github-light");
}
