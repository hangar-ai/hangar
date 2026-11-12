//! What the bot follows of the panes: each turn of the agent a channel (or
//! a worker's thread) shows, as a card edited while it runs and folded when
//! it ends, Claude's answer read whole from the transcript; what waits on
//! the person (a permission, questions, a plan), as a card in the
//! workspace's channel; and the orchestration's journal: workers started
//! (with the button that opens their thread) and stopped, deliveries ready,
//! tasks that need the person.

use std::time::Instant;

use serde_json::{Value, json};

use hangar_core::chat::Body;
use hangar_engine::live::{BridgeEvent, ToolPhase, TurnPhase};
use hangar_engine::mobile::contract::ChatPending;
use hangar_engine::mobile::desk;
use hangar_engine::orchestration::events::OrchestratorEvent;
use hangar_engine::state::AgentActivity;
use hangar_engine::state::roles::READY_TO_LAND_LABEL;
use hangar_engine::state::types::PaneRole;

use crate::bot::{Ask, Bot, Turn, find, voice};
use crate::cards;
use crate::config::Worker;
use crate::rest::Upload;

/// Seconds since the epoch.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// One line for a call of the agent's: the tool and what it acts on.
pub(crate) fn describe(tool: &str, input: &Value) -> String {
    let field = |name: &str| input.get(name).and_then(Value::as_str).map(str::trim);
    let what = match tool {
        "Bash" => field("command"),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            field("file_path").or(field("notebook_path"))
        }
        "Grep" | "Glob" => field("pattern"),
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        "Task" | "Agent" => field("description"),
        _ => None,
    };
    match what.filter(|w| !w.is_empty()) {
        Some(what) => format!(
            "{tool} {}",
            cards::clip(what.lines().next().unwrap_or(what), 120)
        ),
        None => tool.to_string(),
    }
}

/// What a pending ask is known by: answering it again is refused unless it
/// is still the one asked.
pub(crate) fn ask_key(pending: &ChatPending) -> String {
    match pending {
        ChatPending::Question { tool_use_id, .. } => tool_use_id.clone(),
        ChatPending::Permission { prompt, .. } => prompt.clone(),
    }
}

impl Bot {
    /// Where pane `pane`'s conversation is shown: its workspace's channel
    /// when it is that channel's voice, its thread when one is open.
    fn shown_in(&self, pane: &str) -> Option<(String, Option<String>, String)> {
        let snapshot = self.host().snapshot();
        let (ws, found) = find(&snapshot.state, pane)?;
        self.place(&ws.id)?;
        if voice(ws).is_some_and(|v| v.id == pane) {
            return Some((ws.id.clone(), None, found.name.clone()));
        }
        let thread = self
            .state()
            .layout
            .workers
            .get(pane)
            .and_then(|worker| worker.thread.clone())?;
        Some((ws.id.clone(), Some(thread), found.name.clone()))
    }

    /// One of the mod's reports about a turn, in its pane's lane.
    pub(crate) async fn on_bridge(&self, pane: &str, event: BridgeEvent) {
        match event {
            BridgeEvent::Turn(TurnPhase::Start) => {
                let Some((ws, thread, name)) = self.shown_in(pane) else {
                    return;
                };
                let started = now_secs();
                let card = cards::turn(self.words(), &name, started, &[], pane);
                match self
                    .post_as(&ws, &name, thread.as_deref(), card, Vec::new())
                    .await
                {
                    Ok(message) => {
                        self.state().turns.insert(
                            pane.to_string(),
                            Turn {
                                ws,
                                thread,
                                message,
                                started,
                                calls: Vec::new(),
                                edited: Instant::now(),
                                dirty: false,
                            },
                        );
                    }
                    Err(error) => eprintln!("discord: a turn's card: {error}"),
                }
            }
            BridgeEvent::Tool(ToolPhase::Start { tool, input, .. }) => {
                if let Some(turn) = self.state().turns.get_mut(pane) {
                    turn.calls.push(describe(&tool, &input));
                    turn.dirty = true;
                }
            }
            BridgeEvent::Turn(TurnPhase::End { answer, .. }) => {
                let turn = self.state().turns.remove(pane);
                let Some((ws, thread, name)) = self.shown_in(pane) else {
                    return;
                };
                if let Some(turn) = &turn {
                    let minutes = now_secs().saturating_sub(turn.started).div_ceil(60);
                    let folded = cards::turn_folded(self.words(), &name, turn.calls.len(), minutes);
                    self.edit_as(&turn.ws, turn.thread.as_deref(), &turn.message, folded)
                        .await;
                }
                let whole = self.last_answer(&ws, pane).await.unwrap_or(answer);
                for body in cards::answer(&whole) {
                    if let Err(error) = self
                        .post_as(&ws, &name, thread.as_deref(), body, Vec::new())
                        .await
                    {
                        eprintln!("discord: an answer: {error}");
                    }
                }
            }
            _ => {}
        }
    }

