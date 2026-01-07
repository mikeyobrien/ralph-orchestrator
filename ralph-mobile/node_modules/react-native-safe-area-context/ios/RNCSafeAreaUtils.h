#import <React/RCTDefines.h>
#import <React/RCTView.h>
#import <UIKit/UIKit.h>

extern NSString *const RNCSafeAreaDidChange;

RCT_EXTERN BOOL
UIEdgeInsetsEqualToEdgeInsetsWithThreshold(UIEdgeInsets insets1, UIEdgeInsets insets2, CGFloat threshold);

RCT_EXTERN UIViewController *RNCParentViewController(UIView *view);
