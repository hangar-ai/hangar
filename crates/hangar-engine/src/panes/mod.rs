//! What is kept of each pane, with no clock and no window inside: the state
//! machines the terminal pane used to keep (P4 of the port). The host feeds
//! them what the panes, the hangar-bridge mod and core report, with the
//! moment as `now`, and reads what they decide; the app's `panes_logic`
//! package is that host for the desktop.
//!
//! | file | holds |
//! | --- | --- |
//! | `agent_state` | published activity, plan limits, overlap marks |
//! | `pane_insight` | model, context, cost, events, edited files |
//! | `pane_agent` | one machine per mounted pane; the reducer of the mod's reports |
//! | `bridge_events` | queue carried over a restart, text helpers |
//! | `activity` | output bursts and silences |
//! | `hibernate` | the verdicts: may this workspace sleep? |
//! | `hibernation` | rest data, asleep, the sweep |
//! | `agents` | launch commands, resume, the trust question |
//! | `session_watch` | following a pane's session |
//! | `agent_pane` | which pane signs a board task |
//! | `context_gauge` | the gauge and token counts |
//! | `shells` | which shell a pane runs |
//! | `pane_branch` | the branch cache |
//! | `pane_index` | where each pane is in the state |
//! | `delivery_stage` | each task's delivery stage, from core's `delivery:stages` |

pub mod activity;
pub mod agent_pane;
pub mod agent_state;
pub mod agents;
pub mod bridge_events;
pub mod context_gauge;
pub mod delivery_stage;
pub mod hibernate;
pub mod hibernation;
pub mod pane_agent;
pub mod pane_branch;
pub mod pane_index;
pub mod pane_insight;
pub mod session_watch;
pub mod shells;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub use self::pane_agent::{Ambient, Effect, PaneAgent, PaneInfo};
pub use crate::live::EpochMs;

use self::agent_state::AgentStates;
use self::agents::Claims;
use self::bridge_events::CarriedQueues;
use self::delivery_stage::{DeliveryStage, DeliveryStages};
use self::hibernation::{Asleep, Hibernation};
use self::pane_branch::{BranchCache, PaneBranch};
use self::pane_index::PaneIndex;
use self::pane_insight::Insights;
use self::session_watch::SessionWatch;
use crate::live::{
    AgentActivity, AgentRoster, PaneActivity, PaneInsight, PaneMcp, PaneStatus, PlanLimits,
    WorkspaceActivity,
};
use crate::state::types::{PaneId, WsId};

/// What is kept of the panes: the app holds it as `Model.live.panes_logic`,
/// a Hangar with no window alone. Its fields are open to the host that
/// drives it (the app's panes_logic package for now); what others read goes
/// through the methods below.
///
/// Activity per pane, plan limits, insight per pane, hibernation (`touched`,
/// `rest`, `asleep`), branch cache: the shared value types are in
/// `state::live`. The terminal's package writes through `Msg`, never into it.
#[derive(Debug, Default)]
pub struct Live {
    pub agents: AgentStates,
    pub insights: Insights,
    /// One machine per mounted pane.
    pub machines: HashMap<PaneId, PaneAgent>,
    /// The shell's life, per mounted pane (`status` of the header).
    pub status: HashMap<PaneId, PaneStatus>,
    pub hibernation: Hibernation,
    pub carried: CarriedQueues,
    pub claims: Claims,
    /// The saved sessions a launch did not find on disk, this run: one miss
    /// keeps the id (a transcript under a root not looked in, a read that
    /// failed), a second forgets it.
    pub missed: HashSet<String>,
    pub branches: BranchCache,
    /// The transcript hunt of every pane whose agent can resume.
    pub watches: HashMap<PaneId, SessionWatch>,
    /// The workspaces the main window showed, the latest first.
    pub recent: Vec<WsId>,
    /// The workspace it showed when `sync_active` last looked.
    pub last_active: Option<WsId>,
    /// Where each pane is in the state (`index_panes`).
    pub index: PaneIndex,
    /// Where each task's delivery stands (`delivery_stage`).
    pub deliveries: DeliveryStages,
}