    /// Claude's last answer, whole: what it wrote after the last thing it
    /// was told, read from the transcript.
    async fn last_answer(&self, ws: &str, pane: &str) -> Option<String> {
        let desk_ = self.host().desk()?;
        let port = self.host().port();
        let view = desk::chat(&port, &desk_, ws, pane, None, Some(40))
            .await
            .ok()?;
        let mut texts = Vec::new();
        for item in view.items.iter().rev() {
            match &item.body {
                Body::Assistant { text } => texts.push(text.clone()),
                Body::User { .. } => break,
                _ => {}
            }
        }
        texts.reverse();
        let whole = texts.join("\n\n");
        (!whole.trim().is_empty()).then_some(whole)
    }

    /// The panes whose turn card changed and is due an edit (at most every
    /// two seconds each).
    pub(crate) fn turns_due(&self) -> Vec<String> {
        self.state()
            .turns
            .iter()
            .filter(|(_, turn)| turn.dirty && turn.edited.elapsed() >= crate::bot::TURN_EDIT_DUE)
            .map(|(pane, _)| pane.clone())
            .collect()
    }

    /// The turn card of `pane` edited with its calls so far, in the pane's
    /// lane: never after the turn ended and its card was folded.
    pub(crate) async fn flush_turn(&self, pane: String) {
        let turn = {
            let mut state = self.state();
            let Some(turn) = state.turns.get_mut(&pane).filter(|turn| turn.dirty) else {
                return;
            };
            turn.dirty = false;
            turn.edited = Instant::now();
            turn.clone()
        };
        let snapshot = self.host().snapshot();
        let name = find(&snapshot.state, &pane)
            .map(|(_, p)| p.name.clone())
            .unwrap_or_default();
        let card = cards::turn(self.words(), &name, turn.started, &turn.calls, &pane);
        self.edit_as(&turn.ws, turn.thread.as_deref(), &turn.message, card)
            .await;
    }

    /// The panes whose ask is to be looked at now: those that wait on the
    /// person or have a card up, when their mod reported something since
    /// the last look (or they started or stopped waiting). Reading what a
    /// pane asks reads its transcript: a card up with nothing reported
    /// since still stands, and is looked at again once in a while; an ask
    /// not found yet is looked for at each tick for a while, then less
    /// often.
    pub(crate) fn asks_due(&self) -> Vec<(String, String)> {
        let snapshot = self.host().snapshot();
        let mut state = self.state();
        let now = Instant::now();
        let mut due = Vec::new();
        for ws in &snapshot.state.workspaces {
            if !state.layout.workspaces.contains_key(&ws.id) {
                continue;
            }
            for pane in &ws.panes {
                let waiting = snapshot
                    .activity
                    .get(&pane.id)
                    .is_some_and(|a| a.activity == AgentActivity::Waiting);
                let known = state.asks.contains_key(&pane.id);
                if !waiting && !known {
                    state.looked.remove(&pane.id);
                    continue;
                }
                let reports = state.reports.get(&pane.id).copied().unwrap_or(0);
                let since = match state.looked.get(&pane.id) {
                    Some(last) if last.reports == reports && last.waiting == waiting => {
                        let every = if known {
                            crate::bot::ASK_STANDS
                        } else if last.since.elapsed() < crate::bot::ASK_SOUGHT {
                            std::time::Duration::ZERO
                        } else {
                            crate::bot::ASK_SOUGHT_LATER
                        };
                        if last.at.elapsed() < every {
                            continue;
                        }
                        last.since
                    }
                    _ => now,
                };
                state.looked.insert(
                    pane.id.clone(),
                    crate::bot::Looked {
                        reports,
                        waiting,
                        since,
                        at: now,
                    },
                );
                due.push((ws.id.clone(), pane.id.clone()));
            }
        }
        due
    }

