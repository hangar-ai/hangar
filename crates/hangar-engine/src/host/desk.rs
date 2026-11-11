//! The desk over a host with no window: what the chat view and the phone
//! call (`mobile::desk`, the `remote_*` operations), and what a connector
//! (Discord) calls the same way, read from the host and this machine.
//!
//! What a server has no use for answers plainly: no activity history on
//! disk, no usage statistics, no apps listening on a PC.

use std::sync::{Arc, Weak};
use std::time::Duration;

use hangar_core::chat::ChatPage;
use hangar_core::git::{ChangedFile, Changes, FileDiff};
use hangar_core::mobile_data::ActivityEntry;
use hangar_core::stats::UsageStats;

use super::{EngineHost, Inner};
use crate::control::port::BoxFuture;
use crate::mobile::engine::{ChatRead, Sources};
use crate::orchestration::manager_feed::ManagerJournalEntry;
use crate::orchestration::projects::ProjectSummary;

/// The desk's reads, over the host (held weakly: the host holds the desk).
pub struct HostSources {
    host: Weak<Inner>,
}

impl HostSources {
    pub(super) fn new(host: &EngineHost) -> HostSources {
        HostSources {
            host: Arc::downgrade(&host.0),
        }
    }

    fn host(&self) -> Option<EngineHost> {
        self.host.upgrade().map(EngineHost)
    }
}

const GONE: &str = "Hangar is stopping.";

impl Sources for HostSources {
    fn projects(&self) -> BoxFuture<Vec<ProjectSummary>> {
        let runtime = self.host().and_then(|host| host.orchestration().cloned());
        Box::pin(async move {
            match runtime {
                Some(runtime) => runtime.load_projects().await,
                None => Vec::new(),
            }
        })
    }

    fn manager_journal(&self) -> Vec<ManagerJournalEntry> {
        self.host()
            .and_then(|host| host.orchestration().map(|r| r.manager_journal()))
            .unwrap_or_default()
    }

    /// No history is kept on disk here: what is lately is the journals'.
    fn history(&self, since: u64) -> BoxFuture<Result<Vec<ActivityEntry>, String>> {
        let _ = since;
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    fn chat(&self, read: ChatRead) -> BoxFuture<Result<Option<ChatPage>, String>> {
        Box::pin(hangar_core::chat::chat_page(
            read.session_id,
            read.cwd,
            read.before,
            read.limit,
        ))
    }

    fn usage(&self) -> BoxFuture<Result<Arc<UsageStats>, String>> {
        Box::pin(std::future::ready(Err(
            "The usage statistics are not read on this Hangar.".to_string(),
        )))
    }

    fn worktrees(&self, cwd: &str) -> BoxFuture<Result<Vec<String>, String>> {
        let cwd = cwd.to_string();
        Box::pin(async move {
            Ok(hangar_core::git::git_worktree_list(cwd)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|tree| tree.path)
                .collect())
        })
    }

    fn stats_now(&self) -> i64 {
        crate::stats::time::now_ms()
    }

    fn changes(&self, path: &str, base: &str) -> BoxFuture<Result<Changes, String>> {
        Box::pin(hangar_core::git::git_changes(
            path.to_string(),
            Some(base.to_string()),
        ))
    }

    fn file_diff(
        &self,
        path: &str,
        file: &ChangedFile,
        base: &str,
    ) -> BoxFuture<Result<FileDiff, String>> {
        Box::pin(hangar_core::git::git_file_diff(
            path.to_string(),
            file.path.clone(),
            file.old_path.clone(),
            file.status == "?",
            Some(base.to_string()),
        ))
    }

    fn wait(&self, ms: u64) -> BoxFuture<()> {
        Box::pin(tokio::time::sleep(Duration::from_millis(ms)))
    }

    /// No app of a worker is opened from here.
    fn listening(&self, ports: Vec<u16>) -> BoxFuture<Result<Vec<u16>, String>> {
        let _ = ports;
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    fn tasks(&self, cwd: &str) -> BoxFuture<Result<Vec<(String, String)>, String>> {
        let cwd = cwd.to_string();
        let alive = self.host().is_some();
        Box::pin(async move {
            if !alive {
                return Err(GONE.to_string());
            }
            tokio::task::spawn_blocking(move || {
                hangar_core::board_load(cwd)
                    .tasks
                    .into_iter()
                    .map(|task| (task.id, task.title))
                    .collect()
            })
            .await
            .map_err(|err| err.to_string())
        })
    }
}
