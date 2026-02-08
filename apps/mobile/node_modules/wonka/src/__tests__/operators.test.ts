import { describe, it, expect, beforeEach, vi } from 'vitest';

import { Source, Sink, Signal, SignalKind, TalkbackKind, TalkbackFn } from '../types';
import { push, start } from '../helpers';

import {
  passesPassivePull,
  passesActivePush,
  passesSinkClose,
  passesSourceEnd,
  passesSingleStart,
  passesStrictEnd,
  passesSourcePushThenEnd,
  passesAsyncSequence,
  passesCloseAndEnd,
} from './compliance';

import * as sources from '../sources';
import * as sinks from '../sinks';
import * as operators from '../operators';

beforeEach(() => {
  vi.useFakeTimers();
});

describe('buffer', () => {
  const valueThenNever: Source<any> = sink =>
    sink(
      start(signal => {
        if (signal === TalkbackKind.Pull) sink(push(null));
      })
    );

  const noop = operators.buffer(valueThenNever);

  passesPassivePull(noop, [0]);
  passesActivePush(noop, [0]);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop, [0]);
  passesSingleStart(noop);
  passesStrictEnd(noop);

  it('emits batches of input values when a notifier emits', () => {
    const { source: notifier$, next: notify } = sources.makeSubject();
    const { source: input$, next } = sources.makeSubject();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.buffer(notifier$)(input$));

    next(1);
    next(2);
    expect(fn).not.toHaveBeenCalled();

    notify(null);
    expect(fn).toHaveBeenCalledWith([1, 2]);

    next(3);
    notify(null);
    expect(fn).toHaveBeenCalledWith([3]);
  });
});

describe('concatMap', () => {
  const noop = operators.concatMap(x => sources.fromValue(x));
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  // This synchronous test for concatMap will behave the same as mergeMap & switchMap
  it('emits values from each flattened synchronous source', () => {
    const { source, next, complete } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.concatMap((x: number) => sources.fromArray([x, x + 1]))(source)(fn);

    next(1);
    next(3);
    complete();

    expect(fn).toHaveBeenCalledTimes(6);
    expect(fn.mock.calls).toEqual([
      [start(expect.any(Function))],
      [push(1)],
      [push(2)],
      [push(3)],
      [push(4)],
      [SignalKind.End],
    ]);
  });

  // This synchronous test for concatMap will behave the same as mergeMap & switchMap
  it('lets inner sources finish when outer source ends', () => {
    const signals: Signal<any>[] = [];
    const teardown = vi.fn();
    const fn = (signal: Signal<any>) => {
      signals.push(signal);
      if (signal !== SignalKind.End && signal.tag === SignalKind.Start) {
        signal[0](TalkbackKind.Pull);
        signal[0](TalkbackKind.Close);
      }
    };

    operators.concatMap(() => {
      return sources.make(() => teardown);
    })(sources.fromValue(null))(fn);

    expect(teardown).toHaveBeenCalled();
    expect(signals).toEqual([start(expect.any(Function))]);
  });

  // This asynchronous test for concatMap will behave differently than mergeMap & switchMap
  it('emits values from each flattened asynchronous source, one at a time', () => {
    const source = operators.delay<number>(4)(sources.fromArray([1, 10]));
    const fn = vi.fn();

    sinks.forEach(fn)(
      operators.concatMap((x: number) => {
        return operators.delay(5)(sources.fromArray([x, x * 2]));
      })(source)
    );

    vi.advanceTimersByTime(14);
    expect(fn.mock.calls).toEqual([[1], [2]]);

    vi.runAllTimers();
    expect(fn.mock.calls).toEqual([[1], [2], [10], [20]]);
  });

  it('works for fully asynchronous sources', () => {
    const fn = vi.fn();

    sinks.forEach(fn)(
      operators.concatMap(() => {
        return sources.make(observer => {
          setTimeout(() => observer.next(1));
          return () => {};
        });
      })(sources.fromValue(null))
    );

    vi.runAllTimers();
    expect(fn).toHaveBeenCalledWith(1);
  });

  it('emits synchronous values in order', () => {
    const values: any[] = [];

    sinks.forEach(x => values.push(x))(
      operators.concat([sources.fromArray([1, 2]), sources.fromArray([3, 4])])
    );

    expect(values).toEqual([1, 2, 3, 4]);
  });
});

describe('debounce', () => {
  const noop = operators.debounce(() => 0);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  it('waits for a specified amount of silence before emitting the last value', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.debounce(() => 100)(source));

    next(1);
    vi.advanceTimersByTime(50);
    expect(fn).not.toHaveBeenCalled();

    next(2);
    vi.advanceTimersByTime(99);
    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledWith(2);
  });

  it('emits debounced value with delayed End signal', () => {
    const { source, next, complete } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.debounce(() => 100)(source));

    next(1);
    complete();
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalled();
  });
});

