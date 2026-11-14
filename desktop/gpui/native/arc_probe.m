// Scripted performance prototype only. No installed build links this file.
#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>

#ifdef TB_ARC_TEST
// Unattached views only: the main-thread Rust integration test opens no
// NSWindow or NSApplication and never takes over a screen.
void *tb_arc_probe_test_view_create(void) {
    NSCAssert(NSThread.isMainThread, @"arc fixture on main");
    NSView *view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 600, 400)];
    view.wantsLayer = YES;
    CAMetalLayer *metal = [CAMetalLayer layer];
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceDisplayP3);
    metal.colorspace = space;
    CGColorSpaceRelease(space);
    view.layer = metal;
    return (__bridge_retained void *)view;
}
uint32_t tb_arc_probe_test_view_layer_count(void *pointer) {
    return (uint32_t)((__bridge NSView *)pointer).layer.sublayers.count;
}
uint32_t tb_arc_probe_test_clip_count(void *pointer) {
    CALayer *host = ((__bridge NSView *)pointer).layer.sublayers.firstObject;
    return (uint32_t)host.sublayers.count;
}
double tb_arc_probe_test_model_period(void *pointer) {
    CALayer *host = ((__bridge NSView *)pointer).layer.sublayers.firstObject;
    CALayer *ring = host.sublayers.firstObject.sublayers.firstObject;
    return [ring animationForKey:@"turn"].duration;
}
void tb_arc_probe_test_view_drop(void *pointer) {
    NSView *view = (__bridge_transfer NSView *)pointer;
    (void)view;
}
#endif

typedef struct {
    float center_x, center_y, side, radius, width;
    float clip_x, clip_y, clip_w, clip_h;
    float r, g, b, a;
} TBArcSpec;

typedef struct {
    uint32_t placed, visible, animating, presented;
    double phase, period, time;
} TBArcHealth;

@interface TBArcSlot : CALayer
@property(nonatomic) TBArcSpec previous;
@property(nonatomic) BOOL hasSpec;
@end
@implementation TBArcSlot
@end

@interface TBArcProbe : NSObject
@property(nonatomic, strong) CALayer *host;
@property(nonatomic, strong) NSMutableArray<TBArcSlot *> *clips;
@end
@implementation TBArcProbe
@end

void *tb_arc_probe_create(void *parent) {
    NSCAssert(NSThread.isMainThread, @"arc probe on main");
    NSView *view = (__bridge NSView *)parent;
    if (!view.layer) return NULL;
    TBArcProbe *probe = [TBArcProbe new];
    probe.host = [CALayer layer];
    probe.host.frame = view.bounds;
    probe.host.geometryFlipped = YES;
    probe.host.autoresizingMask = kCALayerWidthSizable | kCALayerHeightSizable;
    probe.clips = [NSMutableArray array];
    [view.layer addSublayer:probe.host];
    return (__bridge_retained void *)probe;
}

