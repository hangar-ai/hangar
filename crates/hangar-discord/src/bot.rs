//! The connector: a bot on the person's server, beside an engine host
//! (discord-connector.md).
//!
//! It keeps the server's structure after the engine's state (a category per
//! folder, a channel per workspace with a webhook the agents speak through,
//! a pinned dashboard), follows the panes (the mod's reports: turns,
//! answers, what waits on the person; the orchestration's journal: workers
//! started and stopped, deliveries ready), and carries what the paired
//! person writes and clicks back through the desk, the chat view's own
//! calls (follow.rs, act.rs).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

use crate::hangar::Hangar;
use hangar_engine::live::{BridgeEvent, ToolPhase};
use hangar_engine::mobile::contract::ChatPending;
use hangar_engine::state::types::{AppState, Pane, PaneRole, Workspace};

use crate::cards;
use crate::config::{Layout, Place, Settings};
use crate::gateway::{self, Dispatch};
use crate::lanes::Lanes;
use crate::rest::{Http, Refused, Upload};
use crate::words::Words;

/// Where the bot reaches Discord, and where it keeps its files.
#[derive(Debug, Clone)]
pub struct Options {
    /// Discord's REST API (`rest::API`; a fake's in the tests).
    pub api: String,
    /// The gateway; `None`: asked of Discord (`GET /gateway/bot`).
    pub gateway: Option<String>,
    /// Where `discord.json` and `discord-layout.json` are.
    pub dir: PathBuf,
}

/// How often the bot looks at what changed: turn cards edited, asks and
/// journals looked at, dashboards drawn again.
const TICK: Duration = Duration::from_millis(1_500);
/// A turn's card is edited at most this often.
pub(crate) const TURN_EDIT_DUE: Duration = Duration::from_secs(2);
/// A dashboard is drawn again at most this often.
const DASHBOARD_EDIT: Duration = Duration::from_secs(5);

/// A turn under way, its card in a channel or a thread.
#[derive(Debug, Clone)]
pub(crate) struct Turn {
    pub ws: String,
    pub thread: Option<String>,
    pub message: String,
    pub started: u64,
    pub calls: Vec<String>,
    pub edited: Instant,
    pub dirty: bool,
}

/// What the person wrote that waits in an agent's queue: their message on
/// Discord, and the queue's entry once core has it.
#[derive(Debug, Clone)]
pub(crate) struct Queued {
    pub ws: String,
    pub pane: String,
    pub channel: String,
    pub text: String,
    pub id: Option<String>,
}

/// A card waiting on the person, and what it asked.
#[derive(Debug, Clone)]
pub(crate) struct Ask {
    pub ws: String,
    pub message: String,
    pub key: String,
    pub pending: ChatPending,
    /// Answered from Discord: not to be marked answered elsewhere.
    pub settled: bool,
}

#[derive(Debug, Default)]
pub(crate) struct State {
    pub settings: Settings,
    pub layout: Layout,
    pub me: String,
    pub app: String,
    pub turns: HashMap<String, Turn>,
    pub asks: HashMap<String, Ask>,
    /// The wrong pairing codes sent since the bot started, by who sent them.
    pub pair_tries: HashMap<String, u32>,
    /// The journal's entries posted, by workspace: up to this moment.
    pub seen: HashMap<String, u64>,
    /// Deliveries with a card already, by task.
    pub delivered: HashSet<String>,
    /// By the Discord message.
    pub queued: HashMap<String, Queued>,
    /// Each pane's queue, as core last told it.
    pub queues: HashMap<String, Vec<hangar_core::bridge::Queued>>,
    /// Each dashboard's last body, and when it was drawn.
    pub drawn: HashMap<String, (String, Instant)>,
    /// How many reports each pane's mod made: what tells that a pane's
    /// ask may have changed.
    pub reports: HashMap<String, u64>,
    /// When each pane's ask was last looked at, and what stood then.
    pub looked: HashMap<String, Looked>,
    /// The structure is to be looked at again.
    pub dirty: bool,
    /// The state the structure was last looked at for.
    pub synced: Option<Arc<AppState>>,
}

