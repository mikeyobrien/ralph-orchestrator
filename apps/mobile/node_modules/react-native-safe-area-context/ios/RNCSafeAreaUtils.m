#import "RNCSafeAreaUtils.h"

#import <React/RCTUIManager.h>

NSString *const RNCSafeAreaDidChange = @"RNCSafeAreaDidChange";

#if TARGET_OS_IPHONE
BOOL UIEdgeInsetsEqualToEdgeInsetsWithThreshold(UIEdgeInsets insets1, UIEdgeInsets insets2, CGFloat threshold)
#elif TARGET_OS_OSX
BOOL NSEdgeInsetsEqualToEdgeInsetsWithThreshold(NSEdgeInsets insets1, NSEdgeInsets insets2, CGFloat threshold)
#endif
{
  return ABS(insets1.left - insets2.left) <= threshold && ABS(insets1.right - insets2.right) <= threshold &&
      ABS(insets1.top - insets2.top) <= threshold && ABS(insets1.bottom - insets2.bottom) <= threshold;
}

#if TARGET_OS_IPHONE
UIViewController *RNCParentViewController(UIView *view)
{
  UIResponder *responder = view.nextResponder;
  while (responder != nil) {
    if ([responder isKindOfClass:[UIViewController class]]) {
      return (UIViewController *)responder;
    }
    responder = responder.nextResponder;
  }
  return nil;
}
#elif TARGET_OS_OSX
NSViewController *RNCParentViewController(NSView *view)
{
  NSResponder *responder = view.nextResponder;
  while (responder != nil) {
    if ([responder isKindOfClass:[NSViewController class]]) {
      return (NSViewController *)responder;
    }
    responder = responder.nextResponder;
  }
  return nil;
}
#endif
