//! How a machine is shown, decided once for every surface that names one:
//! the navigator's rows, the palette, the pane's bar and card, the settings.
//!
//! A machine goes by one icon, its kind's, and its label. Its link has a
//! state, and the state has one look: calm while it is up or only just
//! down, the unplugged mark in the warning colour once it has been down
//! longer than a blip. Nothing here is red: a machine that is away is
//! waited for, not an error to fix.

use std::time::{Duration, Instant};

use hangar_i18n::{Key, Translator, args};
use hangar_theme::Theme;
use hangar_ui::Icon;
use iced::Color;

use crate::state::Model;
use crate::state::types::{MachineKind, NodeConfig};

use super::model::{Failure, NodeStatus};

/// How long a link may be down before anything says so: a laptop waking, a
/// network changing and a server restarting are back within it.
pub const GRACE: Duration = Duration::from_secs(5);

/// How long a link is down before switching the machine off is offered.
pub const SWITCH_OFF_AFTER: Duration = Duration::from_secs(20);

/// What a link went through since it was last up, which its status alone
/// does not say: a status is replaced at each step of each attempt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Link {
    /// When it was first seen down; `None` while it is up.
    pub down_since: Option<Instant>,
    /// Why the last attempt failed, once one has since it was last up. The
    /// drop of a tunnel that was up is not a failed attempt.
    pub failure: Option<Failure>,
    /// It has been up since the app started.
    pub was_connected: bool,
    /// When the next attempt starts, while one is waited for.
    pub retry_at: Option<Instant>,
}

impl Link {
    /// The link's story continued with the status its connection reports.
    pub fn note(&mut self, before: &NodeStatus, status: &NodeStatus, now: Instant) {
        match status {
            NodeStatus::Connected { .. } => {
                *self = Link {
                    was_connected: true,
                    ..Link::default()
                };
            }
            NodeStatus::Connecting(_) => {
                self.down_since.get_or_insert(now);
                self.retry_at = None;
            }
            NodeStatus::Unreachable { failure, retry_in } => {
                self.down_since.get_or_insert(now);
                self.retry_at = Some(now + Duration::from_secs(*retry_in));
                if !matches!(before, NodeStatus::Connected { .. }) {
                    self.failure = Some(failure.clone());
                }
            }
            NodeStatus::Disconnected => *self = Link::default(),
        }
    }

    /// How long it has been down.
    pub fn down_for(&self, now: Instant) -> Option<Duration> {
        self.down_since
            .map(|since| now.saturating_duration_since(since))
    }

    /// The seconds left before the next attempt, rounded up.
    pub fn retry_in(&self, now: Instant) -> Option<u64> {
        let left = self.retry_at?.saturating_duration_since(now);
        Some(left.as_secs() + u64::from(left.subsec_nanos() > 0))
    }
}

/// Where a machine stands, as the surfaces show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    Connected,
    /// Not connected, for less than [`GRACE`]: drawn as connected.
    Settling,
    /// Down past the grace period, no attempt failed yet. `again`: it was
    /// connected before, so this is a reconnection.
    Connecting {
        again: bool,
    },
    /// An attempt failed since it was last up, and why. Without a reason:
    /// nothing is trying (the experiment is off, the machine was removed).
    Offline(Option<Failure>),
    /// Switched off by the person.
    Off,
}

impl Presence {
    /// The link was up, or was tried, and is not up, past the grace
    /// period. A first connection still under way is not down: nothing
    /// failed, the machine is being set up (after an update of the app
    /// its binary is uploaded, which takes more than a moment).
    pub fn is_down(&self) -> bool {
        matches!(
            self,
            Presence::Connecting { again: true } | Presence::Offline(_)
        )
    }

    /// Nothing can be asked of the machine now.
    pub fn is_away(&self) -> bool {
        !matches!(self, Presence::Connected)
    }
}

/// Where node `node` stands at `now`.
pub fn of(m: &Model, node: &str, now: Instant) -> Presence {
    read(m, node, Some(now))
}

/// The same without the grace period, for the settings, which say what is
/// going on as it happens.
pub fn exact(m: &Model, node: &str) -> Presence {
    read(m, node, None)
}

fn read(m: &Model, node: &str, grace_at: Option<Instant>) -> Presence {
    let Some(config) = config(m, node).filter(|_| super::available(m)) else {
        return Presence::Offline(None);
    };
    if !config.is_on() {
        return Presence::Off;
    }
    let live = &m.live.nodes;
    if matches!(live.statuses.get(node), Some(NodeStatus::Connected { .. })) {
        return Presence::Connected;
    }
    let link = live.links.get(node);
    let settling = match (grace_at, link.and_then(|link| link.down_since)) {
        (Some(now), Some(since)) => now.saturating_duration_since(since) < GRACE,
        // Not tried yet: the app has just started, or it was just switched on.
        (Some(_), None) => true,
        (None, _) => false,
    };
    if settling {
        return Presence::Settling;
    }
    match link.and_then(|link| link.failure.clone()) {
        Some(failure) => Presence::Offline(Some(failure)),
        None => Presence::Connecting {
            again: link.is_some_and(|link| link.was_connected),
        },
    }
}

