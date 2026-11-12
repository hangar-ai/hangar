//! The Discord connector in the node (docs/native/headless.md, step 5):
//! its settings on the server (`~/.local/state/hangar-node/discord.json`,
//! the token among them), run by the daemon beside the workspaces it runs
//! itself, started again when the settings change.
//!
//! - `hangar-node discord setup --guild <id>`: the bot's token read from
//!   `HANGAR_DISCORD_TOKEN` or the first line of standard input (never an
//!   argument: it would stay in the shell's history), the server's id; a
//!   pairing code made, the invite link printed;
//! - `hangar-node discord status`: configured, paired, the code to pair;
//! - `hangar-node discord off`: the token forgotten, the bot stopped.
//!
//! The node API: `GET /node/discord` (the same status, and the channels the
//! node's bot made), `POST /node/discord/reload` (the settings read again,
//! the bot started again).
//!
//! **One bot with the desktop.** The desktop that drives the node shares its
//! own bot with it (`PUT /node/discord/shared`): the node's workspaces get
//! channels of their own on the desktop's server, in categories named after
//! the node, and its bot answers in them, the desktop in its own. While no
//! desktop drives the node, the node's bot leads: it answers where neither
//! made a channel too. A node set up on its own (`discord setup`, another
//! bot) keeps it: the desktop's settings are refused.
#![cfg(target_os = "linux")]

use std::collections::HashSet;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hangar_discord::Settings;
use hangar_engine::host::EngineHost;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::api::Shared;
use crate::control::Drivers;

/// The node's workspaces, as its bot serves them.
struct NodeHangar {
    own: EngineHost,
    drivers: Arc<Drivers>,
    /// The bot is the desktop's, shared.
    shared: bool,
}

impl hangar_discord::Hangar for NodeHangar {
    fn port(&self) -> hangar_engine::control::port::Port {
        self.own.port()
    }

    fn desk(&self) -> Option<Arc<hangar_engine::mobile::engine::Mobile>> {
        self.own.desk()
    }

    fn journal(&self, ws: &str) -> Vec<hangar_engine::orchestration::events::JournalEntry> {
        hangar_discord::Hangar::journal(&self.own, ws)
    }

    fn tasks(&self, ws: &str) -> Vec<hangar_core::board::Task> {
        hangar_discord::Hangar::tasks(&self.own, ws)
    }

    fn events(&self) -> broadcast::Receiver<hangar_core::Event> {
        self.own.core().subscribe()
    }

    fn snapshot(&self) -> Arc<hangar_engine::state::Snapshot> {
        self.own.snapshot()
    }

    /// Its own bot, it leads; the desktop's, only while no desktop drives
    /// the node.
    fn leads(&self) -> bool {
        !self.shared || self.drivers.connected() == 0
    }

    fn others(&self) -> HashSet<String> {
        HashSet::new()
    }
}

/// The bot the daemon runs, when its settings are complete.
#[derive(Default)]
pub struct Connector {
    running: Mutex<Option<JoinHandle<()>>>,
}

impl Connector {
    /// The bot stopped, then started again from the settings in `dir`
    /// when they are complete. Called on the runtime.
    pub fn restart(&self, own: &EngineHost, drivers: &Arc<Drivers>, dir: &Path) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(task) = running.take() {
            task.abort();
        }
        let settings = Settings::load(dir).unwrap_or_default();
        if !settings.ready() {
            return;
        }
        let hangar = Arc::new(NodeHangar {
            own: own.clone(),
            drivers: drivers.clone(),
            shared: settings.shared,
        });
        *running = Some(tokio::spawn(keep_running(hangar, dir.to_path_buf())));
    }

    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    }
}

/// The bot, against discord.com or the Discord the environment names (a
/// fake one, in the tests).
async fn keep_running(hangar: Arc<NodeHangar>, dir: PathBuf) {
    let named = |var: &str| std::env::var(var).ok().filter(|v| !v.trim().is_empty());
    let options = hangar_discord::Options {
        api: named("HANGAR_DISCORD_API").unwrap_or_else(|| hangar_discord::rest::API.into()),
        gateway: named("HANGAR_DISCORD_GATEWAY"),
        dir,
    };
    hangar_discord::keep_running(hangar, options).await;
}

