#import "RNCSafeAreaViewComponentView.h"

#import <react/renderer/components/safeareacontext/EventEmitters.h>
#import <react/renderer/components/safeareacontext/Props.h>
#import <react/renderer/components/safeareacontext/RCTComponentViewHelpers.h>
#import <react/renderer/components/safeareacontext/RNCSafeAreaViewComponentDescriptor.h>
#import <react/renderer/components/safeareacontext/RNCSafeAreaViewShadowNode.h>

#import <React/RCTConversions.h>
#import <React/RCTFabricComponentsPlugins.h>

#import "RNCSafeAreaProviderComponentView.h"
#import "RNCSafeAreaUtils.h"

using namespace facebook::react;

@interface RNCSafeAreaViewComponentView () <RCTRNCSafeAreaViewViewProtocol>
@end

@implementation RNCSafeAreaViewComponentView {
  RNCSafeAreaViewShadowNode::ConcreteState::Shared _state;
  UIEdgeInsets _currentSafeAreaInsets;
  __weak UIView *_Nullable _providerView;
}

// Needed because of this: https://github.com/facebook/react-native/pull/37274
+ (void)load
{
  [super load];
}

- (instancetype)initWithFrame:(CGRect)frame
{
  if (self = [super initWithFrame:frame]) {
    static const auto defaultProps = std::make_shared<const RNCSafeAreaViewProps>();
    _props = defaultProps;
  }

  return self;
}

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
  NSString *providerViewSafeAreaInsetsString = [NSString stringWithFormat:@"{%f,%f,%f,%f}",
                                                                          _providerView.safeAreaInsets.top,
                                                                          _providerView.safeAreaInsets.left,
                                                                          _providerView.safeAreaInsets.bottom,
                                                                          _providerView.safeAreaInsets.right];
  NSString *currentSafeAreaInsetsString = [NSString stringWithFormat:@"{%f,%f,%f,%f}",
                                                                     _currentSafeAreaInsets.top,
                                                                     _currentSafeAreaInsets.left,
                                                                     _currentSafeAreaInsets.bottom,
                                                                     _currentSafeAreaInsets.right];
#endif

  return [NSString stringWithFormat:@"%@; RNCSafeAreaInsets = %@; appliedRNCSafeAreaInsets = %@>",
                                    superDescription,
                                    providerViewSafeAreaInsetsString,
                                    currentSafeAreaInsetsString];
}

- (void)didMoveToWindow
{
  UIView *previousProviderView = _providerView;
  _providerView = [self findNearestProvider];

  [self updateStateIfNecessary];

  if (previousProviderView != _providerView) {
    [NSNotificationCenter.defaultCenter removeObserver:self name:RNCSafeAreaDidChange object:previousProviderView];
    [NSNotificationCenter.defaultCenter addObserver:self
                                           selector:@selector(safeAreaProviderInsetsDidChange:)
                                               name:RNCSafeAreaDidChange
                                             object:_providerView];
  }
}

- (void)safeAreaProviderInsetsDidChange:(NSNotification *)notification
{
  [self updateStateIfNecessary];
}

- (void)updateStateIfNecessary
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
  [self updateState];
}

- (UIView *)findNearestProvider
{
  UIView *current = self.superview;
  while (current != nil) {
    if ([current isKindOfClass:RNCSafeAreaProviderComponentView.class]) {
      return current;
    }
    current = current.superview;
  }
  return self;
}

- (void)updateState
{
  if (!_state) {
    return;
  }

  _state->updateState(
      [=](RNCSafeAreaViewShadowNode::ConcreteState::Data const &oldData)
          -> RNCSafeAreaViewShadowNode::ConcreteState::SharedData {
        auto newData = oldData;
        newData.insets = RCTEdgeInsetsFromUIEdgeInsets(_currentSafeAreaInsets);
        return std::make_shared<RNCSafeAreaViewShadowNode::ConcreteState::Data const>(newData);
      });
}

#pragma mark - RCTComponentViewProtocol

+ (ComponentDescriptorProvider)componentDescriptorProvider
{
  return concreteComponentDescriptorProvider<RNCSafeAreaViewComponentDescriptor>();
}

- (void)updateState:(State::Shared const &)state oldState:(State::Shared const &)oldState
{
  _state = std::static_pointer_cast<RNCSafeAreaViewShadowNode::ConcreteState const>(state);
}

- (void)finalizeUpdates:(RNComponentViewUpdateMask)updateMask
{
  [super finalizeUpdates:updateMask];
  [self updateStateIfNecessary];
}

- (void)prepareForRecycle
{
  [super prepareForRecycle];

  [NSNotificationCenter.defaultCenter removeObserver:self];
  _state.reset();
  _providerView = nil;
  _currentSafeAreaInsets = UIEdgeInsetsZero;
}

@end

Class<RCTComponentViewProtocol> RNCSafeAreaViewCls(void)
{
  return RNCSafeAreaViewComponentView.class;
}
