//! The navigator with several machines: the mark on a row, the filter by
//! machine, the rows of a machine that is switched off, the line at the
//! foot of the list.

use hangar_i18n::Locale;
use hangar_theme::{style, tokens};
use hangar_ui::Icon;
use iced::widget::{Space, container, row};
use iced::{Element, Length};
use iced_test::Simulator;

use super::model::MachineFilter;
use super::*;
use crate::nodes::model::{Failure, Step};
use crate::nodes::testing::{down, machine, up};
use crate::state::types::{AgentId, MachineKind, Pane, WorkspaceFolder};
use crate::workspace_dialog::testing::{Harness, messages};

fn on(id: &str, name: &str, node: Option<&str>, folder: Option<&str>) -> Workspace {
    Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/home/me/dev/{name}"),
        node: node.map(str::to_string),
        folder_id: folder.map(str::to_string),
        panes: vec![Pane {
            id: format!("{id}-p"),
            name: "Ava".into(),
            agent: AgentId::Claude,
            ..Pane::default()
        }],
        ..Workspace::default()
    }
}

/// Two workspaces of this computer, two on `GPU box` (connected), one on
/// `vps` (its link down for half a minute) and one on `nas`, which is
/// switched off.
fn machines(locale: Locale, theme: &str) -> Harness {
    let mut h = Harness::new(locale, theme).edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![
            machine("gpu", "GPU box", MachineKind::Tower),
            machine("vps", "vps", MachineKind::Cloud),
            machine("nas", "nas", MachineKind::MiniPc),
        ];
        state.settings.nodes[2].enabled = Some(false);
        state.folders = vec![WorkspaceFolder {
            id: "clients".into(),
            name: "Clients".into(),
            ..WorkspaceFolder::default()
        }];
        state.workspaces.extend([
            on("w3", "training", Some("gpu"), None),
            on("w4", "billing", Some("vps"), None),
            on("w5", "render-farm", Some("gpu"), Some("clients")),
            on("w6", "storefront", None, Some("clients")),
            on("w7", "archive", Some("nas"), None),
        ]);
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
    h
}

/// The navigator as the window frames it, at the left of the test's window.
fn framed<'a>(st: &'a State, h: &'a Harness) -> Element<'a, Message> {
    let t = h.model.theme();
    let sidebar = container(view(st, h.ctx()))
        .width(tokens::size::SIDEBAR_WIDTH)
        .height(Length::Fill)
        .style(style::container::sidebar(t));
    container(row![sidebar, Space::new().width(Length::Fill)])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::container::window(t))
        .into()
}

fn listed(st: &State, m: &Model) -> Vec<String> {
    rows_of(st, m).into_iter().map(|row| row.key).collect()
}

fn has(ui: &mut Simulator<'_, Message>, text: &str) -> bool {
    ui.find(text).is_ok()
}

#[test]
fn the_rows_of_a_machine_switched_off_are_out_of_the_list() {
    let h = machines(Locale::En, "graphite");
    let st = State::default();
    assert_eq!(
        listed(&st, &h.model),
        [
            "ws:w1",
            "ws:w2",
            "ws:w3",
            "ws:w4",
            "folder:clients",
            "ws:w5",
            "ws:w6"
        ]
    );
    // Switched back on, they are listed again; nothing was removed.
    let h = h.edit(|state| state.settings.nodes[2].enabled = None);
    assert!(listed(&st, &h.model).contains(&"ws:w7".to_string()));
    // Without the experiment the machines do not hide anything.
    let h = h.edit(|state| {
        state.settings.nodes[2].enabled = Some(false);
        state.settings.experiments.clear();
    });
    assert!(listed(&st, &h.model).contains(&"ws:w7".to_string()));
}

#[test]
fn the_list_narrows_to_one_machine() {
    let h = machines(Locale::En, "graphite");
    let mut st = State {
        machine: MachineFilter::Node("gpu".into()),
        ..State::default()
    };
    // The folder is held open over what it keeps, as under any filter.
    assert_eq!(listed(&st, &h.model), ["ws:w3", "folder:clients", "ws:w5"]);
    st.machine = MachineFilter::Local;
    assert_eq!(
        listed(&st, &h.model),
        ["ws:w1", "ws:w2", "folder:clients", "ws:w6"]
    );
    st.machine = MachineFilter::Node("vps".into());
    assert_eq!(listed(&st, &h.model), ["ws:w4"]);
    assert!(filtering(&st, &h.model));

    // A machine that is switched off, or gone, narrows nothing.
    st.machine = MachineFilter::Node("nas".into());
    assert_eq!(machine_filter(&st, &h.model), MachineFilter::All);
    st.machine = MachineFilter::Node("removed".into());
    assert_eq!(machine_filter(&st, &h.model), MachineFilter::All);
    assert!(!filtering(&st, &h.model));
}

