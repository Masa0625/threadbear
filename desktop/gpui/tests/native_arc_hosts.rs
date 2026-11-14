// Main-thread integration of the REAL host, queue and failure callback. No
// NSWindow, NSApplication, physical display, input event or live app is used.
#[cfg(target_os = "macos")]
#[path = "../src/spinner/arc_policy.rs"]
mod arc_policy;
#[cfg(target_os = "macos")]
#[path = "../src/spinner/native_hosts.rs"]
mod native_hosts;

#[cfg(target_os = "macos")]
mod mac {
    use super::{arc_policy::Spec, native_hosts::{self, Hosts, Update}};
    use gpui_kit::{div, px, size, AppContext as _, Context, IntoElement, Render, Styled as _, TestAppContext, Window, WindowId};
    use std::{cell::{Cell, RefCell}, collections::HashSet, ffi::c_void, rc::Rc, time::{Duration, Instant}};

    const PERIOD: f64 = 0.9;
    unsafe extern "C" {
        fn tb_arc_probe_test_view_create() -> *mut c_void;
        fn tb_arc_probe_test_view_layer_count(view: *mut c_void) -> u32;
        fn tb_arc_probe_test_clip_count(view: *mut c_void) -> u32;
        fn tb_arc_probe_test_model_period(view: *mut c_void) -> f64;
        fn tb_arc_probe_test_view_drop(view: *mut c_void);
    }
    struct View(*mut c_void);
    impl View {
        fn new() -> Self { let view = Self(unsafe { tb_arc_probe_test_view_create() }); assert!(!view.0.is_null()); view }
        fn layers(&self) -> u32 { unsafe { tb_arc_probe_test_view_layer_count(self.0) } }
        fn clips(&self) -> u32 { unsafe { tb_arc_probe_test_clip_count(self.0) } }
        fn period(&self) -> f64 { unsafe { tb_arc_probe_test_model_period(self.0) } }
    }
    impl Drop for View { fn drop(&mut self) { unsafe { tb_arc_probe_test_view_drop(self.0); } } }
    struct Blank;
    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div().size_full() }
    }
    thread_local! { static PHASE_CALLS: Cell<u32> = const { Cell::new(0) }; }
    fn phase() -> f64 { PHASE_CALLS.with(|n| n.set(n.get() + 1)); 0.25 }
    fn specs() -> Vec<Spec> {
        [20., 80.].into_iter().map(|y| Spec { center_x: 40., center_y: y, side: 12., radius: 4.5, width: 1.6,
            clip_x: 0., clip_y: 0., clip_w: 600., clip_h: 400., r: 0.1, g: 0.5, b: 0.8, a: 0.8 }).collect()
    }
    fn prepare(hosts: &Rc<RefCell<Hosts>>, id: WindowId, specs: &[Spec], now: Instant, view: Option<&View>) -> (bool, Option<Update>) {
        hosts.borrow_mut().prepare(id, specs, 1., 0, &HashSet::from([id]), now, || view.map(|v| v.0))
    }
    fn report(hosts: &Rc<RefCell<Hosts>>, id: WindowId) -> serde_json::Value { hosts.borrow().report(id, true, PERIOD) }

    fn a_failed_last_arc_hide_removes_actual_layers_and_retires_the_host() {
        let view = View::new();
        let hosts = Rc::new(RefCell::new(Hosts::default()));
        let mut cx = TestAppContext::single();
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| Blank);
        cx.run_until_parked(); let id = window.window_id(); let now = Instant::now(); let specs = specs();
        assert!(!prepare(&hosts, id, &specs, now, Some(&view)).0);
        let (native, update) = prepare(&hosts, id, &specs, now + Duration::from_millis(150), Some(&view));
        assert!(native);
        PHASE_CALLS.with(|n| n.set(0));
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            native_hosts::queue_update(&hosts, update.unwrap(), window, phase, PERIOD, move || now);
            window.simulate_failed_presentations(0);
            assert_eq!(window.simulate_presentation(false), (false, 0));
        }).unwrap();
        assert_eq!(PHASE_CALLS.with(|n| n.get()), 1, "successful queue must compute the current phase");
        assert_eq!(report(&hosts, id)["placed"], 2);
        assert_eq!(report(&hosts, id)["visible"], 2);
        assert_eq!(report(&hosts, id)["animating"], 2);
        assert_eq!(report(&hosts, id)["lifetimeAppliedUpdates"], 1);
        assert_eq!(report(&hosts, id)["lifetimeNativeFailures"], 0);
        assert_eq!(view.layers(), 1);
        assert_eq!(view.clips(), 2, "inspect the actual layer tree as well as the probe array");
        assert_eq!(view.period(), PERIOD);
        let at = now + Duration::from_millis(200);
        let (native, hide) = prepare(&hosts, id, &[], at, Some(&view));
        assert!(!native);
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            native_hosts::queue_update(&hosts, hide.unwrap(), window, phase, PERIOD, move || at);
            window.simulate_failed_presentations(1000);
            assert!(!window.simulated_refresh_pending(), "start the failure with a clean frame");
            assert_eq!(window.simulate_presentation(false), (false, 0));
            assert!(window.simulated_refresh_pending(), "failed last hide must request a fresh frame");
            assert_eq!(report(&hosts, id)["placed"], 0, "the real CA layers must be gone, not merely marked gone");
            assert_eq!(report(&hosts, id)["visible"], 0);
            assert_eq!(view.clips(), 0, "the CA tree must actually be empty");
            assert_eq!(report(&hosts, id)["lifetimeNativeFailures"], 1);
            // Failure and success handlers cannot run again on this old scene.
            window.simulate_failed_presentations(0);
            window.simulate_presentation(true);
            assert_eq!(report(&hosts, id)["placed"], 0);
            assert_eq!(report(&hosts, id)["lifetimeAppliedUpdates"], 1, "abandoned callbacks must not replay even after recovery");
        }).unwrap();
        assert!(prepare(&hosts, id, &[], at + Duration::from_millis(20), Some(&view)).1.is_none());
        hosts.borrow_mut().remove(id);
        assert_eq!(view.layers(), 0, "retirement must detach the real CA host from its parent");
        cx.quit();
        println!("PASS: failed last hide clears real layers, requests Rust frame, retires host");
    }

    fn persistent_failures_reach_the_real_queue_then_back_off() {
        let view = View::new(); let hosts = Rc::new(RefCell::new(Hosts::default()));
        let mut cx = TestAppContext::single();
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| Blank);
        cx.run_until_parked(); let id = window.window_id(); let now = Instant::now(); let specs = specs();
        let clock = Rc::new(Cell::new(now)); let mut attempts = 0; let mut rust_frames = 0;
        cx.update_window(window.into(), |_, window, _| window.simulate_failed_presentations(1000)).unwrap();
        for ms in (0..=3000).step_by(20) {
            let at = now + Duration::from_millis(ms); clock.set(at);
            let (native, update) = prepare(&hosts, id, &specs, at, Some(&view));
            cx.update_window(window.into(), |_, window, cx| {
                window.draw(cx).clear(cx);
                if let Some(update) = update {
                    attempts += 1;
                    let clock = clock.clone();
                    native_hosts::queue_update(&hosts, update, window, phase, PERIOD, move || clock.get());
                } else {
                    assert!(!native); rust_frames += 1;
                }
                assert_eq!(window.simulate_presentation(false), (false, 0));
            }).unwrap();
            assert_eq!(report(&hosts, id)["placed"], 0);
        }
        assert_eq!(attempts, 3, "the actual queue's failure callback must drive cooldown");
        assert!(rust_frames > 140);
        assert_eq!(report(&hosts, id)["lifetimeNativeFailures"], 3);
        assert_eq!(report(&hosts, id)["lifetimeAppliedUpdates"], 0);
        hosts.borrow_mut().remove(id); assert_eq!(view.layers(), 0); cx.quit();
        println!("PASS: actual queue backs off after failures; Rust frames continue");
    }

    fn unavailable_native_parent_keeps_rust_ownership() {
        let hosts = Rc::new(RefCell::new(Hosts::default())); let id = WindowId::from(100u64);
        let now = Instant::now(); let specs = specs();
        prepare(&hosts, id, &specs, now, None);
        let (native, update) = prepare(&hosts, id, &specs, now + Duration::from_millis(150), None);
        assert!(!native, "failed host creation must not suppress Rust painting"); assert!(update.is_none());
        assert_eq!(report(&hosts, id)["lifetimeNativeFailures"], 1);
        assert!(!prepare(&hosts, id, &specs, now + Duration::from_millis(300), None).0);
        assert_eq!(report(&hosts, id)["lifetimeNativeFailures"], 1, "creation must respect cooldown too");
        hosts.borrow_mut().remove(id);
        println!("PASS: unavailable native parent retains Rust rendering and bounded retries");
    }
    pub fn run() {
        objc2::rc::autoreleasepool(|_| {
            a_failed_last_arc_hide_removes_actual_layers_and_retires_the_host();
            persistent_failures_reach_the_real_queue_then_back_off();
            unavailable_native_parent_keeps_rust_ownership();
        });
    }
}
#[cfg(target_os = "macos")]
fn main() { mac::run(); }
#[cfg(not(target_os = "macos"))]
fn main() { println!("Native arc ownership test uses AppKit; skipped on this platform"); }
