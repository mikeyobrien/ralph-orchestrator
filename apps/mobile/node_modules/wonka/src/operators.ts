import { Push, Source, Sink, Operator, SignalKind, TalkbackKind, TalkbackFn } from './types';
import { push, start, talkbackPlaceholder } from './helpers';
import { fromArray } from './sources';

const identity = <T>(x: T): T => x;

/** Buffers values and emits the array of bufferd values each time a `notifier` Source emits.
 *
 * @param notifier - A {@link Source} that releases the current buffer.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `buffer` will buffer values from the input {@link Source}. When the passed `notifier` Source
 * emits, it will emit an array of all buffered values.
 *
 * This can be used to group values over time. A buffer will only be emitted when it contains any
 * values.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   buffer(interval(100)),
 *   subscribe(x => {
 *     console.log(text); // logs: [0], [1, 2], [3, 4]...
 *   })
 * );
 * ```
 */
export function buffer<S, T>(notifier: Source<S>): Operator<T, T[]> {
  return source => sink => {
    let buffer: T[] = [];
    let sourceTalkback = talkbackPlaceholder;
    let notifierTalkback = talkbackPlaceholder;
    let pulled = false;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        notifierTalkback(TalkbackKind.Close);
        if (buffer.length) sink(push(buffer));
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sourceTalkback = signal[0];
        notifier(signal => {
          if (ended) {
            /*noop*/
          } else if (signal === SignalKind.End) {
            ended = true;
            sourceTalkback(TalkbackKind.Close);
            if (buffer.length) sink(push(buffer));
            sink(SignalKind.End);
          } else if (signal.tag === SignalKind.Start) {
            notifierTalkback = signal[0];
          } else if (buffer.length) {
            const signal = push(buffer);
            buffer = [];
            sink(signal);
          }
        });
      } else {
        buffer.push(signal[0]);
        if (!pulled) {
          pulled = true;
          sourceTalkback(TalkbackKind.Pull);
          notifierTalkback(TalkbackKind.Pull);
        } else {
          pulled = false;
        }
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          sourceTalkback(TalkbackKind.Close);
          notifierTalkback(TalkbackKind.Close);
        } else if (!ended && !pulled) {
          pulled = true;
          sourceTalkback(TalkbackKind.Pull);
          notifierTalkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Emits in order from the Sources returned by a mapping function per value of the Source.
 *
 * @param map - A function returning a {@link Source} per value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `concatMap` accepts a mapping function which must return a {@link Source} per value.
 * The output {@link Source} will emit values from each Source the function returned, in order,
 * queuing sources that aren't yet active.
 *
 * This can be used to issue multiple values per emission of an input {@link Source}, while keeping
 * the order of their outputs consistent.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2]),
 *   concatMap(x => fromArray([x, x * 2])),
 *   subscribe(x => {
 *     console.log(text); // logs: 1, 2, 2, 4
 *   })
 * );
 * ```
 */
export function concatMap<In, Out>(map: (value: In) => Source<Out>): Operator<In, Out> {
  return source => sink => {
    const inputQueue: In[] = [];
    let outerTalkback = talkbackPlaceholder;
    let innerTalkback = talkbackPlaceholder;
    let outerPulled = false;
    let innerPulled = false;
    let innerActive = false;
    let ended = false;
    function applyInnerSource(innerSource: Source<Out>): void {
      innerActive = true;
      innerSource(signal => {
        if (signal === SignalKind.End) {
          if (innerActive) {
            innerActive = false;
            if (inputQueue.length) {
              applyInnerSource(map(inputQueue.shift()!));
            } else if (ended) {
              sink(SignalKind.End);
            } else if (!outerPulled) {
              outerPulled = true;
              outerTalkback(TalkbackKind.Pull);
            }
          }
        } else if (signal.tag === SignalKind.Start) {
          innerPulled = false;
          (innerTalkback = signal[0])(TalkbackKind.Pull);
        } else if (innerActive) {
          sink(signal);
          if (innerPulled) {
            innerPulled = false;
          } else {
            innerTalkback(TalkbackKind.Pull);
          }
        }
      });
    }
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        if (!innerActive && !inputQueue.length) sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        outerTalkback = signal[0];
      } else {
        outerPulled = false;
        if (innerActive) {
          inputQueue.push(signal[0]);
        } else {
          applyInnerSource(map(signal[0]));
        }
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          if (!ended) {
            ended = true;
            outerTalkback(TalkbackKind.Close);
          }
          if (innerActive) {
            innerActive = false;
            innerTalkback(TalkbackKind.Close);
          }
        } else {
          if (!ended && !outerPulled) {
            outerPulled = true;
            outerTalkback(TalkbackKind.Pull);
          }
          if (innerActive && !innerPulled) {
            innerPulled = true;
            innerTalkback(TalkbackKind.Pull);
          }
        }
      })
    );
  };
}

/** Flattens a Source emitting Sources into a single Source emitting the inner values in order.
 *
 * @see {@link concatMap} which this helper uses and instead accept a mapping function.
 * @param source - An {@link Source} emitting {@link Source | Sources}.
 * @returns A {@link Source} emitting values from the inner Sources.
 *
 * @remarks
 * `concatAll` accepts a {@link Source} emitting {@link Source | Sources}.
 * The output {@link Source} will emit values from each Source, in order, queuing sources that
 * aren't yet active.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([
 *     fromArray([1, 2]),
 *     fromArray([3, 4]),
 *   ]),
 *   concatAll,
 *   subscribe(x => {
 *     console.log(text); // logs: 1, 2, 3, 4
 *   })
 * );
 * ```
 */
