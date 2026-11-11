//! What the agents' operations ask of the host beyond the model's writes.

use crate::state::types::PaneId;

/// « From a Git repository » (`FromGitRequest` of lib/workspaceFromGit.ts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRequest {
    /// As typed or handed over: `owner/repo`, an https or ssh address.
    pub repo: String,
    /// The workspace's name; the repository's by default.
    pub name: Option<String>,
    /// The folder the clone lands in, when the person said where.
    pub parent: Option<String>,
    pub origin: GitOrigin,
    /// The folder of the workspace asking: where the clone goes when no
    /// projects folder is set.
    pub asker_cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitOrigin {
    /// An agent, through the MCP tool: a repository not the person's needs
    /// their word (the consent card).
    Agent { pane: Option<PaneId> },
    /// The person clicked: asking again would be asking what they just said.
    Person,
}
