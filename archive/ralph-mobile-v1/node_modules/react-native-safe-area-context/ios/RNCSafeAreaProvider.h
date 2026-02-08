#if TARGET_OS_IPHONE
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

#import <React/RCTEventDispatcherProtocol.h>
#import <React/RCTView.h>

NS_ASSUME_NONNULL_BEGIN

@interface RNCSafeAreaProvider : RCTView

- (instancetype)initWithEventDispatcher:(id<RCTEventDispatcherProtocol>)eventDispatcher NS_DESIGNATED_INITIALIZER;

// NOTE: currently these event props are only declared so we can export the
// event names to JS - we don't call the blocks directly because events
// need to be coalesced before sending, for performance reasons.
@property (nonatomic, copy) RCTBubblingEventBlock onInsetsChange;

@end

NS_ASSUME_NONNULL_END
