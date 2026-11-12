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
//! A node runs its own workspaces' bot the same way (hangar-node's
//! discord.rs). One bot token serves one Hangar: the desktop's and a node's
//! need two bots.

mod page;

use std::path::PathBuf;
use std::sync::Arc;
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

/// The package's UI state; lives in `App.pkg.discord`.
#[derive(Default)]
pub struct State {
    running: Option<Running>,
    /// The settings as last read.
    saved: Settings,
    form: Form,
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
}

/// Whether the bot should run: the experiment on, the state in, the
/// settings complete.
pub fn wanted(experiment: bool, hydrated: bool, saved: &Settings) -> bool {
    experiment && hydrated && saved.ready()
}

pub fn update(st: &mut State, m: &mut Model, sv: &Services, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Reconcile => reconcile(st, m, sv),
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
    });
    let options = hangar_discord::Options {
        api: hangar_discord::rest::API.to_string(),
        gateway: None,
        dir,
    };
    st.running = Some(Running {
        task: tokio::spawn(hangar_discord::keep_running(hangar, options)),
        token: st.saved.token.clone(),
        guild: st.saved.guild.clone(),
    });
}

/// While the experiment is on (or the bot still runs, to stop it), the
/// settings are read again every 2 s.
pub fn subscription(st: &State, m: &Model) -> Subscription<Message> {
    let experiment = experiments::enabled(m.settings(), experiments::DISCORD_CONNECTOR);
    if (experiment && m.live.hydrated) || st.running.is_some() {
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
