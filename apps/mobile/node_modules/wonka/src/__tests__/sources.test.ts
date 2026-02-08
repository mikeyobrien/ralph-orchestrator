import { describe, it, expect, beforeEach, vi } from 'vitest';

import { Source, Sink, Signal, SignalKind, TalkbackKind, TalkbackFn } from '../types';
import { push, start, talkbackPlaceholder } from '../helpers';

import * as sources from '../sources';
import * as operators from '../operators';
import * as callbag from '../callbag';
import * as observable from '../observable';

import callbagFromArray from 'callbag-from-iter';
import Observable from 'zen-observable';

const collectSignals = (source: Source<any>, onStart?: (talkbackCb: TalkbackFn) => void) => {
  let talkback = talkbackPlaceholder;
  const signals: Signal<any>[] = [];
  source(signal => {
    signals.push(signal);
    if (signal === SignalKind.End) {
      /*noop*/
    } else if (signal.tag === SignalKind.Start) {
      talkback = signal[0];
      if (onStart) onStart(talkback);
      talkback(TalkbackKind.Pull);
    } else {
      talkback(TalkbackKind.Pull);
    }
  });

  return signals;
};

/* When a Close talkback signal is sent the source should immediately end */
const passesActiveClose = (source: Source<any>) => {
  it('stops emitting when a Close talkback signal is received (spec)', () => {
    let talkback: TalkbackFn | null = null;
    const sink: Sink<any> = signal => {
      expect(signal).not.toBe(SignalKind.End);
      expect((signal as any).tag).not.toBe(SignalKind.Push);
      if ((signal as any).tag === SignalKind.Start) {
        (talkback = signal[0])(TalkbackKind.Close);
      }
    };
    source(sink);
    expect(talkback).not.toBe(null);
  });
};

/* All synchronous, cold sources won't send anything unless a Pull signal
  has been received. */
const passesColdPull = (source: Source<any>) => {
  it('sends nothing when no Pull talkback signal has been sent (spec)', () => {
    let talkback: TalkbackFn | null = null;
    let pushes = 0;

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Push) {
        pushes++;
      } else {
        talkback = signal[0];
      }
    };

    source(sink);
    expect(talkback).not.toBe(null);
    expect(pushes).toBe(0);

    setTimeout(() => {
      expect(pushes).toBe(0);
      talkback!(TalkbackKind.Pull);
    }, 10);

    vi.runAllTimers();
    expect(pushes).toBe(1);
  });
};

/* All synchronous, cold sources need to use trampoline scheduling to avoid
  recursively sending more and more Push signals which would eventually lead
  to a call stack overflow when too many values are emitted. */
const passesTrampoline = (source: Source<any>) => {
  it('uses trampoline scheduling instead of recursive push signals (spec)', () => {
    let talkback: TalkbackFn | null = null;
    let pushes = 0;

    const signals: Signal<any>[] = [];
    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        signals.push(signal);
        expect(pushes).toBe(2);
      } else if (signal.tag === SignalKind.Push) {
        const lastPushes = ++pushes;
        signals.push(signal);
        talkback!(TalkbackKind.Pull);
        expect(lastPushes).toBe(pushes);
      } else if (signal.tag === SignalKind.Start) {
        (talkback = signal[0])(TalkbackKind.Pull);
        expect(pushes).toBe(2);
      }
    };

    source(sink);
    expect(signals).toEqual([push(1), push(2), SignalKind.End]);
  });
};

beforeEach(() => {
  vi.useFakeTimers();
});

describe('fromArray', () => {
  passesTrampoline(sources.fromArray([1, 2]));
  passesColdPull(sources.fromArray([0]));
  passesActiveClose(sources.fromArray([0]));
});

describe('fromValue', () => {
  passesColdPull(sources.fromValue(0));
  passesActiveClose(sources.fromValue(0));

  it('sends a single value and ends', () => {
    expect(collectSignals(sources.fromValue(1))).toEqual([
      start(expect.any(Function)),
      push(1),
      SignalKind.End,
    ]);
  });
});

describe('merge', () => {
  const source = operators.merge<any>([sources.fromValue(0), sources.empty]);

  passesColdPull(source);
  passesActiveClose(source);

  it('correctly merges two sources where the second is empty', () => {
    const source = operators.merge<any>([sources.fromValue(0), sources.empty]);

    expect(collectSignals(source)).toEqual([start(expect.any(Function)), push(0), SignalKind.End]);
  });

  it('correctly merges hot sources', () => {
    const onStart = vi.fn();
    const source = operators.merge<any>([
      operators.onStart(onStart)(sources.never),
      operators.onStart(onStart)(sources.fromArray([1, 2])),
    ]);

    const signals = collectSignals(source);
    expect(onStart).toHaveBeenCalledTimes(2);

    expect(signals).toEqual([start(expect.any(Function)), push(1), push(2)]);
  });

  it('correctly merges asynchronous sources', () => {
    vi.useFakeTimers();

    const onStart = vi.fn();
    const source = operators.merge<any>([
      operators.onStart(onStart)(sources.fromValue(-1)),
      operators.onStart(onStart)(operators.take(2)(sources.interval(50))),
    ]);

    const signals = collectSignals(source);
    vi.advanceTimersByTime(100);
    expect(onStart).toHaveBeenCalledTimes(2);

    expect(signals).toEqual([
      start(expect.any(Function)),
      push(-1),
      push(0),
      push(1),
      SignalKind.End,
    ]);
  });
});

describe('concat', () => {
  const source = operators.concat<any>([sources.fromValue(0), sources.empty]);

  passesColdPull(source);
  passesActiveClose(source);

  it('correctly concats two sources where the second is empty', () => {
    const source = operators.concat<any>([sources.fromValue(0), sources.empty]);

    expect(collectSignals(source)).toEqual([start(expect.any(Function)), push(0), SignalKind.End]);
  });
});

