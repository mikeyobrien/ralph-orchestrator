package com.th3rdwave.safeareacontext

import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.WritableMap
import com.facebook.react.uimanager.events.Event

internal class InsetsChangeEvent(
    surfaceId: Int,
    viewTag: Int,
    private val mInsets: EdgeInsets,
    private val mFrame: Rect
) : Event<InsetsChangeEvent>(surfaceId, viewTag) {
  override fun getEventName() = EVENT_NAME

  override fun getEventData(): WritableMap? {
    val event = Arguments.createMap()
    event.putMap("insets", edgeInsetsToJsMap(mInsets))
    event.putMap("frame", rectToJsMap(mFrame))
    return event
  }

  companion object {
    const val EVENT_NAME = "topInsetsChange"
  }
}
