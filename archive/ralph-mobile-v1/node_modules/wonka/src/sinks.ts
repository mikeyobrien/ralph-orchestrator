import { Source, Subscription, TalkbackKind, SignalKind, SourceIterable } from './types';
import { talkbackPlaceholder, asyncIteratorSymbol } from './helpers';

/** Creates a subscription to a given source and invokes a `subscriber` callback for each value.
 * @param subscriber - A callback function called for each issued value.
 * @returns A function accepting a {@link Source} and returning a {@link Subscription}.
 *
 * @remarks
 * `subscribe` accepts a `subscriber` callback and returns a function accepting a {@link Source}.
 * When a source is passed to the returned funtion, the subscription will start and `subscriber`
 * will be called for each new value the Source issues. This will also return a {@link Subscription}
 * object that can cancel the ongoing {@link Source} early.
 *
 * @example
 * ```ts
 * const subscription = pipe(
 *   fromValue('test'),
 *   subscribe(text => {
 *     console.log(text); // 'test'
 *   })
 * );
 * ```
 */
export function subscribe<T>(subscriber: (value: T) => void) {
  return (source: Source<T>): Subscription => {
    let talkback = talkbackPlaceholder;
    let ended = false;
    source(signal => {
      if (signal === SignalKind.End) {
        ended = true;
      } else if (signal.tag === SignalKind.Start) {
        (talkback = signal[0])(TalkbackKind.Pull);
      } else if (!ended) {
        subscriber(signal[0]);
        talkback(TalkbackKind.Pull);
      }
    });
    return {
      unsubscribe() {
        if (!ended) {
          ended = true;
          talkback(TalkbackKind.Close);
        }
      },
    };
  };
}

/** Creates a subscription to a given source and invokes a `subscriber` callback for each value.
 * @see {@link subscribe} which this helper aliases without returnin a {@link Subscription}.
 * @param subscriber - A callback function called for each issued value.
 * @returns A function accepting a {@link Source}.
 *
 * @remarks
 * `forEach` accepts a `subscriber` callback and returns a function accepting a {@link Source}.
 * When a source is passed to the returned funtion, the subscription will start and `subscriber`
 * will be called for each new value the Source issues. Unlike `subscribe` it will not return a
 * Subscription object and can't be cancelled early.
 *
 * @example
 * ```ts
 * pipe(
 *   fromValue('test'),
 *   forEach(text => {
 *     console.log(text); // 'test'
 *   })
 * ); // undefined
 * ```
 */
export function forEach<T>(subscriber: (value: T) => void) {
  return (source: Source<T>): void => {
    subscribe(subscriber)(source);
  };
}

/** Creates a subscription to a given source and invokes a `subscriber` callback for each value.
 * @see {@link subscribe} which this helper aliases without accepting parameters or returning a
 * {@link Subscription | Subscription}.
 *
 * @param source - A {@link Source}.
 *
 * @remarks
 * `publish` accepts a {@link Source} and subscribes to it, starting its values. The resulting
 * values cannot be observed and the subscription can't be cancelled, as this helper is purely
 * intended to start side-effects.
 *
 * @example
 * ```ts
 * pipe(
 *   lazy(() => {
 *     console.log('test'); // this is called
 *     return fromValue(123); // this is never used
 *   }),
 *   publish
 * ); // undefined
 * ```
 */
export function publish<T>(source: Source<T>): void {
  subscribe(_value => {
    /*noop*/
  })(source);
}

const doneResult = { done: true } as IteratorReturnResult<void>;

/** Converts a Source to an AsyncIterable that pulls and issues values from the Source.
 *
 * @param source - A {@link Source}.
 * @returns An {@link AsyncIterable | `AsyncIterable`} issuing values from the Source.
 *
 * @remarks
 * `toAsyncIterable` will create an {@link AsyncIterable} that pulls and issues values from a given
 * {@link Source}. This can be used in many interoperability situations, to provide an iterable when
 * a consumer requires it.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols#the_async_iterator_and_async_iterable_protocols}
 * for the JS Iterable protocol.
 *
 * @example
 * ```ts
 * const iterable = toAsyncIterable(fromArray([1, 2, 3]));
 * for await (const value of iterable) {
 *   console.log(value); // outputs: 1, 2, 3
 * }
 * ```
 */
