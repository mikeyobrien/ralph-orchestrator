#import "RNCSafeAreaViewLocalData.h"

@implementation RNCSafeAreaViewLocalData

#if TARGET_OS_IPHONE
- (instancetype)initWithInsets:(UIEdgeInsets)insets mode:(RNCSafeAreaViewMode)mode edges:(RNCSafeAreaViewEdges)edges
#elif TARGET_OS_OSX
- (instancetype)initWithInsets:(NSEdgeInsets)insets mode:(RNCSafeAreaViewMode)mode edges:(RNCSafeAreaViewEdges)edges
#endif
{
  if (self = [super init]) {
    _insets = insets;
    _mode = mode;
    _edges = edges;
  }

  return self;
}

@end