export function concatAll<T>(source: Source<Source<T>>): Source<T> {
  return concatMap<Source<T>, T>(identity)(source);
}

/** Emits values from the passed sources in order.
 *
 * @param sources - An array of {@link Source | Sources}.
 * @returns A {@link Source} emitting values from the input Sources.
 *
 * @remarks
 * `concat` accepts an array of {@link Source | Sources} and will emit values from them, starting
 * with the first one and continuing to the next only when the prior source ended.
 *
 * This can be used to issue combine sources while keeping the order of their values intact.
 *
 * @example
 * ```ts
 * pipe(
 *   concat([
 *     fromArray([1, 2]),
 *     fromArray([3, 4]),
 *   ]),
 *   subscribe(x => {
 *     console.log(text); // logs: 1, 2, 3, 4
 *   })
 * );
 * ```
 */
export function concat<T>(sources: Source<T>[]): Source<T> {
  return concatAll(fromArray(sources));
}

/** Filters out emitted values for which the passed predicate function returns `false`.
 *
 * @param predicate - A function returning a boolean per value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `filter` will omit values from the {@link Source} for which the passed `predicate` function
 * returns `false`.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   filter(x => x % 2 === 0),
 *   subscribe(x => {
 *     console.log(text); // logs: 2
 *   })
 * );
 * ```
 */
function filter<In, Out extends In>(predicate: (value: In) => value is Out): Operator<In, Out>;
function filter<T>(predicate: (value: T) => boolean): Operator<T, T>;
function filter<In, Out>(predicate: (value: In) => boolean): Operator<In, Out> {
  return source => sink => {
    let talkback = talkbackPlaceholder;
    source(signal => {
      if (signal === SignalKind.End) {
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        talkback = signal[0];
        sink(signal);
      } else if (!predicate(signal[0])) {
        talkback(TalkbackKind.Pull);
      } else {
        sink(signal as Push<any>);
      }
    });
  };
}

export { filter };

/** Maps emitted values using the passed mapping function.
 *
 * @param map - A function returning transforming the {@link Source | Source's} values.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `map` accepts a transform function and calls it on each emitted value. It then emits
 * the values returned by the transform function instead.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   map(x => x * 2),
 *   subscribe(x => {
 *     console.log(text); // logs: 2, 4, 6
 *   })
 * );
 * ```
 */
export function map<In, Out>(map: (value: In) => Out): Operator<In, Out> {
  return source => sink =>
    source(signal => {
      if (signal === SignalKind.End || signal.tag === SignalKind.Start) {
        sink(signal);
      } else {
        sink(push(map(signal[0])));
      }
    });
}

/** Emits from the Sources returned by a mapping function per value of the Source.
 *
 * @param map - A function returning a {@link Source} per value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `mergeMap` accepts a mapping function which must return a {@link Source} per value.
 * The output {@link Source} will emit values from all {@link Source | Sources} the mapping function
 * returned.
 *
 * This can be used to issue multiple values per emission of an input {@link Source}, essentially
 * multiplexing all values to multiple Sources.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   mergeMap(x => pipe(
 *     fromValue(x),
 *     delay(100)
 *   )),
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 1, 2...
 *   })
 * );
 * ```
 */