/// Milliseconds since the Unix epoch, now. The one clock of this directory,
/// read where a message arrives and handed down as `now`.
pub fn wall_clock_ms() -> EpochMs {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as EpochMs)
}

// ----- what other packages read

impl Live {
    /// A pane's published activity (`getPaneActivity`): only the panes that
    /// report one, which is an agent's, or a shell's with a Claude Code in it
    /// that loaded the mod.
    pub fn pane_activity(&self, pane: &str) -> Option<&PaneActivity> {
        self.agents.get(pane)
    }

    /// The pane's activity, `Idle` for one that reports none.
    pub fn activity_or_idle(&self, pane: &str) -> AgentActivity {
        self.agents
            .get(pane)
            .map_or(AgentActivity::Idle, |entry| entry.activity)
    }

    /// Every published activity: what the snapshot carries for the engines.
    pub fn activity_map(&self) -> &HashMap<PaneId, PaneActivity> {
        self.agents.all()
    }

    /// The pane's agent is waiting for an answer, rather than at work or done.
    pub fn is_waiting(&self, pane: &str) -> bool {
        self.agents.is_waiting(pane)
    }

    /// The pane's agent is inside a turn: at work, or waiting for an answer
    /// the turn needs. What must not be restarted from under its person, and
    /// what keeps a workspace awake (`hibernation::is_busy`).
    pub fn is_mid_turn(&self, pane: &str) -> bool {
        hibernation::is_busy(self.activity_or_idle(pane))
    }

    /// How many of a workspace's agents are working, waiting, or done.
    pub fn workspace_activity(&self, workspace_id: &str) -> WorkspaceActivity {
        self.agents.summarize(workspace_id)
    }

    /// Which panes of these workspaces are working, waiting, or done.
    pub fn agents_in(&self, workspace_ids: &[WsId]) -> AgentRoster {
        self.agents.roster(workspace_ids)
    }

