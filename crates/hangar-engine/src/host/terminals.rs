//! The panes' shells for a host with no window: what the desktop's terminals
//! package does for its panes (spawn through core, type, read the screen,
//! kill), with an emulated screen per pane instead of a widget.
//!
//! [`Terminals`] is what the host asks; [`CorePtys`] answers it with core's
//! PTY manager on this machine. A node keeps its own panes (their history,
//! the desktop's streams) and answers it over them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hangar_core::pty::{self, PtyEvent, PtyManager};

use super::screen::Screen;
use crate::state::PaneId;

/// The size a pane's shell starts at, with no window to measure.
pub const COLUMNS: u16 = 120;
pub const ROWS: u16 = 40;

/// A pane's shell to start (`pty_spawn_with`'s arguments).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spawn {
    pub id: PaneId,
    pub cwd: String,
    pub name: String,
    /// The pane's agent (`claude`, `shell`).
    pub agent: String,
    /// The hangar-bridge mod is loaded into its Claude Code.
    pub bridge: bool,
    pub port_base: Option<u16>,
    pub build_env: Vec<(String, String)>,
    /// The `--mcp-config` file, handed as `HANGAR_MCP_CONFIG`.
    pub mcp_config: Option<String>,
    pub claude_account: Option<String>,
}

/// What a pane's shell tells the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    /// It printed something.
    Output(PaneId),
    /// It ended by itself, or was killed.
    Exit(PaneId),
}

/// Where a pane's events go: called on the pane's reader thread.
pub type TermSink = Arc<dyn Fn(TermEvent) + Send + Sync>;

pub trait Terminals: Send + Sync + 'static {
    /// Starts the shell; blocks while it starts.
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String>;
    fn write(&self, pane: &str, data: &str) -> Result<(), String>;
    fn alive(&self, pane: &str) -> bool;
    /// Its whole process tree.
    fn kill(&self, pane: &str) -> Result<(), String>;
    /// The last `n` lines it printed, oldest first; `None` for a pane this
    /// host does not run.
    fn lines(&self, pane: &str, n: usize) -> Option<Vec<String>>;
    /// The rows of its screen, without the history.
    fn screen(&self, pane: &str) -> Option<Vec<String>>;
    /// Its program asked for pastes to be bracketed.
    fn bracketed_paste(&self, pane: &str) -> bool {
        let _ = pane;
        false
    }
    /// Its program asked for the cursor keys' application sequences.
    fn application_cursor(&self, pane: &str) -> bool {
        let _ = pane;
        false
    }
    /// Every shell, on the way out.
    fn kill_all(&self);
}

/// Core's PTYs on this machine, each with a screen.
pub struct CorePtys {
    pty: Arc<PtyManager>,
    screens: Arc<Mutex<HashMap<PaneId, Screen>>>,
}

impl CorePtys {
    pub fn new(pty: Arc<PtyManager>) -> CorePtys {
        CorePtys {
            pty,
            screens: Arc::default(),
        }
    }

    fn screens(&self) -> MutexGuard<'_, HashMap<PaneId, Screen>> {
        self.screens.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Terminals for CorePtys {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        let id = spawn.id.clone();
        self.screens()
            .insert(id.clone(), Screen::new(COLUMNS, ROWS));
        let screens = self.screens.clone();
        let pty = self.pty.clone();
        let pane = id.clone();
        let output: pty::PtySink = Box::new(move |event| match event {
            PtyEvent::Output(output) => {
                let answers = screens
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_mut(&pane)
                    .map(|screen| screen.feed(output.data.as_bytes()))
                    .unwrap_or_default();
                if !answers.is_empty() {
                    // Off the reader's thread: writing waits on the pane's
                    // writer, which nothing here should hold up.
                    let (pty, pane) = (pty.clone(), pane.clone());
                    std::thread::spawn(move || {
                        for answer in answers {
                            let _ = pty::pty_write(&pty, pane.clone(), answer);
                        }
                    });
                }
                sink(TermEvent::Output(pane.clone()));
            }
            PtyEvent::Exit(_) => sink(TermEvent::Exit(pane.clone())),
        });
        let spawned = pty::pty_spawn_with(
            &self.pty,
            output,
            spawn.id,
            spawn.cwd,
            None,
            COLUMNS,
            ROWS,
            Some(spawn.name),
            Some(spawn.bridge),
            spawn.port_base,
            spawn.build_env,
            spawn.mcp_config,
            None,
            Some(spawn.agent),
            spawn.claude_account,
            false,
        );
        match spawned {
            Ok(_) => Ok(()),
            Err(error) => {
                self.screens().remove(&id);
                Err(error)
            }
        }
    }

    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        pty::pty_write(&self.pty, pane.to_string(), data.to_string())
    }

    fn alive(&self, pane: &str) -> bool {
        pty::pty_alive(&self.pty, pane.to_string())
    }

    fn kill(&self, pane: &str) -> Result<(), String> {
        let killed = pty::pty_kill(&self.pty, pane.to_string(), Some(true));
        self.screens().remove(pane);
        killed
    }

    fn lines(&self, pane: &str, n: usize) -> Option<Vec<String>> {
        self.screens().get(pane).map(|screen| screen.lines(n))
    }

    fn screen(&self, pane: &str) -> Option<Vec<String>> {
        self.screens().get(pane).map(Screen::rows)
    }

    fn bracketed_paste(&self, pane: &str) -> bool {
        self.screens()
            .get(pane)
            .is_some_and(Screen::bracketed_paste)
    }

    fn application_cursor(&self, pane: &str) -> bool {
        self.screens()
            .get(pane)
            .is_some_and(Screen::application_cursor)
    }

    fn kill_all(&self) {
        pty::kill_all(&self.pty);
        self.screens().clear();
    }
}
