//! The one mapping from a link's story to what is shown of it.

use std::time::{Duration, Instant};

use hangar_i18n::{Locale, Translator};
use hangar_ui::Icon;

use super::*;
use crate::nodes::model::{NodeInfo, Step};
use crate::nodes::testing::{down, machine, up};
use crate::state::AppState;
use crate::state::types::MachineKind;

fn model() -> Model {
    let mut state = AppState::default();
    state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
    state.settings.nodes = vec![
        machine("gpu", "GPU box", MachineKind::Tower),
        machine("vps", "vps", MachineKind::Cloud),
    ];
    let mut model = Model::default();
    model.hydrate(state);
    model
}

fn ssh_failure() -> Failure {
    Failure::Ssh {
        step: Step::Probe,
        detail: "Connection timed out".into(),
    }
}

fn connected() -> NodeStatus {
    NodeStatus::Connected {
        info: NodeInfo::default(),
        local_port: 1,
        token: "t".into(),
    }
}

#[test]
fn a_link_remembers_when_it_dropped_and_why_an_attempt_failed() {
    let start = Instant::now();
    let mut link = Link::default();
    link.note(
        &NodeStatus::Disconnected,
        &NodeStatus::Connecting(Step::Probe),
        start,
    );
    assert_eq!(link.down_since, Some(start));
    assert!(!link.was_connected);

    link.note(&NodeStatus::Connecting(Step::Probe), &connected(), start);
    assert_eq!(
        link,
        Link {
            was_connected: true,
            ..Link::default()
        }
    );

    // The tunnel drops: down from now, but no attempt has failed yet.
    let dropped = start + Duration::from_secs(60);
    let drop = NodeStatus::Unreachable {
        failure: Failure::Tunnel("reset".into()),
        retry_in: 2,
    };
    link.note(&connected(), &drop, dropped);
    assert_eq!(link.down_since, Some(dropped));
    assert_eq!(link.failure, None);
    assert_eq!(link.retry_at, Some(dropped + Duration::from_secs(2)));
    assert_eq!(link.retry_in(dropped + Duration::from_millis(500)), Some(2));
    assert_eq!(link.retry_in(dropped + Duration::from_secs(5)), Some(0));

    // The attempt that follows fails: that is its reason, and the time it
    // went down stays the drop's.
    let later = dropped + Duration::from_secs(3);
    link.note(&drop, &NodeStatus::Connecting(Step::Probe), later);
    assert_eq!(link.retry_at, None);
    let failed = NodeStatus::Unreachable {
        failure: ssh_failure(),
        retry_in: 4,
    };
    link.note(&NodeStatus::Connecting(Step::Probe), &failed, later);
    assert_eq!(link.down_since, Some(dropped));
    assert_eq!(link.failure, Some(ssh_failure()));
    assert_eq!(link.down_for(later), Some(Duration::from_secs(3)));
    assert!(link.was_connected);
}

#[test]
fn a_link_says_nothing_until_the_grace_period_is_over() {
    let mut m = model();
    let now = Instant::now();
    // Not tried yet: the app has just started.
    assert_eq!(of(&m, "gpu", now), Presence::Settling);

    up(&mut m, "gpu", "gpu-01");
    assert_eq!(of(&m, "gpu", now), Presence::Connected);

    let at = down(&mut m, "gpu", 2, None);
    assert_eq!(of(&m, "gpu", at), Presence::Settling);
    // The grace period ends on time.
    assert_eq!(
        of(&m, "gpu", at + Duration::from_secs(3)),
        Presence::Connecting { again: true }
    );
    // The settings do not wait to say it.
    assert_eq!(exact(&m, "gpu"), Presence::Connecting { again: true });

    let at = down(&mut m, "gpu", 6, None);
    assert_eq!(of(&m, "gpu", at), Presence::Connecting { again: true });

    let at = down(&mut m, "gpu", 6, Some(ssh_failure()));
    assert_eq!(of(&m, "gpu", at), Presence::Offline(Some(ssh_failure())));
    // A new attempt under way does not take the reason away.
    m.live
        .nodes
        .statuses
        .insert("gpu".into(), NodeStatus::Connecting(Step::Tunnel));
    assert_eq!(of(&m, "gpu", at), Presence::Offline(Some(ssh_failure())));
}