pub(crate) struct Inner {
    pub host: Arc<dyn Hangar>,
    pub http: Http,
    pub dir: PathBuf,
    pub state: Mutex<State>,
    /// What waits on Discord, in order per agent, channel and workspace,
    /// side by side between them (lanes.rs).
    pub lanes: Lanes,
    /// Held while a file of the connector's is written: two lanes saving
    /// at once would write over each other's temporary file.
    pub saving: Mutex<()>,
}

/// A pane's ask, as it stood when last looked at (`Bot::asks_due`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Looked {
    pub reports: u64,
    pub waiting: bool,
    /// Since when this stands.
    pub since: Instant,
    pub at: Instant,
}

/// A card up is looked at again this often with nothing reported since.
pub(crate) const ASK_STANDS: Duration = Duration::from_secs(15);
/// An ask not found yet is looked for at each tick for this long…
pub(crate) const ASK_SOUGHT: Duration = Duration::from_secs(15);
/// …then this often.
pub(crate) const ASK_SOUGHT_LATER: Duration = Duration::from_secs(5);

/// The connector. Cloned freely.
#[derive(Clone)]
pub struct Bot(pub(crate) Arc<Inner>);

impl Bot {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn host(&self) -> &dyn Hangar {
        self.0.host.as_ref()
    }

    pub(crate) fn http(&self) -> &Http {
        &self.0.http
    }

    pub(crate) fn words(&self) -> Words {
        Words::of(self.host().snapshot().state.settings.locale)
    }

    pub(crate) fn owner(&self) -> Option<String> {
        self.state().settings.owner.clone()
    }

    pub(crate) fn guild(&self) -> String {
        self.state().settings.guild.clone()
    }

    pub(crate) fn save_layout(&self) {
        let _saving = self.0.saving.lock().unwrap_or_else(PoisonError::into_inner);
        let layout = self.state().layout.clone();
        if let Err(error) = layout.save(&self.0.dir) {
            eprintln!("discord: the layout could not be saved: {error}");
        }
    }

    pub(crate) fn save_settings(&self) {
        let _saving = self.0.saving.lock().unwrap_or_else(PoisonError::into_inner);
        let settings = self.state().settings.clone();
        if let Err(error) = settings.save(&self.0.dir) {
            eprintln!("discord: the settings could not be saved: {error}");
        }
    }

    /// A link to a channel or a thread of the server.
    pub(crate) fn link(&self, channel: &str) -> String {
        format!("https://discord.com/channels/{}/{channel}", self.guild())
    }

    /// The workspace's place on the server.
    pub(crate) fn place(&self, ws: &str) -> Option<Place> {
        self.state().layout.workspaces.get(ws).cloned()
    }

    /// Whether this Hangar acts on what happens in `channel`: one it made,
    /// or, when it leads, one no other Hangar of the bot made.
    pub(crate) fn handles(&self, channel: &str) -> bool {
        self.where_is(channel).is_some()
            || (self.host().leads() && !self.host().others().contains(channel))
    }

    /// The workspace a channel is, or the worker's pane a thread is.
    pub(crate) fn where_is(&self, channel: &str) -> Option<Where> {
        let state = self.state();
        if let Some((ws, _)) = state
            .layout
            .workspaces
            .iter()
            .find(|(_, place)| place.channel == channel)
        {
            return Some(Where::Workspace(ws.clone()));
        }
        state
            .layout
            .workers
            .iter()
            .find(|(_, worker)| worker.thread.as_deref() == Some(channel))
            .map(|(pane, worker)| Where::Thread {
                ws: worker.workspace.clone(),
                pane: pane.clone(),
            })
    }

    /// A message as the bot, in `channel`; its id.
    pub(crate) async fn post(
        &self,
        channel: &str,
        body: Value,
        files: Vec<Upload>,
    ) -> Result<String, Refused> {
        let sent = self
            .http()
            .upload(
                "POST",
                &format!("/channels/{channel}/messages"),
                body,
                files,
            )
            .await?;
        Ok(sent["id"].as_str().unwrap_or_default().to_string())
    }

    pub(crate) async fn edit(&self, channel: &str, message: &str, body: Value) {
        let edited = self
            .http()
            .call(
                "PATCH",
                &format!("/channels/{channel}/messages/{message}"),
                Some(body),
            )
            .await;
        if let Err(error) = edited {
            eprintln!("discord: a message could not be edited: {error}");
        }
    }

