import { Source, SignalKind } from './types';
import { push, start } from './helpers';

/** A definition of the Callbag type as per its specification.
 * @see {@link https://github.com/callbag/callbag} for the Callbag specification.
 */
interface Callbag<I, O> {
  (t: 0, d: Callbag<O, I>): void;
  (t: 1, d: I): void;
  (t: 2, d?: any): void;
}

/** Converts a Callbag to a {@link Source}.
 * @param callbag - The {@link Callbag} object that will be converted.
 * @returns A {@link Source} wrapping the passed Callbag.
 *
 * @remarks
 * This converts a Callbag to a {@link Source}. When this Source receives a {@link Sink} and
 * the subscription starts, internally, it'll subscribe to the passed Callbag, passing through
 * all of its emitted values.
 */
export function fromCallbag<T>(callbag: Callbag<any, T>): Source<T> {
  return sink => {
    callbag(0, (signal: number, data: any) => {
      if (signal === 0) {
        sink(
          start(signal => {
            data(signal + 1);
          })
        );
      } else if (signal === 1) {
        sink(push(data));
      } else {
        sink(SignalKind.End);
      }
    });
  };
}

/** Converts a {@link Source} to a Callbag.
 * @param source - The {@link Source} that will be converted.
 * @returns A {@link Callbag} wrapping the passed Source.
 *
 * @remarks
 * This converts a {@link Source} to a {@link Callbag}. When this Callbag is subscribed to, it
 * internally subscribes to the Wonka Source and pulls new values.
 */
export function toCallbag<T>(source: Source<T>): Callbag<any, T> {
  return (signal: number, sink: any) => {
    if (signal === 0) {
      source(signal => {
        if (signal === SignalKind.End) {
          sink(2);
        } else if (signal.tag === SignalKind.Start) {
          sink(0, (num: number) => {
            if (num < 3) signal[0](num - 1);
          });
        } else {
          sink(1, signal[0]);
        }
      });
    }
  };
}
