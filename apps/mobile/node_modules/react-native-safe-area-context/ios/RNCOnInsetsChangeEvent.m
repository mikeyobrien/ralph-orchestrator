#import "RNCOnInsetsChangeEvent.h"
#import <React/RCTAssert.h>

@implementation RNCOnInsetsChangeEvent {
  UIEdgeInsets _insets;
  CGRect _frame;
  uint16_t _coalescingKey;
}

@synthesize eventName = _eventName;
@synthesize viewTag = _viewTag;

- (instancetype)initWithEventName:(NSString *)eventName
                         reactTag:(NSNumber *)reactTag
                           insets:(UIEdgeInsets)insets
                            frame:(CGRect)frame
                    coalescingKey:(uint16_t)coalescingKey
{
  RCTAssertParam(reactTag);

  if ((self = [super init])) {
    _eventName = [eventName copy];
    _viewTag = reactTag;
    _frame = frame;
    _insets = insets;
    _coalescingKey = coalescingKey;
  }

  return self;
}

RCT_NOT_IMPLEMENTED(-(instancetype)init)

- (uint16_t)coalescingKey
{
  return _coalescingKey;
}

- (NSDictionary *)body
{
  NSDictionary *body = @{
    @"insets" : @{
      @"top" : @(_insets.top),
      @"right" : @(_insets.right),
      @"bottom" : @(_insets.bottom),
      @"left" : @(_insets.left),
    },
    @"frame" : @{
      @"x" : @(_frame.origin.x),
      @"y" : @(_frame.origin.y),
      @"width" : @(_frame.size.width),
      @"height" : @(_frame.size.height),
    },
  };

  return body;
}

- (BOOL)canCoalesce
{
  return YES;
}

- (RNCOnInsetsChangeEvent *)coalesceWithEvent:(RNCOnInsetsChangeEvent *)newEvent
{
  return newEvent;
}

+ (NSString *)moduleDotMethod
{
  return @"RCTEventEmitter.receiveEvent";
}

- (NSArray *)arguments
{
  return @[ self.viewTag, RCTNormalizeInputEventName(self.eventName), [self body] ];
}

@end
