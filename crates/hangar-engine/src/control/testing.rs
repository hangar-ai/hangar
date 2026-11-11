//! A delivery's world for tests, shared by the engine's and the hosts': a
//! fake of what deliveries reach (`FakeDelivery`), the orchestration hooks
//! that record the events (`Events`), and a workspace with a worker on a
//! task, ready to submit (`setup`).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::oneshot;

use hangar_core::accounts::Account;
use hangar_core::board::{Board, Comment, Task, TaskPatch};
use hangar_core::delivery::{Cleanup, Delivery, Kind, Outcome, Plan, Running, Tested};
use hangar_core::shells::ShellInfo;

use super::delivery::{control, delivery_state};
use super::fake::{Fake, lock, pane, workspace};
use super::port::{BoxFuture, DeliveryDeps, OrchestrationHooks, Port, Projects};
use super::types::{Caller, OrchestratorEvent};
use crate::state::types::{OrchestrationSettings, PaneRole};
use crate::state::{AgentId, AppState, Pane, Workspace, WsId};

/// What the next delivery run answers: at once, or once `finish` says.
pub enum Next {
    Now(Box<Result<Outcome, String>>),
    Hold,
}

pub struct FakeDelivery {
    pub log: Arc<Mutex<Vec<String>>>,
    pub tasks: Mutex<Vec<Task>>,
    pub runs: Mutex<Vec<Delivery>>,
    pub next: Mutex<Next>,
    pub held: Mutex<Option<oneshot::Sender<Outcome>>>,
    pub preflight_error: Mutex<Option<String>>,
    /// What preflight says waits for the person, when asked about a task.
    pub tested: Mutex<Option<Tested>>,
    pub preflight_tasks: Mutex<Vec<Option<String>>>,
    pub running: Mutex<Vec<Running>>,
    /// How many times the accounts were listed.
    pub listed_accounts: Mutex<usize>,
    /// What bringing a branch back from a node says instead of its checkout.
    pub bring_back_error: Mutex<Option<String>>,
}

pub fn ready<T: Send + 'static>(value: T) -> BoxFuture<T> {
    Box::pin(std::future::ready(value))
}

impl FakeDelivery {
    pub fn push(&self, line: String) {
        lock(&self.log).push(line);
    }

    pub fn change(&self, id: &str, edit: impl FnOnce(&mut Task)) -> Result<Task, String> {
        let mut tasks = lock(&self.tasks);
        let task = tasks
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("task not found")?;
        edit(task);
        Ok(task.clone())
    }
}

impl DeliveryDeps for FakeDelivery {
    fn preflight(
        &self,
        _workspace: String,
        worktree: String,
        target: Option<String>,
        landing: bool,
        task: Option<String>,
    ) -> BoxFuture<Result<Plan, String>> {
        self.push(format!(
            "preflight:{worktree}:{}:{landing}",
            target.as_deref().unwrap_or("null")
        ));
        let asked = task.is_some();
        lock(&self.preflight_tasks).push(task);
        if let Some(error) = lock(&self.preflight_error).clone() {
            return ready(Err(error));
        }
        ready(Ok(Plan {
            repo: "C:\\dev\\app".into(),
            queue: "c:/dev/app/.git".into(),
            branch: "worker/t1".into(),
            target: "main".into(),
            ahead: 2,
            tested: if asked {
                lock(&self.tested).clone()
            } else {
                None
            },
        }))
    }

    fn run(&self, delivery: Delivery) -> BoxFuture<Result<Outcome, String>> {
        lock(&self.runs).push(delivery);
        match &*lock(&self.next) {
            Next::Now(outcome) => ready((**outcome).clone()),
            Next::Hold => {
                let (tx, rx) = oneshot::channel();
                *lock(&self.held) = Some(tx);
                Box::pin(async move { rx.await.map_err(|_| "never finished".to_string()) })
            }
        }
    }

    fn cleanup(
        &self,
        worktree: String,
        branch: String,
        target: String,
        _task_id: String,
    ) -> BoxFuture<Result<Cleanup, String>> {
        self.push(format!("cleanup:{worktree}:{branch}:{target}"));
        ready(Ok(Cleanup {
            worktree_removed: true,
            branch_deleted: true,
            kept: vec![],
        }))
    }

    fn running(&self) -> BoxFuture<Result<Vec<Running>, String>> {
        ready(Ok(lock(&self.running).clone()))
    }

    fn accounts(&self) -> BoxFuture<Result<Vec<Account>, String>> {
        *lock(&self.listed_accounts) += 1;
        ready(Ok(vec![Account {
            id: "work".into(),
            label: "Work".into(),
            is_main: false,
            logged_in: true,
            email: None,
            plan: None,
        }]))
    }

    fn shells(&self) -> Vec<ShellInfo> {
        vec![ShellInfo {
            id: "pwsh".into(),
            label: "PowerShell 7".into(),
            program: "pwsh.exe".into(),
            args: vec![],
        }]
    }

