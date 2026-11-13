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
    assert!(
        !has(&mut ui, "1 switched off"),
        "a machine switched off needs nobody: it is in the tooltip"
    );
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
    assert!(!has(&mut ui, "1 désactivée"));
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

/// What a node runs itself is listed like any workspace of that machine:
/// `GPU box` (connected) runs `nightly`, one agent at work and one waiting,
/// and `evals`; `vps`, whose link is down, runs `billing-jobs`, shown as it
/// was last seen.
fn mirrors(locale: Locale, theme: &str) -> Harness {
    use crate::nodes::hosted::{self, OwnState};
    use crate::state::live::AgentActivity;
    let agent = |id: &str, name: &str| Pane {
        id: id.into(),
        name: name.into(),
        agent: AgentId::Claude,
        ..Pane::default()
    };
    let theirs = |id: &str, name: &str, panes: Vec<Pane>| Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/srv/{name}"),
        panes,
        ..Workspace::default()
    };
    let mut h = Harness::new(locale, theme).edit(|state| {
        state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
        state.settings.nodes = vec![
            machine("gpu", "GPU box", MachineKind::Tower),
            machine("vps", "vps", MachineKind::Cloud),
        ];
        state
            .workspaces
            .push(on("w3", "training", Some("gpu"), None));
    });
    up(&mut h.model, "gpu", "gpu-01");
    let own = |workspaces, activity: &[(&str, AgentActivity)]| OwnState {
        boot: "b1".into(),
        revision: 1,
        workspaces,
        activity: activity
            .iter()
            .map(|(pane, activity)| (pane.to_string(), *activity))
            .collect(),
        bridged: activity.iter().map(|(pane, _)| pane.to_string()).collect(),
        origins: None,
    };
    hosted::take_in(
        &mut h.model,
        "gpu",
        own(
            vec![
                theirs(
                    "o1",
                    "nightly",
                    vec![agent("o1-a", "Rorch"), agent("o1-b", "Wes")],
                ),
                theirs("o2", "evals", vec![agent("o2-a", "Kit")]),
            ],
            &[
                ("o1-a", AgentActivity::Working),
                ("o1-b", AgentActivity::Waiting),
            ],
        ),
        1_000,
    );
    hosted::take_in(
        &mut h.model,
        "vps",
        own(
            vec![theirs("o3", "billing-jobs", vec![agent("o3-a", "Bo")])],
            &[],
        ),
        1_000,
    );
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

#[test]
fn what_a_node_runs_itself_is_listed_among_the_workspaces() {
    let h = mirrors(Locale::En, "graphite");
    let st = State::default();
    assert_eq!(
        listed(&st, &h.model),
        ["ws:w1", "ws:w2", "ws:w3", "ws:o1", "ws:o2", "ws:o3"]
    );
    {
        let mut ui = h.show(framed(&st, &h));
        for name in ["training", "nightly", "evals", "billing-jobs"] {
            assert!(has(&mut ui, name), "{name} is listed");
        }
        assert!(has(&mut ui, "3 machines"));
        assert!(has(&mut ui, "1 offline"));
        h.shot(&mut ui, "hosted-navigator-en-graphite");
    }

    // Its agents count as any workspace's do, never opened though it is.
    let counts = h.model.live.panes_logic.workspace_activity("o1");
    assert_eq!((counts.working, counts.waiting), (1, 1));

    // Narrowed to one machine, its own workspaces come with those driven
    // from here.
    let narrowed = State {
        machine: MachineFilter::Node("gpu".into()),
        ..State::default()
    };
    assert_eq!(listed(&narrowed, &h.model), ["ws:w3", "ws:o1", "ws:o2"]);

    // Switched off, a machine takes what it runs itself out of the list
    // too; nothing is forgotten.
    let h = h.edit(|state| state.settings.nodes[0].enabled = Some(false));
    assert_eq!(listed(&st, &h.model), ["ws:w1", "ws:w2", "ws:o3"]);
    assert!(h.model.workspace("o1").is_some());
}

#[test]
fn the_mirrors_are_listed_in_french_in_a_light_theme() {
    let h = mirrors(Locale::Fr, "github-light");
    let st = State::default();
    let mut ui = h.show(framed(&st, &h));
    assert!(has(&mut ui, "nightly"));
    assert!(has(&mut ui, "1 hors ligne"));
    h.shot(&mut ui, "hosted-navigator-fr-github-light");
}

