#import "RNCSafeAreaProviderComponentView.h"

#import <react/renderer/components/safeareacontext/ComponentDescriptors.h>
#import <react/renderer/components/safeareacontext/EventEmitters.h>
#import <react/renderer/components/safeareacontext/Props.h>
#import <react/renderer/components/safeareacontext/RCTComponentViewHelpers.h>

#import <React/RCTFabricComponentsPlugins.h>
#import "RNCSafeAreaUtils.h"

using namespace facebook::react;

@interface RNCSafeAreaProviderComponentView () <RCTRNCSafeAreaProviderViewProtocol>
@end

@implementation RNCSafeAreaProviderComponentView {
  UIEdgeInsets _currentSafeAreaInsets;
  CGRect _currentFrame;
  BOOL _initialInsetsSent;
  BOOL _registeredNotifications;
}

// Needed because of this: https://github.com/facebook/react-native/pull/37274
+ (void)load
{
  [super load];
}

- (instancetype)initWithFrame:(CGRect)frame
{
  if (self = [super initWithFrame:frame]) {
    static const auto defaultProps = std::make_shared<const RNCSafeAreaProviderProps>();
    _props = defaultProps;
  }

  return self;
}

#if !TARGET_OS_OSX
- (void)willMoveToSuperview:(UIView *)newSuperView
{
  [super willMoveToSuperview:newSuperView];

  if (newSuperView != nil && !_registeredNotifications) {
    _registeredNotifications = YES;
    [self registerNotifications];
  }
}
#endif

- (void)registerNotifications
{
#if !TARGET_OS_TV && !TARGET_OS_OSX
  [NSNotificationCenter.defaultCenter addObserver:self
                                         selector:@selector(invalidateSafeAreaInsets)
                                             name:UIKeyboardDidShowNotification
                                           object:nil];
  [NSNotificationCenter.defaultCenter addObserver:self
                                         selector:@selector(invalidateSafeAreaInsets)
                                             name:UIKeyboardDidHideNotification
                                           object:nil];
  [NSNotificationCenter.defaultCenter addObserver:self
                                         selector:@selector(invalidateSafeAreaInsets)
                                             name:UIKeyboardDidChangeFrameNotification
                                           object:nil];
#endif
}

- (void)safeAreaInsetsDidChange
{
  [self invalidateSafeAreaInsets];
}

- (void)invalidateSafeAreaInsets
{
  if (self.superview == nil) {
    return;
  }
  // This gets called before the view size is set by react-native so
  // make sure to wait so we don't set wrong insets to JS.
  if (CGSizeEqualToSize(self.frame.size, CGSizeZero)) {
    return;
  }

  UIEdgeInsets safeAreaInsets = self.safeAreaInsets;
  CGRect frame = [self convertRect:self.bounds toView:RNCParentViewController(self).view];

  if (_initialInsetsSent &&
#if TARGET_OS_IPHONE
      UIEdgeInsetsEqualToEdgeInsetsWithThreshold(safeAreaInsets, _currentSafeAreaInsets, 1.0 / RCTScreenScale()) &&
#elif TARGET_OS_OSX
      NSEdgeInsetsEqualToEdgeInsetsWithThreshold(safeAreaInsets, _currentSafeAreaInsets, 1.0 / RCTScreenScale()) &&
#endif
      CGRectEqualToRect(frame, _currentFrame)) {
    return;
  }

  _initialInsetsSent = YES;
  _currentSafeAreaInsets = safeAreaInsets;
  _currentFrame = frame;

  [NSNotificationCenter.defaultCenter postNotificationName:RNCSafeAreaDidChange object:self userInfo:nil];

  if (_eventEmitter) {
    RNCSafeAreaProviderEventEmitter::OnInsetsChange event = {
        .insets =
            {
                .top = safeAreaInsets.top,
                .left = safeAreaInsets.left,
                .bottom = safeAreaInsets.bottom,
                .right = safeAreaInsets.right,
            },
        .frame =
            {
                .x = frame.origin.x,
                .y = frame.origin.y,
                .width = frame.size.width,
                .height = frame.size.height,
            },
    };
    std::static_pointer_cast<RNCSafeAreaProviderEventEmitter const>(_eventEmitter)->onInsetsChange(event);
  }
}

- (void)layoutSubviews
{
  [super layoutSubviews];

  [self invalidateSafeAreaInsets];
}

#pragma mark - RCTComponentViewProtocol

+ (ComponentDescriptorProvider)componentDescriptorProvider
{
  return concreteComponentDescriptorProvider<RNCSafeAreaProviderComponentDescriptor>();
}

- (void)prepareForRecycle
{
  [super prepareForRecycle];
  _currentSafeAreaInsets = UIEdgeInsetsZero;
  _currentFrame = CGRectZero;
  _initialInsetsSent = NO;
  [NSNotificationCenter.defaultCenter removeObserver:self];
  _registeredNotifications = NO;
}

@end

Class<RCTComponentViewProtocol> RNCSafeAreaProviderCls(void)
{
  return RNCSafeAreaProviderComponentView.class;
}
