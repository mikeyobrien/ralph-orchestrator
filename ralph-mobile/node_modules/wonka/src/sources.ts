import { Source, Sink, SignalKind, TalkbackKind, Observer, Subject, TeardownFn } from './types';
import {
  push,
  start,
  talkbackPlaceholder,
  teardownPlaceholder,
  asyncIteratorSymbol,
} from './helpers';
import { share } from './operators';

/** Helper creating a Source from a factory function when it's subscribed to.
 * @param produce - A factory function returning a {@link Source}.
 * @returns A {@link Source} lazyily subscribing to the Source returned by the given factory
 *   function.
 *
 * @remarks
 * At times it's necessary to create a {@link Source} lazily. The time of a {@link Source} being
 * created could be different from when it's subscribed to, and hence we may want to split the
 * creation and subscription time. This is especially useful when the Source we wrap is "hot" and
 * issues values as soon as it's created, which we may then not receive in a subscriber.
 *
 * @example An example of creating a {@link Source} that issues the timestamp of subscription. Here
 * we effectively use `lazy` with the simple {@link fromValue | `fromValue`} source, to quickly
 * create a Source that issues the time of its subscription, rather than the time of its creation
 * that it would otherwise issue without `lazy`.
 *
 * ```ts
 * lazy(() => fromValue(Date.now()));
 * ```
 */
export function lazy<T>(produce: () => Source<T>): Source<T> {
  return sink => produce()(sink);
}

/** Converts an AsyncIterable to a Source that pulls and issues values from it as requested.
 *
 * @see {@link fromIterable | `fromIterable`} for the non-async Iterable version of this helper,
 * which calls this helper automatically as needed.
 *
 * @param iterable - An {@link AsyncIterable | `AsyncIterable`}.
 * @returns A {@link Source} issuing values sourced from the Iterable.
 *
 * @remarks
 * `fromAsyncIterable` will create a {@link Source} that pulls and issues values from a given
 * {@link AsyncIterable}. This can be used in many interoperability situations, including to consume
 * an async generator function.
 *
 * When the {@link Sink} throws an exception when a new value is pushed, this helper will rethrow it
 * using {@link AsyncIterator.throw}, which allows an async generator to recover from the exception.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols#the_async_iterator_and_async_iterable_protocols}
 * for the JS Iterable protocol.
 */
export function fromAsyncIterable<T>(iterable: AsyncIterable<T> | AsyncIterator<T>): Source<T> {
  return sink => {
    const iterator: AsyncIterator<T> =
      (iterable[asyncIteratorSymbol()] && iterable[asyncIteratorSymbol()]()) || iterable;

    let ended = false;
    let looping = false;
    let pulled = false;
    let next: IteratorResult<T>;
    sink(
      start(async signal => {
        if (signal === TalkbackKind.Close) {
          ended = true;
          if (iterator.return) iterator.return();
        } else if (looping) {
          pulled = true;
        } else {
          for (pulled = looping = true; pulled && !ended; ) {
            if ((next = await iterator.next()).done) {
              ended = true;
              if (iterator.return) await iterator.return();
              sink(SignalKind.End);
            } else {
              try {
                pulled = false;
                sink(push(next.value));
              } catch (error) {
                if (iterator.throw) {
                  if ((ended = !!(await iterator.throw(error)).done)) sink(SignalKind.End);
                } else {
                  throw error;
                }
              }
            }
          }
          looping = false;
        }
      })
    );
  };
}

/** Converts an Iterable to a Source that pulls and issues values from it as requested.
 * @see {@link fromAsyncIterable | `fromAsyncIterable`} for the AsyncIterable version of this helper.
 * @param iterable - An {@link Iterable | `Iterable`} or an `AsyncIterable`
 * @returns A {@link Source} issuing values sourced from the Iterable.
 *
 * @remarks
 * `fromIterable` will create a {@link Source} that pulls and issues values from a given
 * {@link Iterable | JS Iterable}. As iterables are the common standard for any lazily iterated list
 * of values in JS it can be applied to many different JS data types, including a JS Generator
 * function.
 *
 * This Source will only call {@link Iterator.next} on the iterator when the subscribing {@link Sink}
 * has pulled a new value with the {@link TalkbackKind.Pull | Pull signal}. `fromIterable` can
 * therefore also be applied to "infinite" iterables, without a predefined end.
 *
 * This helper will call {@link fromAsyncIterable | `fromAsyncIterable`} automatically when the
 * passed object also implements the async iterator protocol.
 *
 * When the {@link Sink} throws an exception when a new value is pushed, this helper will rethrow it
 * using {@link Iterator.throw}, which allows a generator to recover from the exception.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols#the_iterable_protocol}
 * for the JS Iterable protocol.
 */
