//! The engines' model writes, applied by a host with no window: the app's
//! `control/writes.rs` without what only a window does.
//!
//! A pane an agent opens or closes lays its workspace out again, as on the
//! desktop: the layout is part of the state a desktop that connects later
//! draws. Switching to a pane, moving a workspace to a window of its own and
//! running a palette command have no window to happen in: the first two are
//! refused, the palette's command too; showing a pane is nothing to do.

use super::{Model, News};
use crate::control::port::{CmdOut, ModelCmd};
use crate::panes::EpochMs;
use crate::state::{StoreAction, StoreOutcome};

/// Said to an agent that asks for what only a window does.
pub const NO_WINDOW: &str =
    "This Hangar runs with no window (a server): there is nothing to show, detach or bring back.";

pub(super) fn carry_out(
    model: &mut Model,
    cmd: ModelCmd,
    now: EpochMs,
) -> (Result<CmdOut, String>, Vec<News>) {
    let mut said = Vec::new();
    let outcome = match cmd {
        ModelCmd::AddPane {
            ws,
            agent,
            near,
            dir,
            cwd,
            name,
            init,
        } => {
            let outcome = model.store.apply(StoreAction::AddPane {
                ws: ws.clone(),
                agent,
                near,
                dir,
                cwd,
                name,
                init,
            });
            if outcome.pane.is_some() {
                model.store.apply(StoreAction::ArrangePanes(ws));
            }
            Ok(outcome)
        }
        ModelCmd::ClosePane { ws, pane } => {
            let count = |model: &Model| model.store.workspace(&ws).map_or(0, |w| w.panes.len());
            let before = count(model);
            let outcome = model.store.apply(StoreAction::ClosePane {
                ws: ws.clone(),
                pane,
            });
            if count(model) < before {
                model.store.apply(StoreAction::ArrangePanes(ws));
            }
            Ok(outcome)
        }
        ModelCmd::RespawnPane {
            ws,
            pane,
            patch,
            fresh,
        } => {
            let outcome = model.store.apply(StoreAction::RespawnPane {
                ws,
                pane: pane.clone(),
                patch,
            });
            // Restarted while its workspace slept: still owed its session,
            // unless the restart asked for a fresh one.
            if let Some(next) = &outcome.pane
                && !fresh
            {
                model.panes.carry_resume(&pane, next);
            }
            Ok(outcome)
        }
        ModelCmd::UpdatePane { ws, pane, patch } => Ok(model
            .store
            .apply(StoreAction::UpdatePane { ws, pane, patch })),
        ModelCmd::UpdateWorkspace { ws, patch } => {
            Ok(model.store.apply(StoreAction::UpdateWorkspace(ws, patch)))
        }
        ModelCmd::OpenWorkspace(ws) => {
            model.panes.mark_awake(&ws);
            if model.store.workspace(&ws).is_some() {
                model.opened.insert(ws);
            }
            Ok(StoreOutcome::default())
        }
        ModelCmd::Touch(ws) => {
            model.panes.hibernation.touch_workspace(&ws, now);
            Ok(StoreOutcome::default())
        }
        ModelCmd::Activate { .. } | ModelCmd::ViewsChanged { .. } => Ok(StoreOutcome::default()),
        ModelCmd::DevCommand { .. } | ModelCmd::Detach(_) | ModelCmd::Reattach(_) => {
            Err(NO_WINDOW.to_string())
        }
        ModelCmd::Announce(event) => {
            said.push(News::Announce(event));
            Ok(StoreOutcome::default())
        }
        ModelCmd::CreateWorkspace { draft, activate } => {
            let previous = model.store.state().active_workspace_id.clone();
            let outcome = model.store.apply(StoreAction::AddWorkspace(draft));
            if let Some(ws) = &outcome.workspace {
                model.opened.insert(ws.clone());
                if !activate {
                    model.store.apply(StoreAction::SetActiveWorkspace(previous));
                }
            }
            Ok(outcome)
        }
        ModelCmd::RemoveWorkspace(ws) => {
            let outcome = model.store.apply(StoreAction::RemoveWorkspace(ws.clone()));
            if model.store.workspace(&ws).is_none() {
                model.opened.remove(&ws);
                model.runs.remove(&ws);
            }
            Ok(outcome)
        }
        ModelCmd::BeginDelivery { ws, task } => {
            *model.runs.entry(ws).or_default().entry(task).or_default() += 1;
            Ok(StoreOutcome::default())
        }
        ModelCmd::EndDelivery { ws, task } => {
            if let Some(tasks) = model.runs.get_mut(&ws) {
                if let Some(count) = tasks.get_mut(&task) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        tasks.remove(&task);
                    }
                }
                if tasks.is_empty() {
                    model.runs.remove(&ws);
                }
            }
            Ok(StoreOutcome::default())
        }
    };
    (outcome, said)
}