describe('delay', () => {
  const noop = operators.delay(0);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('delays outputs by a specified delay timeout value', () => {
    const { source, next } = sources.makeSubject();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.delay(100)(source));

    next(1);
    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledWith(1);
  });
});

describe('filter', () => {
  const noop = operators.filter(() => true);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('prevents emissions for which a predicate fails', () => {
    const { source, next } = sources.makeSubject<boolean>();
    const fn = vi.fn();

    sinks.forEach((x: true) => {
      fn(x);
    })(operators.filter((x): x is true => !!x)(source));

    next(false);
    expect(fn).not.toHaveBeenCalled();

    next(true);
    expect(fn).toHaveBeenCalledWith(true);
  });
});

describe('map', () => {
  const noop = operators.map(x => x);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('maps over values given a transform function', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.map((x: number) => x + 1)(source));

    next(1);
    expect(fn).toHaveBeenCalledWith(2);
  });
});

describe('mergeMap', () => {
  const noop = operators.mergeMap(x => sources.fromValue(x));
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  // This synchronous test for mergeMap will behave the same as concatMap & switchMap
  it('emits values from each flattened synchronous source', () => {
    const { source, next, complete } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.mergeMap((x: number) => sources.fromArray([x, x + 1]))(source)(fn);

    next(1);
    next(3);
    complete();

    expect(fn.mock.calls).toEqual([
      [start(expect.any(Function))],
      [push(1)],
      [push(2)],
      [push(3)],
      [push(4)],
      [SignalKind.End],
    ]);
  });

  // This synchronous test for mergeMap will behave the same as concatMap & switchMap
  it('lets inner sources finish when outer source ends', () => {
    const values: Signal<any>[] = [];
    const teardown = vi.fn();
    const fn = (signal: Signal<any>) => {
      values.push(signal);
      if (signal !== SignalKind.End && signal.tag === SignalKind.Start) {
        signal[0](TalkbackKind.Pull);
        signal[0](TalkbackKind.Close);
      }
    };

    operators.mergeMap(() => {
      return sources.make(() => teardown);
    })(sources.fromValue(null))(fn);

    expect(teardown).toHaveBeenCalled();
    expect(values).toEqual([start(expect.any(Function))]);
  });

  // This asynchronous test for mergeMap will behave differently than concatMap & switchMap
  it('emits values from each flattened asynchronous source simultaneously', () => {
    const source = operators.delay<number>(4)(sources.fromArray([1, 10]));
    const fn = vi.fn();

    sinks.forEach(fn)(
      operators.mergeMap((x: number) => {
        return operators.delay(5)(sources.fromArray([x, x * 2]));
      })(source)
    );

    vi.runAllTimers();
    expect(fn.mock.calls).toEqual([[1], [10], [2], [20]]);
  });

  it('emits synchronous values in order', () => {
    const values: any[] = [];

    sinks.forEach(x => values.push(x))(
      operators.merge([sources.fromArray([1, 2]), sources.fromArray([3, 4])])
    );

    expect(values).toEqual([1, 2, 3, 4]);
  });
});

describe('onEnd', () => {
  const noop = operators.onEnd(() => {});
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesStrictEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('calls a callback when the source ends', () => {
    const { source, next, complete } = sources.makeSubject<any>();
    const fn = vi.fn();

    sinks.forEach(() => {})(operators.onEnd(fn)(source));

    next(null);
    expect(fn).not.toHaveBeenCalled();

    complete();
    expect(fn).toHaveBeenCalled();
  });
});

describe('onPush', () => {
  const noop = operators.onPush(() => {});
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesStrictEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('calls a callback when the source emits', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(() => {})(operators.onPush(fn)(source));

    next(1);
    expect(fn).toHaveBeenCalledWith(1);
    next(2);
    expect(fn).toHaveBeenCalledWith(2);
  });

  it('is the same as `tap`', () => {
    expect(operators.onPush).toBe(operators.tap);
  });
});

describe('onStart', () => {
  const noop = operators.onStart(() => {});
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('is called when the source starts', () => {
    let sink: Sink<any>;

    const fn = vi.fn();
    const source: Source<any> = _sink => {
      sink = _sink;
    };

    sinks.forEach(() => {})(operators.onStart(fn)(source));

    expect(fn).not.toHaveBeenCalled();

    sink!(start(() => {}));
    expect(fn).toHaveBeenCalled();
  });
});

