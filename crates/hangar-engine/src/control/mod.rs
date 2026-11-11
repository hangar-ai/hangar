//! The agents' operations (MCP), whatever runs them (P18 of the port).
//!
//! Agents drive Hangar over MCP: core's server relays each call as a control
//! request, the host hands it to `run`, and the operation runs on tokio
//! against the port (`port.rs`), never against a model. Every message an
//! operation refuses with is English read by a model, and says what to do
//! next; the tests hold them word for word.
//!
//! - `port.rs`: what an operation reaches (`AppPort`, `ModelCmd`, the hooks
//!   the host installs);
//! - `types.rs`: the values the engines share;
//! - `run.rs`: the dispatch, the gate, which workspace and pane a request
//!   means; `paths.rs`: paths as agents give them;
//! - `panes.rs`, `preview.rs`, `kit_tabs.rs`, `workspace.rs`, `manager.rs`,
//!   `delivery.rs` (+ `delivery_text.rs`), `board_relay.rs`: the operations;
//! - `requests.rs`: what they ask of the host beyond the model's writes.
//!
//! The host (the app's `control` package on the desktop) builds the port
//! over what it runs, applies the `ModelCmd`s to its model, and installs the
//! hooks.

pub mod board_relay;
pub mod delivery;
pub mod delivery_text;
pub mod kit_tabs;
pub mod manager;
pub mod panes;
pub mod paths;
pub mod port;
pub mod preview;
pub mod requests;
pub mod run;
pub mod types;
pub mod workspace;

#[cfg(any(test, feature = "testing"))]
pub mod fake;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
#[cfg(test)]
mod tests;

pub use port::{AppPort, CmdOut, Hook, ModelCmd, Port};
pub use run::{ASLEEP, DISABLED};

use serde_json::Value;

/// An operation run as the person, from the phone (`runRemote`): the same
/// operations, never refused for the setting. P20's `pane_read` and
/// `pane_send` go through here.
pub async fn run_as_person(op: &str, args: Value, port: &Port) -> Result<Value, String> {
    run::run(op, args, port, run::Gate::Person).await
}
