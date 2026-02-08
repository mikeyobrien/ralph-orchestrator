#if TARGET_OS_IPHONE
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

#import <React/RCTViewComponentView.h>

NS_ASSUME_NONNULL_BEGIN

@interface RNCSafeAreaProviderComponentView : RCTViewComponentView

extern NSString *const RNCSafeAreaDidChange;

@end

NS_ASSUME_NONNULL_END
