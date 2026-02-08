package com.th3rdwave.safeareacontext

import com.facebook.react.bridge.ReadableMap
import com.facebook.react.module.annotations.ReactModule
import com.facebook.react.uimanager.ReactStylesDiffMap
import com.facebook.react.uimanager.StateWrapper
import com.facebook.react.uimanager.ThemedReactContext
import com.facebook.react.uimanager.annotations.ReactProp
import com.facebook.react.views.view.ReactViewGroup
import com.facebook.react.views.view.ReactViewManager

@ReactModule(name = SafeAreaViewManager.REACT_CLASS)
class SafeAreaViewManager : ReactViewManager() {
  override fun getName() = REACT_CLASS

  override fun createViewInstance(context: ThemedReactContext) = SafeAreaView(context)

  override fun createShadowNodeInstance() = SafeAreaViewShadowNode()

  override fun getShadowNodeClass() = SafeAreaViewShadowNode::class.java

  @ReactProp(name = "mode")
  fun setMode(view: SafeAreaView, mode: String?) {
    when (mode) {
      "padding" -> {
        view.setMode(SafeAreaViewMode.PADDING)
      }
      "margin" -> {
        view.setMode(SafeAreaViewMode.MARGIN)
      }
    }
  }

  @ReactProp(name = "edges")
  fun setEdges(view: SafeAreaView, propList: ReadableMap?) {
    if (propList != null) {
      view.setEdges(
          SafeAreaViewEdges(
              top = propList.getString("top")?.let { SafeAreaViewEdgeModes.valueOf(it.uppercase()) }
                      ?: SafeAreaViewEdgeModes.OFF,
              right =
                  propList.getString("right")?.let { SafeAreaViewEdgeModes.valueOf(it.uppercase()) }
                      ?: SafeAreaViewEdgeModes.OFF,
              bottom =
                  propList.getString("bottom")?.let {
                    SafeAreaViewEdgeModes.valueOf(it.uppercase())
                  }
                      ?: SafeAreaViewEdgeModes.OFF,
              left =
                  propList.getString("left")?.let { SafeAreaViewEdgeModes.valueOf(it.uppercase()) }
                      ?: SafeAreaViewEdgeModes.OFF))
    }
  }

  override fun updateState(
      view: ReactViewGroup,
      props: ReactStylesDiffMap?,
      stateWrapper: StateWrapper?
  ): Any? {
    (view as SafeAreaView).setStateWrapper(stateWrapper)
    return null
  }

  companion object {
    const val REACT_CLASS = "RNCSafeAreaView"
  }
}
