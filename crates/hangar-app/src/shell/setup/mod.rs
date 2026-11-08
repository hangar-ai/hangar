//! The Claude Code setup: what the window shows when Claude Code is not on
//! this machine, at start and whenever a workspace is about to be made.
//!
//! Hangar is made to run Claude Code, so the sheet stands in the way until it
//! is installed — one click, Anthropic's official installer, its output shown
//! as it runs ([`install`]) — and then offers to sign in. The one way past it
//! without installing is « Use Hangar with shells only », kept in the settings
//! (`shellsOnly`) and undone there; with it, the new-workspace dialog falls
//! back to a shell as it always did.
//!
//! The sheet is an overlay of the shell's (`Overlay::ClaudeSetup`), with no
//! close button and deaf to Escape. Its state is `Packages::setup`; its
//! messages ride in `shell::Action::Setup`.

pub mod install;
mod view;

#[cfg(test)]
mod tests;

use iced::Task;

use crate::message::{Intent, Message};
use crate::shell::Overlay;
use crate::state::types::AgentId;
use crate::state::{Model, Patch, StoreAction};

pub use view::view;

/// The setup's state.
#[derive(Debug, Default)]
pub struct State {
    /// Whether `claude_installed` has answered: before it does,
    /// `ui.claude_installed` is `false` for every machine.
    pub detected: bool,
    pub step: Step,
    /// The installer's output so far, its last [`install::KEPT_LINES`].
    pub output: Vec<String>,
    /// The sheet stood in the way of a new workspace: the dialog opens once
    /// it is done.
    pub then_create: bool,
    /// The install being run: what an earlier one still sends is ignored.
    run: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Step {
    /// What Claude Code is, and the button that installs it.
    #[default]
    Offer,
    /// The installer is running.
    Installing,
    /// It ran and is done: why it did not work.
    Failed(Failure),
    /// Installed: sign in, or done.
    SignIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The installer exited with this code.
    Exit(i32),
    /// A signal ended it.
    Stopped,
    /// It could not be started, for this reason.
    Start(String),
    /// It said it worked, and `claude` is still nowhere on the PATH.
    NotFound,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// « Install Claude Code », or « Try again ».
    Install,
    /// A line the installer of run `.0` printed.
    Output(u64, String),
    /// The installer of run `.0` ended.
    Exited(u64, install::Exit),
    /// After the install, the PATH asked again: whether `claude` is on it.
    Found(u64, bool),
    /// « Use Hangar with shells only ».
    ShellsOnly,
    /// « Sign in »: Claude Code in a pane, where it asks for the account.
    SignIn,
    /// « Done »: the sheet closes.
    Done,
    /// The install guide, in the browser.
    OpenDocs,
}

fn msg(msg: Msg) -> Message {
    Message::Shell(crate::shell::Action::Setup(msg))
}

/// Whether the setup stands in the way: Claude Code was looked for and not
/// found, and the person did not choose to go without it.
pub fn needed(st: &State, m: &Model) -> bool {
    st.detected && !m.ui.claude_installed && !crate::settings::shells_only(m.settings())
}

/// Whether the setup is the focused window's top overlay.
pub fn is_open(m: &Model) -> bool {
    m.ui.overlays.last() == Some(&Overlay::ClaudeSetup)
}

/// At start, once both the state and the detection are in: the sheet, when
/// it is needed and not up already.
pub fn offer_at_start(st: &State, m: &Model) -> Option<Intent> {
    (m.live.hydrated && needed(st, m) && !m.ui.overlays.contains(&Overlay::ClaudeSetup))
        .then_some(Intent::OpenOverlay(Overlay::ClaudeSetup))
}

/// Every intent passes here before the shell and the packages hear it: a new
/// workspace asked for while the setup is needed opens the setup instead,
/// and the dialog once it is done.
pub fn redirect(st: &mut State, m: &Model, intent: Intent) -> Intent {
    match intent {
        Intent::OpenOverlay(Overlay::WorkspaceDialog) if needed(st, m) => {
            st.then_create = true;
            Intent::OpenOverlay(Overlay::ClaudeSetup)
        }
        other => other,
    }
}

/// Whether the main account is logged in to Claude Code, as the accounts
/// package last read it (its watcher reads the files every few seconds, so
/// a login made in a pane shows here by itself).
pub fn signed_in(m: &Model) -> bool {
    m.live
        .accounts
        .accounts
        .iter()
        .flatten()
        .any(|account| account.is_main && account.logged_in)
}

pub fn update(st: &mut State, m: &mut Model, message: Msg) -> Task<Message> {
    match message {
        Msg::Install => {
            if st.step == Step::Installing {
                return Task::none();
            }
            st.run += 1;
            st.step = Step::Installing;
            st.output.clear();
            Task::run(install::run(install::chosen(), st.run), |m| m)
        }
        Msg::Output(run, line) => {
            if run == st.run {
                st.output.push(line);
                let over = st.output.len().saturating_sub(install::KEPT_LINES);
                st.output.drain(..over);
            }
            Task::none()
        }
        Msg::Exited(run, exit) => {
            if run != st.run || st.step != Step::Installing {
                return Task::none();
            }
            match exit {
                // It says it worked: look again, through a PATH asked again
                // of the login shell, which the installer may just have
                // taught where `claude` is.
                Ok(Some(0)) => Task::perform(redetect(), move |found| msg(Msg::Found(run, found))),
                Ok(Some(code)) => {
                    st.step = Step::Failed(Failure::Exit(code));
                    Task::none()
                }
                Ok(None) => {
                    st.step = Step::Failed(Failure::Stopped);
                    Task::none()
                }
                Err(error) => {
                    st.step = Step::Failed(Failure::Start(error));
                    Task::none()
                }
            }
        }
        Msg::Found(run, found) => {
            if run != st.run {
                return Task::none();
            }
            st.step = if found {
                m.ui.claude_installed = true;
                Step::SignIn
            } else {
                Step::Failed(Failure::NotFound)
            };
            Task::none()
        }
        Msg::ShellsOnly => {
            m.apply(StoreAction::UpdateSettings(Patch::new(|s| {
                crate::settings::set_shells_only(s, true)
            })));
            finish(st)
        }
        Msg::Done => finish(st),
        Msg::SignIn => {
            // The pane goes in the workspace on screen, when it is one of this
            // machine; else, or when a new workspace was on its way, that
            // workspace's own Claude Code pane is where the login happens.
            let here = m
                .active_workspace()
                .filter(|ws| ws.node.is_none())
                .map(|ws| ws.id.clone());
            match here {
                Some(ws) if !st.then_create => {
                    let created = m
                        .apply(StoreAction::AddPane {
                            ws: ws.clone(),
                            agent: Some(AgentId::Claude),
                            near: None,
                            dir: None,
                            cwd: None,
                            name: None,
                            init: None,
                        })
                        .pane;
                    let close = finish(st);
                    match created {
                        Some(pane) => close.chain(Task::done(Message::Intent(Intent::FocusPane {
                            ws,
                            pane,
                            terminal: true,
                        }))),
                        None => close,
                    }
                }
                _ => {
                    st.then_create = true;
                    finish(st)
                }
            }
        }
        Msg::OpenDocs => {
            if let Err(error) = hangar_core::opener::open_url(install::DOCS_URL) {
                eprintln!("the install guide could not be opened: {error}");
            }
            Task::none()
        }
    }
}

/// The sheet closes, and the new-workspace dialog opens when the sheet stood
/// in its way.
fn finish(st: &mut State) -> Task<Message> {
    // In turn, not batched: the sheet is closed before the dialog opens.
    closing(st).into_iter().fold(Task::none(), |done, intent| {
        done.chain(Task::done(Message::Intent(intent)))
    })
}

/// What [`finish`] sends, in order, the sheet's state back to its start.
fn closing(st: &mut State) -> Vec<Intent> {
    let then_create = std::mem::take(&mut st.then_create);
    st.step = Step::Offer;
    st.output.clear();
    let mut intents = vec![Intent::CloseOverlay];
    if then_create {
        intents.push(Intent::OpenOverlay(Overlay::WorkspaceDialog));
    }
    intents
}

/// The PATH repaired again (`path_env::refresh`), then `claude` looked for.
async fn redetect() -> bool {
    tokio::task::spawn_blocking(|| {
        hangar_core::path_env::refresh();
        hangar_core::sessions::claude_installed()
    })
    .await
    .unwrap_or(false)
}