export function fromIterable<T>(iterable: Iterable<T> | AsyncIterable<T>): Source<T> {
  if (iterable[Symbol.asyncIterator]) return fromAsyncIterable(iterable as AsyncIterable<T>);
  return sink => {
    const iterator = iterable[Symbol.iterator]();
    let ended = false;
    let looping = false;
    let pulled = false;
    let next: IteratorResult<T>;
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          ended = true;
          if (iterator.return) iterator.return();
        } else if (looping) {
          pulled = true;
        } else {
          for (pulled = looping = true; pulled && !ended; ) {
            if ((next = iterator.next()).done) {
              ended = true;
              if (iterator.return) iterator.return();
              sink(SignalKind.End);
            } else {
              try {
                pulled = false;
                sink(push(next.value));
              } catch (error) {
                if (iterator.throw) {
                  if ((ended = !!iterator.throw(error).done)) sink(SignalKind.End);
                } else {
                  throw error;
                }
              }
            }
          }
          looping = false;
        }
      })
    );
  };
}

/** Creates a Source that issues a each value of a given array synchronously.
 * @see {@link fromIterable} which `fromArray` aliases.
 * @param array - The array whose values will be issued one by one.
 * @returns A {@link Source} issuing the array's values.
 *
 * @remarks
 * `fromArray` will create a {@link Source} that issues the values of a given JS array one by one. It
 * will issue values as they're pulled and is hence a "cold" source, not eagerly emitting values. It
 * will end and issue the {@link SignalKind.End | End signal} when the array is exhausted of values.
 *
 * @example
 * ```ts
 * fromArray([1, 2, 3]);
 * ```
 */
export const fromArray: <T>(array: T[]) => Source<T> = fromIterable;

/** Creates a Source that issues a single value and ends immediately after.
 * @param value - The value that will be issued.
 * @returns A {@link Source} issuing the single value.
 *
 * @example
 * ```ts
 * fromValue('test');
 * ```
 */
export function fromValue<T>(value: T): Source<T> {
  return sink => {
    let ended = false;
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          ended = true;
        } else if (!ended) {
          ended = true;
          sink(push(value));
          sink(SignalKind.End);
        }
      })
    );
  };
}

/** Creates a new Source from scratch from a passed `subscriber` function.
 * @param subscriber - A callback that is called when the {@link Source} is subscribed to.
 * @returns A {@link Source} created from the `subscriber` parameter.
 *
 * @remarks
 * `make` is used to create a new, arbitrary {@link Source} from scratch. It calls the passed
 * `subscriber` function when it's subscribed to.
 *
 * The `subscriber` function receives an {@link Observer}. You may call {@link Observer.next} to
 * issue values on the Source, and {@link Observer.complete} to end the Source.
 *
 * Your `subscribr` function must return a {@link TeardownFn | teardown function} which is only
 * called when your source is cancelled — not when you invoke `complete` yourself. As this creates a
 * "cold" source, every time this source is subscribed to, it will invoke the `subscriber` function
 * again and create a new source.
 *
 * @example
 *
 * ```ts
 * make(observer => {
 *   const frame = requestAnimationFrame(() => {
 *     observer.next('animate!');
 *   });
 *   return () => {
 *     cancelAnimationFrame(frame);
 *   };
 * });
 * ```
 */
export function make<T>(subscriber: (observer: Observer<T>) => TeardownFn): Source<T> {
  return sink => {
    let ended = false;
    const teardown = subscriber({
      next(value: T) {
        if (!ended) sink(push(value));
      },
      complete() {
        if (!ended) {
          ended = true;
          sink(SignalKind.End);
        }
      },
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          teardown();
        }
      })
    );
  };
}

/** Creates a new Subject which can be used as an IO event hub.
 * @returns A new {@link Subject}.
 *
 * @remarks
 * `makeSubject` creates a new {@link Subject}. A Subject is a {@link Source} and an {@link Observer}
 * combined in one interface, as the Observer is used to send new signals to the Source. This means
 * that it's "hot" and hence all subscriptions to {@link Subject.source} share the same underlying
 * signals coming from {@link Subject.next} and {@link Subject.complete}.
 *
 * @example
 * ```ts
 * const subject = makeSubject();
 * pipe(subject.source, subscribe(console.log));
 * // This will log the string on the above subscription
 * subject.next('hello subject!');
 * ```
 */