    /// A message under an agent's name, through the workspace's webhook, in
    /// its channel or one of its threads; its id.
    pub(crate) async fn post_as(
        &self,
        ws: &str,
        name: &str,
        thread: Option<&str>,
        mut body: Value,
        files: Vec<Upload>,
    ) -> Result<String, Refused> {
        let Some((id, token)) = self.place(ws).and_then(|place| place.webhook) else {
            return Err(Refused {
                status: 0,
                message: "the workspace has no webhook".into(),
            });
        };
        body["username"] = json!(cards::clip(&webhook_name(name), 80));
        body["avatar_url"] = json!(avatar(name));
        let thread = thread
            .map(|thread| format!("&thread_id={thread}"))
            .unwrap_or_default();
        let sent = self
            .http()
            .upload(
                "POST",
                &format!("/webhooks/{id}/{token}?wait=true&with_components=true{thread}"),
                body,
                files,
            )
            .await?;
        Ok(sent["id"].as_str().unwrap_or_default().to_string())
    }

    pub(crate) async fn edit_as(&self, ws: &str, thread: Option<&str>, message: &str, body: Value) {
        let Some((id, token)) = self.place(ws).and_then(|place| place.webhook) else {
            return;
        };
        let thread = thread
            .map(|thread| format!("&thread_id={thread}"))
            .unwrap_or_default();
        let edited = self
            .http()
            .call(
                "PATCH",
                &format!("/webhooks/{id}/{token}/messages/{message}?with_components=true{thread}"),
                Some(body),
            )
            .await;
        if let Err(error) = edited {
            eprintln!("discord: a card could not be edited: {error}");
        }
    }
}

/// What a channel of the server is to the bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Where {
    Workspace(String),
    Thread { ws: String, pane: String },
}

/// An agent's face: one of Discord's own default avatars, the same for a
/// name each time (nothing of the agent's goes elsewhere for it).
pub(crate) fn avatar(name: &str) -> String {
    let n = name.bytes().fold(0u32, |hash, b| {
        hash.wrapping_mul(31).wrapping_add(u32::from(b))
    }) % 6;
    format!("https://cdn.discordapp.com/embed/avatars/{n}.png")
}

/// Webhook names cannot hold `clyde` or `discord`.
fn webhook_name(name: &str) -> String {
    let mut out = name.to_string();
    for word in ["clyde", "Clyde", "CLYDE", "discord", "Discord", "DISCORD"] {
        out = out.replace(word, "·");
    }
    if out.trim().is_empty() {
        "Hangar".into()
    } else {
        out
    }
}

/// The pane a workspace's channel talks to: its orchestrator, else its
/// first Claude Code pane.
pub(crate) fn voice(ws: &Workspace) -> Option<&Pane> {
    ws.panes
        .iter()
        .find(|pane| pane.role == Some(PaneRole::Orchestrator))
        .or_else(|| {
            ws.panes
                .iter()
                .find(|pane| pane.agent == hangar_engine::state::types::AgentId::Claude)
        })
}

/// A pane and its workspace.
pub(crate) fn find<'a>(state: &'a AppState, pane: &str) -> Option<(&'a Workspace, &'a Pane)> {
    state.workspaces.iter().find_map(|ws| {
        ws.panes
            .iter()
            .find(|p| p.id == pane)
            .map(|found| (ws, found))
    })
}

/// A channel's name from a workspace's: lowercase letters, digits and
/// dashes, as Discord keeps them.
pub(crate) fn channel_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').chars().take(90).collect::<String>();
    if out.is_empty() {
        "workspace".into()
    } else {
        out
    }
}

/// What the bot gives itself on the categories it makes, so that it keeps
/// seeing them once channels it cannot view are hidden from it (November
/// 2026): view, send, history, files, reactions, webhooks, threads, pins.
pub const BOT_ALLOW: u64 = (1 << 4)
    | (1 << 6)
    | (1 << 10)
    | (1 << 11)
    | (1 << 14)
    | (1 << 15)
    | (1 << 16)
    | (1 << 29)
    | (1 << 35)
    | (1 << 38)
    | (1 << 51);

