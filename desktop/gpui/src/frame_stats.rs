// How smoothly the window draws, from GPUI's own frame profiler. Only in a
// build with the `frame-stats` feature (the one installed for Mike, and the
// perf runs): View > Show Frame Rate puts a readout in the corner, and the
// automation's `frame-stats` hands the numbers to features/tools/scroll-perf.ts.
//
// GPUI asks macOS for a frame at every refresh of the screen the window is on
// (a CAMetalDisplayLink with the default rate range), so 120 a second on a
// ProMotion screen, 60 on most external ones, and only while something is
// changing. A frame is late when its work, from the screen's signal to handing
// the frame over, took longer than one refresh: that's a hitch you can see.
use crate::style::px;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// Below this many frames a second nothing is moving: the readout's own
/// redraw is twice a second.
const IDLE_FPS: f64 = 4.;
const SAMPLE_EVERY: Duration = Duration::from_millis(500);

#[cfg(feature = "automation")]
struct Baseline {
    window: WindowId,
    frames: FrameDurationSnapshot,
    input: InputLatencySnapshot,
    cpu: u64,
}

#[cfg(feature = "automation")]
thread_local! {
    /// What `frame-stats` with `reset` saw, so the next report is just what
    /// happened since (the perf tool resets before it starts scrolling).
    static BASELINE: RefCell<Option<Baseline>> = const { RefCell::new(None) };
}

// Scripted perf builds count only the marked transcript loader: another
// animation can keep window FPS high even if this loader stops repainting.
#[cfg(feature = "automation")]
thread_local! {
    static LOADERS: RefCell<std::collections::HashMap<WindowId, LoaderPaints>> = RefCell::new(std::collections::HashMap::new());
}
#[cfg(feature = "automation")]
struct LoaderPaints { paints: u64, changes: u64, phase: Option<f32>, last_change: Instant, max_gap: Duration }
#[cfg(feature = "automation")]
impl LoaderPaints {
    fn new(now: Instant) -> Self { Self { paints: 0, changes: 0, phase: None, last_change: now, max_gap: Duration::ZERO } }
    fn note(&mut self, phase: f32, now: Instant) {
        self.paints += 1;
        if self.phase.is_none_or(|old| (old - phase).abs() > 0.0001) {
            self.max_gap = self.max_gap.max(now.saturating_duration_since(self.last_change));
            self.last_change = now;
            self.phase = Some(phase);
            self.changes += 1;
        }
    }
    fn reset(&mut self, now: Instant) { self.paints = 0; self.changes = 0; self.max_gap = Duration::ZERO; self.last_change = now; }
}
#[cfg(feature = "automation")]
pub(crate) fn loader_painted(window: &Window, phase: f32) {
    let now = Instant::now();
    LOADERS.with(|all| all.borrow_mut().entry(window.window_handle().window_id()).or_insert_with(|| LoaderPaints::new(now)).note(phase, now));
}
#[cfg(feature = "automation")]
fn loader_sample(window: &Window, reset: bool) -> Value {
    let now = Instant::now();
    LOADERS.with(|all| {
        let mut all = all.borrow_mut();
        let Some(sample) = all.get_mut(&window.window_handle().window_id()) else { return Value::Null };
        let out = json!({ "source": "transcript", "paints": sample.paints, "phaseChanges": sample.changes,
            "maxGapMs": sample.max_gap.max(now.saturating_duration_since(sample.last_change)).as_secs_f64() * 1000. });
        if reset { sample.reset(now); }
        out
    })
}
#[cfg(all(test, feature = "automation"))]
mod loader_probe_tests {
    use super::LoaderPaints;
    use std::time::{Duration, Instant};
    #[test]
    fn repeated_paints_do_not_count_as_animation_and_a_frozen_tail_is_visible() {
        let now = Instant::now();
        let mut sample = LoaderPaints::new(now);
        sample.note(0., now); sample.reset(now);
        for i in 1..=120 { sample.note(0., now + Duration::from_millis(i * 16)); }
        assert_eq!(sample.paints, 120); assert_eq!(sample.changes, 0);
        sample.note(0.25, now + Duration::from_secs(2));
        assert_eq!(sample.changes, 1); assert_eq!(sample.max_gap, Duration::from_secs(2));
        sample.reset(now + Duration::from_secs(2));
        assert_eq!((sample.paints, sample.changes, sample.max_gap), (0, 0, Duration::ZERO));
        assert_eq!(sample.phase, Some(0.25));
    }
}

