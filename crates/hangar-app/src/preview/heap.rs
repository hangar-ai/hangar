//! Giving back to the system the memory a closed view freed.
//!
//! A viewer of a large file frees a lot at once when it goes: the pixels of a
//! picture, the lines and runs of a coloured text, a rendered Markdown page
//! (two hundred megabytes for a long one), a 3D model's meshes, a kit tab's
//! instance and the working memory of the compiler that made its module.
//! glibc's allocator keeps what was freed in its arenas, one per thread that
//! allocated, for the next allocation rather than returning it, so a Hangar
//! that showed one long page held on to those megabytes for the rest of its
//! run. Asking it to trim them once the view is gone returns them.
//!
//! It runs a moment after the view goes, when the window has been drawn again
//! without it and the renderer has let go of what it drew from it too, on a
//! thread of its own: a trim walks the whole heap.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How long after a viewer goes the heap is trimmed.
pub const AFTER: Duration = Duration::from_secs(2);

/// A trim is waiting: viewers that go meanwhile are covered by it.
static PENDING: AtomicBool = AtomicBool::new(false);

/// Trims the heap [`AFTER`] from now, unless a trim is already waiting.
pub fn give_back_soon() {
    schedule(&PENDING, AFTER);
}

/// A trim `after` from now, unless `pending` says one is waiting already.
fn schedule(pending: &'static AtomicBool, after: Duration) {
    if pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("hangar-heap-trim".into())
        .spawn(move || {
            std::thread::sleep(after);
            pending.store(false, Ordering::SeqCst);
            trim();
        });
    if spawned.is_err() {
        pending.store(false, Ordering::SeqCst);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim() {
    unsafe extern "C" {
        /// glibc's: releases the free memory at the top of the heap and the
        /// free pages within it, in every arena.
        fn malloc_trim(pad: usize) -> std::ffi::c_int;
    }
    // SAFETY: malloc_trim takes no pointer and may be called from any thread
    // at any time; it only returns free memory to the system.
    unsafe {
        malloc_trim(0);
    }
}

/// Other allocators (musl, macOS, Windows) give back free pages by
/// themselves, or have no such call.
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim() {}

#[cfg(test)]
mod tests {
    use super::*;

    // A flag of its own: the app's PENDING is shared with every preview test
    // that closes a viewer, and one of them may ask for a trim meanwhile.
    static TEST_PENDING: AtomicBool = AtomicBool::new(false);

    #[test]
    fn a_trim_asked_while_one_waits_is_the_same_trim() {
        let after = Duration::from_millis(200);
        schedule(&TEST_PENDING, after);
        assert!(TEST_PENDING.load(Ordering::SeqCst));
        // Asked again meanwhile: no second thread, the first one covers it.
        schedule(&TEST_PENDING, after);
        std::thread::sleep(after + Duration::from_millis(800));
        assert!(!TEST_PENDING.load(Ordering::SeqCst));
    }
}
