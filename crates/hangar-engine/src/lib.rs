//! Hangar without its window (docs/native/headless.md): what runs the
//! workspaces whatever draws them. The desktop app builds its window around
//! it; a Hangar with no window runs it alone.
//!
//! - `state`: the persisted state of the workspaces and the store through
//!   which it changes;
//! - `live`: what is kept of each pane and agent beside it (activity,
//!   insight, the hangar-bridge mod's reports);
//! - `panes`: the state machines that decide it from what the panes and the
//!   mod report.

pub mod live;
pub mod panes;
pub mod state;