    fn board_load(&self, _cwd: &str) -> BoxFuture<Result<Board, String>> {
        ready(Ok(Board {
            tasks: lock(&self.tasks).clone(),
        }))
    }

    fn board_update(
        &self,
        _cwd: &str,
        id: &str,
        patch: TaskPatch,
    ) -> BoxFuture<Result<Task, String>> {
        self.push(format!(
            "update:{id}:{}:{}",
            patch.column.as_deref().unwrap_or(""),
            patch
                .labels
                .as_ref()
                .map(|labels| labels.join(","))
                .unwrap_or_default()
        ));
        ready(self.change(id, |task| {
            if let Some(column) = patch.column {
                task.column = column;
            }
            if let Some(labels) = patch.labels {
                task.labels = labels;
            }
        }))
    }

    fn board_comment(
        &self,
        _cwd: &str,
        id: &str,
        author: &str,
        text: &str,
    ) -> BoxFuture<Result<Task, String>> {
        self.push(format!("comment:{id}:{author}:{text}"));
        ready(self.change(id, |task| {
            task.comments.push(Comment {
                id: "c".into(),
                author: author.into(),
                text: text.into(),
                created_at: 0,
            })
        }))
    }

    fn tell(&self, pane: &crate::state::PaneId, text: &str) -> BoxFuture<Result<(), String>> {
        self.push(format!("tell:{pane}:{text}"));
        ready(Ok(()))
    }

    fn bring_back(
        &self,
        workspace: String,
        node: String,
        worktree: String,
        landing: bool,
    ) -> BoxFuture<Result<String, String>> {
        self.push(format!(
            "bring_back:{workspace}:{node}:{worktree}:{landing}"
        ));
        match lock(&self.bring_back_error).clone() {
            Some(error) => ready(Err(error)),
            None => ready(Ok(BROUGHT.into())),
        }
    }

    fn clean_node(
        &self,
        node: String,
        worktree: String,
        branch: String,
    ) -> BoxFuture<Result<Cleanup, String>> {
        self.push(format!("clean_node:{node}:{worktree}:{branch}"));
        ready(Ok(Cleanup {
            worktree_removed: true,
            branch_deleted: false,
            kept: vec!["the branch worker/t1 on the node could not be deleted: held".into()],
        }))
    }
}

/// A worker's worktree on a node, and where its branch is checked out here
/// once brought back.
pub const NODE_TREE: &str = "/home/h/hangar-repos/app-1a2b3c4d-worktrees/worker-t1";
pub const BROUGHT: &str = "C:\\dev\\app-worktrees\\worker-t1";

/// The orchestration engine, as far as deliveries reach it: the events.
#[derive(Default)]
pub struct Events(pub Mutex<Vec<(WsId, OrchestratorEvent)>>);

impl OrchestrationHooks for Events {
    fn worker_control(
        &self,
        _port: Port,
        _op: String,
        _ws: Workspace,
        _args: Value,
        _caller: Caller,
    ) -> BoxFuture<Result<Value, String>> {
        ready(Err("not here".into()))
    }

    fn briefing(&self, _port: Port, _args: Value) -> BoxFuture<Result<Value, String>> {
        ready(Ok(Value::Null))
    }

    fn post(&self, ws: &WsId, event: OrchestratorEvent, _quiet: bool) {
        lock(&self.0).push((ws.clone(), event));
    }

    fn projects(&self, _port: Port) -> BoxFuture<Result<Projects, String>> {
        ready(Ok(Projects::default()))
    }
}

pub fn task() -> Task {
    Task {
        id: "t1".into(),
        title: "Add the export button".into(),
        description: "A button that exports the board as CSV.".into(),
        column: "doing".into(),
        priority: 2,
        assignee: Some("Bo".into()),
        labels: vec!["ui".into()],
        comments: vec![],
        depends_on: vec![],
        created_at: 0,
        updated_at: 0,
        order: 1.0,
        doing_since: None,
        owns: vec!["src/export".into()],
        attempts: 0,
        blocked: None,
        progress: None,
    }
}

pub fn outcome(kind: Kind, detail: &str) -> Outcome {
    Outcome {
        outcome: kind,
        detail: detail.into(),
        files: vec![],
        in_checkout: false,
        changed: vec!["src/export/button.tsx".into()],
        outside_owns: vec![],
        check: None,
        check_skipped: None,
        review: None,
        merge: Some("abcdef1234567890".into()),
        base: Some("1234567abcdef".into()),
        landed_in: Some("C:\\dev\\app".into()),
    }
}

pub fn worker() -> Pane {
    let mut bo = pane("p2", "Bo", AgentId::Claude, Some("C:\\dev\\app-wt\\t1"));
    bo.session_id = None;
    bo.role = Some(PaneRole::Worker);
    bo.task_id = Some("t1".into());
    bo.port_base = Some(41020);
    bo
}

