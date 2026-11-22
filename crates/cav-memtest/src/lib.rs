//! Test support: an allocator that counts.
//!
//! A test binary installs [`Counting`] as its global allocator and asks [`peak_during`] how many
//! bytes more than at the start were ever live while a closure ran. That is how "this read's
//! memory does not grow with the length of the flight" is measured rather than argued.
//!
//! Depends on nothing. It is the only crate in the workspace with `unsafe` in it (two functions
//! that pass straight through to the system allocator), which is why it is a crate of its own and
//! does not take the workspace's lints.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Bytes live now, and the most that have been live since the last reset.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// The system allocator, counting what passes through it.
pub struct Counting;

// SAFETY: every call is handed unchanged to `System`, which upholds the contract; the counters
// are atomics and touch no memory the allocator manages.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        // SAFETY: the caller's contract is `System::alloc`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract is `System::dealloc`'s.
        unsafe { System.dealloc(pointer, layout) }
    }
}

/// The bytes live now. Meaningful only in a binary whose global allocator is [`Counting`].
pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Runs `work` and returns its result with the most bytes that were live at once during it, over
/// and above what was live when it began. Meaningful only in a binary whose global allocator is
/// [`Counting`], and only while no other thread is allocating.
pub fn peak_during<T>(work: impl FnOnce() -> T) -> (T, usize) {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let result = work();
    (result, PEAK.load(Ordering::Relaxed).saturating_sub(before))
}
