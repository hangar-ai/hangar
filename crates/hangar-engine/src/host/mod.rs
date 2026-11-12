//! A host for the engine with no window (docs/native/headless.md, step 2):
//! what the desktop app does around the engine, done on tokio's clock.
//!
//! [`EngineHost`] holds the store and the panes' read-model in one place, and
//! publishes their [`Snapshot`] after every change, which the engines read.
//! It
//!
//! - loads the state from its file and saves it back on tokio's clock,
//!   [`SAVE_DEBOUNCE`] after the last change, written whole to a temporary
//!   file then renamed, and once more on the way out ([`EngineHost::shutdown`]);
//! - applies the engines' model writes ([`ModelCmd`], `writes.rs`); what only
//!   a window can do (switch to a pane, detach a workspace, run a palette
//!   command) is refused or has nothing to do;
//! - keeps every workspace open and a shell running for each of its panes
//!   (`launch.rs`): started through [`Terminals`], the agent's command typed
//!   once the shell has settled, as the desktop's panes do;
//! - feeds core's events to the panes' state machines (the mod's reports,
//!   the queue, the deliveries' stages) and to the orchestration runtime,
//!   which it starts with its own ports (`orchestrate.rs`);
//! - answers the agents' control requests through the engine's dispatch,
//!   over a port of its own (`port.rs`), when asked to
//!   ([`EngineHost::serve_control`]); a node answers those of its own panes
//!   only, and hands the others to the desktop that drives it
//!   ([`EngineHost::answer`]).
//!
//! What it says happened (an agent's announcement, a change) goes out as
//! [`News`] to whoever follows it: a connector (Discord) or a desktop.

mod desk;
mod launch;
mod orchestrate;
mod port;
pub mod screen;
pub mod terminals;
mod writes;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Notify, broadcast};

use hangar_core::Event;
use hangar_i18n::{Locale, Translator};

use crate::control::delivery::Deliveries;
use crate::control::port::{CmdOut, MobileHooks, ModelCmd, Port};
use crate::control::run::{self, Gate};
use crate::control::types::ControlEvent;
use crate::live::{BridgeEvent, Delivering, PaneStatus};
use crate::mobile::engine::{Hook as DeskHook, Mobile};
use crate::orchestration::runtime::{self as orchestration_runtime, EngineMsg};
use crate::panes::bridge_events::carry_key;
use crate::panes::pane_index::info_of;
use crate::panes::{self, Ambient, Effect, EpochMs, PaneInfo, wall_clock_ms};
use crate::state::persist::{SAVE_DEBOUNCE, Writer};
use crate::state::types::{AppState, Pane, PaneId, Workspace, WsId};
use crate::state::{PanePatch, Snapshot, Store, StoreAction, StoreOutcome};

pub use desk::HostSources;
pub use orchestrate::{HostPorts, HostRuntime};
pub use port::HostPort;
pub use terminals::{CorePtys, Spawn, TermEvent, TermSink, Terminals};

/// The orchestration runtime, as this host runs it.
pub type Orchestration = orchestration_runtime::OrchestrationHandle<HostRuntime>;

/// What a host with no window starts from.
pub struct Config {
    /// Core's bus and folders (`hangar_core::headless::start`'s).
    pub core: hangar_core::Host,
    /// The panes' shells.
    pub terminals: Arc<dyn Terminals>,
    /// Where the state is kept (`state.json`).
    pub state_file: PathBuf,
}

/// What the host says happened.
#[derive(Debug, Clone, PartialEq)]
pub enum News {
    /// Something an agent did, which the desktop says on screen
    /// (`ModelCmd::Announce`).
    Announce(ControlEvent),
    /// The state or the panes' read-model changed: read the snapshot again.
    Changed,
    /// A pane's machine asked for a notification (an agent waiting, done).
    Notify {
        workspace: WsId,
        pane: PaneId,
        title: String,
        body: String,
    },
}

/// The state and the panes' read-model, behind one lock.
#[derive(Debug, Default)]
struct Model {
    store: Store,
    panes: panes::Live,
    /// Every workspace is open on a host with no window; one removed goes.
    opened: HashSet<WsId>,
    /// Deliveries running, per workspace, by task: how many hold each.
    runs: HashMap<WsId, HashMap<String, usize>>,
}

