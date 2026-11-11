//! Writing `state.json` back (inventory §1.4, "Writer rules"): only the main
//! window's model saves, never before hydration, 400 ms after the last change,
//! the whole state each time, and once more on exit so a change made just
//! before quitting is not lost (the TS lost it).
//!
//! The mechanism: a change bumps `Model::revision`; the shell answers with a
//! `Message::SaveDue(revision)` 400 ms later (`schedule`), and saves only if
//! no later change came meanwhile (`is_due`). The write itself runs on a
//! blocking thread (`services::core::save_state`), through a `Writer` that
//! never lets an older revision land over a newer one: the exit's own write
//! can overtake a debounced one still running.

use iced::Task;

pub use hangar_engine::state::persist::{SAVE_DEBOUNCE, Save, Writer, is_due};

use crate::message::Message;

/// The `SaveDue` that fires once the model has been still long enough.
pub fn schedule(revision: u64) -> Task<Message> {
    Task::perform(tokio::time::sleep(SAVE_DEBOUNCE), move |()| {
        Message::SaveDue(revision)
    })
}
