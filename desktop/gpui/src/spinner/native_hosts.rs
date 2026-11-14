// Main-thread ownership and submission of the working arcs' native layers.
// Geometry comes from ArcLayer; policy is shared with the headless tests.
use super::arc_policy::{Spec, State, Target};
use gpui_kit::{Window, WindowId};
use serde_json::json;
use std::{cell::RefCell, collections::{HashMap, HashSet}, ffi::c_void, rc::Rc, time::Instant};

#[repr(C)]
#[derive(Default)]
struct Health {
    placed: u32, visible: u32, animating: u32, presented: u32,
    phase: f64, period: f64, time: f64,
}
unsafe extern "C" {
    fn tb_arc_probe_create(parent: *mut c_void) -> *mut c_void;
    fn tb_arc_probe_set(host: *mut c_void, specs: *const Spec, count: usize, phase: f64, duration: f64);
    fn tb_arc_probe_drop(host: *mut c_void);
    fn tb_arc_probe_snapshot(host: *mut c_void, health: *mut Health);
}
struct Host(*mut c_void);
impl Drop for Host { fn drop(&mut self) { unsafe { tb_arc_probe_drop(self.0); } } }

#[derive(Default)]
pub(super) struct Hosts {
    hosts: HashMap<WindowId, Host>,
    states: HashMap<WindowId, State>,
}
pub(super) struct Update { id: WindowId, target: Target, flags: u32 }

/// Use the same queue in ArcLayer and the main-thread native integration test.
pub(super) fn queue_update(
    hosts: &Rc<RefCell<Hosts>>, update: Update, window: &mut Window,
    phase: fn() -> f64, period: f64, now: impl FnOnce() -> Instant + 'static,
) {
    let id = update.id;
    let applied = hosts.clone(); let failed = hosts.clone();
    window.defer_until_present_with_fallback(
        move || applied.borrow_mut().apply(update, phase(), period),
        move || failed.borrow_mut().failed(id, period, now()),
    );
}

impl Hosts {
    pub fn remove(&mut self, id: WindowId) { self.hosts.remove(&id); self.states.remove(&id); }
    fn prune(&mut self, open: &HashSet<WindowId>) {
        self.hosts.retain(|key, _| open.contains(key));
        self.states.retain(|key, _| open.contains(key));
    }
    pub fn report(&self, id: WindowId, requested: bool, period: f64) -> serde_json::Value {
        let mut health = Health::default();
        if let Some(host) = self.hosts.get(&id) { unsafe { tb_arc_probe_snapshot(host.0, &mut health); } }
        let state = self.states.get(&id);
        let flags = state.map_or(0, |s| s.flags);
        let names = ["dock-drag", "app-overlay", "capture", "deferred", "tooltip", "component-overlay", "prompt", "drag", "placement-changing", "native-backoff"];
        let blocked: Vec<_> = names.into_iter().enumerate().filter_map(|(i, name)| (flags & (1 << i) != 0).then_some(name)).collect();
        json!({"requested": requested, "backend": if health.visible > 0 { "core-animation" } else { "rust" },
            "blockedBy": blocked, "placed": health.placed, "visible": health.visible, "animating": health.animating,
            "presented": health.presented, "phase": health.phase, "periodSeconds": health.period,
            "expectedPeriodSeconds": period, "snapshotSeconds": health.time,
            "lifetimeAppliedUpdates": state.map_or(0, |s| s.lifetime_applied_updates),
            "lifetimeNativeFailures": state.map_or(0, |s| s.lifetime_failures)})
    }
    pub fn prepare(
        &mut self, id: WindowId, specs: &[Spec], scale: f32, flags: u32,
        open: &HashSet<WindowId>, now: Instant, parent: impl FnOnce() -> Option<*mut c_void>,
    ) -> (bool, Option<Update>) {
        self.prune(open);
        let decision = self.states.entry(id).or_default().decide(specs, scale, flags, self.hosts.contains_key(&id), now);
        let Some(target) = decision.update else { return (decision.native, None); };
        if !self.hosts.contains_key(&id) {
            let raw = parent().map(|parent| unsafe { tb_arc_probe_create(parent) }).unwrap_or(std::ptr::null_mut());
            if raw.is_null() {
                // No native content exists to clear. Keep Rust painting while
                // creation is unavailable, with the same bounded cooldown.
                self.states.get_mut(&id).unwrap().failed(now);
                return (false, None);
            }
            self.hosts.insert(id, Host(raw));
        }
        let flags = self.states[&id].flags;
        (decision.native, Some(Update { id, target, flags }))
    }
    pub fn apply(&mut self, update: Update, phase: f64, period: f64) {
        // Closing the window can retire its host after the draw was queued.
        let Some(host) = self.hosts.get(&update.id) else { return; };
        unsafe { tb_arc_probe_set(host.0, update.target.specs.as_ptr(), update.target.specs.len(), phase, period); }
        if let Some(state) = self.states.get_mut(&update.id) { state.applied(update.target, update.flags); }
    }
    pub fn failed(&mut self, id: WindowId, period: f64, now: Instant) {
        // Fault-only degradation: hide decoration BEFORE recording empty
        // ownership, so no stale native arc can cover a fresh Rust scene.
        if let Some(host) = self.hosts.get(&id) {
            unsafe { tb_arc_probe_set(host.0, std::ptr::null(), 0, 0., period); }
        }
        if let Some(state) = self.states.get_mut(&id) { state.failed(now); }
    }
}

#[cfg(test)]
mod tests {
    use super::{Hosts, State};
    use gpui_kit::WindowId;
    use std::{collections::HashSet, time::Instant};
    #[test]
    fn close_and_pruning_retire_the_entire_window_state() {
        let a = WindowId::from(10u64); let b = WindowId::from(11u64);
        let mut hosts = Hosts::default();
        hosts.states.entry(a).or_default().failed(Instant::now());
        hosts.states.insert(b, State::default());
        hosts.prune(&HashSet::from([b]));
        assert!(!hosts.states.contains_key(&a));
        hosts.remove(b);
        assert!(hosts.states.is_empty()); assert!(hosts.hosts.is_empty());
    }
}