/// The lines of the menu of workspace `id`: each label, with why it is
/// greyed when it is.
fn menu_lines(h: &Harness, id: &str) -> Vec<(String, Option<String>)> {
    menu::workspace_entries(h.ctx(), id)
        .into_iter()
        .filter_map(|entry| match entry {
            hangar_ui::menu::Entry::Item(item) => {
                Some((item.label().to_owned(), item.refusal().map(str::to_owned)))
            }
            _ => None,
        })
        .collect()
}

fn line<'a>(lines: &'a [(String, Option<String>)], label: &str) -> Option<&'a Option<String>> {
    lines
        .iter()
        .find(|(found, _)| found == label)
        .map(|(_, refusal)| refusal)
}

/// A mirror its machine started again without stays in the list, marked:
/// its menu offers to run it there again, greyed with the reason while the
/// machine is away, and it is not moved, which it already was.
#[test]
fn a_mirror_its_machine_no_longer_has_offers_to_run_there_again() {
    let mut h = mirrors(Locale::En, "graphite");
    assert!(line(&menu_lines(&h, "o1"), "Run on GPU box again").is_none());
    h.model.live.nodes.lost.insert("o1".into());
    let lines = menu_lines(&h, "o1");
    assert_eq!(line(&lines, "Run on GPU box again"), Some(&None));
    assert!(line(&lines, "Delete workspace").is_some());
    let item = menu::workspace_entries(h.ctx(), "o1")
        .into_iter()
        .find_map(|entry| match entry {
            hangar_ui::menu::Entry::Item(item) if item.label() == "Run on GPU box again" => {
                Some(item)
            }
            _ => None,
        })
        .expect("the line");
    assert!(matches!(
        item.selects(),
        Some(Message::Navigator(Msg::Menu(menu::Action::RunAgain(ws)))) if ws == "o1"
    ));
    {
        let st = State {
            menu: Some(MenuState {
                kind: RowKind::Ws,
                id: "o1".into(),
                at: iced::Vector::new(24.0, 20.0),
                from_keyboard: true,
            }),
            ..State::default()
        };
        let mut ui = h.show(framed(&st, &h));
        h.shot(&mut ui, "lost-navigator-menu-en-graphite");
    }
    {
        let st = State::default();
        let mut ui = h.show(framed(&st, &h));
        h.shot(&mut ui, "lost-navigator-en-graphite");
    }
    // Its machine away: there is nobody to hand it to.
    h.model
        .live
        .nodes
        .statuses
        .insert("gpu".into(), crate::nodes::model::NodeStatus::Disconnected);
    let lines = menu_lines(&h, "o1");
    assert!(
        matches!(line(&lines, "Run on GPU box again"), Some(Some(_))),
        "{lines:?}"
    );
}

/// A workspace this computer still drives on a node can be handed to it
/// from its menu; one the node runs already, or one of this computer, has
/// no such line. A mirror is not put to sleep either.
#[test]
fn a_workspace_driven_on_a_node_offers_to_move_there() {
    let h = mirrors(Locale::En, "graphite");
    let driven = menu_lines(&h, "w3");
    assert_eq!(line(&driven, "Move to GPU box"), Some(&None));
    assert!(line(&driven, "Put to sleep").is_some());
    let mirror = menu_lines(&h, "o1");
    assert!(line(&mirror, "Move to GPU box").is_none());
    assert!(line(&mirror, "Put to sleep").is_none());
    assert!(line(&mirror, "Delete workspace").is_some());
    assert!(line(&menu_lines(&h, "w1"), "Move to GPU box").is_none());

    // Picked, the line asks first.
    let item = menu::workspace_entries(h.ctx(), "w3")
        .into_iter()
        .find_map(|entry| match entry {
            hangar_ui::menu::Entry::Item(item) if item.label() == "Move to GPU box" => Some(item),
            _ => None,
        })
        .expect("the line");
    assert!(matches!(
        item.selects(),
        Some(Message::Navigator(Msg::Menu(menu::Action::MoveToMachine(ws)))) if ws == "w3"
    ));

    let st = State {
        menu: Some(MenuState {
            kind: RowKind::Ws,
            id: "w3".into(),
            at: iced::Vector::new(24.0, 20.0),
            from_keyboard: true,
        }),
        ..State::default()
    };
    let mut ui = h.show(framed(&st, &h));
    h.shot(&mut ui, "move-navigator-menu-en-graphite");
}