pub struct Setup {
    pub fake: Arc<Fake>,
    pub delivery: Arc<FakeDelivery>,
    pub events: Arc<Events>,
}

pub fn setup() -> Setup {
    let mut ava = pane("p1", "Ava", AgentId::Claude, None);
    ava.role = Some(PaneRole::Orchestrator);
    let mut app = workspace(
        "w1",
        "App",
        "C:\\dev\\app",
        vec![ava, worker(), pane("p3", "Cy", AgentId::Claude, None)],
    );
    app.shell_id = Some("pwsh".into());
    app.orchestration = Some(OrchestrationSettings {
        check_command: Some("pnpm test".into()),
        ..OrchestrationSettings::default()
    });
    let fake = Fake::with_state(AppState {
        workspaces: vec![app],
        active_workspace_id: Some("w1".into()),
        ..AppState::default()
    });
    let delivery = Arc::new(FakeDelivery {
        log: fake.log.clone(),
        tasks: Mutex::new(vec![task()]),
        runs: Mutex::new(vec![]),
        next: Mutex::new(Next::Now(Box::new(Ok(outcome(
            Kind::Landed,
            "worker landed on main (abcdef1).",
        ))))),
        held: Mutex::new(None),
        preflight_error: Mutex::new(None),
        tested: Mutex::new(None),
        preflight_tasks: Mutex::new(vec![]),
        running: Mutex::new(vec![]),
        listed_accounts: Mutex::new(0),
        bring_back_error: Mutex::new(None),
    });
    let events = Arc::new(Events::default());
    *lock(&fake.delivery) = Some(delivery.clone());
    *lock(&fake.engine) = Some(events.clone());
    Setup {
        fake,
        delivery,
        events,
    }
}

impl Setup {
    pub fn ws(&self) -> Workspace {
        lock(&self.fake.state).workspaces[0].clone()
    }

    pub fn port(&self) -> Port {
        self.fake.port()
    }

    pub async fn submit_as(&self, args: Value, pane: Option<&str>) -> Result<Value, String> {
        let caller = Caller {
            pane_id: pane.map(str::to_string),
            pane_name: None,
            cwd: Some("C:\\dev\\app".into()),
        };
        control("delivery_submit", &self.ws(), &args, &caller, &self.port()).await
    }

    pub async fn submit(&self, args: Value) -> Result<Value, String> {
        self.submit_as(args, Some("p2")).await
    }

    pub fn next(&self, next: Next) {
        *lock(&self.delivery.next) = next;
    }

    pub fn now(&self, kind: Kind, detail: &str, edit: impl FnOnce(&mut Outcome)) {
        let mut answer = outcome(kind, detail);
        edit(&mut answer);
        self.next(Next::Now(Box::new(Ok(answer))));
    }

    /// Ends the delivery held in its run.
    pub fn finish(&self, answer: Outcome) {
        let held = lock(&self.delivery.held)
            .take()
            .expect("no delivery is held");
        let _ = held.send(answer);
    }

    pub fn runs(&self) -> Vec<Delivery> {
        lock(&self.delivery.runs).clone()
    }

    pub fn task(&self) -> Task {
        lock(&self.delivery.tasks)[0].clone()
    }

    pub fn set_tasks(&self, tasks: Vec<Task>) {
        *lock(&self.delivery.tasks) = tasks;
    }

    pub fn line(&self, prefix: &str) -> String {
        self.fake
            .logged(prefix)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("nothing logged as {prefix}"))
    }

    pub fn events(&self) -> Vec<Value> {
        lock(&self.events.0)
            .iter()
            .map(|(_, event)| serde_json::to_value(event).unwrap())
            .collect()
    }

    pub fn state(&self) -> Option<String> {
        delivery_state(&self.port(), "t1").map(|state| state.state().to_string())
    }

    pub fn settings(&self, edit: impl FnOnce(&mut OrchestrationSettings)) {
        self.fake
            .patch("w1", |ws| edit(ws.orchestration.as_mut().unwrap()));
    }
}

/// Until `cond` holds, letting what runs in the background go on meanwhile.
pub async fn until(cond: impl Fn() -> bool) {
    for _ in 0..1000 {
        if cond() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("never came");
}

pub fn delivery_json(delivery: &Delivery) -> Value {
    json!({
        "workspace": delivery.workspace,
        "worktree": delivery.worktree,
        "branch": delivery.branch,
        "target": delivery.target,
        "task": {"id": delivery.task.id, "title": delivery.task.title, "owns": delivery.task.owns},
        "summary": delivery.summary,
        "checkCommand": delivery.check_command,
        "reviewerModel": delivery.reviewer_model,
        "claudeAccount": delivery.claude_account,
        "land": delivery.land,
        "reuse": delivery.reuse,
        "portBase": delivery.port_base,
        "shell": delivery.shell.as_ref().map(|shell| json!({"id": shell.id})),
    })
}

/* ── delivery_submit ─────────────────────────────────────────── */