#[test]
fn a_machine_that_never_connected_is_connecting_not_reconnecting() {
    let mut m = model();
    let since = Instant::now();
    m.live.nodes.links.insert(
        "vps".into(),
        Link {
            down_since: Some(since),
            ..Link::default()
        },
    );
    let later = since + GRACE + Duration::from_secs(1);
    assert_eq!(of(&m, "vps", later), Presence::Connecting { again: false });
}

#[test]
fn switched_off_removed_and_without_the_experiment() {
    let mut m = model();
    let now = Instant::now();
    let mut state = m.state().clone();
    state.settings.nodes[1].enabled = Some(false);
    m.hydrate(state.clone());
    assert_eq!(of(&m, "vps", now), Presence::Off);
    assert_eq!(of(&m, "gone", now), Presence::Offline(None));

    state.settings.experiments.clear();
    m.hydrate(state);
    assert_eq!(of(&m, "gpu", now), Presence::Offline(None));
    assert_eq!(of(&m, "vps", now), Presence::Offline(None));
}

#[test]
fn each_state_has_one_look() {
    let calm = Look {
        icon: Icon::Tower,
        tone: Tone::Muted,
        dimmed: false,
    };
    assert_eq!(look(MachineKind::Tower, &Presence::Connected), calm);
    // Within the grace period nothing changes.
    assert_eq!(look(MachineKind::Tower, &Presence::Settling), calm);
    let alarm = Look {
        icon: Icon::Unplugged,
        tone: Tone::Warning,
        dimmed: true,
    };
    for presence in [
        Presence::Connecting { again: true },
        Presence::Connecting { again: false },
        Presence::Offline(Some(ssh_failure())),
        Presence::Offline(None),
    ] {
        assert_eq!(look(MachineKind::Tower, &presence), alarm, "{presence:?}");
        assert!(presence.is_down());
    }
    assert_eq!(
        look(MachineKind::Tower, &Presence::Off),
        Look {
            icon: Icon::Tower,
            tone: Tone::Muted,
            dimmed: true,
        }
    );
    assert!(!Presence::Off.is_down());
}

#[test]
fn every_kind_has_its_icon_and_its_name() {
    let tr = Translator::new(Locale::En);
    let mut icons = Vec::new();
    let mut names = Vec::new();
    for kind in MachineKind::ALL {
        icons.push(kind_icon(kind));
        names.push(tr.t(kind_key(kind)).into_owned());
    }
    assert_eq!(
        icons,
        [
            Icon::Server,
            Icon::Cloud,
            Icon::Laptop,
            Icon::Desktop,
            Icon::MiniPc,
            Icon::Tower
        ]
    );
    assert_eq!(
        names,
        ["Server", "Cloud", "Laptop", "Desktop", "Mini PC", "Tower"]
    );
}

#[test]
fn a_tooltip_is_the_label_and_the_state_in_words() {
    let en = Translator::new(Locale::En);
    let fr = Translator::new(Locale::Fr);
    assert_eq!(tip(en, "vps", &Presence::Connected), "vps · connected");
    assert_eq!(
        tip(en, "vps", &Presence::Connecting { again: true }),
        "vps · reconnecting…"
    );
    assert_eq!(
        tip(en, "vps", &Presence::Connecting { again: false }),
        "vps · connecting…"
    );
    assert_eq!(tip(en, "vps", &Presence::Off), "vps · switched off");
    assert_eq!(
        tip(en, "vps", &Presence::Offline(Some(ssh_failure()))),
        "vps · offline: unreachable over SSH (Connection timed out)"
    );
    assert_eq!(tip(fr, "vps", &Presence::Off), "vps · désactivé");
}

#[test]
fn the_census_counts_this_computer_and_what_is_down_or_off() {
    let mut m = model();
    let now = Instant::now();
    assert_eq!(
        census(&m, now),
        Census {
            machines: 3,
            offline: 0,
            off: 0
        }
    );
    down(&mut m, "gpu", 30, Some(ssh_failure()));
    let mut state = m.state().clone();
    state.settings.nodes[1].enabled = Some(false);
    m.hydrate(state);
    assert_eq!(
        census(&m, Instant::now()),
        Census {
            machines: 3,
            offline: 1,
            off: 1
        }
    );
}

#[test]
fn a_node_goes_by_its_kind_s_icon_and_a_removed_one_by_a_server_s() {
    let m = model();
    assert_eq!(icon_of(&m, "gpu"), Icon::Tower);
    assert_eq!(icon_of(&m, "vps"), Icon::Cloud);
    assert_eq!(icon_of(&m, "gone"), Icon::Server);
}
