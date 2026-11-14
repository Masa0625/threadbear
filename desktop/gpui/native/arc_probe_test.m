#import "arc_probe.m"
#include <assert.h>
int main(void) { @autoreleasepool {
 assert(tb_arc_probe_create(NULL) == NULL);
 NSView *view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 600, 400)];
 view.wantsLayer = YES;
 CAMetalLayer *metal = [CAMetalLayer layer];
 CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceDisplayP3);
 metal.colorspace = space; CGColorSpaceRelease(space);
 view.layer = metal;
 void *raw = tb_arc_probe_create((__bridge void *)view);
 assert(raw != NULL);
 TBArcProbe *probe = (__bridge TBArcProbe *)raw;
 assert(probe.host.superlayer == view.layer);
 TBArcSpec s[2] = {
  {.center_x=10,.center_y=20,.side=11,.radius=4,.width=1.6,.clip_x=9,.clip_y=19,.clip_w=40,.clip_h=30,.r=.1,.g=.5,.b=.8,.a=.4},
  {.center_x=80,.center_y=120,.side=11,.radius=4,.width=1.6,.clip_x=0,.clip_y=0,.clip_w=600,.clip_h=400,.r=.1,.g=.5,.b=.8,.a=.8}
 };
 tb_arc_probe_set(raw, s, 2, .25, .9);
 TBArcHealth health;
 tb_arc_probe_snapshot(raw, &health);
 assert(health.placed == 2 && health.visible == 2 && health.animating == 2);
 assert(health.presented == 0); // A headless layer must not pass live presentation checks.
 assert(probe.clips.count == 2);
 CALayer *clip = probe.clips[0];
 CAShapeLayer *ring = (CAShapeLayer *)clip.sublayers[0];
 assert(clip.masksToBounds && CGRectEqualToRect(clip.frame, CGRectMake(9,19,40,30)));
 assert(CGPointEqualToPoint(ring.position, CGPointMake(1,1)));
 CABasicAnimation *turn = (CABasicAnimation *)[ring animationForKey:@"turn"];
 assert(fabs(turn.duration - .9) < 1e-9 && fabs([turn.toValue doubleValue] - 2*M_PI) < 1e-9);
 assert(fabs(CGColorGetAlpha(ring.strokeColor) - .4) < 1e-6);
 assert(CFEqual(CGColorSpaceGetName(CGColorGetColorSpace(ring.strokeColor)), kCGColorSpaceDisplayP3));
 CFTimeInterval began = turn.beginTime;
 view.layer.bounds = CGRectMake(0,0,900,700);
 s[0].center_x += 20;
 tb_arc_probe_set(raw, s, 2, .75, .9);
 assert(probe.clips[0] == clip);
 assert(CGRectEqualToRect(probe.host.frame, view.layer.bounds));
 assert(fabs([ring animationForKey:@"turn"].beginTime - began) < 1e-9);
 assert(CGPointEqualToPoint(ring.position, CGPointMake(21,1)));
 tb_arc_probe_set(raw, NULL, 0, 0, .9);
 tb_arc_probe_snapshot(raw, &health);
 assert(health.placed == 0 && health.visible == 0 && health.animating == 0);
 assert(probe.clips.count == 0 && probe.host.sublayers.count == 0);
 CALayer *host = probe.host;
 probe = nil;
 tb_arc_probe_drop(raw);
 assert(host.superlayer == nil);
 puts("PASS: null host, placement+clip+alpha+P3, .9s turn, stable phase while moving, resize, hide, teardown");
 } return 0; }
