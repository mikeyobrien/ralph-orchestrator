import { Source, TypeOfSource, SignalKind, TalkbackKind, TalkbackFn } from './types';
import { push, start, talkbackPlaceholder } from './helpers';

type TypeOfSourceArray<T extends readonly [...any[]]> = T extends [infer Head, ...infer Tail]
  ? [TypeOfSource<Head>, ...TypeOfSourceArray<Tail>]
  : [];

/** Combines the latest values of several sources into a Source issuing either tuple or dictionary
 * values.
 *
 * @param sources - Either an array or dictionary object of Sources.
 * @returns A {@link Source} issuing a zipped value whenever any input Source updates.
 *
 * @remarks
 * `zip` combines several {@link Source | Sources}. The resulting Source will issue its first value
 * once all input Sources have at least issued one value, and will subsequently issue a new value
 * each time any of the Sources emits a new value.
 *
 * Depending on whether an array or dictionary object of Sources is passed to `zip`, its emitted
 * values will be arrays or dictionary objects of the Sources' values.
 *
 * @example
 * An example of passing a dictionary object to `zip`. If an array is passed, the resulting
 * values will output arrays of the sources' values instead.
 *
 * ```ts
 * pipe(
 *   zip({
 *     x: fromValue(1),
 *     y: fromArray([2, 3]),
 *   }),
 *   subscribe(result => {
 *     // logs { x: 1, y: 2 } then { x: 1, y: 3 }
 *     console.log(result);
 *   })
 * );
 * ```
 */
interface zip {
  <Sources extends readonly [...Source<any>[]]>(sources: [...Sources]): Source<
    TypeOfSourceArray<Sources>
  >;

  <Sources extends { [prop: string]: Source<any> }>(sources: Sources): Source<{
    [Property in keyof Sources]: TypeOfSource<Sources[Property]>;
  }>;
}

function zip<T>(sources: Source<T>[] | Record<string, Source<T>>): Source<T[] | Record<string, T>> {
  const size = Object.keys(sources).length;
  return sink => {
    const filled: Set<string | number> = new Set();

    const talkbacks: TalkbackFn[] | Record<string, TalkbackFn | void> = Array.isArray(sources)
      ? new Array(size).fill(talkbackPlaceholder)
      : {};
    const buffer: T[] | Record<string, T> = Array.isArray(sources) ? new Array(size) : {};

    let gotBuffer = false;
    let gotSignal = false;
    let ended = false;
    let endCount = 0;

    for (const key in sources) {
      (sources[key] as Source<T>)(signal => {
        if (signal === SignalKind.End) {
          if (endCount >= size - 1) {
            ended = true;
            sink(SignalKind.End);
          } else {
            endCount++;
          }
        } else if (signal.tag === SignalKind.Start) {
          talkbacks[key] = signal[0];
        } else if (!ended) {
          buffer[key] = signal[0];
          filled.add(key);
          if (!gotBuffer && filled.size < size) {
            if (!gotSignal) {
              for (const key in sources)
                if (!filled.has(key)) (talkbacks[key] || talkbackPlaceholder)(TalkbackKind.Pull);
            } else {
              gotSignal = false;
            }
          } else {
            gotBuffer = true;
            gotSignal = false;
            sink(push(Array.isArray(buffer) ? buffer.slice() : { ...buffer }));
          }
        }
      });
    }
    sink(
      start(signal => {
        if (ended) {
          /*noop*/
        } else if (signal === TalkbackKind.Close) {
          ended = true;
          for (const key in talkbacks) talkbacks[key](TalkbackKind.Close);
        } else if (!gotSignal) {
          gotSignal = true;
          for (const key in talkbacks) talkbacks[key](TalkbackKind.Pull);
        }
      })
    );
  };
}

export { zip };

/** Combines the latest values of all passed sources into a Source issuing tuple values.
 *
 * @see {@link zip | `zip`} which this helper wraps and uses.
 * @param sources - A variadic list of {@link Source} parameters.
 * @returns A {@link Source} issuing a zipped value whenever any input Source updates.
 *
 * @remarks
 * `combine` takes one or more {@link Source | Sources} as arguments. Once all input Sources have at
 * least issued one value it will issue an array of all of the Sources' values. Subsequently, it
 * will issue a new array value whenever any of the Sources update.
 *
 * @example
 *
 * ```ts
 * pipe(
 *   combine(fromValue(1), fromValue(2)),
 *   subscribe(result => {
 *     console.log(result); // logs [1, 2]
 *   })
 * );
 * ```
 */
export function combine<Sources extends Source<any>[]>(
  ...sources: Sources
): Source<TypeOfSourceArray<Sources>> {
  return zip(sources) as Source<any>;
}