/// While the move would break something the line is greyed and says why.
#[test]
fn the_move_line_is_greyed_with_its_reason() {
    let mut h = mirrors(Locale::En, "graphite");
    h.model.live.control.begin_for_test("w3", "t1");
    let lines = menu_lines(&h, "w3");
    assert_eq!(
        line(&lines, "Move to GPU box"),
        Some(&Some(
            "training cannot move to GPU box while one of its deliveries runs.".to_string()
        ))
    );
}

/// The question a move asks, in plain words, with the agents it interrupts.
#[test]
fn the_move_asks_first_and_says_what_happens() {
    use crate::nodes::Ask;
    let mut h = mirrors(Locale::En, "graphite");
    let ask = Ask::Move("w3".into());
    {
        let mut ui = h.show(crate::nodes::ask::view(h.ctx(), &ask));
        for text in [
            "Move training to GPU box?",
            "Its agents are restarted on GPU box and pick up their Claude sessions.",
            "From then on it keeps running with this computer off.",
            "This cannot be undone from here.",
            "Cancel",
            "Move to GPU box",
        ] {
            assert!(has(&mut ui, text), "{text}");
        }
        assert!(!has(&mut ui, "Mia is working and will be interrupted."));
        h.shot(&mut ui, "move-confirm-en-graphite");
        ui.click("Move to GPU box").expect("the yes");
        let sent = messages(ui);
        assert!(sent.iter().any(|message| matches!(
            message,
            Message::Nodes(crate::nodes::Msg::Answer(Ask::Move(ws), true)) if ws == "w3"
        )));
    }
    // An agent in the middle of a turn is named.
    h.model.live.panes_logic.seed_activity(
        "w3-p",
        "w3",
        crate::state::live::AgentActivity::Working,
    );
    let mut ui = h.show(crate::nodes::ask::view(h.ctx(), &ask));
    assert!(has(&mut ui, "Mia is working and will be interrupted."));
    h.shot(&mut ui, "move-confirm-working-en-graphite");
}

#[test]
fn the_questions_are_asked_in_french_too() {
    use crate::nodes::Ask;
    let h = mirrors(Locale::Fr, "github-light");
    let (moved, removed) = (Ask::Move("w3".into()), Ask::Remove("o1".into()));
    {
        let mut ui = h.show(crate::nodes::ask::view(h.ctx(), &moved));
        assert!(has(&mut ui, "Déplacer training vers GPU box\u{202f}?"));
        h.shot(&mut ui, "move-confirm-fr-github-light");
    }
    let mut ui = h.show(crate::nodes::ask::view(h.ctx(), &removed));
    assert!(has(
        &mut ui,
        "Il tourne sur GPU box\u{202f}: ses agents y sont arrêtés."
    ));
    h.shot(&mut ui, "hosted-remove-confirm-fr-github-light");
}

/// Deleting a workspace its node runs asks first, worded for what it is.
#[test]
fn deleting_a_mirror_asks_first() {
    use crate::nodes::Ask;
    let mut h = mirrors(Locale::En, "graphite");
    let mut st = State::default();
    let _ = run_action(
        &mut st,
        &mut h.model,
        menu::Action::RemoveWorkspace("o1".into()),
    );
    assert!(h.model.workspace("o1").is_some(), "not before the yes");
    let ask = Ask::Remove("o1".into());
    {
        let mut ui = h.show(crate::nodes::ask::view(h.ctx(), &ask));
        for text in [
            "Delete nightly?",
            "It runs on GPU box: its agents there are stopped.",
            "Its folder stays on GPU box.",
            "Delete workspace",
        ] {
            assert!(has(&mut ui, text), "{text}");
        }
        h.shot(&mut ui, "hosted-remove-confirm-en-graphite");
    }
    // Its node away, nothing is asked: the refusal is said instead.
    let _ = run_action(
        &mut st,
        &mut h.model,
        menu::Action::RemoveWorkspace("o3".into()),
    );
    assert!(h.model.workspace("o3").is_some());
    assert!(h.model.live.nodes.refused.is_some());
}

/// [`machines`], with one repository open in three places: `shop` here,
/// `training` on GPU box and `billing` on vps. `storefront`, here, is a
/// repository open nowhere else; `hangar` has no origin.
fn shared(locale: Locale, theme: &str) -> Harness {
    let mut h = machines(locale, theme);
    let said = [
        ("w2", "git@github.com:Acme/Shop.git"),
        ("w3", "https://github.com/acme/shop"),
        ("w4", "ssh://git@github.com/acme/shop.git"),
        ("w6", "git@github.com:acme/storefront.git"),
    ];
    for (id, origin) in said {
        let ws = h.model.workspace(id).unwrap().clone();
        h.model.live.nodes.repos.said(&ws, Some(origin));
    }
    h
}

