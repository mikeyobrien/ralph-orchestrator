import { Source, SignalKind, TalkbackKind } from './types';
import { push, start, talkbackPlaceholder, observableSymbol } from './helpers';

// NOTE: This must be placed in an exported file for `rollup-plugin-dts`
// to include it in output typings files
declare global {
  interface SymbolConstructor {
    readonly observable: symbol;
  }
}

/** A definition of the ES Observable Subscription type that is returned by
 * {@link Observable.subscribe}
 *
 * @remarks
 * The Subscription in ES Observables is a handle that is held while the Observable is actively
 * streaming values. As such, it's used to indicate with {@link ObservableSubscription.closed}
 * whether it's active, and {@link ObservableSubscription.unsubscribe} may be used to cancel the
 * ongoing subscription and end the {@link Observable} early.
 *
 * @see {@link https://github.com/tc39/proposal-observable} for the ES Observable specification.
 */
interface ObservableSubscription {
  /** A boolean flag indicating whether the subscription is closed.
   * @remarks
   * When `true`, the subscription will not issue new values to the {@link ObservableObserver} and
   * has terminated. No new values are expected.
   *
   * @readonly
   */
  closed: boolean;
  /** Cancels the subscription.
   * @remarks
   * This cancels the ongoing subscription and the {@link ObservableObserver}'s callbacks will
   * subsequently not be called at all. The subscription will be terminated and become inactive.
   */
  unsubscribe(): void;
}

/** A definition of the ES Observable Observer type that is used to receive data from an
 * {@link Observable}.
 *
 * @remarks
 * The Observer in ES Observables is supplied to {@link Observable.subscribe} to receive events from
 * an {@link Observable} as it issues them.
 *
 * @see {@link https://github.com/tc39/proposal-observable#observer} for the ES Observable
 * specification of an Observer.
 */
interface ObservableObserver<T> {
  /** Callback for the Observable issuing new values.
   * @param value - The value that the {@link Observable} is sending.
   */
  next(value: T): void;
  /** Callback for the Observable encountering an error, terminating it.
   * @param error - The error that the {@link Observable} has encountered.
   */
  error?(error: any): void;
  /** Callback for the Observable ending, after all values have been issued. */
  complete?(): void;
}

/** A looser definition of ES Observable-like types that is used for interoperability.
 * @remarks
 * The Observable is often used by multiple libraries supporting or creating streams to provide
 * interoperability for push-based streams. When converting from an Observable to a {@link Source},
 * this looser type is accepted as an input.
 *
 * @see {@link https://github.com/tc39/proposal-observable} for the ES Observable specification.
 * @see {@link Observable} for the full ES Observable type.
 */
interface ObservableLike<T> {
  /**
   * Subscribes to new signals from an {@link Observable} via callbacks.
   * @param observer - An object containing callbacks for the various events of an Observable.
   * @returns Subscription handle of type {@link ObservableSubscription}.
   *
   * @see {@link ObservableObserver} for the callbacks in an object that are called as Observables
   * issue events.
   */
  subscribe(observer: ObservableObserver<T>): { unsubscribe(): void };

  /** The well-known symbol specifying the default ES Observable for an object. */
  [Symbol.observable]?(): Observable<T>;
}

/** An ES Observable type that is a de-facto standard for push-based data sources across the JS
 * ecosystem.
 *
 * @remarks
 * The Observable is often used by multiple libraries supporting or creating streams to provide
 * interoperability for push-based streams. As Wonka's {@link Source | Sources} are similar in
 * functionality to Observables, it provides utilities to cleanly convert to and from Observables.
 *
 * @see {@link https://github.com/tc39/proposal-observable} for the ES Observable specification.
 */
interface Observable<T> {
  /** Subscribes to new signals from an {@link Observable} via callbacks.
   * @param observer - An object containing callbacks for the various events of an Observable.
   * @returns Subscription handle of type {@link ObservableSubscription}.
   *
   * @see {@link ObservableObserver} for the callbacks in an object that are called as Observables
   * issue events.
   */
  subscribe(observer: ObservableObserver<T>): ObservableSubscription;

  /** Subscribes to new signals from an {@link Observable} via callbacks.
   * @param onNext - Callback for the Observable issuing new values.
   * @param onError - Callback for the Observable encountering an error, terminating it.
   * @param onComplete - Callback for the Observable ending, after all values have been issued.
   * @returns Subscription handle of type {@link ObservableSubscription}.
   */
  subscribe(
    onNext: (value: T) => any,
    onError?: (error: any) => any,
    onComplete?: () => any
  ): ObservableSubscription;

  /** The well-known symbol specifying the default ES Observable for an object. */
  [Symbol.observable](): Observable<T>;
}

/** Converts an ES Observable to a {@link Source}.
 * @param input - The {@link ObservableLike} object that will be converted.
 * @returns A {@link Source} wrapping the passed Observable.
 *
 * @remarks
 * This converts an ES Observable to a {@link Source}. When this Source receives a {@link Sink} and
 * the subscription starts, internally, it'll subscribe to the passed Observable, passing through
 * all of the Observable's values. As such, this utility provides intercompatibility converting from
 * standard Observables to Wonka Sources.
 *
 * @throws
 * When the passed ES Observable throws, the error is simply re-thrown as {@link Source} does
 * not support or expect errors to be handled by streams.
 */
export function fromObservable<T>(input: ObservableLike<T>): Source<T> {
  return sink => {
    const subscription = (
      input[observableSymbol()] ? input[observableSymbol()]!() : input
    ).subscribe({
      next(value: T) {
        sink(push(value));
      },
      complete() {
        sink(SignalKind.End);
      },
      error(error) {
        throw error;
      },
    });
    sink(
      start(signal => {
        if (signal === TalkbackKind.Close) subscription.unsubscribe();
      })
    );
  };
}

/** Converts a {@link Source} to an ES Observable.
 * @param source - The {@link Source} that will be converted.
 * @returns An {@link Observable} wrapping the passed Source.
 *
 * @remarks
 * This converts a {@link Source} to an {@link Observable}. When this Observable is subscribed to, it
 * internally subscribes to the Wonka Source and pulls new values. As such, this utility provides
 * intercompatibility converting from Wonka Sources to standard ES Observables.
 */
export function toObservable<T>(source: Source<T>): Observable<T> {
  return {
    subscribe(
      next: ObservableObserver<T> | ((value: T) => any),
      error?: (error: any) => any | undefined,
      complete?: () => any | undefined
    ) {
      const observer: ObservableObserver<T> =
        typeof next == 'object' ? next : { next, error, complete };
      let talkback = talkbackPlaceholder;
      let ended = false;
      source(signal => {
        if (ended) {
          /*noop*/
        } else if (signal === SignalKind.End) {
          ended = true;
          if (observer.complete) observer.complete();
        } else if (signal.tag === SignalKind.Start) {
          (talkback = signal[0])(TalkbackKind.Pull);
        } else {
          observer.next(signal[0]);
          talkback(TalkbackKind.Pull);
        }
      });
      const subscription = {
        closed: false,
        unsubscribe() {
          subscription.closed = true;
          ended = true;
          talkback(TalkbackKind.Close);
        },
      };
      return subscription;
    },
    [observableSymbol()]() {
      return this;
    },
  };
}
