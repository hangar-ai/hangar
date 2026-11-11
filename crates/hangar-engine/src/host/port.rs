//! The control port over a host with no window: what the agents' operations
//! reach (the app's `control/live.rs`, without the window and the nodes).
//!
//! The model writes are applied at once (`EngineHost::apply`); the panes'
//! shells and screens are the host's [`Terminals`](super::Terminals); core's
//! commands run directly, the blocking ones on tokio's blocking pool. There
//! is no preview to show a file or a model in, and no window to detach a
//! workspace to: the operations that need one say so.

use std::sync::{Arc, OnceLock, PoisonError};
use std::time::Duration;

use serde_json::Value;

use hangar_core::Event;
use hangar_core::accounts::Account;
use hangar_core::board::{Board, Task, TaskPatch};
use hangar_core::delivery::{Cleanup, Delivery, Outcome, Plan, Running};
use hangar_core::git::Worktree;
use hangar_core::kits::KitInfo;
use hangar_core::shells::ShellInfo;

use super::EngineHost;
use crate::control::delivery::Deliveries;
use crate::control::delivery_text::HANGAR;
use crate::control::port::{
    AppPort, BoxFuture, CmdOut, DeliveryDeps, MobileHooks, ModelCmd, ModelFileInfo,
    OrchestrationHooks, PaneTerminal, PreviewHost,
};
use crate::state::{PaneId, Snapshot, WsId};

/// The port, over `host`.
#[derive(Clone)]
pub struct HostPort {
    host: EngineHost,
}

impl HostPort {
    pub fn new(host: EngineHost) -> HostPort {
        HostPort { host }
    }
}

/// `f` on tokio's blocking pool: the core commands that touch the disk or a
/// lock the panes' readers hold.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| format!("the task stopped: {err}"))
}

impl AppPort for HostPort {
    fn snapshot(&self) -> Arc<Snapshot> {
        self.host.snapshot()
    }

    fn apply(&self, cmd: ModelCmd) -> BoxFuture<Result<CmdOut, String>> {
        Box::pin(std::future::ready(self.host.apply(cmd)))
    }

    fn terminal(&self, pane: &PaneId) -> Option<Arc<dyn PaneTerminal>> {
        self.host.terminals().screen(pane).map(|_| {
            Arc::new(HostTerminal {
                host: self.host.clone(),
                pane: pane.clone(),
            }) as Arc<dyn PaneTerminal>
        })
    }

    fn write(&self, pane: &PaneId, data: &str) -> BoxFuture<Result<(), String>> {
        let terminals = self.host.terminals().clone();
        let (pane, data) = (pane.clone(), data.to_string());
        Box::pin(async move { blocking(move || terminals.write(&pane, &data)).await? })
    }

    fn alive(&self, pane: &PaneId) -> BoxFuture<bool> {
        let terminals = self.host.terminals().clone();
        let pane = pane.clone();
        Box::pin(async move {
            blocking(move || terminals.alive(&pane))
                .await
                .unwrap_or(false)
        })
    }

    fn kill(&self, pane: &PaneId) -> BoxFuture<Result<(), String>> {
        let terminals = self.host.terminals().clone();
        let pane = pane.clone();
        Box::pin(async move { blocking(move || terminals.kill(&pane)).await? })
    }

    fn enqueue(
        &self,
        pane: &PaneId,
        text: &str,
        from: Option<&str>,
    ) -> BoxFuture<Result<(), String>> {
        let core = self.host.core().clone();
        let (pane, text, from) = (pane.clone(), text.to_string(), from.map(str::to_string));
        Box::pin(async move {
            blocking(move || {
                hangar_core::bridge::bridge_enqueue(&core, pane, text, from);
            })
            .await
        })
    }

    fn unqueue(&self, pane: &PaneId, id: &str) -> BoxFuture<Result<(), String>> {
        let core = self.host.core().clone();
        let (pane, id) = (pane.clone(), id.to_string());
        Box::pin(async move {
            blocking(move || hangar_core::bridge::bridge_cancel(&core, pane, Some(id))).await
        })
    }

