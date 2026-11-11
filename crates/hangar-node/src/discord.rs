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
//! The node API: `GET /node/discord` (the same status), `POST
//! /node/discord/reload` (the settings read again, the bot started again).
#![cfg(target_os = "linux")]

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hangar_discord::Settings;
use hangar_engine::host::EngineHost;
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::api::Shared;

/// The longest wait between two tries of a bot that stopped.
const RETRY_MAX: Duration = Duration::from_secs(300);

/// The bot the daemon runs, when its settings are complete.
#[derive(Default)]
pub struct Connector {
    running: Mutex<Option<JoinHandle<()>>>,
}

impl Connector {
    /// The bot stopped, then started again from the settings in `dir`
    /// when they are complete. Called on the runtime.
    pub fn restart(&self, own: &EngineHost, dir: &Path) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(task) = running.take() {
            task.abort();
        }
        let ready = Settings::load(dir).map(|s| s.ready()).unwrap_or(false);
        if !ready {
            return;
        }
        let (own, dir) = (own.clone(), dir.to_path_buf());
        *running = Some(tokio::spawn(keep_running(own, dir)));
    }

    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    }
}

/// The bot, tried again after a while when it stops (Discord down, the
/// network cut, a token refused until the person fixes it).
async fn keep_running(own: EngineHost, dir: PathBuf) {
    let mut wait = Duration::from_secs(5);
    loop {
        // Another Discord than discord.com: a fake one, in the tests.
        let named = |var: &str| std::env::var(var).ok().filter(|v| !v.trim().is_empty());
        let options = hangar_discord::Options {
            api: named("HANGAR_DISCORD_API").unwrap_or_else(|| hangar_discord::rest::API.into()),
            gateway: named("HANGAR_DISCORD_GATEWAY"),
            dir: dir.clone(),
        };
        match hangar_discord::run(own.clone(), options).await {
            Ok(()) => return,
            Err(why) => eprintln!("discord: {why}; trying again in {}s", wait.as_secs()),
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(RETRY_MAX);
    }
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
        Err(err) => return crate::api::error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    match &node.own {
        Some(own) => {
            node.discord.restart(own, &dir);
            Json(status(&dir, node.discord.is_running())).into_response()
        }
        None => crate::api::error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "this node runs no workspace of its own",
        ),
    }
}

/// The bot's user id, read from its token: the token's first part is that
/// id in base64. An application's bot shares its id, which the invite link
/// names.
pub fn bot_id(token: &str) -> Option<String> {
    let first = token.trim().split('.').next()?;
    let bytes = base64_decode(first)?;
    let id = String::from_utf8(bytes).ok()?;
    id.chars().all(|c| c.is_ascii_digit()).then_some(id)
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text.bytes().filter(|c| *c != b'=') {
        buffer = (buffer << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The link that adds the bot to a server, with the scopes and permissions
/// it needs (discord-connector.md, "Setup and security").
pub fn invite(bot: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={bot}&scope=bot%20applications.commands&permissions={}",
        hangar_discord::bot::BOT_ALLOW
    )
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
    if guild.is_empty() || !guild.chars().all(|c| c.is_ascii_digit()) {
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
    if let Some(bot) = bot_id(&token) {
        println!("Add the bot to your server: {}", invite(&bot));
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
    fn the_bot_s_id_is_read_from_its_token() {
        // "123456789012345678" in base64, as Discord writes it.
        let token = "MTIzNDU2Nzg5MDEyMzQ1Njc4.GAbcde.xyz";
        assert_eq!(bot_id(token).as_deref(), Some("123456789012345678"));
        assert_eq!(bot_id("not-a-token"), None);
        let link = invite("123");
        assert!(link.contains("client_id=123"));
        assert!(link.contains("scope=bot%20applications.commands"));
    }

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
