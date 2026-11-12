//! What the connector keeps on disk, beside the engine's state: its
//! settings (`discord.json`: the bot's token, the server, who is paired,
//! made the owner's alone) and where each workspace lives on the server
//! (`discord-layout.json`: categories, channels, webhooks, dashboards,
//! the workers' threads).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The connector's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The bot's token (Developer Portal › Bot › Reset Token).
    pub token: String,
    /// The server (guild) the workspaces' channels are on.
    pub guild: String,
    /// The Discord account Hangar listens to: the only one heard.
    pub owner: Option<String>,
    /// The code that pairs an account while none is (`pair <code>`).
    pub pairing: Option<String>,
    /// The settings came from the desktop that drives this node, the bot
    /// shared with it: the desktop's settings follow, and its pairing.
    pub shared: bool,
    /// What this Hangar's categories are told apart by when it shares the
    /// bot (the node's name): `Hangar · vps`.
    pub label: Option<String>,
}

/// Where a workspace lives on the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Place {
    pub channel: String,
    /// The bot's webhook in it: the agents speak under their names.
    pub webhook: Option<(String, String)>,
    /// The pinned dashboard.
    pub dashboard: Option<String>,
}

/// A worker's line in its workspace's channel, its thread once opened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Worker {
    pub workspace: String,
    pub name: String,
    pub task: String,
    pub channel: String,
    pub line: Option<String>,
    pub thread: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Layout {
    /// The server these places are on, and the bot that made them (its
    /// categories are private to it): on another server, or for another
    /// bot, none of them stands.
    pub guild: Option<String>,
    pub bot: Option<String>,
    /// A category per folder of Hangar's (`""`: the workspaces in none).
    pub categories: HashMap<String, String>,
    pub workspaces: HashMap<String, Place>,
    /// By the worker's pane.
    pub workers: HashMap<String, Worker>,
}

pub fn settings_file(dir: &Path) -> PathBuf {
    dir.join("discord.json")
}

pub fn layout_file(dir: &Path) -> PathBuf {
    dir.join("discord-layout.json")
}

fn read<T: for<'de> Deserialize<'de> + Default>(file: &Path) -> Result<T, String> {
    match std::fs::read_to_string(file) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|err| format!("{}: {err}", file.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(format!("{}: {err}", file.display())),
    }
}

/// Written whole (a temporary file renamed), the owner's alone: the
/// settings hold the bot's token.
fn write<T: Serialize>(file: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let body = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    // A name of its writer's own: the daemon, its command line and the
    // desktop's window write these files with no lock between them.
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nth = WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = file.with_extension(format!("json.{}.{nth}.tmp", std::process::id()));
    let written = private_file(&temporary).and_then(|mut out| {
        use std::io::Write;
        out.write_all(&body)
    });
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("{}: {err}", temporary.display()));
    }
    std::fs::rename(&temporary, file).map_err(|err| {
        let _ = std::fs::remove_file(&temporary);
        format!("{}: {err}", file.display())
    })
}

/// A new file nobody else reads, from the moment it exists: made with the
/// usual mode and closed to others afterwards, the token in it could be
/// read in between.
fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl Settings {
    pub fn load(dir: &Path) -> Result<Settings, String> {
        read(&settings_file(dir))
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        write(&settings_file(dir), self)
    }

    /// Whether the connector has what it needs to run.
    pub fn ready(&self) -> bool {
        !self.token.trim().is_empty() && !self.guild.trim().is_empty()
    }

    /// A pairing code, made when there is neither an owner nor a code.
    pub fn pairing_code(&mut self) -> Option<String> {
        if self.owner.is_some() {
            return None;
        }
        if self.pairing.is_none() {
            let digits: String = uuid::Uuid::new_v4()
                .as_u128()
                .to_string()
                .chars()
                .rev()
                .take(6)
                .collect();
            self.pairing = Some(digits);
        }
        self.pairing.clone()
    }
}

impl Layout {
    pub fn load(dir: &Path) -> Result<Layout, String> {
        read(&layout_file(dir))
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        write(&layout_file(dir), self)
    }

    /// The layout as it stands for bot `bot` on server `guild`: empty when
    /// it was made on another server or by another bot (its channels are
    /// there, or closed to this one), marked with both otherwise. Whether
    /// it changed.
    pub fn stand_for(&mut self, guild: &str, bot: &str) -> bool {
        let elsewhere = self.guild.as_deref().is_some_and(|known| known != guild)
            || self.bot.as_deref().is_some_and(|known| known != bot);
        if elsewhere {
            *self = Layout::default();
        }
        let marked = self.guild.as_deref() == Some(guild) && self.bot.as_deref() == Some(bot);
        if !marked {
            self.guild = Some(guild.to_string());
            self.bot = Some(bot.to_string());
        }
        elsewhere || !marked
    }

    /// The channels and threads this Hangar made on the server: what the
    /// other Hangars of the bot leave to it.
    pub fn channels(&self) -> Vec<String> {
        self.workspaces
            .values()
            .map(|place| place.channel.clone())
            .chain(self.workers.values().filter_map(|w| w.thread.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_layout_read_back_and_the_settings_are_private() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), Settings::default());
        let mut settings = Settings {
            token: "t".into(),
            guild: "g".into(),
            ..Settings::default()
        };
        let code = settings.pairing_code().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(settings.pairing_code(), Some(code));
        settings.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), settings);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(settings_file(dir.path()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        settings.owner = Some("42".into());
        assert_eq!(settings.pairing_code(), None);

        let mut layout = Layout::default();
        layout.workspaces.insert(
            "w1".into(),
            Place {
                channel: "c1".into(),
                webhook: Some(("h".into(), "tok".into())),
                dashboard: None,
            },
        );
        layout.save(dir.path()).unwrap();
        assert_eq!(Layout::load(dir.path()).unwrap(), layout);
        // No temporary file is left beside them.
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");

        // A layout read from before it named its server stands for the
        // one the bot is on; on another server, or for another bot, none
        // of its places does.
        assert!(layout.stand_for("g1", "bot1"));
        assert_eq!(layout.workspaces.len(), 1);
        assert!(!layout.stand_for("g1", "bot1"));
        let mut moved = layout.clone();
        assert!(moved.stand_for("g2", "bot1"));
        assert!(moved.workspaces.is_empty());
        assert_eq!(moved.guild.as_deref(), Some("g2"));
        let mut other_bot = layout.clone();
        assert!(other_bot.stand_for("g1", "bot2"));
        assert!(other_bot.workspaces.is_empty());
    }
}