export const toAsyncIterable = <T>(source: Source<T>): SourceIterable<T> => {
  const buffer: T[] = [];

  let ended = false;
  let started = false;
  let pulled = false;
  let talkback = talkbackPlaceholder;
  let next: ((value: IteratorResult<T>) => void) | void;

  return {
    async next(): Promise<IteratorResult<T>> {
      if (!started) {
        started = true;
        source(signal => {
          if (ended) {
            /*noop*/
          } else if (signal === SignalKind.End) {
            if (next) next = next(doneResult);
            ended = true;
          } else if (signal.tag === SignalKind.Start) {
            pulled = true;
            (talkback = signal[0])(TalkbackKind.Pull);
          } else {
            pulled = false;
            if (next) {
              next = next({ value: signal[0], done: false });
            } else {
              buffer.push(signal[0]);
            }
          }
        });
      }

      if (ended && !buffer.length) {
        return doneResult;
      } else if (!ended && !pulled && buffer.length <= 1) {
        pulled = true;
        talkback(TalkbackKind.Pull);
      }

      return buffer.length
        ? { value: buffer.shift()!, done: false }
        : new Promise(resolve => (next = resolve));
    },
    async return(): Promise<IteratorReturnResult<void>> {
      if (!ended) next = talkback(TalkbackKind.Close);
      ended = true;
      return doneResult;
    },
    [asyncIteratorSymbol()](): SourceIterable<T> {
      return this;
    },
  };
};

/** Subscribes to a given source and collects all synchronous values into an array.
 * @param source - A {@link Source}.
 * @returns An array of values collected from the {@link Source}.
 *
 * @remarks
 * `toArray` accepts a {@link Source} and returns an array of all synchronously issued values from
 * this Source. It will issue {@link TalkbackKind.Pull | Pull signals} after every value it receives
 * and expects the Source to recursively issue values.
 *
 * Any asynchronously issued values will not be
 * added to the array and a {@link TalkbackKind.Close | Close signal} is issued by the sink before
 * returning the array.
 *
 * @example
 * ```ts
 * toArray(fromArray([1, 2, 3])); // [1, 2, 3]
 * ```
 */
export function toArray<T>(source: Source<T>): T[] {
  const values: T[] = [];
  let talkback = talkbackPlaceholder;
  let ended = false;
  source(signal => {
    if (signal === SignalKind.End) {
      ended = true;
    } else if (signal.tag === SignalKind.Start) {
      (talkback = signal[0])(TalkbackKind.Pull);
    } else {
      values.push(signal[0]);
      talkback(TalkbackKind.Pull);
    }
  });
  if (!ended) talkback(TalkbackKind.Close);
  return values;
}

/** Subscribes to a given source and returns a Promise that will resolve with the last value the
 * source issues.
 *
 * @param source - A {@link Source}.
 * @returns A {@link Promise} resolving to the last value of the {@link Source}.
 *
 * @remarks
 * `toPromise` will subscribe to the passed {@link Source} and resolve to the last value of it once
 * it receives the last value, as signaled by the {@link SignalKind.End | End signal}.
 *
 * To keep its implementation simple, padding sources that don't issue any values to `toPromise` is
 * undefined behaviour and `toPromise` will issue `undefined` in that case.
 *
 * The returned {@link Promise} delays its value by a microtick, using `Promise.resolve`.
 *
 * @example
 * ```ts
 * toPromise(fromValue('test')); // resolves: 'test'
 * ```
 */
export function toPromise<T>(source: Source<T>): Promise<T> {
  return new Promise(resolve => {
    let talkback = talkbackPlaceholder;
    let value: T | void;
    source(signal => {
      if (signal === SignalKind.End) {
        Promise.resolve(value!).then(resolve);
      } else if (signal.tag === SignalKind.Start) {
        (talkback = signal[0])(TalkbackKind.Pull);
      } else {
        value = signal[0];
        talkback(TalkbackKind.Pull);
      }
    });
  });
}
