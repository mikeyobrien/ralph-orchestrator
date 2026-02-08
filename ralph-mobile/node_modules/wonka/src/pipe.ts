import { Source, Sink, Operator } from './types';

interface UnaryFn<T, R> {
  (source: T): R;
}

/** Chain calls operators on a given source and returns the last result.
 * @param args - A source, then a variable number of transform functions
 *
 * @remarks
 * The `pipe` utility can be called with a {@link Source} then one or more unary transform functions.
 * Each transform function will be called in turn with the last function's return value, starting
 * with the source passed as the first argument to `pipe`.
 *
 * It's used to transform a source with a list of {@link Operator | Operators}. The last argument may
 * also be a {@link Sink} that returns something else than a Source.
 *
 * @example
 *
 * ```ts
 * pipe(
 *   fromArray([1, 2, 3]),
 *   map(x => x * 2),
 *   subscribe(console.log)
 * );
 * ```
 *
 * @see {@link https://github.com/tc39/proposal-pipeline-operator} for the JS Pipeline Operator spec, for which this is a replacement utility for.
 */
interface pipe {
  /* pipe definitions for source + operators composition */

  <T, A>(source: Source<T>, op1: UnaryFn<Source<T>, Source<A>>): Source<A>;

  <T, A, B>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>
  ): Source<B>;

  <T, A, B, C>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>
  ): Source<C>;

  <T, A, B, C, D>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>
  ): Source<D>;

  <T, A, B, C, D, E>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>
  ): Source<E>;

  <T, A, B, C, D, E, F>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>
  ): Source<F>;

  <T, A, B, C, D, E, F, G>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>,
    op7: UnaryFn<Source<F>, Source<G>>
  ): Source<G>;

  <T, A, B, C, D, E, F, G, H>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>,
    op7: UnaryFn<Source<F>, Source<G>>,
    op8: UnaryFn<Source<G>, Source<H>>
  ): Source<H>;

  /* pipe definitions for source + operators + consumer composition */

  <T, R>(source: Source<T>, consumer: UnaryFn<Source<T>, R>): R;

  <T, A, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    consumer: UnaryFn<Source<A>, R>
  ): R;

  <T, A, B, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    consumer: UnaryFn<Source<B>, R>
  ): R;

  <T, A, B, C, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    consumer: UnaryFn<Source<C>, R>
  ): R;

  <T, A, B, C, D, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    consumer: UnaryFn<Source<D>, R>
  ): R;

  <T, A, B, C, D, E, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    consumer: UnaryFn<Source<E>, R>
  ): R;

  <T, A, B, C, D, E, F, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>,
    consumer: UnaryFn<Source<F>, R>
  ): R;

  <T, A, B, C, D, E, F, G, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>,
    op7: UnaryFn<Source<F>, Source<G>>,
    consumer: UnaryFn<Source<G>, R>
  ): R;

  <T, A, B, C, D, E, F, G, H, R>(
    source: Source<T>,
    op1: UnaryFn<Source<T>, Source<A>>,
    op2: UnaryFn<Source<A>, Source<B>>,
    op3: UnaryFn<Source<B>, Source<C>>,
    op4: UnaryFn<Source<C>, Source<D>>,
    op5: UnaryFn<Source<D>, Source<E>>,
    op6: UnaryFn<Source<E>, Source<F>>,
    op7: UnaryFn<Source<F>, Source<G>>,
    op8: UnaryFn<Source<G>, Source<H>>,
    consumer: UnaryFn<Source<H>, R>
  ): R;
}

const pipe: pipe = (...args: Function[]): any => {
  let x = args[0];
  for (let i = 1, l = args.length; i < l; i++) x = args[i](x);
  return x;
};

export { pipe };