export function mergeMap<In, Out>(map: (value: In) => Source<Out>): Operator<In, Out> {
  return source => sink => {
    let innerTalkbacks: TalkbackFn[] = [];
    let outerTalkback = talkbackPlaceholder;
    let outerPulled = false;
    let ended = false;
    function applyInnerSource(innerSource: Source<Out>): void {
      let talkback = talkbackPlaceholder;
      innerSource(signal => {
        if (signal === SignalKind.End) {
          if (innerTalkbacks.length) {
            const index = innerTalkbacks.indexOf(talkback);
            if (index > -1) (innerTalkbacks = innerTalkbacks.slice()).splice(index, 1);
            if (!innerTalkbacks.length) {
              if (ended) {
                sink(SignalKind.End);
              } else if (!outerPulled) {
                outerPulled = true;
                outerTalkback(TalkbackKind.Pull);
              }
            }
          }
        } else if (signal.tag === SignalKind.Start) {
          innerTalkbacks.push((talkback = signal[0]));
          talkback(TalkbackKind.Pull);
        } else if (innerTalkbacks.length) {
          sink(signal);
          talkback(TalkbackKind.Pull);
        }
      });
    }
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        if (!innerTalkbacks.length) sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        outerTalkback = signal[0];
      } else {
        outerPulled = false;
        applyInnerSource(map(signal[0]));
        if (!outerPulled) {
          outerPulled = true;
          outerTalkback(TalkbackKind.Pull);
        }
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          if (!ended) {
            ended = true;
            outerTalkback(TalkbackKind.Close);
          }
          for (let i = 0, a = innerTalkbacks, l = innerTalkbacks.length; i < l; i++)
            a[i](TalkbackKind.Close);
          innerTalkbacks.length = 0;
        } else {
          if (!ended && !outerPulled) {
            outerPulled = true;
            outerTalkback(TalkbackKind.Pull);
          } else {
            outerPulled = false;
          }
          for (let i = 0, a = innerTalkbacks, l = innerTalkbacks.length; i < l; i++)
            a[i](TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Flattens a Source emitting Sources into a single Source emitting the inner values.
 *
 * @see {@link mergeMap} which this helper uses and instead accept a mapping function.
 * @param source - An {@link Source} emitting {@link Source | Sources}.
 * @returns A {@link Source} emitting values from the inner Sources.
 *
 * @remarks
 * `mergeAll` accepts a {@link Source} which must emit {@link Source | Sources}. It will subscribe
 * to each incoming source immediately and start passing its emitted values through.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([
 *     interval(50),
 *     interval(100),
 *   ]),
 *   mergeAll,
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 0, 1, 2, 1, 3, 4, 2
 *   })
 * );
 * ```
 */
export function mergeAll<T>(source: Source<Source<T>>): Source<T> {
  return mergeMap<Source<T>, T>(identity)(source);
}

/** Emits values from the passed sources simultaneously.
 *
 * @param sources - An array of {@link Source | Sources}.
 * @returns A {@link Source} emitting values from the input Sources.
 *
 * @remarks
 * `merge` accepts an array of {@link Source | Sources} and will subscribe to all of them, passing
 * through all their emitted values simultaneously.
 *
 * This can be used to interleave the values of multiple sources.
 *
 * @example
 * ```ts
 * pipe(
 *   merge([
 *     interval(50),
 *     interval(100),
 *   ]),
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 0, 1, 2, 1, 3, 4, 2
 *   })
 * );
 * ```
 */
export function merge<T>(sources: Source<T>[]): Source<T> {
  return mergeAll(fromArray(sources));
}

/** Calls the passed callback function when the Source ends or is closed.
 *
 * @param callback - A function that is called when the {@link Source} ends.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `onEnd` accepts a callback which is called when the {@link Source} either ends
 * or is closed.
 *
 * This operator can be used to add side-effects to a Source.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   take(1),
 *   onEnd(() => {
 *     console.log('end');
 *   }),
 *   publish
 * );
 * ```
 */