impl Model {
    fn delivering(&self) -> Delivering {
        self.runs
            .iter()
            .filter(|(_, tasks)| !tasks.is_empty())
            .map(|(ws, tasks)| (ws.clone(), tasks.keys().cloned().collect()))
            .collect()
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.store.state_arc(),
            opened: self.opened.clone(),
            asleep: self.panes.asleep_ids(),
            activity: self.panes.activity_map().clone(),
            delivering: self.delivering(),
            hydrated: true,
        }
    }

    fn locale(&self) -> Locale {
        self.store.settings().locale.unwrap_or_else(Locale::system)
    }

    /// Nobody looks at a pane of a host with no window, and no window has
    /// the system's focus: a notification is always wanted.
    fn ambient(&self, now: EpochMs) -> Ambient {
        let settings = self.store.settings();
        Ambient {
            now,
            looking: false,
            window_focused: false,
            notify_on_idle: settings.notify_on_idle,
            notify_only_when_away: settings.notify_only_when_away,
            t: Translator::new(self.locale()),
        }
    }

    /// The index of the panes brought up to date; the panes gone lose their
    /// API token.
    fn index(&mut self) {
        let state = self.store.state_arc();
        let revision = self.store.revision();
        let tokens = hangar_core::server::pane_tokens();
        for gone in self.panes.index.refresh(&state, revision) {
            tokens.revoke(&gone);
        }
    }

    fn info(&self, pane: &str) -> Option<Arc<PaneInfo>> {
        let (ws, found, info) = self.panes.index.find(self.store.state(), pane)?;
        Some(info.unwrap_or_else(|| Arc::new(info_of(ws, found))))
    }

    fn open_all(&mut self) {
        self.opened = self
            .store
            .state()
            .workspaces
            .iter()
            .map(|ws| ws.id.clone())
            .collect();
    }
}

struct Inner {
    core: hangar_core::Host,
    terminals: Arc<dyn Terminals>,
    state_file: PathBuf,
    model: Mutex<Model>,
    snapshot: RwLock<Arc<Snapshot>>,
    /// The revision last written to the file.
    saved: AtomicU64,
    writer: Writer,
    /// Rung on a change: the saver and the panes look.
    changed: Notify,
    deliveries: Deliveries,
    orchestration: OnceLock<Orchestration>,
    mobile: RwLock<Option<Arc<dyn MobileHooks>>>,
    /// The desk: what the chat view, the phone and a connector call.
    desk: OnceLock<Arc<Mobile>>,
    news: broadcast::Sender<News>,
    /// The shells this host started or is starting, by pane: the launch
    /// that holds each (a pane put to sleep and woken is launched again
    /// under the same id; an exited one stays held until it is restarted,
    /// under a new id).
    shells: Mutex<HashMap<PaneId, u64>>,
    /// The launches so far.
    launches: std::sync::atomic::AtomicU64,
    /// What each pane printed since the panes were last looked at.
    printed: Mutex<HashMap<PaneId, Arc<Printed>>>,
    started: AtomicBool,
    stopping: AtomicBool,
}

/// What a pane printed since the host last looked: written by the pane's
/// reader thread at each chunk with no lock taken, read on the host's tick
/// (as the desktop looks at the panes off screen with its own), which hands
/// the burst to the pane's machine.
#[derive(Debug, Default)]
pub(crate) struct Printed {
    /// When the burst not yet handed over began, in ms; 0: none.
    first: AtomicU64,
    /// When the pane last printed, in ms; 0: never.
    last: AtomicU64,
}

impl Printed {
    /// The pane printed, now.
    pub(crate) fn note(&self, now: EpochMs) {
        self.last.store(now, Ordering::Relaxed);
        let _ = self
            .first
            .compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
    }

    /// The burst since the last call, from its first chunk to its last.
    fn take(&self) -> Option<(EpochMs, EpochMs)> {
        let first = self.first.swap(0, Ordering::Relaxed);
        (first != 0).then(|| (first, self.last.load(Ordering::Relaxed).max(first)))
    }

    /// When the pane last printed.
    pub(crate) fn last(&self) -> Option<EpochMs> {
        Some(self.last.load(Ordering::Relaxed)).filter(|at| *at != 0)
    }
}

/// The engine, hosted with no window. Cloned freely: every clone is the same
/// host.
#[derive(Clone)]
pub struct EngineHost(Arc<Inner>);

impl std::fmt::Debug for EngineHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHost")
            .field("state_file", &self.0.state_file)
            .finish()
    }
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The state in `file`: `None` when there is none yet. An unreadable one is
/// an error, never an empty state: the first save would write over it.
pub fn read_state_file(file: &Path) -> Result<Option<crate::state::Loaded>, String> {
    let raw = match std::fs::read_to_string(file) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("{} cannot be read: {err}", file.display())),
    };
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("{} is not JSON: {err}", file.display()))?;
    let (state, kept) = crate::state::read_checked(value)
        .map_err(|reason| format!("{} cannot be used: {reason}", file.display()))?;
    Ok(Some(crate::state::Loaded { state, kept }))
}

