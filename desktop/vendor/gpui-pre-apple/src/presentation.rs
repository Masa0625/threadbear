use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};

/// Native layer changes cannot overtake a drawable queued for a later vsync.
#[derive(Default)]
pub(crate) struct PresentationTracker(Arc<AtomicUsize>);

impl PresentationTracker {
    pub(crate) fn pending(&self) -> bool { self.0.load(Ordering::Acquire) != 0 }

    pub(crate) fn begin(&self) -> PendingPresentation {
        self.0.fetch_add(1, Ordering::AcqRel);
        PendingPresentation { count: self.0.clone(), finished: AtomicBool::new(false) }
    }
}

pub(crate) struct PendingPresentation {
    count: Arc<AtomicUsize>,
    finished: AtomicBool,
}

impl PendingPresentation {
    pub(crate) fn complete(&self) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            self.count.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for PendingPresentation {
    fn drop(&mut self) {
        // Metal may release a cancelled drawable's handler without calling it.
        // Once released it cannot show up later and block future handoffs.
        self.complete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_older_drawables_must_finish_and_completion_is_idempotent() {
        let tracker = PresentationTracker::default();
        let first = tracker.begin();
        let second = tracker.begin();
        first.complete();
        first.complete();
        drop(first);
        assert!(tracker.pending());
        second.complete();
        assert!(!tracker.pending());
        drop(second);
        assert!(!tracker.pending());
    }

    #[test]
    fn releasing_a_cancelled_handler_unblocks_the_handoff() {
        let tracker = PresentationTracker::default();
        let pending = tracker.begin();
        assert!(tracker.pending());
        drop(pending);
        assert!(!tracker.pending());
    }

    #[test]
    fn completion_can_arrive_from_the_metal_thread() {
        let tracker = PresentationTracker::default();
        let pending = tracker.begin();
        std::thread::spawn(move || pending.complete()).join().unwrap();
        assert!(!tracker.pending());
    }
}
