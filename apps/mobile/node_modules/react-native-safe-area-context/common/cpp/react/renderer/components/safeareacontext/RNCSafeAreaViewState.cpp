#include "RNCSafeAreaViewState.h"

namespace facebook {
namespace react {

#ifdef ANDROID

folly::dynamic RNCSafeAreaViewState::getDynamic() const {
  folly::dynamic insetsValue = folly::dynamic::object();
  insetsValue["top"] = insets.top;
  insetsValue["left"] = insets.left;
  insetsValue["bottom"] = insets.bottom;
  insetsValue["right"] = insets.right;

  folly::dynamic data = folly::dynamic::object();
  data["insets"] = insetsValue;

  return data;
}
#endif

} // namespace react
} // namespace facebook
