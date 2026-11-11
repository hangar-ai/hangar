//! The Discord connector (docs/native/discord-connector.md): a Discord
//! server as one more surface of a Hangar, for whoever would rather not
//! open the app. A category per folder, a channel per workspace (its
//! orchestrator's conversation), a worker's thread opened on demand with a
//! button; cards for what waits on the person; only the paired account is
//! heard.
//!
//! It runs beside an engine host (`hangar_engine::host`), wherever that
//! runs: a node with no desktop (`hangar-node serve`).
//!
//! - `rest`, `gateway`: the thin client of Discord's own it speaks with;
//! - `cards`: the messages, as Components V2; `ids`: what a click acts on;
//! - `bot`: the connector, the server's structure and the dashboards;
//!   `follow`: the panes followed; `act`: what the person does.

mod act;
pub mod bot;
pub mod cards;
pub mod config;
mod follow;
pub mod gateway;
pub mod ids;
pub mod rest;
pub mod words;

pub use bot::{Options, run};
pub use config::Settings;
