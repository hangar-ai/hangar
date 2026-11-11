//! The orchestration runtime, hosted with no window: its ports read the
//! host's snapshot and write through the host (the app's
//! `orchestration/live.rs`, without the window and the nodes). The boards,
//! git, the memory and the bridge queue are core's on this machine.

use std::sync::{Arc, Mutex};

use hangar_core::board::Task;
use hangar_core::git::{
    Changes, CopyResult, DeleteBranchResult, HookReport, RemoveResult, RepoInfo, Worktree,
};
use hangar_core::memory::MemoryEntry;

use super::{EngineHost, Orchestration};
use crate::control::port::{ModelCmd, open_in};
use crate::orchestration::brief::{BriefingPort, CheckCost};
use crate::orchestration::checks::last_check;
use crate::orchestration::events::{EngineIo, JournalEntry, OrchestratorEvent};
use crate::orchestration::manager_feed::ManagerJournalEntry;
use crate::orchestration::port::{Announcement, NewPane};
use crate::orchestration::projects::{self, ProjectSummary, ProjectsPort};
use crate::orchestration::runtime::RuntimeHost;
use crate::orchestration::text::now_ms;
use crate::orchestration::worker::{Starting, WorkerPort};
use crate::state::types::{AppState, Pane, PaneId};
use crate::state::{PaneActivity, PanePatch, Snapshot, StoreOutcome};

/// The host as the runtime's.
pub struct HostRuntime {
    host: EngineHost,
}

impl HostRuntime {
    pub(super) fn new(host: EngineHost) -> HostRuntime {
        HostRuntime { host }
    }
}

impl RuntimeHost for HostRuntime {
    type Port = HostPorts;

    fn port(&self, handle: &Orchestration) -> HostPorts {
        sweep_leftovers_once(&self.host.snapshot().state);
        HostPorts {
            host: self.host.clone(),
            handle: handle.clone(),
        }
    }
}

/// The deliveries' check worktrees left by a run that ended badly, swept
/// once in the workspaces' folders.
fn sweep_leftovers_once(state: &AppState) {
    static DONE: std::sync::Once = std::sync::Once::new();
    let folders: Vec<String> = state
        .workspaces
        .iter()
        .map(|ws| ws.cwd.clone())
        .filter(|cwd| !cwd.is_empty())
        .collect();
    if folders.is_empty() {
        return;
    }
    DONE.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("delivery-sweep".into())
            .spawn(move || hangar_core::delivery::sweep_leftovers(&folders));
    });
}

/// The engines' ports over the host.
#[derive(Clone)]
pub struct HostPorts {
    host: EngineHost,
    handle: Orchestration,
}

impl std::fmt::Debug for HostPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HostPorts(..)")
    }
}

/// `f` on tokio's blocking pool: board files and the memory are read there.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| format!("the task stopped: {err}"))
}

impl HostPorts {
    fn snapshot(&self) -> Arc<Snapshot> {
        self.host.snapshot()
    }

    fn write(&self, cmd: ModelCmd) -> Option<StoreOutcome> {
        self.host.apply(cmd).ok()
    }

    fn enqueue_now(&self, pane: &str, text: &str, from: &str) {
        hangar_core::bridge::bridge_enqueue(
            self.host.core(),
            pane.into(),
            text.into(),
            Some(from.into()),
        );
    }

    async fn board_tasks(&self, cwd: &str) -> Result<Vec<Task>, String> {
        let cwd = cwd.to_string();
        blocking(move || hangar_core::board_load(cwd).tasks).await
    }

    async fn memory(&self) -> Result<Vec<MemoryEntry>, String> {
        let core = self.host.core().clone();
        blocking(move || hangar_core::memory::memory_load(&core).map(|memory| memory.entries))
            .await?
    }
}

impl EngineIo for HostPorts {
    fn state(&self) -> Arc<AppState> {
        self.snapshot().state.clone()
    }

    fn activity(&self, pane: &str) -> Option<PaneActivity> {
        self.snapshot().activity.get(pane).cloned()
    }

    fn load_board(&self, cwd: &str) -> Result<Vec<Task>, String> {
        Ok(hangar_core::board_load(cwd.into()).tasks)
    }

    fn enqueue(&self, pane: &str, text: &str, from: &str) -> Result<(), String> {
        self.enqueue_now(pane, text, from);
        Ok(())
    }
}

