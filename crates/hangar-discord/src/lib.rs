//! The Discord connector (docs/native/discord-connector.md): a Discord
//! server as one more surface of a Hangar, for whoever would rather not
//! open the app. A category per folder, a channel per workspace (its
//! orchestrator's conversation), a worker's thread opened on demand with a
//! button; cards for what waits on the person; only the paired account is
//! heard.
//!
//! It serves whichever Hangar runs it (`Hangar`): the desktop app, where it
//! reaches the same workspaces as the phone, local and on nodes; or a node
//! with no desktop (`hangar-node serve`), its own workspaces.
//!
//! - `rest`, `gateway`: the thin client of Discord's own it speaks with;
//! - `cards`: the messages, as Components V2; `ids`: what a click acts on;
//! - `bot`: the connector, the server's structure and the dashboards;
//!   `follow`: the panes followed; `act`: what the person does; `lanes`:
//!   what waits on Discord kept in order per agent and channel, side by
//!   side between them.

mod act;
pub mod bot;
pub mod cards;
pub mod config;
mod follow;
pub mod gateway;
pub mod hangar;
pub mod ids;
mod lanes;
pub mod rest;
pub mod setup;
pub mod words;

pub use bot::{Options, run};
pub use config::Settings;
pub use hangar::Hangar;
pub use setup::keep_running;
