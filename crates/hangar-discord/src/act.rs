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

use std::collections::HashSet;

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

/// The wrong pairing codes one person may send, and everyone together,
/// while Hangar's Discord connection runs.
const PAIR_TRIES: u32 = 5;
const PAIR_TRIES_ALL: u32 = 30;

/// What became of a try at the pairing code.
enum Pairing {
    Paired,
    Wrong,
    /// Too many wrong codes were sent: nothing pairs any more.
    Closed,
    /// From someone who tried too often: not answered.
    Unheard,
}

/// What a card clicked is to its pane's ask.
enum Clicked {
    /// The card of what the pane asks now, not answered yet.
    Current(crate::bot::Ask),
    /// That card, answered from Discord a moment ago: nothing more to do.
    Settled,
    /// The card of an ask that no longer stands.
    Stale,
}

/// What is said on Discord of a modal, once its work is done in Hangar.
enum After {
    /// The pane's ask is settled, in these words.
    Settled(String, String),
    /// The message the modal came from becomes this card.
    Card(Value),
    /// To the person alone.
    Whisper(String),
    /// What was asked no longer stands.
    Gone,
    Nothing,
}

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
            "MESSAGE_DELETE" => self.on_deleted(&dispatch.data).await,
            "MESSAGE_UPDATE" => self.on_edited(&dispatch.data).await,
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
        if !self.handles(&channel) {
            return;
        }
        let words = self.words();
        let (owner, code, guild) = {
            let state = self.state();
            (
                state.settings.owner.clone(),
                state.settings.pairing.clone(),
                state.settings.guild.clone(),
            )
        };
        let Some(owner) = owner else {
            // The code is heard on the server Hangar was set up for alone
            // (not in a direct message, not on another server the bot was
            // invited to), and guessed a few times at most.
            let here = m["guild_id"].as_str() == Some(guild.as_str());
            let tried = text
                .get(..5)
                .filter(|pair| pair.eq_ignore_ascii_case("pair "))
                .map(|_| text[5..].trim());
            match (code, tried) {
                (Some(code), Some(tried)) if here => {
                    let said = match self.try_pairing(&author, tried == code) {
                        Pairing::Paired => {
                            self.save_settings();
                            words.paired()
                        }
                        Pairing::Wrong => words.wrong_code(),
                        Pairing::Closed => words.pairing_closed(),
                        Pairing::Unheard => return,
                    };
                    self.say(&channel, Some(&message), said).await;
                }
                _ if self.where_is(&channel).is_some() => {
                    self.say(&channel, Some(&message), words.not_paired()).await;
                }
                _ => {}
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
        let snapshot_activity = self.host().snapshot().activity.get(&pane.id).cloned();
        let activity = snapshot_activity.as_ref().map(|a| a.activity);
        // An agent that waits on an answer has a question open in its
        // terminal: what is typed there, and the Enter after it, would
        // answer it (the first choice of a permission is "Yes").
        let asked = self.ask_of(&pane.id).is_some_and(|ask| !ask.settled);
        if activity == Some(AgentActivity::Waiting) || asked {
            let said = words.answer_first(&pane.name);
            return self.say(&channel, Some(&message), &said).await;
        }
        // Queued for the turn's end while the agent works, and behind what
        // already waits there (its mod takes the queue a moment after the
        // turn ends: typed in that moment, this would be read first). Only
        // where the mod is there to take it: any other agent is typed to,
        // and its own terminal holds what comes mid-turn.
        let bridged = snapshot_activity.as_ref().is_some_and(|a| a.bridged);
        let behind = self
            .state()
            .queues
            .get(&pane.id)
            .is_some_and(|waiting| !waiting.is_empty());
        let working = bridged && (activity == Some(AgentActivity::Working) || behind);
        let sent = if working {
            desk::queue(&port, &ws.id, &pane.id, &text).await
        } else {
            desk::send_text(&port, &desk_, &ws.id, &pane.id, &text).await
        };
        match sent {
            Ok(sent) if sent.queued == Some(true) || working => {
                self.state().queued.insert(
                    message.clone(),
                    crate::bot::Queued {
                        ws: ws.id.clone(),
                        pane: pane.id.clone(),
                        channel: channel.clone(),
                        text: text.clone(),
                        id: None,
                    },
                );
                // core may have told the queue before this was noted.
                let known = self.state().queues.get(&pane.id).cloned();
                if let Some(pending) = known {
                    self.match_queue(&pane.id, &pending);
                }
                self.react(&channel, &message, HOURGLASS).await
            }
            Ok(_) => self.react(&channel, &message, CHECK).await,
            Err(error) => self.say(&channel, Some(&message), &error).await,
        }
    }

    /// The agent's queue changed: what the person wrote finds its entry,
    /// and what left the queue was taken (⏳ becomes ✅).
    pub(crate) async fn on_queue(&self, pane: &str, pending: &[hangar_core::bridge::Queued]) {
        self.state()
            .queues
            .insert(pane.to_string(), pending.to_vec());
        let taken = self.match_queue(pane, pending);
        for (message, channel) in taken {
            let _ = self
                .http()
                .call(
                    "DELETE",
                    &format!("/channels/{channel}/messages/{message}/reactions/{HOURGLASS}/@me"),
                    None,
                )
                .await;
            self.react(&channel, &message, CHECK).await;
        }
    }

    /// What the person wrote, matched with the queue's entries; what left
    /// the queue, by its message and channel.
    fn match_queue(
        &self,
        pane: &str,
        pending: &[hangar_core::bridge::Queued],
    ) -> Vec<(String, String)> {
        let mut taken = Vec::new();
        {
            let mut state = self.state();
            let mut claimed: HashSet<String> =
                state.queued.values().filter_map(|q| q.id.clone()).collect();
            for (message, queued) in state.queued.iter_mut().filter(|(_, q)| q.pane == pane) {
                match &queued.id {
                    None => {
                        let found = pending.iter().find(|entry| {
                            entry.from.is_none()
                                && entry.text.trim() == queued.text.trim()
                                && !claimed.contains(&entry.id)
                        });
                        if let Some(entry) = found {
                            claimed.insert(entry.id.clone());
                            queued.id = Some(entry.id.clone());
                        }
                    }
                    Some(id) if !pending.iter().any(|entry| &entry.id == id) => {
                        taken.push((message.clone(), queued.channel.clone()));
                    }
                    Some(_) => {}
                }
            }
            for (message, _) in &taken {
                state.queued.remove(message);
            }
        }
        taken
    }

    /// A message the person deleted while it waited: taken back from the
    /// queue.
    async fn on_deleted(&self, data: &Value) {
        let message = data["id"].as_str().unwrap_or_default();
        let Some(queued) = self.state().queued.remove(message) else {
            return;
        };
        let Some(id) = queued.id else {
            return;
        };
        if let Err(error) = desk::unqueue(&self.host().port(), &queued.ws, &queued.pane, &id).await
        {
            eprintln!("discord: a deleted message stayed in the queue: {error}");
        }
    }

    /// A message the person edited while it waited: what waits is the new
    /// text.
    async fn on_edited(&self, data: &Value) {
        let message = data["id"].as_str().unwrap_or_default().to_string();
        let Some(text) = data["content"].as_str().map(str::trim).map(str::to_string) else {
            return;
        };
        let known = self.state().queued.get(&message).cloned();
        let Some(queued) = known else {
            return;
        };
        if text.is_empty() || text == queued.text {
            return;
        }
        let Some(id) = queued.id.clone() else {
            return;
        };
        // Taken meanwhile (the queue no longer holds it): the edit comes
        // too late, and queued again it would be read twice.
        let waits = self
            .state()
            .queues
            .get(&queued.pane)
            .is_some_and(|waiting| waiting.iter().any(|entry| entry.id == id));
        if !waits {
            return;
        }
        // Noted under its new text before the queue changes: the queue's
        // own telling of the old entry gone is not this message taken.
        self.state().queued.insert(
            message.clone(),
            crate::bot::Queued {
                text: text.clone(),
                id: None,
                ..queued.clone()
            },
        );
        let port = self.host().port();
        let requeued = match desk::unqueue(&port, &queued.ws, &queued.pane, &id).await {
            Ok(()) => desk::queue(&port, &queued.ws, &queued.pane, &text)
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        };
        if let Err(error) = requeued {
            eprintln!("discord: an edited message is not in the queue: {error}");
            self.state().queued.remove(&message);
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

    /// Acknowledges the click while `work` is done, and waits for both:
    /// what the person clicked reaches the agent without waiting on
    /// Discord's answer, and what is said of it afterwards (the card
    /// settled, an error whispered) finds the click acknowledged.
    async fn acknowledged<T>(&self, i: &Interaction, work: impl Future<Output = T>) -> T {
        let (_, done) = tokio::join!(self.respond(i, reply::DEFERRED_UPDATE, None), work);
        done
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
        // Another Hangar of the bot answers in its own channels.
        if !self.handles(&i.channel) {
            return;
        }
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
            // What reads as nothing of ours is acknowledged all the same:
            // Discord shows a failure on a click left without an answer.
            Some(3) => {
                let Some(click) = data["data"]["custom_id"].as_str().and_then(Click::parse) else {
                    return self.respond(&i, reply::DEFERRED_UPDATE, None).await;
                };
                self.on_click(&i, click).await;
            }
            Some(5) => {
                let Some(click) = data["data"]["custom_id"].as_str().and_then(Click::parse) else {
                    return self.respond(&i, reply::DEFERRED_UPDATE, None).await;
                };
                let values = modal_values(&data["data"]);
                self.on_modal(&i, click, values).await;
            }
            _ => {}
        }
    }

    /// One try at the pairing code by `author`: `right` pairs them. A
    /// person has [`PAIR_TRIES`] wrong tries, and everyone together
    /// [`PAIR_TRIES_ALL`]: past that, nothing pairs until Hangar's Discord
    /// connection is started again. A million codes are not guessed in
    /// thirty tries.
    fn try_pairing(&self, author: &str, right: bool) -> Pairing {
        let mut state = self.state();
        let all: u32 = state.pair_tries.values().sum();
        let mine = state.pair_tries.get(author).copied().unwrap_or(0);
        if all >= PAIR_TRIES_ALL {
            // Said once to each person, then not at all.
            return if mine > PAIR_TRIES {
                Pairing::Unheard
            } else {
                state.pair_tries.insert(author.to_string(), PAIR_TRIES + 1);
                Pairing::Closed
            };
        }
        if mine >= PAIR_TRIES {
            return Pairing::Unheard;
        }
        if right {
            state.settings.owner = Some(author.to_string());
            state.settings.pairing = None;
            state.pair_tries.clear();
            return Pairing::Paired;
        }
        state.pair_tries.insert(author.to_string(), mine + 1);
        Pairing::Wrong
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

    /// The ask the card clicked stands for: the one posted for `pane`, when
    /// the card is that ask's and nobody answered it yet. A card left from
    /// an earlier ask (answered on the PC, or posted before Hangar
    /// restarted) never answers what the pane asks now: the person is
    /// looking at another question. A modal names no card when Discord
    /// sends none: it was opened from one that was checked.
    fn ask_clicked(&self, i: &Interaction, pane: &str, modal: bool) -> Clicked {
        let Some(ask) = self.ask_of(pane) else {
            return Clicked::Stale;
        };
        let its_card = match i.message.as_deref() {
            Some(message) => message == ask.message,
            None => modal,
        };
        match (its_card, ask.settled) {
            (false, _) => Clicked::Stale,
            (true, true) => Clicked::Settled,
            (true, false) => Clicked::Current(ask),
        }
    }

    /// A click on a card that asks nothing any more: acknowledged, and a
    /// card left behind says so in place of its buttons.
    async fn outdated(&self, i: &Interaction, clicked: Clicked) {
        self.respond(i, reply::DEFERRED_UPDATE, None).await;
        if matches!(clicked, Clicked::Stale) {
            self.gone(i).await;
        }
    }

    async fn on_click(&self, i: &Interaction, click: Click) {
        let words = self.words();
        let port = self.host().port();
        let Some(desk_) = self.host().desk() else {
            return self.respond(i, reply::DEFERRED_UPDATE, None).await;
        };
        // An answer to an ask is the card's own ask's, or nothing is done.
        let pending = match &click {
            Click::Other { pane }
            | Click::AnswerAll { pane }
            | Click::Plan { pane, .. }
            | Click::Permit { pane, .. }
            | Click::Choose { pane, .. } => match self.ask_clicked(i, pane, false) {
                Clicked::Current(ask) => Some(ask.pending),
                other => return self.outdated(i, other).await,
            },
            Click::Deliver { .. } | Click::Thread { .. } | Click::Stop { .. } => None,
        };
        match click {
            Click::Other { pane } => {
                let question = match &pending {
                    Some(ChatPending::Question { questions, .. }) => questions.first().cloned(),
                    _ => None,
                };
                match question {
                    Some(question) => {
                        let modal = cards::other_modal(words, &question, &pane);
                        self.respond(i, reply::MODAL, Some(modal)).await
                    }
                    None => self.outdated(i, Clicked::Stale).await,
                }
            }
            Click::AnswerAll { pane } => match &pending {
                Some(ChatPending::Question { questions, .. }) => {
                    let modal = cards::questions_modal(words, questions, &pane);
                    self.respond(i, reply::MODAL, Some(modal)).await
                }
                _ => self.outdated(i, Clicked::Stale).await,
            },
            Click::Plan {
                pane,
                approve: false,
            } => match &pending {
                Some(ChatPending::Permission { .. }) => {
                    let modal = cards::plan_modal(words, &pane);
                    self.respond(i, reply::MODAL, Some(modal)).await
                }
                _ => self.outdated(i, Clicked::Stale).await,
            },
            Click::Deliver {
                ws,
                task,
                land: false,
            } => {
                let modal = cards::send_back_modal(words, &ws, &task);
                self.respond(i, reply::MODAL, Some(modal)).await
            }
            Click::Permit { pane, allow } => {
                let (Some(ChatPending::Permission { prompt, .. }), Some((ws, found))) =
                    (pending, self.located(&pane))
                else {
                    return self.outdated(i, Clicked::Stale).await;
                };
                let done = self
                    .acknowledged(
                        i,
                        desk::permit(&port, &desk_, &ws.id, &found.id, allow, Some(&prompt)),
                    )
                    .await;
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
                let asked = match pending {
                    Some(ChatPending::Question {
                        tool_use_id,
                        questions,
                    }) => questions
                        .first()
                        .and_then(|q| q.options.get(option))
                        .map(|o| (tool_use_id, o.label.clone())),
                    _ => None,
                };
                let (Some((tool_use_id, label)), Some((ws, found))) = (asked, self.located(&pane))
                else {
                    return self.outdated(i, Clicked::Stale).await;
                };
                let answers = [Answer {
                    selected: vec![label.clone()],
                    other: None,
                }];
                let answered = self
                    .acknowledged(
                        i,
                        desk::answer(&port, &desk_, &ws.id, &found.id, &tool_use_id, &answers),
                    )
                    .await;
                match answered {
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
                let (Some(ChatPending::Permission { prompt, .. }), Some((ws, found))) =
                    (pending, self.located(&pane))
                else {
                    return self.outdated(i, Clicked::Stale).await;
                };
                let approved = self
                    .acknowledged(
                        i,
                        desk::plan(&port, &desk_, &ws.id, &found.id, true, None, Some(&prompt)),
                    )
                    .await;
                match approved {
                    Ok(_) => self.settled_here(i, &pane, words.plan_approved()).await,
                    Err(error) => self.whisper(i, &error).await,
                }
            }
            Click::Deliver {
                task, land: true, ..
            } => {
                let landing = json!({ "task": task });
                let landed = self
                    .acknowledged(
                        i,
                        hangar_engine::mobile::engine::run("remote_land", &landing, &port, &desk_),
                    )
                    .await;
                match landed {
                    // Landed: the card says so, and by whom.
                    Ok(done) if done["landed"] == true => {
                        self.state().delivered.remove(&task);
                        let card = cards::settled(words.landed(), Some(&i.user), now_secs(), words);
                        self.edit_original(i, card).await
                    }
                    // Still landing (its checks run): the card says that,
                    // and what the journal tells of the landing later
                    // (landed, or why not) is said on this card.
                    Ok(_) => {
                        if let Some(message) = &i.message {
                            let card = Some((i.channel.clone(), message.clone()));
                            self.state().delivered.insert(task.clone(), card);
                        }
                        let card =
                            cards::settled(words.landing(), Some(&i.user), now_secs(), words);
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
                let Some((ws, found)) = self.located(&pane) else {
                    self.respond(i, reply::DEFERRED_UPDATE, None).await;
                    return self.gone(i).await;
                };
                let stopped = self
                    .acknowledged(i, desk::interrupt(&port, &desk_, &ws.id, &found.id))
                    .await;
                if let Err(error) = stopped {
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
        let Some(desk_) = self.host().desk() else {
            return self.respond(i, reply::DEFERRED_UPDATE, None).await;
        };
        let after = self
            .acknowledged(i, self.modal_done(i, click, values, &desk_))
            .await;
        match after {
            After::Settled(pane, said) => self.settled_here(i, &pane, &said).await,
            After::Card(card) => self.edit_original(i, card).await,
            After::Whisper(error) => self.whisper(i, &error).await,
            After::Gone => self.gone(i).await,
            After::Nothing => {}
        }
    }

    /// What a modal sent is done with, in Hangar; what to say of it on
    /// Discord comes back.
    async fn modal_done(
        &self,
        i: &Interaction,
        click: Click,
        values: Vec<(String, Value)>,
        desk_: &hangar_engine::mobile::engine::Mobile,
    ) -> After {
        let words = self.words();
        let port = self.host().port();
        let text = |id: &str| {
            value_of(&values, id)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string()
        };
        match click {
            Click::Other { pane } | Click::AnswerAll { pane } => {
                let ask = match self.ask_clicked(i, &pane, true) {
                    Clicked::Current(ask) => ask,
                    Clicked::Settled => return After::Nothing,
                    Clicked::Stale => return After::Gone,
                };
                let ChatPending::Question {
                    tool_use_id,
                    questions,
                } = ask.pending
                else {
                    return After::Gone;
                };
                let Some((ws, found)) = self.located(&pane) else {
                    return After::Gone;
                };
                let answers = answers_of(&questions, &values);
                match desk::answer(&port, desk_, &ws.id, &found.id, &tool_use_id, &answers).await {
                    Ok(_) => After::Settled(pane, words.answered().to_string()),
                    Err(error) => After::Whisper(error),
                }
            }
            Click::Plan { pane, .. } => {
                let ask = match self.ask_clicked(i, &pane, true) {
                    Clicked::Current(ask) => ask,
                    Clicked::Settled => return After::Nothing,
                    Clicked::Stale => return After::Gone,
                };
                let (ChatPending::Permission { prompt, .. }, Some((ws, found))) =
                    (ask.pending, self.located(&pane))
                else {
                    return After::Gone;
                };
                let feedback = text("text");
                match desk::plan(
                    &port,
                    desk_,
                    &ws.id,
                    &found.id,
                    false,
                    Some(&feedback),
                    Some(&prompt),
                )
                .await
                {
                    Ok(_) => After::Settled(pane, words.plan_sent_back().to_string()),
                    Err(error) => After::Whisper(error),
                }
            }
            Click::Deliver { task, .. } => {
                let reworked = hangar_engine::mobile::engine::run(
                    "remote_rework",
                    &json!({ "task": task, "text": text("text") }),
                    &port,
                    desk_,
                )
                .await;
                match reworked {
                    Ok(_) => After::Card(cards::settled(
                        words.sent_back(),
                        Some(&i.user),
                        now_secs(),
                        words,
                    )),
                    Err(error) => After::Whisper(error),
                }
            }
            _ => After::Nothing,
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
            let made = match self.host().make_workspace(&named, &folder) {
                Some(made) => made,
                None => crate::hangar::add_workspace(&self.host().port(), &named, &folder).await,
            };
            match made {
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