export function onEnd<T>(callback: () => void): Operator<T, T> {
  return source => sink => {
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        sink(SignalKind.End);
        callback();
      } else if (signal.tag === SignalKind.Start) {
        const talkback = signal[0];
        sink(
          start(signal => {
            if (signal === TalkbackKind.Close) {
              ended = true;
              talkback(TalkbackKind.Close);
              callback();
            } else {
              talkback(signal);
            }
          })
        );
      } else {
        sink(signal);
      }
    });
  };
}

/** Calls the passed callback function when the Source emits a value.
 *
 * @param callback - A function that is called with each value the {@link Source} emits.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `onPush` accepts a callback which is called for every emitted value of
 * the {@link Source}.
 *
 * This operator can be used to add side-effects to a Source.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   onPush(value => {
 *     console.log(value); // logs: 1, 2, 3
 *   }),
 *   publish
 * );
 * ```
 */
export function onPush<T>(callback: (value: T) => void): Operator<T, T> {
  return source => sink => {
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        const talkback = signal[0];
        sink(
          start(signal => {
            if (signal === TalkbackKind.Close) ended = true;
            talkback(signal);
          })
        );
      } else {
        callback(signal[0]);
        sink(signal);
      }
    });
  };
}

/** Calls the passed callback function when the Source starts.
 *
 * @param callback - A function that is called when the {@link Source} is started.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `onPush` accepts a callback which is called for every emitted value of
 * the {@link Source}.
 *
 * This operator can be used to add side-effects to a Source.
 * Specifically, it's useful to add a side-effect for a Source that triggers only once
 * the {@link Source} is used and started.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   onStart(() => {
 *     console.log('start');
 *   }),
 *   publish
 * );
 * ```
 */
export function onStart<T>(callback: () => void): Operator<T, T> {
  return source => sink =>
    source(signal => {
      if (signal === SignalKind.End) {
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sink(signal);
        callback();
      } else {
        sink(signal);
      }
    });
}

/** Emits the last value the {@link Source} emitted, whenever the notifier Source emits a value.
 *
 * @param notifier - A {@link Source} that triggers the last value to be emitted.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `sample` will store the latest value the {@link Source} emitted. Every time the `notifier` Source
 * emits, it will emit the latest value.
 *
 * This is a back pressure operator that can be used to omit values from a {@link Source} coming in
 * too frequently.
 *
 * {@link Source | Sources} emitting `undefined` are undefined behaviour and these values will be
 * ignored.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   sample(interval(100)),
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 2, 4...
 *   })
 * );
 * ```
 */
export function sample<S, T>(notifier: Source<S>): Operator<T, T> {
  return source => sink => {
    let sourceTalkback = talkbackPlaceholder;
    let notifierTalkback = talkbackPlaceholder;
    let value: T | void;
    let pulled = false;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        notifierTalkback(TalkbackKind.Close);
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sourceTalkback = signal[0];
      } else {
        value = signal[0];
        if (!pulled) {
          pulled = true;
          notifierTalkback(TalkbackKind.Pull);
          sourceTalkback(TalkbackKind.Pull);
        } else {
          pulled = false;
        }
      }
    });
    notifier(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        sourceTalkback(TalkbackKind.Close);
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        notifierTalkback = signal[0];
      } else if (value !== undefined) {
        const signal = push(value);
        value = undefined;
        sink(signal);
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          sourceTalkback(TalkbackKind.Close);
          notifierTalkback(TalkbackKind.Close);
        } else if (!ended && !pulled) {
          pulled = true;
          sourceTalkback(TalkbackKind.Pull);
          notifierTalkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Maps emitted values using the passed reducer function.
 *
 * @param reducer - A function called with the last value by the `reducer` and the emitted value.
 * @param seed - The initial value that is passed to the `reducer`.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `scan` accepts a reducer function and a seed value. The reducer will be called initially with the
 * seed value and the first emitted value. The {@link Source} will then emit the value returned by
 * the reducer function. Subsequently, the `reducer` is called with the last value the `reducer`
 * returned and the emitted value.
 *
 * This operator is similar to `Array.prototype.reduce`, but instead is called over time and emits
 * each value of the reducer.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   scan((acc, x) => acc + x, 0),
 *   subscribe(x => {
 *     console.log(text); // logs: 1, 3, 6
 *   })
 * );
 * ```
 */
export function scan<In, Out>(reducer: (acc: Out, value: In) => Out, seed: Out): Operator<In, Out> {
  return source => sink => {
    let acc = seed;
    source(signal => {
      if (signal === SignalKind.End) {
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sink(signal);
      } else {
        sink(push((acc = reducer(acc, signal[0]))));
      }
    });
  };
}

/** Shares one underlying subscription to the Source between all Sinks.
 *
 * @param source - A {@link Source} that should be shared.
 * @returns A shared {@link Source}.
 *
 * @remarks
 * `share` accepts a {@link Source} and returns one. It will emit all values as normal, however, it
 * will share one subscription to the input source. This allows side-effects on the input
 * {@link Source} to only be triggerd once.
 */
export function share<T>(source: Source<T>): Source<T> {
  let sinks: Sink<T>[] = [];
  let talkback = talkbackPlaceholder;
  let gotSignal = false;
  return sink => {
    sinks.push(sink);
    if (sinks.length === 1) {
      source(signal => {
        if (signal === SignalKind.End) {
          for (let i = 0, a = sinks, l = sinks.length; i < l; i++) a[i](SignalKind.End);
          sinks.length = 0;
        } else if (signal.tag === SignalKind.Start) {
          talkback = signal[0];
        } else {
          gotSignal = false;
          for (let i = 0, a = sinks, l = sinks.length; i < l; i++) a[i](signal);
        }
      });
    }
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          const index = sinks.indexOf(sink);
          if (index > -1) (sinks = sinks.slice()).splice(index, 1);
          if (!sinks.length) talkback(TalkbackKind.Close);
        } else if (!gotSignal) {
          gotSignal = true;
          talkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Omits `wait` amount of values from the Source and then runs as usual.
 *
 * @param wait - The number of values to be omitted.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `skip` will skip `wait` number of emitted values, then issue all values as normal afterwards.
 * This essentially skips a given number of values on the input {@link Source}.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   skip(2),
 *   subscribe(x => {
 *     console.log(text); // logs: 3
 *   })
 * );
 * ```
 */
