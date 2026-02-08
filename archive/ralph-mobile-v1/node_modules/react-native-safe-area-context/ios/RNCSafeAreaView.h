#import <React/RCTBridge.h>
#import <React/RCTView.h>
#if TARGET_OS_IPHONE
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

#import "RNCSafeAreaViewEdges.h"
#import "RNCSafeAreaViewMode.h"

NS_ASSUME_NONNULL_BEGIN

@class RNCSafeAreaView;

@interface RNCSafeAreaView : RCTView

- (instancetype)initWithBridge:(RCTBridge *)bridge;

@property (nonatomic, assign) RNCSafeAreaViewMode mode;
@property (nonatomic, assign) RNCSafeAreaViewEdges edges;

@end

NS_ASSUME_NONNULL_END
