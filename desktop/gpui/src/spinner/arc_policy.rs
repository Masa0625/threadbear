// The native arc owns a committed placement, not just the most recent paint.
// Kept separate from AppKit so decisions, cancellation and failure recovery
// can be tested against the same state the real host uses.
use std::time::{Duration, Instant};

pub(super) const CAPTURE: u32 = 1 << 2;
pub(super) const CHANGING: u32 = 1 << 8;
pub(super) const BACKOFF: u32 = 1 << 9;
const SETTLE: Duration = Duration::from_millis(150);

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Spec {
    pub center_x: f32, pub center_y: f32, pub side: f32, pub radius: f32, pub width: f32,
    pub clip_x: f32, pub clip_y: f32, pub clip_w: f32, pub clip_h: f32,
    pub r: f32, pub g: f32, pub b: f32, pub a: f32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Target { pub specs: Vec<Spec>, pub scale: f32 }
impl PartialEq for Target {
    fn eq(&self, other: &Self) -> bool {
        self.specs == other.specs && (self.specs.is_empty() || self.scale == other.scale)
    }
}

struct Observation { target: Target, flags: u32, changed: Instant }
#[derive(Default)]
pub(super) struct State {
    observed: Option<Observation>,
    committed: Target,
    pub flags: u32,
    pub lifetime_applied_updates: u64,
    pub lifetime_failures: u64,
    consecutive_failures: u32,
    retry_at: Option<Instant>,
}
pub(super) struct Decision { pub native: bool, pub update: Option<Target> }

impl State {
    pub fn decide(&mut self, specs: &[Spec], scale: f32, flags: u32, has_host: bool, now: Instant) -> Decision {
        let target = Target { specs: specs.to_vec(), scale };
        // A raster screenshot needs a Rust scene, but does not move the rows.
        let placement_flags = flags & !CAPTURE;
        let observed = self.observed.get_or_insert_with(|| Observation { target: target.clone(), flags: placement_flags, changed: now });
        if observed.target != target || observed.flags != placement_flags {
            *observed = Observation { target: target.clone(), flags: placement_flags, changed: now };
        }
        self.flags = flags
            | if now.saturating_duration_since(observed.changed) < SETTLE { CHANGING } else { 0 }
            | if self.retry_at.is_some_and(|at| now < at) { BACKOFF } else { 0 };
        let target = if self.flags == 0 { target } else { Target::default() };
        let native = !target.specs.is_empty();
        let update = (target != self.committed && (has_host || !target.specs.is_empty())).then_some(target);
        Decision { native, update }
    }

    pub fn applied(&mut self, target: Target, flags: u32) {
        self.committed = target;
        self.flags = flags;
        self.lifetime_applied_updates += 1;
        self.consecutive_failures = 0;
        self.retry_at = None;
    }

    /// Called only after the host has hidden its layers following a failed
    /// submission. A fresh Rust draw can then proceed without a stale overlay.
    pub fn failed(&mut self, now: Instant) {
        self.committed = Target::default();
        self.lifetime_failures += 1;
        self.consecutive_failures = self.consecutive_failures.saturating_add(1).min(3);
        self.retry_at = Some(now + Duration::from_millis(500 << (self.consecutive_failures - 1)));
        self.flags |= BACKOFF;
    }
}

#[cfg(test)]
mod tests {
    use super::{Decision, Spec, State, Target, CAPTURE, CHANGING, SETTLE};
    use std::time::{Duration, Instant};
    fn spec(y: f32) -> Spec {
        Spec { center_x: 10., center_y: y, side: 12., radius: 4.5, width: 1.6,
            clip_x: 0., clip_y: 0., clip_w: 100., clip_h: 100., r: 0., g: 0.5, b: 1., a: 1. }
    }
    fn expect_rust(d: &Decision) { assert!(!d.native, "unstable or failed native ownership must use Rust"); }
    fn native(now: Instant) -> State {
        let mut state = State::default();
        expect_rust(&state.decide(&[spec(10.)], 1., 0, false, now));
        let d = state.decide(&[spec(10.)], 1., 0, false, now + SETTLE);
        assert!(d.native);
        state.applied(d.update.expect("first settled draw needs a native commit"), state.flags);
        state
    }
    #[test]
    fn first_placement_settles_and_only_a_successful_commit_advances_ownership() {
        let now = Instant::now(); let mut state = State::default();
        for ms in [0, 16, 32, 80, 149] {
            let d = state.decide(&[spec(10.)], 1., 0, false, now + Duration::from_millis(ms));
            expect_rust(&d); assert!(d.update.is_none()); assert_ne!(state.flags & CHANGING, 0);
        }
        let a = state.decide(&[spec(10.)], 1., 0, false, now + SETTLE);
        assert!(a.native); let target = a.update.unwrap();
        assert!(state.committed.specs.is_empty(), "deciding cannot advance a failed or superseded commit");
        let b = state.decide(&[spec(10.)], 1., 0, true, now + SETTLE);
        assert!(b.native); assert_eq!(b.update, Some(target.clone()));
        state.applied(target, state.flags);
        let c = state.decide(&[spec(10.)], 1., 0, true, now + Duration::from_secs(60));
        assert!(c.native); assert!(c.update.is_none());
    }
    #[test]
    fn two_seconds_of_motion_send_one_hide_and_no_restore_until_settled() {
        let now = Instant::now(); let mut state = native(now); let mut updates = 0;
        for ms in (160..=2160).step_by(16) {
            let d = state.decide(&[spec(ms as f32)], 1., 0, true, now + Duration::from_millis(ms));
            expect_rust(&d);
            if let Some(target) = d.update {
                assert!(target.specs.is_empty()); updates += 1; state.applied(target, state.flags);
            }
        }
        assert_eq!(updates, 1);
        let at = now + Duration::from_millis(2160);
        expect_rust(&state.decide(&[spec(2160.)], 1., 0, true, at + SETTLE - Duration::from_millis(1)));
        let d = state.decide(&[spec(2160.)], 1., 0, true, at + SETTLE);
        assert!(d.native); assert_eq!(d.update.unwrap().specs, vec![spec(2160.)]);
    }
    #[test]
    fn gate_open_and_close_each_settle_but_capture_does_not_move_placement() {
        let now = Instant::now(); let mut state = native(now);
        let d = state.decide(&[spec(10.)], 1., 1 << 3, true, now + SETTLE * 2);
        expect_rust(&d); state.applied(d.update.unwrap(), state.flags);
        expect_rust(&state.decide(&[spec(10.)], 1., 0, true, now + SETTLE * 3));
        let d = state.decide(&[spec(10.)], 1., 0, true, now + SETTLE * 4);
        assert!(d.native); state.applied(d.update.unwrap(), state.flags);
        let capture = state.decide(&[spec(10.)], 1., CAPTURE, true, now + SETTLE * 5);
        expect_rust(&capture); assert!(capture.update.unwrap().specs.is_empty());
        // A screenshot draw is superseded without a live commit.
        let after = state.decide(&[spec(10.)], 1., 0, true, now + SETTLE * 5 + Duration::from_millis(1));
        assert!(after.native, "raster capture must not restart placement settling");
        assert!(after.update.is_none());
    }
    #[test]
    fn scale_clip_and_opacity_changes_settle_and_empty_scale_never_submits() {
        let now = Instant::now(); let mut state = native(now);
        let d = state.decide(&[spec(10.)], 2., 0, true, now + SETTLE * 2);
        expect_rust(&d); state.applied(d.update.unwrap(), state.flags);
        let mut changed = spec(10.); changed.clip_h = 90.;
        expect_rust(&state.decide(&[changed.clone()], 2., 0, true, now + SETTLE * 3));
        changed.a = 0.5;
        expect_rust(&state.decide(&[changed.clone()], 2., 0, true, now + SETTLE * 4));
        let d = state.decide(&[changed], 2., 0, true, now + SETTLE * 5);
        assert!(d.native); state.applied(d.update.unwrap(), state.flags);
        let d = state.decide(&[], 2., 0, true, now + SETTLE * 6);
        expect_rust(&d); state.applied(d.update.unwrap(), state.flags);
        assert!(state.decide(&[], 5., 0, true, now + SETTLE * 7).update.is_none());
        assert!(state.decide(&[], 5., 0, true, now + SETTLE * 8).update.is_none(), "even a settled empty host has no scale update");
        assert!(state.decide(&[], 3., 0, false, now + SETTLE * 9).update.is_none());
    }
    #[test]
    fn failed_restores_back_off_and_keep_drawing_rust_between_attempts() {
        let now = Instant::now(); let mut state = State::default();
        state.decide(&[spec(10.)], 1., 0, false, now);
        let mut attempts = 0; let mut rust_frames = 0;
        for ms in (150..=3150).step_by(20) {
            let at = now + Duration::from_millis(ms);
            let d = state.decide(&[spec(10.)], 1., 0, true, at);
            if d.update.is_some() { attempts += 1; state.failed(at); }
            else { expect_rust(&d); rust_frames += 1; }
        }
        assert_eq!(attempts, 3, "failed native restores must not retry at every Rust frame");
        assert!(rust_frames > 140);
        assert_eq!(state.lifetime_applied_updates, 0);
    }
    #[test]
    fn failed_last_arc_hide_leaves_no_native_ownership_or_pending_clock() {
        let now = Instant::now(); let mut state = native(now);
        let at = now + SETTLE * 2;
        let hide = state.decide(&[], 1., 0, true, at);
        expect_rust(&hide); assert!(hide.update.unwrap().specs.is_empty());
        // The failure callback has removed the actual native layers.
        state.failed(at);
        let d = state.decide(&[], 1., 0, true, at + Duration::from_millis(20));
        expect_rust(&d); assert!(d.update.is_none(), "a failed hide must not leave a native host to resurrect");
        assert_eq!(state.lifetime_failures, 1);
    }
    #[test]
    fn successful_native_commit_resets_failure_backoff_but_not_lifetime_counters() {
        let now = Instant::now(); let mut state = State::default();
        state.decide(&[spec(10.)], 1., 0, false, now);
        let at = now + SETTLE;
        let d = state.decide(&[spec(10.)], 1., 0, true, at); assert!(d.update.is_some());
        state.failed(at);
        expect_rust(&state.decide(&[spec(10.)], 1., 0, true, at + Duration::from_millis(499)));
        let d = state.decide(&[spec(10.)], 1., 0, true, at + Duration::from_millis(500));
        assert!(d.native); state.applied(d.update.unwrap(), state.flags);
        assert_eq!(state.lifetime_failures, 1); assert_eq!(state.lifetime_applied_updates, 1);
        state.failed(at + Duration::from_secs(1));
        assert_eq!(state.retry_at, Some(at + Duration::from_millis(1500)));
        assert_eq!(state.lifetime_failures, 2); assert_eq!(state.lifetime_applied_updates, 1);
    }
    #[test]
    fn placement_changes_cannot_bypass_a_failed_restore_cooldown() {
        let now = Instant::now(); let mut state = native(now);
        state.failed(now + SETTLE);
        let retry = state.retry_at;
        let mut changed = spec(10.); changed.center_x = 12.;
        for ms in [200, 300, 400, 500, 600] {
            let d = state.decide(&[changed.clone()], 1., 0, true, now + Duration::from_millis(ms));
            expect_rust(&d); assert!(d.update.is_none()); assert_eq!(state.retry_at, retry);
        }
    }
    #[test]
    fn repeated_failures_cap_the_cooldown_at_two_seconds() {
        let now = Instant::now(); let mut state = State::default();
        for n in 0..8 {
            let at = now + Duration::from_secs(n * 5);
            state.failed(at);
            assert_eq!(state.retry_at.unwrap().duration_since(at), Duration::from_millis(match n { 0 => 500, 1 => 1000, _ => 2000 }));
        }
    }
}