/// A repository open on several machines is said on each of its rows, by
/// one mark: a row of this computer gets a faint one, a row that has its
/// machine's says it in that mark's tooltip. No row moves.
#[test]
fn a_repository_open_on_several_machines_is_said_on_each_row() {
    use crate::nodes::{presence, repos};
    let h = shared(Locale::En, "graphite");
    let st = State::default();
    let tr = h.ctx().i18n;
    let model = &h.model;
    let ws = |id: &str| model.workspace(id).unwrap();
    assert_eq!(
        repos::also_on(model, tr, ws("w2")).as_deref(),
        Some("Also on GPU box (training), vps (billing)")
    );
    let now = h.ctx().now;
    let tip = presence::tip_of(model, tr, "gpu", now);
    assert_eq!(
        repos::tip_with_others(model, tr, ws("w3"), tip),
        "GPU box · connected · also on this computer (shop), vps (billing)"
    );
    // Open here only, or with no origin: nothing is added.
    assert_eq!(repos::also_on(model, tr, ws("w6")), None);
    assert_eq!(repos::also_on(model, tr, ws("w1")), None);
    let tip = presence::tip_of(model, tr, "gpu", now);
    assert_eq!(
        repos::tip_with_others(model, tr, ws("w5"), tip.clone()),
        tip,
        "render-farm is another repository"
    );
    assert_eq!(
        listed(&st, model),
        listed(&st, &machines(Locale::En, "graphite").model),
        "a workspace stays where the person put it"
    );
    let mut ui = h.show(framed(&st, &h));
    h.shot(&mut ui, "repos-navigator-en-graphite");
}

#[test]
fn the_marks_of_a_shared_repository_in_french_in_a_light_theme() {
    let h = shared(Locale::Fr, "github-light");
    let st = State::default();
    let tr = h.ctx().i18n;
    let shop = h.model.workspace("w2").unwrap();
    assert_eq!(
        crate::nodes::repos::also_on(&h.model, tr, shop).as_deref(),
        Some("Aussi sur GPU box (training), vps (billing)")
    );
    let mut ui = h.show(framed(&st, &h));
    h.shot(&mut ui, "repos-navigator-fr-github-light");
}

/// The entries of the menu of workspace `id`, headings included.
fn menu_entries(h: &Harness, id: &str) -> Vec<String> {
    menu::workspace_entries(h.ctx(), id)
        .into_iter()
        .map(|entry| match entry {
            hangar_ui::menu::Entry::Item(item) => item.label().to_owned(),
            hangar_ui::menu::Entry::Heading(label) => format!("# {label}"),
            hangar_ui::menu::Entry::Separator => "-".to_owned(),
        })
        .collect()
}