/// The guild's slash commands.
pub(crate) fn commands(words: Words) -> Value {
    let string = |name: &str, description: &str, required: bool| json!({ "type": 3, "name": name, "description": description, "required": required });
    let levels: Vec<Value> = ["low", "medium", "high", "xhigh", "max"]
        .iter()
        .map(|c| json!({ "name": c, "value": c }))
        .collect();
    json!([
        { "name": "stop", "description": words.command_stop(), "type": 1 },
        { "name": "model", "description": words.command_model(), "type": 1,
          "options": [string("model", "opus, sonnet, haiku…", true)] },
        { "name": "effort", "description": words.command_effort(), "type": 1,
          "options": [{ "type": 3, "name": "level", "description": "low · medium · high · xhigh · max", "required": true,
                        "choices": levels }] },
        { "name": "plan", "description": words.command_plan(), "type": 1 },
        { "name": "status", "description": words.command_status(), "type": 1 },
        { "name": "workspace", "description": words.command_workspace(), "type": 1,
          "options": [string("folder", "/srv/project", true), string("name", "Project", false)] },
    ])
}

/// Runs the connector beside `host` until the gateway refuses it for good
/// (the error) or the host stops.
pub async fn run(host: Arc<dyn Hangar>, options: Options) -> Result<(), String> {
    let mut settings = Settings::load(&options.dir)?;
    if !settings.ready() {
        return Err(
            "the Discord connector has no token or no server: run `hangar-node discord setup`"
                .into(),
        );
    }
    if let Some(code) = settings.pairing_code() {
        eprintln!("discord: not paired yet; send `pair {code}` to the bot on your server");
    }
    settings.save(&options.dir)?;
    let http = Http::new(&options.api, &settings.token);
    let me = http
        .call("GET", "/users/@me", None)
        .await
        .map_err(|err| format!("the bot's token: {err}"))?;
    let app = http
        .call("GET", "/oauth2/applications/@me", None)
        .await
        .map_err(|err| format!("the bot's application: {err}"))?;
    let gateway_url = match &options.gateway {
        Some(url) => url.clone(),
        None => http
            .call("GET", "/gateway/bot", None)
            .await
            .map_err(|err| format!("the gateway: {err}"))?["url"]
            .as_str()
            .unwrap_or("wss://gateway.discord.gg")
            .to_string(),
    };
    let layout = Layout::load(&options.dir)?;
    let token = settings.token.clone();
    let bot = Bot(Arc::new(Inner {
        host: host.clone(),
        http,
        dir: options.dir.clone(),
        lanes: Lanes::default(),
        saving: Mutex::new(()),
        state: Mutex::new(State {
            settings,
            layout,
            me: me["id"].as_str().unwrap_or_default().to_string(),
            app: app["id"].as_str().unwrap_or_default().to_string(),
            dirty: true,
            ..State::default()
        }),
    }));
    // What runs beside the loop ends with it (the set aborts its tasks when
    // dropped): the gateway's connection, and the task that hears core.
    let mut beside = tokio::task::JoinSet::new();
    // The gateway first: the events of the moments the structure takes to
    // make wait in the channel rather than go unheard.
    let (events, mut dispatches) = mpsc::unbounded_channel::<Dispatch>();
    beside.spawn(gateway::run(gateway_url, token, gateway::INTENTS, events));
    beside.spawn(hear(bot.clone(), host.events()));
    bot.register_commands().await;
    bot.mark_journals_seen();
    bot.sync().await;
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Nothing here waits on Discord: what does is given to a lane.
    loop {
        tokio::select! {
            ended = beside.join_next() => {
                return match ended {
                    Some(Ok(Err(why))) => Err(why),
                    Some(Err(error)) if error.is_panic() => Err(format!("the connector broke: {error}")),
                    _ => Ok(()),
                };
            }
            dispatch = dispatches.recv() => {
                let Some(dispatch) = dispatch else { return Ok(()) };
                bot.route(dispatch);
            }
            _ = tick.tick() => bot.tick(),
        }
    }
}