    /// How many of these panes have a Claude Code that loaded the mod.
    pub fn bridged_count<'a>(&self, pane_ids: impl IntoIterator<Item = &'a PaneId>) -> usize {
        self.agents.bridged_count(pane_ids)
    }

    /// The plan's rate-limit windows, from whichever session reported last.
    pub fn plan_limits(&self) -> Option<&PlanLimits> {
        self.agents.plan_limits()
    }

    /// The name of the pane that just edited a file this one had edited.
    pub fn conflict(&self, pane: &str) -> Option<&str> {
        self.agents.conflict(pane)
    }

    /// What the pane knows of its agent: the empty insight for a pane that
    /// never reported.
    pub fn insight(&self, pane: &str) -> &PaneInsight {
        self.insights.get(pane)
    }

    /// The shell of a mounted pane: starting, running, exited.
    pub fn pane_status(&self, pane: &str) -> Option<PaneStatus> {
        self.status.get(pane).copied()
    }

    /// The pane handed control back while the person was looking elsewhere:
    /// its outline, and a bell on a shell's header.
    pub fn needs_attention(&self, pane: &str) -> bool {
        self.machines
            .get(pane)
            .is_some_and(|machine| machine.attention)
    }

    /// Whether a workspace was put to sleep and not woken since.
    pub fn is_asleep(&self, workspace_id: &str) -> bool {
        self.hibernation.is_asleep(workspace_id)
    }

    /// Every workspace asleep, with since when and how many agents stopped.
    pub fn asleep(&self) -> &HashMap<WsId, Asleep> {
        self.hibernation.asleep()
    }

    /// The ids of the workspaces asleep: what the snapshot carries.
    pub fn asleep_ids(&self) -> HashSet<WsId> {
        self.hibernation.asleep().keys().cloned().collect()
    }

    /// The branch the header of a pane on this folder shows.
    /// Core said where the deliveries stand (`CoreEvent::DeliveryStages`).
    pub fn deliveries_changed(&mut self, stages: &hangar_core::delivery::Stages, now: EpochMs) {
        self.deliveries.apply(stages, now);
    }

    /// Where `task`'s delivery stands at `now`: under way, or just ended.
    pub fn delivery_stage(&self, task: &str, now: EpochMs) -> Option<DeliveryStage> {
        self.deliveries.stage(task, now)
    }

    pub fn pane_branch(&self, folder: &str) -> Option<&PaneBranch> {
        self.branches.peek(folder)
    }

    /// Whether a pane's terminal is mounted.
    pub fn is_pane_mounted(&self, pane: &str) -> bool {
        self.machines.contains_key(pane)
    }

    // ----- the few doors the terminal's package writes through

    /// A session id handed to a pane for this app run, so that no other pane
    /// latches onto it (`claim`). The launch of a resumed session claims it.
    pub fn claim_session(&mut self, session_id: &str) {
        self.claims.claim(session_id);
    }

    /// Hands a session id back: another pane may latch onto it again.
    pub fn release_session(&mut self, session_id: Option<&str>) {
        self.claims.release(session_id);
    }

    /// A launch looked for `session_id`'s transcript: whether the pane is to
    /// forget it, which only a second miss in a row decides.
    pub fn session_looked_up(&mut self, session_id: &str, exists: Option<bool>) -> bool {
        match exists {
            Some(false) => !self.missed.insert(session_id.to_owned()),
            Some(true) => {
                self.missed.remove(session_id);
                false
            }
            None => false,
        }
    }

    /// Whether the pane's next launch must resume whatever the auto-resume
    /// setting says (a workspace was put to sleep under it); `take` uses the
    /// promise up. Called by the launch it was meant for.
    pub fn must_resume(&mut self, pane: &str, take: bool) -> bool {
        self.hibernation.must_resume(pane, take)
    }

    /// A pane restarted under a new id keeps what its old id was owed.
    pub fn carry_resume(&mut self, from: &str, to: &str) {
        self.hibernation.carry_resume(from, to);
    }

    /// Panes put to sleep that resume on their next launch (`resumeOnWake`),
    /// for a workspace that moves to a window while asleep.
    pub fn resume_on_wake<'a>(&mut self, panes: impl IntoIterator<Item = &'a str>) {
        self.hibernation.resume_on_wake(panes);
    }

    /// Awake again: a workspace sent to a window of its own or opened by an
    /// agent. Whether it was asleep.
    pub fn mark_awake(&mut self, workspace_id: &str) -> bool {
        self.hibernation.mark_awake(workspace_id)
    }

    /// The MCP servers the pane's Claude Code was launched with
    /// (`updatePaneInsight(pane, { mcp })` of `mcpLaunchConfig`). Written by
    /// the launch, once the servers' file is known and before the agent
    /// starts, and read back by the inspector's MCP row and by the servers
    /// panel, which restarts the panes that run with something else.
    ///
    /// `None` forgets it, which is what a launch that could not say leaves:
    /// the panel then counts the pane as out of date, as the TS did for a
    /// launch it never saw. It goes with the pane when its terminal goes
    /// (`unmount_pane`), so a restart under a new id starts without. Whether
    /// anything changed.
    pub fn set_launch_mcp(&mut self, pane: &str, mcp: Option<PaneMcp>) -> bool {
        self.insights.update(pane, |insight| insight.mcp = mcp)
    }
}

/// What another package's test needs to put a pane in a state: the reducer
/// that normally does it is driven by core's events and the terminals.
#[cfg(any(test, feature = "testing"))]
impl Live {
    /// The pane reports this activity, as an agent with the mod would.
    pub fn seed_activity(&mut self, pane: &str, workspace: &str, activity: AgentActivity) {
        self.agents.set(
            pane,
            PaneActivity {
                workspace_id: workspace.to_string(),
                activity,
                bridged: true,
            },
        );
    }

    /// The pane has reported this much about its agent.
    pub fn seed_insight(&mut self, pane: &str, edit: impl FnOnce(&mut PaneInsight)) {
        self.insights.update(pane, edit);
    }
}

impl Live {
    /// What `attend` can change, to tell whether it did.
    pub fn pane_state_mark(&self, pane: &str) -> Option<(AgentActivity, bool)> {
        self.machines
            .get(pane)
            .map(|machine| (machine.activity, machine.attention))
    }
}
