//! The Discord connector, the window's side (docs/native/discord-connector.md):
//! the bot of `hangar-discord`, run beside Hangar Mobile, serving the
//! workspaces the phone reaches (local, and those this Hangar drives on its
//! nodes) through the same desk, port and orchestration runtime.
//!
//! It runs while the `discord-connector` experiment is on and its settings
//! are complete: the bot's token and the server's id, kept in the app's
//! data folder (`discord/discord.json`, the owner's alone), with who is
//! paired. Settings › Discord sets them up (`settings_page`). Every 2 s the
//! package reads them again and starts or stops the bot to match: the
//! pairing the bot writes shows on the page that way, and a token or a
//! server changed starts it again.
//!
//! **One bot for the nodes too.** The workspaces a node runs itself (a
//! Hangar with no window, hangar-node's own.rs) get their channels from the
//! same bot: once the person's account is paired, the desktop hands its
//! settings to every node it is connected to (`PUT /node/discord/shared`),
//! and each node's bot makes and answers its own channels, in categories
//! named after the node, the PC on or off. The desktop leaves the node's
//! channels to it (`GET /node/discord` says which) and answers everywhere
//! else; a node no desktop drives answers there in its place. A node set up
//! with a bot of its own keeps it.

mod page;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use iced::{Subscription, Task};
use tokio::sync::broadcast;

use hangar_core::board::Task as BoardTask;
use hangar_discord::Settings;
use hangar_engine::control::port::Port;
use hangar_engine::mobile::engine::Mobile;
use hangar_engine::orchestration::events::JournalEntry;

use crate::message::{Intent, Message};
use crate::orchestration::OrchestrationHandle;
use crate::services::{CoreEvent, Services};
use crate::settings::experiments;
use crate::state::Model;

pub use page::settings_page;

/// How often the settings are read again and the bot made to match.
const RECONCILE: Duration = Duration::from_secs(2);
/// The connected nodes are asked about their bot every this many of those
/// (10 s).
const NODES_EVERY: u32 = 5;

/// The package's UI state; lives in `App.pkg.discord`.
#[derive(Default)]
pub struct State {
    running: Option<Running>,
    /// The settings as last read.
    saved: Settings,
    form: Form,
    /// The channels the nodes' bots made, which the desktop's leaves to
    /// them; shared with the running bot.
    others: Arc<RwLock<HashSet<String>>>,
    /// What each node said of its bot, by node.
    nodes: HashMap<String, NodeBot>,
    /// What was handed to each node, by node: handed again when it changes.
    handed: HashMap<String, String>,
    /// How many times the settings were read again.
    reconciles: u32,
}

/// A node's bot, as `GET /node/discord` says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeBot {
    pub configured: bool,
    pub shared: bool,
    pub running: bool,
    pub channels: Vec<String>,
}