    fn dir_exists(&self, path: &str) -> BoxFuture<bool> {
        let path = path.to_string();
        Box::pin(async move {
            blocking(move || hangar_core::store::dir_exists(path))
                .await
                .unwrap_or(false)
        })
    }

    fn can_detach(&self) -> bool {
        false
    }

    fn sleep(&self, ms: u64) -> BoxFuture<()> {
        Box::pin(tokio::time::sleep(Duration::from_millis(ms)))
    }

    /// No palette: no command to run.
    fn knows_command(&self, id: &str) -> bool {
        let _ = id;
        false
    }

    /// Nothing is served to a viewer here.
    fn model_allow_roots(&self, ws: &WsId, roots: Vec<String>) -> BoxFuture<Result<(), String>> {
        let _ = (ws, roots);
        Box::pin(std::future::ready(Ok(())))
    }

    fn model_stat(&self, path: &str) -> BoxFuture<Result<ModelFileInfo, String>> {
        let stat = hangar_core::model::model_stat(path.to_string());
        Box::pin(async move {
            let file = stat.await?;
            Ok(ModelFileInfo {
                path: file.path,
                size: file.size,
                modified_ms: file.modified_ms,
            })
        })
    }

    fn worktrees(&self, path: &str) -> BoxFuture<Result<Vec<Worktree>, String>> {
        Box::pin(hangar_core::git::git_worktree_list(path.to_string()))
    }

    /// No window draws a preview.
    fn preview(&self, ws: &WsId, detached: bool) -> BoxFuture<Option<Arc<dyn PreviewHost>>> {
        let _ = (ws, detached);
        Box::pin(std::future::ready(None))
    }

    fn preview_inspect(&self, path: &str) -> BoxFuture<Result<Value, String>> {
        let inspect = hangar_core::preview::preview_inspect(path.to_string());
        Box::pin(async move {
            let file = inspect.await?;
            serde_json::to_value(&file).map_err(|err| err.to_string())
        })
    }

    fn kits(&self) -> BoxFuture<Result<Vec<KitInfo>, String>> {
        Box::pin(hangar_core::kits::kits_list())
    }

    fn delivery(&self) -> Option<Arc<dyn DeliveryDeps>> {
        Some(Arc::new(HostDelivery {
            core: self.host.core().clone(),
        }))
    }

    fn orchestration(&self) -> Option<Arc<dyn OrchestrationHooks>> {
        self.host
            .orchestration()
            .map(|runtime| Arc::new(runtime.clone()) as Arc<dyn OrchestrationHooks>)
    }

    fn mobile(&self) -> Option<Arc<dyn MobileHooks>> {
        self.host
            .0
            .mobile
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn deliveries(&self) -> &Deliveries {
        self.host.deliveries()
    }
}

/// A pane's screen, as an agent reads and pastes into it.
struct HostTerminal {
    host: EngineHost,
    pane: PaneId,
}

impl PaneTerminal for HostTerminal {
    fn paste(&self, text: &str) -> BoxFuture<Result<(), String>> {
        let terminals = self.host.terminals().clone();
        let pane = self.pane.clone();
        let text = if terminals.bracketed_paste(&pane) {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text.to_string()
        };
        Box::pin(async move { blocking(move || terminals.write(&pane, &text)).await? })
    }

    fn read(&self, n: usize) -> Vec<String> {
        self.host
            .terminals()
            .lines(&self.pane, n)
            .unwrap_or_default()
    }

    fn application_cursor(&self) -> bool {
        self.host.terminals().application_cursor(&self.pane)
    }
}

/// core's delivery, board and accounts, and the bridge queue, on this
/// machine.
struct HostDelivery {
    core: hangar_core::Host,
}

/// The shells detected on this machine, once.
fn shells() -> &'static Vec<ShellInfo> {
    static SHELLS: OnceLock<Vec<ShellInfo>> = OnceLock::new();
    SHELLS.get_or_init(hangar_core::shells::detect_shells)
}