/// What `status` and `GET /node/discord` say.
pub fn status(dir: &Path, running: bool) -> Value {
    let settings = Settings::load(dir).unwrap_or_default();
    json!({
        "configured": settings.ready(),
        "guild": (!settings.guild.is_empty()).then_some(settings.guild.clone()),
        "paired": settings.owner.is_some(),
        "pairing": settings.pairing,
        "running": running,
        "shared": settings.shared,
        "label": settings.label,
        "channels": hangar_discord::config::Layout::load(dir)
            .map(|layout| layout.channels())
            .unwrap_or_default(),
    })
}

pub async fn get_status(State(node): State<Shared>) -> Response {
    let dir = match crate::state::dir() {
        Ok(dir) => dir,
        Err(err) => return crate::api::error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    Json(status(&dir, node.discord.is_running())).into_response()
}

pub async fn reload(State(node): State<Shared>) -> Response {
    let dir = match crate::state::dir() {
        Ok(dir) => dir,
        Err(err) => return crate::api::error(StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    match &node.own {
        Some(own) => {
            node.discord.restart(own, &node.drivers, &dir);
            Json(status(&dir, node.discord.is_running())).into_response()
        }
        None => crate::api::error(
            StatusCode::SERVICE_UNAVAILABLE,
            "this node runs no workspace of its own",
        ),
    }
}

/// The desktop's bot, shared with the node.
#[derive(Deserialize)]
pub struct SharedBody {
    token: String,
    guild: String,
    owner: String,
    #[serde(default)]
    label: Option<String>,
}

/// `PUT /node/discord/shared`: the desktop's bot serves the node's
/// workspaces too. Nothing changes when it is already so; a node with a
/// bot of its own refuses.
pub async fn share(State(node): State<Shared>, Json(body): Json<SharedBody>) -> Response {
    let dir = match crate::state::dir() {
        Ok(dir) => dir,
        Err(err) => return crate::api::error(StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    let Some(own) = &node.own else {
        return crate::api::error(
            StatusCode::SERVICE_UNAVAILABLE,
            "this node runs no workspace of its own",
        );
    };
    if body.token.trim().is_empty()
        || !hangar_discord::setup::is_guild_id(&body.guild)
        || body.owner.trim().is_empty()
    {
        return crate::api::error(StatusCode::BAD_REQUEST, "a token, a server and an owner");
    }
    let mut settings = Settings::load(&dir).unwrap_or_default();
    if settings.ready() && !settings.shared {
        return crate::api::error(
            StatusCode::CONFLICT,
            "this node has a Discord bot of its own (hangar-node discord off lets the desktop's in)",
        );
    }
    let wanted = Settings {
        token: body.token.trim().to_string(),
        guild: body.guild.trim().to_string(),
        owner: Some(body.owner.trim().to_string()),
        pairing: None,
        shared: true,
        label: body.label.filter(|l| !l.trim().is_empty()),
    };
    if settings == wanted && node.discord.is_running() {
        return Json(json!({ "changed": false })).into_response();
    }
    settings = wanted;
    if let Err(err) = settings.save(&dir) {
        return crate::api::error(StatusCode::INTERNAL_SERVER_ERROR, &err);
    }
    node.discord.restart(own, &node.drivers, &dir);
    Json(json!({ "changed": true })).into_response()
}

/// `DELETE /node/discord/shared`: the desktop's bot no longer serves the
/// node (the desktop disconnected it). A bot of the node's own stays.
pub async fn unshare(State(node): State<Shared>) -> Response {
    let dir = match crate::state::dir() {
        Ok(dir) => dir,
        Err(err) => return crate::api::error(StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    let mut settings = Settings::load(&dir).unwrap_or_default();
    if !settings.shared {
        return Json(json!({ "changed": false })).into_response();
    }
    settings.token.clear();
    settings.shared = false;
    if let Err(err) = settings.save(&dir) {
        return crate::api::error(StatusCode::INTERNAL_SERVER_ERROR, &err);
    }
    if let Some(own) = &node.own {
        node.discord.restart(own, &node.drivers, &dir);
    }
    Json(json!({ "changed": true })).into_response()
}

/// `hangar-node discord setup --guild <id>`.
pub fn setup(args: &[&str]) -> Result<(), String> {
    let mut guild = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--guild" => {
                guild = Some(
                    rest.next()
                        .ok_or("--guild wants the server's id")?
                        .to_string(),
                )
            }
            other => return Err(format!("unexpected: {other}")),
        }
    }
    let guild = guild.ok_or("discord setup wants --guild <the server's id>")?;
    if !hangar_discord::setup::is_guild_id(&guild) {
        return Err(
            "a server's id is digits: Discord › Server Settings › Widget, or Copy Server ID".into(),
        );
    }
    let token = match std::env::var("HANGAR_DISCORD_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token.trim().to_string(),
        _ => {
            eprintln!("The bot's token (Developer Portal › your application › Bot › Reset Token):");
            let mut line = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|err| err.to_string())?;
            line.trim().to_string()
        }
    };
    if token.is_empty() {
        return Err("no token given".into());
    }
    let dir = crate::state::dir()?;
    crate::state::ensure_dir(&dir)?;
    let mut settings = Settings::load(&dir)?;
    // Another server or another bot: whoever was paired is no longer.
    if settings.guild != guild || settings.token != token {
        settings.owner = None;
        settings.pairing = None;
    }
    settings.guild = guild;
    settings.token = token.clone();
    let code = settings.pairing_code();
    settings.save(&dir)?;
    if let Some(bot) = hangar_discord::setup::bot_id(&token) {
        println!(
            "Add the bot to your server: {}",
            hangar_discord::setup::invite(&bot)
        );
    }
    match code {
        Some(code) => {
            println!("Then send `pair {code}` in any channel the bot sees: only you will be heard.")
        }
        None => println!("Already paired."),
    }
    reload_running();
    Ok(())
}

/// `hangar-node discord status`.
pub fn print_status() -> Result<(), String> {
    let dir = crate::state::dir()?;
    let running = crate::cli::call("GET", "/node/discord", None)
        .ok()
        .and_then(|(status, body)| (status == 200).then_some(body["running"] == true))
        .unwrap_or(false);
    let status = status(&dir, running);
    if status["configured"] != true {
        println!("Discord: not set up (hangar-node discord setup --guild <id>)");
        return Ok(());
    }
    println!(
        "Discord: server {}, {}",
        status["guild"].as_str().unwrap_or_default(),
        if running {
            "running"
        } else {
            "not running (is hangar-node serve up?)"
        }
    );
    match status["pairing"].as_str() {
        Some(code) if status["paired"] != true => {
            println!("Not paired: send `pair {code}` to the bot.")
        }
        _ => println!("Paired."),
    }
    Ok(())
}

/// `hangar-node discord off`.
pub fn off() -> Result<(), String> {
    let dir = crate::state::dir()?;
    let mut settings = Settings::load(&dir)?;
    settings.token.clear();
    settings.save(&dir)?;
    reload_running();
    println!("Discord: off.");
    Ok(())
}

/// The running daemon told to read the settings again, when there is one.
fn reload_running() {
    if let Err(err) = crate::cli::call("POST", "/node/discord/reload", Some(json!({}))) {
        if !err.contains("not running") {
            eprintln!("the running node could not be told: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_says_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(status(dir.path(), false)["configured"], false);
        let mut settings = Settings {
            token: "t".into(),
            guild: "1".into(),
            ..Settings::default()
        };
        let code = settings.pairing_code().unwrap();
        settings.save(dir.path()).unwrap();
        let said = status(dir.path(), true);
        assert_eq!(said["configured"], true);
        assert_eq!(said["paired"], false);
        assert_eq!(said["pairing"], code.as_str());
        assert_eq!(said["running"], true);
    }
}
