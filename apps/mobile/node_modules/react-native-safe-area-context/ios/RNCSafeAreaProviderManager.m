#import "RNCSafeAreaProviderManager.h"

#import "RNCSafeAreaProvider.h"

@implementation RNCSafeAreaProviderManager

RCT_EXPORT_MODULE(RNCSafeAreaProvider)

RCT_EXPORT_VIEW_PROPERTY(onInsetsChange, RCTDirectEventBlock)

#if TARGET_OS_IPHONE
- (UIView *)view
#elif TARGET_OS_OSX
- (NSView *)view
#endif
{
  return [[RNCSafeAreaProvider alloc] initWithEventDispatcher:self.bridge.eventDispatcher];
}

@end
