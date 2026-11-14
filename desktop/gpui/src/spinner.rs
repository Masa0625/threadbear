// The turning arc a working thread shows in the sidebar (Spinner.swift's
// SpinnerView): three quarters of a ring with round ends, once round every
// 0.9 s, clockwise.
//
// The Swift app turns it in the render server, with nothing for the main
// thread to do. GPUI turns a thing by drawing it again, and a request for the
// next frame from inside a view has that view and every view it's in (the
// sidebar, the dock, the window, the open transcript) draw again. The kit's
// spinner asks every frame, so while any thread worked the whole window drew
// at the screen's rate, 120 times a second on Mike's MacBook, and a debug
// build spent all its time there. This one waits 20 ms after it's drawn
// before asking again, one timer for all the arcs a view has: with the wait
// for the screen's next frame on top, about 30 frames a second, which reads
// as smooth at this size. (33 ms came to 20 a second, a visible step.)
//
// Even at that rate a turn redrew the whole sidebar, building every row: a
// third of a core while any thread worked (Mike, 13 Nov: "everything does
// seem to slow down a bit once the system loads up"). So the sidebar's arcs
// (`layered`) aren't drawn where they sit. Each notes its place, clip, colour
// and opacity as its row is drawn, and the window's `ArcLayer`, a small view
// of its own drawn last at the window's root as the archive flight is, turns
// them all. A turn asks only that view to draw again: the root draws with it,
// and the sidebar's cached drawing is used as it was. (Asking the app's own
// view would have the sidebar, which watches it, draw again every turn too.)
//
// A sidebar drawing again forgets its arcs and places the ones it still has,
// so a row whose arc goes (its thread done, hovered) loses it then. One taken
// off screen never draws to say so; the layer sees that in the dock (see
// `ArcLayer`). Asking the sidebar to draw to show it's still there didn't
// work: GPUI passes an ask on only to a window it knows shows the view, and
// can lose track of a cached one (Codex, 14 Nov).
use crate::style::{self, px};
use gpui_kit::component::{Icon, Sizable, Size};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const TURN_SECONDS: f64 = 0.9;
const FRAME: Duration = Duration::from_millis(20);

#[cfg(test)]
thread_local! {
    static RENDERS: [Cell<u32>; 2] = const { [Cell::new(0), Cell::new(0)] };
}
/// How much of the sidebar list's foot fades into the background
/// (row_motion.rs's FOOT_FADE): an arc in it fades as its row does.
const FOOT_FADE: f32 = 34.;

#[derive(IntoElement)]
pub(crate) struct TurningArc {
    side: f32,
    color: Hsla,
    layered: bool,
}

impl TurningArc {
    /// `side` in points, before the zoom.
    pub(crate) fn new(side: f32, color: Hsla) -> Self {
        Self { side, color, layered: false }
    }

    /// Turned by the window's layer rather than by drawing its view again:
    /// for a row in the sidebar's list, whose foot fades. Not for an arc in
    /// something drawn over the window's content (a card, a menu), which the
    /// layer would turn under it, and only in a view that calls
    /// `view_drawing` as it draws.
    pub(crate) fn layered(mut self) -> Self {
        self.layered = true;
        self
    }
}

impl RenderOnce for TurningArc {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        #[cfg(test)]
        RENDERS.with(|counts| counts[0].set(counts[0].get() + 1));
        let (color, layered) = (self.color, self.layered);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let id = window.window_handle().window_id();
                // Layered only where its view has said it's drawing this frame
                // (`view_drawing`), so it clears them next time: not in the
                // dock's drag ghost, which draws the sidebar under the dock's
                // name and turns its arcs itself as it's drawn every frame.
                if layered && LAYERED.with(|l| l.borrow().contains(&id)) && drawing_now(id, window.current_view()) {
                    // `opacity` scales the alpha: the row's own opacity, once.
                    let color = color.opacity(window.element_opacity());
                    place(id, window.current_view(), bounds, window.content_mask(), color);
                } else {
                    // A window with no layer (a panel in one of its own)
                    // turns its arcs itself, redrawing their view.
                    paint_arc(bounds, color, window);
                    next_frame(window, cx);
                }
            },
        )
        .flex_none()
        .size(px(self.side))
    }
}

fn paint_arc(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let turned = arc_phase() as f32;
    // The ring as Swift's: 1.6 wide, 3 in from the sides, zoomed like an
    // icon's strokes.
    let width = style::zoomed(1.6);
    let side = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
    let radius = ((side - style::zoomed(3.)) / 2.).max(1.);
    let center = bounds.center();
    let at = |angle: f32| point(center.x + gpui_kit::px(radius * angle.cos()), center.y + gpui_kit::px(radius * angle.sin()));
    let start = turned * std::f32::consts::TAU;
    let end = start + 0.75 * std::f32::consts::TAU;
    let mut path = PathBuilder::stroke(gpui_kit::px(width));
    path.move_to(at(start));
    // Clockwise on screen (y runs down), the long way round.
    path.arc_to(point(gpui_kit::px(radius), gpui_kit::px(radius)), gpui_kit::px(0.), true, true, at(end));
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
    // Round ends: lyon's caps aren't GPUI's to set, so a dot at each.
    for angle in [start, end] {
        let c = at(angle);
        let dot = Bounds { origin: point(c.x - gpui_kit::px(width / 2.), c.y - gpui_kit::px(width / 2.)), size: size(gpui_kit::px(width), gpui_kit::px(width)) };
        window.paint_quad(fill(dot, color).corner_radii(gpui_kit::px(width / 2.)));
    }
}

