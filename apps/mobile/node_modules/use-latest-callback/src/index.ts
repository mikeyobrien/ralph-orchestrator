import * as React from 'react';

/**
 * Use `useEffect` during SSR and `useLayoutEffect` in the Browser & React Native to avoid warnings.
 */
const useClientLayoutEffect =
  typeof document !== 'undefined' ||
  (typeof navigator !== 'undefined' && navigator.product === 'ReactNative')
    ? React.useLayoutEffect
    : React.useEffect;

/**
 * React hook which returns the latest callback without changing the reference.
 */
// eslint-disable-next-line @typescript-eslint/ban-types
function useLatestCallback<T extends Function>(callback: T): T {
  const ref = React.useRef<T>(callback);

  const latestCallback = React.useRef(function latestCallback(
    this: unknown,
    ...args: unknown[]
  ) {
    return ref.current.apply(this, args);
  } as unknown as T).current;

  useClientLayoutEffect(() => {
    ref.current = callback;
  });

  return latestCallback;
}

// Use export assignment to compile to module.exports =
export = useLatestCallback;
