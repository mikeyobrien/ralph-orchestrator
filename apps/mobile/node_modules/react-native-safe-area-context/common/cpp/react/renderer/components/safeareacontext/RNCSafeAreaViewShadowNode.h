#pragma once

#include <jsi/jsi.h>
#include <react/renderer/components/safeareacontext/EventEmitters.h>
#include <react/renderer/components/safeareacontext/Props.h>
#include <react/renderer/components/safeareacontext/RNCSafeAreaViewState.h>
#include <react/renderer/components/view/ConcreteViewShadowNode.h>

namespace facebook {
namespace react {

JSI_EXPORT extern const char RNCSafeAreaViewComponentName[];

/*
 * `ShadowNode` for <RNCSafeAreaView> component.
 */
class JSI_EXPORT RNCSafeAreaViewShadowNode final
    : public ConcreteViewShadowNode<
          RNCSafeAreaViewComponentName,
          RNCSafeAreaViewProps,
          ViewEventEmitter,
          RNCSafeAreaViewState> {
  using ConcreteViewShadowNode::ConcreteViewShadowNode;

 public:
  void adjustLayoutWithState();
};

} // namespace react
} // namespace facebook