fn arc_phase() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    (START.get_or_init(Instant::now).elapsed().as_secs_f64() / TURN_SECONDS).fract()
}

thread_local! {
    #[cfg(feature = "frame-stats")]
    static RUST_ARC_PAINTS: RefCell<HashMap<WindowId, u64>> = RefCell::new(HashMap::new());
    static RASTER_CAPTURE: Cell<bool> = const { Cell::new(false) };
    /// The views with a frame on its way for their arcs.
    static TICKING: RefCell<HashSet<EntityId>> = RefCell::new(HashSet::new());
    /// The windows with an arc layer. App's actual window list retires them;
    /// an open, quiet window need not redraw to prove that it is alive.
    static LAYERED: RefCell<HashSet<WindowId>> = RefCell::new(HashSet::new());
    /// The arcs each window's layer draws.
    static PLACED: RefCell<HashMap<WindowId, Vec<Placed>>> = RefCell::new(HashMap::new());
    /// Counts the frames layers draw: one ends as its layer's paint does.
    static FRAMES: Cell<u64> = const { Cell::new(0) };
    /// The frame each view last said it was drawing in, by window (the
    /// same sidebar can be in a window of its own).
    static DRAWN: RefCell<HashMap<(WindowId, EntityId), u64>> = RefCell::new(HashMap::new());
}

/// An arc as its view last drew it: where, clipped to what, in what colour
/// (its row's opacity in it).
#[derive(Clone, Debug, PartialEq)]
struct Placed {
    view: EntityId,
    bounds: Bounds<Pixels>,
    mask: ContentMask<Pixels>,
    color: Hsla,
}

/// A view with layered arcs is drawing: it places the ones it still has as it
/// paints, so the ones from before go. Call it from the view's render.
pub(crate) fn view_drawing(window: &Window) {
    let (id, view) = (window.window_handle().window_id(), window.current_view());
    DRAWN.with(|d| d.borrow_mut().insert((id, view), FRAMES.get()));
    PLACED.with(|p| {
        if let Some(arcs) = p.borrow_mut().get_mut(&id) {
            arcs.retain(|a| a.view != view);
        }
    });
}

/// Whether `view` has said it's drawing in this window in this frame.
fn drawing_now(window: WindowId, view: EntityId) -> bool {
    DRAWN.with(|d| d.borrow().get(&(window, view)).is_some_and(|frame| *frame == FRAMES.get()))
}

fn place(window: WindowId, view: EntityId, bounds: Bounds<Pixels>, mask: ContentMask<Pixels>, color: Hsla) {
    PLACED.with(|p| p.borrow_mut().entry(window).or_default().push(Placed { view, bounds, mask, color }));
}

/// The window's arcs, turned: drawn last over the window's own content, and
/// under anything deferred (menus, tooltips, the archive flight). One per
/// window, after the dock in its root view.
///
/// The sidebar stops being drawn only when the dock changes, and it can't
/// say so, so the layer looks at the dock as it paints: with the Threads
/// panel not in front in this window (another tab, closed, or out in a window
/// of its own) the window's arcs go. A sidebar moved or resized draws again
/// and places its own. While a panel's dragged, or its ghost lands, the arcs
/// aren't drawn: the ghost is drawn in the dock, under them.
pub(crate) struct ArcLayer {
    dock: Entity<threadbear_dock::DockModel>,
    dock_view: Entity<threadbear_dock::DockView>,
    /// The panel the arcs are in.
    panel: &'static str,
    // The canvas keeps this alive while its frame is retained; closing the
    // window drops the last owner and removes its native layers immediately.
    #[cfg(all(target_os = "macos", feature = "automation"))]
    native: std::rc::Rc<RefCell<arc_probe::Hosts>>,
    #[cfg(all(target_os = "macos", feature = "automation"))]
    native_closed: Option<Subscription>,
    #[cfg(all(target_os = "macos", feature = "automation"))]
    native_blocked: bool,
}

impl ArcLayer {
    #[cfg(all(feature = "automation", feature = "frame-stats"))]
    pub(crate) fn native_report(&self, window: &Window, reset: bool) -> serde_json::Value {
        #[cfg(target_os = "macos")]
        let mut report = self.native.borrow().report(window.window_handle().window_id());
        #[cfg(not(target_os = "macos"))]
        let mut report = serde_json::json!({"requested": false, "backend": "rust"});
        let paints = RUST_ARC_PAINTS.with(|paints| {
            let mut paints = paints.borrow_mut();
            let entry = paints.entry(window.window_handle().window_id()).or_default();
            let count = *entry;
            if reset { *entry = 0; }
            count
        });
        report["fallbackPaints"] = serde_json::json!(paints);
        report
    }
    #[cfg(all(target_os = "macos", feature = "automation"))]
    pub(crate) fn block_native(&mut self, blocked: bool) { self.native_blocked = blocked; }
    pub(crate) fn new(dock: Entity<threadbear_dock::DockModel>, dock_view: Entity<threadbear_dock::DockView>, panel: &'static str) -> Self {
        Self {
            dock, dock_view, panel,
            #[cfg(all(target_os = "macos", feature = "automation"))]
            native: Default::default(),
            #[cfg(all(target_os = "macos", feature = "automation"))]
            native_closed: None,
            #[cfg(all(target_os = "macos", feature = "automation"))]
            native_blocked: false,
        }
    }
}

