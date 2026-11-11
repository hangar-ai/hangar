//! What Hangar Mobile's records are, apart from the server that keeps them
//! (`remote`, desktop only): the lines of the activity log and the files
//! agents share. A Hangar with no window reads and writes the same records.

use serde::{Deserialize, Serialize};

/// A line of the activity log (`remote::ActivityEntry`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Milliseconds since the epoch.
    pub at: u64,
    pub workspace: String,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub id: String,
    /// Milliseconds since the epoch.
    pub at: u64,
    /// The folder of the workspace the agent works in.
    pub workspace: String,
    pub pane: String,
    pub name: String,
    /// The Content-Type `/m/files/{id}` answers with.
    pub mime: String,
    /// The viewer the phone shows it in. A record from before kinds were
    /// told reads as `File`, and is judged again when the index loads.
    #[serde(default)]
    pub kind: ShareKind,
    pub size: u64,
    pub caption: Option<String>,
}

/// What the phone shows a share as: the kinds of the PC's preview, but the
/// 3D model, which the phone has no viewer for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareKind {
    Image,
    Video,
    Audio,
    /// Text and source code.
    Text,
    Markdown,
    Pdf,
    Html,
    /// Nothing the phone shows: it is handed over, to be opened or saved.
    #[default]
    File,
}