/// `value` written to `file` whole or not at all: to a temporary file beside
/// it, then renamed over it, as the desktop writes its own (compact, and
/// the owner's alone). The folder is made when missing.
pub fn write_state_file(file: &Path, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    let temporary = file.with_extension("json.tmp");
    let write = || -> std::io::Result<()> {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut out = options.open(&temporary)?;
        out.write_all(&body)?;
        // On disk before it takes the state's name: renamed first, a power
        // cut can leave the name on an empty file (and a write a network
        // disk refused shows here, not after the old state is gone).
        out.sync_all()
    };
    if let Err(first) = write() {
        // The folder is missing on the first save alone.
        let made = file
            .parent()
            .filter(|_| first.kind() == std::io::ErrorKind::NotFound)
            .map(std::fs::create_dir_all);
        match made {
            Some(Ok(())) => write().map_err(|err| format!("{}: {err}", temporary.display()))?,
            _ => return Err(format!("{}: {first}", temporary.display())),
        }
    }
    std::fs::rename(&temporary, file).map_err(|err| format!("{}: {err}", file.display()))?;
    // The rename itself, on disk: a folder's entries are its own to flush.
    #[cfg(unix)]
    if let Some(dir) = file.parent()
        && let Ok(dir) = std::fs::File::open(dir)
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

impl EngineHost {
    /// The host, with the state its file holds (or an empty one). Nothing
    /// runs before [`EngineHost::start`].
    pub fn load(config: Config) -> Result<EngineHost, String> {
        let mut model = Model::default();
        if let Some(loaded) = read_state_file(&config.state_file)? {
            model.store.hydrate_read(loaded.state, loaded.kept);
        } else {
            model.store.hydrate(AppState::default());
        }
        model.open_all();
        model.index();
        let snapshot = Arc::new(model.snapshot());
        let (news, _) = broadcast::channel(256);
        Ok(EngineHost(Arc::new(Inner {
            core: config.core,
            terminals: config.terminals,
            state_file: config.state_file,
            saved: AtomicU64::new(model.store.revision()),
            model: Mutex::new(model),
            snapshot: RwLock::new(snapshot),
            writer: Writer::default(),
            changed: Notify::new(),
            deliveries: Deliveries::default(),
            orchestration: OnceLock::new(),
            mobile: RwLock::new(None),
            desk: OnceLock::new(),
            news,
            shells: Mutex::new(HashMap::new()),
            launches: std::sync::atomic::AtomicU64::new(0),
            printed: Mutex::new(HashMap::new()),
            started: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        })))
    }

    /// Starts the host's work on the current tokio runtime: core's events
    /// followed, the saves, the panes' shells, the orchestration runtime.
    /// Once; a second call does nothing.
    pub fn start(&self) {
        if self.0.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let events = self.0.core.subscribe();
        let runtime = orchestration_runtime::start(HostRuntime::new(self.clone()));
        let _ = self.0.orchestration.set(runtime);
        let desk = Mobile::new(Arc::new(HostSources::new(self)));
        self.install_mobile(Arc::new(DeskHook(desk.clone())));
        let _ = self.0.desk.set(desk);
        tokio::spawn(follow_events(self.clone(), events));
        // Listening from now, not from the task's first turn: a change in
        // between would be told to nobody.
        tokio::spawn(save_loop(self.clone(), self.news()));
        tokio::spawn(panes_loop(self.clone()));
        tokio::spawn(tick_loop(self.clone()));
        self.0.changed.notify_one();
    }

    /// Answers every control request core is asked, the agents' and
    /// Hangar's own (a briefing): for a host that is the only one answering
    /// on its machine. A node answers its own panes' alone
    /// ([`EngineHost::answer`]).
    pub fn serve_control(&self) {
        let host = self.clone();
        let mut events = self.0.core.subscribe();
        tokio::spawn(async move {
            loop {
                let (request, channel) = match events.recv().await {
                    Ok(Event::ControlRequest(request)) => (request, Gate::Agent),
                    Ok(Event::ControlInternal(request)) => (request, Gate::Person),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let host = host.clone();
                tokio::spawn(async move {
                    let internal = matches!(channel, Gate::Person);
                    let answer = host.answer(&request.op, request.args, internal).await;
                    reply(request.id, answer);
                });
            }
        });
    }

    /// One control request answered: an agent's (`internal` false), through
    /// the engine's dispatch, or Hangar's own (a pane's briefing, a phone's
    /// call).
    pub async fn answer(&self, op: &str, args: Value, internal: bool) -> Result<Value, String> {
        let port = self.port();
        if !internal {
            return run::run(op, args, &port, Gate::Agent).await;
        }
        if op == "orchestrator_briefing" {
            return match self.orchestration() {
                Some(runtime) => runtime.briefing(&args).await,
                None => Err("The orchestration engine is not running yet.".into()),
            };
        }
        if op.starts_with("remote_") {
            let mobile = self
                .0
                .mobile
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            return match mobile {
                Some(mobile) => mobile.remote(port, op.to_string(), args).await,
                None => Err("Hangar Mobile is not served by this Hangar.".into()),
            };
        }
        Err(run::unknown(op))
    }

    /// Whether `pane` is one of this host's.
    pub fn has_pane(&self, pane: &str) -> bool {
        self.snapshot()
            .state
            .workspaces
            .iter()
            .any(|ws| ws.panes.iter().any(|p| p.id == pane))
    }

    /// Puts what serves the phone's calls on the bus.
    pub fn install_mobile(&self, mobile: Arc<dyn MobileHooks>) {
        *self
            .0
            .mobile
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(mobile);
    }

    /// The state and the read-models as they stand.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.0
            .snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The control port, over this host.
    pub fn port(&self) -> Port {
        Arc::new(HostPort::new(self.clone()))
    }

    /// The orchestration runtime, once started.
    pub fn orchestration(&self) -> Option<&Orchestration> {
        self.0.orchestration.get()
    }

    /// The desk (`mobile::desk`'s calls take it), once started.
    pub fn desk(&self) -> Option<Arc<Mobile>> {
        self.0.desk.get().cloned()
    }

    pub fn core(&self) -> &hangar_core::Host {
        &self.0.core
    }

    pub fn terminals(&self) -> &Arc<dyn Terminals> {
        &self.0.terminals
    }

    /// What the host says from now on.
    pub fn news(&self) -> broadcast::Receiver<News> {
        self.0.news.subscribe()
    }

    /// The deliveries this host's port runs.
    pub fn deliveries(&self) -> &Deliveries {
        &self.0.deliveries
    }

    /// A model write of the engines', applied.
    pub fn apply(&self, cmd: ModelCmd) -> Result<CmdOut, String> {
        let (outcome, said) = {
            let mut model = locked(&self.0.model);
            let done = writes::carry_out(&mut model, cmd, wall_clock_ms());
            model.index();
            done
        };
        let outcome = outcome?;
        for news in said {
            let _ = self.0.news.send(news);
        }
        self.changed();
        Ok(outcome)
    }

    /// A store action of the person's (a connector creating a workspace,
    /// renaming one), applied as the window applies its own.
    pub fn edit(&self, action: StoreAction) -> StoreOutcome {
        let outcome = edited(&mut locked(&self.0.model), action);
        self.changed();
        outcome
    }

    /// A workspace in folder `cwd` (absolute, existing) with one Claude Code
    /// pane, its orchestrator unless `orchestrator` is false: how a server's
    /// workspace is made, from its command line or a connector. An empty
    /// name is the folder's.
    pub fn add_workspace(&self, name: &str, cwd: &str, orchestrator: bool) -> Result<WsId, String> {
        let path = Path::new(cwd);
        if !path.is_absolute() {
            return Err(format!("{cwd}: a workspace's folder is an absolute path"));
        }
        if !path.is_dir() {
            return Err(format!("{cwd} is not a folder here"));
        }
        let name = if name.trim().is_empty() {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Workspace".into())
        } else {
            name.trim().to_string()
        };
        // The workspace and its orchestrator's role in one change: told of
        // the pane before its role, the panes would start it as a plain
        // Claude Code, without Hangar's tools, and nothing here repairs a
        // pane that runs.
        let ws = {
            let mut model = locked(&self.0.model);
            let made = edited(
                &mut model,
                StoreAction::AddWorkspace(crate::state::WorkspaceDraft {
                    name,
                    cwd: cwd.to_string(),
                    layout: 1,
                    agents: vec![crate::state::types::AgentId::Claude],
                    ..Default::default()
                }),
            );
            let Some(ws) = made.workspace else {
                drop(model);
                self.changed();
                return Err("the workspace could not be made".into());
            };
            let first = model
                .store
                .workspace(&ws)
                .and_then(|w| w.panes.first().map(|p| p.id.clone()));
            if let Some(pane) = first.filter(|_| orchestrator) {
                edited(
                    &mut model,
                    StoreAction::UpdatePane {
                        ws: ws.clone(),
                        pane,
                        patch: PanePatch::new(|pane: &mut Pane| {
                            pane.role = Some(crate::state::types::PaneRole::Orchestrator);
                        }),
                    },
                );
            }
            ws
        };
        self.changed();
        Ok(ws)
    }

    /// The snapshot published again when what it holds changed, and those
    /// who follow told: the saver and the panes when the state, or which
    /// workspaces are up, did; nobody when nothing did (a report of the mod
    /// that moved no agent's activity changes nothing the engines read).
    fn changed(&self) {
        let panes_moved = {
            let mut model = locked(&self.0.model);
            // Nobody announces a respawn here: the panes follow the state.
            model.store.take_respawns();
            // A workspace removed takes its delivery check worktree with
            // it, as on the desktop: git forgets it, the disk lets it go.
            for removed in model.store.take_removed() {
                off_thread(move || {
                    hangar_core::delivery::remove_workspace_check(&removed.ws, &removed.cwd, None);
                });
            }
            let published = self.snapshot();
            let state_moved = !Arc::ptr_eq(&published.state, &model.store.state_arc());
            let up_moved = published.opened != model.opened
                || !same_keys(&published.asleep, model.panes.asleep());
            let live_moved = published.activity != *model.panes.activity_map()
                || !same_runs(&published.delivering, &model.runs);
            if !(state_moved || up_moved || live_moved) {
                return;
            }
            // Published while the model is held: two changes side by side
            // publish in the order they were made, never the older one
            // over the newer.
            *self
                .0
                .snapshot
                .write()
                .unwrap_or_else(PoisonError::into_inner) = Arc::new(model.snapshot());
            state_moved || up_moved
        };
        if panes_moved {
            self.0.changed.notify_one();
        }
        let _ = self.0.news.send(News::Changed);
    }

    /// The read-model may have changed, the persisted state did not.
    fn live_changed(&self) {
        self.changed();
    }

    /// Writes the state if it changed since the last save.
    pub async fn save(&self) -> Result<bool, String> {
        let (revision, save) = {
            let mut model = locked(&self.0.model);
            let revision = model.store.revision();
            if revision == self.0.saved.load(Ordering::SeqCst) {
                return Ok(false);
            }
            (revision, model.store.take_save())
        };
        let (file, writer) = (self.0.state_file.clone(), self.0.writer.clone());
        let written = tokio::task::spawn_blocking(move || {
            writer.write(revision, || write_state_file(&file, &save.into_json()))
        })
        .await
        .map_err(|err| format!("the save stopped: {err}"))??;
        self.0.saved.fetch_max(revision, Ordering::SeqCst);
        Ok(written)
    }

    /// The last save, then every shell taken down.
    pub async fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::SeqCst);
        if let Err(error) = self.save().await {
            eprintln!("the state could not be saved: {error}");
        }
        let terminals = self.0.terminals.clone();
        let _ = tokio::task::spawn_blocking(move || terminals.kill_all()).await;
    }

    /// A pane's shell said something.
    fn on_term(&self, event: TermEvent) {
        match event {
            TermEvent::Output(pane) => {
                // A pane's own sink notes its output without coming here
                // (`printed_of`); this is for whoever only has the event.
                self.printed_of(&pane).note(wall_clock_ms());
            }
            TermEvent::Exit(pane) => {
                let mine = locked(&self.0.shells).contains_key(&pane);
                if !mine {
                    return;
                }
                {
                    let mut model = locked(&self.0.model);
                    let now = wall_clock_ms();
                    if let Some(info) = model.info(&pane) {
                        model.panes.shell_exited(&info, now);
                    }
                    model.panes.status.insert(pane.clone(), PaneStatus::Exited);
                }
                if let Some(runtime) = self.orchestration() {
                    runtime.send(EngineMsg::PtyExit(pane));
                }
                self.live_changed();
            }
        }
    }

    /// One of core's events, taken in now: the mod's reports, the queue,
    /// the deliveries' stages and the boards written reach the panes' state
    /// machines and the orchestration. `start` follows core's bus with it; a
    /// host that hears the events some other way hands them over here.
    pub fn feed(&self, event: Event) {
        let now = wall_clock_ms();
        match event {
            Event::BridgeEvent { pane_id, event } => {
                let Some(report) = BridgeEvent::parse(&event) else {
                    return;
                };
                if let Some(desk) = self.0.desk.get() {
                    desk.note_bridge_event(&pane_id, &report, now);
                }
                self.with_pane(&pane_id, now, |live, info, amb| {
                    live.on_bridge(info, report.clone(), amb)
                });
                if let Some(runtime) = self.orchestration() {
                    runtime.send(EngineMsg::Bridge {
                        pane: pane_id,
                        event: report,
                    });
                }
            }
            Event::BridgeQueue(queue) => {
                locked(&self.0.model)
                    .panes
                    .queue_changed(&queue.pane_id, queue.pending, now);
                self.live_changed();
            }
            Event::DeliveryStages(stages) => {
                locked(&self.0.model).panes.deliveries_changed(&stages, now);
                self.live_changed();
            }
            Event::BoardChanged { .. } => {
                if let Some(runtime) = self.orchestration() {
                    runtime.send(EngineMsg::BoardChanged);
                }
            }
            _ => {}
        }
    }

    /// A machine step for a pane, and what it asked for done.
    fn with_pane(
        &self,
        pane: &str,
        now: EpochMs,
        step: impl FnOnce(&mut panes::Live, &PaneInfo, &Ambient) -> Vec<Effect>,
    ) {
        let (info, effects) = {
            let mut model = locked(&self.0.model);
            model.index();
            let Some(info) = model.info(pane) else {
                return;
            };
            let amb = model.ambient(now);
            let effects = step(&mut model.panes, &info, &amb);
            (info, effects)
        };
        for effect in effects {
            match effect {
                Effect::Notify { title, body } => {
                    let _ = self.0.news.send(News::Notify {
                        workspace: info.workspace_id.clone(),
                        pane: info.id.clone(),
                        title,
                        body,
                    });
                }
                Effect::CaptureSession(session_id) => {
                    let mut model = locked(&self.0.model);
                    model.store.apply(StoreAction::UpdatePane {
                        ws: info.workspace_id.clone(),
                        pane: info.id.clone(),
                        patch: PanePatch::new(move |pane: &mut Pane| {
                            pane.session_id = Some(session_id.clone());
                        }),
                    });
                    release_unheld_sessions(&mut model);
                }
            }
        }
        self.changed();
    }

    /// The panes that should run and do not, started; those that run and
    /// should not, killed.
    fn reconcile(&self) {
        if self.0.stopping.load(Ordering::SeqCst) {
            return;
        }
        let snapshot = self.snapshot();
        let up =
            |ws: &&Workspace| snapshot.opened.contains(&ws.id) && !snapshot.asleep.contains(&ws.id);
        let wanted: HashSet<&str> = snapshot
            .state
            .workspaces
            .iter()
            .filter(up)
            .flat_map(|ws| ws.panes.iter().map(|pane| pane.id.as_str()))
            .collect();
        let (gone, new): (Vec<PaneId>, Vec<(Workspace, Pane, u64)>) = {
            let mut shells = locked(&self.0.shells);
            let gone: Vec<PaneId> = shells
                .keys()
                .filter(|pane| !wanted.contains(pane.as_str()))
                .cloned()
                .collect();
            for pane in &gone {
                shells.remove(pane);
            }
            // Only a pane to start takes a copy of its workspace.
            let new: Vec<(Workspace, Pane, u64)> = snapshot
                .state
                .workspaces
                .iter()
                .filter(up)
                .flat_map(|ws| ws.panes.iter().map(move |pane| (ws, pane)))
                .filter(|(_, pane)| !shells.contains_key(&pane.id))
                .map(|(ws, pane)| {
                    let launch = self.0.launches.fetch_add(1, Ordering::Relaxed) + 1;
                    (ws.clone(), pane.clone(), launch)
                })
                .collect();
            for (_, pane, launch) in &new {
                shells.insert(pane.id.clone(), *launch);
            }
            (gone, new)
        };
        for pane in gone {
            self.unmounted(&pane);
            let terminals = self.0.terminals.clone();
            tokio::task::spawn_blocking(move || {
                let _ = terminals.kill(&pane);
            });
        }
        for (ws, pane, launch) in new {
            tokio::spawn(launch::start(self.clone(), ws, pane, launch));
        }
    }

    /// Whether `launch` still holds `pane`: not once the pane left the
    /// state (or its workspace went to sleep) while its shell was starting.
    fn held_by(&self, pane: &str, launch: u64) -> bool {
        locked(&self.0.shells).get(pane) == Some(&launch)
    }

    /// A pane's shell is about to start: its machine exists, so no report
    /// of the mod goes unheard, and the prompts a restart carried come along.
    fn mounted(&self, pane: &Pane, ws: &Workspace) {
        let now = wall_clock_ms();
        let carried = {
            let mut model = locked(&self.0.model);
            model.index();
            model.panes.mount_pane(&pane.id, &ws.id, &pane.name, now);
            // What was queued for it before it had a machine to hear of it
            // (a worker's brief is queued the moment its pane is added).
            let waiting = hangar_core::bridge::bridge_queue(pane.id.clone());
            if !waiting.is_empty() {
                model.panes.queue_changed(&pane.id, waiting, now);
            }
            model
                .panes
                .status
                .insert(pane.id.clone(), PaneStatus::Starting);
            model
                .panes
                .carried
                .take(&carry_key(&ws.id, &pane.name), now)
        };
        for prompt in carried {
            hangar_core::bridge::bridge_enqueue(
                &self.0.core,
                pane.id.clone(),
                prompt.text,
                prompt.from,
            );
        }
        self.live_changed();
    }

    fn set_status(&self, pane: &str, status: PaneStatus) {
        {
            let mut model = locked(&self.0.model);
            if model.panes.machines.contains_key(pane) {
                model.panes.status.insert(pane.to_string(), status);
            }
        }
        self.live_changed();
    }

    /// The agent's command was typed.
    fn launched(&self, pane: &str, resumed: Option<&str>) {
        let now = wall_clock_ms();
        {
            let mut model = locked(&self.0.model);
            let Some(info) = model.info(pane) else {
                return;
            };
            if let Some(id) = resumed {
                model.panes.claim_session(id);
            }
            // A Claude Code with the mod says it all in its hello.
            let says_itself = info.agent == crate::state::types::AgentId::Claude
                && model.store.settings().claude_bridge;
            if !says_itself && info.agent != crate::state::types::AgentId::Shell {
                model.panes.launched(&info, resumed, now);
            }
        }
        self.live_changed();
    }

    /// A pane's shell is gone with it: what was still queued for it comes
    /// along to its successor (a restart under a new id).
    fn unmounted(&self, pane: &str) {
        let now = wall_clock_ms();
        {
            let mut model = locked(&self.0.model);
            let carry = model.panes.machines.get(pane).and_then(|machine| {
                let successor = model
                    .store
                    .workspace(&machine.workspace_id)
                    .is_some_and(|ws| {
                        ws.panes
                            .iter()
                            .any(|p| p.name == machine.name && p.id != pane)
                    });
                successor.then(|| {
                    (
                        carry_key(&machine.workspace_id, &machine.name),
                        model.panes.insights.get(pane).queue.clone(),
                    )
                })
            });
            if let Some((key, queue)) = carry {
                model.panes.carried.stash(&key, queue, now);
            }
            model.panes.unmount_pane(pane, false);
            release_unheld_sessions(&mut model);
        }
        locked(&self.0.printed).remove(pane);
        self.live_changed();
    }

    /// Where pane `pane`'s output is noted: handed to its sink once, so a
    /// chunk printed takes no lock.
    pub(crate) fn printed_of(&self, pane: &str) -> Arc<Printed> {
        locked(&self.0.printed)
            .entry(pane.to_string())
            .or_default()
            .clone()
    }

    /// When `pane` last printed.
    fn last_printed(&self, pane: &str) -> Option<EpochMs> {
        locked(&self.0.printed).get(pane).and_then(|p| p.last())
    }

    /// The watchers' checks that came due, and the trust question looked
    /// for on the screens of the panes that print.
    fn tick(&self) {
        let now = wall_clock_ms();
        // What the panes printed since the last look, each as one burst.
        let bursts: Vec<(PaneId, EpochMs, EpochMs)> = locked(&self.0.printed)
            .iter()
            .filter_map(|(pane, printed)| {
                let (first, last) = printed.take()?;
                Some((pane.clone(), first, last))
            })
            .collect();
        let mut model = locked(&self.0.model);
        for (pane, first, last) in bursts {
            model.panes.note_printed(&pane, first, last);
        }
        let idle_ms = model.store.settings().notify_idle_ms;
        let panes: Vec<Arc<PaneInfo>> = model
            .panes
            .machines
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .iter()
            .filter_map(|pane| model.info(pane))
            .collect();
        let amb = model.ambient(now);
        let mut changed = false;
        let mut notices = Vec::new();
        for info in panes {
            let before = model.panes.pane_state_mark(&info.id);
            let mut effects = model.panes.poll_pane(&info, idle_ms, &amb);
            if model.panes.trust_look_due(&info.id, now) {
                let screen = self
                    .0
                    .terminals
                    .screen(&info.id)
                    .unwrap_or_default()
                    .join("\n");
                effects.extend(model.panes.look_for_trust_question(&info, &screen, &amb));
            }
            changed |= model.panes.pane_state_mark(&info.id) != before;
            for effect in effects {
                if let Effect::Notify { title, body } = effect {
                    notices.push(News::Notify {
                        workspace: info.workspace_id.clone(),
                        pane: info.id.clone(),
                        title,
                        body,
                    });
                }
            }
        }
        drop(model);
        for notice in notices {
            let _ = self.0.news.send(notice);
        }
        if changed {
            self.live_changed();
        }
    }
}