/// CPU time this thread has used, in nanoseconds. Asked from the automation's
/// commands, which run on the main thread, so it's the main thread's. It
/// leaves out time spent waiting for a core, which a frame's wall-clock work
/// doesn't, so it moves less with other processes' load; it still moves some
/// (macOS may put the thread on an efficiency core), so builds are compared
/// in turns under the same load (features/tools/render-ab.mjs).
#[cfg(all(feature = "automation", unix))]
pub(crate) fn thread_cpu_nanos() -> u64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid timespec to write into, and a clock every Unix has.
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) } != 0 {
        return 0;
    }
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

#[cfg(all(feature = "automation", not(unix)))]
pub(crate) fn thread_cpu_nanos() -> u64 {
    0
}

#[derive(Default, Clone, Copy)]
struct Counts {
    presented: u64,
    late: u64,
    missed: u64,
}

fn counts(frames: &RefreshIntervalFrames) -> Counts {
    Counts {
        presented: frames.frames_on_time + frames.frames_late + frames.frames_unmeasured,
        late: frames.frames_late,
        missed: frames.missed_refreshes,
    }
}

fn hz(frames: &RefreshIntervalFrames) -> Option<f64> {
    frames.refresh_interval.map(|i| (1. / i.as_secs_f64()).round())
}

fn ms(nanos: u64) -> f64 {
    (nanos as f64 / 1e4).round() / 100.
}

#[cfg(feature = "automation")]
fn dirty_to_present_ms(now: &FrameDurationSnapshot, before: Option<&FrameDurationSnapshot>) -> Value {
    // workMs stops before platform submission. This existing histogram runs
    // from the frame's first invalidation through submission. It includes
    // waiting for the frame signal, drawing,
    // and submission (including a blocked Metal drawable acquisition).
    // It is not pure submission time or the pixels reaching the display.
    // Compare paired runs with the same input; subtracting workMs does not
    // isolate time spent in GPU work or submission.
    let mut durations = now.dirty_to_present_histogram.clone();
    if before.is_some_and(|before| durations.subtract(&before.dirty_to_present_histogram).is_err()) {
        return json!({ "error": "handoff baseline does not belong to this frame history" });
    }
    let samples = durations.len();
    json!({ "samples": samples,
        "p50": (samples > 0).then(|| ms(durations.value_at_quantile(0.5))),
        "p90": (samples > 0).then(|| ms(durations.value_at_quantile(0.9))),
        "p99": (samples > 0).then(|| ms(durations.value_at_quantile(0.99))),
        "max": (samples > 0).then(|| ms(durations.max())) })
}

#[cfg(feature = "automation")]
fn input_to_handoff_ms(now: &InputLatencySnapshot, before: Option<&InputLatencySnapshot>) -> Value {
    // Only frames responding to input, so timer-only animation frames cannot
    // dilute a scroll comparison. Starts when GPUI handles the first input,
    // excluding time queued before dispatch, and ends at platform submission.
    // This is neither event-post-to-frame latency nor pointer-to-glass latency.
    let mut durations = now.latency_histogram.clone();
    if before.is_some_and(|before| durations.subtract(&before.latency_histogram).is_err()) {
        return json!({ "error": "input baseline does not belong to this frame history" });
    }
    let samples = durations.len();
    json!({ "samples": samples,
        "p50": (samples > 0).then(|| ms(durations.value_at_quantile(0.5))),
        "p90": (samples > 0).then(|| ms(durations.value_at_quantile(0.9))),
        "p99": (samples > 0).then(|| ms(durations.value_at_quantile(0.99))),
        "max": (samples > 0).then(|| ms(durations.max())) })
}

#[cfg(all(test, feature = "automation"))]
mod handoff_probe_tests {
    use super::{dirty_to_present_ms, input_to_handoff_ms, report, BASELINE};
    use gpui_kit::{div, size, Context, IntoElement, Render, TestAppContext, Window, WindowId, WindowProfiler};

    #[test]
    fn handoff_duration_excludes_samples_before_the_reset() {
        let mut now = WindowProfiler::new(WindowId::from(1u64)).unwrap().frame_duration_snapshot();
        now.dirty_to_present_histogram.record(100_000_000).unwrap();
        let before = now.clone();
        now.dirty_to_present_histogram.record(19_000_000).unwrap();
        now.dirty_to_present_histogram.record(27_000_000).unwrap();
        let report = dirty_to_present_ms(&now, Some(&before));
        assert_eq!(report["samples"], 2);
        assert!((report["p50"].as_f64().unwrap() - 19.).abs() < 0.05);
        assert!((report["max"].as_f64().unwrap() - 27.).abs() < 0.05);
    }

    #[test]
    fn an_out_of_range_handoff_baseline_is_reported_as_an_error() {
        let now = WindowProfiler::new(WindowId::from(2u64)).unwrap().frame_duration_snapshot();
        let mut before = now.clone();
        before.dirty_to_present_histogram.record(100_000_000).unwrap();
        assert!(dirty_to_present_ms(&now, Some(&before))["error"].is_string());
    }

