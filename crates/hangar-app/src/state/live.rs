//! What the window knows and never saves: `Model.ui` (App.tsx's `useState`,
//! inventory §1.5), `Model.live` (one read-model per package, plus the
//! shell's own), and the value types several packages share.
//!
//! The shared value types (`AgentActivity`, `PaneActivity`, `PlanLimits`,
//! `PaneInsight`, `BridgeEvent`) are declared here from the TS so that every
//! package names the same ones; P4 (`panes_logic`) filled them in and owns
//! their meaning. P4 edited this file for those types only.

use std::collections::HashMap;

use iced::window;

use super::types::{PaneId, ShellInfo, WsId};
use crate::shell::{Drawer, Focus, Notice, Overlay, View, WindowKind};

/// `Model.ui`: the window's transient state.
#[derive(Debug, Default)]
pub struct Ui {
    pub view: View,
    /// A stack: the top one has the keyboard, Escape closes it.
    pub overlays: Vec<Overlay>,
    pub drawer: Option<Drawer>,
    pub focus_by_workspace: HashMap<WsId, PaneId>,
    pub zoom_by_workspace: HashMap<WsId, bool>,
    /// Workspaces visited once, whose panes stay alive while hidden.
    pub opened_ids: Vec<WsId>,
    /// `pane.resetAll` and `workspace.stopWorkers` arm a confirmation first.
    pub confirm_reset: bool,
    pub confirm_stop_workers: bool,
    pub needs_you_open: bool,
    /// The one-line notice at the bottom, with the id its expiry names.
    pub notice: Option<(u64, Notice)>,
    /// The chords of a shortcut sequence typed so far, as P7 formats them for
    /// the chord hint (`keymap::chord_hint`).
    pub pending_chords: Vec<String>,
    /// Whether Claude Code is on PATH (`claude_installed`), once at start.
    pub claude_installed: bool,
    /// From `detect_shells`, once at start.
    pub shells: Vec<ShellInfo>,
    /// The measured width of each window's main body, for `side_panels`.
    pub body_width: HashMap<window::Id, f32>,
    pub focus: Option<Focus>,
    /// The window that last had the system's focus: where a command runs.
    pub focused_window: Option<window::Id>,
    /// The window of the app that has the system's focus, if one does
    /// (`panes_logic`'s attention: a pane in it is being watched).
    pub system_focus: Option<window::Id>,

    /// What had the keyboard when the open drawer opened, to give it back
    /// when it closes on Escape (shell/drawer.rs).
    pub drawer_opener: Option<Focus>,
    /// What had the keyboard when each overlay opened, parallel to
    /// `overlays`: closing one gives the keyboard back.
    pub overlay_openers: Vec<Option<Focus>>,
    /// The view, overlays, drawer, focus and broadcast text of every window
    /// but the focused one. Each window has its own, as each Tauri window had
    /// its own App: the fields above are the focused window's, and trade
    /// places with these when the focus moves (`Model::view_in`).
    pub parked: HashMap<window::Id, WindowUi>,
    /// Where the pointer is, for the drawers' "a press outside closes it".
    pub pointer: Pointer,
}

/// A window's own transient state, while another window has the focus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindowUi {
    pub view: View,
    pub overlays: Vec<Overlay>,
    pub overlay_openers: Vec<Option<Focus>>,
    pub drawer: Option<Drawer>,
    pub drawer_opener: Option<Focus>,
    pub focus: Option<Focus>,
}

/// Over which of the frame's parts the pointer is, as their mouse areas
/// report it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pointer {
    /// Over the open drawer.
    pub over_drawer: bool,
    /// Over the toolbar, whose buttons toggle the drawers themselves
    /// (`data-drawer-toggle`): a press there is theirs, not an outside one.
    pub over_toolbar: bool,
}