impl WorkerPort for HostPorts {
    fn state(&self) -> Arc<AppState> {
        self.snapshot().state.clone()
    }

    fn is_open(&self, workspace_id: &str) -> bool {
        open_in(&self.snapshot(), workspace_id)
    }

    fn activity(&self, pane: &str) -> Option<PaneActivity> {
        self.snapshot().activity.get(pane).cloned()
    }

    fn starting(&self) -> &Mutex<Starting> {
        self.handle.starting()
    }

    fn journal(&self, workspace_id: &str) -> Vec<JournalEntry> {
        self.handle.journal(workspace_id)
    }

    fn post_event(&self, workspace_id: &str, event: OrchestratorEvent) {
        self.handle.post(workspace_id, event);
    }

    fn announce(&self, announcement: Announcement) {
        let _ = self.host.apply(ModelCmd::Announce(announcement));
    }

    async fn add_pane(&self, workspace_id: &str, pane: NewPane) -> Option<PaneId> {
        let init = pane.init.map(|init| {
            PanePatch::new(move |target: &mut Pane| {
                target.role = Some(init.role);
                target.task_id = Some(init.task_id.clone());
                target.model = Some(init.model.clone());
                target.port_base = Some(init.port_base);
                if init.shared_folder.is_some() {
                    target.shared_folder = init.shared_folder;
                }
            })
        });
        self.write(ModelCmd::AddPane {
            ws: workspace_id.into(),
            agent: Some(pane.agent),
            near: pane.near,
            dir: None,
            cwd: Some(pane.cwd),
            name: Some(pane.name),
            init,
        })?
        .pane
    }

    async fn close_pane(&self, workspace_id: &str, pane: &str) {
        self.write(ModelCmd::ClosePane {
            ws: workspace_id.into(),
            pane: pane.into(),
        });
    }

    fn touch(&self, workspace_id: &str) {
        self.write(ModelCmd::Touch(workspace_id.into()));
    }

    fn open_workspace(&self, workspace_id: &str) {
        self.write(ModelCmd::OpenWorkspace(workspace_id.into()));
    }

    async fn kill_pane(&self, pane: &str) -> Result<(), String> {
        let terminals = self.host.terminals().clone();
        let pane = pane.to_string();
        blocking(move || terminals.kill(&pane)).await?
    }

    async fn alive(&self, pane: &str) -> bool {
        let terminals = self.host.terminals().clone();
        let pane = pane.to_string();
        blocking(move || terminals.alive(&pane))
            .await
            .unwrap_or(false)
    }

    async fn enqueue(&self, pane: &str, text: &str, from: &str) -> Result<(), String> {
        self.enqueue_now(pane, text, from);
        Ok(())
    }

    async fn dir_exists(&self, path: &str) -> bool {
        let path = path.to_string();
        blocking(move || hangar_core::store::dir_exists(path))
            .await
            .unwrap_or(false)
    }

    async fn sleep(&self, ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }

    async fn board_load(&self, cwd: &str) -> Result<Vec<Task>, String> {
        self.board_tasks(cwd).await
    }

    async fn claim(
        &self,
        cwd: &str,
        id: &str,
        worker: &str,
        shared_folder: bool,
    ) -> Result<Task, String> {
        let (core, cwd, id, worker) = (
            self.host.core().clone(),
            cwd.to_string(),
            id.to_string(),
            worker.to_string(),
        );
        blocking(move || {
            hangar_core::board_claim_worker(&core, cwd, id, worker, Some(shared_folder))
        })
        .await?
    }

    async fn release(&self, cwd: &str, id: &str, worker: &str, undo: bool) -> Result<Task, String> {
        let (core, cwd, id, worker) = (
            self.host.core().clone(),
            cwd.to_string(),
            id.to_string(),
            worker.to_string(),
        );
        blocking(move || hangar_core::board_release_worker(&core, cwd, id, worker, undo)).await?
    }

    async fn repo_info(&self, path: &str) -> RepoInfo {
        hangar_core::git::git_repo_info(path.into()).await
    }

    async fn worktree_list(&self, path: &str) -> Result<Vec<Worktree>, String> {
        hangar_core::git::git_worktree_list(path.into()).await
    }