describe('make', () => {
  it('may be used to create async sources', () => {
    const teardown = vi.fn();
    const source = sources.make(observer => {
      setTimeout(() => observer.next(1), 10);
      setTimeout(() => observer.complete(), 20);
      return teardown;
    });

    const signals = collectSignals(source);
    expect(signals).toEqual([start(expect.any(Function))]);
    vi.runAllTimers();

    expect(signals).toEqual([start(expect.any(Function)), push(1), SignalKind.End]);
  });

  it('supports active cancellation', () => {
    const teardown = vi.fn();
    const source = sources.make(() => teardown);

    const sink: Sink<any> = signal => {
      expect(signal).not.toBe(SignalKind.End);
      expect((signal as any).tag).not.toBe(SignalKind.Push);
      setTimeout(() => signal[0](TalkbackKind.Close));
    };

    source(sink);
    expect(teardown).not.toHaveBeenCalled();
    vi.runAllTimers();
    expect(teardown).toHaveBeenCalled();
  });
});

describe('makeSubject', () => {
  it('may be used to emit signals programmatically', () => {
    const { source, next, complete } = sources.makeSubject();
    const signals = collectSignals(source);

    expect(signals).toEqual([start(expect.any(Function))]);

    next(1);

    expect(signals).toEqual([start(expect.any(Function)), push(1)]);

    complete();

    expect(signals).toEqual([start(expect.any(Function)), push(1), SignalKind.End]);
  });

  it('ignores signals after complete has been called', () => {
    const { source, next, complete } = sources.makeSubject();
    const signals = collectSignals(source);
    complete();

    expect(signals).toEqual([start(expect.any(Function)), SignalKind.End]);

    next(1);
    complete();
    expect(signals.length).toBe(2);
  });
});

describe('never', () => {
  it('emits nothing and ends immediately', () => {
    const signals = collectSignals(sources.never);
    expect(signals).toEqual([start(expect.any(Function))]);
  });
});

describe('empty', () => {
  it('emits nothing and ends immediately', () => {
    const signals = collectSignals(sources.empty);

    expect(signals).toEqual([start(expect.any(Function)), SignalKind.End]);
  });
});

describe('fromPromise', () => {
  passesActiveClose(sources.fromPromise(Promise.resolve(null)));

  it('emits a value when the promise resolves', async () => {
    const promise = Promise.resolve(1);
    const signals = collectSignals(sources.fromPromise(promise));

    expect(signals).toEqual([start(expect.any(Function))]);

    await Promise.resolve();
    await promise;
    await Promise.resolve();

    expect(signals).toEqual([start(expect.any(Function)), push(1), SignalKind.End]);
  });
});

describe('fromObservable', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('converts an Observable to a Wonka source', async () => {
    const source = observable.fromObservable(Observable.from([1, 2]));
    const signals = collectSignals(source);

    await new Promise(resolve => setTimeout(resolve));

    expect(signals).toEqual([start(expect.any(Function)), push(1), push(2), SignalKind.End]);
  });

  it('supports cancellation on converted Observables', async () => {
    const source = observable.fromObservable(Observable.from([1, 2]));
    const signals = collectSignals(source, talkback => {
      talkback(TalkbackKind.Close);
    });

    await new Promise(resolve => setTimeout(resolve));

    expect(signals).toEqual([start(expect.any(Function))]);
  });
});

describe('fromCallbag', () => {
  it('converts a Callbag to a Wonka source', () => {
    const source = callbag.fromCallbag(callbagFromArray([1, 2]) as any);
    const signals = collectSignals(source);

    expect(signals).toEqual([start(expect.any(Function)), push(1), push(2), SignalKind.End]);
  });

  it('supports cancellation on converted Observables', () => {
    const source = callbag.fromCallbag(callbagFromArray([1, 2]) as any);
    const signals = collectSignals(source, talkback => {
      talkback(TalkbackKind.Close);
    });

    expect(signals).toEqual([start(expect.any(Function))]);
  });
});

describe('interval', () => {
  it('emits Push signals until Cancel is sent', () => {
    let pushes = 0;
    let talkback: TalkbackFn | null = null;

    const sink: Sink<any> = signal => {
      if (signal === SignalKind.End) {
        /*noop*/
      } else if (signal.tag === SignalKind.Push) {
        pushes++;
      } else {
        talkback = signal[0];
      }
    };

    sources.interval(100)(sink);
    expect(talkback).not.toBe(null);
    expect(pushes).toBe(0);

    vi.advanceTimersByTime(100);
    expect(pushes).toBe(1);
    vi.advanceTimersByTime(100);
    expect(pushes).toBe(2);

    talkback!(TalkbackKind.Close);
    vi.advanceTimersByTime(100);
    expect(pushes).toBe(2);
  });
});

describe('fromDomEvent', () => {
  it('emits Push signals for events on a DOM element', () => {
    let talkback: TalkbackFn | null = null;

    const element = {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    };

    const sink: Sink<any> = signal => {
      expect(signal).not.toBe(SignalKind.End);
      if ((signal as any).tag === SignalKind.Start) talkback = signal[0];
    };

    sources.fromDomEvent(element as any, 'click')(sink);

    expect(element.addEventListener).toHaveBeenCalledWith('click', expect.any(Function));
    expect(element.removeEventListener).not.toHaveBeenCalled();
    const listener = element.addEventListener.mock.calls[0][1];

    listener(1);
    listener(2);
    talkback!(TalkbackKind.Close);
    expect(element.removeEventListener).toHaveBeenCalledWith('click', listener);
  });
});