/// What packages tell the shell, that the shell needs for the frame, the
/// command table or the snapshot and must not read from a package's own
/// `Live` (it would depend on every package's types). Each field names the
/// package that writes it; the shell only reads.
#[derive(Debug, Default)]
pub struct Signals {
    /// P6: how many items Needs-you lists; `view.needsYou` is live while
    /// there are some.
    pub needs_you: usize,
    /// P9, P12: a side panel's grip is held, and this is the width the
    /// panel is drawn at meanwhile, in points (the setting's unit). Its
    /// package writes the setting once, when the grip is let go, and clears
    /// this: the frame (`Panels::of`) follows the pointer without a store
    /// action, a save and a snapshot for every move of it.
    pub panel_drag: Option<PanelDrag>,
}

/// A side panel's width while its grip is held (`Signals::panel_drag`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelDrag {
    pub panel: Drawer,
    pub width: f32,
}

/// `Model.live`: every package's shared read-model, and the shell's own.
/// Every package has its slot here by convention (PACKAGES.md), read or not
/// yet: an empty one is no mistake.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct Live {
    /// `load_state` has answered and the state is in. Nothing is saved, and
    /// no engine or control request runs, before.
    pub hydrated: bool,
    pub windows: HashMap<window::Id, WindowKind>,
    /// The title bars Hangar draws on Linux and Windows (shell/title_bar.rs).
    pub title_bar: crate::shell::title_bar::Live,
    /// Bumped whenever a read-model changes, so views can cache on it.
    pub version: u64,
    /// What packages tell the shell (see `Signals`).
    pub signals: Signals,

    pub panes_logic: crate::panes_logic::Live,
    pub panes: crate::panes::Live,
    pub navigator: crate::navigator::Live,
    pub toolbar: crate::toolbar::Live,
    pub status_bar: crate::status_bar::Live,
    pub needs_you: crate::needs_you::Live,
    pub update_banner: crate::update_banner::Live,
    pub voice: crate::voice::Live,
    pub keymap: crate::keymap::Live,
    pub palette: crate::palette::Live,
    pub saved_commands: crate::saved_commands::Live,
    pub session_picker: crate::session_picker::Live,
    pub pane_menu: crate::pane_menu::Live,
    pub workspace_dialog: crate::workspace_dialog::Live,
    pub git_flow: crate::git_flow::Live,
    pub consent_card: crate::consent_card::Live,
    pub accounts: crate::accounts::Live,
    pub inspector: crate::inspector::Live,
    pub git_ui: crate::git_ui::Live,
    pub memory: crate::memory::Live,
    pub mcp: crate::mcp::Live,
    pub orchestration_ui: crate::orchestration_ui::Live,
    pub stats: crate::stats::Live,
    pub board: crate::board::Live,
    pub settings: crate::settings::Live,
    pub kits: crate::kits::Live,
    pub preview: crate::preview::Live,
    pub kit_tabs: crate::kit_tabs::Live,
    pub model3d_view: crate::model3d_view::Live,
    pub control: crate::control::Live,
    pub orchestration: crate::orchestration::Live,
    pub mobile: crate::mobile::Live,
    pub nodes: crate::nodes::Live,
}

// What packages name at `crate::state::live::*`, now hangar-engine's.
#[allow(unused_imports)]
pub use hangar_engine::live::{
    AgentActivity, AgentRoster, BridgeEvent, ContextGauge, Delivering, EpochMs, ExitWhat,
    GlyphState, PaneActivity, PaneEvent, PaneEventDetail, PaneInsight, PaneMcp, PaneMcpMode,
    PaneStatus, PlanLimits, QueuedPrompt, RateLimit, ToolPhase, TurnOutcome, TurnPhase,
    UsageContext, WorkspaceActivity,
};

/// The state colour an activity is drawn in.
pub fn agent_state(activity: AgentActivity) -> hangar_theme::AgentState {
    match activity {
        AgentActivity::Working => hangar_theme::AgentState::Working,
        AgentActivity::Waiting => hangar_theme::AgentState::Waiting,
        AgentActivity::Yours => hangar_theme::AgentState::Yours,
        AgentActivity::Idle => hangar_theme::AgentState::Idle,
    }
}