export function makeSubject<T>(): Subject<T> {
  let next: Subject<T>['next'] | void;
  let complete: Subject<T>['complete'] | void;
  return {
    source: share(
      make(observer => {
        next = observer.next;
        complete = observer.complete;
        return teardownPlaceholder;
      })
    ),
    next(value: T) {
      if (next) next(value);
    },
    complete() {
      if (complete) complete();
    },
  };
}

/** A {@link Source} that immediately ends.
 * @remarks
 * `empty` is a {@link Source} that immediately issues an {@link SignalKind.End | End signal} when
 * it's subscribed to, ending immediately.
 *
 * @see {@link never | `never`} for a source that instead never ends.
 */
export const empty: Source<any> = (sink: Sink<any>): void => {
  let ended = false;
  sink(
    start(signal => {
      if (signal === TalkbackKind.Close) {
        ended = true;
      } else if (!ended) {
        ended = true;
        sink(SignalKind.End);
      }
    })
  );
};

/** A {@link Source} without values that never ends.
 * @remarks
 * `never` is a {@link Source} that never issues any signals and neither sends values nor ends.
 *
 * @see {@link empty | `empty`} for a source that instead ends immediately.
 */
export const never: Source<any> = (sink: Sink<any>): void => {
  sink(start(talkbackPlaceholder));
};

/** Creates a Source that issues an incrementing integer in intervals.
 * @param ms - The interval in milliseconds.
 * @returns A {@link Source} issuing an incrementing count on each interval.
 *
 * @remarks
 * `interval` will create a {@link Source} that issues an incrementing counter each time the `ms`
 * interval expires.
 *
 * It'll only stop when it's cancelled by a {@link TalkbackKind.Close | Close signal}.
 *
 * @example
 * An example printing `0`, then `1`, and so on, in intervals of 50ms.
 *
 * ```ts
 * pipe(interval(50), subscribe(console.log));
 * ```
 */
export function interval(ms: number): Source<number> {
  return make(observer => {
    let i = 0;
    const id = setInterval(() => observer.next(i++), ms);
    return () => clearInterval(id);
  });
}

/** Converts DOM Events to a Source given an `HTMLElement` and an event's name.
 * @param element - The {@link HTMLElement} to listen to.
 * @param event - The DOM Event name to listen to.
 * @returns A {@link Source} issuing the {@link Event | DOM Events} as they're issued by the DOM.
 *
 * @remarks
 * `fromDomEvent` will create a {@link Source} that listens to the given element's events and issues
 * them as values on the source. This source will only stop when it's cancelled by a
 * {@link TalkbackKind.Close | Close signal}.
 *
 * @example
 * An example printing `'clicked!'` when the given `#root` element is clicked.
 *
 * ```ts
 * const element = document.getElementById('root');
 * pipe(
 *   fromDomEvent(element, 'click'),
 *   subscribe(() => console.log('clicked!'))
 * );
 * ```
 */
export function fromDomEvent(element: HTMLElement, event: string): Source<Event> {
  return make(observer => {
    element.addEventListener(event, observer.next);
    return () => element.removeEventListener(event, observer.next);
  });
}

/** Converts a Promise to a Source that issues the resolving Promise's value and then ends.
 * @param promise - The promise that will be wrapped.
 * @returns A {@link Source} issuing the promise's value when it resolves.
 *
 * @remarks
 * `fromPromise` will create a {@link Source} that issues the {@link Promise}'s resolving value
 * asynchronously and ends immediately after resolving.
 *
 * This helper will not handle the promise's exceptions, and will cause uncaught errors if the
 * promise rejects without a value.
 *
 * @example
 * An example printing `'resolved!'` when the given promise resolves after a tick.
 *
 * ```ts
 * pipe(fromPromise(Promise.resolve('resolved!')), subscribe(console.log));
 * ```
 */
export function fromPromise<T>(promise: Promise<T>): Source<T> {
  return make(observer => {
    promise.then(value => {
      Promise.resolve(value).then(() => {
        observer.next(value);
        observer.complete();
      });
    });
    return teardownPlaceholder;
  });
}
