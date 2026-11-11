//! Saving the state: what is written (`to_json`), when (`is_due`), and that
//! an older save never lands over a newer one (`Writer`). The debounce
//! itself is the host's: the app schedules it with iced, a Hangar with no
//! window with tokio.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::types::AppState;
use super::unread::Unread;

/// How long the state settles after a change before it is saved.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

/// A save scheduled at `due` runs only if nothing changed since and that
/// revision is not saved yet.
pub fn is_due(due: u64, current: u64, saved: u64) -> bool {
    due == current && current != saved
}

/// The state as `state.json` holds it, with what this version could not read
/// put back.
pub fn to_json(state: &AppState, unread: &Unread) -> Value {
    let mut value = serde_json::to_value(state)
        .unwrap_or_else(|err| panic!("the state could not be written as JSON: {err}"));
    if !unread.is_empty() {
        let mut unread = unread.clone();
        unread.prune(&value);
        unread.restore(&mut value);
    }
    value
}

/// A save, taken from the store and turned into JSON elsewhere.
#[derive(Debug, Clone)]
pub struct Save {
    pub state: Arc<AppState>,
    pub unread: Unread,
}

impl Save {
    pub fn into_json(self) -> Value {
        to_json(&self.state, &self.unread)
    }
}

/// The revision last written to disk, shared by every save in flight.
#[derive(Debug, Clone, Default)]
pub struct Writer(Arc<Mutex<u64>>);

impl Writer {
    /// Runs `write` for `revision` unless a newer one is written already.
    pub fn write(
        &self,
        revision: u64,
        write: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        let mut written = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if *written >= revision {
            return Ok(false);
        }
        write()?;
        *written = revision;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_change_saves_and_only_once() {
        assert!(is_due(3, 3, 2));
        assert!(!is_due(2, 3, 1), "a later change is still settling");
        assert!(!is_due(3, 3, 3), "already written");
    }

    #[test]
    fn an_older_revision_never_lands_over_a_newer_one() {
        let writer = Writer::default();
        let mut files = Vec::new();
        assert_eq!(
            writer.write(5, || {
                files.push(5);
                Ok(())
            }),
            Ok(true)
        );
        assert_eq!(
            writer.write(4, || {
                files.push(4);
                Ok(())
            }),
            Ok(false)
        );
        assert_eq!(
            writer.write(5, || {
                files.push(5);
                Ok(())
            }),
            Ok(false)
        );
        assert_eq!(
            writer.write(6, || Err("disk full".into())),
            Err("disk full".into())
        );
        assert_eq!(
            writer.write(6, || {
                files.push(6);
                Ok(())
            }),
            Ok(true)
        );
        assert_eq!(files, [5, 6]);
    }
}