describe('sample', () => {
  const valueThenNever: Source<any> = sink =>
    sink(
      start(signal => {
        if (signal === TalkbackKind.Pull) sink(push(null));
      })
    );

  const noop = operators.sample(valueThenNever);

  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);

  it('emits the latest value when a notifier source emits', () => {
    const { source: notifier$, next: notify } = sources.makeSubject();
    const { source: input$, next } = sources.makeSubject();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.sample(notifier$)(input$));

    next(1);
    next(2);
    expect(fn).not.toHaveBeenCalled();

    notify(null);
    expect(fn).toHaveBeenCalledWith(2);
  });
});

describe('scan', () => {
  const noop = operators.scan<any, any>((_acc, x) => x, null);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('folds values continuously with a reducer and initial value', () => {
    const { source: input$, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    const reducer = (acc: number, x: number) => acc + x;
    sinks.forEach(fn)(operators.scan(reducer, 0)(input$));

    next(1);
    expect(fn).toHaveBeenCalledWith(1);
    next(2);
    expect(fn).toHaveBeenCalledWith(3);
  });
});

describe('share', () => {
  const noop = operators.share;
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  it('shares output values between sinks', () => {
    let onPush = () => {};

    const source: Source<any> = operators.share(sink => {
      sink(start(() => {}));
      onPush = () => {
        sink(push([0]));
        sink(SignalKind.End);
      };
    });

    const fnA = vi.fn();
    const fnB = vi.fn();

    sinks.forEach(fnA)(source);
    sinks.forEach(fnB)(source);
    onPush();

    expect(fnA).toHaveBeenCalledWith([0]);
    expect(fnB).toHaveBeenCalledWith([0]);
    expect(fnA.mock.calls[0][0]).toBe(fnB.mock.calls[0][0]);
  });

  it('completes the source when no more sink is listening', () => {
    let onPush = () => {};

    const talkback = vi.fn();
    const source: Source<any> = operators.share(sink => {
      sink(start(talkback));
      onPush = () => {
        sink(push([0]));
        sink(push([1]));
        sink(SignalKind.End);
      };
    });

    const fnA = vi.fn();
    const fnB = vi.fn();

    sinks.forEach(fnA)(operators.take(1)(source));
    sinks.forEach(fnB)(operators.take(1)(source));
    onPush();

    expect(fnA).toHaveBeenCalledWith([0]);
    expect(fnB).toHaveBeenCalledWith([0]);
    expect(fnA.mock.calls[0][0]).toBe(fnB.mock.calls[0][0]);
    expect(talkback).toHaveBeenCalledWith(TalkbackKind.Close);
  });
});

describe('skip', () => {
  const noop = operators.skip(0);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('skips a number of values before emitting normally', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.skip(1)(source));

    next(1);
    expect(fn).not.toHaveBeenCalled();
    next(2);
    expect(fn).toHaveBeenCalledWith(2);
  });
});

describe('skipUntil', () => {
  const noop = operators.skipUntil(sources.fromValue(null));
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);
  passesStrictEnd(noop);

  it('skips values until the notifier source emits', () => {
    const { source: notifier$, next: notify } = sources.makeSubject();
    const { source: input$, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.skipUntil(notifier$)(input$));

    next(1);
    expect(fn).not.toHaveBeenCalled();
    notify(null);
    next(2);
    expect(fn).toHaveBeenCalledWith(2);
  });
});

describe('skipWhile', () => {
  const noop = operators.skipWhile(() => false);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('skips values until one fails a predicate', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.skipWhile((x: any) => x <= 1)(source));

    next(1);
    expect(fn).not.toHaveBeenCalled();
    next(2);
    expect(fn).toHaveBeenCalledWith(2);
  });
});

describe('switchMap', () => {
  const noop = operators.switchMap(x => sources.fromValue(x));
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  // This synchronous test for switchMap will behave the same as concatMap & mergeMap
  it('emits values from each flattened synchronous source', () => {
    const { source, next, complete } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.switchMap((x: number) => sources.fromArray([x, x + 1]))(source)(fn);

    next(1);
    next(3);
    complete();

    expect(fn).toHaveBeenCalledTimes(6);
    expect(fn.mock.calls).toEqual([
      [start(expect.any(Function))],
      [push(1)],
      [push(2)],
      [push(3)],
      [push(4)],
      [SignalKind.End],
    ]);
  });

  // This synchronous test for switchMap will behave the same as concatMap & mergeMap
  it('lets inner sources finish when outer source ends', () => {
    const signals: Signal<any>[] = [];
    const teardown = vi.fn();
    const fn = (signal: Signal<any>) => {
      signals.push(signal);
      if (signal !== SignalKind.End && signal.tag === SignalKind.Start) {
        signal[0](TalkbackKind.Pull);
        signal[0](TalkbackKind.Close);
      }
    };

    operators.switchMap(() => {
      return sources.make(() => teardown);
    })(sources.fromValue(null))(fn);

    expect(teardown).toHaveBeenCalled();
    expect(signals).toEqual([start(expect.any(Function))]);
  });

  // This asynchronous test for switchMap will behave differently than concatMap & mergeMap
  it('emits values from each flattened asynchronous source, one at a time', () => {
    const source = operators.delay<number>(4)(sources.fromArray([1, 10]));
    const fn = vi.fn();

    sinks.forEach(fn)(
      operators.switchMap((x: number) =>
        operators.take(2)(operators.map((y: number) => x * (y + 1))(sources.interval(5)))
      )(source)
    );

    vi.runAllTimers();
    expect(fn.mock.calls).toEqual([[1], [10], [20]]);
  });
});