/// Whether `published` names exactly the workspaces asleep now.
fn same_keys<V>(published: &HashSet<WsId>, now: &HashMap<WsId, V>) -> bool {
    published.len() == now.len() && now.keys().all(|ws| published.contains(ws))
}

/// Whether `published` says the deliveries that run now.
fn same_runs(published: &Delivering, now: &HashMap<WsId, HashMap<String, usize>>) -> bool {
    let running = now.iter().filter(|(_, tasks)| !tasks.is_empty());
    running.clone().count() == published.len()
        && running.into_iter().all(|(ws, tasks)| {
            published.get(ws).is_some_and(|said| {
                said.len() == tasks.len() && tasks.keys().all(|t| said.contains(t))
            })
        })
}

/// The claims no pane holds any more, released (a pane closed, or moved on
/// to another session).
fn release_unheld_sessions(model: &mut Model) {
    let held: HashSet<String> = model
        .store
        .state()
        .workspaces
        .iter()
        .flat_map(|ws| &ws.panes)
        .filter_map(|pane| pane.session_id.clone())
        .collect();
    let unheld: Vec<String> = model
        .panes
        .claims
        .ids()
        .filter(|id| !held.contains(*id))
        .map(str::to_owned)
        .collect();
    for id in unheld {
        model.panes.release_session(Some(&id));
    }
}