/// The scripted image reads Metal's scene, which does not include native
/// layers. Paint the arc into that scene, then its normal next turn restores
/// native animation at the same phase. No Screen Recording grant is needed.
#[cfg(feature = "automation")]
pub(crate) fn raster_capture<R>(capture: impl FnOnce() -> R) -> R {
    RASTER_CAPTURE.with(|flag| {
        struct Restore<'a>(&'a Cell<bool>, bool);
        impl Drop for Restore<'_> { fn drop(&mut self) { self.0.set(self.1); } }
        let _restore = Restore(flag, flag.replace(true));
        capture()
    })
}

impl Render for ArcLayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(all(target_os = "macos", feature = "automation"))]
        if self.native_closed.is_none() {
            let native = std::rc::Rc::downgrade(&self.native);
            self.native_closed = Some(cx.on_window_closed(move |_, id| {
                if let Some(native) = native.upgrade() { native.borrow_mut().remove(id); }
                LAYERED.with(|l| l.borrow_mut().remove(&id));
                PLACED.with(|p| p.borrow_mut().remove(&id));
                DRAWN.with(|d| d.borrow_mut().retain(|(window, _), _| *window != id));
                #[cfg(feature = "frame-stats")]
                RUST_ARC_PAINTS.with(|p| p.borrow_mut().remove(&id));
            }));
        }
        let _ = cx;
        let (dock, dock_view, panel) = (self.dock.clone(), self.dock_view.clone(), self.panel);
        #[cfg(all(target_os = "macos", feature = "automation"))]
        let native = self.native.clone();
        #[cfg(all(target_os = "macos", feature = "automation"))]
        let blocked = self.native_blocked;
        canvas(
            |_, window, _| {
                LAYERED.with(|l| l.borrow_mut().insert(window.window_handle().window_id()));
            },
            move |_, _, window, cx| {
                // As the dock draws it: a collapsed sidebar (⌘/) is left out,
                // and a tab with nothing to show gives way to the next.
                let in_front = dock_view.read(cx).shows(panel, cx);
                let dragging = dock.read(cx).controller.drag.is_some() || dock_view.read(cx).is_landing();
                if !in_front {
                    PLACED.with(|p| p.borrow_mut().remove(&window.window_handle().window_id()));
                }
                paint_layer(!dragging, window, cx,
                    #[cfg(all(target_os = "macos", feature = "automation"))]
                    &native,
                    #[cfg(all(target_os = "macos", feature = "automation"))]
                    blocked,
                )
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// How much of an arc shows at the sidebar list's foot, where its rows fade
/// into the background: `bottom` is the list's, `y` the arc's centre.
fn foot_shown(bottom: f32, y: f32) -> f32 {
    ((bottom - y) / FOOT_FADE).clamp(0., 1.)
}

fn paint_layer(draw: bool, window: &mut Window, cx: &mut App,
    #[cfg(all(target_os = "macos", feature = "automation"))]
    native: &std::rc::Rc<RefCell<arc_probe::Hosts>>,
    #[cfg(all(target_os = "macos", feature = "automation"))]
    blocked: bool,
) {
    let id = window.window_handle().window_id();
    let frame = FRAMES.get();
    // Closed windows' arcs go.
    let open: HashSet<WindowId> = cx.windows().into_iter().map(|w| w.window_id()).collect();
    LAYERED.with(|l| l.borrow_mut().retain(|id| open.contains(id)));
    let arcs = PLACED.with(|p| {
        let mut p = p.borrow_mut();
        p.retain(|w, _| open.contains(w));
        p.get(&id).cloned().unwrap_or_default()
    });
    // What's kept of views that drew no arcs, or not lately, goes.
    let placed: HashSet<(WindowId, EntityId)> = PLACED.with(|p| p.borrow().iter().flat_map(|(w, arcs)| arcs.iter().map(move |a| (*w, a.view))).collect());
    DRAWN.with(|d| d.borrow_mut().retain(|key, drew| *drew == frame || placed.contains(key)));
    // This native-overlay experiment is available only in scripted builds.
    // It intentionally has no production path until overlay ordering is proved.
    #[cfg(all(target_os = "macos", feature = "automation"))]
    let native = if arc_probe::enabled() {
        use gpui_kit::component::WindowExt as _;
        let gates = [!draw, blocked, RASTER_CAPTURE.get(), window.has_deferred_draws(),
            window.has_tooltip(), window.has_component_overlay(cx), window.has_active_prompt(), cx.has_active_drag()];
        let flags = gates.into_iter().enumerate().fold(0, |flags, (i, blocked)| flags | (u32::from(blocked) << i));
        let (draw_native, update) = native.borrow_mut().prepare(&arcs, flags, &open, window);
        if let Some(update) = update {
            let native = native.clone();
            window.defer_until_present(move || native.borrow_mut().apply(update));
        }
        draw_native
    } else { false };
    #[cfg(not(all(target_os = "macos", feature = "automation")))]
    let native = false;
    #[cfg(feature = "frame-stats")]
    RUST_ARC_PAINTS.with(|paints| {
        let mut paints = paints.borrow_mut();
        paints.retain(|id, _| open.contains(id));
        if !native && !arcs.is_empty() { *paints.entry(id).or_default() += 1; }
    });
    for arc in arcs.iter().filter(|_| draw && !native) {
        let shown = foot_shown(f32::from(arc.mask.bounds.bottom()), f32::from(arc.bounds.center().y));
        let color = arc.color.opacity(shown);
        window.with_content_mask(Some(arc.mask.clone()), |window| paint_arc(arc.bounds, color, window));
    }
    FRAMES.set(frame + 1);
    // The next turn: this layer's view draws again (and the root with it),
    // not the sidebar.
    if !arcs.is_empty() && !native {
        next_frame(window, cx);
    }
}

#[cfg(all(target_os = "macos", feature = "automation"))]
mod arc_probe {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, PartialEq)]
    struct Spec {
        center_x: f32, center_y: f32, side: f32, radius: f32, width: f32,
        clip_x: f32, clip_y: f32, clip_w: f32, clip_h: f32,
        r: f32, g: f32, b: f32, a: f32,
    }
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
    impl Drop for Host {
        fn drop(&mut self) { unsafe { tb_arc_probe_drop(self.0); } }
    }
    #[derive(Default)]
    pub(super) struct Hosts {
        hosts: HashMap<WindowId, Host>,
        gates: HashMap<WindowId, u32>,
        committed: HashMap<WindowId, (Vec<Spec>, f32)>,
    }
    pub(super) struct Update {
        id: WindowId,
        specs: Vec<Spec>,
        scale: f32,
        flags: u32,
    }

    pub(super) fn enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("TB_PERF_NATIVE_ARCS").as_deref() == Ok("on"))
    }

    impl Hosts {
        pub(super) fn remove(&mut self, id: WindowId) { self.hosts.remove(&id); self.gates.remove(&id); self.committed.remove(&id); }
        pub(super) fn report(&self, id: WindowId) -> serde_json::Value {
            let mut health = Health::default();
            if let Some(host) = self.hosts.get(&id) {
                unsafe { tb_arc_probe_snapshot(host.0, &mut health); }
            }
            let flags = self.gates.get(&id).copied().unwrap_or_default();
            let names = ["dock-drag", "app-overlay", "capture", "deferred", "tooltip", "component-overlay", "prompt", "drag"];
            let blocked: Vec<_> = names.into_iter().enumerate().filter_map(|(i, name)| (flags & (1 << i) != 0).then_some(name)).collect();
            serde_json::json!({"requested": enabled(), "backend": if health.visible > 0 { "core-animation" } else { "rust" },
                "blockedBy": blocked, "placed": health.placed, "visible": health.visible, "animating": health.animating,
                "presented": health.presented, "phase": health.phase, "periodSeconds": health.period,
                "expectedPeriodSeconds": TURN_SECONDS, "snapshotSeconds": health.time})
        }
        // ArcLayer is uncached and paints on every draw. Preparation may create
        // an empty host, but visible mutations wait for the matching GPU scene.
        pub(super) fn prepare(&mut self, arcs: &[Placed], flags: u32, open: &HashSet<WindowId>, window: &Window) -> (bool, Option<Update>) {
            let id = window.window_handle().window_id();
            let draw = flags == 0;
            self.gates.retain(|key, _| open.contains(key));
            self.hosts.retain(|key, _| open.contains(key));
            self.committed.retain(|key, _| open.contains(key));
            let specs: Vec<Spec> = arcs.iter().filter(|_| draw).map(|arc| {
                let side = f32::from(arc.bounds.size.width).min(f32::from(arc.bounds.size.height));
                let color: Rgba = arc.color.opacity(foot_shown(f32::from(arc.mask.bounds.bottom()), f32::from(arc.bounds.center().y))).into();
                Spec {
                    center_x: arc.bounds.center().x.into(), center_y: arc.bounds.center().y.into(), side,
                    radius: ((side - style::zoomed(3.)) / 2.).max(1.), width: style::zoomed(1.6),
                    clip_x: arc.mask.bounds.origin.x.into(), clip_y: arc.mask.bounds.origin.y.into(),
                    clip_w: arc.mask.bounds.size.width.into(), clip_h: arc.mask.bounds.size.height.into(),
                    r: color.r, g: color.g, b: color.b, a: color.a,
                }
            }).collect();
            if specs.is_empty() && !self.hosts.contains_key(&id) {
                self.gates.insert(id, flags);
                return (draw, None);
            }
            if !self.hosts.contains_key(&id) {
                let Some(parent) = crate::web_panels::native_view(window) else { return (false, None); };
                let raw = unsafe { tb_arc_probe_create(parent) };
                if raw.is_null() { return (false, None); }
                self.hosts.insert(id, Host(raw));
            }
            let scale = window.scale_factor();
            if self.committed.get(&id).is_some_and(|(old, old_scale)| *old == specs && *old_scale == scale) {
                self.gates.insert(id, flags);
                return (draw, None);
            }
            (draw, Some(Update { id, specs, scale, flags }))
        }

        pub(super) fn apply(&mut self, update: Update) {
            // Closing the window may have removed the host after this draw.
            let Some(host) = self.hosts.get(&update.id) else { return; };
            unsafe { tb_arc_probe_set(host.0, update.specs.as_ptr(), update.specs.len(), arc_phase(), TURN_SECONDS); }
            self.gates.insert(update.id, update.flags);
            self.committed.insert(update.id, (update.specs, update.scale));
        }
    }
}

