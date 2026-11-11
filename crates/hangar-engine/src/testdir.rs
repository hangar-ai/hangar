//! Scratch folders for tests: made under the system temp folder and removed
//! when the test's thread ends, also when it panics.

use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static KEPT: RefCell<Vec<tempfile::TempDir>> = const { RefCell::new(Vec::new()) };
}

/// A new, empty folder named `hangar-{tag}-…`, removed when the test's thread
/// ends.
pub fn scratch(tag: &str) -> PathBuf {
    let dir = tempfile::Builder::new()
        .prefix(&format!("hangar-{tag}-"))
        .tempdir()
        .expect("a scratch folder");
    let path = dir.path().to_path_buf();
    KEPT.with(|kept| kept.borrow_mut().push(dir));
    path
}
