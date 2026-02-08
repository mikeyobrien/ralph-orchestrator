<h1 align="center">
  <img src="https://raw.githubusercontent.com/software-mansion-labs/react-freeze/main/.github/images/react-freeze.png" width="170px"/><br/>
  React Freeze
</h1>
<p align="center" style="font-size: 130%">
Prevent React component subtrees from rendering.
</p>

# What is this? 🤔

This library lets you freeze the renders of parts of the React component tree using `Suspense` mechanism introduced in React 17.
The main use case of this library is to avoid unnecessary re-renders of parts of the app that are not visible to the user at a given moment.
The frozen components **are not unmounted** when they are replaced with a placeholder view, so their React state and corresponding native view instances are retained during the freeze (DOM elements for `react-dom` or platform-native views for React Native apps) keeping things like scroll position, input state, or loaded images (for `<img>` components) unchanged.

The most prominent use case is the navigation in React Native apps, which typically is based on a stack.
When opening a new screen, we push the screen onto the stack but also keep the previous screens on the stack in case the user goes back to them later.
Since we want to keep the state of those previous screens, the components rendering it are retained on the stack, which results in them receiving updates (e.g. redux store changes) and getting re-rendered, even though they are completely covered by other screens.
With `react-freeze`, we are able to suspend renders for the hidden screens and, as a result, save React from doing unnecessary computation (such as reconciliation, sending view change updates, etc.).

# Quick start with react-navigation (React Native) 🧭

> You have to be using React Native 0.64 or higher and react-navigation 5.x or 6.x.

Thanks to the fact [react-navigation](https://reactnavigation.org/) relies on [react-native-screens](https://github.com/software-mansion/react-native-screens) we published an updated version of the screens library that takes advantage of the information about which screens should be active.
In order to try react-freeze in your React Native app that uses React Navigation you need to upgrade react-native-screens to version 3.9.0:

```bash
yarn upgrade react-native-screens@3.9.0
```

If you're building for ios you also need to run

```bash
pod install --project-directory=ios
```

Then enable experimental freeze support using new method we added to screens library.
In order to do that, add the following snippet (along with the import) to the main file of your application:

```js
import { enableFreeze } from "react-native-screens";

enableFreeze(true);
```

The new react-native-screens library is compatible with React Navigation v5 and v6, however, when using React Navigation v5 you also need to enable "screens" support. This can be done by adding call to `enableScreens(true)` in your main application file and passing `detachInactiveScreens` option to your navigators (stack / tabs / etc.).

**IMPORTANT:** The current version of screens/freeze integration, freezes updates only on views that are more than one level deep the stack hierarchy (the top and second to top screens are not freezing). This is necessary for slide-to-go-back functionality to work as by sliding back we may reveal the content that is displayed below. This is something we plan on improving in the future such that only the top screen is not frozen.

# Quick start - using Freeze directly (React and React Native) ⚡

> In order to use this package you'll have to be using React 17 or higher, or React Native 0.64 or higher.

Install `react-freeze` package from npm:

```bash
yarn add react-freeze
```

Import `Freeze` component in your app:

```js
import { Freeze } from "react-freeze";
```

Wrap some components you want to freeze and pass `freeze` option to control whether renders in that components should be suspended:

```js
function SomeComponent({ shouldSuspendRendering }) {
  return (
    <Freeze freeze={shouldSuspendRendering}>
      <MyOtherComponent />
    </Freeze>
  );
}
```

# Component docs 📚

The `react-freeze` library exports a single component called `<Freeze>`.
It can be used as a boundary for components for which we want to suspend rendering.
This takes the following options:

### `freeze: boolean`

This options can be used to control whether components rendered under `Freeze` should or should not re-render.
If set to `true`, all renders for components from `Freeze` subtree will be suspended until the prop changes to `false`.
Additionally, during the time components are "frozen", `Freeze` component instead of rendering children will render component provided as `placeholder` parameter removing frozen components from screen while retaining their native view instances and state.

### `placeholder?: React.ReactNode`

This parameter can be used to customize what `Freeze` component should render when it is in the frozen state (`freeze={true}`).
This is an optional parameter and by default it renders `null`.
Note, that it is best to "freeze" only components that are not visible to the user at given moment, so in general customizing this should not be necessary.
However, if replacing frozen views with just `null` can break layout of you app, you can use this parameter to show a placeholder that will keep all non-frozen parts of your application in the same place.

# Known issues 😭

## React Native Debugger does not allow profiling

When profiling React-Native apps with [React Native Debugger](https://github.com/jhen0409/react-native-debugger) starting React profiler for the app with frozen components throws an error ("Error: Could not find ID for Fiber ...").

> Have other problems with react-freeze? Start a [New issue](//github.com/software-mansion-labs/react-freeze/issues).

# FAQ ❓

## When component subtree is frozen what happens to state changes that are executed on that subtree

All state changes are executed as usual, they just won't trigger a render of the updated component until the component comes back from the frozen state.

## What happens to the non-react state of the component after defrost? Like for example scroll position?

Since all the "native views" (DOM elements or platform-specific views in react native) are kept when the component is frozen their state (such as scroll position, text typed into text input fields, etc.) is restored when they come back from the frozen state.
In fact, they are just the same component (same DOM nodes for react-dom / same views for react-native).

## What happens when there is an update in a redux store that frozen component is subscribed to?

Redux and other state-management solutions rely on manually triggering re-renders for components that they want to update when the store state changes.
When component is frozen it won't render even when redux requests it, however methods such as `mapStateToProps` or selectors provided to `useSelector` will still run on store updates.
After the component comes back from frozen state, it will render and pick up the most up-to-date data from the store.

## Can freezing some of my app components break my app? And how?

There are few ways that we are aware of when `<Freeze>` can alter the app behavior:

1. When attempting to freeze parts of the app that is visible to the user at a given moment -- in this case the frozen part is going to be replaced by the placeholder (or just by nothing if no placeholder is provided). So unless you really want this behavior make sure to only set `freeze` to `true` when the given subtree should not be visible and you expect user to not interact with it. A good example are screens on the navigation stack that are down the stack hierarchy when you push more content.
2. When you rely on the frozen parts layout to properly position the unfrozen parts. Note that when component is in frozen state it gets replaced by a placeholder (or by nothing if you don't provide one). This may impact the layout of the rest of your application. This can be workaround by making placeholder take the same amount of space as the view it replaces, or by only freezing parts that are positioned absolutely (e.g. the component that takes up the whole screen).
3. When component render method has side-effects that relay on running for all prop/state updates. Typically, performing side-effects in render is undesirable when writing react code but can happen in your codebase nonetheless. Note that when subtree is frozen your component may not be rendered for all the state updates and render method won't execute for some of the changes that happen during that phase. However, when the component gets back from the frozen state it will render with the most up-to-date version of the state, and if that suffice for the side-effect logic to be correct you should be ok.

<br/>
<hr/>

Made by [Software Mansion](https://github.com/software-mansion) and licenced under [The MIT License](LICENSE).