/// The view this arc is in draws again in a moment, once for all its arcs.
fn next_frame(window: &mut Window, cx: &mut App) {
    let view = window.current_view();
    if !TICKING.with(|t| t.borrow_mut().insert(view)) {
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(FRAME).await;
        TICKING.with(|t| t.borrow_mut().remove(&view));
        let _ = cx.update(|cx| cx.notify(view));
    })
    .detach();
}

#[cfg(test)]
mod tests {
    // By name: a glob would bring in GPUI's own `test` attribute in place of Rust's.
    use super::{foot_shown, FOOT_FADE};

    #[test]
    fn an_arc_fades_with_its_row_at_the_lists_foot() {
        assert_eq!(foot_shown(500., 400.), 1., "well above the foot: all there");
        assert_eq!(foot_shown(500., 500. - FOOT_FADE / 2.), 0.5);
        assert_eq!(foot_shown(500., 500.), 0., "at the very bottom: gone into the background");
    }
}

/// The kit's sizing and icons. The asymmetric ring keeps its direction clear
/// when a busy main thread drops a frame.
#[derive(IntoElement)]
pub(crate) struct Spinner {
    size: Size,
    color: Option<Hsla>,
    icon: Icon,
    #[cfg(all(feature = "automation", feature = "frame-stats"))]
    transcript_loader: bool,
}

