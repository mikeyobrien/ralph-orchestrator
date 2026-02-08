# react-native-is-edge-to-edge

Many libraries provide options to account for the transparency of status and navigation bars. For example, the [`useHideAnimation`](https://github.com/zoontek/react-native-bootsplash?tab=readme-ov-file#usehideanimation) hook in `react-native-bootsplash` includes `statusBarTranslucent` and `navigationBarTranslucent` options, while the [`useAnimatedKeyboard`](https://docs.swmansion.com/react-native-reanimated/docs/device/useAnimatedKeyboard) hook in `react-native-reanimated` offers an `isStatusBarTranslucentAndroid` option, among others.

> [!IMPORTANT]  
> Until third-party libraries officially add support for `react-native-edge-to-edge` to set these options automatically, you may need to adjust them manually to prevent interference with the library.

To support library authors, we provide this lightweight package called `react-native-is-edge-to-edge` (note the `-is-`!), which checks whether `react-native-edge-to-edge` is installed, making it easier to update your library accordingly:

```tsx
import {
  controlEdgeToEdgeValues,
  isEdgeToEdge,
} from "react-native-is-edge-to-edge";

const EDGE_TO_EDGE = isEdgeToEdge();

function MyAwesomeLibraryComponent({
  statusBarTranslucent,
  navigationBarTranslucent,
}) {
  if (__DEV__) {
    // warn the user once about unnecessary defined values
    controlEdgeToEdgeValues({
      statusBarTranslucent,
      navigationBarTranslucent,
    });
  }

  return (
    <MyAwesomeLibraryNativeComponent
      statusBarTranslucent={EDGE_TO_EDGE || statusBarTranslucent}
      navigationBarTranslucent={EDGE_TO_EDGE || navigationBarTranslucent}
      // …
    />
  );
}
```

If you want to check for the library's presence on the native side to bypass certain parts of your code, consider using this small utility:

```kotlin
object EdgeToEdge {
  // we cannot detect edge-to-edge, but we can detect react-native-edge-to-edge install
  val ENABLED: Boolean = try {
      Class.forName("com.zoontek.rnedgetoedge.EdgeToEdgePackage")
      true
    } catch (exception: ClassNotFoundException) {
      false
    }
}
```
