package com.th3rdwave.safeareacontext

import android.content.Context
import android.view.View
import com.facebook.react.bridge.ReactContext
import com.facebook.react.uimanager.UIManagerHelper

fun getReactContext(view: View): ReactContext {
  return UIManagerHelper.getReactContext(view)
}

fun getSurfaceId(context: Context): Int {
  return UIManagerHelper.getSurfaceId(context)
}