/// core's events, heard as they come and never waited on: each report of a
/// turn goes to its pane's lane, so a burst from many agents is neither
/// missed (core keeps a thousand events at most for who lags) nor slowed
/// by Discord.
async fn hear(bot: Bot, mut core: broadcast::Receiver<hangar_core::Event>) -> Result<(), String> {
    loop {
        match core.recv().await {
            Ok(hangar_core::Event::BridgeEvent { pane_id, event }) => {
                let Some(report) = BridgeEvent::parse(&event) else {
                    continue;
                };
                *bot.state().reports.entry(pane_id.clone()).or_default() += 1;
                if matches!(
                    report,
                    BridgeEvent::Turn(_) | BridgeEvent::Tool(ToolPhase::Start { .. })
                ) {
                    let (job, pane) = (bot.clone(), pane_id.clone());
                    bot.0
                        .lanes
                        .run(&pane_id, async move { job.on_bridge(&pane, report).await });
                }
            }
            Ok(hangar_core::Event::BridgeQueue(queue)) => {
                let job = bot.clone();
                bot.0.lanes.run(&queue.pane_id.clone(), async move {
                    job.on_queue(&queue.pane_id, &queue.pending).await
                });
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                eprintln!("discord: {missed} of core's events were missed");
            }
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

impl Bot {
    async fn register_commands(&self) {
        let (app, guild) = {
            let state = self.state();
            (state.app.clone(), state.settings.guild.clone())
        };
        let registered = self
            .http()
            .call(
                "PUT",
                &format!("/applications/{app}/guilds/{guild}/commands"),
                Some(commands(self.words())),
            )
            .await;
        if let Err(error) = registered {
            eprintln!("discord: the commands could not be registered: {error}");
        }
    }

    /// What the journals hold already is not news.
    fn mark_journals_seen(&self) {
        let snapshot = self.host().snapshot();
        let mut state = self.state();
        for ws in &snapshot.state.workspaces {
            let last = self
                .host()
                .journal(&ws.id)
                .last()
                .map(|e| e.at)
                .unwrap_or(0);
            state.seen.insert(ws.id.clone(), last);
        }
    }

    /// What the gateway dispatched, to where it is handled: what the
    /// person writes, edits or deletes in a channel in that channel's lane
    /// (two messages sent one after the other reach the agent in that
    /// order), a click or a command at once (Discord waits 3 s for its
    /// acknowledgement).
    fn route(&self, dispatch: Dispatch) {
        let bot = self.clone();
        match dispatch.kind.as_str() {
            "MESSAGE_CREATE" | "MESSAGE_UPDATE" | "MESSAGE_DELETE" => {
                let channel = dispatch.data["channel_id"].as_str().unwrap_or_default();
                self.0.lanes.run(&format!("channel:{channel}"), async move {
                    bot.on_dispatch(dispatch).await
                });
            }
            "INTERACTION_CREATE" => {
                tokio::spawn(async move { bot.on_dispatch(dispatch).await });
            }
            _ => {}
        }
    }

    /// `job` given to lane `key`, unless the same (`tag`) still waits there.
    fn give<F>(&self, key: &str, tag: &'static str, job: impl FnOnce(Bot) -> F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.0.lanes.run_once(key, tag, job(self.clone()));
    }

    /// What is due, given to the lanes: the structure after a change, the
    /// turn cards, the asks, the journals, the dashboards. Waits on nothing.
    pub(crate) fn tick(&self) {
        // The state changed since the structure was last looked at.
        let snapshot = self.host().snapshot();
        let dirty = {
            let mut state = self.state();
            let changed = state
                .synced
                .as_ref()
                .is_none_or(|synced| !Arc::ptr_eq(synced, &snapshot.state));
            if changed {
                state.synced = Some(snapshot.state.clone());
            }
            std::mem::take(&mut state.dirty) || changed
        };
        if dirty {
            self.give("structure", "sync", |bot| async move { bot.sync().await });
        }
        for pane in self.turns_due() {
            self.give(&pane.clone(), "turn", |bot| async move {
                bot.flush_turn(pane).await
            });
        }
        for (ws, pane) in self.asks_due() {
            self.give(&pane.clone(), "ask", |bot| async move {
                bot.check_ask(ws, pane).await
            });
        }
        for ws in &snapshot.state.workspaces {
            if self.place(&ws.id).is_none() {
                continue;
            }
            let lane = format!("workspace:{}", ws.id);
            let id = ws.id.clone();
            self.give(
                &lane,
                "journal",
                |bot| async move { bot.read_journal(id).await },
            );
            if self.dashboard_due(ws, &snapshot) {
                let id = ws.id.clone();
                self.give(&lane, "dashboard", |bot| async move {
                    bot.draw_dashboard(id).await
                });
            }
        }
    }

    /// A category per folder, a channel (with its webhook) per workspace:
    /// the categories one after the other (each is made once), the
    /// channels at once.
    pub(crate) async fn sync(&self) {
        let snapshot = self.host().snapshot();
        let state_now = snapshot.state.clone();
        let missing: Vec<&Workspace> = state_now
            .workspaces
            .iter()
            .filter(|ws| self.place(&ws.id).is_none())
            .collect();
        if missing.is_empty() {
            return;
        }
        let mut folders: Vec<String> = Vec::new();
        for ws in &missing {
            let folder = ws.folder_id.clone().unwrap_or_default();
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        for folder in folders {
            if let Err(error) = self.category_of(&state_now, &folder).await {
                eprintln!("discord: no category for the workspaces: {error}");
                return;
            }
        }
        let made = futures_util::future::join_all(
            missing.iter().map(|ws| self.make_place(&state_now, ws)),
        )
        .await;
        for (ws, place) in missing.iter().zip(made) {
            match place {
                Ok(place) => {
                    self.state().layout.workspaces.insert(ws.id.clone(), place);
                }
                Err(error) => {
                    eprintln!("discord: no channel for workspace {}: {error}", ws.name);
                }
            }
        }
        self.save_layout();
    }

    /// The category of folder `folder` (`""`: the workspaces in none), made
    /// when it is not there yet.
    async fn category_of(&self, state: &AppState, folder: &str) -> Result<String, Refused> {
        let known = self.state().layout.categories.get(folder).cloned();
        if let Some(category) = known {
            return Ok(category);
        }
        let name = state
            .folders
            .iter()
            .find(|f| f.id == folder)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "Hangar".into());
        // A node sharing the bot with its desktop: its categories say whose
        // they are.
        let name = match self.state().settings.label.clone() {
            Some(label) if !label.trim().is_empty() => format!("{name} · {label}"),
            _ => name,
        };
        let me = self.state().me.clone();
        let made = self
            .http()
            .call(
                "POST",
                &format!("/guilds/{}/channels", self.guild()),
                Some(json!({
                    "name": cards::clip(&name, 100),
                    "type": 4,
                    "permission_overwrites": [
                        { "id": me, "type": 1, "allow": BOT_ALLOW.to_string() },
                    ],
                })),
            )
            .await?;
        let id = made["id"].as_str().unwrap_or_default().to_string();
        self.state()
            .layout
            .categories
            .insert(folder.to_string(), id.clone());
        Ok(id)
    }

    async fn make_place(&self, state: &AppState, ws: &Workspace) -> Result<Place, Refused> {
        let folder = ws.folder_id.clone().unwrap_or_default();
        let category = self.category_of(state, &folder).await?;
        let channel = self
            .http()
            .call(
                "POST",
                &format!("/guilds/{}/channels", self.guild()),
                Some(json!({
                    "name": channel_name(&ws.name),
                    "type": 0,
                    "parent_id": category,
                    "topic": cards::clip(&ws.cwd, 1000),
                })),
            )
            .await?;
        let channel = channel["id"].as_str().unwrap_or_default().to_string();
        let hook = self
            .http()
            .call(
                "POST",
                &format!("/channels/{channel}/webhooks"),
                Some(json!({ "name": "Hangar" })),
            )
            .await?;
        let webhook = hook["id"]
            .as_str()
            .zip(hook["token"].as_str())
            .map(|(id, token)| (id.to_string(), token.to_string()));
        Ok(Place {
            channel,
            webhook,
            dashboard: None,
        })
    }

    /// Whether workspace `ws`'s dashboard shows something else than when
    /// it was last drawn, and may be drawn again (every 5 s at most).
    fn dashboard_due(&self, ws: &Workspace, snapshot: &hangar_engine::state::Snapshot) -> bool {
        let flat = self.dashboard_of(ws, snapshot, self.words()).to_string();
        match self.state().drawn.get(&ws.id) {
            Some((last, at)) => *last != flat && at.elapsed() >= DASHBOARD_EDIT,
            None => true,
        }
    }

    /// Workspace `ws`'s dashboard drawn: posted and pinned the first time,
    /// edited after. In the workspace's lane.
    pub(crate) async fn draw_dashboard(&self, ws: String) {
        let snapshot = self.host().snapshot();
        let Some(ws) = snapshot.state.workspaces.iter().find(|w| w.id == ws) else {
            return;
        };
        let Some(place) = self.place(&ws.id) else {
            return;
        };
        let body = self.dashboard_of(ws, &snapshot, self.words());
        self.state()
            .drawn
            .insert(ws.id.clone(), (body.to_string(), Instant::now()));
        match &place.dashboard {
            Some(message) => self.edit(&place.channel, message, body).await,
            None => match self.post(&place.channel, body, Vec::new()).await {
                Ok(message) => {
                    let _ = self
                        .http()
                        .call(
                            "PUT",
                            &format!("/channels/{}/messages/pins/{message}", place.channel),
                            None,
                        )
                        .await;
                    if let Some(place) = self.state().layout.workspaces.get_mut(&ws.id) {
                        place.dashboard = Some(message);
                    }
                    self.save_layout();
                }
                Err(error) => eprintln!("discord: no dashboard for {}: {error}", ws.name),
            },
        }
    }

    fn dashboard_of(
        &self,
        ws: &Workspace,
        snapshot: &hangar_engine::state::Snapshot,
        words: Words,
    ) -> Value {
        use hangar_engine::state::AgentActivity;
        let threads: HashMap<String, String> = self
            .state()
            .layout
            .workers
            .iter()
            .filter_map(|(pane, worker)| Some((pane.clone(), worker.thread.clone()?)))
            .collect();
        let links: HashMap<String, String> = threads
            .iter()
            .map(|(pane, thread)| (pane.clone(), self.link(thread)))
            .collect();
        let mut rows = Vec::new();
        for pane in &ws.panes {
            let activity = snapshot.activity.get(&pane.id);
            let state = match activity.map(|a| a.activity) {
                Some(AgentActivity::Working) => words.working(),
                Some(AgentActivity::Waiting) => words.waiting(),
                Some(AgentActivity::Yours) => words.done(),
                Some(AgentActivity::Idle) => words.idle(),
                None => words.starting(),
            };
            let worker = (pane.role == Some(PaneRole::Worker))
                .then(|| (pane.id.as_str(), links.get(&pane.id).map(String::as_str)));
            rows.push(cards::Row {
                name: &pane.name,
                state,
                since_secs: None,
                worker,
            });
        }
        let board = Some(self.host().tasks(&ws.id))
            .filter(|tasks| !tasks.is_empty())
            .map(|tasks| {
                let mut columns: Vec<(String, usize)> = Vec::new();
                for task in tasks {
                    match columns.iter_mut().find(|(c, _)| *c == task.column) {
                        Some((_, n)) => *n += 1,
                        None => columns.push((task.column.clone(), 1)),
                    }
                }
                columns
                    .into_iter()
                    .map(|(column, n)| format!("{column} {n}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        cards::dashboard(words, &ws.name, &rows, &board)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_is_named_after_its_workspace_as_discord_keeps_names() {
        assert_eq!(channel_name("My Shop (v2)"), "my-shop-v2");
        assert_eq!(channel_name("Éditeur"), "éditeur");
        assert_eq!(channel_name("!!!"), "workspace");
    }

    #[test]
    fn an_agent_keeps_its_face() {
        assert_eq!(avatar("Leo"), avatar("Leo"));
        assert!(avatar("Ada").starts_with("https://cdn.discordapp.com/embed/avatars/"));
        let faces: HashSet<String> = ["Leo", "Ada", "Bo", "Cy", "Max", "Ivy"]
            .iter()
            .map(|name| avatar(name))
            .collect();
        assert!(faces.len() > 1);
    }

    #[test]
    fn a_webhook_never_says_discord_or_clyde() {
        assert_eq!(webhook_name("Discord bot"), "· bot");
        assert_eq!(webhook_name("clyde"), "·");
        assert_eq!(webhook_name(""), "Hangar");
        assert_eq!(webhook_name("Leo"), "Leo");
    }
}
