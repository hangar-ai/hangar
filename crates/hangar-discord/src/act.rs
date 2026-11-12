//! What the person does on Discord, carried back to the panes through the
//! desk (the chat view's own calls): what they write in a workspace's
//! channel (to its orchestrator) or in a worker's thread (to that worker),
//! what they click on a card, what they type in a modal, and the slash
//! commands. Only the paired account is heard; until one is, `pair <code>`
//! pairs whoever sends it.
//!
//! A click is acknowledged at once (`DEFERRED_UPDATE_MESSAGE`, Discord
//! waits 3 s at most), then the card is edited with what was done; a
//! refusal is told to the one who clicked alone.

use serde_json::{Value, json};

use hangar_core::chat::{Answer, Body, Question};
use hangar_engine::mobile::contract::ChatPending;
use hangar_engine::mobile::desk;
use hangar_engine::state::AgentActivity;
use hangar_engine::state::types::{Pane, Workspace};

use crate::bot::{Bot, Where, find, voice};
use crate::cards::{self, EPHEMERAL};
use crate::config::Worker;
use crate::follow::now_secs;
use crate::gateway::Dispatch;
use crate::ids::Click;

/// Interaction callback types.
mod reply {
    pub const MESSAGE: u8 = 4;
    pub const DEFERRED_MESSAGE: u8 = 5;
    pub const DEFERRED_UPDATE: u8 = 6;
    pub const MODAL: u8 = 9;
}

/// ⏳ and ✅, as a reaction's path wants them.
const HOURGLASS: &str = "%E2%8F%B3";
const CHECK: &str = "%E2%9C%85";

/// Where an interaction answers.
struct Interaction {
    id: String,
    token: String,
    user: String,
    channel: String,
    message: Option<String>,
}

impl Interaction {
    fn of(data: &Value) -> Interaction {
        let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
        Interaction {
            id: text(&data["id"]),
            token: text(&data["token"]),
            user: data["member"]["user"]["id"]
                .as_str()
                .or(data["user"]["id"].as_str())
                .unwrap_or_default()
                .to_string(),
            channel: text(&data["channel_id"]),
            message: data["message"]["id"].as_str().map(str::to_string),
        }
    }
}

/// The values a modal sent, by the `custom_id` of each field.
fn modal_values(data: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    fn walk(value: &Value, out: &mut Vec<(String, Value)>) {
        if let Some(id) = value.get("custom_id").and_then(Value::as_str) {
            let given = value
                .get("values")
                .cloned()
                .or_else(|| value.get("value").cloned())
                .unwrap_or(Value::Null);
            out.push((id.to_string(), given));
        }
        for key in ["components", "component"] {
            match value.get(key) {
                Some(Value::Array(items)) => items.iter().for_each(|i| walk(i, out)),
                Some(item @ Value::Object(_)) => walk(item, out),
                _ => {}
            }
        }
    }
    if let Some(Value::Array(items)) = data.get("components") {
        items.iter().for_each(|item| walk(item, &mut out));
    }
    out
}

fn value_of<'a>(values: &'a [(String, Value)], id: &str) -> Option<&'a Value> {
    values.iter().find(|(key, _)| key == id).map(|(_, v)| v)
}