    /// What pane `pane` waits on the person for, as a card: posted when it
    /// starts waiting on something new, marked answered when it no longer
    /// does. In the pane's lane.
    pub(crate) async fn check_ask(&self, ws: String, pane: String) {
        let Some(desk_) = self.host().desk() else {
            return;
        };
        let port = self.host().port();
        let snapshot = self.host().snapshot();
        let Some((ws, pane)) = find(&snapshot.state, &pane).filter(|(found, _)| found.id == ws)
        else {
            return;
        };
        let words = self.words();
        let waiting = snapshot
            .activity
            .get(&pane.id)
            .is_some_and(|a| a.activity == AgentActivity::Waiting);
        let known = self.state().asks.get(&pane.id).cloned();
        let pending = if waiting {
            desk::pending_now(&port, &desk_, ws, pane)
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        match (pending, known) {
            (Some(pending), Some(known)) if ask_key(&pending) == known.key => {}
            (Some(pending), known) => {
                if let Some(known) = known.filter(|k| !k.settled) {
                    self.settle(&known, words.answered_elsewhere(), None).await;
                }
                self.post_ask(ws, pane, pending, self.owner().as_deref())
                    .await;
            }
            (None, Some(known)) => {
                self.state().asks.remove(&pane.id);
                if !known.settled {
                    self.settle(&known, words.answered_elsewhere(), None).await;
                }
            }
            (None, None) => {}
        }
    }

    async fn post_ask(
        &self,
        ws: &hangar_engine::state::types::Workspace,
        pane: &hangar_engine::state::types::Pane,
        pending: ChatPending,
        owner: Option<&str>,
    ) {
        let words = self.words();
        let mut files = Vec::new();
        let card = match &pending {
            ChatPending::Question { questions, .. } => {
                cards::questions(words, &pane.name, questions, &pane.id, owner)
            }
            ChatPending::Permission {
                plan: Some(plan), ..
            } => {
                let (card, file) = cards::plan(words, &pane.name, plan, &pane.id, owner);
                if let Some(file) = file {
                    files.push(Upload {
                        name: "plan.md".into(),
                        bytes: file.into_bytes(),
                    });
                }
                card
            }
            ChatPending::Permission { title, detail, .. } => {
                cards::permission(words, &pane.name, title, detail.as_deref(), &pane.id, owner)
            }
        };
        match self.post_as(&ws.id, &pane.name, None, card, files).await {
            Ok(message) => {
                self.state().asks.insert(
                    pane.id.clone(),
                    Ask {
                        ws: ws.id.clone(),
                        key: ask_key(&pending),
                        pending,
                        message,
                        settled: false,
                    },
                );
            }
            Err(error) => eprintln!("discord: a card for {}: {error}", pane.name),
        }
    }

    /// The card of `ask` in its settled form, no button left.
    pub(crate) async fn settle(&self, ask: &Ask, said: &str, by: Option<&str>) {
        let card = cards::settled(said, by, now_secs(), self.words());
        self.edit_as(&ask.ws, None, &ask.message, card).await;
    }

    /// The journal's entries of workspace `ws` since the last look, as
    /// lines and cards, in the workspace's lane.
    pub(crate) async fn read_journal(&self, ws: String) {
        let snapshot = self.host().snapshot();
        let Some(ws) = snapshot.state.workspaces.iter().find(|w| w.id == ws) else {
            return;
        };
        let Some(place) = self.place(&ws.id) else {
            return;
        };
        let seen = self.state().seen.get(&ws.id).copied().unwrap_or(0);
        let fresh: Vec<_> = self
            .host()
            .journal(&ws.id)
            .into_iter()
            .filter(|entry| entry.at > seen)
            .collect();
        if let Some(last) = fresh.last() {
            self.state().seen.insert(ws.id.clone(), last.at);
        }
        for entry in fresh {
            self.on_journal(ws, &place.channel, entry.event).await;
        }
    }

    async fn on_journal(
        &self,
        ws: &hangar_engine::state::types::Workspace,
        channel: &str,
        event: OrchestratorEvent,
    ) {
        let words = self.words();
        match event {
            OrchestratorEvent::WorkerStarted { worker, task_id } => {
                let Some(pane) = ws
                    .panes
                    .iter()
                    .find(|p| p.name == worker && p.role == Some(PaneRole::Worker))
                else {
                    return;
                };
                let task = task_id
                    .as_deref()
                    .and_then(|id| self.task_title(&ws.id, id))
                    .or(task_id.clone())
                    .unwrap_or_default();
                let card = cards::worker_started(words, &worker, &task, &pane.id, None);
                let line = self.post(channel, card, Vec::new()).await.ok();
                self.state().layout.workers.insert(
                    pane.id.clone(),
                    Worker {
                        workspace: ws.id.clone(),
                        name: worker,
                        task,
                        channel: channel.to_string(),
                        line,
                        thread: None,
                    },
                );
                self.save_layout();
            }
            OrchestratorEvent::WorkerStopped { worker, .. }
            | OrchestratorEvent::WorkerExited { worker, .. } => {
                let _ = self
                    .post(
                        channel,
                        cards::line(&format!("**{worker}** {}", words.stopped())),
                        Vec::new(),
                    )
                    .await;
                let ended: Vec<(String, Worker)> = {
                    let state = self.state();
                    state
                        .layout
                        .workers
                        .iter()
                        .filter(|(_, w)| w.workspace == ws.id && w.name == worker)
                        .map(|(pane, w)| (pane.clone(), w.clone()))
                        .collect()
                };
                for (pane, ended) in ended {
                    if let Some(thread) = &ended.thread {
                        let _ = self
                            .post(thread, cards::line(words.thread_ended()), Vec::new())
                            .await;
                        let _ = self
                            .http()
                            .call(
                                "PATCH",
                                &format!("/channels/{thread}"),
                                Some(json!({ "archived": true })),
                            )
                            .await;
                    }
                    self.state().layout.workers.remove(&pane);
                }
                self.save_layout();
            }
            OrchestratorEvent::TaskMoved { task_id, to, .. } if to == READY_TO_LAND_LABEL => {
                if !self.state().delivered.insert(task_id.clone()) {
                    return;
                }
                self.post_delivery(ws, channel, &task_id).await;
            }
            OrchestratorEvent::TaskNeedsYou { title, reason, .. } => {
                let owner = self.owner();
                let mention = owner
                    .as_deref()
                    .map(|id| format!("<@{id}> "))
                    .unwrap_or_default();
                let body = cards::calling(
                    vec![cards::text(format!(
                        "{mention}⚠️ **{}** — {}",
                        cards::clip(&title, 200),
                        cards::clip(&reason, 1_000)
                    ))],
                    owner.as_deref(),
                );
                let _ = self.post(channel, body, Vec::new()).await;
            }
            _ => {}
        }
    }

    fn task_title(&self, ws: &str, task: &str) -> Option<String> {
        self.host()
            .tasks(ws)
            .into_iter()
            .find(|t| t.id == task)
            .map(|t| t.title)
    }

    /// A delivery ready to land, as the desk reads it (`remote_delivery`).
    async fn post_delivery(
        &self,
        ws: &hangar_engine::state::types::Workspace,
        channel: &str,
        task: &str,
    ) {
        let Some(desk_) = self.host().desk() else {
            return;
        };
        let port = self.host().port();
        let read = hangar_engine::mobile::engine::run(
            "remote_delivery",
            &json!({ "task": task }),
            &port,
            &desk_,
        )
        .await;
        let d = match read {
            Ok(d) => d,
            Err(error) => {
                eprintln!("discord: the delivery of {task}: {error}");
                return;
            }
        };
        let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let files: Vec<(String, u32, u32)> = d["files"]
            .as_array()
            .map(|files| {
                files
                    .iter()
                    .map(|f| {
                        (
                            text(&f["path"]),
                            f["additions"].as_u64().unwrap_or(0) as u32,
                            f["deletions"].as_u64().unwrap_or(0) as u32,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let check = d.get("check").filter(|c| c.is_object()).map(|c| {
            format!(
                "{} `{}` ({:.0} s)",
                if c["passed"] == true { "✅" } else { "❌" },
                text(&c["command"]),
                c["seconds"].as_f64().unwrap_or(0.0)
            )
        });
        let review = d.get("review").filter(|r| r.is_object()).map(|r| {
            format!(
                "{} {}",
                if r["approved"] == true { "✅" } else { "❌" },
                text(&r["notes"])
            )
        });
        let (title, worker, branch, target, summary) = (
            text(&d["title"]),
            text(&d["worker"]),
            text(&d["branch"]),
            text(&d["target"]),
            d["summary"].as_str().map(str::to_string),
        );
        let card = cards::delivery(
            self.words(),
            &cards::Delivery {
                ws: &ws.id,
                task,
                title: &title,
                worker: &worker,
                branch: &branch,
                target: &target,
                files,
                more_files: d["moreFiles"] == true,
                summary: summary.as_deref(),
                check,
                review,
            },
            self.owner().as_deref(),
        );
        if let Err(error) = self.post(channel, card, Vec::new()).await {
            eprintln!("discord: a delivery's card: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_reads_as_its_tool_and_what_it_acts_on() {
        assert_eq!(
            describe("Bash", &json!({ "command": "npm test\n--watch" })),
            "Bash npm test"
        );
        assert_eq!(
            describe("Edit", &json!({ "file_path": "src/a.rs" })),
            "Edit src/a.rs"
        );
        assert_eq!(describe("TodoWrite", &json!({})), "TodoWrite");
    }
}