export function skip<T>(wait: number): Operator<T, T> {
  return source => sink => {
    let talkback = talkbackPlaceholder;
    let rest = wait;
    source(signal => {
      if (signal === SignalKind.End) {
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        talkback = signal[0];
        sink(signal);
      } else if (rest-- > 0) {
        talkback(TalkbackKind.Pull);
      } else {
        sink(signal);
      }
    });
  };
}

/** Omits values from an input Source until a notifier Source emits a value.
 *
 * @param notifier - A {@link Source} that starts the operator's sent values.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `skipUntil` will omit all values from the input {@link Source} until the `notifier`
 * Source emits a value of its own. It'll then start passing values from the Source through.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   skipUntil(interval(150)),
 *   subscribe(x => {
 *     console.log(text); // logs: 2, 3...
 *   })
 * );
 * ```
 */
export function skipUntil<S, T>(notifier: Source<S>): Operator<T, T> {
  return source => sink => {
    let sourceTalkback = talkbackPlaceholder;
    let notifierTalkback = talkbackPlaceholder;
    let skip = true;
    let pulled = false;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        if (skip) notifierTalkback(TalkbackKind.Close);
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sourceTalkback = signal[0];
        notifier(signal => {
          if (signal === SignalKind.End) {
            if (skip) {
              ended = true;
              sourceTalkback(TalkbackKind.Close);
            }
          } else if (signal.tag === SignalKind.Start) {
            (notifierTalkback = signal[0])(TalkbackKind.Pull);
          } else {
            skip = false;
            notifierTalkback(TalkbackKind.Close);
          }
        });
      } else if (!skip) {
        pulled = false;
        sink(signal);
      } else if (!pulled) {
        pulled = true;
        sourceTalkback(TalkbackKind.Pull);
        notifierTalkback(TalkbackKind.Pull);
      } else {
        pulled = false;
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          sourceTalkback(TalkbackKind.Close);
          if (skip) notifierTalkback(TalkbackKind.Close);
        } else if (!ended && !pulled) {
          pulled = true;
          if (skip) notifierTalkback(TalkbackKind.Pull);
          sourceTalkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Omits values from an input Source until a predicate function returns `false`.
 *
 * @param predicate - A function returning a boolean per value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `skipWhile` will omit all values from the input {@link Source} until the `predicate`
 * function returns `false`. When the `predicate` function returns `false`, the Source's values will
 * be passed through.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   skipWhile(x => x < 2),
 *   subscribe(x => {
 *     console.log(text); // logs: 2, 3
 *   })
 * );
 * ```
 */
export function skipWhile<T>(predicate: (value: T) => boolean): Operator<T, T> {
  return source => sink => {
    let talkback = talkbackPlaceholder;
    let skip = true;
    source(signal => {
      if (signal === SignalKind.End) {
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        talkback = signal[0];
        sink(signal);
      } else if (skip) {
        if (predicate(signal[0])) {
          talkback(TalkbackKind.Pull);
        } else {
          skip = false;
          sink(signal);
        }
      } else {
        sink(signal);
      }
    });
  };
}

/** Emits from the latest Source returned by a mapping function per value of the Source.
 *
 * @param map - A function returning a {@link Source} per value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `switchMap` accepts a mapping function which must return a {@link Source} per value.
 * The output {@link Source} will emit values from the latest Source the mapping function
 * returned. If a value is emitted while the last returned Source is still active, the prior Source
 * will be closed.
 *
 * This can be used to issue multiple values per emission of an input {@link Source}, while only
 * letting one of these sub-Sources be active at a time.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(100),
 *   switchMap(() => interval(50)),
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 0, 0...
 *   })
 * );
 * ```
 */
export function switchMap<In, Out>(map: (value: In) => Source<Out>): Operator<In, Out> {
  return source => sink => {
    let outerTalkback = talkbackPlaceholder;
    let innerTalkback = talkbackPlaceholder;
    let outerPulled = false;
    let innerPulled = false;
    let innerActive = false;
    let ended = false;
    function applyInnerSource(innerSource: Source<Out>): void {
      innerActive = true;
      innerSource(signal => {
        if (!innerActive) {
          /*noop*/
        } else if (signal === SignalKind.End) {
          innerActive = false;
          if (ended) {
            sink(SignalKind.End);
          } else if (!outerPulled) {
            outerPulled = true;
            outerTalkback(TalkbackKind.Pull);
          }
        } else if (signal.tag === SignalKind.Start) {
          innerPulled = false;
          (innerTalkback = signal[0])(TalkbackKind.Pull);
        } else {
          sink(signal);
          if (!innerPulled) {
            innerTalkback(TalkbackKind.Pull);
          } else {
            innerPulled = false;
          }
        }
      });
    }
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        if (!innerActive) sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        outerTalkback = signal[0];
      } else {
        if (innerActive) {
          innerTalkback(TalkbackKind.Close);
          innerTalkback = talkbackPlaceholder;
        }
        if (!outerPulled) {
          outerPulled = true;
          outerTalkback(TalkbackKind.Pull);
        } else {
          outerPulled = false;
        }
        applyInnerSource(map(signal[0]));
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) {
          if (!ended) {
            ended = true;
            outerTalkback(TalkbackKind.Close);
          }
          if (innerActive) {
            innerActive = false;
            innerTalkback(TalkbackKind.Close);
          }
        } else {
          if (!ended && !outerPulled) {
            outerPulled = true;
            outerTalkback(TalkbackKind.Pull);
          }
          if (innerActive && !innerPulled) {
            innerPulled = true;
            innerTalkback(TalkbackKind.Pull);
          }
        }
      })
    );
  };
}