/// The answers of a modal of several questions: the options picked for
/// each, and the person's own words for those they picked nothing for (the
/// first question when they picked for all).
pub(crate) fn answers_of(questions: &[Question], values: &[(String, Value)]) -> Vec<Answer> {
    let other = value_of(values, "other")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    let mut answers: Vec<Answer> = questions
        .iter()
        .enumerate()
        .map(|(n, question)| {
            let picked = value_of(values, &format!("q{n}"))
                .and_then(Value::as_array)
                .map(|picked| {
                    picked
                        .iter()
                        .filter_map(|v| v.as_str()?.parse::<usize>().ok())
                        .filter_map(|i| question.options.get(i).map(|o| o.label.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Answer {
                selected: picked,
                other: None,
            }
        })
        .collect();
    if let Some(other) = other {
        let empty: Vec<usize> = answers
            .iter()
            .enumerate()
            .filter(|(_, a)| a.selected.is_empty())
            .map(|(n, _)| n)
            .collect();
        if empty.is_empty() {
            if let Some(first) = answers.first_mut() {
                first.other = Some(other);
            }
        } else {
            for n in empty {
                answers[n].other = Some(other.clone());
            }
        }
    }
    answers
}

impl Bot {
    pub(crate) async fn on_dispatch(&self, dispatch: Dispatch) {
        match dispatch.kind.as_str() {
            "MESSAGE_CREATE" => self.on_message(&dispatch.data).await,
            "INTERACTION_CREATE" => self.on_interaction(&dispatch.data).await,
            _ => {}
        }
    }

    /// The pane `channel` talks to, with its workspace.
    fn pane_of_channel(&self, channel: &str) -> Option<(Workspace, Pane)> {
        let snapshot = self.host().snapshot();
        match self.where_is(channel)? {
            Where::Workspace(ws) => {
                let ws = snapshot.state.workspaces.iter().find(|w| w.id == ws)?;
                let pane = voice(ws)?.clone();
                Some((ws.clone(), pane))
            }
            Where::Thread { pane, .. } => {
                let (ws, pane) = find(&snapshot.state, &pane)?;
                Some((ws.clone(), pane.clone()))
            }
        }
    }

    async fn react(&self, channel: &str, message: &str, emoji: &str) {
        let _ = self
            .http()
            .call(
                "PUT",
                &format!("/channels/{channel}/messages/{message}/reactions/{emoji}/@me"),
                None,
            )
            .await;
    }

    async fn say(&self, channel: &str, reply_to: Option<&str>, text: &str) {
        let mut body =
            json!({ "content": cards::clip(text, 1_900), "allowed_mentions": { "parse": [] } });
        if let Some(message) = reply_to {
            body["message_reference"] =
                json!({ "message_id": message, "fail_if_not_exists": false });
        }
        let _ = self
            .http()
            .call("POST", &format!("/channels/{channel}/messages"), Some(body))
            .await;
    }

    async fn on_message(&self, m: &Value) {
        if m["author"]["bot"] == true || m.get("webhook_id").is_some_and(|w| !w.is_null()) {
            return;
        }
        let text = m["content"].as_str().unwrap_or_default().trim().to_string();
        let author = m["author"]["id"].as_str().unwrap_or_default().to_string();
        let channel = m["channel_id"].as_str().unwrap_or_default().to_string();
        let message = m["id"].as_str().unwrap_or_default().to_string();
        let words = self.words();
        let (owner, code) = {
            let state = self.state();
            (state.settings.owner.clone(), state.settings.pairing.clone())
        };
        let Some(owner) = owner else {
            if let Some(code) = code
                && text.eq_ignore_ascii_case(&format!("pair {code}"))
            {
                {
                    let mut state = self.state();
                    state.settings.owner = Some(author);
                    state.settings.pairing = None;
                }
                self.save_settings();
                self.say(&channel, Some(&message), words.paired()).await;
            } else if self.where_is(&channel).is_some() {
                self.say(&channel, Some(&message), words.not_paired()).await;
            }
            return;
        };
        if author != owner || text.is_empty() {
            return;
        }
        let Some((ws, pane)) = self.pane_of_channel(&channel) else {
            if self.where_is(&channel).is_some() {
                self.say(&channel, Some(&message), words.no_orchestrator())
                    .await;
            }
            return;
        };
        let Some(desk_) = self.host().desk() else {
            return;
        };
        let port = self.host().port();
        let working = self
            .host()
            .snapshot()
            .activity
            .get(&pane.id)
            .is_some_and(|a| a.activity == AgentActivity::Working);
        let sent = if working {
            desk::queue(&port, &ws.id, &pane.id, &text).await
        } else {
            desk::send_text(&port, &desk_, &ws.id, &pane.id, &text).await
        };
        match sent {
            Ok(sent) if sent.queued == Some(true) || working => {
                self.react(&channel, &message, HOURGLASS).await
            }
            Ok(_) => self.react(&channel, &message, CHECK).await,
            Err(error) => self.say(&channel, Some(&message), &error).await,
        }
    }

    async fn respond(&self, i: &Interaction, kind: u8, data: Option<Value>) {
        let mut body = json!({ "type": kind });
        if let Some(data) = data {
            body["data"] = data;
        }
        let answered = self
            .http()
            .call(
                "POST",
                &format!("/interactions/{}/{}/callback", i.id, i.token),
                Some(body),
            )
            .await;
        if let Err(error) = answered {
            eprintln!("discord: an interaction's answer: {error}");
        }
    }

    /// The message the interaction came from (or its deferred answer), as
    /// `body` now.
    async fn edit_original(&self, i: &Interaction, body: Value) {
        let app = self.state().app.clone();
        let _ = self
            .http()
            .call(
                "PATCH",
                &format!("/webhooks/{app}/{}/messages/@original", i.token),
                Some(body),
            )
            .await;
    }

    /// Told to the one who clicked alone.
    async fn whisper(&self, i: &Interaction, text: &str) {
        let app = self.state().app.clone();
        let _ = self
            .http()
            .call(
                "POST",
                &format!("/webhooks/{app}/{}", i.token),
                Some(json!({ "content": cards::clip(text, 1_900), "flags": EPHEMERAL })),
            )
            .await;
    }

    async fn on_interaction(&self, data: &Value) {
        let i = Interaction::of(data);
        let words = self.words();
        let owner = self.owner();
        match owner {
            Some(owner) if owner == i.user => {}
            Some(_) => {
                let said = json!({ "content": words.only_owner(), "flags": EPHEMERAL });
                return self.respond(&i, reply::MESSAGE, Some(said)).await;
            }
            None => {
                let said = json!({ "content": words.not_paired(), "flags": EPHEMERAL });
                return self.respond(&i, reply::MESSAGE, Some(said)).await;
            }
        }
        match data["type"].as_u64() {
            Some(2) => self.on_command(&i, &data["data"]).await,
            Some(3) => {
                let Some(click) = data["data"]["custom_id"].as_str().and_then(Click::parse) else {
                    return;
                };
                self.on_click(&i, click).await;
            }
            Some(5) => {
                let Some(click) = data["data"]["custom_id"].as_str().and_then(Click::parse) else {
                    return;
                };
                let values = modal_values(&data["data"]);
                self.on_modal(&i, click, values).await;
            }
            _ => {}
        }
    }

    /// The pane `pane` and its workspace, as they stand.
    fn located(&self, pane: &str) -> Option<(Workspace, Pane)> {
        let snapshot = self.host().snapshot();
        let (ws, found) = find(&snapshot.state, pane)?;
        Some((ws.clone(), found.clone()))
    }

    /// The ask posted for `pane`, when it is still `kind`'s.
    fn ask_of(&self, pane: &str) -> Option<crate::bot::Ask> {
        self.state().asks.get(pane).cloned()
    }

    async fn settled_here(&self, i: &Interaction, pane: &str, said: &str) {
        let ask = {
            let mut state = self.state();
            state.asks.get_mut(pane).map(|ask| {
                ask.settled = true;
                ask.clone()
            })
        };
        let card = cards::settled(said, Some(&i.user), now_secs(), self.words());
        match ask {
            Some(ask) if Some(&ask.message) == i.message.as_ref() => {
                self.edit_original(i, card).await;
            }
            Some(ask) => self.settle(&ask, said, Some(&i.user)).await,
            None => self.edit_original(i, card).await,
        }
    }

    async fn on_click(&self, i: &Interaction, click: Click) {
        let words = self.words();
        let port = self.host().port();
        let Some(desk_) = self.host().desk() else {
            return;
        };
        match click {
            Click::Other { pane } => {
                let question = match self.ask_of(&pane).map(|a| a.pending) {
                    Some(ChatPending::Question { questions, .. }) => questions.first().cloned(),
                    _ => None,
                };
                match question {
                    Some(question) => {
                        let modal = cards::other_modal(words, &question, &pane);
                        self.respond(i, reply::MODAL, Some(modal)).await
                    }
                    None => self.gone(i).await,
                }
            }
            Click::AnswerAll { pane } => match self.ask_of(&pane).map(|a| a.pending) {
                Some(ChatPending::Question { questions, .. }) => {
                    let modal = cards::questions_modal(words, &questions, &pane);
                    self.respond(i, reply::MODAL, Some(modal)).await
                }
                _ => self.gone(i).await,
            },
            Click::Plan {
                pane,
                approve: false,
            } => {
                let modal = cards::plan_modal(words, &pane);
                self.respond(i, reply::MODAL, Some(modal)).await
            }
            Click::Deliver {
                ws,
                task,
                land: false,
            } => {
                let modal = cards::send_back_modal(words, &ws, &task);
                self.respond(i, reply::MODAL, Some(modal)).await
            }
            Click::Permit { pane, allow } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                let Some((ws, found)) = self.located(&pane) else {
                    return self.whisper(i, "This pane is gone.").await;
                };
                let prompt = match self.ask_of(&pane).map(|a| a.pending) {
                    Some(ChatPending::Permission { prompt, .. }) => Some(prompt),
                    _ => None,
                };
                let done =
                    desk::permit(&port, &desk_, &ws.id, &found.id, allow, prompt.as_deref()).await;
                match done {
                    Ok(_) => {
                        let said = if allow {
                            words.allowed()
                        } else {
                            words.denied()
                        };
                        self.settled_here(i, &pane, said).await
                    }
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Choose { pane, option } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                let Some(ChatPending::Question {
                    tool_use_id,
                    questions,
                }) = self.ask_of(&pane).map(|a| a.pending)
                else {
                    return self.gone(i).await;
                };
                let Some(label) = questions
                    .first()
                    .and_then(|q| q.options.get(option))
                    .map(|o| o.label.clone())
                else {
                    return self.gone(i).await;
                };
                let Some((ws, found)) = self.located(&pane) else {
                    return self.gone(i).await;
                };
                let answers = [Answer {
                    selected: vec![label.clone()],
                    other: None,
                }];
                match desk::answer(&port, &desk_, &ws.id, &found.id, &tool_use_id, &answers).await {
                    Ok(_) => {
                        let said = format!("{} : {label}", words.answered());
                        self.settled_here(i, &pane, &said).await
                    }
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Plan {
                pane,
                approve: true,
            } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                let Some((ws, found)) = self.located(&pane) else {
                    return self.gone(i).await;
                };
                let prompt = match self.ask_of(&pane).map(|a| a.pending) {
                    Some(ChatPending::Permission { prompt, .. }) => Some(prompt),
                    _ => None,
                };
                match desk::plan(
                    &port,
                    &desk_,
                    &ws.id,
                    &found.id,
                    true,
                    None,
                    prompt.as_deref(),
                )
                .await
                {
                    Ok(_) => self.settled_here(i, &pane, words.plan_approved()).await,
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Deliver {
                task, land: true, ..
            } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                let landed = hangar_engine::mobile::engine::run(
                    "remote_land",
                    &json!({ "task": task }),
                    &port,
                    &desk_,
                )
                .await;
                match landed {
                    Ok(_) => {
                        let card = cards::settled(words.landed(), Some(&i.user), now_secs(), words);
                        self.edit_original(i, card).await
                    }
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Thread { pane } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                match self.open_thread(&pane).await {
                    Ok(_) => {
                        // The dashboard shows the link at its next drawing.
                        self.state().drawn.clear();
                    }
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Stop { pane } => {
                self.respond(i, reply::DEFERRED_UPDATE, None).await;
                let Some((ws, found)) = self.located(&pane) else {
                    return self.gone(i).await;
                };
                if let Err(error) = desk::interrupt(&port, &desk_, &ws.id, &found.id).await {
                    self.whisper(i, &error).await;
                }
            }
        }
    }

    async fn gone(&self, i: &Interaction) {
        let card = cards::settled(
            self.words().answered_elsewhere(),
            None,
            now_secs(),
            self.words(),
        );
        self.edit_original(i, card).await;
    }

    async fn on_modal(&self, i: &Interaction, click: Click, values: Vec<(String, Value)>) {
        let words = self.words();
        let port = self.host().port();
        let Some(desk_) = self.host().desk() else {
            return;
        };
        self.respond(i, reply::DEFERRED_UPDATE, None).await;
        let text = |id: &str| {
            value_of(&values, id)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string()
        };
        match click {
            Click::Other { pane } | Click::AnswerAll { pane } => {
                let Some(ChatPending::Question {
                    tool_use_id,
                    questions,
                }) = self.ask_of(&pane).map(|a| a.pending)
                else {
                    return self.gone(i).await;
                };
                let Some((ws, found)) = self.located(&pane) else {
                    return self.gone(i).await;
                };
                let answers = answers_of(&questions, &values);
                match desk::answer(&port, &desk_, &ws.id, &found.id, &tool_use_id, &answers).await {
                    Ok(_) => self.settled_here(i, &pane, words.answered()).await,
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Plan { pane, .. } => {
                let Some((ws, found)) = self.located(&pane) else {
                    return self.gone(i).await;
                };
                let prompt = match self.ask_of(&pane).map(|a| a.pending) {
                    Some(ChatPending::Permission { prompt, .. }) => Some(prompt),
                    _ => None,
                };
                let feedback = text("text");
                match desk::plan(
                    &port,
                    &desk_,
                    &ws.id,
                    &found.id,
                    false,
                    Some(&feedback),
                    prompt.as_deref(),
                )
                .await
                {
                    Ok(_) => self.settled_here(i, &pane, words.plan_sent_back()).await,
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Deliver { task, .. } => {
                let reworked = hangar_engine::mobile::engine::run(
                    "remote_rework",
                    &json!({ "task": task, "text": text("text") }),
                    &port,
                    &desk_,
                )
                .await;
                match reworked {
                    Ok(_) => {
                        let card =
                            cards::settled(words.sent_back(), Some(&i.user), now_secs(), words);
                        self.edit_original(i, card).await
                    }
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            _ => {}
        }
    }

    async fn on_command(&self, i: &Interaction, data: &Value) {
        let words = self.words();
        self.respond(
            i,
            reply::DEFERRED_MESSAGE,
            Some(json!({ "flags": EPHEMERAL })),
        )
        .await;
        let option = |name: &str| {
            data["options"]
                .as_array()
                .and_then(|options| options.iter().find(|o| o["name"] == name))
                .and_then(|o| o["value"].as_str())
                .map(str::to_string)
        };
        let name = data["name"].as_str().unwrap_or_default();
        let said = if name == "workspace" {
            let folder = option("folder").unwrap_or_default();
            let named = option("name").unwrap_or_default();
            match crate::hangar::add_workspace(&self.host().port(), &named, &folder).await {
                Ok(ws) => {
                    self.sync().await;
                    match self.place(&ws) {
                        Some(place) => format!("{} : <#{}>", words.workspace_made(), place.channel),
                        None => words.workspace_made().to_string(),
                    }
                }
                Err(error) => error,
            }
        } else {
            self.on_pane_command(name, &i.channel, option).await
        };
        self.edit_original(i, json!({ "content": cards::clip(&said, 1_900) }))
            .await;
    }

    async fn on_pane_command(
        &self,
        name: &str,
        channel: &str,
        option: impl Fn(&str) -> Option<String>,
    ) -> String {
        let words = self.words();
        let Some((ws, pane)) = self.pane_of_channel(channel) else {
            return words.no_orchestrator().to_string();
        };
        let port = self.host().port();
        let Some(desk_) = self.host().desk() else {
            return String::new();
        };
        let tune = |command: String| {
            let port = port.clone();
            let (ws, pane) = (ws.id.clone(), pane.id.clone());
            async move { desk::queue(&port, &ws, &pane, &command).await.map(|_| ()) }
        };
        let done = match name {
            "stop" => desk::interrupt(&port, &desk_, &ws.id, &pane.id)
                .await
                .map(|_| ()),
            "model" => tune(format!("/model {}", option("model").unwrap_or_default())).await,
            "effort" => tune(format!("/effort {}", option("level").unwrap_or_default())).await,
            "plan" => tune("/plan".to_string()).await,
            "status" => return self.status_of(&ws),
            _ => return String::new(),
        };
        match done {
            Ok(()) => words.sent().to_string(),
            Err(error) => error,
        }
    }

    /// Where a workspace stands, in a few lines.
    fn status_of(&self, ws: &Workspace) -> String {
        let snapshot = self.host().snapshot();
        let words = self.words();
        let mut lines = vec![format!("**{}** — `{}`", ws.name, ws.cwd)];
        for pane in &ws.panes {
            let state = match snapshot.activity.get(&pane.id).map(|a| a.activity) {
                Some(AgentActivity::Working) => words.working(),
                Some(AgentActivity::Waiting) => words.waiting(),
                Some(AgentActivity::Yours) => words.done(),
                Some(AgentActivity::Idle) => words.idle(),
                None => words.starting(),
            };
            lines.push(format!("• {} — {state}", pane.name));
        }
        lines.join("\n")
    }

    /// The worker's thread: started on its line in the workspace's channel
    /// (or in the channel when the line is gone), filled with its work so
    /// far; its id. One open already is unarchived.
    pub(crate) async fn open_thread(&self, pane: &str) -> Result<String, String> {
        let words = self.words();
        let (ws, found) = self.located(pane).ok_or("This worker is gone.")?;
        let known = self.state().layout.workers.get(pane).cloned();
        let worker = match known {
            Some(worker) => worker,
            None => Worker {
                workspace: ws.id.clone(),
                name: found.name.clone(),
                task: found.task_id.clone().unwrap_or_default(),
                channel: self
                    .place(&ws.id)
                    .ok_or("No channel for this workspace.")?
                    .channel,
                line: None,
                thread: None,
            },
        };
        if let Some(thread) = &worker.thread {
            let _ = self
                .http()
                .call(
                    "PATCH",
                    &format!("/channels/{thread}"),
                    Some(json!({ "archived": false })),
                )
                .await;
            return Ok(thread.clone());
        }
        let name = cards::clip(&format!("{} · {}", worker.name, worker.task), 100);
        let made = match &worker.line {
            Some(line) => {
                self.http()
                    .call(
                        "POST",
                        &format!("/channels/{}/messages/{line}/threads", worker.channel),
                        Some(json!({ "name": name, "auto_archive_duration": 10080 })),
                    )
                    .await
            }
            None => {
                self.http()
                    .call(
                        "POST",
                        &format!("/channels/{}/threads", worker.channel),
                        Some(json!({ "name": name, "type": 11, "auto_archive_duration": 10080 })),
                    )
                    .await
            }
        }
        .map_err(|err| err.to_string())?;
        let thread = made["id"].as_str().unwrap_or_default().to_string();
        {
            let mut state = self.state();
            let mut worker = worker.clone();
            worker.thread = Some(thread.clone());
            state.layout.workers.insert(pane.to_string(), worker);
        }
        self.save_layout();
        let _ = self
            .post(&thread, cards::line(words.thread_intro()), Vec::new())
            .await;
        self.backfill(&ws, &found, &thread).await;
        if let Some(line) = &worker.line {
            let card = cards::worker_started(
                words,
                &worker.name,
                &worker.task,
                pane,
                Some(&self.link(&thread)),
            );
            self.edit(&worker.channel, line, card).await;
        }
        Ok(thread)
    }

    /// The worker's work so far, in its new thread: what it was told, what
    /// it answered, its calls counted between.
    async fn backfill(&self, ws: &Workspace, pane: &Pane, thread: &str) {
        let Some(desk_) = self.host().desk() else {
            return;
        };
        let port = self.host().port();
        let Ok(view) = desk::chat(&port, &desk_, &ws.id, &pane.id, None, Some(60)).await else {
            return;
        };
        let words = self.words();
        let mut calls = 0usize;
        for item in view.items {
            match item.body {
                Body::Action { .. } => calls += 1,
                Body::User { text, .. } => {
                    if calls > 0 {
                        let _ = self
                            .post(thread, cards::line(&words.actions(calls, 0)), Vec::new())
                            .await;
                        calls = 0;
                    }
                    let quoted: String = cards::clip(&text, 1_500)
                        .lines()
                        .map(|line| format!("> {line}\n"))
                        .collect();
                    let _ = self
                        .post(
                            thread,
                            cards::message(vec![cards::text(quoted)], true),
                            Vec::new(),
                        )
                        .await;
                }
                Body::Assistant { text } => {
                    for body in cards::answer(&text) {
                        let _ = self
                            .post_as(&ws.id, &pane.name, Some(thread), body, Vec::new())
                            .await;
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hangar_core::chat::QuestionOption;

    fn question(options: &[&str]) -> Question {
        Question {
            question: "?".into(),
            header: "H".into(),
            multi_select: true,
            options: options
                .iter()
                .map(|label| QuestionOption {
                    label: (*label).into(),
                    description: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_modal_s_values_become_one_answer_per_question() {
        let submitted = json!({
            "custom_id": "a:p1",
            "components": [
                { "type": 18, "component": { "type": 3, "custom_id": "q0", "values": ["1"] } },
                { "type": 18, "component": { "type": 3, "custom_id": "q1", "values": [] } },
                { "type": 18, "component": { "type": 4, "custom_id": "other", "value": "Both, really" } },
            ],
        });
        let values = modal_values(&submitted);
        let answers = answers_of(&[question(&["A", "B"]), question(&["C", "D"])], &values);
        assert_eq!(answers[0].selected, ["B"]);
        assert_eq!(answers[0].other, None);
        assert!(answers[1].selected.is_empty());
        assert_eq!(answers[1].other.as_deref(), Some("Both, really"));
    }

    #[test]
    fn the_person_s_words_go_to_the_first_question_when_all_were_picked() {
        let values = vec![
            ("q0".to_string(), json!(["0"])),
            ("other".to_string(), json!("and more")),
        ];
        let answers = answers_of(&[question(&["A"])], &values);
        assert_eq!(answers[0].selected, ["A"]);
        assert_eq!(answers[0].other.as_deref(), Some("and more"));
    }
}