    #[test]
    fn a_negative_handoff_bucket_is_reported_as_an_error() {
        let mut now = WindowProfiler::new(WindowId::from(3u64)).unwrap().frame_duration_snapshot();
        now.dirty_to_present_histogram.record(9_000_000).unwrap();
        let mut before = now.clone();
        before.dirty_to_present_histogram.record(9_000_000).unwrap();
        // The baseline fits the range, but subtracting its bucket would be negative.
        now.dirty_to_present_histogram.record(100_000_000).unwrap();
        assert!(dirty_to_present_ms(&now, Some(&before))["error"].is_string());
    }

    #[test]
    fn no_handoff_samples_are_not_reported_as_zero_latency() {
        let now = WindowProfiler::new(WindowId::from(4u64)).unwrap().frame_duration_snapshot();
        let report = dirty_to_present_ms(&now, None);
        assert_eq!(report["samples"], 0);
        for field in ["p50", "p90", "p99", "max"] { assert!(report[field].is_null(), "{field}"); }
    }

    #[test]
    fn input_handoff_uses_only_input_samples_after_the_reset() {
        let mut now = WindowProfiler::new(WindowId::from(5u64)).unwrap().input_latency_snapshot();
        now.latency_histogram.record(100_000_000).unwrap();
        let before = now.clone();
        now.latency_histogram.record(19_000_000).unwrap();
        now.latency_histogram.record(27_000_000).unwrap();
        let report = input_to_handoff_ms(&now, Some(&before));
        assert_eq!(report["samples"], 2);
        assert!((report["p50"].as_f64().unwrap() - 19.).abs() < 0.05);
        assert!((report["max"].as_f64().unwrap() - 27.).abs() < 0.05);
    }

    #[test]
    fn no_input_handoff_samples_are_not_zero_latency() {
        let now = WindowProfiler::new(WindowId::from(6u64)).unwrap().input_latency_snapshot();
        let report = input_to_handoff_ms(&now, None);
        assert_eq!(report["samples"], 0);
        assert!(report["p50"].is_null());
    }

    struct Blank;
    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() }
    }

    #[gpui_kit::test]
    fn a_reset_on_another_window_is_rejected_even_when_histograms_are_compatible(cx: &mut TestAppContext) {
        let a = cx.open_window(size(gpui_kit::px(20.), gpui_kit::px(20.)), |_, _| Blank);
        let b = cx.open_window(size(gpui_kit::px(20.), gpui_kit::px(20.)), |_, _| Blank);
        cx.run_until_parked();
        a.update(cx, |_, window, _| { report(window, true); }).unwrap();
        let b_frames = b.update(cx, |_, window, _| window.frame_duration_snapshot()).unwrap();
        // Make the counts fully compatible: subtraction alone cannot identify a window.
        BASELINE.with(|baseline| baseline.borrow_mut().as_mut().unwrap().frames = b_frames);
        let refused = b.update(cx, |_, window, _| report(window, false)).unwrap();
        assert!(refused["error"].is_string(), "a report must not subtract another window's reset");
        assert!(a.update(cx, |_, window, _| report(window, false)).unwrap()["error"].is_null());
        b.update(cx, |_, window, _| { report(window, true); }).unwrap();
        assert!(b.update(cx, |_, window, _| report(window, false)).unwrap()["error"].is_null());
        assert!(a.update(cx, |_, window, _| report(window, false)).unwrap()["error"].is_string());
    }
}

/// The same displays' frames in `now`, less what `before` had of them.
fn since(now: &FrameDurationSnapshot, before: Option<&FrameDurationSnapshot>) -> Vec<(RefreshIntervalFrames, Counts)> {
    now.by_refresh_interval
        .iter()
        .map(|frames| {
            let earlier = before.and_then(|b| b.by_refresh_interval.iter().find(|f| f.refresh_interval == frames.refresh_interval));
            let mut frames = frames.clone();
            let mut c = counts(&frames);
            if let Some(earlier) = earlier {
                let e = counts(earlier);
                c = Counts {
                    presented: c.presented.saturating_sub(e.presented),
                    late: c.late.saturating_sub(e.late),
                    missed: c.missed.saturating_sub(e.missed),
                };
                // Histograms only gain counts, so taking the earlier one away
                // leaves exactly the frames since.
                let _ = frames.work.subtract(&earlier.work);
                let _ = frames.draw_duration.subtract(&earlier.draw_duration);
                frames.frames_drawn = frames.frames_drawn.saturating_sub(earlier.frames_drawn);
            }
            (frames, c)
        })
        .filter(|(_, c)| c.presented > 0)
        .collect()
}

