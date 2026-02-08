#if TARGET_OS_IPHONE
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

#import "RNCSafeAreaViewEdges.h"
#import "RNCSafeAreaViewMode.h"

NS_ASSUME_NONNULL_BEGIN

@interface RNCSafeAreaViewLocalData : NSObject

#if TARGET_OS_IPHONE
- (instancetype)initWithInsets:(UIEdgeInsets)insets mode:(RNCSafeAreaViewMode)mode edges:(RNCSafeAreaViewEdges)edges;

@property (atomic, readonly) UIEdgeInsets insets;
#elif TARGET_OS_OSX
- (instancetype)initWithInsets:(NSEdgeInsets)insets mode:(RNCSafeAreaViewMode)mode edges:(RNCSafeAreaViewEdges)edges;

@property (atomic, readonly) NSEdgeInsets insets;
#endif

@property (atomic, readonly) RNCSafeAreaViewMode mode;
@property (atomic, readonly) RNCSafeAreaViewEdges edges;

@end

NS_ASSUME_NONNULL_END
