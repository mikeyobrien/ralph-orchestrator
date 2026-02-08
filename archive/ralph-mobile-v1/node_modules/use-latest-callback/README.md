# use-latest-callback

React hook which returns the latest callback without changing the reference.

This is useful for scenarios such as event listeners where you may not want to resubscribe when the callback changes.

## Installation

Open a Terminal in the project root and run:

```sh
npm install use-latest-callback
```

## Usage

The `useLatestCallback` hook accepts a function as its argument and returns a function that preserves its reference across renders.

```js
import useLatestCallback from 'use-latest-callback';

// ...

function MyComponent() {
  const callback = useLatestCallback((value) => {
    console.log('Changed', value);
  });

  React.useEffect(() => {
    someEvent.addListener(callback);

    return () => someEvent.removeListener(callback);
  }, [callback]);

  return <>{/* whatever */}</>;
}
```

It's important to note that the callback is not intended to be called during the render phase. Only call the callback in response to an event.
