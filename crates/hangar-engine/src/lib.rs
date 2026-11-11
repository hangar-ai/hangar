//! Hangar without its window (docs/native/headless.md): what runs the
//! workspaces whatever draws them. The desktop app builds its window around
//! it; a Hangar with no window runs it alone.
//!
//! - `state`: the persisted state of the workspaces and the store through
//!   which it changes;
//! - `live`: what is kept of each pane and agent beside it (activity,
//!   insight, the hangar-bridge mod's reports);
//! - `panes`: the state machines that decide it from what the panes and the
//!   mod report;
//! - `accounts`: which Claude account a pane runs on;
//! - `nodes`: which node a pane runs on, and how a node's folders are laid
//!   out;
//! - `control`: the agents' operations; `orchestration`: the workers, the
//!   events to the orchestrator, the briefings; `preview`: what reaching the
//!   workspaces' previews takes;
//! - `mobile`: the desk, what a surface away from the window asks of the
//!   panes; `stats`: the statistics;
//! - `host`: the engine hosted with no window (a server).

pub mod accounts;
pub mod control;
pub mod host;
pub mod live;
pub mod mobile;
pub mod nodes;
pub mod orchestration;
pub mod panes;
pub mod preview;
pub mod stats;

pub mod state;
#[cfg(test)]
mod testdir;