/// The board views and the orchestrator's events reload on this, which the
/// writes through the agents' API send by themselves and these do not.
fn changed(core: &hangar_core::Host, cwd: String) {
    let _ = core.emit(Event::BoardChanged { cwd });
}

impl DeliveryDeps for HostDelivery {
    fn preflight(
        &self,
        workspace: String,
        worktree: String,
        target: Option<String>,
        landing: bool,
        task: Option<String>,
    ) -> BoxFuture<Result<Plan, String>> {
        Box::pin(hangar_core::delivery::delivery_preflight(
            workspace,
            worktree,
            target,
            Some(landing),
            task,
        ))
    }

    fn run(&self, delivery: Delivery) -> BoxFuture<Result<Outcome, String>> {
        Box::pin(async move { Ok(hangar_core::delivery::delivery_run(delivery).await) })
    }

    fn cleanup(
        &self,
        worktree: String,
        branch: String,
        target: String,
        task_id: String,
    ) -> BoxFuture<Result<Cleanup, String>> {
        Box::pin(async move {
            Ok(hangar_core::delivery::delivery_cleanup(worktree, branch, target, task_id).await)
        })
    }

    fn running(&self) -> BoxFuture<Result<Vec<Running>, String>> {
        Box::pin(blocking(hangar_core::delivery::delivery_running))
    }

    fn accounts(&self) -> BoxFuture<Result<Vec<Account>, String>> {
        Box::pin(blocking(hangar_core::accounts::accounts_list))
    }

    fn shells(&self) -> Vec<ShellInfo> {
        shells().clone()
    }

    fn board_load(&self, cwd: &str) -> BoxFuture<Result<Board, String>> {
        let cwd = cwd.to_string();
        Box::pin(blocking(move || hangar_core::board_load(cwd)))
    }

    fn board_update(
        &self,
        cwd: &str,
        id: &str,
        patch: TaskPatch,
    ) -> BoxFuture<Result<Task, String>> {
        let (core, cwd, id) = (self.core.clone(), cwd.to_string(), id.to_string());
        Box::pin(async move {
            let task = blocking({
                let cwd = cwd.clone();
                move || hangar_core::board_update(cwd, id, patch)
            })
            .await??;
            changed(&core, cwd);
            Ok(task)
        })
    }

    fn board_comment(
        &self,
        cwd: &str,
        id: &str,
        author: &str,
        text: &str,
    ) -> BoxFuture<Result<Task, String>> {
        let (core, cwd, id, author, text) = (
            self.core.clone(),
            cwd.to_string(),
            id.to_string(),
            author.to_string(),
            text.to_string(),
        );
        Box::pin(async move {
            let task = blocking({
                let cwd = cwd.clone();
                move || hangar_core::board_comment(cwd, id, author, text)
            })
            .await??;
            changed(&core, cwd);
            Ok(task)
        })
    }

    fn tell(&self, pane: &PaneId, text: &str) -> BoxFuture<Result<(), String>> {
        let (core, pane, text) = (self.core.clone(), pane.clone(), text.to_string());
        Box::pin(async move {
            blocking(move || {
                hangar_core::bridge::bridge_enqueue(&core, pane, text, Some(HANGAR.into()));
            })
            .await
        })
    }

    /// No worker of this host's workspaces runs on another machine.
    fn bring_back(
        &self,
        workspace: String,
        node: String,
        worktree: String,
        landing: bool,
    ) -> BoxFuture<Result<String, String>> {
        let _ = (workspace, worktree, landing);
        Box::pin(std::future::ready(Err(format!(
            "This Hangar does not drive node {node}: the branch cannot be brought back from it."
        ))))
    }

    fn clean_node(
        &self,
        node: String,
        worktree: String,
        branch: String,
    ) -> BoxFuture<Result<Cleanup, String>> {
        let _ = (worktree, branch);
        Box::pin(std::future::ready(Err(format!(
            "This Hangar does not drive node {node}."
        ))))
    }
}
