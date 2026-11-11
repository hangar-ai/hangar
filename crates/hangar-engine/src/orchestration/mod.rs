//! The orchestration of the workspaces, whatever runs it (P19 of the port):
//!
//! - `worker`: the `worker_*` ops: a worker started on a task in a git
//!   worktree of its own (or the workspace's folder without git), listed,
//!   stopped;
//! - `events`: what an orchestrator is told between its turns, batched;
//! - `manager_feed`: what the project manager is told waits for the person;
//! - `brief` and `brief_text`: the roles and the state a session is briefed
//!   with, at start and after every compaction;
//! - `projects`: every project summarized, for the manager;
//! - `heard`: the lines written to an orchestrator, read back for the person;
//! - `checks`: the last delivery check of each workspace;
//! - `port`: the shapes shared with the agents' operations.
//!
//! The host (the app's `orchestration` package on the desktop) runs the
//! engines on its clock and gives them their ports.

pub mod brief;
pub mod brief_text;
pub mod checks;
pub mod events;
pub mod heard;
pub mod manager_feed;
pub mod port;
pub mod projects;
pub mod sweep;
pub mod text;
pub mod worker;

#[cfg(any(test, feature = "testing"))]
pub mod testkit;