/// `frame-stats`: per screen rate the window drew at, the frames and how long
/// their work took, since the last `reset` (or since it opened).
#[cfg(feature = "automation")]
pub(crate) fn report(window: &Window, reset: bool) -> Value {
    let id = window.window_handle().window_id();
    if !reset && BASELINE.with(|b| b.borrow().as_ref().is_some_and(|b| b.window != id)) {
        return json!({ "error": "frame-stats was reset on another window; reset this window before sampling" });
    }
    let now = window.frame_duration_snapshot();
    let input = window.input_latency_snapshot();
    let out = BASELINE.with(|b| {
        let b = b.borrow();
        let before = b.as_ref().filter(|b| b.window == id).map(|b| &b.frames);
        let screens: Vec<Value> = since(&now, before)
            .iter()
            .map(|(f, c)| {
                let w = &f.work;
                let d = &f.draw_duration;
                json!({
                    "hz": hz(f),
                    "drawn": f.frames_drawn,
                    "presented": c.presented,
                    "late": c.late,
                    "missedRefreshes": c.missed,
                    "workMs": { "p50": ms(w.value_at_quantile(0.5)), "p90": ms(w.value_at_quantile(0.9)), "p99": ms(w.value_at_quantile(0.99)), "max": ms(w.max()) },
                    "drawMs": { "p50": ms(d.value_at_quantile(0.5)), "p90": ms(d.value_at_quantile(0.9)), "p99": ms(d.value_at_quantile(0.99)), "max": ms(d.max()) },
                })
            })
            .collect();
        let input_before = b.as_ref().filter(|b| b.window == id).map(|b| &b.input);
        json!({ "screens": screens, "dirtyToPresentMs": dirty_to_present_ms(&now, before),
            "inputHandledToHandoffMs": input_to_handoff_ms(&input, input_before) })
    });
    let cpu = thread_cpu_nanos();
    let mut out = out;
    out["windowActive"] = json!(window.is_window_active());
    out["loader"] = loader_sample(window, reset);
    let cpu_before = BASELINE.with(|b| b.borrow().as_ref().filter(|b| b.window == id).map_or(0, |b| b.cpu));
    out["mainCpuMs"] = json!(ms(cpu.saturating_sub(cpu_before)));
    if reset {
        BASELINE.with(|b| *b.borrow_mut() = Some(Baseline { window: id, frames: now, input, cpu }));
    }
    out
}

/// View > Show Frame Rate: frames a second over the last half second, the
/// frames in it that were late, and the slowest one's work.
pub(crate) struct FrameRate {
    last: Option<(Instant, FrameDurationSnapshot)>,
    text: SharedString,
    _sampler: Task<()>,
}

impl FrameRate {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sampler = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(SAMPLE_EVERY).await;
            if this.update_in(cx, |this, window, cx| this.sample(window, cx)).is_err() {
                break;
            }
        });
        Self { last: None, text: "Measuring…".into(), _sampler: sampler }
    }

    fn sample(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = window.frame_duration_snapshot();
        let at = Instant::now();
        if let Some((then, before)) = &self.last {
            let seconds = at.duration_since(*then).as_secs_f64();
            let screens = since(&now, Some(before));
            let c = screens.iter().fold(Counts::default(), |a, (_, c)| Counts {
                presented: a.presented + c.presented,
                late: a.late + c.late,
                missed: a.missed + c.missed,
            });
            // The rate of the screen it drew most of those frames on.
            let screen = screens.iter().max_by_key(|(_, c)| c.presented).and_then(|(f, _)| hz(f));
            let slowest = screens.iter().map(|(f, _)| f.work.max()).max().unwrap_or(0);
            let fps = c.presented as f64 / seconds;
            let of = screen.map(|hz| format!(" of {hz:.0}")).unwrap_or_default();
            self.text = if fps < IDLE_FPS {
                match screen.or_else(|| now.by_refresh_interval.last().and_then(hz)) {
                    Some(hz) => format!("Nothing moving · {hz:.0} Hz screen").into(),
                    None => "Nothing moving".into(),
                }
            } else {
                format!("{fps:.0} fps{of} · {} late · slowest {:.1} ms", c.late, ms(slowest)).into()
            };
            cx.notify();
        }
        self.last = Some((at, now));
    }
}

impl Render for FrameRate {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .bottom(px(10.))
            .right(px(10.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(gpui_kit::black().opacity(0.72))
            .text_color(gpui_kit::white())
            .text_size(px(11.))
            .font_family(cx.theme().mono_font_family.clone())
            .child(self.text.clone())
    }
}
