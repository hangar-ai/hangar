//! The orchestration runtime, hosted by the app: it lives in hangar-engine
//! (`hangar_engine::orchestration::runtime`), generic over what hosts it;
//! the app's [`AppRuntime`] makes its ports over the window's services
//! (`LivePort`, with the workspaces on nodes reached through them).

pub use hangar_engine::orchestration::runtime::{EngineMsg, RuntimeHost};

use hangar_engine::orchestration::runtime;

use super::live::LivePort;
use crate::services::Services;
use crate::state::types::Workspace;

/// The app as the runtime's host.
pub struct AppRuntime {
    services: Services,
}

impl AppRuntime {
    pub fn services(&self) -> &Services {
        &self.services
    }
}

impl RuntimeHost for AppRuntime {
    type Port = LivePort;

    fn port(&self, handle: &OrchestrationHandle) -> LivePort {
        LivePort::new(handle.clone())
    }

    /// On its node, when the workspace lives on one.
    fn port_for(&self, handle: &OrchestrationHandle, ws: &Workspace) -> LivePort {
        LivePort::on(handle.clone(), ws.node.clone())
    }

    /// A pane of a remote workspace is briefed from its node's board and
    /// memory.
    fn port_for_pane(&self, handle: &OrchestrationHandle, pane: &str) -> LivePort {
        let state = self.services.snapshot().state.clone();
        LivePort::on(
            handle.clone(),
            crate::nodes::engine::node_of_pane(&state, pane),
        )
    }

    /// The remote boards are read from their nodes here, on the blocking
    /// pool, for the engines' synchronous reads.
    fn before_reads(&self, port: &LivePort, cwds: &[String]) {
        port.refresh_remote_boards(cwds);
    }
}

/// The way into the runtime, cloned wherever it is needed.
pub type OrchestrationHandle = runtime::OrchestrationHandle<AppRuntime>;

/// Starts the runtime on the current tokio runtime. Called once, on
/// hydration, from the package's `update`.
pub fn start(services: Services) -> OrchestrationHandle {
    runtime::start(AppRuntime { services })
}
