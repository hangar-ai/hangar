//! The desk: what a surface away from the window asks of the workspaces and
//! their panes (Hangar Mobile's calls, the chat view's, a connector's): read
//! a pane's conversation, send to it, answer what it waits on, follow its
//! deliveries, its apps and the statistics. Whatever runs it; the host gives
//! it its sources (`engine::Sources`) and its port.

pub mod activity_log;
pub mod answer;
pub mod apps;
pub mod calls;
pub mod chat_live;
pub mod contract;
pub mod delivery;
pub mod desk;
pub mod engine;
pub mod held;
pub mod ops;
pub mod push;
pub mod stats;

#[cfg(test)]
mod tests;