/** Flattens a Source emitting Sources into a single Source emitting the inner values.
 *
 * @see {@link switchMap} which this helper uses and instead accept a mapping function.
 * @param source - An {@link Source} emitting {@link Source | Sources}.
 * @returns A {@link Source} emitting values from the inner Sources.
 *
 * @remarks
 * `switchAll` accepts a {@link Source} which must emit {@link Source | Sources}. Each time it
 * receives a {@link Source} it will close its prior subscription and subscribe to the new Source
 * instead, passing through its values.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(100),
 *   map(() => interval(50)),
 *   switchAll,
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 0, 0...
 *   })
 * );
 * ```
 */
export function switchAll<T>(source: Source<Source<T>>): Source<T> {
  return switchMap<Source<T>, T>(identity)(source);
}

/** Emits `max` values from the Source and then ends.
 *
 * @param max - The maximum number of values emitted.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `take` will issue all values as normal until the `max` number of emitted values has been reached.
 * It will then end and close the {@link Source}.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   take(2),
 *   subscribe(x => {
 *     console.log(text); // logs: 1, 2
 *   })
 * );
 * ```
 */
export function take<T>(max: number): Operator<T, T> {
  return source => sink => {
    let talkback = talkbackPlaceholder;
    let ended = false;
    let taken = 0;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        if (max <= 0) {
          ended = true;
          sink(SignalKind.End);
          signal[0](TalkbackKind.Close);
        } else {
          talkback = signal[0];
        }
      } else if (taken++ < max) {
        sink(signal);
        if (!ended && taken >= max) {
          ended = true;
          sink(SignalKind.End);
          talkback(TalkbackKind.Close);
        }
      } else {
        sink(signal);
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          talkback(TalkbackKind.Close);
        } else if (signal === TalkbackKind.Pull && !ended && taken < max) {
          talkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Buffers the `max` last values of the Source and emits them once the Source ends.
 *
 * @param max - The maximum number of values buffered.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `takeLast` will buffer values from the input {@link Source} up until the given `max` number. It
 * will only emit values stored in the buffer once the {@link Source} ends.
 *
 * All values in the buffer are emitted like the {@link fromArray | `fromArray`} source would
 * synchronously.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   takeLast(1),
 *   subscribe(x => {
 *     console.log(text); // logs: 3
 *   })
 * );
 * ```
 */
export function takeLast<T>(max: number): Operator<T, T> {
  return source => sink => {
    const queue: T[] = [];
    let talkback = talkbackPlaceholder;
    source(signal => {
      if (signal === SignalKind.End) {
        fromArray(queue)(sink);
      } else if (signal.tag === SignalKind.Start) {
        if (max <= 0) {
          signal[0](TalkbackKind.Close);
          fromArray(queue)(sink);
        } else {
          (talkback = signal[0])(TalkbackKind.Pull);
        }
      } else {
        if (queue.length >= max && max) queue.shift();
        queue.push(signal[0]);
        talkback(TalkbackKind.Pull);
      }
    });
  };
}

/** Takes values from an input Source until a notifier Source emits a value.
 *
 * @param notifier - A {@link Source} that stops the operator's sent values.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `takeUntil` will issue all values as normal from the input {@link Source} until the `notifier`
 * Source emits a value of its own. It'll then close the {@link Source}.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   takeUntil(interval(150)),
 *   subscribe(x => {
 *     console.log(text); // logs: 0, 1
 *   })
 * );
 * ```
 */
export function takeUntil<S, T>(notifier: Source<S>): Operator<T, T> {
  return source => sink => {
    let sourceTalkback = talkbackPlaceholder;
    let notifierTalkback = talkbackPlaceholder;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        notifierTalkback(TalkbackKind.Close);
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        sourceTalkback = signal[0];
        notifier(signal => {
          if (signal === SignalKind.End) {
            /*noop*/
          } else if (signal.tag === SignalKind.Start) {
            (notifierTalkback = signal[0])(TalkbackKind.Pull);
          } else {
            ended = true;
            notifierTalkback(TalkbackKind.Close);
            sourceTalkback(TalkbackKind.Close);
            sink(SignalKind.End);
          }
        });
      } else {
        sink(signal);
      }
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close && !ended) {
          ended = true;
          sourceTalkback(TalkbackKind.Close);
          notifierTalkback(TalkbackKind.Close);
        } else if (!ended) {
          sourceTalkback(TalkbackKind.Pull);
        }
      })
    );
  };
}

