#import "RNCSafeAreaView.h"

#import <React/RCTBridge.h>
#import <React/RCTUIManager.h>

#import "RNCSafeAreaProvider.h"
#import "RNCSafeAreaUtils.h"
#import "RNCSafeAreaViewEdges.h"
#import "RNCSafeAreaViewLocalData.h"
#import "RNCSafeAreaViewMode.h"

@implementation RNCSafeAreaView {
  __weak RCTBridge *_bridge;
#if TARGET_OS_IPHONE
  UIEdgeInsets _currentSafeAreaInsets;
#elif TARGET_OS_OSX
  NSEdgeInsets _currentSafeAreaInsets;
#endif
  RNCSafeAreaViewMode _mode;
  RNCSafeAreaViewEdges _edges;
  __weak RNCSafeAreaProvider *_Nullable _providerView;
}

- (instancetype)initWithBridge:(RCTBridge *)bridge
{
  if (self = [super initWithFrame:CGRectZero]) {
    _bridge = bridge;
    // Defaults
    _mode = RNCSafeAreaViewModePadding;
    _edges = RNCSafeAreaViewEdgesMake(
        RNCSafeAreaViewEdgeModeOff, RNCSafeAreaViewEdgeModeOff, RNCSafeAreaViewEdgeModeOff, RNCSafeAreaViewEdgeModeOff);
  }

  return self;
}

RCT_NOT_IMPLEMENTED(-(instancetype)initWithCoder : (NSCoder *)decoder)
RCT_NOT_IMPLEMENTED(-(instancetype)initWithFrame : (CGRect)frame)

- (NSString *)description
{
  NSString *superDescription = [super description];

  // Cutting the last `>` character.
  if (superDescription.length > 0 && [superDescription characterAtIndex:superDescription.length - 1] == '>') {
    superDescription = [superDescription substringToIndex:superDescription.length - 1];
  }

#if TARGET_OS_IPHONE
  NSString *providerViewSafeAreaInsetsString = NSStringFromUIEdgeInsets(_providerView.safeAreaInsets);
  NSString *currentSafeAreaInsetsString = NSStringFromUIEdgeInsets(_currentSafeAreaInsets);
#elif TARGET_OS_OSX
  NSString *providerViewSafeAreaInsetsString;
  NSString *currentSafeAreaInsetsString;
  if (@available(macOS 11.0, *)) {
    providerViewSafeAreaInsetsString = [NSString stringWithFormat:@"{%f,%f,%f,%f}",
                                                                  _providerView.safeAreaInsets.top,
                                                                  _providerView.safeAreaInsets.left,
                                                                  _providerView.safeAreaInsets.bottom,
                                                                  _providerView.safeAreaInsets.right];
    currentSafeAreaInsetsString = [NSString stringWithFormat:@"{%f,%f,%f,%f}",
                                                             _currentSafeAreaInsets.top,
                                                             _currentSafeAreaInsets.left,
                                                             _currentSafeAreaInsets.bottom,
                                                             _currentSafeAreaInsets.right];
  } else {
    providerViewSafeAreaInsetsString = @"{0.0,0.0,0.0,0.0}";
    currentSafeAreaInsetsString = @"{0.0,0.0,0.0,0.0}";
  }
#endif

  return [NSString stringWithFormat:@"%@; RNCSafeAreaInsets = %@; appliedRNCSafeAreaInsets = %@>",
                                    superDescription,
                                    providerViewSafeAreaInsetsString,
                                    currentSafeAreaInsetsString];
}

- (void)didMoveToWindow
{
#if TARGET_OS_IPHONE
  UIView *previousProviderView = _providerView;
#elif TARGET_OS_OSX
  NSView *previousProviderView = _providerView;
#endif
  _providerView = [self findNearestProvider];

  [self invalidateSafeAreaInsets];

  if (previousProviderView != _providerView) {
    if (previousProviderView != nil) {
      [NSNotificationCenter.defaultCenter removeObserver:self name:RNCSafeAreaDidChange object:previousProviderView];
    }

    if (_providerView != nil) {
      [NSNotificationCenter.defaultCenter addObserver:self
                                             selector:@selector(safeAreaProviderInsetsDidChange:)
                                                 name:RNCSafeAreaDidChange
                                               object:_providerView];
    }
  }
}

- (void)safeAreaProviderInsetsDidChange:(NSNotification *)notification
{
  [self invalidateSafeAreaInsets];
}

- (void)invalidateSafeAreaInsets
{
  if (_providerView == nil) {
    return;
  }
#if TARGET_OS_IPHONE
  UIEdgeInsets safeAreaInsets = _providerView.safeAreaInsets;
  if (UIEdgeInsetsEqualToEdgeInsetsWithThreshold(safeAreaInsets, _currentSafeAreaInsets, 1.0 / RCTScreenScale())) {
    return;
  }
#elif TARGET_OS_OSX
  NSEdgeInsets safeAreaInsets = _providerView.safeAreaInsets;
  if (NSEdgeInsetsEqualToEdgeInsetsWithThreshold(safeAreaInsets, _currentSafeAreaInsets, 1.0 / RCTScreenScale())) {
    return;
  }
#endif

  _currentSafeAreaInsets = safeAreaInsets;
  [self updateLocalData];
}

- (nullable RNCSafeAreaProvider *)findNearestProvider
{
#if TARGET_OS_IPHONE
  UIView *current = self.reactSuperview;
#elif TARGET_OS_OSX
  NSView *current = self.reactSuperview;
#endif
  while (current != nil) {
    if ([current isKindOfClass:RNCSafeAreaProvider.class]) {
      return (RNCSafeAreaProvider *)current;
    }
    current = current.reactSuperview;
  }
  return nil;
}

- (void)updateLocalData
{
  if (_providerView == nil) {
    return;
  }
  RNCSafeAreaViewLocalData *localData = [[RNCSafeAreaViewLocalData alloc] initWithInsets:_currentSafeAreaInsets
                                                                                    mode:_mode
                                                                                   edges:_edges];
  [_bridge.uiManager setLocalData:localData forView:self];
}

- (void)setMode:(RNCSafeAreaViewMode)mode
{
  _mode = mode;
  [self updateLocalData];
}

- (void)setEdges:(RNCSafeAreaViewEdges)edges
{
  _edges = edges;
  [self updateLocalData];
}

@end
