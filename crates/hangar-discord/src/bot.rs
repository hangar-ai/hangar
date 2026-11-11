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

use hangar_engine::host::{EngineHost, News};
use hangar_engine::mobile::contract::ChatPending;
use hangar_engine::state::types::{AppState, Pane, PaneRole, Workspace};

use crate::cards;
use crate::config::{Layout, Place, Settings};
use crate::gateway::{self, Dispatch};
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
    /// The journal's entries posted, by workspace: up to this moment.
    pub seen: HashMap<String, u64>,
    /// Deliveries with a card already, by task.
    pub delivered: HashSet<String>,
    /// Each dashboard's last body, and when it was drawn.
    pub drawn: HashMap<String, (String, Instant)>,
    /// The state changed since the structure was last looked at.
    pub dirty: bool,
}

pub(crate) struct Inner {
    pub host: EngineHost,
    pub http: Http,
    pub dir: PathBuf,
    pub state: Mutex<State>,
}

/// The connector. Cloned freely.
#[derive(Clone)]
pub struct Bot(pub(crate) Arc<Inner>);

impl Bot {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn host(&self) -> &EngineHost {
        &self.0.host
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
        let layout = self.state().layout.clone();
        if let Err(error) = layout.save(&self.0.dir) {
            eprintln!("discord: the layout could not be saved: {error}");
        }
    }

    pub(crate) fn save_settings(&self) {
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
pub async fn run(host: EngineHost, options: Options) -> Result<(), String> {
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
        state: Mutex::new(State {
            settings,
            layout,
            me: me["id"].as_str().unwrap_or_default().to_string(),
            app: app["id"].as_str().unwrap_or_default().to_string(),
            dirty: true,
            ..State::default()
        }),
    }));
    // The gateway first: the events of the moments the structure takes to
    // make wait in the channel rather than go unheard.
    let (events, mut dispatches) = mpsc::unbounded_channel::<Dispatch>();
    let (fatal_tx, mut fatal) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        if let Err(why) = gateway::run(gateway_url, token, gateway::INTENTS, events).await {
            let _ = fatal_tx.send(why);
        }
    });
    bot.register_commands().await;
    bot.mark_journals_seen();
    bot.sync().await;
    let mut core = host.core().subscribe();
    let mut news = host.news();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            why = fatal.recv() => {
                return Err(why.unwrap_or_else(|| "the gateway stopped".into()));
            }
            dispatch = dispatches.recv() => {
                let Some(dispatch) = dispatch else { return Ok(()) };
                let bot = bot.clone();
                tokio::spawn(async move { bot.on_dispatch(dispatch).await });
            }
            event = core.recv() => match event {
                Ok(hangar_core::Event::BridgeEvent { pane_id, event }) => {
                    bot.on_bridge(&pane_id, &event).await;
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
            said = news.recv() => match said {
                Ok(News::Changed) | Err(broadcast::error::RecvError::Lagged(_)) => {
                    bot.state().dirty = true;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
            _ = tick.tick() => bot.tick().await,
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
        let Some(runtime) = self.host().orchestration() else {
            return;
        };
        let snapshot = self.host().snapshot();
        let mut state = self.state();
        for ws in &snapshot.state.workspaces {
            let last = runtime.journal(&ws.id).last().map(|e| e.at).unwrap_or(0);
            state.seen.insert(ws.id.clone(), last);
        }
    }

    /// What is due: the structure after a change, the turn cards, the
    /// asks, the journals, the dashboards.
    pub(crate) async fn tick(&self) {
        let dirty = std::mem::take(&mut self.state().dirty);
        if dirty {
            self.sync().await;
        }
        self.flush_turns().await;
        self.check_asks().await;
        self.read_journals().await;
        self.draw_dashboards().await;
    }

    /// A category per folder, a channel (with its webhook) per workspace.
    pub(crate) async fn sync(&self) {
        let snapshot = self.host().snapshot();
        let state_now = snapshot.state.clone();
        for ws in &state_now.workspaces {
            if self.place(&ws.id).is_some() {
                continue;
            }
            match self.make_place(&state_now, ws).await {
                Ok(place) => {
                    self.state().layout.workspaces.insert(ws.id.clone(), place);
                    self.save_layout();
                }
                Err(error) => {
                    eprintln!("discord: no channel for workspace {}: {error}", ws.name);
                }
            }
        }
    }

    async fn make_place(&self, state: &AppState, ws: &Workspace) -> Result<Place, Refused> {
        let folder = ws.folder_id.clone().unwrap_or_default();
        let known = self.state().layout.categories.get(&folder).cloned();
        let category = match known {
            Some(category) => category,
            None => {
                let name = state
                    .folders
                    .iter()
                    .find(|f| f.id == folder)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "Hangar".into());
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
                self.state().layout.categories.insert(folder, id.clone());
                id
            }
        };
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

    /// Each workspace's dashboard, drawn again when what it shows changed.
    pub(crate) async fn draw_dashboards(&self) {
        let snapshot = self.host().snapshot();
        let words = self.words();
        for ws in &snapshot.state.workspaces {
            let Some(place) = self.place(&ws.id) else {
                continue;
            };
            let body = self.dashboard_of(ws, &snapshot, words);
            let flat = body.to_string();
            let due = {
                let state = self.state();
                match state.drawn.get(&ws.id) {
                    Some((last, at)) => *last != flat && at.elapsed() >= DASHBOARD_EDIT,
                    None => true,
                }
            };
            if !due {
                continue;
            }
            self.state()
                .drawn
                .insert(ws.id.clone(), (flat, Instant::now()));
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
        let board = self
            .host()
            .orchestration()
            .map(|runtime| runtime.tasks(&ws.id))
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
    fn a_webhook_never_says_discord_or_clyde() {
        assert_eq!(webhook_name("Discord bot"), "· bot");
        assert_eq!(webhook_name("clyde"), "·");
        assert_eq!(webhook_name(""), "Hangar");
        assert_eq!(webhook_name("Leo"), "Leo");
    }
}