/// The settings' entry for node `node`.
pub fn config<'a>(m: &'a Model, node: &str) -> Option<&'a NodeConfig> {
    m.settings().nodes.iter().find(|entry| entry.id == node)
}

/// The icon a kind of machine goes by.
pub fn kind_icon(kind: MachineKind) -> Icon {
    match kind {
        MachineKind::Server => Icon::Server,
        MachineKind::Cloud => Icon::Cloud,
        MachineKind::Laptop => Icon::Laptop,
        MachineKind::Desktop => Icon::Desktop,
        MachineKind::MiniPc => Icon::MiniPc,
        MachineKind::Tower => Icon::Tower,
    }
}

/// A kind's name.
pub fn kind_key(kind: MachineKind) -> Key {
    match kind {
        MachineKind::Server => Key::NodesKindServer,
        MachineKind::Cloud => Key::NodesKindCloud,
        MachineKind::Laptop => Key::NodesKindLaptop,
        MachineKind::Desktop => Key::NodesKindDesktop,
        MachineKind::MiniPc => Key::NodesKindMiniPc,
        MachineKind::Tower => Key::NodesKindTower,
    }
}

/// The icon this computer goes by, where it is listed with the others.
pub const LOCAL_ICON: Icon = Icon::Desktop;

/// The icon node `node` goes by: its kind's, a server's for one the
/// settings no longer hold.
pub fn icon_of(m: &Model, node: &str) -> Icon {
    kind_icon(
        config(m, node)
            .map(|config| config.kind)
            .unwrap_or_default(),
    )
}

/// The colour a machine's mark is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Warning,
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color {
        match self {
            Tone::Muted => t.colors.fg_muted,
            Tone::Warning => t.colors.warning,
        }
    }
}

/// How a machine's mark is drawn for a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub icon: Icon,
    pub tone: Tone,
    /// The name beside it steps back: what it names cannot be reached.
    pub dimmed: bool,
}

/// The one mapping from a state to a look.
pub fn look(kind: MachineKind, presence: &Presence) -> Look {
    match presence {
        // A first connection under way is drawn as calmly as a link that
        // is up: nothing is wrong, and the words say it is connecting.
        Presence::Connected | Presence::Settling | Presence::Connecting { again: false } => Look {
            icon: kind_icon(kind),
            tone: Tone::Muted,
            dimmed: false,
        },
        Presence::Connecting { again: true } | Presence::Offline(_) => Look {
            icon: Icon::Unplugged,
            tone: Tone::Warning,
            dimmed: true,
        },
        Presence::Off => Look {
            icon: kind_icon(kind),
            tone: Tone::Muted,
            dimmed: true,
        },
    }
}

/// The look of node `node` at `now`.
pub fn look_of(m: &Model, node: &str, now: Instant) -> Look {
    let kind = config(m, node)
        .map(|config| config.kind)
        .unwrap_or_default();
    look(kind, &of(m, node, now))
}

/// The state in a word or two, without why: what a control that lists the
/// machines has room for.
pub fn short_words(tr: Translator, presence: &Presence) -> String {
    match presence {
        Presence::Offline(_) => tr.t(Key::NodesStateOffline).into_owned(),
        other => words(tr, other),
    }
}

/// The state in words, as a tooltip ends: "connected", "reconnecting…",
/// "offline: unreachable over SSH (Connection timed out)".
pub fn words(tr: Translator, presence: &Presence) -> String {
    match presence {
        Presence::Connected => tr.t(Key::NodesStateConnected).into_owned(),
        Presence::Settling | Presence::Connecting { again: false } => {
            tr.t(Key::NodesStateConnecting).into_owned()
        }
        Presence::Connecting { again: true } => tr.t(Key::NodesStateReconnecting).into_owned(),
        Presence::Offline(Some(failure)) => tr
            .t_with(
                Key::NodesStateOfflineWhy,
                &args!("reason" => super::failure::words(tr, failure)),
            )
            .into_owned(),
        Presence::Offline(None) => tr.t(Key::NodesStateOffline).into_owned(),
        Presence::Off => tr.t(Key::NodesStateOff).into_owned(),
    }
}

/// A machine's tooltip: its label and its state, "vps · reconnecting…".
pub fn tip(tr: Translator, label: &str, presence: &Presence) -> String {
    tr.t_with(
        Key::NodesStateTip,
        &args!("node" => label, "state" => words(tr, presence)),
    )
    .into_owned()
}

/// The tooltip of node `node` at `now`.
pub fn tip_of(m: &Model, tr: Translator, node: &str, now: Instant) -> String {
    tip(tr, &super::label(m, node), &of(m, node, now))
}

/// How many machines the settings hold, this computer counted, and how many
/// of the others are down past the grace period or switched off: the
/// navigator's foot. One still on its first connection is neither.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Census {
    pub machines: usize,
    pub offline: usize,
    pub off: usize,
}

pub fn census(m: &Model, now: Instant) -> Census {
    let mut census = Census {
        machines: 1,
        ..Census::default()
    };
    for node in &m.settings().nodes {
        census.machines += 1;
        match of(m, &node.id, now) {
            Presence::Off => census.off += 1,
            presence if presence.is_down() => census.offline += 1,
            _ => {}
        }
    }
    census
}

#[cfg(test)]
mod tests;