void tb_arc_probe_set(void *pointer, const TBArcSpec *specs, size_t count, double phase, double duration) {
    NSCAssert(NSThread.isMainThread, @"arc probe on main");
    TBArcProbe *probe = (__bridge TBArcProbe *)pointer;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    probe.host.frame = probe.host.superlayer.bounds;
    // GPUI's backing layer tags its RGB components with its own color space
    // (Display P3 on the Mac). Reuse that interpretation, not Generic RGB.
    CALayer *parent = probe.host.superlayer;
    CGColorSpaceRef colorSpace = [parent isKindOfClass:CAMetalLayer.class] ? ((CAMetalLayer *)parent).colorspace : NULL;
    CGColorSpaceRef fallback = colorSpace ? NULL : CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    if (!colorSpace) colorSpace = fallback;
    while (probe.clips.count > count) {
        [probe.clips.lastObject removeFromSuperlayer];
        [probe.clips removeLastObject];
    }
    while (probe.clips.count < count) {
        TBArcSlot *clip = [TBArcSlot layer];
        clip.masksToBounds = YES;
        CAShapeLayer *ring = [CAShapeLayer layer];
        ring.fillColor = nil;
        ring.lineCap = kCALineCapRound;
        [clip addSublayer:ring];
        [probe.host addSublayer:clip];
        [probe.clips addObject:clip];
        CABasicAnimation *turn = [CABasicAnimation animationWithKeyPath:@"transform.rotation.z"];
        turn.fromValue = @0;
        turn.toValue = @(2 * M_PI);
        turn.duration = duration;
        turn.repeatCount = HUGE_VALF;
        turn.timingFunction = [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionLinear];
        turn.beginTime = CACurrentMediaTime() - phase * duration;
        [ring addAnimation:turn forKey:@"turn"];
    }
    for (size_t i = 0; i < count; ++i) {
        const TBArcSpec s = specs[i];
        TBArcSlot *clip = probe.clips[i];
        TBArcSpec old = clip.previous;
        CAShapeLayer *ring = (CAShapeLayer *)clip.sublayers.firstObject;
        clip.frame = CGRectMake(s.clip_x, s.clip_y, s.clip_w, s.clip_h);
        ring.bounds = CGRectMake(0, 0, s.side, s.side);
        ring.position = CGPointMake(s.center_x - s.clip_x, s.center_y - s.clip_y);
        // A scroll or hover may repaint the window many times. Moving a ring
        // does not change its geometry or color, or restart its animation.
        if (!clip.hasSpec || old.side != s.side || old.radius != s.radius) {
            CGMutablePathRef path = CGPathCreateMutable();
            CGPathAddArc(path, NULL, s.side / 2, s.side / 2, s.radius, 0, 1.5 * M_PI, false);
            ring.path = path;
            CGPathRelease(path);
        }
        if (!clip.hasSpec || old.r != s.r || old.g != s.g || old.b != s.b || old.a != s.a
            || !ring.strokeColor || !CFEqual(CGColorGetColorSpace(ring.strokeColor), colorSpace)) {
            CGFloat components[] = {s.r, s.g, s.b, s.a};
            CGColorRef color = CGColorCreate(colorSpace, components);
            ring.strokeColor = color;
            CGColorRelease(color);
        }
        ring.lineWidth = s.width;
        ring.contentsScale = probe.host.superlayer.contentsScale;
        clip.previous = s;
        clip.hasSpec = YES;
    }
    if (fallback) CGColorSpaceRelease(fallback);
    [CATransaction commit];
}

void tb_arc_probe_drop(void *pointer) {
    TBArcProbe *probe = (__bridge_transfer TBArcProbe *)pointer;
    [probe.host removeFromSuperlayer];
}

void tb_arc_probe_snapshot(void *pointer, TBArcHealth *out) {
    NSCAssert(NSThread.isMainThread, @"arc snapshot on main");
    TBArcProbe *probe = (__bridge TBArcProbe *)pointer;
    *out = (TBArcHealth){.placed = (uint32_t)probe.clips.count, .time = CACurrentMediaTime()};
    for (CALayer *clip in probe.clips) {
        CAShapeLayer *ring = (CAShapeLayer *)clip.sublayers.firstObject;
        if (CGColorGetAlpha(ring.strokeColor) <= 0 || CGRectIsEmpty(CGRectIntersection(ring.frame, clip.bounds))) continue;
        out->visible += 1;
        CABasicAnimation *turn = (CABasicAnimation *)[ring animationForKey:@"turn"];
        if (turn && turn.duration > 0 && turn.repeatCount > 1) out->animating += 1;
        CALayer *presented = ring.presentationLayer;
        if (presented) {
            out->presented += 1;
            double phase = atan2(presented.transform.m12, presented.transform.m11) / (2 * M_PI);
            out->phase = phase < 0 ? phase + 1 : phase;
            if (out->phase >= 1) out->phase = 0;
            out->period = turn.duration;
        }
    }
}
