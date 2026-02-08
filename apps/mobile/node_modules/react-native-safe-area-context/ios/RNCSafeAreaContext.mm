#import "RNCSafeAreaContext.h"

#import <React/RCTUtils.h>
#if TARGET_OS_IPHONE
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif
#ifdef RCT_NEW_ARCH_ENABLED
#import <safeareacontext/safeareacontext.h>
#endif

#ifdef RCT_NEW_ARCH_ENABLED
using namespace facebook::react;

@interface RNCSafeAreaContext () <NativeSafeAreaContextSpec>
@end
#endif

@implementation RNCSafeAreaContext

RCT_EXPORT_MODULE()

+ (BOOL)requiresMainQueueSetup
{
  return YES;
}

- (NSDictionary *)constantsToExport
{
  return [self getConstants];
}

- (NSDictionary *)getConstants
{
  __block NSDictionary *constants;

  RCTUnsafeExecuteOnMainQueueSync(^{
#if TARGET_OS_IPHONE
    UIWindow *window = RCTKeyWindow();
#elif TARGET_OS_OSX
    NSWindow *window = RCTKeyWindow();
#endif
    if (window == nil) {
      constants = @{@"initialWindowMetrics" : [NSNull null]};
      return;
    }

#if TARGET_OS_IPHONE
    UIEdgeInsets safeAreaInsets = window.safeAreaInsets;
#elif TARGET_OS_OSX
    NSEdgeInsets safeAreaInsets = NSEdgeInsetsZero;
#endif

    constants = @{
      @"initialWindowMetrics" : @{
        @"insets" : @{
          @"top" : @(safeAreaInsets.top),
          @"right" : @(safeAreaInsets.right),
          @"bottom" : @(safeAreaInsets.bottom),
          @"left" : @(safeAreaInsets.left),
        },
        @"frame" : @{
          @"x" : @(window.frame.origin.x),
          @"y" : @(window.frame.origin.y),
          @"width" : @(window.frame.size.width),
          @"height" : @(window.frame.size.height),
        },
      }
    };
  });

  return constants;
}

#ifdef RCT_NEW_ARCH_ENABLED

- (std::shared_ptr<TurboModule>)getTurboModule:(const ObjCTurboModule::InitParams &)params
{
  return std::make_shared<NativeSafeAreaContextSpecJSI>(params);
}

#endif

@end