/** Takes values from an input Source until a predicate function returns `false`.
 *
 * @param predicate - A function returning a boolean per value.
 * @param addOne - Lets an additional input value pass on.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `takeWhile` will issue all values as normal from the input {@link Source} until the `predicate`
 * function returns `false`. When the `predicate` function returns `false`, the current value is
 * omitted and the {@link Source} is closed.
 *
 * If `addOne` is set to `true`, the value for which the `predicate` first returned `false` is
 * issued and passed on as well instead of being omitted.
 *
 * @example
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   takeWhile(x => x < 2),
 *   subscribe(x => {
 *     console.log(text); // logs: 1
 *   })
 * );
 * ```
 */
export function takeWhile<T>(predicate: (value: T) => boolean, addOne?: boolean): Operator<T, T> {
  return source => sink => {
    let talkback = talkbackPlaceholder;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        talkback = signal[0];
        sink(signal);
      } else if (!predicate(signal[0])) {
        ended = true;
        if (addOne) sink(signal);
        sink(SignalKind.End);
        talkback(TalkbackKind.Close);
      } else {
        sink(signal);
      }
    });
  };
}

/** Debounces a Source by omitting values until a given timeframe has passed.
 *
 * @param timing - A function returning a debounce time (ms) per emitted value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `debounce` accepts a mapping function that can be used to return a time (in ms) per emitted
 * value. All emitted values issued by the {@link Source} during the returned time will be omitted
 * until the time has passed.
 *
 * Debouncing means that the returned {@link Source} will wait for a minimum time of silence until a
 * value is let through.
 *
 * This is a back pressure operator that can be used to omit values from a {@link Source} coming in
 * too frequently.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   debounce(() => 100),
 *   subscribe(x => {
 *     console.log(text); // never logs any value
 *   })
 * );
 * ```
 */