#[test]
fn a_machine_s_label_typed_in_the_filter_finds_its_workspaces() {
    let mut h = machines(Locale::En, "graphite");
    let mut st = State::default();
    let _ = step(&mut st, &mut h.model, Msg::Query("gpu".into()));
    assert_eq!(listed(&st, &h.model), ["ws:w3", "folder:clients", "ws:w5"]);
    let _ = step(&mut st, &mut h.model, Msg::Query("VPS".into()));
    assert_eq!(listed(&st, &h.model), ["ws:w4"]);
    // Text and machine narrow together.
    let _ = step(&mut st, &mut h.model, Msg::Query("render".into()));
    let _ = step(&mut st, &mut h.model, Msg::Machine(MachineFilter::Local));
    assert!(listed(&st, &h.model).is_empty());
    // Clearing the filter clears the machine with the rest.
    let _ = step(&mut st, &mut h.model, Msg::ClearFilter);
    assert_eq!(st.machine, MachineFilter::All);
    assert_eq!(listed(&st, &h.model).len(), 7);
}

#[test]
fn a_remote_row_wears_its_machine_and_a_local_one_nothing() {
    let h = machines(Locale::En, "graphite");
    let st = State::default();
    let mut ui = h.show(framed(&st, &h));
    for name in ["hangar", "shop", "training", "billing", "render-farm"] {
        assert!(has(&mut ui, name), "{name} is listed");
    }
    assert!(!has(&mut ui, "archive"), "its machine is switched off");
    // The foot of the list: this computer and three nodes.
    assert!(has(&mut ui, "4 machines"));
    assert!(has(&mut ui, "1 offline"));
    assert!(has(&mut ui, "1 switched off"));
    h.shot(&mut ui, "machines-navigator-en-graphite");
    ui.click("4 machines").expect("the foot is a button");
    let opened = messages(ui).into_iter().any(|message| {
        matches!(
            message,
            Message::Intent(Intent::OpenSettings(Some(
                crate::shell::SettingsCategory::Nodes
            )))
        )
    });
    assert!(opened, "the foot opens Settings › Nodes");
}

#[test]
fn the_navigator_speaks_french_in_a_light_theme() {
    let h = machines(Locale::Fr, "github-light");
    let st = State::default();
    let mut ui = h.show(framed(&st, &h));
    assert!(has(&mut ui, "4 machines"));
    assert!(has(&mut ui, "1 hors ligne"));
    assert!(has(&mut ui, "1 désactivée"));
    h.shot(&mut ui, "machines-navigator-fr-github-light");
}

#[test]
fn the_filter_bar_offers_the_machines_only_while_there_is_a_node() {
    let h = machines(Locale::En, "graphite");
    let mut st = State::default();
    // Every machine, this computer, then each node that is on; `nas`,
    // switched off, has no row to keep.
    let offered = super::view::machine_choices(h.ctx());
    assert_eq!(
        offered,
        [
            (
                MachineFilter::All,
                Icon::Machines,
                "All machines".to_string()
            ),
            (
                MachineFilter::Local,
                Icon::Desktop,
                "This computer".to_string()
            ),
            (
                MachineFilter::Node("gpu".into()),
                Icon::Tower,
                "GPU box".to_string()
            ),
            (
                MachineFilter::Node("vps".into()),
                Icon::Cloud,
                "vps".to_string()
            ),
        ]
    );
    st.machine_menu = true;
    {
        let mut ui = h.show(framed(&st, &h));
        h.shot(&mut ui, "machines-navigator-menu-en-graphite");
    }
    st.machine_menu = false;
    st.machine = MachineFilter::Node("vps".into());
    {
        let mut ui = h.show(framed(&st, &h));
        assert!(has(&mut ui, "billing"));
        assert!(!has(&mut ui, "hangar"));
        h.shot(&mut ui, "machines-navigator-filtered-en-graphite");
    }

    // No node: no machine button, no foot.
    let plain = Harness::new(Locale::En, "graphite");
    let mut ui = plain.show(framed(&st, &plain));
    assert!(!has(&mut ui, "1 machine"));
    let frame_has_button = |h: &Harness| {
        let ctx = h.ctx();
        crate::nodes::available(ctx.model) && !ctx.model.settings().nodes.is_empty()
    };
    assert!(!frame_has_button(&plain));
    assert!(frame_has_button(&h));
}

#[test]
fn a_machine_with_no_workspace_says_so() {
    let h = machines(Locale::En, "graphite").edit(|state| {
        state
            .workspaces
            .retain(|ws| ws.node.as_deref() != Some("vps"));
    });
    let st = State {
        machine: MachineFilter::Node("vps".into()),
        ..State::default()
    };
    let mut ui = h.show(framed(&st, &h));
    assert!(has(&mut ui, "No workspace on vps."));
    assert!(has(&mut ui, "Clear the filter"));
}
