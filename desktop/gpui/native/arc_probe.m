// Scripted performance prototype only. No installed build links this file.
#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>

typedef struct {
    float center_x, center_y, side, radius, width;
    float clip_x, clip_y, clip_w, clip_h;
    float r, g, b, a;
} TBArcSpec;

@interface TBArcProbe : NSObject
@property(nonatomic, strong) CALayer *host;
@property(nonatomic, strong) NSMutableArray<CALayer *> *clips;
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
    while (probe.clips.count > count) {
        [probe.clips.lastObject removeFromSuperlayer];
        [probe.clips removeLastObject];
    }
    while (probe.clips.count < count) {
        CALayer *clip = [CALayer layer];
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
        CALayer *clip = probe.clips[i];
        CAShapeLayer *ring = (CAShapeLayer *)clip.sublayers.firstObject;
        clip.frame = CGRectMake(s.clip_x, s.clip_y, s.clip_w, s.clip_h);
        ring.bounds = CGRectMake(0, 0, s.side, s.side);
        ring.position = CGPointMake(s.center_x - s.clip_x, s.center_y - s.clip_y);
        CGMutablePathRef path = CGPathCreateMutable();
        CGPathAddArc(path, NULL, s.side / 2, s.side / 2, s.radius, 0, 1.5 * M_PI, false);
        ring.path = path;
        CGPathRelease(path);
        CGFloat components[] = {s.r, s.g, s.b, s.a};
        CGColorRef color = colorSpace ? CGColorCreate(colorSpace, components) : CGColorCreateSRGB(s.r, s.g, s.b, s.a);
        ring.strokeColor = color;
        CGColorRelease(color);
        ring.lineWidth = s.width;
        ring.contentsScale = probe.host.superlayer.contentsScale;
    }
    [CATransaction commit];
}

void tb_arc_probe_drop(void *pointer) {
    TBArcProbe *probe = (__bridge_transfer TBArcProbe *)pointer;
    [probe.host removeFromSuperlayer];
}