impl Spinner {
    pub(crate) fn new() -> Self {
        Self { size: Size::Medium, color: None, icon: Icon::new(gpui_kit::assets::IconName::LoaderCircle),
            #[cfg(all(feature = "automation", feature = "frame-stats"))]
            transcript_loader: false,
        }
    }

    /// Only this loader can satisfy the transcript's performance liveness gate.
    /// Other busy controls must not hide a frozen transcript behind their frames.
    pub(crate) fn transcript_loader(self) -> Self {
        #[cfg(all(feature = "automation", feature = "frame-stats"))]
        { let mut spinner = self; spinner.transcript_loader = true; spinner }
        #[cfg(not(all(feature = "automation", feature = "frame-stats")))]
        { self }
    }

    pub(crate) fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub(crate) fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = icon.into();
        self
    }
}

impl Sizable for Spinner {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        #[cfg(test)]
        RENDERS.with(|counts| counts[1].set(counts[1].get() + 1));
        let side = match self.size {
            Size::Size(side) => side,
            Size::XSmall => window.rem_size() * 0.75,
            Size::Small => window.rem_size() * 0.875,
            Size::Medium => window.rem_size(),
            Size::Large => window.rem_size() * 1.5,
        };
        static START: OnceLock<Instant> = OnceLock::new();
        let phase = if cx.reduce_motion() { 0. } else { (START.get_or_init(Instant::now).elapsed().as_secs_f64() / 0.8).fract() as f32 };
        let body = self.icon.with_size(self.size)
            .when_some(self.color, |icon, color| icon.text_color(color))
            .transform(Transformation::rotate(percentage(phase)));
        #[cfg(all(feature = "automation", feature = "frame-stats"))]
        let transcript_loader = self.transcript_loader;
        div().relative().flex_none().size(side).child(body)
            .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| {
                if bounds.intersects(&window.content_mask().bounds) {
                    #[cfg(all(feature = "automation", feature = "frame-stats"))]
                    if transcript_loader { crate::frame_stats::loader_painted(window, phase); }
                    if !cx.reduce_motion() { next_spinner_frame(window, cx); }
                }
            }).absolute().inset_0())
    }
}

#[derive(Default)]
struct SpinnerClock {
    pending: HashMap<WindowId, SpinnerFrame>,
    serial: u64,
}
impl Global for SpinnerClock {}
struct SpinnerFrame { views: HashSet<EntityId>, due: Instant, serial: u64 }

fn init_spinner_clock(cx: &mut App) {
    if cx.has_global::<SpinnerClock>() { return; }
    cx.set_global(SpinnerClock::default());
    cx.on_window_closed(|cx, id| { cx.global_mut::<SpinnerClock>().pending.remove(&id); }).detach();
}

// Share the existing display link. Early callbacks rearm without dirtying a
// view; a separate background timer adds a wakeup between drawn frames.
fn next_spinner_frame(window: &mut Window, cx: &mut App) {
    init_spinner_clock(cx);
    let id = window.window_handle().window_id();
    let view = window.current_view();
    let now = cx.background_executor().now();
    let clock = cx.global_mut::<SpinnerClock>();
    let register = clock.pending.get(&id).is_none_or(|frame| now > frame.due + FRAME * 3);
    if register {
        // Recover if GPUI discarded a callback after a failed window update.
        // A generation prevents an old callback from driving the new chain.
        let views = clock.pending.remove(&id).map(|frame| frame.views).unwrap_or_default();
        clock.serial += 1;
        // A cached sibling may not paint during this draw. Keep its demand
        // when replacing a late callback, or its spinner would stop turning.
        clock.pending.insert(id, SpinnerFrame { views, due: now + FRAME, serial: clock.serial });
    }
    let frame = clock.pending.get_mut(&id).unwrap();
    frame.views.insert(view);
    if register {
        let serial = frame.serial;
        window.on_next_frame(move |window, cx| present_spinners(serial, window, cx));
    }
}

