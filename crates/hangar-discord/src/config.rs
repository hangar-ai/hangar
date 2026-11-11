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
    let temporary = file.with_extension("json.tmp");
    std::fs::write(&temporary, body).map_err(|err| format!("{}: {err}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&temporary, file).map_err(|err| format!("{}: {err}", file.display()))
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
    }
}
