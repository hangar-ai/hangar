//! Hangar without its window (docs/native/headless.md): what runs the
//! workspaces whatever draws them. The desktop app builds its window around
//! it; a Hangar with no window runs it alone.
//!
//! - `state`: the persisted state of the workspaces and the store through
//!   which it changes.

pub mod state;