fn present_spinners(serial: u64, window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let now = cx.background_executor().now();
    let pending = &mut cx.global_mut::<SpinnerClock>().pending;
    let Some(frame) = pending.get(&id) else { return };
    if frame.serial != serial { return; }
    if now < frame.due {
        window.on_next_frame(move |window, cx| present_spinners(serial, window, cx));
        return;
    }
    let frame = pending.remove(&id).unwrap();
    for view in frame.views { cx.notify(view); }
}

#[cfg(all(test, feature = "automation"))]
mod spinner_tests {
    use super::{SpinnerClock, SpinnerFrame, Spinner, TurningArc, FRAME, RENDERS, TICKING, init_spinner_clock};
    use gpui_kit::component::Sizable as _;
    use gpui_kit::prelude::FluentBuilder as _;
    use gpui_kit::{div, hsla, px, size, AppContext as _, Context, Entity, EntityId, IntoElement, ParentElement, Render, StyleRefinement, Styled as _, TestAppContext, Window, WindowHandle, WindowId};
    use std::{cell::Cell, collections::HashSet, rc::Rc};
    struct SpinnerHost {
        arc: bool,
        visible: bool,
        clipped: bool,
    }

    impl Render for SpinnerHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let spinner = if self.arc {
                TurningArc::new(12., hsla(0., 0., 1., 1.)).into_any_element()
            } else {
                Spinner::new().with_size(px(16.)).color(hsla(0., 0., 1., 1.)).into_any_element()
            };
            div().size_full().overflow_hidden().when(self.visible, |d| d.child(div().relative().left(px(if self.clipped { 1000. } else { 0. })).child(spinner).when(!self.arc, |d| d.child(Spinner::new().with_size(px(12.))))))
        }
    }

    struct TwoViews([Entity<SpinnerHost>; 2]);

    impl Render for TwoViews {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            // Exercise the real cached-parent path as well as normal rendering.
            div().children(self.0.iter().map(|v| v.clone().cached(StyleRefinement::default().size(px(24.)))))
        }
    }

    fn two_spinners(cx: &mut Context<TwoViews>) -> TwoViews {
        // Main's arc timer uses thread-local state across test contexts.
        TICKING.with(|ticks| ticks.borrow_mut().clear());
        RENDERS.with(|counts| counts.iter().for_each(|count| count.set(0)));
        TwoViews([
            cx.new(|_| SpinnerHost { arc: true, visible: true, clipped: false }),
            cx.new(|_| SpinnerHost { arc: false, visible: true, clipped: false }),
        ])
    }


    fn two_loaders(cx: &mut Context<TwoViews>) -> TwoViews {
        let views = two_spinners(cx);
        views.0[0].update(cx, |host, _| host.arc = false);
        views
    }
    fn pending(cx: &mut TestAppContext, id: WindowId) -> HashSet<EntityId> {
        cx.update(|cx| if cx.has_global::<SpinnerClock>() {
            cx.global::<SpinnerClock>().pending.get(&id).map(|f| f.views.clone()).unwrap_or_default()
        } else { HashSet::new() })
    }
    fn frame(window: &WindowHandle<TwoViews>, cx: &mut TestAppContext) -> usize {
        cx.executor().advance_clock(FRAME * 2);
        let delivered = window.update(cx, |_, window, cx| window.simulate_next_frame(cx)).unwrap();
        cx.run_until_parked();
        delivered
    }
    fn renders() -> [u32; 2] { RENDERS.with(|c| [c[0].get(), c[1].get()]) }

    struct OverlayProbe { open: bool, seen: Rc<Cell<bool>> }
    impl Render for OverlayProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            div().size_full()
                .child(gpui_kit::canvas(|_, _, _| {}, move |_, _, window, _| seen.set(window.has_deferred_draws())).size_full())
                .children(self.open.then(|| gpui_kit::deferred(div().size(px(20.)))))
        }
    }

    struct PresentProbe { generation: u32, applied: Rc<std::cell::RefCell<Vec<u32>>> }
    impl Render for PresentProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let generation = self.generation;
            let applied = self.applied.clone();
            gpui_kit::canvas(|_, _, _| {}, move |_, _, window, _| {
                let applied = applied.clone();
                window.defer_until_present(move || applied.borrow_mut().push(generation));
            }).size_full()
        }
    }

    #[gpui_kit::test]
    fn native_updates_wait_for_the_matching_scene_to_present(cx: &mut TestAppContext) {
        let applied = Rc::new(std::cell::RefCell::new(Vec::new()));
        let handle = cx.open_window(size(px(100.), px(100.)), |_, _| PresentProbe { generation: 1, applied: applied.clone() });
        cx.run_until_parked();
        cx.update_window(handle.into(), |root, window, cx| {
            let root = root.downcast::<PresentProbe>().expect("presentation probe");
            applied.borrow_mut().clear();
            root.update(cx, |view, _| view.generation = 2);
            window.refresh();
            window.draw(cx).clear(cx);
            assert!(applied.borrow().is_empty(), "offscreen draws cannot mutate live layers");
            root.update(cx, |view, _| view.generation = 3);
            window.refresh();
            window.draw(cx).clear(cx);
            assert!(applied.borrow().is_empty());
            window.present_if_needed();
            assert_eq!(*applied.borrow(), vec![3], "only the replacement scene's changes may commit");
            window.present_if_needed();
            assert_eq!(*applied.borrow(), vec![3], "present callbacks are consumed exactly once");
        }).unwrap();
    }

    #[gpui_kit::test]
    fn failed_native_presentations_retry_without_replaying_stale_updates(cx: &mut TestAppContext) {
        let handle = cx.open_window(size(px(100.), px(100.)), |_, _| SpinnerHost { arc: false, visible: false, clipped: false });
        let applied = Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            let queue = |window: &mut Window, generation| {
                let applied = applied.clone();
                window.defer_until_present(move || applied.borrow_mut().push(generation));
            };
            window.draw(cx).clear(cx);
            queue(window, 1);
            window.simulate_failed_presentations(1);
            assert_eq!(window.simulate_presentation(false), (true, 1));
            assert!(applied.borrow().is_empty());
            assert_eq!(window.simulate_presentation(false), (false, 0));
            assert_eq!(*applied.borrow(), vec![1]);
            window.simulate_presentation(true);
            assert_eq!(*applied.borrow(), vec![1], "a forced second present cannot replay the update");

            window.draw(cx).clear(cx);
            queue(window, 2);
            window.simulate_failed_presentations(1);
            assert_eq!(window.simulate_presentation(false), (true, 1));
            window.draw(cx).clear(cx);
            queue(window, 3);
            assert_eq!(window.simulate_presentation(false), (false, 0));
            assert_eq!(*applied.borrow(), vec![1, 3], "a new draw replaces failed scene updates");

            window.draw(cx).clear(cx);
            queue(window, 4);
            window.simulate_failed_presentations(10);
            for _ in 0..2 { assert_eq!(window.simulate_presentation(false), (true, 1)); }
            for _ in 0..8 { assert_eq!(window.simulate_presentation(false), (false, 1)); }
            assert_eq!(*applied.borrow(), vec![1, 3], "permanent failure stops retries without changing native layers");
            window.draw(cx).clear(cx);
            queue(window, 5);
            window.simulate_failed_presentations(0);
            assert_eq!(window.simulate_presentation(false), (false, 0));
            assert_eq!(*applied.borrow(), vec![1, 3, 5], "a new draw recovers after failure");
        }).unwrap();
    }

    #[gpui_kit::test]
    fn overlays_are_known_before_the_underlying_canvas_paints(cx: &mut TestAppContext) {
        let seen = Rc::new(Cell::new(true));
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| OverlayProbe { open: false, seen: seen.clone() });
        cx.run_until_parked();
        assert!(!seen.get());
        window.update(cx, |view, _, cx| { view.open = true; cx.notify(); }).unwrap();
        cx.run_until_parked();
        assert!(seen.get(), "native content must yield on the very first overlay frame");
        window.update(cx, |view, _, cx| { view.open = false; cx.notify(); }).unwrap();
        cx.run_until_parked();
        assert!(!seen.get(), "native animation can resume after the overlay leaves");
    }

    #[test]
    fn raster_capture_restores_the_previous_scope() {
        assert!(!super::RASTER_CAPTURE.get());
        super::raster_capture(|| {
            assert!(super::RASTER_CAPTURE.get());
            super::raster_capture(|| assert!(super::RASTER_CAPTURE.get()));
            assert!(super::RASTER_CAPTURE.get());
        });
        assert!(!super::RASTER_CAPTURE.get());
    }

    #[cfg(feature = "frame-stats")]
    struct LoaderProbeHost(bool);
    #[cfg(feature = "frame-stats")]
    impl Render for LoaderProbeHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let spinner = Spinner::new().when(self.0, Spinner::transcript_loader);
            div().size_full().child(spinner)
        }
    }
    #[cfg(feature = "frame-stats")]
    #[gpui_kit::test]
    fn unrelated_spinners_cannot_satisfy_the_transcript_probe(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| LoaderProbeHost(true));
        cx.run_until_parked();
        let initial = window.update(cx, |_, window, _| crate::frame_stats::report(window, true)).unwrap();
        assert!(initial["loader"]["paints"].as_u64().unwrap() > 0);
        let before = renders()[1];
        window.update(cx, |host, _, cx| { host.0 = false; cx.notify(); }).unwrap();
        cx.run_until_parked();
        assert!(renders()[1] > before, "the unmarked spinner must actually render");
        let unrelated = window.update(cx, |_, window, _| crate::frame_stats::report(window, false)).unwrap();
        assert_eq!(unrelated["loader"]["paints"].as_u64(), Some(0));
    }

    #[gpui_kit::test]
    fn reduce_motion_leaves_no_spinner_frame(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(true));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        let before = renders();
        assert!(pending(cx, window.window_id()).is_empty());
        assert_eq!(frame(&window, cx), 0);
        assert_eq!(renders(), before);
    }
    #[gpui_kit::test]
    fn clipped_spinners_leave_no_clock_demand(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| {
            let views = two_loaders(cx);
            for host in &views.0 { host.update(cx, |host, _| host.clipped = true); }
            views
        });
        cx.run_until_parked();
        assert!(pending(cx, window.window_id()).is_empty());
        assert_eq!(frame(&window, cx), 0);
    }
    #[gpui_kit::test]
    fn mixed_arc_and_loader_repaint_across_cached_parents(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_spinners(cx));
        cx.run_until_parked();
        let parents = pending(cx, window.window_id());
        assert_eq!(parents.len(), 1);
        for _ in 0..3 {
            let before = renders();
            assert_eq!(frame(&window, cx), 1);
            let after = renders();
            assert!(after[0] > before[0] && after[1] > before[1]);
            assert_eq!(pending(cx, window.window_id()), parents);
        }
    }
    #[gpui_kit::test]
    fn an_early_callback_does_not_render_spinners(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        let before = renders();
        assert_eq!(window.update(cx, |_, window, cx| window.simulate_next_frame(cx)).unwrap(), 1);
        cx.run_until_parked();
        assert_eq!(renders(), before);
        assert_eq!(frame(&window, cx), 1);
        assert!(renders()[1] > before[1]);
    }
    #[gpui_kit::test]
    fn multiple_spinners_share_one_callback_and_one_notify_per_parent(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        let notified = Rc::new([Cell::new(0), Cell::new(0)]);
        cx.update(|cx| {
            for (index, host) in window.read(cx).unwrap().0.clone().into_iter().enumerate() {
                let notified = notified.clone();
                cx.observe(&host, move |_, _| notified[index].set(notified[index].get() + 1)).detach();
            }
        });
        assert_eq!(pending(cx, window.window_id()).len(), 2);
        assert_eq!(frame(&window, cx), 1);
        assert_eq!([notified[0].get(), notified[1].get()], [1, 1]);
    }
    #[gpui_kit::test]
    fn hiding_stops_demand_and_closing_clears_pending_frames(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        assert_eq!(pending(cx, window.window_id()).len(), 2);
        window.update(cx, |views, _, cx| {
            for host in &views.0 { host.update(cx, |host, cx| { host.visible = false; cx.notify(); }); }
        }).unwrap();
        cx.run_until_parked(); frame(&window, cx);
        assert!(pending(cx, window.window_id()).is_empty());
        assert_eq!(frame(&window, cx), 0);
        window.update(cx, |views, _, cx| {
            for host in &views.0 { host.update(cx, |host, cx| { host.visible = true; cx.notify(); }); }
        }).unwrap();
        cx.run_until_parked();
        assert!(!pending(cx, window.window_id()).is_empty());
        window.update(cx, |_, window, _| window.remove_window()).unwrap();
        cx.run_until_parked();
        assert!(pending(cx, window.window_id()).is_empty());
    }
    #[gpui_kit::test]
    fn an_old_callback_cannot_drive_a_replacement_chain(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(false));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        let old = cx.update(|cx| cx.global::<SpinnerClock>().pending[&window.window_id()].serial);
        cx.executor().advance_clock(FRAME * 5);
        window.update(cx, |views, _, cx| views.0[0].update(cx, |_, cx| cx.notify())).unwrap();
        cx.run_until_parked();
        let replacement = cx.update(|cx| cx.global::<SpinnerClock>().pending[&window.window_id()].serial);
        assert_ne!(old, replacement);
        assert_eq!(pending(cx, window.window_id()).len(), 2, "a late callback must retain the other cached parent");
        let sibling_notified = Rc::new(Cell::new(0));
        cx.update(|cx| {
            let sibling = window.read(cx).unwrap().0[1].clone();
            let sibling_notified = sibling_notified.clone();
            cx.observe(&sibling, move |_, _| sibling_notified.set(sibling_notified.get() + 1)).detach();
        });
        window.update(cx, |_, window, cx| super::present_spinners(old, window, cx)).unwrap();
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| cx.global::<SpinnerClock>().pending[&window.window_id()].serial), replacement);
        assert_eq!(frame(&window, cx), 2, "old and replacement callbacks were queued");
        assert!(sibling_notified.get() > 0, "the cached sibling must keep turning");
        assert_eq!(frame(&window, cx), 1, "only the replacement chain continues");
    }
    #[gpui_kit::test]
    fn a_stale_record_without_a_callback_recovers(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(true));
        let window = cx.open_window(size(px(100.), px(100.)), |_, cx| two_loaders(cx));
        cx.run_until_parked();
        cx.update(|cx| {
            init_spinner_clock(cx);
            let due = cx.background_executor().now() - FRAME * 4;
            cx.global_mut::<SpinnerClock>().pending.insert(window.window_id(), SpinnerFrame { views: HashSet::new(), due, serial: 0 });
            cx.set_reduce_motion(false);
        });
        window.update(cx, |views, _, cx| views.0[0].update(cx, |_, cx| cx.notify())).unwrap();
        cx.run_until_parked();
        assert!(!pending(cx, window.window_id()).is_empty());
        let before = renders()[1];
        assert_eq!(frame(&window, cx), 1);
        assert!(renders()[1] > before);
    }
}