fn menu_item(h: &Harness, id: &str, label: &str) -> hangar_ui::menu::Item<Message> {
    menu::workspace_entries(h.ctx(), id)
        .into_iter()
        .find_map(|entry| match entry {
            hangar_ui::menu::Entry::Item(item) if item.label() == label => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {label} in the menu of {id}"))
}

/// The menu of a workspace goes to the other checkouts of its repository,
/// each under its machine's icon, and offers to open it on one more
/// machine while one is connected that lacks it.
#[test]
fn the_menu_goes_to_the_other_machines_and_opens_on_one_more() {
    let h = shared(Locale::En, "graphite");
    // `training`, on GPU box: also here and on vps, at the head of the
    // menu; nowhere left to open it, said beside the move.
    let entries = menu_entries(&h, "w3");
    assert_eq!(
        entries[..4],
        ["# Also on", "This computer (shop)", "vps (billing)", "-"]
    );
    let at = entries
        .iter()
        .position(|entry| entry == "Open on another machine…")
        .expect("the line");
    assert_eq!(entries[at - 1], "-");
    assert_eq!(entries[at + 1], "Move to GPU box");
    let here = menu_item(&h, "w3", "This computer (shop)");
    assert_eq!(here.lead(), Some(Icon::Desktop));
    assert!(matches!(
        here.selects(),
        Some(Message::Navigator(Msg::Menu(menu::Action::Intent(
            Intent::ActivateWorkspace(ws)
        )))) if ws == "w2"
    ));
    assert_eq!(
        menu_item(&h, "w3", "vps (billing)").lead(),
        Some(Icon::Cloud)
    );
    let open = menu_item(&h, "w3", "Open on another machine…");
    assert_eq!(open.lead(), Some(Icon::MachinePlus));
    assert_eq!(
        open.refusal(),
        Some("Every connected machine has this repository already.")
    );

    // `storefront` is open here only: no other machine to go to, one to
    // open it on.
    let entries = menu_entries(&h, "w6");
    assert!(!entries.contains(&"# Also on".to_string()));
    let open = menu_item(&h, "w6", "Open on another machine…");
    assert_eq!(open.refusal(), None);
    assert!(matches!(
        open.selects(),
        Some(Message::Navigator(Msg::Menu(menu::Action::Intent(
            Intent::OpenOnAnotherMachine(ws)
        )))) if ws == "w6"
    ));
    // No repository, no line; nor without a node.
    assert!(!menu_entries(&h, "w1").contains(&"Open on another machine…".to_string()));
    let alone = shared(Locale::En, "graphite").edit(|state| state.settings.nodes.clear());
    assert!(!menu_entries(&alone, "w6").contains(&"Open on another machine…".to_string()));

    let st = State {
        menu: Some(MenuState {
            kind: RowKind::Ws,
            id: "w3".into(),
            at: iced::Vector::new(24.0, 20.0),
            from_keyboard: true,
        }),
        ..State::default()
    };
    let mut ui = h.show(framed(&st, &h));
    h.shot(&mut ui, "repos-navigator-menu-en-graphite");
}

/// The foot says how many machines and, in the warning colour, how many
/// need a look; those switched off are in its tooltip. Its words fit the
/// navigator's width in French, the longer of the two languages, with more
/// machines than anybody has.
#[test]
fn the_foot_fits_the_navigator_in_french_whatever_the_count() {
    use crate::nodes::presence::Census;
    use hangar_i18n::Translator;
    use hangar_theme::fonts;
    use hangar_theme::tokens::{font_size, space};
    let h = machines(Locale::Fr, "github-light");
    let st = State::default();
    // The fonts the window sets its text in are loaded with a window.
    let mut ui = h.show(framed(&st, &h));
    assert!(has(&mut ui, "4 machines"));

    let said = super::view::foot_words(
        h.ctx().i18n,
        crate::nodes::presence::census(&h.model, h.ctx().now),
    );
    assert_eq!(said.count, "4 machines");
    assert_eq!(said.offline.as_deref(), Some("1 hors ligne"));
    assert_eq!(
        said.tip,
        "Les machines, dans Réglages › Nœuds · 1 désactivée"
    );
    let en = super::view::foot_words(
        Translator::new(Locale::En),
        Census {
            machines: 2,
            offline: 0,
            off: 0,
        },
    );
    assert_eq!(en.count, "2 machines");
    assert_eq!(en.offline, None);
    assert_eq!(en.tip, "Machines, in Settings › Nodes");

    for locale in [Locale::Fr, Locale::En] {
        let worst = super::view::foot_words(
            Translator::new(locale),
            Census {
                machines: 999,
                offline: 998,
                off: 1,
            },
        );
        let width = |words: &str| hangar_ui::text_width(words, font_size::MICRO, fonts::ui());
        let needed = width(&worst.count) + space::SM + width(&worst.offline.unwrap());
        assert!(
            needed <= super::view::FOOT_ROOM,
            "{needed} px of words in {} px",
            super::view::FOOT_ROOM
        );
        // With room to spare: nothing sits against the edge.
        assert!(needed <= super::view::FOOT_ROOM - 24.0, "{needed}");
    }
}

/// The filter's button shows the machine the list is narrowed to, and a
/// plain filter otherwise: never the machines' glyph, which is the foot's.
#[test]
fn the_filter_button_wears_the_machine_it_keeps() {
    let h = machines(Locale::En, "graphite");
    let icon = |machine: MachineFilter| super::view::filter_icon(h.ctx(), &machine);
    assert_eq!(icon(MachineFilter::All), Icon::Filter);
    assert_eq!(icon(MachineFilter::Local), Icon::Desktop);
    assert_eq!(icon(MachineFilter::Node("gpu".into())), Icon::Tower);
    assert_eq!(icon(MachineFilter::Node("vps".into())), Icon::Cloud);
}