describe('take', () => {
  const noop = operators.take(10);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  passesCloseAndEnd(operators.take(0));

  it('emits values until a maximum is reached', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.take(1)(source)(fn);
    next(1);

    expect(fn).toHaveBeenCalledTimes(3);
    expect(fn.mock.calls).toEqual([[start(expect.any(Function))], [push(1)], [SignalKind.End]]);
  });
});

describe('takeUntil', () => {
  const noop = operators.takeUntil(sources.never);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourcePushThenEnd(noop);
  passesSingleStart(noop);
  passesStrictEnd(noop);
  passesAsyncSequence(noop);

  const ending = operators.takeUntil(sources.fromValue(null));
  passesCloseAndEnd(ending);

  it('emits values until a notifier emits', () => {
    const { source: notifier$, next: notify } = sources.makeSubject<any>();
    const { source: input$, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.takeUntil(notifier$)(input$)(fn);
    next(1);

    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn.mock.calls).toEqual([[start(expect.any(Function))], [push(1)]]);

    notify(null);
    expect(fn).toHaveBeenCalledTimes(3);
    expect(fn.mock.calls[2][0]).toEqual(SignalKind.End);
  });

  it('emits values until a notifier emits', () => {
    const { source: input$, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    let hasClosed = false;

    operators.takeUntil(sink => {
      sink(
        start(talkback => {
          if (talkback === TalkbackKind.Close) {
            hasClosed = true;
          } else if (talkback === TalkbackKind.Pull && !hasClosed) {
            sink(push(1));
          }
        })
      );
    })(input$)(fn);

    next(1);

    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn.mock.calls).toEqual([[0], [start(expect.any(Function))]]);

    expect(hasClosed).toBe(true);
  });
});

describe('takeWhile', () => {
  const noop = operators.takeWhile(() => true);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  const ending = operators.takeWhile(() => false);
  passesCloseAndEnd(ending);

  it('emits values while a predicate passes for all values', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.takeWhile((x: any) => x < 2)(source)(fn);
    next(1);
    next(2);
    next(3);

    expect(fn.mock.calls).toEqual([[start(expect.any(Function))], [push(1)], [SignalKind.End]]);
  });

  it('emits values while a predicate passes for all values plus an additional one', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    operators.takeWhile((x: any) => x < 2, true)(source)(fn);
    next(1);
    next(2);
    next(3);

    expect(fn.mock.calls).toEqual([
      [start(expect.any(Function))],
      [push(1)],
      [push(2)],
      [SignalKind.End],
    ]);
  });
});

describe('takeLast', () => {
  passesCloseAndEnd(operators.takeLast(0));

  it('emits the last max values of an ended source', () => {
    const { source, next, complete } = sources.makeSubject<number>();
    const signals: Signal<any>[] = [];

    let talkback: TalkbackFn;

    operators.takeLast(1)(source)(signal => {
      signals.push(signal);
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Start) {
        (talkback = signal[0])(TalkbackKind.Pull);
      } else {
        talkback!(TalkbackKind.Pull);
      }
    });

    next(1);
    next(2);

    expect(signals.length).toBe(0);
    complete();

    expect(signals).toEqual([start(expect.any(Function)), push(2), SignalKind.End]);
  });
});

describe('throttle', () => {
  const noop = operators.throttle(() => 0);
  passesPassivePull(noop);
  passesActivePush(noop);
  passesSinkClose(noop);
  passesSourceEnd(noop);
  passesSingleStart(noop);
  passesAsyncSequence(noop);

  it('should ignore emissions for a period of time after a value', () => {
    const { source, next } = sources.makeSubject<number>();
    const fn = vi.fn();

    sinks.forEach(fn)(operators.throttle(() => 100)(source));

    next(1);
    expect(fn).toHaveBeenCalledWith(1);
    vi.advanceTimersByTime(50);

    next(2);
    expect(fn).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(50);

    next(3);
    expect(fn).toHaveBeenCalledWith(3);
  });
});