/// A control request's answer, given to core.
fn reply(id: String, answer: Result<Value, String>) {
    match answer {
        Ok(value) => hangar_core::control::control_reply(id, true, Some(value), None),
        Err(error) => hangar_core::control::control_reply(id, false, None, Some(error)),
    }
}

async fn follow_events(host: EngineHost, mut events: broadcast::Receiver<Event>) {
    loop {
        match events.recv().await {
            Ok(event) => host.feed(event),
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                eprintln!("the engine missed {missed} of core's events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Saves [`SAVE_DEBOUNCE`] after a change, the latest state each time; a
/// save that failed (the disk full) is tried again by itself, where the
/// newest state would otherwise wait for another change or the shutdown.
async fn save_loop(host: EngineHost, mut news: broadcast::Receiver<News>) {
    // What changed before the host started is saved first.
    let mut due = true;
    loop {
        if !due {
            match news.recv().await {
                Ok(News::Changed) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
            // Whatever else changes in the meantime is in the same save.
            tokio::time::sleep(SAVE_DEBOUNCE).await;
            while news.try_recv().is_ok() {}
        }
        due = false;
        if let Err(error) = host.save().await {
            eprintln!("the state could not be saved: {error}");
            tokio::time::sleep(SAVE_RETRY).await;
            due = true;
        }
    }
}

/// How long after a save failed it is tried again.
const SAVE_RETRY: Duration = Duration::from_secs(5);

/// `work` off the caller's thread: on tokio's blocking pool when there is
/// a runtime, on a thread of its own otherwise (a host edited before it is
/// started).
fn off_thread(work: impl FnOnce() + Send + 'static) {
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
            runtime.spawn_blocking(work);
        }
        Err(_) => {
            std::thread::spawn(work);
        }
    }
}

/// `action` applied to the model, the workspaces that are up and the
/// panes' index kept with it (`EngineHost::edit`, under the model's lock).
fn edited(model: &mut Model, action: StoreAction) -> StoreOutcome {
    let outcome = model.store.apply(action);
    if let Some(ws) = &outcome.workspace {
        model.opened.insert(ws.clone());
    }
    let live: HashSet<WsId> = model
        .store
        .state()
        .workspaces
        .iter()
        .map(|ws| ws.id.clone())
        .collect();
    model.opened.retain(|ws| live.contains(ws));
    model.index();
    outcome
}

/// The panes follow the state.
async fn panes_loop(host: EngineHost) {
    loop {
        host.reconcile();
        host.0.changed.notified().await;
    }
}

/// How often the panes' watchers are looked at.
const TICK: Duration = Duration::from_millis(crate::panes::activity::TICK_MS);

async fn tick_loop(host: EngineHost) {
    let mut every = tokio::time::interval(TICK);
    every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        every.tick().await;
        if host.0.stopping.load(Ordering::SeqCst) {
            break;
        }
        host.tick();
    }
}