export function debounce<T>(timing: (value: T) => number): Operator<T, T> {
  return source => sink => {
    let id: any | void;
    let deferredEnded = false;
    let ended = false;
    source(signal => {
      if (ended) {
        /*noop*/
      } else if (signal === SignalKind.End) {
        ended = true;
        if (id) {
          deferredEnded = true;
        } else {
          sink(SignalKind.End);
        }
      } else if (signal.tag === SignalKind.Start) {
        const talkback = signal[0];
        sink(
          start(signal => {
            if (signal === TalkbackKind.Close && !ended) {
              ended = true;
              deferredEnded = false;
              if (id) clearTimeout(id);
              talkback(TalkbackKind.Close);
            } else if (!ended) {
              talkback(TalkbackKind.Pull);
            }
          })
        );
      } else {
        if (id) clearTimeout(id);
        id = setTimeout(() => {
          id = undefined;
          sink(signal);
          if (deferredEnded) sink(SignalKind.End);
        }, timing(signal[0]));
      }
    });
  };
}

/** Delays each signal emitted by a Source by given time (ms).
 *
 * @param wait - A time (in ms) by which each {@link SignalKind | signal} is delayed.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `delay` accepts a time (in ms) by which each {@link SignalKind | signal} will be delayed by.
 * This will create a timeout per received signal and delay the emitted values accordingly.
 *
 * Since the operator only calls `setTimeout` per signal, it relies on the timeout implementation to
 * be ordered. Otherwise, signals will arrive in the wrong order at the sink.
 */
export function delay<T>(wait: number): Operator<T, T> {
  return source => sink => {
    let active = 0;
    source(signal => {
      if (signal !== SignalKind.End && signal.tag === SignalKind.Start) {
        sink(signal);
      } else {
        active++;
        setTimeout(() => {
          if (active) {
            active--;
            sink(signal);
          }
        }, wait);
      }
    });
  };
}

/** Throttles a Source by omitting values that are emitted before a given timeout.
 *
 * @param timing - A function returning a throttle time (ms) per emitted value.
 * @returns An {@link Operator}.
 *
 * @remarks
 * `throttle` accepts a mapping function that can be used to return a time (in ms) per emitted
 * value. During the returned timeframe all values issued by the {@link Source} will be omitted and
 * dropped.
 *
 * This is a back pressure operator that can be used to omit values from a {@link Source} coming in
 * too frequently.
 *
 * @example
 * ```ts
 * pipe(
 *   interval(50),
 *   throttle(() => 100),
 *   subscribe(x => {
 *     // omits every second value: 0, 2, 4...
 *     console.log(text);
 *   })
 * );
 * ```
 */
export function throttle<T>(timing: (value: T) => number): Operator<T, T> {
  return source => sink => {
    let skip = false;
    let id: any | void;
    source(signal => {
      if (signal === SignalKind.End) {
        if (id) clearTimeout(id);
        sink(SignalKind.End);
      } else if (signal.tag === SignalKind.Start) {
        const talkback = signal[0];
        sink(
          start(signal => {
            if (signal === TalkbackKind.Close) {
              if (id) clearTimeout(id);
              talkback(TalkbackKind.Close);
            } else {
              talkback(TalkbackKind.Pull);
            }
          })
        );
      } else if (!skip) {
        skip = true;
        if (id) clearTimeout(id);
        id = setTimeout(() => {
          id = undefined;
          skip = false;
        }, timing(signal[0]));
        sink(signal);
      }
    });
  };
}

export { mergeAll as flatten, onPush as tap };
