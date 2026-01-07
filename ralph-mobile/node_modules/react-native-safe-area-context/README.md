# react-native-safe-area-context

[![npm](https://img.shields.io/npm/v/react-native-safe-area-context)](https://www.npmjs.com/package/react-native-safe-area-context) ![Supports Android, iOS, web, macOS and Windows](https://img.shields.io/badge/platforms-android%20%7C%20ios%20%7C%20web%20%7C%20macos%20%7C%20windows-lightgrey.svg) ![MIT License](https://img.shields.io/npm/l/react-native-safe-area-context.svg)

[![JavaScript tests](https://github.com/th3rdwave/react-native-safe-area-context/workflows/JavaScript%20tests/badge.svg)](https://github.com/th3rdwave/react-native-safe-area-context/actions?query=workflow%3AJavaScript%20tests) [![iOS build](https://github.com/th3rdwave/react-native-safe-area-context/workflows/iOS%20build/badge.svg)](https://github.com/th3rdwave/react-native-safe-area-context/actions?query=workflow%3AiOS%20build) [![Android build](https://github.com/th3rdwave/react-native-safe-area-context/workflows/Android%20build/badge.svg)](https://github.com/th3rdwave/react-native-safe-area-context/actions?query=workflow%3AAndroid%20build)

A flexible way to handle safe area, also works on Android and Web!

## Getting started

```bash
npm install react-native-safe-area-context
```
```bash
yarn add react-native-safe-area-context
```

You then need to link the native parts of the library for the platforms you are using.

- **iOS Platform:**

  `$ npx pod-install`

## Supported react-native version

| version | react-native version |
| ------- | -------------------- |
| 4.0.0+  | 0.64.0+              |

## New architecture support

This library currently has experimental support for the new react-native architecture. Note that there will be breaking changes and only the latest version of react-native will be supported.

You will need to be on 4.4.0 and react-native 0.70+.

## Usage

This library has 2 important concepts, if you are familiar with React Context this is very similar.

### Providers

The [SafeAreaProvider](#safeareaprovider) component is a `View` from where insets provided by [Consumers](#consumers) are relative to. This means that if this view overlaps with any system elements (status bar, notches, etc.) these values will be provided to descendent consumers. Usually you will have one provider at the top of your app.

### Consumers

Consumers are components and hooks that allow using inset values provided by the nearest parent [Provider](#providers). Values are always relative to a provider and not to these components.

- [SafeAreaView](#safeareaview) is the preferred way to consume insets. This is a regular `View` with insets applied as extra padding or margin. It offers better performance by applying insets natively and avoids flickers that can happen with the other JS based consumers.

- [useSafeAreaInsets](#usesafeareainsets) offers more flexibility, but can cause some layout flicker in certain cases. Use this if you need more control over how insets are applied.

## API

### SafeAreaProvider

You should add `SafeAreaProvider` in your app root component. You may need to add it in other places like the root of modals and routes when using [`react-native-screens`](https://github.com/software-mansion/react-native-screens).

Note that providers should not be inside a `View` that is animated with `Animated` or inside a `ScrollView` since it can cause very frequent updates.

#### Example

```js
import { SafeAreaProvider } from 'react-native-safe-area-context';

function App() {
  return <SafeAreaProvider>...</SafeAreaProvider>;
}
```

#### Props

Accepts all [View](https://reactnative.dev/docs/view#props) props. Has a default style of `{flex: 1}`.

##### `initialMetrics`

Optional, defaults to `null`.

Can be used to provide the initial value for frame and insets, this allows rendering immediatly. See [optimization](#optimization) for more information on how to use this prop.

### SafeAreaView

`SafeAreaView` is a regular `View` component with the safe area insets applied as padding or margin.

Padding or margin styles are added to the insets, for example `style={{paddingTop: 10}}` on a `SafeAreaView` that has insets of 20 will result in a top padding of 30.

#### Example

```js
import { SafeAreaView } from 'react-native-safe-area-context';

function SomeComponent() {
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: 'red' }}>
      <View style={{ flex: 1, backgroundColor: 'blue' }} />
    </SafeAreaView>
  );
}
```

#### Props

Accepts all [View](https://reactnative.dev/docs/view#props) props.

##### `edges`

Optional, array of `top`, `right`, `bottom`, and `left`. Defaults to all.

Sets the edges to apply the safe area insets to.

For example if you don't want insets to apply to the top edge because the view does not touch the top of the screen you can use:

```js
<SafeAreaView edges={['right', 'bottom', 'left']} />
```

Optionally it can be set to an object `{ top?: EdgeMode, right?: EdgeMode, bottom?: EdgeMode, left?: EdgeMode }` where `EdgeMode = 'off' | 'additive' | 'maximum'`. Additive is a default mode and is the same as passing and edge in the array: `finalPadding = safeArea + padding`. Maximum mode will use safe area inset or padding/margin (depends on `mode`) if safe area is less: `finalPadding = max(safeArea, padding)`. For example if you want a floating UI element that should be at the bottom safe area edge on devices with safe area or 24px from the bottom of the screen on devices without safe area or if safe area is less than 24px:

```js
<SafeAreaView style={{paddingBottom: 24}} edges={{bottom: 'maximum'}} />
```

##### `mode`

Optional, `padding` (default) or `margin`.

Apply the safe area to either the padding or the margin.

This can be useful for example to create a safe area aware separator component:

```js
<SafeAreaView mode="margin" style={{ height: 1, backgroundColor: '#eee' }} />
```

### useSafeAreaInsets

Returns the safe area insets of the nearest provider. This allows manipulating the inset values from JavaScript. Note that insets are not updated synchronously so it might cause a slight delay for example when rotating the screen.

Object with `{ top: number, right: number, bottom: number, left: number }`.

```js
import { useSafeAreaInsets } from 'react-native-safe-area-context';

function HookComponent() {
  const insets = useSafeAreaInsets();

  return <View style={{ paddingBottom: Math.max(insets.bottom, 16) }} />;
}
```

### useSafeAreaFrame

Returns the frame of the nearest provider. This can be used as an alternative to the `Dimensions` module.

Object with `{ x: number, y: number, width: number, height: number }`

### `SafeAreaInsetsContext`

React Context with the value of the safe area insets.

Can be used with class components:

```js
import { SafeAreaInsetsContext } from 'react-native-safe-area-context';

class ClassComponent extends React.Component {
  render() {
    return (
      <SafeAreaInsetsContext.Consumer>
        {(insets) => <View style={{ paddingTop: insets.top }} />}
      </SafeAreaInsetsContext.Consumer>
    );
  }
}
```

### `withSafeAreaInsets`

Higher order component that provides safe area insets as the `insets` prop.

```ts
type Props = WithSafeAreaInsetsProps & {
  someProp: number;
};

class ClassComponent extends React.Component<Props> {
  render() {
    return <View style={{ paddingTop: this.props.insets.top }} />;
  }
}

const ClassComponentWithInsets = withSafeAreaInsets(ClassComponent);

<ClassComponentWithInsets someProp={1} />;
```

### `SafeAreaFrameContext`

React Context with the value of the safe area frame.

### `initialWindowMetrics`

Insets and frame of the window on initial render. This can be used with the `initialMetrics` from `SafeAreaProvider`. See [optimization](#optimization) for more information.

Object with:

```ts
{
  frame: { x: number, y: number, width: number, height: number },
  insets: { top: number, left: number, right: number, bottom: number },
}
```

**NOTE:** This value can be null or out of date as it is computed when the native module is created.

## Deprecated apis

### useSafeArea

Use `useSafeAreaInsets` instead.

### SafeAreaConsumer

Use `SafeAreaInsetsContext.Consumer` instead.

### SafeAreaContext

Use `SafeAreaInsetsContext` instead.

### initialWindowSafeAreaInsets

Use `initialWindowMetrics` instead.

## Web SSR

If you are doing server side rendering on the web you can use `initialMetrics` to inject insets and frame value based on the device the user has, or simply pass zero values. Since insets measurement is async it will break rendering your page content otherwise.

## Optimization

If you can, use `SafeAreaView`. It's implemented natively so when rotating the device, there is no delay from the asynchronous bridge.

To speed up the initial render, you can import `initialWindowMetrics` from this package and set as the `initialMetrics` prop on the provider as described in Web SSR. You cannot do this if your provider remounts, or you are using `react-native-navigation`.

```js
import {
  SafeAreaProvider,
  initialWindowMetrics,
} from 'react-native-safe-area-context';

function App() {
  return (
    <SafeAreaProvider initialMetrics={initialWindowMetrics}>
      ...
    </SafeAreaProvider>
  );
}
```

## Testing

This library includes a built in mock for Jest. It will use the following metrics by default:

```js
{
  frame: {
    width: 320,
    height: 640,
    x: 0,
    y: 0,
  },
  insets: {
    left: 0,
    right: 0,
    bottom: 0,
    top: 0,
  },
}
```

To use it, add the following code to the jest setup file:

```js
import mockSafeAreaContext from 'react-native-safe-area-context/jest/mock';

jest.mock('react-native-safe-area-context', () => mockSafeAreaContext);
```

To have more control over the test values it is also possible to pass `initialMetrics` to
`SafeAreaProvider` to provide mock data for frame and insets.

```js
export function TestSafeAreaProvider({ children }) {
  return (
    <SafeAreaProvider
      initialMetrics={{
        frame: { x: 0, y: 0, width: 0, height: 0 },
        insets: { top: 0, left: 0, right: 0, bottom: 0 },
      }}
    >
      {children}
    </SafeAreaProvider>
  );
}
```

## Contributing

See the [Contributing Guide](CONTRIBUTING.md)