    async fn worktree_add(
        &self,
        path: &str,
        worktree_path: &str,
        branch: &str,
        create_branch: bool,
        base_ref: Option<&str>,
    ) -> Result<(), String> {
        hangar_core::git::git_worktree_add(
            path.into(),
            worktree_path.into(),
            branch.into(),
            create_branch,
            base_ref.map(str::to_string),
        )
        .await
    }

    async fn worktree_remove(
        &self,
        path: &str,
        worktree_path: &str,
        force: bool,
    ) -> Result<RemoveResult, String> {
        hangar_core::git::git_worktree_remove(path.into(), worktree_path.into(), force, None).await
    }

    async fn worktree_hook(
        &self,
        path: &str,
        worktree_path: &str,
        command: &str,
    ) -> Result<Option<HookReport>, String> {
        hangar_core::git::git_worktree_hook(path.into(), worktree_path.into(), Some(command.into()))
            .await
    }

    async fn branch_delete(
        &self,
        path: &str,
        branch: &str,
        force: bool,
    ) -> Result<DeleteBranchResult, String> {
        hangar_core::git::git_branch_delete(path.into(), branch.into(), force).await
    }

    async fn copy_ignored(
        &self,
        from: &str,
        to: &str,
        patterns: &[&str],
    ) -> Result<CopyResult, String> {
        let patterns = patterns.iter().map(|pattern| pattern.to_string()).collect();
        hangar_core::git::git_copy_untracked(from.into(), to.into(), patterns, Some(true)).await
    }

    async fn changes(&self, path: &str) -> Result<Changes, String> {
        hangar_core::git::git_changes(path.into(), None).await
    }

    async fn commits_ahead(&self, path: &str, base: &str, head: &str) -> Result<u32, String> {
        hangar_core::git::git_commits_ahead(path.into(), base.into(), head.into()).await
    }
}

impl BriefingPort for HostPorts {
    fn state(&self) -> Arc<AppState> {
        self.snapshot().state.clone()
    }

    fn last_check(&self, workspace_id: &str) -> Option<CheckCost> {
        last_check(workspace_id)
    }

    async fn load_board(&self, cwd: &str) -> Result<Vec<Task>, String> {
        self.board_tasks(cwd).await
    }

    async fn load_memory(&self) -> Result<Vec<MemoryEntry>, String> {
        self.memory().await
    }

    fn journal(&self, workspace_id: &str) -> Vec<JournalEntry> {
        self.handle.journal(workspace_id)
    }

    fn activity(&self, pane: &str) -> Option<PaneActivity> {
        self.snapshot().activity.get(pane).cloned()
    }

    fn now(&self) -> u64 {
        now_ms()
    }

    async fn is_repo(&self, cwd: &str) -> Result<Option<bool>, String> {
        let repo = hangar_core::git::git_repo_info(cwd.into()).await;
        Ok(Some(repo.installed && repo.is_repo))
    }

    async fn load_projects(&self) -> Result<Vec<ProjectSummary>, String> {
        Ok(projects::load_projects(self).await)
    }

    fn manager_journal(&self) -> Vec<ManagerJournalEntry> {
        self.handle.manager_journal()
    }

    fn enqueue(&self, pane: &str, text: &str, from: &str) -> Result<(), String> {
        self.enqueue_now(pane, text, from);
        Ok(())
    }
}

impl ProjectsPort for HostPorts {
    fn state(&self) -> Arc<AppState> {
        self.snapshot().state.clone()
    }

    async fn load_board(&self, cwd: &str) -> Result<Vec<Task>, String> {
        self.board_tasks(cwd).await
    }

    async fn load_memory(&self) -> Result<Vec<MemoryEntry>, String> {
        self.memory().await
    }

    async fn is_repo(&self, cwd: &str) -> Result<Option<bool>, String> {
        let repo = hangar_core::git::git_repo_info(cwd.into()).await;
        Ok(repo.installed.then_some(repo.is_repo))
    }

    fn activity(&self, pane: &str) -> Option<PaneActivity> {
        self.snapshot().activity.get(pane).cloned()
    }

    fn asleep(&self, workspace_id: &str) -> bool {
        self.snapshot().asleep.contains(workspace_id)
    }

    fn is_open(&self, workspace_id: &str) -> bool {
        open_in(&self.snapshot(), workspace_id)
    }

    fn journal(&self, workspace_id: &str) -> Vec<JournalEntry> {
        self.handle.journal(workspace_id)
    }
}
