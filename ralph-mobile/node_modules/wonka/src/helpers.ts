import { TalkbackFn, TeardownFn, Start, Push, SignalKind } from './types';

/** Placeholder {@link TeardownFn | teardown functions} that's a no-op.
 * @see {@link TeardownFn} for the definition and usage of teardowns.
 * @internal
 */
export const teardownPlaceholder: TeardownFn = () => {
  /*noop*/
};

/** Placeholder {@link TalkbackFn | talkback function} that's a no-op.
 * @privateRemarks
 * This is frequently used in the codebase as a no-op initializer value for talkback functions in
 * the implementation of {@link Operator | Operators}. This is cheaper than initializing the
 * variables of talkbacks to `undefined` or `null` and performing an extra check before calling
 * them. Since the {@link Start | Start signal} is assumed to come first and carry a talkback, we can
 * use this to our advantage and use a no-op placeholder before {@link Start} is received.
 *
 * @internal
 */
export const talkbackPlaceholder: TalkbackFn = teardownPlaceholder;

/** Wraps the passed {@link TalkbackFn | talkback function} in a {@link Start | Start signal}.
 * @internal
 */
export function start<T>(talkback: TalkbackFn): Start<T> {
  return {
    tag: SignalKind.Start,
    0: talkback,
  } as Start<T>;
}

/** Wraps the passed value in a {@link Push | Push signal}.
 * @internal
 */
export function push<T>(value: T): Push<T> {
  return {
    tag: SignalKind.Push,
    0: value,
  } as Push<T>;
}

/** Returns the well-known symbol specifying the default AsyncIterator.
 * @internal
 */
export const asyncIteratorSymbol = (): typeof Symbol.asyncIterator =>
  (typeof Symbol === 'function' && Symbol.asyncIterator) || ('@@asyncIterator' as any);

/** Returns the well-known symbol specifying the default ES Observable.
 * @privateRemarks
 * This symbol is used to mark an object as a default ES Observable. By the specification, an object
 * that abides by the default Observable implementation must carry a method set to this well-known
 * symbol that returns the Observable implementation. It's common for this object to be an
 * Observable itself and return itself on this method.
 *
 * @see {@link https://github.com/0no-co/wonka/issues/122} for notes on the intercompatibility
 * between Observable implementations.
 *
 * @internal
 */
export const observableSymbol = (): typeof Symbol.observable =>
  (typeof Symbol === 'function' && Symbol.observable) || ('@@observable' as any);