impl NodeBot {
    fn of(value: &serde_json::Value) -> NodeBot {
        NodeBot {
            configured: value["configured"] == true,
            shared: value["shared"] == true,
            running: value["running"] == true,
            channels: value["channels"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|c| c.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// A bot of the node's own, set up on the server: never replaced.
    fn its_own(&self) -> bool {
        self.configured && !self.shared
    }
}

/// The bot, and what it was started with.
struct Running {
    task: tokio::task::JoinHandle<()>,
    token: String,
    guild: String,
}

/// The setup form of Settings › Discord.
#[derive(Default)]
struct Form {
    token: String,
    guild: String,
    error: Option<hangar_i18n::Key>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("running", &self.running.is_some())
            .field("configured", &self.saved.ready())
            .field("paired", &self.saved.owner.is_some())
            .finish_non_exhaustive()
    }
}

impl State {
    /// The bot runs.
    pub fn running(&self) -> bool {
        self.running.as_ref().is_some_and(|r| !r.task.is_finished())
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// The settings read again, the bot started or stopped to match.
    Reconcile,
    Token(String),
    Guild(String),
    /// The form's token and server saved: the bot starts.
    Connect,
    /// The token forgotten: the bot stops.
    Disconnect,
    /// The invite link opened in the browser.
    Invite,
    /// What node `node` said of its bot; `None`: it could not say (an
    /// older node, or gone).
    NodeBot {
        node: String,
        bot: Option<NodeBot>,
    },
    /// The desktop's bot handed to node `node` (`handed`: what), or taken
    /// back from it (`None`).
    Handed {
        node: String,
        handed: Option<String>,
        ok: bool,
    },
}

/// Where the connector keeps its settings and the server's layout.
fn folder(sv: &Services) -> Option<PathBuf> {
    sv.core()
        .host()
        .paths()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("discord"))
}

/// What the bot reaches of the window's Hangar.
struct AppHangar {
    port: Port,
    desk: Arc<Mobile>,
    handle: OrchestrationHandle,
    core: hangar_core::Host,
    others: Arc<RwLock<HashSet<String>>>,
}

impl hangar_discord::Hangar for AppHangar {
    fn port(&self) -> Port {
        self.port.clone()
    }

    fn desk(&self) -> Option<Arc<Mobile>> {
        Some(self.desk.clone())
    }

    fn journal(&self, ws: &str) -> Vec<JournalEntry> {
        self.handle.journal(ws)
    }

    fn tasks(&self, ws: &str) -> Vec<BoardTask> {
        self.handle.tasks(ws)
    }

    fn events(&self) -> broadcast::Receiver<hangar_core::Event> {
        self.core.subscribe()
    }

    /// The nodes' channels are theirs.
    fn others(&self) -> HashSet<String> {
        self.others
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// What the desktop hands a node: its bot, its server, who is paired, and
/// the node's name for its categories. `None` until the person is paired.
pub fn handed(saved: &Settings, label: &str) -> Option<serde_json::Value> {
    let owner = saved.owner.as_ref().filter(|_| saved.ready())?;
    Some(serde_json::json!({
        "token": saved.token,
        "guild": saved.guild,
        "owner": owner,
        "label": label,
    }))
}

/// Whether the bot should run: the experiment on, the state in, the
/// settings complete.
pub fn wanted(experiment: bool, hydrated: bool, saved: &Settings) -> bool {
    experiment && hydrated && saved.ready()
}

pub fn update(st: &mut State, m: &mut Model, sv: &Services, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Reconcile => {
            reconcile(st, m, sv);
            // The nodes are asked through their tunnels: less often.
            st.reconciles = st.reconciles.wrapping_add(1);
            if st.reconciles % NODES_EVERY == 1 {
                return look_at_nodes(st, m);
            }
        }
        Msg::NodeBot { node, bot } => {
            match bot {
                Some(bot) => {
                    st.nodes.insert(node.clone(), bot);
                }
                None => {
                    st.nodes.remove(&node);
                }
            }
            publish_others(st);
            return hand_to(st, m, &node);
        }
        Msg::Handed { node, handed, ok } => match (handed, ok) {
            (Some(what), true) => {
                st.handed.insert(node, what);
            }
            (None, true) => {
                st.handed.remove(&node);
            }
            (_, false) => {}
        },
        Msg::Token(token) => {
            st.form.token = token;
            st.form.error = None;
        }
        Msg::Guild(guild) => {
            st.form.guild = guild;
            st.form.error = None;
        }
        Msg::Connect => {
            let token = st.form.token.trim().to_string();
            let guild = st.form.guild.trim().to_string();
            if token.is_empty() {
                st.form.error = Some(hangar_i18n::Key::DiscordBotErrorToken);
                return Task::none();
            }
            if !hangar_discord::setup::is_guild_id(&guild) {
                st.form.error = Some(hangar_i18n::Key::DiscordBotErrorGuild);
                return Task::none();
            }
            let Some(dir) = folder(sv) else {
                return Task::none();
            };
            let mut settings = Settings::load(&dir).unwrap_or_default();
            // Another server or another bot: whoever was paired is no longer.
            if settings.guild != guild || settings.token != token {
                settings.owner = None;
                settings.pairing = None;
            }
            settings.token = token;
            settings.guild = guild;
            settings.pairing_code();
            if let Err(error) = settings.save(&dir) {
                eprintln!("discord: the settings could not be saved: {error}");
                return Task::none();
            }
            st.form = Form::default();
            reconcile(st, m, sv);
        }
        Msg::Disconnect => {
            if let Some(dir) = folder(sv) {
                let mut settings = Settings::load(&dir).unwrap_or_default();
                settings.token.clear();
                if let Err(error) = settings.save(&dir) {
                    eprintln!("discord: the settings could not be saved: {error}");
                }
            }
            reconcile(st, m, sv);
        }
        Msg::Invite => {
            if let Some(bot) = hangar_discord::setup::bot_id(&st.saved.token) {
                let link = hangar_discord::setup::invite(&bot);
                if let Err(error) = hangar_core::opener::open_url(&link) {
                    eprintln!("discord: the invite link could not be opened: {error}");
                }
            }
        }
    }
    Task::none()
}

/// The settings read again; the bot started, started again or stopped to
/// match them.
fn reconcile(st: &mut State, m: &Model, sv: &Services) {
    let Some(dir) = folder(sv) else {
        return;
    };
    st.saved = Settings::load(&dir).unwrap_or_default();
    let experiment = experiments::enabled(m.settings(), experiments::DISCORD_CONNECTOR);
    let want = wanted(experiment, m.live.hydrated, &st.saved);
    let same = st.running.as_ref().is_some_and(|running| {
        running.token == st.saved.token
            && running.guild == st.saved.guild
            && !running.task.is_finished()
    });
    if want && same {
        return;
    }
    if let Some(running) = st.running.take() {
        running.task.abort();
    }
    if !want {
        return;
    }
    // What the bot acts through, once Hangar Mobile's side runs.
    let (Some(port), Some(desk), Some(handle)) = (
        m.live.control.port(),
        m.live.mobile.mobile(),
        m.live.orchestration.handle().cloned(),
    ) else {
        return;
    };
    let hangar = Arc::new(AppHangar {
        port,
        desk,
        handle,
        core: sv.core().host().clone(),
        others: st.others.clone(),
    });
    // A debug build may be pointed at a Discord of one's own
    // (`cargo run -p hangar-discord --example fake_discord`).
    let named = |var: &str| {
        cfg!(debug_assertions)
            .then(|| std::env::var(var).ok())
            .flatten()
            .filter(|v| !v.trim().is_empty())
    };
    let options = hangar_discord::Options {
        api: named("HANGAR_DISCORD_API").unwrap_or_else(|| hangar_discord::rest::API.into()),
        gateway: named("HANGAR_DISCORD_GATEWAY"),
        dir,
    };
    st.running = Some(Running {
        task: tokio::spawn(hangar_discord::keep_running(hangar, options)),
        token: st.saved.token.clone(),
        guild: st.saved.guild.clone(),
    });
}

/// The channels of the nodes whose bot is the desktop's, for the desktop's
/// to leave to them.
fn publish_others(st: &State) {
    let channels: HashSet<String> = st
        .nodes
        .values()
        .filter(|bot| bot.shared)
        .flat_map(|bot| bot.channels.iter().cloned())
        .collect();
    *st.others.write().unwrap_or_else(PoisonError::into_inner) = channels;
}

/// Each connected node asked about its bot.
fn look_at_nodes(st: &State, m: &Model) -> Task<Message> {
    let experiment = experiments::enabled(m.settings(), experiments::DISCORD_CONNECTOR);
    if !experiment && st.handed.is_empty() {
        return Task::none();
    }
    let ids: Vec<String> = m.settings().nodes.iter().map(|n| n.id.clone()).collect();
    Task::batch(ids.into_iter().filter_map(|id| {
        let client = crate::nodes::client(m, &id)?;
        Some(Task::perform(
            async move {
                let answer =
                    crate::nodes::wire::request(&client, "GET", "/node/discord", None).await;
                answer
                    .ok()
                    .and_then(|answer| answer.json().ok())
                    .map(|value| NodeBot::of(&value))
            },
            move |bot| {
                Message::Discord(Msg::NodeBot {
                    node: id.clone(),
                    bot,
                })
            },
        ))
    }))
}

/// The desktop's bot handed to node `node` when it is the person's and the
/// node has none of its own; taken back when the desktop's is gone.
fn hand_to(st: &State, m: &Model, node: &str) -> Task<Message> {
    let Some(bot) = st.nodes.get(node) else {
        return Task::none();
    };
    if bot.its_own() {
        return Task::none();
    }
    let Some(client) = crate::nodes::client(m, node) else {
        return Task::none();
    };
    let experiment = experiments::enabled(m.settings(), experiments::DISCORD_CONNECTOR);
    let label = m
        .settings()
        .nodes
        .iter()
        .find(|n| n.id == node)
        .map(|n| {
            if n.label.trim().is_empty() {
                n.ssh_target.clone()
            } else {
                n.label.clone()
            }
        })
        .unwrap_or_default();
    let wanted = handed(&st.saved, &label).filter(|_| experiment);
    let node = node.to_string();
    match wanted {
        Some(body) => {
            let what = body.to_string();
            // Handed already, and the node still runs it.
            if st.handed.get(&node) == Some(&what) && bot.shared && bot.running {
                return Task::none();
            }
            Task::perform(
                async move {
                    crate::nodes::wire::request(&client, "PUT", "/node/discord/shared", Some(&body))
                        .await
                        .is_ok_and(|answer| answer.ok())
                },
                move |ok| {
                    Message::Discord(Msg::Handed {
                        node: node.clone(),
                        handed: Some(what.clone()),
                        ok,
                    })
                },
            )
        }
        None if bot.shared => Task::perform(
            async move {
                crate::nodes::wire::request(&client, "DELETE", "/node/discord/shared", None)
                    .await
                    .is_ok_and(|answer| answer.ok())
            },
            move |ok| {
                Message::Discord(Msg::Handed {
                    node: node.clone(),
                    handed: None,
                    ok,
                })
            },
        ),
        None => Task::none(),
    }
}

/// While the experiment is on (or the bot still runs, to stop it), the
/// settings are read again every 2 s.
pub fn subscription(st: &State, m: &Model) -> Subscription<Message> {
    let experiment = experiments::enabled(m.settings(), experiments::DISCORD_CONNECTOR);
    if (experiment && m.live.hydrated) || st.running.is_some() || !st.handed.is_empty() {
        iced::time::every(RECONCILE).map(|_| Message::Discord(Msg::Reconcile))
    } else {
        Subscription::none()
    }
}

pub fn on_intent(
    st: &mut State,
    m: &mut Model,
    sv: &Services,
    intent: &Intent,
) -> Option<Task<Message>> {
    let _ = (st, m, sv, intent);
    None
}

pub fn on_event(
    st: &mut State,
    m: &mut Model,
    sv: &Services,
    event: &CoreEvent,
) -> Option<Task<Message>> {
    let _ = (st, m, sv, event);
    None
}

/// The bot stopped with the app.
pub fn shutdown(st: &mut State) {
    if let Some(running) = st.running.take() {
        running.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_dialog::testing::{Harness, messages};
    use hangar_i18n::Locale;

    #[test]
    fn the_page_asks_for_the_token_and_the_server_until_they_are_set() {
        let h = Harness::new(Locale::En, "graphite");
        let st = State::default();
        let mut ui = h.show(settings_page(&st, h.ctx()));
        for text in ["Bot token", "Server ID", "Connect"] {
            assert!(ui.find(text).is_ok(), "{text}");
        }
        ui.click("Connect").expect("the button");
        assert!(matches!(
            messages(ui).as_slice(),
            [Message::Discord(Msg::Connect)]
        ));
        let refused = State {
            form: Form {
                error: Some(hangar_i18n::Key::DiscordBotErrorGuild),
                ..Form::default()
            },
            ..State::default()
        };
        let mut ui = h.show(settings_page(&refused, h.ctx()));
        let said = hangar_i18n::t(Locale::En, hangar_i18n::Key::DiscordBotErrorGuild);
        assert!(ui.find(said.as_ref()).is_ok());
    }

    #[test]
    fn once_set_the_page_gives_the_pairing_code_the_invite_and_disconnect() {
        let h = Harness::new(Locale::Fr, "graphite");
        let st = State {
            saved: Settings {
                // "123456789012345678" in base64: the bot's id.
                token: "MTIzNDU2Nzg5MDEyMzQ1Njc4.GAbcde.xyz".into(),
                guild: "42".into(),
                owner: None,
                pairing: Some("314159".into()),
                ..Settings::default()
            },
            ..State::default()
        };
        // As the dictionary spells them, its typography included.
        let fr = |key| hangar_i18n::t(Locale::Fr, key).into_owned();
        let pair = hangar_i18n::t_with(
            Locale::Fr,
            hangar_i18n::Key::DiscordBotPair,
            &hangar_i18n::args!("code" => "314159"),
        )
        .into_owned();
        assert!(pair.contains("pair 314159"));
        let mut ui = h.show(settings_page(&st, h.ctx()));
        for text in [
            "Connexion · serveur 42".to_string(),
            pair,
            fr(hangar_i18n::Key::DiscordBotInvite),
            fr(hangar_i18n::Key::DiscordBotDisconnect),
        ] {
            assert!(ui.find(text.as_str()).is_ok(), "{text}");
        }
        ui.click("Déconnecter").expect("the button");
        assert!(matches!(
            messages(ui).as_slice(),
            [Message::Discord(Msg::Disconnect)]
        ));
        let paired = State {
            saved: Settings {
                owner: Some("u1".into()),
                pairing: None,
                ..st.saved.clone()
            },
            ..State::default()
        };
        let mut ui = h.show(settings_page(&paired, h.ctx()));
        assert!(
            ui.find(fr(hangar_i18n::Key::DiscordBotPaired).as_str())
                .is_ok()
        );
    }

    #[test]
    fn a_node_is_handed_the_bot_once_the_person_is_paired() {
        let mut saved = Settings {
            token: "t".into(),
            guild: "42".into(),
            ..Settings::default()
        };
        assert_eq!(handed(&saved, "vps"), None, "nobody paired yet");
        saved.owner = Some("u1".into());
        let body = handed(&saved, "vps").unwrap();
        assert_eq!(body["token"], "t");
        assert_eq!(body["owner"], "u1");
        assert_eq!(body["label"], "vps");
        saved.token.clear();
        assert_eq!(handed(&saved, "vps"), None, "disconnected");
        let own = NodeBot::of(&serde_json::json!({ "configured": true, "shared": false }));
        assert!(own.its_own());
        let shared = NodeBot::of(&serde_json::json!({
            "configured": true, "shared": true, "running": true, "channels": ["c1", "t1"],
        }));
        assert!(!shared.its_own());
        assert_eq!(shared.channels, ["c1", "t1"]);
    }

    #[test]
    fn the_desktop_leaves_the_channels_of_the_nodes_it_shares_with() {
        let st = State {
            nodes: HashMap::from([
                (
                    "n1".to_string(),
                    NodeBot {
                        shared: true,
                        channels: vec!["c1".into()],
                        ..NodeBot::default()
                    },
                ),
                (
                    "n2".to_string(),
                    NodeBot {
                        configured: true,
                        channels: vec!["c2".into()],
                        ..NodeBot::default()
                    },
                ),
            ]),
            ..State::default()
        };
        publish_others(&st);
        let others = st.others.read().unwrap().clone();
        assert_eq!(others, HashSet::from(["c1".to_string()]));
    }

    #[test]
    fn the_bot_runs_with_the_experiment_the_state_and_complete_settings() {
        let complete = Settings {
            token: "t".into(),
            guild: "42".into(),
            ..Settings::default()
        };
        assert!(wanted(true, true, &complete));
        assert!(!wanted(false, true, &complete));
        assert!(!wanted(true, false, &complete));
        let no_token = Settings {
            token: String::new(),
            ..complete.clone()
        };
        assert!(!wanted(true, true, &no_token));
    }
}
